//! The start screen: choose a size and a key, then go.

use std::io;
use std::path::Path;

use bitbabel_core::{Key, LibraryConfig};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::effect::Effect;
use super::library::{LibraryChoice, fingerprint_text};
use super::typed_char;
use crate::files::Files;

/// The sizes the screen offers, in order.
pub const SIZES: [LibraryConfig; 3] = [
    LibraryConfig::SMALL,
    LibraryConfig::MEDIUM,
    LibraryConfig::LARGE,
];

/// A key file is exactly this long.
const KEY_FILE_LEN: usize = 32;

/// The rows that can have focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Size,
    Key,
    /// The key file's path. Only there when the key row says `KeySource::File`.
    Path,
    Start,
}

/// Where the key comes from. A key given on the command line is not a choice here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    /// The public key everyone shares.
    Canonical,
    /// A private key read from a file.
    File,
}

/// What reading the key file found.
#[derive(Debug, Clone)]
pub enum KeyStatus {
    /// The path was typed or changed and has not been read yet.
    Unchecked,
    /// The file held a key.
    Loaded(Key),
    /// The file could not be used. The text says why.
    Failed(String),
}

/// What is chosen on the setup screen so far.
#[derive(Debug, Clone)]
pub struct SetupForm {
    size: usize,
    focus: Row,
    /// A key from the command line. It replaces the key row.
    fixed_key: Option<Key>,
    source: KeySource,
    path: String,
    status: KeyStatus,
}

/// What a key did to the form.
pub(super) enum Outcome {
    /// Still choosing.
    Editing(Vec<Effect>),
    /// The user chose to start exploring this library.
    Start(LibraryChoice),
}

impl SetupForm {
    /// A form on the medium size, which is the default size everywhere. `key` is a key that
    /// came from the command line, if any.
    pub fn new(key: Option<Key>) -> Self {
        let size = SIZES
            .iter()
            .position(|size| *size == LibraryConfig::default())
            .unwrap_or(0);
        SetupForm {
            size,
            focus: Row::Size,
            fixed_key: key,
            source: KeySource::Canonical,
            path: String::new(),
            status: KeyStatus::Unchecked,
        }
    }

    /// The chosen size.
    pub fn size(&self) -> LibraryConfig {
        SIZES[self.size]
    }

    /// The row that has focus.
    pub fn focus(&self) -> Row {
        self.focus
    }

    /// The key from the command line, if there was one.
    pub fn fixed_key(&self) -> Option<&Key> {
        self.fixed_key.as_ref()
    }

    /// Where the key comes from.
    pub fn source(&self) -> KeySource {
        self.source
    }

    /// The typed key file path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// What reading the key file found.
    pub fn status(&self) -> &KeyStatus {
        &self.status
    }

