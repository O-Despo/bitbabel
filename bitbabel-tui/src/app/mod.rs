//! All state and key handling. No drawing and no terminal: tests build an [`App`], feed it
//! keys and check the state.

mod bookmark_list;
mod effect;
mod explore;
mod prompt;
mod search;
mod session;
mod setup;
#[cfg(test)]
pub(crate) mod test_util;
mod viewport;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub use bitbabel_explorer::{LibraryChoice, SIZES, fingerprint_groups, fingerprint_text};
pub use effect::Effect;
pub use prompt::Prompt;
pub use search::{RESULTS, SearchForm};
pub use session::Session;
pub use setup::{KeySource, KeyStatus, Row, SetupForm, loaded_text};

use crate::bookmarks::{Bookmarks, LockedBookmarks, StartError};
use crate::clock::{NowFn, system_now};
use crate::config::TuiConfig;
use crate::files::Files;
use crate::keymap;
use crate::random::{RandomError, RandomFn};
use setup::Outcome;

/// Which screen is active. Only the active mode's handler sees a key, so the same key can
/// mean different things on different screens.
#[derive(Debug)]
pub enum Mode {
    /// Choosing a size (and later a key).
    Setup(SetupForm),
    /// Browsing pages. The session is part of the mode, so there is no way to be exploring
    /// without one.
    Explore(Box<Session>),
}

/// Rows of the explore screen that are not page lines: the border (2), the index line, the
/// address line, the status line and the hint line.
pub const EXPLORE_CHROME_ROWS: u16 = 6;

/// The terminal height assumed until the loop reports the real one: the smallest that is allowed.
const DEFAULT_HEIGHT: u16 = 24;

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

/// How many page lines fit in a terminal `rows` tall.
fn viewport_for(rows: u16) -> usize {
    usize::from(rows.saturating_sub(EXPLORE_CHROME_ROWS))
}

/// The whole state of the explorer.
pub struct App {
    mode: Mode,
    /// How many page lines fit on screen.
    viewport: usize,
    random: RandomFn,
    now: NowFn,
    files: Box<dyn Files>,
}

impl App {
    /// A new app. With a bookmark file it starts exploring that file's library, locked to
    /// it. Otherwise, with a size it starts exploring that library (canonical unless there is
    /// a key), and without one it opens the setup screen. `random` is where every random byte
    /// comes from, and `files` is how it reads files.
    ///
    /// # Errors
    ///
    /// [`StartError`] if a bookmark file was given and cannot be used: it is missing or
    /// invalid, or a size or key given with it does not match. There is no library to start in
    /// then, so this is fatal.
    pub fn new(
        config: TuiConfig,
        random: RandomFn,
        files: Box<dyn Files>,
    ) -> Result<Self, StartError> {
        let viewport = viewport_for(DEFAULT_HEIGHT);
        if let Some(path) = &config.bookmarks {
            let (library, locked) =
                LockedBookmarks::open(files.as_ref(), path, config.size, config.key)?;
            let mut session =
                Session::new(library, viewport).map_err(|error| StartError::Library {
                    path: path.clone(),
                    error,
                })?;
            session.set_bookmarks(Bookmarks::Locked(locked));
            return Ok(App {
                mode: Mode::Explore(Box::new(session)),
                viewport,
                random,
                now: system_now,
                files,
            });
        }
        let mut form = SetupForm::new(config.key.clone());
        let mode = match config.size {
            None => Mode::Setup(form),
            Some(size) => match Session::new(LibraryChoice::new(size, config.key), viewport) {
                Ok(session) => Mode::Explore(Box::new(session)),
                // The presets are always valid, so this is not expected from the CLI. The
                // message is shown on the setup screen, on the size that was asked for.
                Err(error) => {
                    form.select_size(size);
                    form.set_error(error.to_string());
                    Mode::Setup(form)
                }
            },
        };
        Ok(App {
            mode,
            viewport,
            random,
            now: system_now,
            files,
        })
    }

    /// The same app with another clock. The app reads the time from the system unless a test
    /// replaces it.
    #[cfg(test)]
    pub(crate) fn with_clock(mut self, now: NowFn) -> Self {
        self.now = now;
        self
    }

    /// Tells the app how tall the terminal is, so the page knows how many lines fit. Called
    /// at start and on every resize.
    pub fn set_terminal_height(&mut self, rows: u16) {
        self.viewport = viewport_for(rows);
        if let Mode::Explore(session) = &mut self.mode {
            session.set_viewport(self.viewport);
        }
    }

