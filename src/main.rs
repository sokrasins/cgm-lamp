use log::{error, info};
use core::convert::TryInto;
use core::time::Duration;

use embedded_svc::{
    http::{client::Client},
    io::{Write},
    utils::io,
    wifi::{AuthMethod, ClientConfiguration, Configuration},
};

use esp_idf_svc::hal::{delay::FreeRtos, peripherals::Peripherals};
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};
use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};

use esp_idf_hal::{
    gpio::OutputPin,
    peripheral::Peripheral,
    rmt::{config::TransmitConfig, FixedLengthSignal, PinState, Pulse, RmtChannel, TxRmtDriver},
};

use serde::{Deserialize, Serialize};
use serde_json;

use rgb::RGB8;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DexcomLogin {
    account_name: String,
    password: String,
    application_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DexcomSession {
    account_id: String,
    password: String,
    application_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DexcomGlucose {
    session_id: String,
    minutes: isize,
    max_count: isize,
}

#[derive(Deserialize, Serialize, Debug)]
//#[serde(rename_all = "camelCase")]
struct DexcomGlucoseReading {
    WT: String,
    ST: String,
    DT: String,
    Value: isize,
    Trend: String,
}

#[toml_cfg::toml_config]
pub struct Config {
    #[default(" ")]
    wifi_ssid: &'static str,
    #[default(" ")]
    wifi_pass: &'static str,
    #[default(" ")]
    dexcom_user: &'static str,
    #[default(" ")]
    dexcom_pass: &'static str,
}

const DEXCOM_BASE_URL: &str = "https://share1.dexcom.com/ShareWebServices/Services";
const DEXCOM_APPLICATION_ID: &str = "d89443d2-327c-4a6f-89e5-496bbb0317db";

const DEXCOM_LOGIN_ID_ENDPOINT: &str = "General/LoginPublisherAccountById";
const DEXCOM_AUTHENTICATE_ENDPOINT: &str = "General/AuthenticatePublisherAccount";
const DEXCOM_GLUCOSE_READINGS_ENDPOINT: &str = "Publisher/ReadPublisherLatestGlucoseValues";

const DEXCOM_MAX_MAX_COUNT: isize = 288;

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    EspLogger::initialize_default();

    // The constant `CONFIG` is auto-generated by `toml_config`.
    let app_config = CONFIG;

    // Setup Wifi
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let led = peripherals.pins.gpio8;
    let channel = peripherals.rmt.channel0;
    let mut ws2812 = WS2812RMT::new(led, channel)?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    connect_wifi(&mut wifi, app_config.wifi_ssid, app_config.wifi_pass)?;

    // Make https client
    let connection = EspHttpConnection::new(&HttpConfiguration {
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        ..Default::default()
    })?;
    let mut client = Client::wrap(connection);

    //post_request(&mut client)?;

    // Get user id
    let login_ctx = DexcomLogin {
        account_name: app_config.dexcom_user.to_string(),
        password: app_config.dexcom_pass.to_string(),
        application_id: DEXCOM_APPLICATION_ID.into(),
    };
    let auth_url = format!("{DEXCOM_BASE_URL}/{DEXCOM_AUTHENTICATE_ENDPOINT}");
    let user_id_json = post(&mut client, &auth_url, &(serde_json::to_string(&login_ctx).unwrap())).unwrap();
    let user_id = serde_json::from_str(&user_id_json).unwrap();
    info!("user id: {}", user_id);

    // Login
    let session_ctx = DexcomSession {
        account_id: user_id,
        password: app_config.dexcom_pass.to_string(),
        application_id: DEXCOM_APPLICATION_ID.into(),
    };
    let login_url = format!("{DEXCOM_BASE_URL}/{DEXCOM_LOGIN_ID_ENDPOINT}");
    let session_json = post(&mut client, &login_url, &(serde_json::to_string(&session_ctx).unwrap())).unwrap();
    let session: String = serde_json::from_str(&session_json).unwrap();
    info!("session: {}", session);

    // Monitor glucose
    loop {
        let glucose_ctx = DexcomGlucose {
            session_id: session.clone(),
            minutes: 5,
            max_count: 1,
        };
        let glucose_url = format!("{DEXCOM_BASE_URL}/{DEXCOM_GLUCOSE_READINGS_ENDPOINT}");
        let glucose_json = post(&mut client, &glucose_url, &(serde_json::to_string(&glucose_ctx).unwrap())).unwrap();
        let glucose_readings: Vec<DexcomGlucoseReading> = serde_json::from_str(&glucose_json).unwrap();
        for glucose in &glucose_readings {
            info!("glucose: {} and {}", glucose.Value, glucose.Trend);
        }

        if glucose_readings.len() > 0 {
            let last_value = glucose_readings[0].Value;

            // Turn white for a bit just to signify a new sample
            let color = RGB8::new(128, 128, 128); 
            ws2812.set_pixel(color)?;
            FreeRtos::delay_ms(100);

            // Set color by glucose value
            let color = match last_value {
                0..100 => RGB8::new(255, 0, 0),
                100..200 => RGB8::new(0, 255, 0),
                200..300 => RGB8::new(0, 0, 255),
                _ => RGB8::new(128, 128, 0),
            };
            ws2812.set_pixel(color)?;
        }
        FreeRtos::delay_ms(1000 * 20);
    }

    Ok(())
}

fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>, ssid: &str, pass: &str) -> anyhow::Result<()> {

    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: ssid.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: pass.try_into().unwrap(),
        channel: None,
        ..Default::default()
    });


    wifi.set_configuration(&wifi_configuration)?;

    wifi.start()?;
    info!("Wifi started");

    loop {
        match wifi.connect() {
            Ok(_) => break,
            Err(e) => {
                info!("Error connecting to WIFI: ({}). retrying", e.to_string());
                continue;
            }
        };
    }
    info!("Wifi connected");

    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    Ok(())
}

