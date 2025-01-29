pub mod storage {
    use esp_idf_svc::nvs::*;
    //use postcard::{from_bytes, to_vec};
    use crate::settings::settings::AppSettings;
    use serde::{Deserialize, Serialize};

    use log::info;

    // https://github.com/esp-rs/esp-idf-svc/blob/master/examples/nvs_get_set_raw_storage.rs
    fn test() -> anyhow::Result<()> {
        let nvs_default_partition: EspNvsPartition<NvsDefault> = EspDefaultNvsPartition::take()?;

        let test_namespace = "app_settings_ns";
        //let test_namespace = "test_ns";
        let mut nvs = match EspNvs::new(nvs_default_partition, test_namespace, true) {
            Ok(nvs) => {
                info!("Got namespace {:?} from default partition", test_namespace);
                nvs
            }
            Err(e) => panic!("Could't get namespace {:?}", e),
        };

        Ok(())
    }
}
