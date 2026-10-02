//! All state and key handling. No drawing and no terminal: tests build an [`App`], feed it
//! keys and check the state.

mod effect;
mod explore;
mod library;
mod setup;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub use effect::Effect;
pub use library::{LibraryChoice, fingerprint_text};
pub use setup::{KeySource, KeyStatus, Row, SIZES, SetupForm, loaded_text};

use crate::config::TuiConfig;
use crate::files::Files;
use crate::random::{RandomError, RandomFn};
use setup::Outcome;

/// Which screen is active. Only the active mode's handler sees a key, so the same key can
/// mean different things on different screens.
#[derive(Debug, Clone)]
pub enum Mode {
    /// Choosing a size (and later a key).
    Setup(SetupForm),
    /// Browsing pages.
    Explore,
}

/// The character a key types into a text box, if it types one. Ctrl or Alt alone are
/// shortcuts, not typing; Ctrl+Alt together is how AltGr reports `@`, `~` and `\` on Windows.
fn typed_char(key: &KeyEvent) -> Option<char> {
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    (ctrl == alt).then_some(c)
}

/// The whole state of the explorer.
pub struct App {
    mode: Mode,
    /// The library of this session. `None` while on the setup screen.
    library: Option<LibraryChoice>,
    // Used by the random page key.
    #[allow(dead_code)]
    random: RandomFn,
    files: Box<dyn Files>,
}

impl App {
    /// A new app. With a size it starts exploring that library (canonical unless there is
    /// a key); without one it opens the setup screen. `random` is where every random byte
    /// comes from, and `files` is how it reads files.
    pub fn new(config: TuiConfig, random: RandomFn, files: Box<dyn Files>) -> Self {
        let (mode, library) = match config.size {
            Some(size) => (Mode::Explore, Some(LibraryChoice::new(size, config.key))),
            None => (Mode::Setup(SetupForm::new(config.key)), None),
        };
        App {
            mode,
            library,
            random,
            files,
        }
    }

    /// The active screen.
    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    /// The library of this session, or `None` while choosing one.
    pub fn library(&self) -> Option<&LibraryChoice> {
        self.library.as_ref()
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
        match &mut self.mode {
            Mode::Setup(form) => match form.handle_key(key, self.files.as_ref()) {
                Outcome::Editing(effects) => effects,
                Outcome::Start(library) => {
                    self.library = Some(library);
                    self.mode = Mode::Explore;
                    vec![]
                }
            },
            Mode::Explore => explore::handle_key(self, key),
        }
    }

    /// Goes back to a fresh setup screen: a new session with no library, no page and no
    /// history. A key from the command line is not carried over.
    fn start_over(&mut self) {
        self.mode = Mode::Setup(SetupForm::new(None));
        self.library = None;
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
    use bitbabel_core::{Key, LibraryConfig};
    use ratatui::crossterm::event::KeyEventState;

    use super::*;
    use crate::files::MemFiles;

    fn fake_random(buf: &mut [u8]) -> Result<(), RandomError> {
        buf.fill(0xAB);
        Ok(())
    }

    fn failing_random(_: &mut [u8]) -> Result<(), RandomError> {
        Err(RandomError::new("blocked"))
    }

    fn app() -> App {
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: None,
        };
        App::new(config, fake_random, Box::new(MemFiles::default()))
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn q_quits_and_changes_nothing() {
        let mut app = app();
        assert_eq!(app.handle_key(press(KeyCode::Char('q'))), [Effect::Quit]);
        assert!(matches!(app.mode(), Mode::Explore));
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
        let failing = App::new(
            TuiConfig::default(),
            failing_random,
            Box::new(MemFiles::default()),
        );
        assert_eq!(
            failing.random_bytes(3).unwrap_err(),
            RandomError::new("blocked")
        );
    }

    #[test]
    fn a_size_starts_exploring_that_library() {
        let app = app();
        assert!(matches!(app.mode(), Mode::Explore));
        let library = app.library().unwrap();
        assert_eq!(library.size(), LibraryConfig::SMALL);
        assert!(library.key().is_none());
    }

    #[test]
    fn a_size_with_a_key_starts_exploring_the_private_library() {
        let config = TuiConfig {
            size: Some(LibraryConfig::LARGE),
            key: Some(Key::from_bytes([1; 32])),
        };
        let app = App::new(config, fake_random, Box::new(MemFiles::default()));
        assert!(app.library().unwrap().key().is_some());
    }

    #[test]
    fn no_size_opens_setup_and_enter_starts_exploring() {
        let mut app = App::new(
            TuiConfig::default(),
            fake_random,
            Box::new(MemFiles::default()),
        );
        assert!(matches!(app.mode(), Mode::Setup(_)));
        assert!(app.library().is_none());

        assert!(app.handle_key(press(KeyCode::Enter)).is_empty());
        assert!(matches!(app.mode(), Mode::Explore));
        assert_eq!(app.library().unwrap().size(), LibraryConfig::MEDIUM);
    }

    #[test]
    fn q_quits_from_setup_and_ctrl_c_too() {
        let mut app = App::new(
            TuiConfig::default(),
            fake_random,
            Box::new(MemFiles::default()),
        );
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(ctrl_c), [Effect::Quit]);
        assert_eq!(app.handle_key(press(KeyCode::Char('q'))), [Effect::Quit]);
    }

    #[test]
    fn typed_char_follows_the_modifier_rules() {
        let key = |modifiers| KeyEvent::new(KeyCode::Char('a'), modifiers);
        assert_eq!(typed_char(&key(KeyModifiers::NONE)), Some('a'));
        assert_eq!(typed_char(&key(KeyModifiers::SHIFT)), Some('a'));
        assert_eq!(typed_char(&key(KeyModifiers::CONTROL)), None);
        assert_eq!(typed_char(&key(KeyModifiers::ALT)), None);
        let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
        assert_eq!(typed_char(&key(altgr)), Some('a'));
        assert_eq!(typed_char(&press(KeyCode::Enter)), None);
    }

    #[test]
    fn a_key_file_from_the_setup_screen_names_the_library() {
        let files = MemFiles::default().with("my.key", &[9; 32]);
        let mut app = App::new(TuiConfig::default(), fake_random, Box::new(files));
        let keys = [
            KeyCode::Down,
            KeyCode::Right,
            KeyCode::Down,
            KeyCode::Char('m'),
            KeyCode::Char('y'),
            KeyCode::Char('.'),
            KeyCode::Char('k'),
            KeyCode::Char('e'),
            KeyCode::Char('y'),
            KeyCode::Enter,
            KeyCode::Enter,
        ];
        for code in keys {
            assert!(app.handle_key(press(code)).is_empty());
        }
        assert!(matches!(app.mode(), Mode::Explore));
        assert_eq!(
            app.library().unwrap().key(),
            Some(&Key::from_bytes([9; 32]))
        );
    }

    #[test]
    fn s_starts_over_from_explore() {
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: Some(Key::from_bytes([1; 32])),
        };
        let mut app = App::new(config, fake_random, Box::new(MemFiles::default()));
        assert!(app.handle_key(press(KeyCode::Char('s'))).is_empty());
        assert!(app.library().is_none());
        let Mode::Setup(form) = app.mode() else {
            panic!("should be on the setup screen");
        };
        assert_eq!(form.size(), LibraryConfig::MEDIUM);
        assert!(form.fixed_key().is_none());
    }

    #[test]
    fn explore_ignores_keys_with_modifiers() {
        let mut app = app();
        let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::ALT);
        assert!(app.handle_key(key).is_empty());
        assert!(app.library().is_some());
    }
}
