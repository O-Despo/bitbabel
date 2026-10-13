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
        Action::NextResult => search::step_result(session, true),
        Action::PrevResult => search::step_result(session, false),
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

#[cfg(test)]
mod test {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::app::test_util::*;
    use crate::config::TuiConfig;
    use crate::files::MemFiles;
    use bitbabel_core::LibraryConfig;

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
}
