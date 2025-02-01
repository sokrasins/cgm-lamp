use log::info;
use std::time::{SystemTime, UNIX_EPOCH};

use esp_idf_svc::hal::{delay::FreeRtos, peripherals::Peripherals};
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};

use cgmlamp::dexcom::dexcom::Dexcom;
use cgmlamp::lamp::lamp::Lamp;
use cgmlamp::lamp::lamp::{LedState, WHITE};
use cgmlamp::server::server::Server;
use cgmlamp::settings::settings::{Observer, Store};
use cgmlamp::wifi::wifi::Wifi;

// Application state machine states
enum AppState {
    Boot,
    PresentAp,
    WaitForConfig,
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
    let mut store = Store::new(&nvs);
    store.load_from_flash();

    // App state
    let mut app_state = AppState::Boot;

    // Create dexcom object
    let mut dexcom = Dexcom::new();

    let mut lamp = Lamp::new();
    lamp.start(peripherals.pins.gpio8, peripherals.rmt.channel0)?;

    let mut wifi = Wifi::new(peripherals.modem, &sys_loop, &nvs).unwrap();

    let mut server = Server::new(&store);

    let mut no_measurement_count = 0;
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
                wifi.start_ap().unwrap();

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

                match wifi.start_sta(
                    &(*settings_guard).ap_ssid.as_ref().unwrap(),
                    &(*settings_guard).ap_psk.as_ref().unwrap(),
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

                // TODO: Check for valid dexcom credentials

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
                    if !wifi.is_connected() {
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