    /// The active screen.
    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    /// The library of this session, or `None` while choosing one.
    #[cfg(test)]
    pub fn library(&self) -> Option<&LibraryChoice> {
        match &self.mode {
            Mode::Explore(session) => Some(session.library()),
            Mode::Setup(_) => None,
        }
    }

    /// Handles one key event and returns what the loop should do about it.
    ///
    /// Only presses count: Windows also sends a release for every tap, which would run each
    /// command twice. Ctrl+C quits from every mode.
    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.kind != KeyEventKind::Press {
            return vec![];
        }
        if keymap::is_interrupt(&key) {
            return vec![Effect::Quit];
        }
        match &mut self.mode {
            Mode::Setup(form) => match form.handle_key(key, self.files.as_ref()) {
                Outcome::Editing(effects) => effects,
                Outcome::Start(library) => {
                    match Session::new(library, self.viewport) {
                        Ok(session) => self.mode = Mode::Explore(Box::new(session)),
                        Err(error) => form.set_error(error.to_string()),
                    }
                    vec![]
                }
            },
            Mode::Explore(_) => explore::handle_key(self, key),
        }
    }

    /// Goes back to a fresh setup screen: a new session with no library, no page and no
    /// history. A key from the command line is not carried over.
    fn start_over(&mut self) {
        self.mode = Mode::Setup(SetupForm::new(None));
    }

    /// `len` random bytes from the injected source.
    ///
    /// # Errors
    ///
    /// [`RandomError`] if the source fails.
    pub(crate) fn random_bytes(&self, len: usize) -> Result<Vec<u8>, RandomError> {
        let mut bytes = vec![0u8; len];
        (self.random)(&mut bytes)?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod test {
    use bitbabel_core::{Key, LibraryConfig};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventState, KeyModifiers};

    use super::test_util::*;
    use super::*;
    use crate::files::MemFiles;

    /// A bookmark file of `size` with `count` bookmarks, for the canonical key.
    fn bookmark_bytes(size: LibraryConfig, count: u64) -> Vec<u8> {
        use bitbabel_file::{BookmarkFile, BookmarkKey, BookmarkSettings};
        let settings = BookmarkSettings::new(size, BookmarkKey::Canonical).unwrap();
        let mut file = BookmarkFile::new(settings);
        for n in 1..=count {
            let index = bitbabel_core::PageIndex::from_u64(n, size.page_len());
            file.push(n, &format!("mark {n}"), index).unwrap();
        }
        file.to_bytes()
    }

    fn app_with_bookmarks(
        bytes: &[u8],
        size: Option<LibraryConfig>,
    ) -> Result<App, crate::StartError> {
        let files = MemFiles::default().with("dir/favs.babelbookmark", bytes);
        let config = TuiConfig {
            size,
            key: None,
            bookmarks: Some("dir/favs.babelbookmark".into()),
        };
        App::new(config, fake_random, Box::new(files))
    }

    #[test]
    fn a_bookmark_file_starts_the_session_locked_to_its_library() {
        let app = app_with_bookmarks(&bookmark_bytes(LibraryConfig::SMALL, 2), None).unwrap();
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        assert_eq!(session.library().label(), "small · canonical");
        assert_eq!(session.bookmarks().title(), "locked: favs (2)");
    }

    #[test]
    fn a_session_without_a_bookmark_file_says_so() {
        let app = small_app();
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        assert_eq!(session.bookmarks().title(), "no bookmark file");
    }

    #[test]
    fn a_bookmark_file_that_disagrees_with_the_flags_is_a_start_error() {
        let bytes = bookmark_bytes(LibraryConfig::MEDIUM, 1);
        let error = app_with_bookmarks(&bytes, Some(LibraryConfig::SMALL))
            .err()
            .unwrap();
        assert!(error.to_string().contains("does not match"), "{error}");

        let files = MemFiles::default();
        let config = TuiConfig {
            size: None,
            key: None,
            bookmarks: Some("nope.babelbookmark".into()),
        };
        let error = App::new(config, fake_random, Box::new(files))
            .err()
            .unwrap();
        assert!(error.to_string().contains("cannot read nope.babelbookmark"));
    }

    #[test]
    fn start_over_leaves_the_locked_session() {
        let mut app = app_with_bookmarks(&bookmark_bytes(LibraryConfig::SMALL, 1), None).unwrap();
        app.handle_key(press(KeyCode::Char('s')));
        assert!(matches!(app.mode(), Mode::Setup(_)));
    }

    #[test]
    fn q_quits_and_changes_nothing() {
        let mut app = small_app();
        assert_eq!(app.handle_key(press(KeyCode::Char('q'))), [Effect::Quit]);
        assert!(matches!(app.mode(), Mode::Explore(_)));
    }

    #[test]
    fn ctrl_c_quits() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(small_app().handle_key(key), [Effect::Quit]);
    }

    #[test]
    fn other_keys_do_nothing() {
        let mut app = small_app();
        assert!(app.handle_key(press(KeyCode::Char('x'))).is_empty());
        assert!(app.handle_key(press(KeyCode::Enter)).is_empty());
        // Plain `c` is not Ctrl+C.
        assert!(app.handle_key(press(KeyCode::Char('c'))).is_empty());
    }

    #[test]
    fn releases_and_repeats_are_ignored() {
        let mut app = small_app();
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
        assert_eq!(small_app().random_bytes(3).unwrap(), [0xAB; 3]);
        let failing = App::new(
            TuiConfig::default(),
            failing_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        assert_eq!(
            failing.random_bytes(3).unwrap_err(),
            RandomError::new("blocked")
        );
    }

    #[test]
    fn a_size_starts_exploring_that_library() {
        let app = small_app();
        assert!(matches!(app.mode(), Mode::Explore(_)));
        let library = app.library().unwrap();
        assert_eq!(library.size(), LibraryConfig::SMALL);
        assert!(library.key().is_none());
    }

    #[test]
    fn a_size_with_a_key_starts_exploring_the_private_library() {
        let config = TuiConfig {
            size: Some(LibraryConfig::LARGE),
            key: Some(Key::from_bytes([1; 32])),
            bookmarks: None,
        };
        let app = App::new(config, fake_random, Box::new(MemFiles::default())).unwrap();
        assert!(app.library().unwrap().key().is_some());
    }

    #[test]
    fn no_size_opens_setup_and_enter_starts_exploring() {
        let mut app = App::new(
            TuiConfig::default(),
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        assert!(matches!(app.mode(), Mode::Setup(_)));
        assert!(app.library().is_none());

        assert!(app.handle_key(press(KeyCode::Enter)).is_empty());
        assert!(matches!(app.mode(), Mode::Explore(_)));
        assert_eq!(app.library().unwrap().size(), LibraryConfig::MEDIUM);
    }

    #[test]
    fn q_quits_from_setup_and_ctrl_c_too() {
        let mut app = App::new(
            TuiConfig::default(),
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
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
        let mut app = App::new(TuiConfig::default(), fake_random, Box::new(files)).unwrap();
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
        assert!(matches!(app.mode(), Mode::Explore(_)));
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
            bookmarks: None,
        };
        let mut app = App::new(config, fake_random, Box::new(MemFiles::default())).unwrap();
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
        let mut app = small_app();
        let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::ALT);
        assert!(app.handle_key(key).is_empty());
        assert!(app.library().is_some());
    }

    #[test]
    fn a_library_that_cannot_open_shows_setup_with_the_reason() {
        // An odd page length is not a valid library.
        let config = TuiConfig {
            size: Some(LibraryConfig::new(15, 8)),
            key: None,
            bookmarks: None,
        };
        let app = App::new(config, fake_random, Box::new(MemFiles::default())).unwrap();
        let Mode::Setup(form) = app.mode() else {
            panic!("should fall back to the setup screen");
        };
        assert_eq!(
            form.error(),
            Some("page length must be even and non-zero, got 15")
        );
    }

    #[test]
    fn a_setup_error_is_cleared_by_the_next_key() {
        let config = TuiConfig {
            size: Some(LibraryConfig::new(15, 8)),
            key: None,
            bookmarks: None,
        };
        let mut app = App::new(config, fake_random, Box::new(MemFiles::default())).unwrap();
        app.handle_key(press(KeyCode::Down));
        let Mode::Setup(form) = app.mode() else {
            panic!("should still be on the setup screen");
        };
        assert_eq!(form.error(), None);
    }

    #[test]
    fn a_taller_terminal_shows_more_lines_and_clamps_the_scroll() {
        let mut app = medium_app();
        app.handle_key(press(KeyCode::End));
        app.set_terminal_height(40);
        assert_eq!(scroll_of(&app), 50 - 34);
    }

    #[test]
    fn the_setup_screen_hands_its_height_to_the_new_session() {
        let mut app = App::new(
            TuiConfig::default(),
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        app.set_terminal_height(40);
        app.handle_key(press(KeyCode::Enter));
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        assert_eq!(session.viewport(), 34);
    }
}
