pub mod central;
pub(crate) mod input;
pub mod peripheral;

use core::cell::Cell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::Mutex;

use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer, with_timeout};

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};
use trouble_host::types::gatt_traits::AsGatt;

use super::SplitMessage;
use super::driver::SplitDriverError;

static PAIRING_REQUEST: Signal<crate::RawMutex, ()> = Signal::new();
static PAIRING_REQUEST_PENDING: AtomicBool = AtomicBool::new(false);
static PAIRING_OPEN: AtomicBool = AtomicBool::new(false);
static PAIRING_STATE: AtomicU8 = AtomicU8::new(0);
static PAIRING_DEADLINE: Mutex<crate::RawMutex, Cell<Option<Instant>>> =
    Mutex::new(Cell::new(None));
static PAIRING_TIMER: Signal<crate::RawMutex, Option<Instant>> = Signal::new();
const PAIRING_DURATION: Duration = Duration::from_secs(60);

/// Local pairing feedback. A BLE transport connection alone is not success.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum PairingState {
    Open = 1,
    Success = 2,
    Timeout = 3,
    Failed = 4,
}

/// Forget this half's saved split peer and open a 60-second pairing window.
/// Duplicate requests from a physical gesture and its User7 alias are coalesced.
pub fn request_pairing() {
    if !pairing_window_open() && !PAIRING_REQUEST_PENDING.swap(true, Ordering::AcqRel) {
        PAIRING_REQUEST.signal(());
    }
}

/// Current split pairing state, also published as `SplitPairingEvent`.
pub fn pairing_window_open() -> bool {
    PAIRING_OPEN.load(Ordering::Acquire)
}

/// Last transition, for a status observer subscribing just after that transition.
pub fn pairing_state() -> Option<PairingState> {
    match PAIRING_STATE.load(Ordering::Acquire) {
        1 => Some(PairingState::Open),
        2 => Some(PairingState::Success),
        3 => Some(PairingState::Timeout),
        4 => Some(PairingState::Failed),
        _ => None,
    }
}

fn pairing_deadline() -> Option<Instant> {
    PAIRING_DEADLINE.lock(Cell::get)
}

fn publish_pairing_state(state: PairingState) {
    PAIRING_STATE.store(state as u8, Ordering::Release);
    crate::event::publish_event(crate::event::SplitPairingEvent {
        open: state == PairingState::Open,
        state,
    });
}

fn finish_pairing(state: PairingState) {
    debug_assert!(state != PairingState::Open);
    let was_open = PAIRING_OPEN.swap(false, Ordering::AcqRel);
    PAIRING_REQUEST_PENDING.store(false, Ordering::Release);
    PAIRING_DEADLINE.lock(|deadline| deadline.set(None));
    PAIRING_TIMER.signal(None);
    if was_open || state == PairingState::Failed {
        publish_pairing_state(state);
    }
}

/// A peer reset that could not be committed must release the old session and
/// tell the user that registration did not start.
pub(crate) fn pairing_failed() {
    finish_pairing(PairingState::Failed);
}

pub(crate) fn pairing_succeeded() {
    if pairing_deadline().is_some_and(|deadline| Instant::now() >= deadline) {
        finish_pairing(PairingState::Timeout);
    } else if pairing_window_open() {
        finish_pairing(PairingState::Success);
    }
}

/// This watchdog is independent of scanning, flash writes, and GATT discovery.
/// Their pending futures cannot keep a registration window open indefinitely.
async fn pairing_deadline_task() {
    let mut deadline = PAIRING_TIMER.wait().await;
    loop {
        deadline = match deadline {
            Some(at) => match select(PAIRING_TIMER.wait(), Timer::at(at)).await {
                Either::First(next) => next,
                Either::Second(()) => {
                    if pairing_deadline() == Some(at) {
                        finish_pairing(PairingState::Timeout);
                    }
                    PAIRING_TIMER.wait().await
                }
            },
            None => PAIRING_TIMER.wait().await,
        };
    }
}

/// Cancel an unready session at the registration deadline. Once input is ready,
/// the terminal Success state leaves this future pending for the session's life.
async fn pairing_setup_deadline(deadline: Option<Instant>) {
    if let Some(deadline) = deadline {
        Timer::at(deadline).await;
        if pairing_state() != Some(PairingState::Success) {
            if pairing_window_open() {
                finish_pairing(PairingState::Timeout);
            }
            return;
        }
    }
    core::future::pending::<()>().await;
}

/// A registration write must finish while its window is still open. A cancelled
/// flash request may already have committed, so invalidate it before returning.
async fn save_pairing_peer(peer: PeerAddress) -> Result<(), ()> {
    let mut window = PairingWindow::new();
    let Some(remaining) = window.remaining() else {
        return Err(());
    };
    let invalid = PeerAddress::new(peer.peer_id, false, [0; 6]);
    let result = with_timeout(
        remaining,
        crate::storage::store(crate::storage::StorageItem::PeerAddress(peer)),
    )
    .await;
    if matches!(result, Ok(Ok(_))) && window.remaining().is_some() {
        return Ok(());
    }
    if !matches!(result, Err(_)) && window.remaining().is_some() {
        window.fail();
    } else if pairing_window_open() {
        finish_pairing(PairingState::Timeout);
    }
    if !invalidate_pairing_peer(invalid.peer_id).await {
        // Never report success when a stale stored address could not be cleared.
        window.fail();
    }
    Err(())
}