    /// The rows on screen, top to bottom.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = vec![Row::Size, Row::Key];
        if self.fixed_key.is_none() && self.source == KeySource::File {
            rows.push(Row::Path);
        }
        rows.push(Row::Start);
        rows
    }

    fn step_size(&mut self, forward: bool) {
        let count = SIZES.len();
        self.size = if forward {
            (self.size + 1) % count
        } else {
            (self.size + count - 1) % count
        };
    }

    fn move_focus(&mut self, down: bool) {
        let rows = self.rows();
        let at = rows.iter().position(|row| *row == self.focus).unwrap_or(0);
        let next = if down {
            (at + 1).min(rows.len() - 1)
        } else {
            at.saturating_sub(1)
        };
        self.focus = rows[next];
    }

    fn toggle_source(&mut self) {
        if self.fixed_key.is_some() {
            return;
        }
        self.source = match self.source {
            KeySource::Canonical => KeySource::File,
            KeySource::File => KeySource::Canonical,
        };
    }

    /// Reads the typed path and records what it found. Never fails: a bad path is a status.
    fn check_key_file(&mut self, files: &dyn Files) {
        self.status = if self.path.is_empty() {
            KeyStatus::Failed("type the path of a key file".to_string())
        } else {
            match files.read(Path::new(&self.path), KEY_FILE_LEN) {
                Ok(bytes) => match Key::from_slice(&bytes) {
                    Ok(key) => KeyStatus::Loaded(key),
                    Err(_) => KeyStatus::Failed(wrong_length()),
                },
                Err(error) if error.kind() == io::ErrorKind::FileTooLarge => {
                    KeyStatus::Failed(wrong_length())
                }
                Err(error) => KeyStatus::Failed(format!("cannot read {}: {error}", self.path)),
            }
        };
    }

    /// The library to start, or `None` after moving focus to what is wrong.
    fn library(&mut self, files: &dyn Files) -> Option<LibraryChoice> {
        let key = match (&self.fixed_key, self.source) {
            (Some(key), _) => Some(key.clone()),
            (None, KeySource::Canonical) => None,
            (None, KeySource::File) => {
                if !matches!(self.status, KeyStatus::Loaded(_)) {
                    self.check_key_file(files);
                }
                match &self.status {
                    KeyStatus::Loaded(key) => Some(key.clone()),
                    _ => {
                        self.focus = Row::Path;
                        return None;
                    }
                }
            }
        };
        Some(LibraryChoice::new(self.size(), key))
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent, files: &dyn Files) -> Outcome {
        if self.focus == Row::Path {
            return self.handle_path_key(key, files);
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Outcome::Editing(vec![Effect::Quit]),
            KeyCode::Enter => {
                return match self.library(files) {
                    Some(library) => Outcome::Start(library),
                    None => Outcome::Editing(vec![]),
                };
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.move_focus(true),
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => self.move_focus(false),
            KeyCode::Left | KeyCode::Char('h') => match self.focus {
                Row::Size => self.step_size(false),
                Row::Key => self.toggle_source(),
                _ => {}
            },
            KeyCode::Right | KeyCode::Char('l') => match self.focus {
                Row::Size => self.step_size(true),
                Row::Key => self.toggle_source(),
                _ => {}
            },
            _ => {}
        }
        Outcome::Editing(vec![])
    }

    /// Keys in the path box: everything typed goes into the path, even `q`.
    fn handle_path_key(&mut self, key: KeyEvent, files: &dyn Files) -> Outcome {
        match key.code {
            KeyCode::Esc => self.focus = Row::Key,
            KeyCode::Enter => {
                self.check_key_file(files);
                if matches!(self.status, KeyStatus::Loaded(_)) {
                    self.focus = Row::Start;
                }
            }
            KeyCode::Down | KeyCode::Tab => self.move_focus(true),
            KeyCode::Up | KeyCode::BackTab => self.move_focus(false),
            KeyCode::Backspace => {
                self.path.pop();
                self.status = KeyStatus::Unchecked;
            }
            _ => {
                if let Some(c) = typed_char(&key) {
                    self.path.push(c);
                    self.status = KeyStatus::Unchecked;
                }
            }
        }
        Outcome::Editing(vec![])
    }
}

fn wrong_length() -> String {
    format!("file must be exactly {KEY_FILE_LEN} bytes")
}

/// The text after `✓` on the key file row: `loaded 3f9a-1c07-e2b8-4d10`.
pub fn loaded_text(key: &Key) -> String {
    format!("loaded {}", fingerprint_text(key))
}

#[cfg(test)]
mod test {
    use ratatui::crossterm::event::KeyModifiers;

    use super::*;
    use crate::files::MemFiles;

    fn files() -> MemFiles {
        MemFiles::default()
            .with("good.key", &[1; 32])
            .with("short.key", &[1; 31])
            .with("long.key", &[1; 33])
    }

    fn press(form: &mut SetupForm, code: KeyCode) -> Outcome {
        form.handle_key(KeyEvent::new(code, KeyModifiers::NONE), &files())
    }

    fn type_text(form: &mut SetupForm, text: &str) {
        for c in text.chars() {
            press(form, KeyCode::Char(c));
        }
    }

    /// A form with the key file row chosen and focus in the path box.
    fn path_form() -> SetupForm {
        let mut form = SetupForm::new(None);
        press(&mut form, KeyCode::Down);
        press(&mut form, KeyCode::Right);
        press(&mut form, KeyCode::Down);
        assert_eq!(form.focus(), Row::Path);
        form
    }

    #[test]
    fn starts_on_medium() {
        assert_eq!(SetupForm::new(None).size(), LibraryConfig::MEDIUM);
    }

    #[test]
    fn left_and_right_change_the_size_and_wrap() {
        let mut form = SetupForm::new(None);
        press(&mut form, KeyCode::Right);
        assert_eq!(form.size(), LibraryConfig::LARGE);
        press(&mut form, KeyCode::Right);
        assert_eq!(form.size(), LibraryConfig::SMALL);
        press(&mut form, KeyCode::Left);
        assert_eq!(form.size(), LibraryConfig::LARGE);
    }

    #[test]
    fn size_keys_only_work_on_the_size_row() {
        let mut form = SetupForm::new(None);
        press(&mut form, KeyCode::Down);
        press(&mut form, KeyCode::Down);
        assert_eq!(form.focus(), Row::Start);
        press(&mut form, KeyCode::Right);
        assert_eq!(form.size(), LibraryConfig::MEDIUM);
        press(&mut form, KeyCode::Up);
        press(&mut form, KeyCode::Up);
        assert_eq!(form.focus(), Row::Size);
    }

