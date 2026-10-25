// Copyright 2024 Google LLC; modifications 2025 sekigon-gonnoc.
// Rust adaptation 2026 CoreX prototype. SPDX-License-Identifier: Apache-2.0
//! PAW3222 three-wire protocol, mode 3. CS is tied LOW on the Corcell module.
//! Register sequence follows the working zmk-driver-paw3222 (Apache-2.0).
//! The SDIO output must be released for the entire sensor response.
pub trait Wire {
    fn clock(&mut self, high: bool);
    fn output(&mut self, enabled: bool);
    fn data(&mut self, high: bool);
    fn sample(&mut self) -> bool;
    fn delay_us(&mut self, us: u32);
}
pub struct PawWire<W: Wire>(pub W);
impl<W: Wire> PawWire<W> {
    fn send(&mut self, byte: u8) {
        self.0.output(true);
        for bit in (0..8).rev() {
            self.0.clock(false);
            self.0.data(byte & (1 << bit) != 0);
            self.0.delay_us(1);
            self.0.clock(true);
            self.0.delay_us(1);
        }
    }
    pub fn read(&mut self, reg: u8) -> u8 {
        self.send(reg & 0x7f);
        self.0.output(false);
        self.0.delay_us(4);
        let mut value = 0;
        for _ in 0..8 {
            self.0.clock(false);
            self.0.delay_us(1);
            self.0.clock(true);
            self.0.delay_us(1);
            value = (value << 1) | u8::from(self.0.sample());
        }
        self.0.delay_us(4);
        value
    }
    pub fn write(&mut self, reg: u8, value: u8) {
        self.send(reg | 0x80);
        self.send(value);
        self.0.output(false);
        self.0.delay_us(4);
    }
}
#[derive(Default)]
pub struct Scale {
    x: i32,
    y: i32,
}
/// Keep sensor counts independent of cursor speed, but use the cursor's axes.
pub fn gesture_motion(x: i16, y: i16) -> (i16, i16) {
    // The installed sensor is reversed on both axes, as in Scale below.
    // Do not apply the cursor gain or its fractional accumulator to gestures.
    (x.saturating_neg(), y.saturating_neg())
}
impl Scale {
    // Cursor gain -4/5: 2x the initial RMK/ZMK -2/5 setting.
    // Preserve the fractions instead of discarding slow one-count movements.
    #[cfg(test)]
    pub fn cursor(&mut self, x: i16, y: i16) -> (i16, i16) {
        self.cursor_with_gain(x,y,4,4)
    }
    pub fn cursor_with_gain(&mut self, x: i16, y: i16, nx:i32, ny:i32) -> (i16, i16) {
        self.x -= i32::from(x) * nx;
        self.y -= i32::from(y) * ny;
        let out = ((self.x / 5) as i16, (self.y / 5) as i16);
        self.x %= 5;
        self.y %= 5;
        out
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Mock {
        drive: bool,
        high: bool,
        tx: Vec<bool>,
        rx: u8,
        samples: usize,
        edges: usize,
    }
    impl Wire for Mock {
        fn clock(&mut self, high: bool) {
            if high && !self.high {
                self.edges += 1;
                if self.drive {
                    self.tx.push(self.rx_bit());
                }
            }
            self.high = high;
        }
        fn output(&mut self, enabled: bool) {
            self.drive = enabled;
        }
        fn data(&mut self, high: bool) {
            self.rx = (self.rx & 0xfe) | u8::from(high);
        }
        fn sample(&mut self) -> bool {
            assert!(!self.drive, "host must release SDIO before sensor response");
            let value = (0x30u8 & (1 << (7 - self.samples))) != 0;
            self.samples += 1;
            value
        }
        fn delay_us(&mut self, _: u32) {}
    }
    impl Mock {
        fn rx_bit(&self) -> bool {
            self.rx & 1 != 0
        }
    }
    fn mock() -> PawWire<Mock> {
        PawWire(Mock {
            drive: false,
            high: true,
            tx: vec![],
            rx: 0,
            samples: 0,
            edges: 0,
        })
    }
    fn bits(x: u8) -> Vec<bool> {
        (0..8).rev().map(|i| x & (1 << i) != 0).collect()
    }
    #[test]
    fn read_releases_bus_and_uses_read_address() {
        let mut p = mock();
        assert_eq!(p.read(0x02), 0x30);
        assert_eq!(p.0.tx, bits(0x02));
        assert_eq!(p.0.edges, 16);
        assert!(p.0.high);
        assert!(!p.0.drive);
    }
    #[test]
    fn write_sets_write_bit_and_releases_bus() {
        let mut p = mock();
        p.write(0x09, 0x5a);
        let mut expected = bits(0x89);
        expected.extend(bits(0x5a));
        assert_eq!(p.0.tx, expected);
        assert_eq!(p.0.edges, 16);
        assert_eq!(p.0.samples, 0);
        assert!(!p.0.drive);
    }
    #[test]
    fn slow_motion_is_conserved() {
        let mut s = Scale::default();
        let mut sum = (0, 0);
        for _ in 0..100 {
            let v = s.cursor(1, -1);
            sum.0 += v.0;
            sum.1 += v.1;
        }
        assert_eq!(sum, (-80, 80));
    }
    #[test]
    fn alternating_motion_has_no_bias() {
        let mut s = Scale::default();
        let mut sum = (0, 0);
        for _ in 0..100 {
            for d in [1, -1] {
                let v = s.cursor(d, d);
                sum.0 += v.0;
                sum.1 += v.1;
            }
        }
        assert_eq!(sum, (0, 0));
    }
    #[test]
    fn independent_axes_preserve_small_motion() {
        let mut s=Scale::default();
        let mut total=(0,0);
        for _ in 0..100 { let v=s.cursor_with_gain(1,-1,1,8); total.0+=v.0;total.1+=v.1; }
        assert_eq!(total,(-20,160));
        s.reset();
        assert_eq!(s.cursor_with_gain(-128,127,8,8),(204,-203));
    }
    #[test]
    fn signed_bytes_cover_extremes() {
        assert_eq!((0x80u8 as i8) as i16, -128);
        assert_eq!((0xffu8 as i8) as i16, -1);
        assert_eq!(Scale::default().cursor(-128, 127), (102, -101));
    }

    #[test]
    fn gestures_follow_cursor_direction_at_every_speed() {
        for raw in [(80, 0), (-80, 0), (0, 80), (0, -80), (1, -1), (-128, 127)] {
            let gesture = gesture_motion(raw.0, raw.1);
            // Thresholds use sensor counts, regardless of the cursor preset.
            assert_eq!(gesture.0.unsigned_abs(), raw.0.unsigned_abs());
            assert_eq!(gesture.1.unsigned_abs(), raw.1.unsigned_abs());
            for gain in [1, 2, 3, 4, 6, 8] {
                let mut scale = Scale::default();
                let mut cursor = (0i32, 0i32);
                // Include sub-pixel movements accumulated by the real cursor path.
                for _ in 0..5 {
                    let step = scale.cursor_with_gain(raw.0, raw.1, gain, gain);
                    cursor.0 += i32::from(step.0);
                    cursor.1 += i32::from(step.1);
                }
                assert_eq!(i32::from(gesture.0).signum(), cursor.0.signum(), "X raw={raw:?} gain={gain}");
                assert_eq!(i32::from(gesture.1).signum(), cursor.1.signum(), "Y raw={raw:?} gain={gain}");
            }
        }
    }
}
