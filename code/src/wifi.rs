pub mod wifi {
    use embedded_svc::wifi::{
        AccessPointConfiguration, AuthMethod, ClientConfiguration, Configuration,
    };
    use esp_idf_hal::modem::Modem;
    use esp_idf_svc::eventloop::EspEventLoop;
    use esp_idf_svc::eventloop::System;
    use esp_idf_svc::mdns::EspMdns;
    use esp_idf_svc::nvs::{EspNvsPartition, NvsDefault};
    use esp_idf_svc::wifi::{BlockingWifi, EspWifi};

    use log::info;

    const MAX_TRY_COUNT: usize = 3;
    const AP_SSID: &str = "CGM-LAMP";

    pub struct Wifi<'a> {
        wifi: BlockingWifi<EspWifi<'a>>,
        #[allow(dead_code)]
        mdns: EspMdns,
    }

    impl<'a> Wifi<'a> {
        pub fn new(
            modem: Modem,
            sys_loop: &EspEventLoop<System>,
            nvs: &EspNvsPartition<NvsDefault>,
        ) -> anyhow::Result<Self> {
            let wifi = BlockingWifi::wrap(
                EspWifi::new(modem, sys_loop.clone(), Some(nvs.to_owned()))?,
                sys_loop.clone(),
            )?;

            let mut mdns = EspMdns::take().unwrap();
            mdns.set_hostname("cgm-lamp").unwrap();
            mdns.set_instance_name("Glucose Monitoring Lamp").unwrap();
            mdns.add_service(None, "_http", "_tcp", 80, &[("", "")])
                .unwrap();
            mdns.set_service_instance_name("_http", "_tcp", "Glucose Monitoring Lamp")
                .unwrap();

            Ok(Wifi { wifi, mdns })
        }

        pub fn start_sta(&mut self, ssid: &str, pass: &str) -> anyhow::Result<()> {
            let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
                ssid: ssid.try_into().unwrap(),
                bssid: None,
                auth_method: AuthMethod::WPA2Personal,
                password: pass.try_into().unwrap(),
                channel: None,
                ..Default::default()
            });

            self.wifi.set_configuration(&wifi_configuration)?;
            self.wifi.start()?;
            info!("Wifi started, connecting to {}", ssid);

            // Number of wifi connection attempts
            let mut tries = 0;

            loop {
                match self.wifi.connect() {
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

            self.wifi.wait_netif_up()?;
            info!("Wifi connected");

            Ok(())
        }

        pub fn is_connected(&self) -> bool {
            self.wifi.is_connected().unwrap()
        }

        pub fn start_ap(&mut self) -> anyhow::Result<()> {
            let wifi_configuration: Configuration =
                Configuration::AccessPoint(AccessPointConfiguration {
                    ssid: AP_SSID.try_into().unwrap(),
                    auth_method: AuthMethod::None,
                    channel: 11,
                    ..Default::default()
                });

            self.wifi.set_configuration(&wifi_configuration)?;
            self.wifi.start()?;
            self.wifi.wait_netif_up()?;
            info!("Wifi started, ssid {}", AP_SSID);

            Ok(())
        }
    }
}
