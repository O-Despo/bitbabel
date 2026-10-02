//! Keys while exploring.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::effect::Effect;

/// Handles a key press in [`Mode::Explore`](super::Mode::Explore).
pub(super) fn handle_key(key: KeyEvent) -> Vec<Effect> {
    match key.code {
        KeyCode::Char('q') if key.modifiers.is_empty() => vec![Effect::Quit],
        _ => vec![],
    }
}
