pub mod dimmer {
    use esp_idf_hal::gpio::AnyInputPin;
    use esp_idf_hal::gpio::InputPin;
    use esp_idf_hal::peripheral::Peripheral;

    use std::sync::atomic::AtomicI32;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    use esp_idf_hal::pcnt::*;
    use esp_idf_hal::sys::EspError;

    const LOW_LIMIT: i16 = -100;
    const HIGH_LIMIT: i16 = 100;

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

    pub struct Encoder<'d> {
        unit: PcntDriver<'d>,
        approx_value: Arc<AtomicI32>,
    }

    impl<'d> Encoder<'d> {
        pub fn new<PCNT: Pcnt>(
            pcnt: impl Peripheral<P = PCNT> + 'd,
            pin_a: impl Peripheral<P = impl InputPin> + 'd,
            pin_b: impl Peripheral<P = impl InputPin> + 'd,
        ) -> Result<Self, EspError> {
            let mut unit = PcntDriver::new(
                pcnt,
                Some(pin_a),
                Some(pin_b),
                Option::<AnyInputPin>::None,
                Option::<AnyInputPin>::None,
            )?;
            unit.channel_config(
                PcntChannel::Channel0,
                PinIndex::Pin0,
                PinIndex::Pin1,
                &PcntChannelConfig {
                    lctrl_mode: PcntControlMode::Reverse,
                    hctrl_mode: PcntControlMode::Keep,
                    pos_mode: PcntCountMode::Decrement,
                    neg_mode: PcntCountMode::Increment,
                    counter_h_lim: HIGH_LIMIT,
                    counter_l_lim: LOW_LIMIT,
                },
            )?;

            unit.channel_config(
                PcntChannel::Channel1,
                PinIndex::Pin1,
                PinIndex::Pin0,
                &PcntChannelConfig {
                    lctrl_mode: PcntControlMode::Reverse,
                    hctrl_mode: PcntControlMode::Keep,
                    pos_mode: PcntCountMode::Increment,
                    neg_mode: PcntCountMode::Decrement,
                    counter_h_lim: HIGH_LIMIT,
                    counter_l_lim: LOW_LIMIT,
                },
            )?;

            unit.set_filter_value(800)?;
            unit.filter_enable()?;

            let approx_value = Arc::new(AtomicI32::new(0));
            // unsafe interrupt code to catch the upper and lower limits from the encoder
            // and track the overflow in `value: Arc<AtomicI32>` - I plan to use this for
            // a wheeled robot's odomerty
            unsafe {
                let approx_value = approx_value.clone();
                unit.subscribe(move |status| {
                    let status = PcntEventType::from_repr_truncated(status);
                    if status.contains(PcntEvent::HighLimit) {
                        approx_value.fetch_add(HIGH_LIMIT as i32, Ordering::SeqCst);
                    }
                    if status.contains(PcntEvent::LowLimit) {
                        approx_value.fetch_add(LOW_LIMIT as i32, Ordering::SeqCst);
                    }
                })?;
            }
            unit.event_enable(PcntEvent::HighLimit)?;
            unit.event_enable(PcntEvent::LowLimit)?;
            unit.counter_pause()?;
            unit.counter_clear()?;
            unit.counter_resume()?;

            Ok(Self { unit, approx_value })
        }

        pub fn get_value(&self) -> Result<i32, EspError> {
            let value =
                self.approx_value.load(Ordering::Relaxed) + self.unit.get_counter_value()? as i32;
            Ok(value)
        }
    }
}
