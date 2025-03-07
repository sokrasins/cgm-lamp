pub mod storage {
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

        pub fn store(&mut self, obj: &impl Storable) -> anyhow::Result<()> {
            match self.nvs.set_raw(obj.store_tag(), &obj.store_data()) {
                Ok(_) => info!("Key {} updated", obj.store_tag()),
                Err(e) => info!("Key {} not updated: {:?}", obj.store_tag(), e),
            };

            Ok(())
        }

        pub fn recall(&self, obj: &mut impl Storable) -> anyhow::Result<()> {
            let key_raw_struct_data: &mut [u8] = &mut [0; 1024];

            let settings_bytes_result = self
                .nvs
                .get_raw(obj.store_tag(), key_raw_struct_data)
                .unwrap();

            match settings_bytes_result {
                Some(bytes) => {
                    obj.recall_data(bytes);
                    return Ok(());
                }
                None => Err(anyhow::anyhow!("No settings found")),
            }
        }
    }

    pub trait Storable {
        fn store_tag(&self) -> &str;
        fn store_data(&self) -> Vec<u8>;
        fn recall_data(&mut self, data: &[u8]);
    }
}
