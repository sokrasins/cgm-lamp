pub mod settings {
    use serde::{Deserialize, Serialize};

    pub trait Observer {
        fn update(&self, state: &AppSettings);
    }

    pub trait Subject<'a, T: Observer> {
        fn attach(&mut self, observer: &'a T);
        fn detach(&mut self, observer: &'a T);
        fn notify_observers(&self);
    }

    pub struct Store<'a, T: Observer> {
        settings: AppSettings,
        observers: Vec<&'a T>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct AppSettings {
        pub ap_ssid: Option<String>,
        pub ap_pass: Option<String>,
        pub dexcom_user: Option<String>,
        pub dexcom_pass: Option<String>,
        pub lamp_brightness: Option<usize>,
    }

    impl AppSettings {
        pub fn new() -> Self {
            AppSettings {
                ap_ssid: None,
                ap_pass: None,
                dexcom_user: None,
                dexcom_pass: None,
                lamp_brightness: None,
            }
        }

        pub fn merge(&mut self, delta: &AppSettings) {}
    }

    impl<'a, T: Observer + PartialEq> Store<'a, T> {
        fn new() -> Store<'a, T> {
            Store {
                settings: AppSettings::new(),
                observers: Vec::new(),
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
            let mut changed = false;
            // Check for settings parameters in delta, and take in the new
            // settings that exist
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

            if changed {
                self.save_to_flash().unwrap();
            }

            // TODO: Trigger on_change callbacks
        }

        // TODO: enable registering callbacks to detect settings changes
        pub fn on_change_cb(&self) {}
    }
}
