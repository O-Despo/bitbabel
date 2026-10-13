//! Keys while exploring.

use bitbabel_core::PageIndex;
use ratatui::crossterm::event::KeyEvent;

use super::effect::Effect;
use super::prompt::{self, PromptKind};
use super::search;
use super::{App, Mode};
use crate::keymap::{self, Action};

/// Handles a key press in [`Mode::Explore`].
pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Vec<Effect> {
    // An open prompt gets every key, so typing into it never runs a command.
    if let Mode::Explore(session) = &app.mode
        && session.prompt().is_some()
    {
        return prompt::handle_key(app, key);
    }
    if let Mode::Explore(session) = &app.mode
        && session.search_is_open()
    {
        return search::handle_key(app, key);
    }
    let Some(action) = keymap::action_for(&key) else {
        return vec![];
    };
    if action == Action::Random {
        random_page(app);
        return vec![];
    }
    let Mode::Explore(session) = &mut app.mode else {
        return vec![];
    };
    match action {
        Action::Quit => return vec![Effect::Quit],
        Action::NextPage => session.step(true),
        Action::PrevPage => session.step(false),
        Action::HistoryBack => session.back(),
        Action::HistoryForward => session.forward(),
        Action::GoTo => session.open_prompt(PromptKind::GoTo),
        Action::Export => session.open_prompt(PromptKind::Export),
        Action::CycleView => session.cycle_view(),
        Action::Search => session.open_search(),
        Action::Random => {}
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

/// Visits a page chosen at random. Every byte string of the right length is a valid index, so
/// the random bytes are the index: no hashing and no retry. If the random source fails, the
/// page stays and the status line says why.
fn random_page(app: &mut App) {
    let Mode::Explore(session) = &app.mode else {
        return;
    };
    let len = session.index().as_bytes().len();
    let bytes = app.random_bytes(len);
    let Mode::Explore(session) = &mut app.mode else {
        return;
    };
    match bytes {
        Ok(bytes) => {
            if let Err(error) = session.visit(PageIndex::from_bytes(bytes)) {
                session.set_error(error.to_string());
            }
        }
        Err(error) => session.set_error(error.to_string()),
    }
}
