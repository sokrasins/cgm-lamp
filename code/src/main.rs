use log::info;

use esp_idf_svc::hal::{delay::FreeRtos, peripherals::Peripherals};
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};

use esp_idf_hal::gpio::PinDriver;

use cgmlamp::dexcom::dexcom::Dexcom;
use cgmlamp::dimmer::dimmer::LightDimmer;
use cgmlamp::lamp::lamp::Lamp;
use cgmlamp::lamp::lamp::{LedState, WHITE};
use cgmlamp::power::power::Power;
use cgmlamp::server::server::ServableData;
use cgmlamp::server::server::Server;
use cgmlamp::storage::storage::Storage;
use cgmlamp::sys::sys::{uptime, Sys};
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

    // System module
    let indicator_pin = PinDriver::output(peripherals.pins.gpio5)?;
    let mut sys = Sys::new(indicator_pin, peripherals.temp_sensor);

    // Show the ESP has started
    sys.ind_on();

    // Non-volatile State
    let mut storage = Storage::new(&nvs);

    // App state
    let mut app_state = AppState::Boot;

    // Fuel Gauge
    let i2c = peripherals.i2c0;
    let sda = peripherals.pins.gpio6;
    let scl = peripherals.pins.gpio7;
    let bat_charge_pin = PinDriver::input(peripherals.pins.gpio4)?;
    let mut power = Power::new(i2c, sda, scl, bat_charge_pin).unwrap();

    // Create dexcom object
    let mut dexcom = Dexcom::new();
    storage.recall(&mut dexcom).unwrap_or_else(|error| {
        info!("Couldn't load dexcom settings from flash: {}", error);
    });

    let mut lamp = Lamp::new(peripherals.pins.gpio8, peripherals.rmt.channel0);
    storage.recall(&mut lamp).unwrap_or_else(|error| {
        info!("Couldn't load lamp settings from flash: {}", error);
    });

    let mut wifi = Wifi::new(peripherals.modem, &sys_loop, &nvs).unwrap();
    storage.recall(&mut wifi).unwrap_or_else(|error| {
        info!("Couldn't load wifi settings from flash: {}", error);
    });

    // Instantiate server and register anything that provides data to the server
    let mut server = Server::new();
    server.add_data_channel(&mut lamp);
    server.add_data_channel(&mut wifi);
    server.add_data_channel(&mut dexcom);
    server.add_data_channel(&mut power);
    server.add_data_channel(&mut sys);

    let mut no_measurement_count = 0;
    let mut last_query: u64 = 0;
    const QUERY_INTERVAL: u64 = 20;

    // Set up encoder
    let mut pin_a = peripherals.pins.gpio18;
    let mut pin_b = peripherals.pins.gpio19;
    let encoder_button = PinDriver::input(peripherals.pins.gpio11)?;
    let mut dimmer = LightDimmer::new(peripherals.pcnt0, &mut pin_a, &mut pin_b)?;

    let mut last_button_state = true;

    loop {
        // Get time now. Adding the interval will make the first measurement
        // happen immediately.
        let now = uptime() + QUERY_INTERVAL;

        // Check for encoder change and update brightness
        let bright_change = dimmer.get_change();
        if bright_change != 0 {
            info!("change brightness by: {bright_change}");
            lamp.change_brightness(4 * bright_change);
        }

        // Check for button change and toggle brightness
        let button_state = encoder_button.is_high();
        if last_button_state != button_state {
            if button_state == false {
                info!("Button pushed, toggling lamp");
                lamp.toggle();
            }
            last_button_state = button_state;
        }

        // Let each object that has server-relevant data handle any server requests
        lamp.handle_server_req();
        wifi.handle_server_req();
        dexcom.handle_server_req();
        power.handle_server_req();
        sys.handle_server_req();

        // Let each object that needs to store data do so
        if wifi.need_to_save() {
            storage.store(&mut wifi).unwrap();
            wifi.saved();
        }

        if dexcom.need_to_save() {
            storage.store(&mut dexcom).unwrap();
            dexcom.saved();
        }

        if lamp.need_to_save() {
            storage.store(&mut lamp).unwrap();
            lamp.saved();
        }

        match app_state {
            AppState::Boot => {
                // Update presentation
                lamp.set_color(LedState::Steady(WHITE));

                if wifi.has_creds() && dexcom.has_creds() {
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
                if wifi.has_creds() && dexcom.has_creds() {
                    server.stop();
                    lamp.set_color(LedState::Steady(WHITE));
                    app_state = AppState::ConnectWifi;
                }
            }
            AppState::ConnectWifi => {
                // Set up wifi, connect to AP
                match wifi.start_sta() {
                    Ok(_) => {
                        // Start the http server
                        info!("Wifi connected, starting web interface");
                        server.start().unwrap();
                        app_state = AppState::GetSession;
                    }
                    Err(_) => {
                        // If connection fails too many times, open in AP mode
                        info!("Couldn't connect to wifi, launching AP mode for AP credentials");
                        wifi.reset_creds();
                        app_state = AppState::PresentAp;
                    }
                }
            }
            AppState::GetSession => {
                dexcom.connect().unwrap();

                // TODO: Check for valid dexcom credentials

                app_state = AppState::DisplayGlucose;
            }
            AppState::DisplayGlucose => {
                if !wifi.has_creds() || !dexcom.has_creds() {
                    server.stop();
                    app_state = AppState::PresentAp;
                } else if now > (last_query + QUERY_INTERVAL) {
                    let soc = power.batt_charge().unwrap();
                    let voltage = power.batt_voltage().unwrap();
                    let charge_rate = power.batt_charge_rate().unwrap();
                    info!("Charge: {:.2}%", soc);
                    info!("Charge Rate: {:.2}%", charge_rate);
                    info!("Voltage: {:.2}V", voltage);
                    info!("Battery charging: {}", power.batt_charging());

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
        FreeRtos::delay_ms(10);
    }
}
