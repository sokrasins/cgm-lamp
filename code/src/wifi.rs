pub mod wifi {
    use crate::server::server::{ServableData, ServableDataReq, ServableDataRsp, ServerData};
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
    use std::sync::mpsc;

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
        server_channel: Option<mpsc::Receiver<ServableDataReq>>,
        save_data: bool,
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
                server_channel: None,
                save_data: false,
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
            self.save_data = true;
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

        pub fn need_to_save(&self) -> bool {
            self.save_data
        }

        pub fn saved(&mut self) {
            self.save_data = false;
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
            self.save_data = false;
        }
    }

    impl<'a> ServableData for Wifi<'a> {
        fn get_channel(&mut self) -> mpsc::Sender<ServableDataReq> {
            let (tx, rx) = mpsc::channel::<ServableDataReq>();
            self.server_channel = Some(rx);
            tx
        }

        fn handle_server_req(&mut self) {
            if let Some(channel) = &self.server_channel {
                if let Ok(req) = channel.try_recv() {
                    info!("wifi got a request from server");

                    if let ServableDataReq::Get(back_channel) = &req {
                        info!("Sending wifi state to server");
                        let mut rsp = ServerData::new();
                        rsp.ap_ssid_stored = Some(self.ap_ssid.is_some());
                        rsp.ap_psk_stored = Some(self.ap_psk.is_some());
                        back_channel.send(ServableDataRsp::Data(rsp)).unwrap();
                    }

                    if let ServableDataReq::Set(update) = &req {
                        if let Some(ap_ssid) = &update.ap_ssid {
                            self.ap_ssid = Some(ap_ssid.clone());
                            self.save_data = true;
                        }

                        if let Some(ap_psk) = &update.ap_psk {
                            self.ap_psk = Some(ap_psk.clone());
                            self.save_data = true;
                        }
                    }
                }
            }
        }
    }
}
