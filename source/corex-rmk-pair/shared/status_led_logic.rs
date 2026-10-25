//! Event-driven status indications. Pairing follows its radio lifecycle; every
//! other animation ends with no idle deadline.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Off,
    Red,
    Green,
    Blue,
    Amber,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame(pub Color, pub Color);

pub const OFF: Frame = Frame(Color::Off, Color::Off);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    SplitDisconnected,
    SplitConnected,
    HostConnected,
    HostDisconnected,
    Profile(u8),
    LocalLow,
    PeerLow,
}

impl Notice {
    fn pattern(self) -> (Frame, u8, u64, u8) {
        use Color::*;
        match self {
            Self::SplitDisconnected => (Frame(Off, Blue), 3, 160, 1),
            Self::SplitConnected => (Frame(Off, Green), 2, 160, 1),
            Self::HostConnected => (Frame(Off, Green), 1, 500, 1),
            Self::HostDisconnected => (Frame(Off, Amber), 2, 160, 1),
            // Display profile 0 as one pulse, never as no indication.
            Self::Profile(id) => (Frame(Off, Blue), id.min(4) + 1, 180, 2),
            Self::LocalLow => (Frame(Red, Off), 1, 350, 3),
            Self::PeerLow => (Frame(Red, Off), 2, 350, 3),
        }
    }
}

/// Pairing owns both lights until the radio reports a result. This is separate
/// from ordinary split reconnection and PC Bluetooth notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairingNotice {
    Open,
    Success,
    Failure,
}

#[derive(Debug)]
struct PairingAnimation {
    notice: PairingNotice,
    start_ms: u64,
}

#[derive(Debug)]
pub struct Player {
    notice: Option<Notice>,
    battery_pending: [Option<Notice>; 2],
    start_ms: u64,
    sleeping: bool,
    pairing: Option<PairingAnimation>,
}

impl Player {
    pub const fn new() -> Self {
        Self {
            notice: None,
            battery_pending: [None; 2],
            start_ms: 0,
            sleeping: false,
            pairing: None,
        }
    }

    pub fn request(&mut self, notice: Notice, now_ms: u64) {
        self.expire(now_ms);
        // Do not append a red battery warning or a host notice to the pairing
        // result: its green/amber acknowledgement must end with both lights off.
        if self.sleeping || self.pairing.is_some() {
            return;
        }
        if let Some(current) = self.notice {
            if matches!(notice, Notice::LocalLow | Notice::PeerLow) && current.pattern().3 >= 3 {
                if current != notice && !self.battery_pending.contains(&Some(notice)) {
                    if let Some(slot) = self.battery_pending.iter_mut().find(|slot| slot.is_none()) {
                        *slot = Some(notice);
                    }
                }
                return;
            }
            if current.pattern().3 > notice.pattern().3 {
                return;
            }
        }
        self.notice = Some(notice);
        self.start_ms = now_ms;
    }

    pub fn set_sleeping(&mut self, sleeping: bool) {
        self.sleeping = sleeping;
        // Sleep/wake broadcasts can race with explicit pairing. Its own radio
        // lifecycle remains authoritative, so neither broadcast clears it.
        self.notice = None;
        self.battery_pending = [None; 2];
    }

    pub fn pairing(&mut self, notice: PairingNotice, now_ms: u64) {
        // The authoritative state may be sampled before its queued event arrives.
        // Do not restart the acknowledgement when the same result arrives twice.
        if self.pairing.as_ref().is_some_and(|current| current.notice == notice) {
            return;
        }
        self.sleeping = false;
        self.notice = None;
        self.battery_pending = [None; 2];
        self.pairing = Some(PairingAnimation {
            notice,
            start_ms: now_ms,
        });
    }

    pub fn pairing_is_open(&self) -> bool {
        self.pairing
            .as_ref()
            .is_some_and(|current| current.notice == PairingNotice::Open)
    }

