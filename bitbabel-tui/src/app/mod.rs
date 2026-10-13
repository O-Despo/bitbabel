//! All state and key handling. No drawing and no terminal: tests build an [`App`], feed it
//! keys and check the state.

mod effect;
mod explore;
mod library;
mod needle;
mod prompt;
mod search;
mod session;
mod setup;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub use effect::Effect;
pub use library::{LibraryChoice, fingerprint_text};
pub use prompt::Prompt;
pub use search::SearchForm;
pub use session::Session;
pub use setup::{KeySource, KeyStatus, Row, SIZES, SetupForm, loaded_text};

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
    files: Box<dyn Files>,
}

impl App {
    /// A new app. With a size it starts exploring that library (canonical unless there is
    /// a key); without one it opens the setup screen. `random` is where every random byte
    /// comes from, and `files` is how it reads files.
    pub fn new(config: TuiConfig, random: RandomFn, files: Box<dyn Files>) -> Self {
        let viewport = viewport_for(DEFAULT_HEIGHT);
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
        App {
            mode,
            viewport,
            random,
            files,
        }
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
        assert!(matches!(app.mode(), Mode::Explore(_)));
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
        assert!(matches!(app.mode(), Mode::Explore(_)));
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

    #[test]
    fn a_library_that_cannot_open_shows_setup_with_the_reason() {
        // An odd page length is not a valid library.
        let config = TuiConfig {
            size: Some(LibraryConfig::new(15, 8)),
            key: None,
        };
        let app = App::new(config, fake_random, Box::new(MemFiles::default()));
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
        };
        let mut app = App::new(config, fake_random, Box::new(MemFiles::default()));
        app.handle_key(press(KeyCode::Down));
        let Mode::Setup(form) = app.mode() else {
            panic!("should still be on the setup screen");
        };
        assert_eq!(form.error(), None);
    }

    fn medium_app() -> App {
        let config = TuiConfig {
            size: Some(LibraryConfig::MEDIUM),
            key: None,
        };
        App::new(config, fake_random, Box::new(MemFiles::default()))
    }

    fn scroll_of(app: &App) -> usize {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session.scroll()
    }

    #[test]
    fn j_and_k_scroll_one_line_and_stop_at_the_top() {
        let mut app = medium_app();
        app.handle_key(press(KeyCode::Char('k')));
        assert_eq!(scroll_of(&app), 0);
        app.handle_key(press(KeyCode::Char('j')));
        app.handle_key(press(KeyCode::Char('j')));
        assert_eq!(scroll_of(&app), 2);
        app.handle_key(press(KeyCode::Char('k')));
        assert_eq!(scroll_of(&app), 1);
    }

