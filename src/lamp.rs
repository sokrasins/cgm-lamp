pub mod lamp {
    use core::time::Duration;
    use esp_idf_hal::{gpio::OutputPin, peripheral::Peripheral, rmt::RmtChannel};
    use esp_idf_svc::hal::delay::FreeRtos;
    use esp_idf_svc::timer::EspTaskTimerService;
    use esp_idf_svc::timer::EspTimer;
    use esp_idf_svc::timer::EspTimerService;
    use esp_idf_svc::timer::Task;
    use rgb_led::RGB8;
    use rgb_led::WS2812RMT;
    use std::sync::{Arc, Mutex};

    pub const BRIGHTNESS: u8 = 64;

    pub const RED: RGB8 = RGB8 {
        r: BRIGHTNESS,
        g: 0,
        b: 0,
    };
    pub const BLUE: RGB8 = RGB8 {
        r: 0,
        g: 0,
        b: BRIGHTNESS,
    };
    pub const YELLOW: RGB8 = RGB8 {
        r: BRIGHTNESS,
        g: BRIGHTNESS,
        b: 0,
    };
    pub const BLACK: RGB8 = RGB8 { r: 0, g: 0, b: 0 };
    pub const WHITE: RGB8 = RGB8 {
        r: BRIGHTNESS,
        g: BRIGHTNESS,
        b: BRIGHTNESS,
    };

    pub const GREEN: RGB8 = RGB8 {
        r: 0,
        g: BRIGHTNESS,
        b: 0,
    };

    pub const PURPLE: RGB8 = RGB8 {
        r: BRIGHTNESS,
        g: 0,
        b: BRIGHTNESS,
    };

    #[allow(dead_code)]
    pub enum LedState {
        Steady(RGB8),
        Breathe(RGB8),
        Off,
    }

    pub fn get_color_in_sweep(
        start_color: &RGB8,
        end_color: &RGB8,
        total: usize,
        idx: isize,
    ) -> RGB8 {
        let r_step = (end_color.r as f64 - start_color.r as f64) / (total as f64);
        let g_step = (end_color.g as f64 - start_color.g as f64) / (total as f64);
        let b_step = (end_color.b as f64 - start_color.b as f64) / (total as f64);

        RGB8::new(
            ((start_color.r as f64) + (idx as f64 * r_step)) as u8,
            ((start_color.g as f64) + (idx as f64 * g_step)) as u8,
            ((start_color.b as f64) + (idx as f64 * b_step)) as u8,
        )
    }

    pub struct Lamp {
        state: Arc<Mutex<LedState>>,
        timer: Option<EspTimerService<Task>>,
        cb_timer: Option<EspTimer<'static>>,
    }

    impl Lamp {
        pub fn new() -> Self {
            let lock = Arc::new(Mutex::new(LedState::Off));

            Lamp {
                state: lock,
                timer: None,
                cb_timer: None,
            }
        }

        pub fn start(
            &mut self,
            led: impl Peripheral<P = impl OutputPin> + 'static,
            channel: impl Peripheral<P = impl RmtChannel> + 'static,
        ) -> anyhow::Result<()> {
            // LED-writing thread
            self.timer = Some(EspTaskTimerService::new()?);
            self.cb_timer = Some({
                let lock = Arc::clone(&self.state);
                let mut ws2812 = WS2812RMT::new(led, channel).unwrap();

                self.timer.clone().unwrap().timer(move || {
                    let led = lock.lock().unwrap();
                    match *led {
                        LedState::Steady(color) => ws2812.set_pixel(color).unwrap(),
                        LedState::Breathe(color) => {
                            for i in 0..80 {
                                ws2812
                                    .set_pixel(get_color_in_sweep(&color, &BLACK, 80, i))
                                    .unwrap();
                                FreeRtos::delay_ms(10);
                            }
                            for i in 0..80 {
                                ws2812
                                    .set_pixel(get_color_in_sweep(&BLACK, &color, 80, i))
                                    .unwrap();
                                FreeRtos::delay_ms(10);
                            }
                        }
                        LedState::Off => ws2812.set_pixel(BLACK).unwrap(),
                    };
                })?
            });
            self.cb_timer
                .as_ref()
                .unwrap()
                .every(Duration::from_secs(2))?;

            Ok(())
        }

        pub fn set_color(&mut self, color: LedState) {
            let mut state = self.state.lock().unwrap();
            *state = color;
        }
    }
}
