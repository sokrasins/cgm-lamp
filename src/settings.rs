pub mod settings {
    pub struct AppSettings {
        wifi_ssid: Option<String>,
        wifi_pass: Option<String>,
        dexcom_user: Option<String>,
        dexcom_pass: Option<String>,
        lamp_brightness: Option<usize>,
    }

    impl AppSettings {
        pub fn new() -> Self {
            AppSettings {
                wifi_ssid: None,
                wifi_pass: None,
                dexcom_user: None,
                dexcom_pass: None,
                lamp_brightness: None,
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
            if let Some(wifi_ssid) = &delta.wifi_ssid {
                self.wifi_ssid = Some(wifi_ssid.to_owned());
                changed = true;
            }
            if let Some(wifi_pass) = &delta.wifi_pass {
                self.wifi_pass = Some(wifi_pass.to_owned());
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
