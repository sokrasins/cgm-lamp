pub mod lamp {
    use crate::settings::settings::{AppSettings, Observer};
    use esp_idf_hal::{gpio::OutputPin, peripheral::Peripheral, rmt::RmtChannel};
    use rgb_led::{RGB8, WS2812RMT};

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

    pub struct Lamp<'a> {
        state: LedState,
        brightness: f32,
        led: WS2812RMT<'a>,
    }

    pub fn set_bright(color: &RGB8, brightness: f32) -> RGB8 {
        RGB8 {
            r: ((color.r as f32) * brightness) as u8,
            g: ((color.g as f32) * brightness) as u8,
            b: ((color.b as f32) * brightness) as u8,
        }
    }

    impl<'a> Lamp<'a> {
        pub fn new(
            led: impl Peripheral<P = impl OutputPin> + 'static,
            channel: impl Peripheral<P = impl RmtChannel> + 'static,
        ) -> Self {
            let state = LedState::Off;
            let brightness = 0.25;
            let led = WS2812RMT::new(led, channel).unwrap();

            Lamp {
                state,
                brightness,
                led,
            }
        }

        pub fn set_color(&mut self, color: LedState) {
            self.state = color;
            self.set_led();
        }

        pub fn set_brightness(&mut self, brightness: u8) {
            self.brightness = (brightness as f32) / 255.0;
            self.set_led();
        }

        fn set_led(&mut self) {
            match self.state {
                LedState::Steady(color) => self
                    .led
                    .set_pixel(set_bright(&color, self.brightness))
                    .unwrap(),
                LedState::Breathe(color) => self
                    .led
                    .set_pixel(set_bright(&color, self.brightness))
                    .unwrap(),
                LedState::Off => self.led.set_pixel(BLACK).unwrap(),
            };
        }
    }

    impl<'a> Observer for Lamp<'a> {
        fn update(&mut self, state: &AppSettings) -> bool {
            let mut ret = false;
            if let Some(brightness) = state.brightness {
                self.set_brightness(brightness);
                ret = true;
            }

            ret
        }
    }
}
