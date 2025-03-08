pub mod settings {
    use crate::storage::storage::Storage;
    use esp_idf_svc::nvs::{EspNvsPartition, NvsDefault};
    use log::info;
    use serde::{Deserialize, Serialize};
    use std::sync::mpsc;
    use std::sync::mpsc::{Receiver, Sender};
    use std::sync::{Arc, Mutex};

    pub trait Observer {
        fn update(&mut self, state: &AppSettings) -> bool;
    }

    pub trait Subject<'a> {
        fn attach(&mut self, observer: &'a dyn Observer);
        fn detach(&mut self, observer: &'a dyn Observer);
        fn notify_observers(&self);
    }

    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    pub struct AppSettings {
        pub ap_ssid: Option<String>,
        pub ap_psk: Option<String>,
        pub dexcom_user: Option<String>,
        pub dexcom_pass: Option<String>,
        pub brightness: Option<u8>,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    pub struct AppSettingsDiff {
        pub brightness: Option<i32>,
    }

    impl AppSettings {
        pub fn new() -> Self {
            AppSettings {
                ap_ssid: None,
                ap_psk: None,
                dexcom_user: None,
                dexcom_pass: None,
                brightness: Some(64),
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
            if let Some(brightness) = &delta.brightness {
                self.brightness = Some(*brightness);
                changed = true;
            }

            return changed;
        }

        pub fn modify(&mut self, delta: &AppSettingsDiff) -> bool {
            let mut changed = false;
            if let Some(bright) = &delta.brightness {
                if let Some(cur_bright) = self.brightness {
                    let mut new_brightness: i32 = (cur_bright as i32) + bright;
                    if new_brightness > 255 {
                        new_brightness = 255;
                    }

                    if new_brightness < 0 {
                        new_brightness = 0;
                    }

                    self.brightness = Some(new_brightness as u8);
                    changed = true;
                }
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

    impl AppSettingsDiff {
        pub fn new() -> Self {
            Self { brightness: None }
        }

        pub fn set_brightness_diff(&mut self, bright_change: i32) {
            self.brightness = Some(bright_change);
        }
    }

    pub struct Store<'a> {
        settings: Arc<Mutex<AppSettings>>,
        observers: Vec<&'a dyn Observer>,
        storage: Storage,
        rx_channel: Receiver<SettingsAction>,
        tx_channel: Sender<SettingsAction>,
    }

    impl<'a> Store<'a> {
        pub fn new(nvs_part: &EspNvsPartition<NvsDefault>) -> Store<'a> {
            let (tx_channel, rx_channel) = mpsc::channel::<SettingsAction>();
            let storage = Storage::new(nvs_part);
            Store {
                settings: Arc::new(Mutex::new(AppSettings::new())),
                observers: Vec::new(),
                storage,
                rx_channel,
                tx_channel,
            }
        }

        pub fn load_from_flash(&mut self) {
            let settings = match self.storage.recall() {
                Ok(nvs_settings) => nvs_settings,
                Err(_) => AppSettings::new(),
            };
            self.set(&settings);
        }

        pub fn save_to_flash(&mut self) {
            let settings_lock = self.settings.lock().unwrap();
            self.storage.store(&settings_lock).unwrap();
        }

        pub fn set(&mut self, delta: &AppSettings) {
            {
                let mut settings = self.settings.lock().unwrap();
                let change_made = (*settings).merge(delta);
                std::mem::drop(settings);

                if change_made {
                    self.save_to_flash();
                }
            }

            // Trigger on_change callbacks
            //self.notify_observers();
        }

        pub fn modify(&mut self, delta: &AppSettingsDiff) {
            {
                let mut settings = self.settings.lock().unwrap();
                let change_made = (*settings).modify(delta);
                std::mem::drop(settings);

                if change_made {
                    self.save_to_flash();
                }
            }

            // Trigger on_change callbacks
            //self.notify_observers();
        }

        pub fn reset(&mut self) {
            let mut settings = self.settings.lock().unwrap();
            *settings = AppSettings::new();
            std::mem::drop(settings);
            self.save_to_flash();
        }

        pub fn reset_wifi_creds(&mut self) {
            let mut settings = self.settings.lock().unwrap();
            (*settings).ap_ssid = None;
            (*settings).ap_psk = None;
            std::mem::drop(settings);
            self.save_to_flash();
        }

        pub fn reset_dexcom_creds(&mut self) {
            let mut settings = self.settings.lock().unwrap();
            (*settings).dexcom_user = None;
            (*settings).dexcom_pass = None;
            std::mem::drop(settings);
            self.save_to_flash();
        }

        pub fn check_updates(&mut self) {
            if let Ok(change) = self.rx_channel.try_recv() {
                info!("Update found: {:?}", change);
                match change {
                    SettingsAction::Set(settings) => self.set(&settings),
                    SettingsAction::Reset => self.reset(),
                }
            }
        }

        pub fn settings(&self) -> Arc<Mutex<AppSettings>> {
            Arc::clone(&self.settings)
        }

        pub fn tx_channel(&self) -> Sender<SettingsAction> {
            self.tx_channel.clone()
        }
    }

    impl<'a> Subject<'a> for Store<'a> {
        fn attach(&mut self, observer: &'a dyn Observer) {
            self.observers.push(observer);
        }

        fn detach(&mut self, observer: &'a dyn Observer) {
            self.observers.retain(|o| !std::ptr::eq(*o, observer));
        }

        fn notify_observers(&self) {
            /*let settings = self.settings.lock().unwrap();
            for item in self.observers.iter() {
                item.update(&settings);
            }*/
        }
    }
}
