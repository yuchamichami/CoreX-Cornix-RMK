//! Dedicated physical recovery operation. No periodic polling or keymap reads.
#[path = "pairing_keys_logic.rs"]
mod logic;

use embassy_time::{Duration, Instant, Timer};
use rmk::embassy_futures::select::{Either, select};
use rmk::event::{EventSubscriber, KeyboardEvent, KeyboardEventPos};
use rmk::macros::processor;
use rmk::processor::{PollingProcessor, Processor};

#[processor(subscribe=[KeyboardEvent])]
pub struct PairingKeys {
    keys: logic::PairingKeys,
}

impl PairingKeys {
    pub fn new() -> Self {
        Self {
            keys: logic::PairingKeys::default(),
        }
    }
    async fn on_keyboard_event(&mut self, event: KeyboardEvent) {
        if let KeyboardEventPos::Key(pos) = event.pos {
            self.keys
                .update(pos.row, pos.col, event.pressed, Instant::now().as_millis());
        }
    }
}

impl PollingProcessor for PairingKeys {
    fn interval(&self) -> Duration {
        Duration::MAX
    }
    async fn update(&mut self) {}
    async fn polling_loop(&mut self) -> ! {
        let mut events = Self::subscriber();
        loop {
            match self.keys.deadline() {
                Some(deadline) => match select(
                    events.next_event(),
                    Timer::at(Instant::from_millis(deadline)),
                )
                .await
                {
                    Either::First(event) => self.process(event).await,
                    Either::Second(_) => {
                        if self.keys.fire(Instant::now().as_millis()) {
                            rmk::keyboard::reserve_corex_pairing_releases();
                            rmk::split::ble::request_pairing();
                        }
                    }
                },
                None => self.process(events.next_event().await).await,
            }
        }
    }
}
