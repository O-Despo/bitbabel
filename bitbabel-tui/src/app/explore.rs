//! Keys while exploring.

use ratatui::crossterm::event::KeyEvent;

use super::effect::Effect;
use super::{App, Mode};
use crate::keymap::{self, Action};

/// Handles a key press in [`Mode::Explore`].
pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Vec<Effect> {
    let Some(action) = keymap::action_for(&key) else {
        return vec![];
    };
    let Mode::Explore(session) = &mut app.mode else {
        return vec![];
    };
    match action {
        Action::Quit => return vec![Effect::Quit],
        Action::NextPage => session.step(true),
        Action::PrevPage => session.step(false),
        Action::StartOver => app.start_over(),
        Action::ScrollDown => session.scroll_by(1),
        Action::ScrollUp => session.scroll_by(-1),
        Action::HalfPageDown => session.scroll_half_page(true),
        Action::HalfPageUp => session.scroll_half_page(false),
        Action::Top => session.scroll_to_top(),
        Action::Bottom => session.scroll_to_bottom(),
    }
    vec![]
}
