//! The `B` list of bookmarks: which row is selected, which row is first shown, and the keys
//! the list itself takes.
//!
//! The list state lives on the session, not in the overlay, because a prompt can open over
//! the list (rename, §13.3b): when the prompt closes, the list is where the user left it.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::effect::Effect;
use super::{App, Mode, Session};
use crate::bookmarks::Bookmarks;
use crate::files::Files;

/// The selection and window of the `B` list.
#[derive(Debug, Default)]
pub struct BookmarkList {
    /// The selected row, 0-based.
    selected: usize,
    /// The first row shown.
    top: usize,
}

impl BookmarkList {
    /// The selected row.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The first row shown.
    pub fn top(&self) -> usize {
        self.top
    }

    /// Keeps the selection and the window inside a list of `total` rows of `rows` height.
    /// A selection past the end (a delete, a shorter file) falls back to the last row.
    pub(super) fn fit(&mut self, rows: usize, total: usize) {
        self.selected = self.selected.min(total.saturating_sub(1));
        self.fit_window(rows, total);
    }

    /// Moves the selection one row down or up, stopping at both ends, and keeps it in view.
    pub(super) fn move_selected(&mut self, down: bool, rows: usize, total: usize) {
        if total == 0 {
            return;
        }
        let last = total - 1;
        self.selected = if down {
            (self.selected + 1).min(last)
        } else {
            self.selected.saturating_sub(1)
        };
        self.fit_window(rows, total);
    }

    /// Scrolls the window only as far as it must to hold the selection.
    fn fit_window(&mut self, rows: usize, total: usize) {
        if rows == 0 {
            self.top = 0;
            return;
        }
        if self.selected < self.top {
            self.top = self.selected;
        }
        if self.selected >= self.top + rows {
            self.top = self.selected - rows + 1;
        }
        // Never show past the last row: the window slides back and still holds the selection,
        // because the selection is at most `total - 1`.
        self.top = self.top.min(total.saturating_sub(rows));
    }
}

/// Handles a key while the list is open. ↑/↓ and `j`/`k` move the selection, `enter` opens
/// the selected page and closes the list, `r` renames, `d` deletes at once, `esc` and `q`
/// close. Any other key does nothing: `r` and `d` are the list's rename and delete, not the
/// explore keys (§13.3b).
pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Vec<Effect> {
    // Ctrl and Alt combos neither type nor command here, as in the other screens.
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return vec![];
    }
    let Mode::Explore(session) = &mut app.mode else {
        return vec![];
    };
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => session.move_bookmark_selection(false),
        KeyCode::Down | KeyCode::Char('j') => session.move_bookmark_selection(true),
        KeyCode::Esc | KeyCode::Char('q') => session.close_bookmark_list(),
        KeyCode::Enter => visit_selected(session),
        KeyCode::Char('r') => rename_selected(session),
        KeyCode::Char('d') => delete_selected(session, app.files.as_ref()),
        _ => {}
    }
    vec![]
}

/// Deletes the selected bookmark and re-clamps the list: the next row moves up into the
/// selection, the last row steps back one, the only bookmark leaves `no bookmarks`. No
/// confirming question (§13.3) — the page is still reachable by its index. A failed write
/// closes the list, because its box covers the status line and the reason must be readable.
fn delete_selected(session: &mut Session, files: &dyn Files) {
    if session.bookmark_count() == 0 {
        return;
    }
    match session.delete_selected_bookmark(files) {
        Ok(()) => session.open_bookmark_list(),
        Err(error) => {
            session.close_bookmark_list();
            session.set_error(error.to_string());
        }
    }
}

/// Opens the rename prompt over the list, prefilled with the selected row's label. An empty
/// list has nothing to rename, so `r` does nothing (§13.3b).
fn rename_selected(session: &mut Session) {
    let old_label = match session.bookmarks() {
        Bookmarks::Locked(locked) => locked
            .file()
            .bookmarks()
            .get(session.bookmark_list().selected())
            .map(|mark| mark.label().to_string()),
        Bookmarks::Free => None,
    };
    if let Some(old_label) = old_label {
        let at = session.bookmark_list().selected();
        session.open_rename_prompt(at, &old_label);
    }
}

