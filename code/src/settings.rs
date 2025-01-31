pub mod settings {
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

    pub struct Store<'a> {
        settings: Arc<Mutex<AppSettings>>,
        observers: Vec<&'a dyn Observer>,
        rx_channel: Receiver<SettingsAction>,
        tx_channel: Sender<SettingsAction>,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    pub struct AppSettings {
        pub ap_ssid: Option<String>,
        pub ap_pass: Option<String>,
        pub dexcom_user: Option<String>,
        pub dexcom_pass: Option<String>,
        pub lamp_brightness: Option<f32>,
    }

    impl AppSettings {
        pub fn new() -> Self {
            AppSettings {
                ap_ssid: None,
                ap_pass: None,
                dexcom_user: None,
                dexcom_pass: None,
                lamp_brightness: Some(0.25),
            }
        }

        pub fn merge(&mut self, delta: &AppSettings) -> bool {
            let mut changed = false;

            if let Some(ap_ssid) = &delta.ap_ssid {
                self.ap_ssid = Some(ap_ssid.to_owned());
                changed = true;
            }
            if let Some(ap_pass) = &delta.ap_pass {
                self.ap_pass = Some(ap_pass.to_owned());
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
            if let Some(lamp_brightness) = &delta.lamp_brightness {
                self.lamp_brightness = Some(*lamp_brightness);
                changed = true;
            }

            return changed;
        }

        pub fn has_wifi_creds(&self) -> bool {
            if self.ap_ssid.as_ref().and(self.ap_pass.as_ref()) == None {
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
        Modify(AppSettings),
        Reset,
    }

    impl<'a> Store<'a> {
        pub fn new() -> Store<'a> {
            let (tx_channel, rx_channel) = mpsc::channel::<SettingsAction>();
            Store {
                settings: Arc::new(Mutex::new(AppSettings::new())),
                observers: Vec::new(),
                rx_channel,
                tx_channel,
            }
        }

        // TODO: add NVS
        pub fn load_from_flash(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        // TODO: add NVS
        pub fn save_to_flash(&self) -> anyhow::Result<()> {
            Ok(())
        }

        pub fn modify(&mut self, delta: &AppSettings) {
            let mut settings = self.settings.lock().unwrap();
            if (*settings).merge(delta) {
                self.save_to_flash().unwrap();
            };
            std::mem::drop(settings);

            // Trigger on_change callbacks
            self.notify_observers();
        }

        pub fn reset(&mut self) {
            let mut settings = self.settings.lock().unwrap();
            *settings = AppSettings::new();
            std::mem::drop(settings);
            self.save_to_flash().unwrap();
        }

        pub fn check_updates(&mut self) {
            if let Ok(change) = self.rx_channel.try_recv() {
                info!("Update found: {:?}", change);
                match change {
                    SettingsAction::Modify(settings) => self.modify(&settings),
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
