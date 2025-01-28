pub mod settings {
    use serde::{Deserialize, Serialize};

    pub trait Observer {
        fn update(&self, state: &AppSettings);
    }

    pub trait Subject<'a> {
        fn attach(&mut self, observer: &'a dyn Observer);
        fn detach(&mut self, observer: &'a dyn Observer);
        fn notify_observers(&self);
    }

    pub struct Store<'a> {
        settings: AppSettings,
        observers: Vec<&'a dyn Observer>,
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
    }

    impl<'a> Store<'a> {
        pub fn new() -> Store<'a> {
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
            if self.settings.merge(delta) {
                self.save_to_flash().unwrap();
            }; 

            // TODO: Trigger on_change callbacks
            self.notify_observers();
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
            for item in self.observers.iter() {
                item.update(&self.settings);
            }
        }
    }
}