/// Opens the selected bookmark's page — a visit, so `←` comes back to it — and closes the
/// list. With no bookmark selected (an empty list) nothing happens. A page that cannot be
/// shown leaves the list open and the error on the status line.
fn visit_selected(session: &mut Session) {
    let index = match session.bookmarks() {
        Bookmarks::Locked(locked) => locked
            .file()
            .bookmarks()
            .get(session.bookmark_list().selected())
            .map(|mark| mark.index().clone()),
        Bookmarks::Free => None,
    };
    let Some(index) = index else {
        return;
    };
    if session.visit(index).is_ok() {
        session.close_bookmark_list();
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use bitbabel_core::{LibraryConfig, PageIndex};
    use ratatui::crossterm::event::KeyCode;

    use super::*;
    use crate::app::Prompt;
    use crate::app::test_util::*;
    use crate::files::{FailingFiles, MemFiles};

    const SMALL: LibraryConfig = LibraryConfig::SMALL;

    fn marks_app(count: usize) -> App {
        marks_app_with_files(SMALL, count).0
    }

    fn session_of(app: &App) -> &Session {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session
    }

    fn list_is_open(app: &App) -> bool {
        session_of(app).bookmark_list_is_open()
    }

    fn selected_of(app: &App) -> usize {
        session_of(app).bookmark_list().selected()
    }

    fn top_of(app: &App) -> usize {
        session_of(app).bookmark_list().top()
    }

    fn prompt_of(app: &App) -> &Prompt {
        session_of(app).prompt().expect("a prompt should be open")
    }

    #[test]
    fn shift_b_opens_the_list_and_esc_closes_it() {
        let mut app = marks_app(2);
        app.handle_key(press(KeyCode::Char('B')));
        assert!(list_is_open(&app));
        app.handle_key(press(KeyCode::Esc));
        assert!(!list_is_open(&app));
    }

    #[test]
    fn q_closes_the_list_rather_than_quitting() {
        let mut app = marks_app(2);
        app.handle_key(press(KeyCode::Char('B')));
        assert!(app.handle_key(press(KeyCode::Char('q'))).is_empty());
        assert!(!list_is_open(&app));
        // The app is still running: B opens the list again.
        app.handle_key(press(KeyCode::Char('B')));
        assert!(list_is_open(&app));
    }

    #[test]
    fn a_free_session_says_there_is_no_file_and_shows_no_list() {
        let mut app = small_app();
        app.handle_key(press(KeyCode::Char('B')));
        assert!(!list_is_open(&app));
        let status = status_of(&app).unwrap();
        assert!(!status.error);
        assert_eq!(status.text, "no bookmark file");
    }

    #[test]
    fn arrows_and_j_k_move_the_selection_and_stop_at_the_first_row() {
        let mut app = marks_app(4);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Down));
        app.handle_key(press(KeyCode::Char('j')));
        assert_eq!(selected_of(&app), 2);
        app.handle_key(press(KeyCode::Up));
        app.handle_key(press(KeyCode::Char('k')));
        app.handle_key(press(KeyCode::Char('k')));
        assert_eq!(selected_of(&app), 0);
    }

    #[test]
    fn the_window_follows_the_selection_down_and_up() {
        // The default screen is 24 rows, so 18 bookmark rows fit.
        let mut app = marks_app(40);
        app.handle_key(press(KeyCode::Char('B')));
        for _ in 0..30 {
            app.handle_key(press(KeyCode::Char('j')));
        }
        assert_eq!(selected_of(&app), 30);
        assert_eq!(top_of(&app), 30 - 18 + 1);
        // Back up to the top row of the window does not move it; one further row does.
        for _ in 0..17 {
            app.handle_key(press(KeyCode::Char('k')));
        }
        assert_eq!(selected_of(&app), 13);
        assert_eq!(top_of(&app), 13);
    }

    #[test]
    fn enter_visits_the_page_is_a_history_visit_and_closes() {
        let mut app = marks_app(2);
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Down));
        app.handle_key(press(KeyCode::Enter));
        assert!(!list_is_open(&app));
        assert_eq!(
            index_of(&app),
            PageIndex::from_u64(2, LibraryConfig::SMALL.page_len())
        );
        // It was a visit: one back returns to where the session started.
        app.handle_key(press(KeyCode::Left));
        assert_eq!(index_of(&app), start);
    }

    #[test]
    fn list_keys_do_not_fall_through_to_the_explore_table() {
        // `e` must not cycle the view, `l` must not step a page, while the list is open.
        // (`r` is the list's rename now, tested separately.)
        let mut app = marks_app(2);
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('B')));
        for c in ['e', 'l', 'x', 'h', 'n'] {
            app.handle_key(press(KeyCode::Char(c)));
        }
        assert_eq!(index_of(&app), start);
        assert!(list_is_open(&app));
        assert_eq!(selected_of(&app), 0);
    }

    #[test]
    fn the_selection_survives_closing_and_reopening() {
        let mut app = marks_app(3);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('j')));
        app.handle_key(press(KeyCode::Char('j')));
        app.handle_key(press(KeyCode::Esc));
        app.handle_key(press(KeyCode::Char('B')));
        assert_eq!(selected_of(&app), 2);
    }

    #[test]
    fn an_empty_list_opens_scrolls_nowhere_and_has_nothing_to_visit() {
        let files = Arc::new(MemFiles::default().with(MARKS, &empty_marks()));
        let mut app = bookmarked_app(files);
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('B')));
        assert!(list_is_open(&app));
        app.handle_key(press(KeyCode::Char('j')));
        app.handle_key(press(KeyCode::Enter));
        assert!(list_is_open(&app));
        assert_eq!(index_of(&app), start);
        assert_eq!(selected_of(&app), 0);
    }

    #[test]
    fn ctrl_or_alt_keys_do_nothing_and_ctrl_c_still_quits() {
        let mut app = marks_app(2);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
        assert_eq!(selected_of(&app), 0);
        assert!(list_is_open(&app));
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(ctrl_c), vec![Effect::Quit]);
    }

    #[test]
    fn r_opens_the_rename_prompt_prefilled_with_the_row_label() {
        let mut app = marks_app(2);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('j')));
        app.handle_key(press(KeyCode::Char('r')));
        let prompt = prompt_of(&app);
        assert_eq!(prompt.title(), "rename");
        assert_eq!(prompt.input(), "mark 1");
        assert!(prompt.label().contains("bookmark 2"), "{}", prompt.label());
        assert_eq!(prompt.footer(), "enter save   esc back");
        assert!(!list_is_open(&app));
    }

    #[test]
    fn esc_on_the_rename_returns_to_the_list_with_the_selection_kept() {
        let mut app = marks_app(2);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('j')));
        app.handle_key(press(KeyCode::Char('r')));
        app.handle_key(press(KeyCode::Esc));
        assert!(list_is_open(&app));
        assert_eq!(selected_of(&app), 1);
    }

    #[test]
    fn a_rename_writes_the_new_label_and_time_and_index_stay() {
        let files = Arc::new(MemFiles::default().with(MARKS, &marks_bytes(SMALL, 2)));
        let mut app = bookmarked_app(files.clone());
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('r')));
        for _ in 0..6 {
            app.handle_key(press(KeyCode::Backspace));
        }
        type_text(&mut app, "kept");
        app.handle_key(press(KeyCode::Enter));
        // Back to the list, prompt gone.
        assert!(list_is_open(&app));
        assert!(session_of(&app).prompt().is_none());
        let on_disk = String::from_utf8(files.get(MARKS).unwrap()).unwrap();
        // Same time and index, new label.
        assert!(on_disk.contains("1790812800\tkept\t"), "{on_disk}");
        assert!(!on_disk.contains("mark 0"), "{on_disk}");
    }

    #[test]
    fn the_rename_box_refuses_characters_a_label_cannot_hold() {
        // The same filter as the bookmark label: a control character typed here would
        // otherwise only fail when the file is written (§13.3b).
        let mut app = marks_app(1);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('r')));
        let before = prompt_of(&app).input().to_string();
        app.handle_key(press(KeyCode::Char('\t')));
        app.handle_key(press(KeyCode::Char('\u{200b}')));
        assert_eq!(prompt_of(&app).input(), before);
    }

    #[test]
    fn an_empty_rename_is_allowed() {
        let files = Arc::new(MemFiles::default().with(MARKS, &marks_bytes(SMALL, 1)));
        let mut app = bookmarked_app(files.clone());
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('r')));
        for _ in 0..6 {
            app.handle_key(press(KeyCode::Backspace));
        }
        app.handle_key(press(KeyCode::Enter));
        assert!(list_is_open(&app));
        let on_disk = String::from_utf8(files.get(MARKS).unwrap()).unwrap();
        assert!(
            on_disk.ends_with("\t\tAAAAAAAAAAAAAAAAAAAAAQ==\n"),
            "{on_disk}"
        );
    }

    #[test]
    fn a_failed_write_keeps_the_prompt_open_and_the_list_unchanged() {
        let files = Arc::new(FailingFiles::default().with(MARKS, &marks_bytes(SMALL, 2)));
        files.fail_writes(true);
        let config = crate::config::TuiConfig {
            size: None,
            key: None,
            bookmarks: Some(MARKS.into()),
        };
        let mut app = App::new(config, fake_random, Box::new(SharedFiles(files)))
            .unwrap()
            .with_clock(fixed_now);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('r')));
        type_text(&mut app, "!");
        app.handle_key(press(KeyCode::Enter));
        // Still the prompt, with the reason in it, and what was typed kept.
        let prompt = prompt_of(&app);
        assert!(
            prompt.error().unwrap().contains("cannot save"),
            "{prompt:?}"
        );
        assert_eq!(prompt.input(), "mark 0!");
        // Esc returns to the list; nothing was renamed.
        app.handle_key(press(KeyCode::Esc));
        assert!(list_is_open(&app));
        assert_eq!(session_of(&app).bookmark_count(), 2);
    }

    #[test]
    fn d_deletes_the_selected_row_at_once_with_no_question() {
        let files = Arc::new(MemFiles::default().with(MARKS, &marks_bytes(SMALL, 2)));
        let mut app = bookmarked_app(files.clone());
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('j')));
        app.handle_key(press(KeyCode::Char('d')));
        // No confirmation step: one press, gone. The list stays open, re-clamped.
        assert!(list_is_open(&app));
        assert_eq!(session_of(&app).bookmark_count(), 1);
        assert_eq!(selected_of(&app), 0, "the last row stepped back");
        let on_disk = String::from_utf8(files.get(MARKS).unwrap()).unwrap();
        assert!(on_disk.contains("mark 0"), "{on_disk}");
        assert!(!on_disk.contains("mark 1"), "{on_disk}");
    }

    #[test]
    fn deleting_a_row_keeps_the_selection_on_the_row_that_moved_up() {
        let mut app = marks_app(3);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('d'))); // row 0 gone, selection stays on 0
        assert_eq!(selected_of(&app), 0);
        // Enter now visits what was row 1: page 2.
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(
            index_of(&app),
            PageIndex::from_u64(2, LibraryConfig::SMALL.page_len())
        );
    }

    #[test]
    fn deleting_the_only_bookmark_leaves_the_empty_list_open() {
        let mut app = marks_app(1);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('d')));
        assert!(list_is_open(&app));
        assert_eq!(session_of(&app).bookmark_count(), 0);
        assert!(status_of(&app).is_none(), "an empty list is not an error");
        app.handle_key(press(KeyCode::Char('j'))); // the empty list scrolls nowhere
        assert_eq!(selected_of(&app), 0);
    }

    #[test]
    fn d_on_the_empty_list_does_nothing() {
        let mut app = marks_app(0);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('d')));
        assert!(list_is_open(&app));
        assert!(status_of(&app).is_none());
    }

    #[test]
    fn a_failed_delete_closes_the_list_and_shows_the_reason() {
        // The list box covers the status line, so the error is readable only outside it.
        let files = Arc::new(FailingFiles::default().with(MARKS, &marks_bytes(SMALL, 2)));
        files.fail_writes(true);
        let config = crate::config::TuiConfig {
            size: None,
            key: None,
            bookmarks: Some(MARKS.into()),
        };
        let mut app = App::new(config, fake_random, Box::new(SharedFiles(files.clone())))
            .unwrap()
            .with_clock(fixed_now);
        app.handle_key(press(KeyCode::Char('B')));
        app.handle_key(press(KeyCode::Char('d')));
        assert!(!list_is_open(&app));
        let status = status_of(&app).unwrap();
        assert!(status.error);
        assert!(status.text.contains("cannot save"), "{}", status.text);
        // Memory and disk still hold both bookmarks.
        assert_eq!(session_of(&app).bookmark_count(), 2);
        let on_disk = String::from_utf8(files.get(MARKS).unwrap()).unwrap();
        assert!(
            on_disk.contains("mark 0") && on_disk.contains("mark 1"),
            "{on_disk}"
        );
    }
}
