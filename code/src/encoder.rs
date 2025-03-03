pub mod encoder {

    use esp_idf_hal::gpio::{InputPin, OutputPin};
    use esp_idf_hal::peripheral::Peripheral;

    pub enum Direction {
        Clockwise,
        CounterClockwise,
        None,
    }

    impl From<u8> for Direction {
        fn from(s: u8) -> Self {
            match s {
                0b0001 | 0b0111 | 0b1000 | 0b1110 => Direction::Clockwise,
                0b0010 | 0b0100 | 0b1011 | 0b1101 => Direction::CounterClockwise,
                _ => Direction::None,
            }
        }
    }

    pub struct Encoder<A, B> {
        pin_a: A,
        pin_b: B,
        state: u8,
    }

    impl<A, B> Encoder<A, B>
    where
        A: Peripheral<P = impl InputPin>,
        B: Peripheral<P = impl InputPin>,
    {
        pub fn new(pin_a: A, pin_b: B) -> Self {
            Self {
                pin_a,
                pin_b,
                state: 0u8,
            }
        }

        pub fn update(&mut self) -> anyhow::Result<Direction> {
            // use mask to get previous state value
            let mut s = self.state & 0b11;
            // move in the new state
            if self.pin_a.is_low().unwrap() {
                s |= 0b100;
            }
            if self.pin_b.is_low().map_err(Either::Right)? {
                s |= 0b1000;
            }
            // shift new to old
            self.state = s >> 2;
            // and here we use the From<u8> implementation above to return a Direction
            Ok(s.into())
        }
    }
}
