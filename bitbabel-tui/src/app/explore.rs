//! Keys while exploring.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::App;
use super::effect::Effect;

/// Handles a key press in [`Mode::Explore`](super::Mode::Explore).
pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Vec<Effect> {
    if !key.modifiers.is_empty() {
        return vec![];
    }
    match key.code {
        KeyCode::Char('q') => vec![Effect::Quit],
        KeyCode::Char('s') => {
            app.start_over();
            vec![]
        }
        _ => vec![],
    }
}