    #[test]
    fn home_end_and_ctrl_d_u_move_the_scroll() {
        let mut app = medium_app();
        app.handle_key(press(KeyCode::End));
        assert_eq!(scroll_of(&app), 50 - 18);
        app.handle_key(press(KeyCode::Home));
        assert_eq!(scroll_of(&app), 0);
        let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        app.handle_key(ctrl('d'));
        assert_eq!(scroll_of(&app), 9);
        app.handle_key(ctrl('u'));
        assert_eq!(scroll_of(&app), 0);
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
        );
        app.set_terminal_height(40);
        app.handle_key(press(KeyCode::Enter));
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        assert_eq!(session.viewport(), 34);
    }

    fn index_of(app: &App) -> bitbabel_core::PageIndex {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session.index().clone()
    }

    #[test]
    fn left_and_right_walk_back_and_forward_through_the_visits() {
        let mut app = medium_app();
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('l')));
        app.handle_key(press(KeyCode::Char('l')));
        let third = index_of(&app);
        assert_ne!(third, start);

        app.handle_key(press(KeyCode::Left));
        app.handle_key(press(KeyCode::Left));
        assert_eq!(index_of(&app), start);
        app.handle_key(press(KeyCode::Right));
        app.handle_key(press(KeyCode::Right));
        assert_eq!(index_of(&app), third);
    }

    fn status_of(app: &App) -> Option<super::session::Status> {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session.status().cloned()
    }

    #[test]
    fn r_visits_the_index_made_of_the_random_bytes() {
        let mut app = medium_app();
        app.handle_key(press(KeyCode::Char('r')));
        // The fake source fills every byte with 0xAB.
        assert_eq!(
            index_of(&app),
            bitbabel_core::PageIndex::from_bytes(vec![0xAB; 3200])
        );
    }

    #[test]
    fn a_random_page_is_a_visit_so_back_returns() {
        let mut app = medium_app();
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('r')));
        app.handle_key(press(KeyCode::Left));
        assert_eq!(index_of(&app), start);
    }

    #[test]
    fn a_failing_random_source_leaves_the_page_and_says_why() {
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: None,
        };
        let mut app = App::new(config, failing_random, Box::new(MemFiles::default()));
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('r')));
        assert_eq!(index_of(&app), start);
        let status = status_of(&app).unwrap();
        assert!(status.error);
        assert_eq!(status.text, "cannot get random bytes from the OS: blocked");
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle_key(press(KeyCode::Char(c)));
        }
    }

    fn prompt_of(app: &App) -> Option<&Prompt> {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session.prompt()
    }

    #[test]
    fn x_opens_the_export_prompt_and_esc_closes_it() {
        let mut app = medium_app();
        assert!(prompt_of(&app).is_none());
        app.handle_key(press(KeyCode::Char('x')));
        assert_eq!(prompt_of(&app).unwrap().title(), "export index");
        app.handle_key(press(KeyCode::Esc));
        assert!(prompt_of(&app).is_none());
    }

    #[test]
    fn typing_into_a_prompt_never_runs_a_command() {
        let mut app = medium_app();
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('x')));
        // `l` would step a page and `q` would quit if the prompt did not take them.
        type_text(&mut app, "lq.txt");
        assert_eq!(prompt_of(&app).unwrap().input(), "lq.txt");
        assert_eq!(index_of(&app), start);
        app.handle_key(press(KeyCode::Backspace));
        assert_eq!(prompt_of(&app).unwrap().input(), "lq.tx");
    }

    #[test]
    fn ctrl_c_still_quits_with_a_prompt_open() {
        let mut app = medium_app();
        app.handle_key(press(KeyCode::Char('x')));
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(ctrl_c), [Effect::Quit]);
    }

    /// An app on `files`, small size, with the export prompt open and `name` typed.
    fn export_to(files: MemFiles, name: &str) -> App {
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: None,
        };
        let mut app = App::new(config, fake_random, Box::new(files));
        app.handle_key(press(KeyCode::Char('x')));
        type_text(&mut app, name);
        app
    }

    #[test]
    fn export_writes_the_index_as_base64_and_a_newline() {
        let files = std::sync::Arc::new(MemFiles::default());
        // Share the fake with the app so the test can look inside it afterwards.
        struct Shared(std::sync::Arc<MemFiles>);
        impl Files for Shared {
            fn read(&self, path: &std::path::Path, max: usize) -> std::io::Result<Vec<u8>> {
                self.0.read(path, max)
            }
            fn write_new(&self, path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
                self.0.write_new(path, bytes)
            }
        }
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: None,
        };
        let mut app = App::new(config, fake_random, Box::new(Shared(files.clone())));
        app.handle_key(press(KeyCode::Char('l')));
        app.handle_key(press(KeyCode::Char('x')));
        type_text(&mut app, "idx.txt");
        app.handle_key(press(KeyCode::Enter));

        let expected = format!("{}\n", index_of(&app).to_base64());
        assert_eq!(files.get("idx.txt").unwrap(), expected.as_bytes());
        assert!(prompt_of(&app).is_none());
        assert_eq!(status_of(&app).unwrap().text, "index written to idx.txt");
    }

    #[test]
    fn export_refuses_an_existing_file_and_keeps_the_prompt_open() {
        let files = MemFiles::default().with("taken.txt", b"keep me");
        let mut app = export_to(files, "taken.txt");
        app.handle_key(press(KeyCode::Enter));
        let prompt = prompt_of(&app).unwrap();
        assert_eq!(
            prompt.error(),
            Some("taken.txt already exists, choose another name")
        );
        // The name is still there to edit, and typing clears the error.
        assert_eq!(prompt.input(), "taken.txt");
        app.handle_key(press(KeyCode::Char('2')));
        assert_eq!(prompt_of(&app).unwrap().error(), None);
    }

    #[test]
    fn export_with_no_name_says_so() {
        let mut app = export_to(MemFiles::default(), "  ");
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(prompt_of(&app).unwrap().error(), Some("type a file name"));
    }

    /// A fake file system the test can keep a handle on after giving it to the app.
    struct SharedFiles(std::sync::Arc<MemFiles>);

    impl Files for SharedFiles {
        fn read(&self, path: &std::path::Path, max: usize) -> std::io::Result<Vec<u8>> {
            self.0.read(path, max)
        }

        fn write_new(&self, path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
            self.0.write_new(path, bytes)
        }
    }

    #[test]
    fn g_visits_the_index_that_x_exported() {
        let files = std::sync::Arc::new(MemFiles::default());
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::MEDIUM),
                key: None,
            },
            fake_random,
            Box::new(SharedFiles(files)),
        );
        app.handle_key(press(KeyCode::Char('l')));
        let saved = index_of(&app);

        app.handle_key(press(KeyCode::Char('x')));
        type_text(&mut app, "here.txt");
        app.handle_key(press(KeyCode::Enter));

        app.handle_key(press(KeyCode::Char('r')));
        assert_ne!(index_of(&app), saved);

        app.handle_key(press(KeyCode::Char('g')));
        assert_eq!(prompt_of(&app).unwrap().title(), "go to");
        type_text(&mut app, "here.txt");
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(index_of(&app), saved);
        assert!(prompt_of(&app).is_none());
    }

    #[test]
    fn g_with_a_bad_file_keeps_the_prompt_open_and_the_page() {
        let files = MemFiles::default().with("short.txt", b"abc");
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
            },
            fake_random,
            Box::new(files),
        );
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('g')));
        type_text(&mut app, "short.txt");
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(index_of(&app), start);
        assert_eq!(
            prompt_of(&app).unwrap().error(),
            Some("3 chars: not 32 (hex) or 24 (base64, with its = padding)")
        );
        app.handle_key(press(KeyCode::Esc));
        assert!(prompt_of(&app).is_none());
    }

    #[test]
    fn a_went_to_page_is_a_visit_so_back_returns() {
        let mut want = bitbabel_core::PageIndex::zero(16).to_string();
        want.replace_range(31.., "7");
        let files = MemFiles::default().with("seven.txt", want.as_bytes());
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
            },
            fake_random,
            Box::new(files),
        );
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('g')));
        type_text(&mut app, "seven.txt");
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(index_of(&app), bitbabel_core::PageIndex::from_u64(7, 16));
        app.handle_key(press(KeyCode::Left));
        assert_eq!(index_of(&app), start);
    }

    fn form_of(app: &App) -> SearchForm {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session
            .search_form()
            .expect("the search form should exist")
            .clone()
    }

    fn search_open(app: &App) -> bool {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session.search_is_open()
    }

    /// A small-library app with the search overlay open and `needle` typed.
    fn searching(needle: &str) -> App {
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: None,
        };
        let mut app = App::new(config, fake_random, Box::new(MemFiles::default()));
        app.handle_key(press(KeyCode::Char('/')));
        type_text(&mut app, needle);
        app
    }

    #[test]
    fn slash_opens_the_search_overlay_and_esc_closes_it() {
        let mut app = medium_app();
        assert!(!search_open(&app));
        app.handle_key(press(KeyCode::Char('/')));
        assert!(search_open(&app));
        app.handle_key(press(KeyCode::Esc));
        assert!(!search_open(&app));
    }

    #[test]
    fn typing_in_the_search_box_never_runs_a_command() {
        let mut app = searching("");
        let start = index_of(&app);
        // `l`, `r` and `q` are commands outside the box.
        type_text(&mut app, "lrq");
        assert_eq!(form_of(&app).input(), "lrq");
        assert_eq!(index_of(&app), start);
        app.handle_key(press(KeyCode::Backspace));
        assert_eq!(form_of(&app).input(), "lr");
    }

    #[test]
    fn enter_lists_ten_pages_that_each_hold_the_needle_where_it_says() {
        let mut app = searching("hello");
        app.handle_key(press(KeyCode::Enter));
        let form = form_of(&app);
        assert_eq!(form.error(), None);
        assert_eq!(form.hits().len(), 10);
        assert!(form.is_fresh());

        let library = bitbabel_core::BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
        for hit in form.hits() {
            let page = library.page_at(hit.index()).unwrap();
            let at = hit.start();
            assert_eq!(&page.bytes()[at..at + 5], b"hello");
        }
    }

    #[test]
    fn the_same_seed_and_needle_give_the_same_results() {
        let mut first = searching("hi");
        first.handle_key(press(KeyCode::Enter));
        let mut second = searching("hi");
        second.handle_key(press(KeyCode::Enter));
        assert_eq!(form_of(&first).hits(), form_of(&second).hits());
    }

    #[test]
    fn a_bad_needle_is_an_inline_error_and_keeps_typing_possible() {
        let mut app = searching("a中");
        app.handle_key(press(KeyCode::Enter));
        let form = form_of(&app);
        assert!(
            form.error()
                .unwrap()
                .contains("not in the BitBabel text table")
        );
        assert!(form.hits().is_empty());
        app.handle_key(press(KeyCode::Backspace));
        assert_eq!(form_of(&app).error(), None);
    }

    #[test]
    fn a_needle_longer_than_a_page_names_the_limit() {
        let mut app = searching(&"a".repeat(17));
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(
            form_of(&app).error(),
            Some("needle is 17 bytes but a page is only 16")
        );
    }

    #[test]
    fn an_empty_needle_asks_for_something_to_search_for() {
        let mut app = searching("");
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(form_of(&app).error(), Some("type something to search for"));
    }

    #[test]
    fn a_failing_random_source_is_an_error_and_no_search_happens() {
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: None,
        };
        let mut app = App::new(config, failing_random, Box::new(MemFiles::default()));
        app.handle_key(press(KeyCode::Char('/')));
        type_text(&mut app, "hi");
        app.handle_key(press(KeyCode::Enter));
        let form = form_of(&app);
        assert!(form.hits().is_empty());
        assert_eq!(
            form.error(),
            Some("cannot get random bytes from the OS: blocked")
        );
    }

    #[test]
    fn editing_the_needle_makes_the_results_stale_and_arrows_pick_a_row() {
        let mut app = searching("hi");
        app.handle_key(press(KeyCode::Enter));
        app.handle_key(press(KeyCode::Down));
        app.handle_key(press(KeyCode::Down));
        assert_eq!(form_of(&app).selected(), 2);
        app.handle_key(press(KeyCode::Up));
        assert_eq!(form_of(&app).selected(), 1);

        type_text(&mut app, "x");
        assert!(!form_of(&app).is_fresh());
        // The old results are still there to look at.
        assert_eq!(form_of(&app).hits().len(), 10);
    }

    #[test]
    fn the_overlay_keeps_its_needle_and_results_when_closed_and_reopened() {
        let mut app = searching("hi");
        app.handle_key(press(KeyCode::Enter));
        app.handle_key(press(KeyCode::Esc));
        app.handle_key(press(KeyCode::Char('/')));
        assert_eq!(form_of(&app).input(), "hi");
        assert_eq!(form_of(&app).hits().len(), 10);
    }

    #[test]
    fn the_needle_is_read_in_the_current_view() {
        let mut app = searching("");
        app.handle_key(press(KeyCode::Esc));
        app.handle_key(press(KeyCode::Char('e'))); // hex
        app.handle_key(press(KeyCode::Char('/')));
        type_text(&mut app, "6869");
        app.handle_key(press(KeyCode::Enter));
        let form = form_of(&app);
        assert_eq!(form.error(), None);
        let library = bitbabel_core::BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
        let page = library.page_at(form.hits()[0].index()).unwrap();
        let at = form.hits()[0].start();
        assert_eq!(&page.bytes()[at..at + 2], b"hi");
    }

    fn highlight_of(app: &App) -> Option<std::ops::Range<usize>> {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session.highlight().cloned()
    }

    #[test]
    fn enter_on_fresh_results_opens_the_picked_one_with_the_needle_marked() {
        let mut app = searching("hello");
        app.handle_key(press(KeyCode::Enter)); // search
        app.handle_key(press(KeyCode::Down));
        app.handle_key(press(KeyCode::Down));
        let hit = form_of(&app).hits()[2].clone();

        app.handle_key(press(KeyCode::Enter)); // open row 3
        assert_eq!(index_of(&app), *hit.index());
        assert_eq!(highlight_of(&app), Some(hit.range()));
        assert!(!search_open(&app));
    }

    #[test]
    fn the_marked_text_is_the_needle() {
        let mut app = searching("hello");
        app.handle_key(press(KeyCode::Enter));
        app.handle_key(press(KeyCode::Enter));
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        let marked: String = session
            .lines()
            .iter()
            .flat_map(|line| &line.segments)
            .filter(|segment| segment.highlighted)
            .map(|segment| segment.text.as_str())
            .collect();
        assert_eq!(marked, "hello");
    }

    #[test]
    fn an_opened_result_is_a_visit_so_back_returns() {
        let mut app = searching("hi");
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Enter));
        app.handle_key(press(KeyCode::Enter));
        assert_ne!(index_of(&app), start);
        app.handle_key(press(KeyCode::Left));
        assert_eq!(index_of(&app), start);
        assert_eq!(highlight_of(&app), None);
    }

    #[test]
    fn the_mark_stays_through_a_view_change_and_scrolling_but_not_a_page_change() {
        let mut app = searching("hi");
        app.handle_key(press(KeyCode::Enter));
        app.handle_key(press(KeyCode::Enter));
        let marked = highlight_of(&app);
        assert!(marked.is_some());

        app.handle_key(press(KeyCode::Char('e')));
        app.handle_key(press(KeyCode::Char('j')));
        assert_eq!(highlight_of(&app), marked);

        app.handle_key(press(KeyCode::Char('l')));
        assert_eq!(highlight_of(&app), None);
    }

    #[test]
    fn opening_a_result_scrolls_to_the_match() {
        let config = TuiConfig {
            size: Some(LibraryConfig::MEDIUM),
            key: None,
        };
        let mut app = App::new(config, fake_random, Box::new(MemFiles::default()));
        app.handle_key(press(KeyCode::Char('/')));
        type_text(&mut app, "needle");
        app.handle_key(press(KeyCode::Enter));
        app.handle_key(press(KeyCode::Esc));
        // Try every result: the marked line must be on screen for each.
        for pick in 0..10 {
            app.handle_key(press(KeyCode::Char('/')));
            // The picked row is kept between openings, so go to the top first.
            for _ in 0..10 {
                app.handle_key(press(KeyCode::Up));
            }
            for _ in 0..pick {
                app.handle_key(press(KeyCode::Down));
            }
            app.handle_key(press(KeyCode::Enter));
            let Mode::Explore(session) = app.mode() else {
                panic!("should be exploring");
            };
            let visible = session
                .lines()
                .iter()
                .skip(session.scroll())
                .take(session.viewport());
            let any_marked = visible
                .flat_map(|line| &line.segments)
                .any(|segment| segment.highlighted);
            assert!(any_marked, "result {pick} is not on screen");
        }
    }

    #[test]
    fn enter_on_stale_results_searches_again_instead_of_opening() {
        let mut app = searching("hi");
        app.handle_key(press(KeyCode::Enter));
        type_text(&mut app, "s"); // "his" is a new needle
        app.handle_key(press(KeyCode::Enter));
        assert!(search_open(&app));
        assert!(form_of(&app).is_fresh());
        assert_eq!(form_of(&app).hits().len(), 10);
    }
}
