pub mod storage {
    use crate::settings::settings::{AppSettings, Observer};
    use esp_idf_svc::nvs::*;
    use log::info;

    pub struct Storage {
        nvs: EspNvs<NvsDefault>,
    }

    impl Storage {
        pub fn new(nvs_part: &EspNvsPartition<NvsDefault>) -> Self {
            let namespace = "app_settings";
            let nvs = match EspNvs::new(nvs_part.to_owned(), namespace, true) {
                Ok(nvs) => {
                    info!("Got namespace {:?} from default partition", namespace);
                    nvs
                }
                Err(e) => panic!("Could't get namespace {:?}", e),
            };

            Storage { nvs }
        }

        pub fn store(&mut self, settings: &AppSettings) -> anyhow::Result<()> {
            let key_raw_struct: &str = "settings";
            match self.nvs.set_raw(
                key_raw_struct,
                serde_json::to_string(settings).unwrap().as_bytes(),
            ) {
                Ok(_) => info!("Key {} updated", key_raw_struct),
                Err(e) => info!("key {} not updated {:?}", key_raw_struct, e),
            };

            Ok(())
        }

        pub fn recall(&self) -> anyhow::Result<AppSettings> {
            let key_raw_struct: &str = "settings";
            let key_raw_struct_data: &mut [u8] = &mut [0; 1024];

            let settings_bytes = self
                .nvs
                .get_raw(key_raw_struct, key_raw_struct_data)
                .unwrap()
                .unwrap();

            let settings = serde_json::from_slice::<AppSettings>(settings_bytes).unwrap();
            Ok(settings)
        }
    }

    impl Observer for Storage {
        fn update(&mut self, state: &AppSettings) -> bool {
            let existing_state = self.recall().unwrap();
            if *state != existing_state {
                let _ = self.store(state);
                return true;
            }
            false
        }
    }
}
