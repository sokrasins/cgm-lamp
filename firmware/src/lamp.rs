pub mod lamp {
    use crate::settings::settings::{AppSettings, Observer};
    use core::time::Duration;
    use esp_idf_hal::{gpio::OutputPin, peripheral::Peripheral, rmt::RmtChannel};
    use esp_idf_svc::hal::delay::FreeRtos;
    use esp_idf_svc::timer::{EspTaskTimerService, EspTimer, EspTimerService, Task};
    use rgb_led::{RGB8, WS2812RMT};
    use std::sync::{Arc, Mutex};

    pub const COLOR_MAX: u8 = 255;

    pub const RED: RGB8 = RGB8 {
        r: COLOR_MAX,
        g: 0,
        b: 0,
    };
    pub const BLUE: RGB8 = RGB8 {
        r: 0,
        g: 0,
        b: COLOR_MAX,
    };
    pub const YELLOW: RGB8 = RGB8 {
        r: COLOR_MAX,
        g: COLOR_MAX,
        b: 0,
    };
    pub const BLACK: RGB8 = RGB8 { r: 0, g: 0, b: 0 };
    pub const WHITE: RGB8 = RGB8 {
        r: COLOR_MAX,
        g: COLOR_MAX,
        b: COLOR_MAX,
    };

    pub const GREEN: RGB8 = RGB8 {
        r: 0,
        g: COLOR_MAX,
        b: 0,
    };

    pub const PURPLE: RGB8 = RGB8 {
        r: COLOR_MAX,
        g: 0,
        b: COLOR_MAX,
    };

    #[allow(dead_code)]
    pub enum LedState {
        Steady(RGB8),
        Breathe(RGB8),
        Off,
    }

    impl LedState {
        pub fn from_glucose(value: isize) -> LedState {
            // Multi-colored colormap
            // Red -> Green -> Blue -> Purple
            match value {
                0..55 => LedState::Breathe(RED),
                55..152 => LedState::Steady(get_color_in_sweep(&RED, &GREEN, 152 - 55, value - 55)),
                152..250 => {
                    LedState::Steady(get_color_in_sweep(&GREEN, &BLUE, 250 - 152, value - 152))
                }
                250..300 => {
                    LedState::Steady(get_color_in_sweep(&BLUE, &PURPLE, 300 - 250, value - 250))
                }
                300..500 => LedState::Breathe(PURPLE),
                _ => LedState::Breathe(WHITE),
            }
        }
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
        brightness: Arc<Mutex<f32>>,
    }

    pub fn set_bright(color: &RGB8, brightness: f32) -> RGB8 {
        RGB8 {
            r: ((color.r as f32) * brightness) as u8,
            g: ((color.g as f32) * brightness) as u8,
            b: ((color.b as f32) * brightness) as u8,
        }
    }

    impl Lamp {
        pub fn new() -> Self {
            let lock = Arc::new(Mutex::new(LedState::Off));
            let brightness = Arc::new(Mutex::new(0.25));

            Lamp {
                state: lock,
                timer: None,
                cb_timer: None,
                brightness,
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
                let brightness = Arc::clone(&self.brightness);

                self.timer.clone().unwrap().timer(move || {
                    let led = lock.lock().unwrap();
                    let bright = brightness.lock().unwrap();
                    match *led {
                        LedState::Steady(color) => {
                            ws2812.set_pixel(set_bright(&color, *bright)).unwrap()
                        }
                        LedState::Breathe(color) => {
                            for i in 0..80 {
                                ws2812
                                    .set_pixel(get_color_in_sweep(
                                        &set_bright(&color, *bright),
                                        &BLACK,
                                        80,
                                        i,
                                    ))
                                    .unwrap();
                                FreeRtos::delay_ms(10);
                            }
                            for i in 0..80 {
                                ws2812
                                    .set_pixel(get_color_in_sweep(
                                        &BLACK,
                                        &set_bright(&color, *bright),
                                        80,
                                        i,
                                    ))
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

        pub fn set_brightness(&self, brightness: f32) {
            let mut self_brightness = self.brightness.lock().unwrap();
            *self_brightness = brightness;
        }
    }

    impl Observer for Lamp {
        fn update(&mut self, state: &AppSettings) -> bool {
            let mut ret = false;
            if let Some(brightness) = state.lamp_brightness {
                self.set_brightness(brightness);
                ret = true;
            }

            ret
        }
    }
}
