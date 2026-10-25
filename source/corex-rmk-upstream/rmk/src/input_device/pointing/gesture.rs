//! Hold-to-flick recognition and a bounded, local shortcut queue.
//!
//! Assignments live in ordinary Vial keymap cells. This module does not change
//! the storage schema or serialize virtual events over the split connection.
use core::cell::RefCell;

use embassy_sync::{blocking_mutex::Mutex, channel::Channel};
use embassy_time::Instant;
use rmk_types::{
    action::{Action, KeyAction},
    keycode::{HidKeyCode, KeyCode},
    modifier::ModifierCombination,
};

use crate::{
    RawMutex,
    channel::OutputTarget,
    event::{KeyboardEvent, KeyboardEventPos},
    keymap::KeyMap,
};

/// Up, down, left, right; deliberately independent of physical key positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// A single flick per hold. Slow drift and ambiguous diagonals do not fire.
#[derive(Clone, Copy, Debug)]
pub struct Recognizer {
    threshold: i32,
    x: i32,
    y: i32,
    window: Option<u64>,
    fired: bool,
}
impl Recognizer {
    pub const fn new(threshold: u16) -> Self {
        Self {
            threshold: threshold as i32,
            x: 0,
            y: 0,
            window: None,
            fired: false,
        }
    }
    pub fn motion(&mut self, x: i16, y: i16, now_ms: u64) -> Option<Direction> {
        if self.fired {
            return None;
        }
        if self
            .window
            .is_none_or(|start| now_ms.saturating_sub(start) > 240)
        {
            self.x = 0;
            self.y = 0;
            self.window = Some(now_ms);
        }
        self.x = self
            .x
            .saturating_add(i32::from(x))
            .clamp(-1_000_000, 1_000_000);
        self.y = self
            .y
            .saturating_add(i32::from(y))
            .clamp(-1_000_000, 1_000_000);
        let ax = self.x.abs();
        let ay = self.y.abs();
        let direction = if ax >= self.threshold && ax * 2 > ay * 3 {
            Some(if self.x < 0 {
                Direction::Left
            } else {
                Direction::Right
            })
        } else if ay >= self.threshold && ay * 2 > ax * 3 {
            Some(if self.y < 0 {
                Direction::Up
            } else {
                Direction::Down
            })
        } else {
            None
        };
        self.fired = direction.is_some();
        direction
    }
}

#[derive(Clone, Copy)]
pub struct Config {
    pub trigger_user: u8,
    /// (layer, row, column); up, down, left, right, then sensitivity.
    pub cells: [(u8, u8, u8); 5],
    pub sensitivity_users: [u8; 3],
    pub thresholds: [u16; 3],
}
impl Config {
    fn values(self, keymap: &KeyMap<'_>) -> [KeyAction; 5] {
        self.cells
            .map(|(layer, row, col)| keymap.action_at_pos(usize::from(layer), row, col))
    }
    fn threshold(self, setting: KeyAction) -> Option<u16> {
        let KeyAction::Single(Action::User(id)) = setting else {
            return None;
        };
        self.sensitivity_users
            .iter()
            .position(|value| *value == id)
            .map(|i| self.thresholds[i])
    }
}

struct Session {
    owner: KeyboardEventPos,
    values: [KeyAction; 5],
    recognizer: Recognizer,
    target: OutputTarget,
    started: Instant,
}
struct Runtime {
    config: Option<Config>,
    session: Option<Session>,
    generation: u32,
}
static RUNTIME: Mutex<RawMutex, RefCell<Runtime>> = Mutex::new(RefCell::new(Runtime {
    config: None,
    session: None,
    generation: 0,
}));

#[derive(Clone, Copy, Debug)]
pub(crate) struct Shortcut {
    pub key: HidKeyCode,
    pub modifiers: ModifierCombination,
    pub target: OutputTarget,
    pub generation: u32,
    pub queued: Instant,
}
pub(crate) static SHORTCUTS: Channel<RawMutex, Shortcut, 2> = Channel::new();

/// Only paired keyboard shortcuts are accepted. Layer toggles, macros, mouse
/// holds and tap dances have different lifetime semantics and are not executed.
pub fn shortcut(action: KeyAction) -> Option<(HidKeyCode, ModifierCombination)> {
    let (key, mods) = match action {
        KeyAction::Single(Action::Key(KeyCode::Hid(key))) => (key, ModifierCombination::new()),
        KeyAction::Single(Action::KeyWithModifier(key, mods)) => (key, mods),
        _ => return None,
    };
    let value = key as u16;
    if (4..=0xa4).contains(&value) || (0xe0..=0xe7).contains(&value) {
        Some((key, mods))
    } else {
        None
    }
}

