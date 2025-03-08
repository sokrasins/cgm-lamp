pub mod wifi {
    use crate::storage::storage::Storable;
    use embedded_svc::wifi::{
        AccessPointConfiguration, AuthMethod, ClientConfiguration, Configuration,
    };
    use esp_idf_hal::modem::Modem;
    use esp_idf_svc::eventloop::EspEventLoop;
    use esp_idf_svc::eventloop::System;
    use esp_idf_svc::mdns::EspMdns;
    use esp_idf_svc::nvs::{EspNvsPartition, NvsDefault};
    use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
    use serde::{Deserialize, Serialize};

    use log::info;

    const MAX_TRY_COUNT: usize = 3;
    const AP_SSID: &str = "CGM-LAMP";
    const MDNS_HOSTNAME: &str = "cgmlamp";

    pub struct Wifi<'a> {
        wifi: BlockingWifi<EspWifi<'a>>,
        #[allow(dead_code)]
        mdns: EspMdns,
        ap_ssid: Option<String>,
        ap_psk: Option<String>,
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
            mdns.set_hostname(MDNS_HOSTNAME).unwrap();
            mdns.set_instance_name("Glucose Monitoring Lamp").unwrap();
            mdns.add_service(None, "_http", "_tcp", 80, &[("", "")])
                .unwrap();
            mdns.set_service_instance_name("_http", "_tcp", "Glucose Monitoring Lamp")
                .unwrap();

            Ok(Wifi {
                wifi,
                mdns,
                ap_ssid: None,
                ap_psk: None,
            })
        }

        pub fn has_creds(&self) -> bool {
            if self.ap_ssid == None {
                return false;
            }

            if self.ap_psk == None {
                return false;
            }

            return true;
        }

        pub fn reset_creds(&mut self) {
            self.ap_ssid = None;
            self.ap_psk = None;
        }

        pub fn start_sta(&mut self) -> anyhow::Result<()> {
            let ssid = self.ap_ssid.clone().unwrap();
            let pass = self.ap_psk.clone().unwrap();

            let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
                ssid: ssid.as_str().try_into().unwrap(),
                bssid: None,
                auth_method: AuthMethod::WPA2Personal,
                password: pass.as_str().try_into().unwrap(),
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
            info!(
                "Wifi connected, available on {}",
                format!("{}.local", MDNS_HOSTNAME)
            );

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
            info!(
                "Wifi started, ssid {}, available on {}",
                AP_SSID,
                format!("{}.local", MDNS_HOSTNAME)
            );

            Ok(())
        }
    }

    #[derive(Serialize, Deserialize)]
    struct NvsWifiState {
        ap_ssid: Option<String>,
        ap_psk: Option<String>,
    }

    impl<'a> Storable for Wifi<'a> {
        fn store_tag(&self) -> &str {
            return &"wifi_credentials";
        }

        fn store_data(&self) -> Vec<u8> {
            let data = NvsWifiState {
                ap_ssid: self.ap_ssid.to_owned(),
                ap_psk: self.ap_psk.to_owned(),
            };

            serde_json::to_string(&data).unwrap().into_bytes()
        }

        fn recall_data(&mut self, data: &[u8]) {
            let nvs_state = serde_json::from_slice::<NvsWifiState>(data).unwrap();
            self.ap_ssid = nvs_state.ap_ssid;
            self.ap_psk = nvs_state.ap_psk;
        }
    }
}