    fn expire(&mut self, now_ms: u64) {
        if let Some(pairing) = &self.pairing {
            let duration = match pairing.notice {
                PairingNotice::Open => None,
                PairingNotice::Success => Some(2_000),
                // Three 500 ms pulses, with a 500 ms dark gap between each.
                PairingNotice::Failure => Some(2_500),
            };
            if duration.is_some_and(|duration| now_ms.saturating_sub(pairing.start_ms) >= duration) {
                self.pairing = None;
            }
        }
        if let Some(notice) = self.notice {
            let (_, pulses, width, _) = notice.pattern();
            // Leave a visible dark gap between queued battery indications.
            let phases = u64::from(pulses) * 2 - u64::from(self.battery_pending[0].is_none());
            if now_ms.saturating_sub(self.start_ms) >= phases * width {
                self.notice = None;
            }
        }
        if self.notice.is_none() {
            self.notice = self.battery_pending[0];
            self.battery_pending[0] = self.battery_pending[1];
            self.battery_pending[1] = None;
            self.start_ms = now_ms;
        }
    }

    pub fn sample(&mut self, now_ms: u64) -> (Frame, Option<u64>) {
        self.expire(now_ms);
        if let Some(pairing) = &self.pairing {
            if pairing.notice == PairingNotice::Success {
                return (Frame(Color::Green, Color::Green), Some(pairing.start_ms + 2_000));
            }
            let phase = now_ms.saturating_sub(pairing.start_ms) / 500;
            let color = if pairing.notice == PairingNotice::Open {
                Color::Blue
            } else {
                Color::Amber
            };
            let frame = if phase % 2 == 0 { Frame(color, color) } else { OFF };
            return (frame, Some(pairing.start_ms + (phase + 1) * 500));
        }
        let Some(notice) = self.notice else {
            return (OFF, None);
        };
        let (frame, _, width, _) = notice.pattern();
        let phase = now_ms.saturating_sub(self.start_ms) / width;
        let deadline = self.start_ms + (phase + 1) * width;
        (if phase % 2 == 0 { frame } else { OFF }, Some(deadline))
    }
}

/// Valid measurements only, with hysteresis and one-minute warning suppression.
#[derive(Debug)]
pub struct LowBattery {
    low: bool,
    last_warning_ms: Option<u64>,
}

impl LowBattery {
    pub const fn new() -> Self {
        Self {
            low: false,
            last_warning_ms: None,
        }
    }

    pub fn update(&mut self, level: Option<u8>, charging: bool, now_ms: u64) -> bool {
        if charging || level.is_none_or(|v| v > 100) {
            self.low = false;
            return false;
        }
        let level = level.unwrap();
        if level <= 20 {
            self.low = true;
        } else if level >= 25 {
            self.low = false;
        }
        if self.low
            && self
                .last_warning_ms
                .is_none_or(|last| now_ms.saturating_sub(last) >= 60_000)
        {
            self.last_warning_ms = Some(now_ms);
            true
        } else {
            false
        }
    }
}

