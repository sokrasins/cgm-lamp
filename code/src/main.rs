use core::convert::TryInto;

use log::info;

use embedded_svc::wifi::{
    AccessPointConfiguration, AuthMethod, ClientConfiguration, Configuration,
};
use esp_idf_svc::hal::{delay::FreeRtos, peripherals::Peripherals};
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::mdns::EspMdns;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};
use std::time::{SystemTime, UNIX_EPOCH};

use cgmlamp::dexcom::dexcom::Dexcom;
use cgmlamp::lamp::lamp::Lamp;
use cgmlamp::lamp::lamp::{LedState, WHITE};
use cgmlamp::server::server::Server;
use cgmlamp::settings::settings::{Observer, SettingsAction, Store};
use cgmlamp::storage::storage::Storage;

// Application state machine states
enum AppState {
    Boot,
    PresentAp,
    WaitForConfig,
    ConnectWifi,
    GetSession,
    DisplayGlucose,
}

const MAX_TRY_COUNT: usize = 3;

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
        tx_channel.send(SettingsAction::Modify(settings)).unwrap();
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

            // If credentials were lost at some point then ask for new ones via AP
            //if !settings.has_wifi_creds() || !settings.has_dexcom_creds() {
            //    app_state = AppState::PresentAp;
            //}
        }

        match app_state {
            AppState::Boot => {
                // Update presentation
                lamp.set_color(LedState::Steady(WHITE));

                let settings_guard = store.settings();
                let settings_guard = settings_guard.lock().unwrap();

                if (*settings_guard).has_wifi_creds() && (*settings_guard).has_dexcom_creds() {
                    app_state = AppState::ConnectWifi;
                } else {
                    // Advance to next state
                    app_state = AppState::PresentAp;
                }
            }
            AppState::PresentAp => {
                launch_ap(&mut wifi)?;

                let mut mdns = EspMdns::take().unwrap();
                mdns.set_hostname("cgm-lamp").unwrap();
                mdns.set_instance_name("Glucose Monitoring Lamp").unwrap();
                mdns.add_service(None, "_http", "_tcp", 80, &[("", "")])
                    .unwrap();
                mdns.set_service_instance_name("_http", "_tcp", "Glucose Monitoring Lamp")
                    .unwrap();
                core::mem::forget(mdns);

                server.start().unwrap();

                lamp.set_color(LedState::Breathe(WHITE));
                app_state = AppState::WaitForConfig;
            }
            AppState::WaitForConfig => {
                let settings_guard = store.settings();
                let settings_guard = settings_guard.lock().unwrap();

                if (*settings_guard).has_wifi_creds() && (*settings_guard).has_dexcom_creds() {
                    server.stop();
                    lamp.set_color(LedState::Steady(WHITE));
                    app_state = AppState::ConnectWifi;
                }
            }
            AppState::ConnectWifi => {
                // Set up wifi, connect to AP
                let settings_guard = store.settings();
                let settings_guard = settings_guard.lock().unwrap();

                match connect_wifi(
                    &mut wifi,
                    &(*settings_guard).ap_ssid.as_ref().unwrap(),
                    &(*settings_guard).ap_pass.as_ref().unwrap(),
                ) {
                    Ok(_) => {
                        // Start the http server
                        core::mem::drop(settings_guard);
                        info!("Wifi connected, starting web interface");
                        server.start().unwrap();
                        app_state = AppState::GetSession;
                    }
                    Err(_) => {
                        // If connection fails too many times, open in AP mode
                        info!("Couldn't connect to wifi, launching AP mode for AP credentials");
                        core::mem::drop(settings_guard);
                        store.reset_wifi_creds();
                        app_state = AppState::PresentAp;
                    }
                }
            }
            AppState::GetSession => {
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
                let settings_guard = store.settings();
                let settings_guard = settings_guard.lock().unwrap();

                if !(*settings_guard).has_wifi_creds() || !(*settings_guard).has_dexcom_creds() {
                    server.stop();
                    app_state = AppState::PresentAp;
                } else if now > (last_query + QUERY_INTERVAL) {
                    // Update last
                    info!("{}: getting latest glucose", now);
                    last_query = now;
                    no_measurement_count += 1;

                    // Are we still connected to wifi? If not, sending a request will crash the program
                    if !wifi.is_connected().unwrap() {
                        // TODO: Not enough to prevent a crash when radio -> init
                        info!("Not connected to wifi, reconnecting");
                        server.stop();
                        app_state = AppState::ConnectWifi;
                    } else {
                        // Get new reading
                        if let Ok(measurement) = dexcom.get_latest_glucose() {
                            info!("{:?}", measurement);

                            lamp.set_color(LedState::from_glucose(measurement.value));
                            no_measurement_count = 0;
                        } else if no_measurement_count >= 600 {
                            lamp.set_color(LedState::Steady(WHITE));
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

    let mut tries = 0;

    loop {
        match wifi.connect() {
            Ok(_) => break,
            Err(e) => {
                if tries > MAX_TRY_COUNT {
                    anyhow::bail!(
                        "Number of wifi connection attempts exceeds limit: {}",
                        tries
                    );
                } else {
                    info!("Error connecting to WIFI: ({}). retrying", e.to_string());
                }
            }
        };
        tries += 1;
    }
    info!("Wifi connected");

    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    Ok(())
}

fn launch_ap(wifi: &mut BlockingWifi<EspWifi<'static>>) -> anyhow::Result<()> {
    let ssid = "CGM-Lamp";
    let wifi_configuration: Configuration = Configuration::AccessPoint(AccessPointConfiguration {
        ssid: ssid.try_into().unwrap(),
        auth_method: AuthMethod::None,
        channel: 11,
        ..Default::default()
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start()?;
    info!("Wifi started, setting up ssid {}", ssid);

    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    /*let mut mdns = EspMdns::take().unwrap();
    mdns.set_hostname("cgm-lamp").unwrap();
    mdns.set_instance_name("Glucose Monitoring Lamp").unwrap();
    mdns.add_service(None, "_http", "_tcp", 80, &[("", "")])
        .unwrap();
    mdns.set_service_instance_name("_http", "_tcp", "Glucose Monitoring Lamp")
        .unwrap();*/
    Ok(())
}