fn post(client: &mut Client<EspHttpConnection>, url: &str, payload: &str) -> anyhow::Result<String> {
    let content_length_header = format!("{}", payload.len());
    let headers = [
        ("accept-encoding", "application/json"),
        ("content-type", "application/json"),
        ("content-length", &*content_length_header),
    ];

    let mut request = client.post(url, &headers)?;
    request.write_all(payload.as_bytes())?;
    request.flush()?;
    info!("-> POST {}", url);
    let mut response = request.submit()?;

    let status = response.status();
    info!("<- {}", status);
    let mut buf = [0u8; 4096];
    let bytes_read = io::try_read_full(&mut response, &mut buf).map_err(|e| e.0)?;
    info!("Read {} bytes", bytes_read);
    match std::str::from_utf8(&buf[0..bytes_read]) {
        Ok(body_string) => {
            info!(
                "Response body (truncated to {} bytes): {:?}",
                buf.len(),
                body_string
            );
            return Ok(body_string.to_owned())
        },
        Err(e) => error!("Error decoding response body: {}", e),
    };

    Ok("".to_owned())
}




pub struct WS2812RMT<'a> {
    tx_rtm_driver: TxRmtDriver<'a>,
}

impl<'d> WS2812RMT<'d> {
    // Rust ESP Board gpio2,  ESP32-C3-DevKitC-02 gpio8
    pub fn new(
        led: impl Peripheral<P = impl OutputPin> + 'd,
        channel: impl Peripheral<P = impl RmtChannel> + 'd,
    ) -> anyhow::Result<Self> {
        let config = TransmitConfig::new().clock_divider(2);
        let tx = TxRmtDriver::new(channel, led, &config)?;
        Ok(Self { tx_rtm_driver: tx })
    }

    pub fn set_pixel(&mut self, rgb: RGB8) -> anyhow::Result<()> {
        let color: u32 = ((rgb.g as u32) << 16) | ((rgb.r as u32) << 8) | rgb.b as u32;
        let ticks_hz = self.tx_rtm_driver.counter_clock()?;
        let t0h = Pulse::new_with_duration(ticks_hz, PinState::High, &ns(350))?;
        let t0l = Pulse::new_with_duration(ticks_hz, PinState::Low, &ns(800))?;
        let t1h = Pulse::new_with_duration(ticks_hz, PinState::High, &ns(700))?;
        let t1l = Pulse::new_with_duration(ticks_hz, PinState::Low, &ns(600))?;
        let mut signal = FixedLengthSignal::<24>::new();
        for i in (0..24).rev() {
            let p = 2_u32.pow(i);
            let bit = p & color != 0;
            let (high_pulse, low_pulse) = if bit { (t1h, t1l) } else { (t0h, t0l) };
            signal.set(23 - i as usize, &(high_pulse, low_pulse))?;
        }
        self.tx_rtm_driver.start_blocking(&signal)?;

        Ok(())
    }
}

fn ns(nanos: u64) -> Duration {
    Duration::from_nanos(nanos)
}
