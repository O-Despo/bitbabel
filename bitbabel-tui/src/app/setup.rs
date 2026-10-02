//! The start screen: choose a size and go. The key is the canonical one for now.

use bitbabel_core::{Key, LibraryConfig};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::effect::Effect;
use super::library::LibraryChoice;

/// The sizes the screen offers, in order.
pub const SIZES: [LibraryConfig; 3] = [
    LibraryConfig::SMALL,
    LibraryConfig::MEDIUM,
    LibraryConfig::LARGE,
];

/// The rows that can have focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Size,
    Start,
}

/// What is chosen on the setup screen so far.
#[derive(Debug, Clone)]
pub struct SetupForm {
    size: usize,
    focus: Row,
    /// A key that came from the command line, kept for the library this form builds.
    key: Option<Key>,
}

/// What a key did to the form.
pub(super) enum Outcome {
    /// Still choosing.
    Editing(Vec<Effect>),
    /// The user chose to start exploring this library.
    Start(LibraryChoice),
}

impl SetupForm {
    /// A form on the medium size, which is the default size everywhere.
    pub fn new(key: Option<Key>) -> Self {
        let size = SIZES
            .iter()
            .position(|size| *size == LibraryConfig::default())
            .unwrap_or(0);
        SetupForm {
            size,
            focus: Row::Size,
            key,
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

    /// Whether a key came from the command line.
    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    fn step_size(&mut self, forward: bool) {
        let count = SIZES.len();
        self.size = if forward {
            (self.size + 1) % count
        } else {
            (self.size + count - 1) % count
        };
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Outcome::Editing(vec![Effect::Quit]),
            KeyCode::Enter => {
                return Outcome::Start(LibraryChoice::new(self.size(), self.key.clone()));
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.focus = Row::Start,
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => self.focus = Row::Size,
            KeyCode::Left | KeyCode::Char('h') if self.focus == Row::Size => self.step_size(false),
            KeyCode::Right | KeyCode::Char('l') if self.focus == Row::Size => self.step_size(true),
            _ => {}
        }
        Outcome::Editing(vec![])
    }
}

#[cfg(test)]
mod test {
    use ratatui::crossterm::event::KeyModifiers;

    use super::*;

    fn press(form: &mut SetupForm, code: KeyCode) -> Outcome {
        form.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
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
        assert_eq!(form.focus(), Row::Start);
        press(&mut form, KeyCode::Right);
        assert_eq!(form.size(), LibraryConfig::MEDIUM);
        press(&mut form, KeyCode::Up);
        assert_eq!(form.focus(), Row::Size);
    }

    #[test]
    fn enter_starts_with_the_chosen_size_and_the_given_key() {
        let mut form = SetupForm::new(Some(Key::from_bytes([1; 32])));
        press(&mut form, KeyCode::Left);
        let Outcome::Start(library) = press(&mut form, KeyCode::Enter) else {
            panic!("enter should start");
        };
        assert_eq!(library.size(), LibraryConfig::SMALL);
        assert!(library.key().is_some());
    }

    #[test]
    fn q_and_esc_quit() {
        for code in [KeyCode::Char('q'), KeyCode::Esc] {
            let Outcome::Editing(effects) = press(&mut SetupForm::new(None), code) else {
                panic!("should still be editing");
            };
            assert_eq!(effects, [Effect::Quit]);
        }
    }
}
