//! CoreX right recovery chord in physical scan coordinates, independent of keymap.

pub const FIRST: (u8, u8) = (1, 5); // H
pub const SECOND: (u8, u8) = (3, 4); // thumb beside Space
const HOLD_MS: u64 = 5_000;
const CHORD: u64 = (1 << (FIRST.0 * 7 + FIRST.1)) | (1 << (SECOND.0 * 7 + SECOND.1));

#[derive(Default)]
pub struct PairingKeys {
    held: u64,
    deadline: Option<u64>,
    latched: bool,
}

impl PairingKeys {
    pub fn update(&mut self, row: u8, col: u8, pressed: bool, now: u64) {
        if row >= 8 || col >= 7 {
            return;
        }
        let bit = 1u64 << (row * 7 + col);
        if pressed {
            self.held |= bit;
        } else {
            self.held &= !bit;
        }
        if self.held & CHORD == 0 {
            self.latched = false;
        }
        if self.held != CHORD {
            // Releasing a chord key or pressing another key cancels the hold.
            // Require both chord keys released before another attempt.
            if self.deadline.take().is_some() {
                self.latched = true;
            }
        } else if !self.latched && self.deadline.is_none() {
            self.deadline = Some(now.saturating_add(HOLD_MS));
        }
    }

    pub fn deadline(&self) -> Option<u64> {
        self.deadline
    }

    pub fn fire(&mut self, now: u64) -> bool {
        if self.held == CHORD && self.deadline.is_some_and(|deadline| now >= deadline) {
            self.deadline = None;
            self.latched = true;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(state: &mut PairingKeys, at: (u8, u8), down: bool, now: u64) {
        state.update(at.0, at.1, down, now);
    }
    #[test]
    fn either_order_requires_five_seconds_with_both_keys_held_and_fires_once() {
        for (a, b) in [(FIRST, SECOND), (SECOND, FIRST)] {
            let mut state = PairingKeys::default();
            key(&mut state, a, true, 0);
            assert_eq!(state.deadline(), None);
            key(&mut state, b, true, 1_000);
            assert_eq!(state.deadline(), Some(6_000));
            assert!(!state.fire(5_999));
            assert!(state.fire(6_000));
            assert!(!state.fire(60_000));
            assert_eq!(state.deadline(), None);
        }
    }
    #[test]
    fn short_hold_or_another_key_cancels_and_requires_full_release() {
        for interrupted in [FIRST, (4, 1)] {
            let mut state = PairingKeys::default();
            key(&mut state, FIRST, true, 0);
            key(&mut state, SECOND, true, 0);
            key(&mut state, interrupted, interrupted != FIRST, 4_999);
            assert!(!state.fire(5_000));
            assert_eq!(state.deadline(), None);
            key(&mut state, interrupted, interrupted == FIRST, 5_001);
            assert!(!state.fire(20_000));
            key(&mut state, FIRST, false, 20_001);
            key(&mut state, SECOND, false, 20_001);
            key(&mut state, FIRST, true, 20_002);
            key(&mut state, SECOND, true, 20_002);
            assert!(state.fire(25_002));
        }
    }
    #[test]
    fn another_held_key_and_unrelated_positions_cannot_start_pairing() {
        let mut state = PairingKeys::default();
        key(&mut state, (7, 6), true, 0);
        key(&mut state, FIRST, true, 0);
        key(&mut state, SECOND, true, 0);
        assert_eq!(state.deadline(), None);
        assert!(!state.fire(60_000));
        state.update(255, 255, true, 0);
        assert_eq!(state.deadline(), None);
    }
}