/// Also used when GATT/input setup fails after the initial address write.
pub(crate) async fn invalidate_pairing_peer(peer_id: u8) -> bool {
    matches!(
        with_timeout(
            Duration::from_secs(5),
            crate::storage::store(crate::storage::StorageItem::PeerAddress(PeerAddress::new(
                peer_id, false, [0; 6]
            )),)
        )
        .await,
        Ok(Ok(_))
    )
}

fn registration_succeeded(deadline: Option<Instant>, ready: bool) -> bool {
    deadline.is_none() || (ready && pairing_state() == Some(PairingState::Success))
}

struct PairingWindow;

impl PairingWindow {
    fn new() -> Self {
        // An empty peer record is not permission to discover a new keyboard.
        Self
    }

    fn open(&mut self) {
        let deadline = Instant::now() + PAIRING_DURATION;
        PAIRING_DEADLINE.lock(|current| current.set(Some(deadline)));
        PAIRING_OPEN.store(true, Ordering::Release);
        PAIRING_REQUEST_PENDING.store(false, Ordering::Release);
        publish_pairing_state(PairingState::Open);
        PAIRING_TIMER.signal(Some(deadline));
    }

    fn fail(&mut self) {
        finish_pairing(PairingState::Failed);
    }

    fn remaining(&mut self) -> Option<Duration> {
        let deadline = pairing_deadline()?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining == Duration::MIN {
            finish_pairing(PairingState::Timeout);
            None
        } else {
            Some(remaining)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PeerAddress {
    pub peer_id: u8,
    pub is_valid: bool,
    pub address: [u8; 6],
}

impl PeerAddress {
    pub(crate) fn new(peer_id: u8, is_valid: bool, address: [u8; 6]) -> Self {
        Self {
            peer_id,
            is_valid,
            address,
        }
    }
}

#[derive(Default, Clone)]
pub(crate) struct GattSplitMessage {
    buf: [u8; SplitMessage::POSTCARD_MAX_SIZE],
    len: usize,
}

impl TryFrom<&SplitMessage> for GattSplitMessage {
    type Error = SplitDriverError;

    fn try_from(value: &SplitMessage) -> Result<Self, Self::Error> {
        let mut buf = [0; SplitMessage::POSTCARD_MAX_SIZE];
        let encoded = postcard::to_slice(value, &mut buf).map_err(|e| {
            error!("Postcard serialize split message error: {}", e);
            SplitDriverError::SerializeError
        })?;

        let len = encoded.len();

        // Check if slice starts at the beginning of buffer
        if encoded.as_ptr() != buf.as_ptr() {
            error!("Postcard serialize split message did not use the buffer correctly!");
            return Err(SplitDriverError::SerializeError);
        }

        Ok(Self { buf, len })
    }
}

impl AsGatt for GattSplitMessage {
    const MIN_SIZE: usize = 0;

    const MAX_SIZE: usize = SplitMessage::POSTCARD_MAX_SIZE;

    fn as_gatt(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventSubscriber, SplitPairingEvent, SubscribableEvent};
    use crate::test_support::test_block_on;

    #[test]
    fn pairing_window_closes_and_publishes_its_expiry() {
        test_block_on(async {
            PAIRING_OPEN.store(false, Ordering::Release);
            let mut events = SplitPairingEvent::subscriber();
            let mut window = PairingWindow::new();
            window.open();
            assert!(pairing_window_open());
            assert!(events.next_event().await.open);
            embassy_time::MockDriver::get().advance(PAIRING_DURATION);
            assert!(window.remaining().is_none());
            assert!(!pairing_window_open());
            assert!(!events.next_event().await.open);
            assert!(window.remaining().is_none());
            assert!(events.try_next_message_pure().is_none());
        });
    }
    #[test]
    fn startup_is_closed_until_an_explicit_request() {
        test_block_on(async {
            let mut events = SplitPairingEvent::subscriber();
            let mut window = PairingWindow::new();
            assert!(!pairing_window_open());
            assert_eq!(pairing_state(), None);
            assert!(window.remaining().is_none());
            embassy_time::MockDriver::get().advance(PAIRING_DURATION);
            assert!(events.try_next_message_pure().is_none());
            request_pairing();
            assert!(PAIRING_REQUEST.try_take().is_some());
            // The lifecycle opens only after the old address was durably cleared.
            assert!(!pairing_window_open());
            window.open();
            assert_eq!(events.next_event().await.state, PairingState::Open);
        });
    }

    #[test]
    fn duplicate_physical_and_user7_requests_do_not_restart_the_window() {
        test_block_on(async {
            request_pairing();
            assert!(PAIRING_REQUEST.try_take().is_some());
            request_pairing();
            assert!(
                PAIRING_REQUEST.try_take().is_none(),
                "flash clear is already pending"
            );
            let mut window = PairingWindow::new();
            window.open();
            let deadline = pairing_deadline();
            embassy_time::MockDriver::get().advance(Duration::from_secs(5));
            request_pairing();
            assert!(PAIRING_REQUEST.try_take().is_none());
            assert_eq!(pairing_deadline(), deadline);
            pairing_succeeded();
            request_pairing();
            assert!(
                PAIRING_REQUEST.try_take().is_some(),
                "a later deliberate attempt remains possible"
            );
        });
    }

    #[test]
    fn watchdog_expires_without_lifecycle_or_gatt_progress() {
        test_block_on(async {
            let mut events = SplitPairingEvent::subscriber();
            PairingWindow::new().open();
            assert_eq!(events.next_event().await.state, PairingState::Open);
            let observer = async {
                Timer::after_millis(1).await;
                embassy_time::MockDriver::get().advance(PAIRING_DURATION);
                assert_eq!(events.next_event().await.state, PairingState::Timeout);
                assert!(!pairing_window_open());
                assert!(pairing_deadline().is_none());
                assert!(events.try_next_message_pure().is_none());
            };
            assert!(matches!(
                select(observer, pairing_deadline_task()).await,
                Either::First(())
            ));
        });
    }

    #[test]
    fn successful_input_setup_cancels_the_old_timeout() {
        test_block_on(async {
            let mut events = SplitPairingEvent::subscriber();
            PairingWindow::new().open();
            events.next_event().await;
            let observer = async {
                Timer::after_millis(1).await;
                pairing_succeeded();
                assert_eq!(events.next_event().await.state, PairingState::Success);
                embassy_time::MockDriver::get().advance(PAIRING_DURATION);
                Timer::after_millis(1).await;
                assert_eq!(pairing_state(), Some(PairingState::Success));
                assert!(events.try_next_message_pure().is_none());
            };
            select(observer, pairing_deadline_task()).await;
        });
    }

    #[test]
    fn setup_cannot_report_success_after_the_deadline() {
        test_block_on(async {
            let mut events = SplitPairingEvent::subscriber();
            PairingWindow::new().open();
            events.next_event().await;
            embassy_time::MockDriver::get().advance(PAIRING_DURATION);
            pairing_succeeded();
            assert_eq!(events.next_event().await.state, PairingState::Timeout);
            assert!(!pairing_window_open());
        });
    }

    #[test]
    fn unready_session_is_cancelled_at_the_pairing_deadline() {
        test_block_on(async {
            PairingWindow::new().open();
            let advance = async {
                Timer::after_millis(1).await;
                embassy_time::MockDriver::get().advance(PAIRING_DURATION);
                core::future::pending::<()>().await;
            };
            assert!(matches!(
                select(pairing_setup_deadline(pairing_deadline()), advance).await,
                Either::First(())
            ));
            assert_eq!(pairing_state(), Some(PairingState::Timeout));
        });
    }
    #[test]
    fn a_flash_reply_after_expiry_cannot_register_a_new_peer() {
        test_block_on(async {
            crate::storage::clear_flash_channel();
            PairingWindow::new().open();
            let delayed_flash = async {
                Timer::after_millis(1).await;
                embassy_time::MockDriver::get().advance(PAIRING_DURATION);
                crate::storage::drain_flash_channel().await;
            };
            let saved = select(
                save_pairing_peer(PeerAddress::new(0, true, [7; 6])),
                delayed_flash,
            )
            .await;
            assert!(matches!(saved, Either::First(Err(()))));
            assert_eq!(pairing_state(), Some(PairingState::Timeout));
            assert!(!pairing_window_open());
        });
    }

    #[test]
    fn session_keeps_its_deadline_after_watchdog_clears_global_state() {
        test_block_on(async {
            PairingWindow::new().open();
            let deadline = pairing_deadline();
            embassy_time::MockDriver::get().advance(PAIRING_DURATION);
            finish_pairing(PairingState::Timeout);
            assert!(pairing_deadline().is_none());
            pairing_setup_deadline(deadline).await;
            assert_eq!(pairing_state(), Some(PairingState::Timeout));
        });
    }
    #[test]
    fn only_a_ready_ack_keeps_a_new_registration_for_reconnect() {
        test_block_on(async {
            PairingWindow::new().open();
            let deadline = pairing_deadline();
            assert!(!registration_succeeded(deadline, false));
            assert!(
                !registration_succeeded(deadline, true),
                "transport readiness alone is insufficient"
            );
            pairing_succeeded();
            assert!(registration_succeeded(deadline, true));
            assert!(!registration_succeeded(deadline, false));
            assert!(
                registration_succeeded(None, false),
                "an existing saved peer survives a radio timeout"
            );
        });
    }

    #[test]
    fn timeout_requires_rollback_even_if_an_ack_arrived_too_late() {
        test_block_on(async {
            PairingWindow::new().open();
            let deadline = pairing_deadline();
            embassy_time::MockDriver::get().advance(PAIRING_DURATION);
            pairing_succeeded();
            assert_eq!(pairing_state(), Some(PairingState::Timeout));
            assert!(!registration_succeeded(deadline, true));
        });
    }
}