    #[test]
    fn the_key_row_toggles_and_adds_the_path_row() {
        let mut form = SetupForm::new(None);
        assert_eq!(form.rows(), [Row::Size, Row::Key, Row::Start]);
        press(&mut form, KeyCode::Down);
        press(&mut form, KeyCode::Right);
        assert_eq!(form.source(), KeySource::File);
        assert_eq!(form.rows(), [Row::Size, Row::Key, Row::Path, Row::Start]);
        press(&mut form, KeyCode::Left);
        assert_eq!(form.source(), KeySource::Canonical);
    }

    #[test]
    fn enter_starts_a_canonical_library() {
        let mut form = SetupForm::new(None);
        press(&mut form, KeyCode::Left);
        let Outcome::Start(library) = press(&mut form, KeyCode::Enter) else {
            panic!("enter should start");
        };
        assert_eq!(library.size(), LibraryConfig::SMALL);
        assert!(library.key().is_none());
    }

    #[test]
    fn a_command_line_key_replaces_the_key_row() {
        let mut form = SetupForm::new(Some(Key::from_bytes([1; 32])));
        press(&mut form, KeyCode::Down);
        press(&mut form, KeyCode::Right);
        assert_eq!(form.source(), KeySource::Canonical);
        assert_eq!(form.rows(), [Row::Size, Row::Key, Row::Start]);
        let Outcome::Start(library) = press(&mut form, KeyCode::Enter) else {
            panic!("enter should start");
        };
        assert!(library.key().is_some());
    }

    #[test]
    fn typing_fills_the_path_even_letters_that_are_commands() {
        let mut form = path_form();
        type_text(&mut form, "qhjk.key");
        assert_eq!(form.path(), "qhjk.key");
        press(&mut form, KeyCode::Backspace);
        assert_eq!(form.path(), "qhjk.ke");
    }

    #[test]
    fn enter_in_the_path_box_loads_a_good_key_and_moves_on() {
        let mut form = path_form();
        type_text(&mut form, "good.key");
        assert!(matches!(form.status(), KeyStatus::Unchecked));
        press(&mut form, KeyCode::Enter);
        let KeyStatus::Loaded(key) = form.status() else {
            panic!("should be loaded");
        };
        assert_eq!(key, &Key::from_bytes([1; 32]));
        assert_eq!(form.focus(), Row::Start);
    }

    #[test]
    fn a_bad_key_file_is_a_message_not_an_exit() {
        let cases = [
            ("short.key", "file must be exactly 32 bytes"),
            ("long.key", "file must be exactly 32 bytes"),
            (
                "missing.key",
                "cannot read missing.key: No such file or directory",
            ),
            ("", "type the path of a key file"),
        ];
        for (path, message) in cases {
            let mut form = path_form();
            type_text(&mut form, path);
            press(&mut form, KeyCode::Enter);
            let KeyStatus::Failed(text) = form.status() else {
                panic!("{path:?} should fail");
            };
            assert_eq!(text, message);
            assert_eq!(form.focus(), Row::Path);
        }
    }

    #[test]
    fn editing_the_path_forgets_the_check() {
        let mut form = path_form();
        type_text(&mut form, "good.key");
        press(&mut form, KeyCode::Enter);
        press(&mut form, KeyCode::Up);
        press(&mut form, KeyCode::Char('x'));
        assert!(matches!(form.status(), KeyStatus::Unchecked));
    }

    #[test]
    fn esc_leaves_the_path_box_instead_of_quitting() {
        let mut form = path_form();
        assert!(matches!(
            press(&mut form, KeyCode::Esc),
            Outcome::Editing(effects) if effects.is_empty()
        ));
        assert_eq!(form.focus(), Row::Key);
    }

    #[test]
    fn start_with_an_unchecked_path_checks_it_first() {
        let mut form = path_form();
        type_text(&mut form, "good.key");
        press(&mut form, KeyCode::Down); // to Start, without checking
        let Outcome::Start(library) = press(&mut form, KeyCode::Enter) else {
            panic!("a good key file should start");
        };
        assert_eq!(library.key(), Some(&Key::from_bytes([1; 32])));
    }

    #[test]
    fn start_with_a_bad_path_goes_back_to_the_path_box() {
        let mut form = path_form();
        type_text(&mut form, "short.key");
        press(&mut form, KeyCode::Down);
        assert!(matches!(
            press(&mut form, KeyCode::Enter),
            Outcome::Editing(_)
        ));
        assert_eq!(form.focus(), Row::Path);
        assert!(matches!(form.status(), KeyStatus::Failed(_)));
    }

    #[test]
    fn q_and_esc_quit_outside_the_path_box() {
        for code in [KeyCode::Char('q'), KeyCode::Esc] {
            let Outcome::Editing(effects) = press(&mut SetupForm::new(None), code) else {
                panic!("should still be editing");
            };
            assert_eq!(effects, [Effect::Quit]);
        }
    }
}
