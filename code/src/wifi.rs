pub mod wifi {
    pub struct wifi {
        wifi: BlockingWifi,
    }

    impl Wifi {
        pub fn new() -> Self {

            let wifi = BlockingWifi::wrap(
                EspWifi::new(peripherals.modem sys_loop.clone(), Some(nvs))?,
                sys_loop
            )?;

            Wifi {
                wifi
            }
        }
    }
}
