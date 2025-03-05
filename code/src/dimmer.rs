pub mod dimmer {
    use crate::encoder::encoder::Encoder;
    use esp_idf_hal::gpio::InputPin;
    use esp_idf_hal::pcnt::Pcnt;
    use esp_idf_hal::peripheral::Peripheral;

    pub struct LightDimmer<'a> {
        encoder: Encoder<'a>,
        last_pos: i32,
    }

    impl<'a> LightDimmer<'a> {
        pub fn new<PCNT: Pcnt>(
            pcnt: impl Peripheral<P = PCNT> + 'a,
            pin_a: impl Peripheral<P = impl InputPin> + 'a,
            pin_b: impl Peripheral<P = impl InputPin> + 'a,
        ) -> anyhow::Result<Self> {
            let encoder = Encoder::new(pcnt, pin_a, pin_b)?;

            Ok(Self {
                encoder,
                last_pos: 0,
            })
        }

        pub fn get_change(&mut self) -> i32 {
            let new_val = self.encoder.get_value().unwrap();
            let diff = new_val - self.last_pos;
            self.last_pos = new_val;

            diff
        }
    }
}
