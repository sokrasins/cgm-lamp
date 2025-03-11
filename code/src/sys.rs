pub mod sys {
    use crate::server::server::{ServableData, ServableDataReq, ServableDataRsp, ServerData};
    use esp_idf_hal::temp_sensor::*;
    use esp_idf_svc::hal::{gpio::Gpio5, gpio::Output, gpio::PinDriver};
    use log::info;
    use std::sync::mpsc;
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn uptime() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
    }

    pub struct Sys<'a> {
        indicator: PinDriver<'a, Gpio5, Output>,
        temp: TempSensorDriver<'a>,
        server_channel: Option<mpsc::Receiver<ServableDataReq>>,
    }

    impl<'a> Sys<'a> {
        pub fn new(indicator: PinDriver<'a, Gpio5, Output>, temp_sensor: TempSensor) -> Self {
            let cfg = TempSensorConfig::default();
            let mut temp = TempSensorDriver::new(&cfg, temp_sensor).unwrap();
            temp.enable().unwrap();

            Sys {
                indicator,
                temp,
                server_channel: None,
            }
        }

        pub fn ind_on(&mut self) {
            self.indicator.set_high().unwrap();
        }

        pub fn ind_off(&mut self) {
            self.indicator.set_low().unwrap();
        }

        pub fn get_temp(&self) -> f32 {
            self.temp.get_celsius().unwrap()
        }
    }

    impl<'a> ServableData for Sys<'a> {
        fn get_channel(&mut self) -> mpsc::Sender<ServableDataReq> {
            let (tx, rx) = mpsc::channel::<ServableDataReq>();
            self.server_channel = Some(rx);
            tx
        }

        fn handle_server_req(&mut self) {
            if let Some(channel) = &self.server_channel {
                if let Ok(req) = channel.try_recv() {
                    info!("lamp got a request from server");

                    if let ServableDataReq::Get(back_channel) = &req {
                        info!("Sending lamp state to server");
                        let mut rsp = ServerData::new();
                        rsp.uptime = Some(uptime());
                        rsp.temp = Some(self.get_temp());
                        back_channel.send(ServableDataRsp::Data(rsp)).unwrap();
                    }
                }
            }
        }
    }
}
