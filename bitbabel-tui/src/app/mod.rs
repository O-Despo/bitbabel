//! All state and key handling. No drawing and no terminal: tests build an [`App`], feed it
//! keys and check the state.

mod effect;
mod explore;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub use effect::Effect;

use crate::random::{RandomError, RandomFn};

/// Which screen is active. Only the active mode's handler sees a key, so the same key can
/// mean different things on different screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Browsing pages.
    Explore,
}

/// The whole state of the explorer.
pub struct App {
    mode: Mode,
    // Used by the random page key.
    #[allow(dead_code)]
    random: RandomFn,
}

impl App {
    /// A new app in [`Mode::Explore`]. `random` is where every random byte comes from.
    pub fn new(random: RandomFn) -> Self {
        App {
            mode: Mode::Explore,
            random,
        }
    }

    /// The active screen.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Handles one key event and returns what the loop should do about it.
    ///
    /// Only presses count: Windows also sends a release for every tap, which would run each
    /// command twice. Ctrl+C quits from every mode.
    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.kind != KeyEventKind::Press {
            return vec![];
        }
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
            return vec![Effect::Quit];
        }
        match self.mode {
            Mode::Explore => explore::handle_key(key),
        }
    }

    /// `len` random bytes from the injected source.
    ///
    /// # Errors
    ///
    /// [`RandomError`] if the source fails.
    #[allow(dead_code)]
    pub(crate) fn random_bytes(&self, len: usize) -> Result<Vec<u8>, RandomError> {
        let mut bytes = vec![0u8; len];
        (self.random)(&mut bytes)?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod test {
    use ratatui::crossterm::event::KeyEventState;

    use super::*;

    fn fake_random(buf: &mut [u8]) -> Result<(), RandomError> {
        buf.fill(0xAB);
        Ok(())
    }

    fn failing_random(_: &mut [u8]) -> Result<(), RandomError> {
        Err(RandomError::new("blocked"))
    }

    fn app() -> App {
        App::new(fake_random)
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn q_quits_and_changes_nothing() {
        let mut app = app();
        assert_eq!(app.handle_key(press(KeyCode::Char('q'))), [Effect::Quit]);
        assert_eq!(app.mode(), Mode::Explore);
    }

    #[test]
    fn ctrl_c_quits() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app().handle_key(key), [Effect::Quit]);
    }

    #[test]
    fn other_keys_do_nothing() {
        let mut app = app();
        assert!(app.handle_key(press(KeyCode::Char('x'))).is_empty());
        assert!(app.handle_key(press(KeyCode::Enter)).is_empty());
        // Plain `c` is not Ctrl+C.
        assert!(app.handle_key(press(KeyCode::Char('c'))).is_empty());
    }

    #[test]
    fn releases_and_repeats_are_ignored() {
        let mut app = app();
        for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
            let key = KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                kind,
                state: KeyEventState::NONE,
            };
            assert!(app.handle_key(key).is_empty());
        }
    }

    #[test]
    fn random_bytes_come_from_the_injected_source() {
        assert_eq!(app().random_bytes(3).unwrap(), [0xAB; 3]);
        let failing = App::new(failing_random);
        assert_eq!(
            failing.random_bytes(3).unwrap_err(),
            RandomError::new("blocked")
        );
    }
}
