pub mod settings {
    use crate::storage::storage::Storable;
    use log::info;
    use serde::{Deserialize, Serialize};
    use std::sync::mpsc;
    use std::sync::mpsc::{Receiver, Sender};

    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    pub struct AppSettings {
        pub ap_ssid: Option<String>,
        pub ap_psk: Option<String>,
        pub dexcom_user: Option<String>,
        pub dexcom_pass: Option<String>,
    }

    impl AppSettings {
        pub fn new() -> Self {
            AppSettings {
                ap_ssid: None,
                ap_psk: None,
                dexcom_user: None,
                dexcom_pass: None,
            }
        }

        pub fn merge(&mut self, delta: &AppSettings) -> bool {
            let mut changed = false;

            if let Some(ap_ssid) = &delta.ap_ssid {
                self.ap_ssid = Some(ap_ssid.to_owned());
                changed = true;
            }
            if let Some(ap_psk) = &delta.ap_psk {
                self.ap_psk = Some(ap_psk.to_owned());
                changed = true;
            }
            if let Some(dexcom_user) = &delta.dexcom_user {
                self.dexcom_user = Some(dexcom_user.to_owned());
                changed = true;
            }
            if let Some(dexcom_pass) = &delta.dexcom_pass {
                self.dexcom_pass = Some(dexcom_pass.to_owned());
                changed = true;
            }

            return changed;
        }

        pub fn has_wifi_creds(&self) -> bool {
            if self.ap_ssid.as_ref().and(self.ap_psk.as_ref()) == None {
                return false;
            }
            true
        }

        pub fn has_dexcom_creds(&self) -> bool {
            if self.dexcom_user.as_ref().and(self.dexcom_pass.as_ref()) == None {
                return false;
            }
            true
        }
    }

    #[derive(Debug)]
    pub enum SettingsAction {
        Set(AppSettings),
        Reset,
    }

    pub struct Store {
        settings: AppSettings,
        rx_channel: Receiver<SettingsAction>,
        tx_channel: Sender<SettingsAction>,
    }

    impl Store {
        pub fn new() -> Store {
            let (tx_channel, rx_channel) = mpsc::channel::<SettingsAction>();

            Store {
                settings: AppSettings::new(),
                rx_channel,
                tx_channel,
            }
        }

        pub fn set(&mut self, delta: &AppSettings) {
            self.settings.merge(delta);
            // TODO: Replace saving to flash
        }

        pub fn reset_wifi_creds(&mut self) {
            self.settings.ap_ssid = None;
            self.settings.ap_psk = None;
            // TODO: Save to flash
        }

        pub fn reset_dexcom_creds(&mut self) {
            self.settings.dexcom_user = None;
            self.settings.dexcom_pass = None;
            // TODO: Save to flash
        }

        pub fn check_updates(&mut self) {
            if let Ok(change) = self.rx_channel.try_recv() {
                info!("Update found: {:?}", change);
                match change {
                    SettingsAction::Set(settings) => self.set(&settings),
                    SettingsAction::Reset => info!("ERROR: Reset unimplemented"),
                }
            }
        }

        pub fn settings(&self) -> AppSettings {
            self.settings.clone()
        }

        pub fn tx_channel(&self) -> Sender<SettingsAction> {
            self.tx_channel.clone()
        }
    }

    impl Storable for Store {
        fn store_tag(&self) -> &str {
            return &"credentials";
        }

        fn store_data(&self) -> Vec<u8> {
            serde_json::to_string(&self.settings).unwrap().into_bytes()
        }

        fn recall_data(&mut self, data: &[u8]) {
            let nvs_state = serde_json::from_slice::<AppSettings>(data).unwrap();
            self.settings.merge(&nvs_state);
        }
    }
}