/// Install board-specific slot addresses once, after restoring the keymap.
pub fn configure(config: Config) {
    cancel();
    RUNTIME.lock(|runtime| runtime.borrow_mut().config = Some(config));
}
/// Cancels in-flight recognition and queued output, e.g. on settings changes.
pub fn cancel() {
    RUNTIME.lock(|runtime| {
        let mut runtime = runtime.borrow_mut();
        runtime.session = None;
        runtime.generation = runtime.generation.wrapping_add(1);
    });
}
/// Handle release before looking up its action, even if Vial remapped the held key.
pub(crate) fn release(event: KeyboardEvent) {
    if !event.pressed {
        RUNTIME.lock(|runtime| {
            let mut runtime = runtime.borrow_mut();
            if runtime
                .session
                .as_ref()
                .is_some_and(|session| session.owner == event.pos)
            {
                runtime.session = None;
            }
        });
    }
}
/// Called synchronously by the keyboard executor, including combo outputs.
pub(crate) fn action(keymap: &KeyMap<'_>, action: Action, event: KeyboardEvent) {
    release(event);
    if !event.pressed {
        return;
    }
    RUNTIME.lock(|runtime| {
        let mut runtime = runtime.borrow_mut();
        let Some(config) = runtime.config else {
            return;
        };
        if action != Action::User(config.trigger_user) || runtime.session.is_some() {
            return;
        }
        let values = config.values(keymap);
        let Some(threshold) = config.threshold(values[4]) else {
            return;
        };
        let Some(target) = OutputTarget::capture() else {
            return;
        };
        runtime.session = Some(Session {
            owner: event.pos,
            values,
            recognizer: Recognizer::new(threshold),
            target,
            started: Instant::now(),
        });
    });
}

/// Consume sensor counts in cursor-axis directions, before cursor gain or
/// auto-mouse-layer logic. The board must apply its mounting-axis transform.
/// No await: a full output queue drops this flick instead of blocking the sensor.
pub fn consume_motion(keymap: &KeyMap<'_>, x: i16, y: i16) -> bool {
    RUNTIME.lock(|runtime| {
        let mut runtime = runtime.borrow_mut();
        let Some(config) = runtime.config else {
            return false;
        };
        let generation = runtime.generation;
        let Some(session) = runtime.session.as_mut() else {
            return false;
        };
        if !session.target.is_current()
            || config.values(keymap) != session.values
            || session.started.elapsed().as_secs() >= 5
        {
            runtime.session = None;
            runtime.generation = generation.wrapping_add(1);
            return false;
        }
        if let Some(direction) = session.recognizer.motion(x, y, Instant::now().as_millis()) {
            if let Some((key, modifiers)) = shortcut(session.values[direction as usize]) {
                let _ = SHORTCUTS.try_send(Shortcut {
                    key,
                    modifiers,
                    target: session.target,
                    generation,
                    queued: Instant::now(),
                });
            }
        }
        true
    })
}
pub(crate) fn is_current(request: Shortcut) -> bool {
    request.target.is_current()
        && request.queued.elapsed().as_millis() <= 250
        && RUNTIME.lock(|runtime| runtime.borrow().generation == request.generation)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn four_directions_and_one_flick_per_hold() {
        for (x, y, direction) in [
            (0, -80, Direction::Up),
            (0, 80, Direction::Down),
            (-80, 0, Direction::Left),
            (80, 0, Direction::Right),
        ] {
            let mut recognizer = Recognizer::new(80);
            assert_eq!(recognizer.motion(x / 2, y / 2, 0), None);
            assert_eq!(recognizer.motion(x / 2, y / 2, 40), Some(direction));
            assert_eq!(recognizer.motion(x, y, 80), None);
        }
    }
    #[test]
    fn diagonals_jitter_and_slow_drift_do_not_fire() {
        let mut diagonal = Recognizer::new(80);
        assert_eq!(diagonal.motion(200, 200, 0), None);
        let mut drift = Recognizer::new(80);
        for time in 0..100 {
            assert_eq!(drift.motion(10, 0, time * 250), None);
        }
        let mut jitter = Recognizer::new(80);
        for _ in 0..100 {
            assert_eq!(jitter.motion(20, -20, 0), None);
            assert_eq!(jitter.motion(-20, 20, 1), None);
        }
    }
    #[test]
    fn sensitivity_has_three_meaningful_thresholds() {
        for threshold in [40, 80, 120] {
            let mut recognizer = Recognizer::new(threshold);
            assert_eq!(recognizer.motion(threshold as i16 - 1, 0, 0), None);
            assert_eq!(recognizer.motion(1, 0, 1), Some(Direction::Right));
        }
    }
    #[test]
    fn only_shortcuts_can_be_gesture_outputs() {
        assert!(shortcut(crate::k!(A)).is_some());
        assert!(
            shortcut(KeyAction::Single(Action::KeyWithModifier(
                HidKeyCode::Left,
                ModifierCombination::new().with_left_ctrl(true)
            )))
            .is_some()
        );
        for action in [
            crate::k!(MouseBtn1),
            crate::k!(AudioVolUp),
            crate::mo!(1),
            KeyAction::Single(Action::TriggerMacro(0)),
            KeyAction::Single(Action::User(28)),
            KeyAction::No,
        ] {
            assert!(shortcut(action).is_none());
        }
    }
}
