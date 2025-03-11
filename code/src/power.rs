pub mod power {
    use crate::server::server::{ServableData, ServableDataReq, ServableDataRsp, ServerData};

    use esp_idf_hal::gpio::Input;
    use esp_idf_hal::i2c::*;
    use esp_idf_hal::prelude::*;
    use esp_idf_svc::hal::{
        gpio::Gpio4, gpio::InputPin, gpio::OutputPin, gpio::PinDriver, peripheral::Peripheral,
    };

    use log::info;
    use max170xx::Max17048;
    use std::sync::mpsc;

    pub struct Power<'a> {
        gauge: Max17048<I2cDriver<'a>>,
        charge_pin: PinDriver<'a, Gpio4, Input>,
        server_channel: Option<mpsc::Receiver<ServableDataReq>>,
    }

    impl<'a> Power<'a> {
        pub fn new(
            i2c: impl Peripheral<P = impl I2c> + 'static,
            sda: impl Peripheral<P = impl InputPin + OutputPin> + 'static,
            scl: impl Peripheral<P = impl InputPin + OutputPin> + 'static,
            charge_pin: PinDriver<'a, Gpio4, Input>,
        ) -> anyhow::Result<Self> {
            let config = I2cConfig::new().baudrate(100.kHz().into());
            let i2c = I2cDriver::new(i2c, sda, scl, &config).unwrap();
            let gauge = Max17048::new(i2c);

            Ok(Power {
                gauge,
                charge_pin,
                server_channel: None,
            })
        }

        pub fn batt_charge(&mut self) -> anyhow::Result<f32> {
            Ok(self.gauge.soc().unwrap())
        }

        pub fn batt_voltage(&mut self) -> anyhow::Result<f32> {
            Ok(self.gauge.voltage().unwrap())
        }

        pub fn batt_charge_rate(&mut self) -> anyhow::Result<f32> {
            Ok(self.gauge.charge_rate().unwrap())
        }

        pub fn batt_charging(&mut self) -> bool {
            self.charge_pin.is_low()
        }

        pub fn batt_connected(&self) -> bool {
            // TODO: Detect this somehow
            true
        }

        pub fn usb_connected(&self) -> bool {
            true
        }
    }

    impl<'a> ServableData for Power<'a> {
        fn get_channel(&mut self) -> mpsc::Sender<ServableDataReq> {
            let (tx, rx) = mpsc::channel::<ServableDataReq>();
            self.server_channel = Some(rx);
            tx
        }

        fn handle_server_req(&mut self) {
            if let Some(channel) = &self.server_channel {
                if let Ok(req) = channel.try_recv() {
                    info!("power got a request from server");

                    if let ServableDataReq::Get(back_channel) = &req {
                        info!("Sending power state to server");
                        let mut rsp = ServerData::new();
                        rsp.bat_capacity = Some(self.batt_charge().unwrap());
                        rsp.bat_charging = Some(self.batt_charging());
                        rsp.bat_attached = Some(self.batt_connected());
                        back_channel.send(ServableDataRsp::Data(rsp)).unwrap();
                    }
                }
            }
        }
    }
}
