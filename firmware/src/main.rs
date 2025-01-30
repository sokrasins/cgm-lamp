use core::convert::TryInto;

use log::info;

use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};
use esp_idf_svc::hal::{delay::FreeRtos, peripherals::Peripherals};
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};
use std::time::{SystemTime, UNIX_EPOCH};

use cgmlamp::dexcom::dexcom::Dexcom;
use cgmlamp::lamp::lamp::Lamp;
use cgmlamp::lamp::lamp::{LedState, WHITE, YELLOW};
use cgmlamp::server::server::Server;
use cgmlamp::settings::settings::{Observer, Store};
use cgmlamp::storage::storage::Storage;

// Credentials stored in config file
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

// Application state machine states
enum AppState {
    Boot,
    ConnectWifi,
    GetSession,
    DisplayGlucose,
}

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    EspLogger::initialize_default();

    // Setup ESP-type stuff
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // Application state
    let mut store = Store::new();
    let mut storage = Storage::new(&nvs);

    // Load the hard-coded credentials into the store
    {
        // Get a transmit channel for settings, just for this scope
        let tx_channel = store.tx_channel();

        // Get settings stored in flash
        let settings = storage.recall().unwrap();

        // Take flash settings into current state
        tx_channel.send(settings).unwrap();
        store.check_updates();
    }

    // App state
    let mut app_state = AppState::Boot;

    // Create dexcom object
    let mut dexcom = Dexcom::new();

    // Monitor glucose
    let mut no_measurement_count = 0;

    let mut lamp = Lamp::new();
    lamp.start(peripherals.pins.gpio8, peripherals.rmt.channel0)?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    let mut server = Server::new(&store);
    let mut last_query: u64 = 0;
    const QUERY_INTERVAL: u64 = 20;

    loop {
        // Get time now. Adding the interval will make the first measurement
        // happen immediately.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
            + QUERY_INTERVAL;

        // Ingest settings updates
        // Define closure to be explicit about lock lifetime
        {
            store.check_updates();
            let settings = store.settings();
            let settings = settings.lock().unwrap();
            lamp.update(&settings);
            storage.update(&settings);
        }

        match app_state {
            AppState::Boot => {
                // Update presentation
                lamp.set_color(LedState::Breathe(WHITE));

                // Advance to next state
                app_state = AppState::ConnectWifi;
            }
            AppState::ConnectWifi => {
                // Set up wifi, connect to AP
                // TODO: Check for wifi credentials
                let settings_guard = store.settings();
                let settings_guard = settings_guard.lock().unwrap();

                // TODO: If connection fails too many times, open in AP mode?
                connect_wifi(
                    &mut wifi,
                    &(*settings_guard).ap_ssid.as_ref().unwrap(),
                    &(*settings_guard).ap_pass.as_ref().unwrap(),
                )?;

                core::mem::drop(settings_guard);

                // Start the http server
                server.start().unwrap();

                // Advance to next state
                app_state = AppState::GetSession;
            }
            AppState::GetSession => {
                // TODO: Check for dexcom credentials
                let settings_guard = store.settings();
                let settings_guard = settings_guard.lock().unwrap();

                dexcom
                    .connect(
                        &(*settings_guard).dexcom_user.as_ref().unwrap(),
                        &(*settings_guard).dexcom_pass.as_ref().unwrap(),
                    )
                    .unwrap();

                core::mem::drop(settings_guard);

                app_state = AppState::DisplayGlucose;
            }
            AppState::DisplayGlucose => {
                if now > (last_query + QUERY_INTERVAL) {
                    // Update last
                    info!("{}: getting latest glucose", now);
                    last_query = now;
                    no_measurement_count += 1;

                    // Are we still connected to wifi? If not, sending a request will crash the program
                    if !wifi.is_connected().unwrap() {
                        info!("Not connected to wifi, reconnecting");
                        app_state = AppState::ConnectWifi;
                    } else {
                        // Get new reading
                        if let Ok(measurement) = dexcom.get_latest_glucose() {
                            info!("{:?}", measurement);

                            lamp.set_color(LedState::from_glucose(measurement.value));
                            no_measurement_count = 0;
                        } else if no_measurement_count >= 600 {
                            lamp.set_color(LedState::Breathe(YELLOW));
                        }
                    }
                }
            }
        };

        // 100 ms delay to let rtos do some work
        FreeRtos::delay_ms(100);
    }
}

fn connect_wifi(
    wifi: &mut BlockingWifi<EspWifi<'static>>,
    ssid: &str,
    pass: &str,
) -> anyhow::Result<()> {
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
    info!("Wifi started, connecting to {}", ssid);

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