/// Two GRB pixels plus a final always-low PWM sample. nRF PWM at 16 MHz,
/// COUNTERTOP=20: 0=375 ns high; 1=875 ns high. The final sample's end-delay
/// provides a reset low longer than 280 us, including newer WS2812 revisions.
pub fn ws2812_words(frame: Frame) -> [u16; 49] {
    let mut words = [0x8000; 49];
    for (pixel, color) in [frame.0, frame.1].into_iter().enumerate() {
        let grb = match color {
            Color::Off => [0, 0, 0],
            Color::Red => [0, 8, 0],
            Color::Green => [8, 0, 0],
            Color::Blue => [0, 0, 8],
            Color::Amber => [4, 8, 0],
        };
        for (byte, value) in grb.into_iter().enumerate() {
            for bit in 0..8 {
                words[pixel * 24 + byte * 8 + bit] = 0x8000 | if value & (0x80 >> bit) != 0 { 14 } else { 6 };
            }
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_pulses_are_bounded_and_leave_no_idle_deadline() {
        for profile in 0..5 {
            let mut player = Player::new();
            player.request(Notice::Profile(profile), 0);
            for pulse in 0..=u64::from(profile) {
                assert_ne!(player.sample(pulse * 360).0, OFF);
            }
            assert_eq!(player.sample((u64::from(profile) * 2 + 1) * 180), (OFF, None));
            assert_eq!(player.sample(60_000), (OFF, None));
        }
    }

    #[test]
    fn sleep_cancels_running_and_new_indications() {
        let mut player = Player::new();
        player.request(Notice::LocalLow, 0);
        player.set_sleeping(true);
        player.request(Notice::HostConnected, 1);
        assert_eq!(player.sample(2), (OFF, None));
        player.set_sleeping(false);
        assert_eq!(player.sample(3), (OFF, None));
    }

    #[test]
    fn battery_priority_is_not_replaced_by_connection_churn() {
        let mut player = Player::new();
        player.request(Notice::PeerLow, 0);
        player.request(Notice::HostConnected, 10);
        assert_eq!(player.sample(700).0, Frame(Color::Red, Color::Off));
        assert_eq!(player.sample(1050), (OFF, None));
    }

    #[test]
    fn both_low_batteries_are_queued_without_an_idle_timer() {
        let mut player = Player::new();
        player.request(Notice::LocalLow, 0);
        player.request(Notice::PeerLow, 20);
        player.request(Notice::PeerLow, 30); // No duplicate queued warning.
        assert_eq!(player.sample(31).0, Frame(Color::Red, Color::Off));
        assert_eq!(player.sample(350).0, OFF);
        assert_eq!(player.sample(700).0, Frame(Color::Red, Color::Off));
        assert_eq!(player.sample(1050).0, OFF);
        assert_eq!(player.sample(1400).0, Frame(Color::Red, Color::Off));
        assert_eq!(player.sample(1750), (OFF, None));
    }

    #[test]
    fn waiting_tracks_the_real_pairing_window_instead_of_three_pulses() {
        let mut player = Player::new();
        player.pairing(PairingNotice::Open, 100);
        assert!(player.pairing_is_open());
        assert_eq!(player.sample(100), (Frame(Color::Blue, Color::Blue), Some(600)));
        assert_eq!(player.sample(599).0, Frame(Color::Blue, Color::Blue));
        assert_eq!(player.sample(600), (OFF, Some(1100)));
        assert_eq!(player.sample(59_100).0, Frame(Color::Blue, Color::Blue));
        // The radio's terminal event is authoritative, including a late event.
        assert_eq!(player.sample(60_100).0, Frame(Color::Blue, Color::Blue));
        player.pairing(PairingNotice::Failure, 60_100);
        assert!(!player.pairing_is_open());
        assert_eq!(player.sample(62_600), (OFF, None));
    }

    #[test]
    fn successful_pairing_holds_both_green_for_two_seconds_then_stops() {
        let mut player = Player::new();
        player.pairing(PairingNotice::Open, 0);
        player.pairing(PairingNotice::Success, 1234);
        assert!(!player.pairing_is_open());
        assert_eq!(player.sample(1234), (Frame(Color::Green, Color::Green), Some(3234)));
        assert_eq!(player.sample(3233).0, Frame(Color::Green, Color::Green));
        assert_eq!(player.sample(3234), (OFF, None));
        assert_eq!(player.sample(90_000), (OFF, None));
    }

    #[test]
    fn unsuccessful_pairing_has_exactly_three_amber_pulses_then_stops() {
        let mut player = Player::new();
        // A storage failure can happen before the pairing window opens.
        player.pairing(PairingNotice::Failure, 100);
        for (elapsed, lit) in [
            (0, true),
            (499, true),
            (500, false),
            (999, false),
            (1000, true),
            (1499, true),
            (1500, false),
            (1999, false),
            (2000, true),
            (2499, true),
        ] {
            let expected = if lit { Frame(Color::Amber, Color::Amber) } else { OFF };
            assert_eq!(player.sample(100 + elapsed).0, expected);
        }
        assert_eq!(player.sample(2600), (OFF, None));
        assert_eq!(player.sample(60_000), (OFF, None));
    }

    #[test]
    fn wake_order_and_sleep_broadcast_do_not_drop_explicit_pairing() {
        let mut player = Player::new();
        player.set_sleeping(true);
        player.pairing(PairingNotice::Open, 100);
        player.set_sleeping(false); // Old event ordering: Open before wake.
        assert_eq!(player.sample(100).0, Frame(Color::Blue, Color::Blue));
        player.set_sleeping(true); // A queued sleep event is not a pairing result.
        assert_eq!(player.sample(1100).0, Frame(Color::Blue, Color::Blue));
        player.set_sleeping(false);
        player.pairing(PairingNotice::Success, 1200);
        player.set_sleeping(false);
        assert_eq!(player.sample(1200).0, Frame(Color::Green, Color::Green));
        assert_eq!(player.sample(3200), (OFF, None));
    }

    #[test]
    fn pairing_and_its_result_cannot_be_overwritten_or_followed_by_stale_notices() {
        let mut player = Player::new();
        player.request(Notice::LocalLow, 0);
        player.request(Notice::PeerLow, 1); // Pairing also discards old queued warnings.
        player.pairing(PairingNotice::Open, 2);
        for notice in [
            Notice::LocalLow,
            Notice::PeerLow,
            Notice::HostConnected,
            Notice::Profile(1),
            Notice::SplitConnected,
            Notice::SplitDisconnected,
        ] {
            player.request(notice, 10);
        }
        assert_eq!(player.sample(1002).0, Frame(Color::Blue, Color::Blue));
        player.pairing(PairingNotice::Success, 2000);
        player.request(Notice::SplitConnected, 2001);
        player.request(Notice::LocalLow, 2002);
        assert_eq!(player.sample(3999).0, Frame(Color::Green, Color::Green));
        assert_eq!(player.sample(4000), (OFF, None));
        // A genuinely new event works normally after the acknowledgement.
        player.request(Notice::HostDisconnected, 5000);
        assert_eq!(player.sample(5000).0, Frame(Color::Off, Color::Amber));
    }

    #[test]
    fn duplicate_lifecycle_event_does_not_extend_result() {
        let mut player = Player::new();
        player.pairing(PairingNotice::Open, 0);
        player.pairing(PairingNotice::Success, 100);
        player.pairing(PairingNotice::Success, 110);
        assert_eq!(player.sample(2100), (OFF, None));
        player.pairing(PairingNotice::Open, 3000);
        assert_eq!(player.sample(3000).0, Frame(Color::Blue, Color::Blue));
    }

    #[test]
    fn unavailable_and_charging_are_not_low_battery() {
        let mut battery = LowBattery::new();
        assert!(!battery.update(None, false, 0));
        assert!(!battery.update(Some(255), false, 0));
        assert!(!battery.update(Some(5), true, 0));
        assert!(battery.update(Some(5), false, 0));
    }

    #[test]
    fn low_battery_is_throttled_and_has_hysteresis() {
        let mut battery = LowBattery::new();
        assert!(battery.update(Some(20), false, 0));
        assert!(!battery.update(Some(19), false, 59_999));
        assert!(battery.update(Some(22), false, 60_000));
        assert!(!battery.update(Some(25), false, 120_000));
        assert!(!battery.update(Some(22), false, 180_000));
        assert!(battery.update(Some(20), false, 180_000));
    }

    #[test]
    fn late_timer_does_not_extend_led_or_start_an_idle_ticker() {
        let mut player = Player::new();
        player.request(Notice::SplitDisconnected, 0);
        assert_eq!(player.sample(30_000), (OFF, None));
    }

    #[test]
    fn every_notice_finishes_with_led_off() {
        for notice in [
            Notice::SplitDisconnected,
            Notice::SplitConnected,
            Notice::HostConnected,
            Notice::HostDisconnected,
            Notice::Profile(255),
            Notice::LocalLow,
            Notice::PeerLow,
        ] {
            let mut player = Player::new();
            player.request(notice, 10);
            assert_ne!(player.sample(10).0, OFF);
            assert_eq!(player.sample(3_000), (OFF, None));
        }
    }

    #[test]
    fn ws2812_uses_grb_msb_first_and_low_latch_sample() {
        let words = ws2812_words(Frame(Color::Red, Color::Green));
        assert_eq!(words.len(), 49);
        assert_eq!(words[8 + 4], 0x8000 | 14); // Red 8's bit3, after first G byte.
        assert_eq!(words[24 + 4], 0x8000 | 14); // Second pixel G=8.
        assert_eq!(words.iter().filter(|&&word| word == 0x8000 | 14).count(), 2);
        assert_eq!(words[48], 0x8000); // Low throughout the latch interval.
    }
}
