//! The search overlay: a needle box and ten results.
//!
//! Search builds pages that contain the needle and asks for each page's index (core does the
//! work). The needle is typed in the current view's encoding. The form stays on the session
//! when the overlay is closed, so the results are still there to step through.

use bitbabel_core::{BabelLibrary, PageIndex, SearchOptions};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::effect::Effect;
use super::needle;
use super::session::Session;
use super::{App, Mode, typed_char};
use crate::random::RandomFn;
use crate::render::View;

/// How many results a search shows.
pub const RESULTS: usize = 10;

/// One search result: the page's index and where the needle sits in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    index: PageIndex,
    start: usize,
    end: usize,
}

impl Hit {
    /// The index of the page that holds the needle.
    pub fn index(&self) -> &PageIndex {
        &self.index
    }

    /// The first byte of the needle in the page.
    pub fn start(&self) -> usize {
        self.start
    }

    /// The bytes of the page that are the needle.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

/// The needle being typed and the results of the last search.
#[derive(Debug, Clone, Default)]
pub struct SearchForm {
    input: String,
    /// Why the last Enter did not search. The overlay stays open.
    error: Option<String>,
    hits: Vec<Hit>,
    selected: usize,
    /// The needle text and the view it was read in, which the current results belong to.
    /// Editing the needle, or changing the view, makes them stale: the same text means
    /// different bytes in another view.
    searched: Option<(String, View)>,
    /// The result opened last. `n` and `N` step from here, whatever page you are on now.
    last_opened: Option<usize>,
}

impl SearchForm {
    /// What has been typed.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Why the last Enter did not search, if it did not.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The results of the last search.
    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }

    /// The highlighted result row.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Whether the results are for what is typed now, read in `view`.
    pub fn is_fresh(&self, view: View) -> bool {
        let same = self
            .searched
            .as_ref()
            .is_some_and(|(text, searched_in)| *text == self.input && *searched_in == view);
        !self.hits.is_empty() && same
    }

    /// How many bytes the needle is in `view`, or `None` if it is empty or not valid there.
    pub fn byte_count(&self, view: View) -> Option<usize> {
        needle::parse(view, &self.input)
            .ok()
            .map(|bytes| bytes.len())
    }

    fn select(&mut self, down: bool) {
        let last = self.hits.len().saturating_sub(1);
        self.selected = if down {
            (self.selected + 1).min(last)
        } else {
            self.selected.saturating_sub(1)
        };
    }

    /// Searches `library` for `needle`, which was parsed from the typed text in `view`. A
    /// problem is kept as the error and the earlier results are left alone.
    fn search(&mut self, library: &BabelLibrary, view: View, needle: &[u8], seed: [u8; 32]) {
        let options = SearchOptions::surrounded(seed).limit(RESULTS);
        match library.search(needle, &options) {
            Ok(found) => {
                self.hits = found
                    .iter()
                    .map(|result| Hit {
                        index: result.page().index().clone(),
                        start: result.start(),
                        end: result.end(),
                    })
                    .collect();
                self.selected = 0;
                self.last_opened = None;
                self.searched = Some((self.input.clone(), view));
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    /// The needle text of the results a hit was opened from: `Some` while the page on screen
    /// answers to these results. A new search makes it `None` again until one of its hits is
    /// opened, so the label prefill never names a needle the page does not hold (§13.3b).
    pub fn opened_needle(&self) -> Option<&str> {
        match (self.last_opened, &self.searched) {
            (Some(_), Some((text, _))) => Some(text.as_str()),
            _ => None,
        }
    }
}

/// Handles a key while the search overlay is open.
pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Vec<Effect> {
    let random = app.random;
    let Mode::Explore(session) = &mut app.mode else {
        return vec![];
    };
    // Ctrl+R: the same needle with a new seed, so a new set of results.
    if key.code == KeyCode::Char('r') && key.modifiers == KeyModifiers::CONTROL {
        run_search(session, random);
        return vec![];
    }
    match key.code {
        KeyCode::Esc => session.close_search(),
        KeyCode::Enter => {
            // Results for this needle are already there: Enter opens the picked one. Otherwise
            // Enter searches.
            let view = session.view();
            let picked = session
                .search_form()
                .filter(|form| form.is_fresh(view))
                .map(SearchForm::selected);
            match picked {
                Some(at) => open_hit(session, at),
                None => run_search(session, random),
            }
        }
        KeyCode::Up | KeyCode::Down => {
            if let Some(form) = session.search_form_mut() {
                form.select(key.code == KeyCode::Down);
            }
        }
        KeyCode::Backspace => {
            if let Some(form) = session.search_form_mut() {
                form.input.pop();
                form.error = None;
            }
        }
        _ => {
            if let (Some(c), Some(form)) = (typed_char(&key), session.search_form_mut()) {
                form.input.push(c);
                form.error = None;
            }
        }
    }
    vec![]
}

/// Opens result `at` of the saved set: goes to its page, marks the needle in it and scrolls to
/// the mark. The overlay closes, and the set stays so `n` and `N` can carry on from here.
///
/// A failure goes to the status line, which shows whether or not the overlay is open.
fn open_hit(session: &mut Session, at: usize) {
    let Some((hit, total)) = session
        .search_form()
        .and_then(|form| Some((form.hits.get(at)?.clone(), form.hits.len())))
    else {
        return;
    };
    match session.visit_with_highlight(hit.index().clone(), hit.range()) {
        Ok(()) => {
            if let Some(form) = session.search_form_mut() {
                form.selected = at;
                form.last_opened = Some(at);
            }
            session.close_search();
            session.set_status(format!("search result {}/{total}", at + 1));
        }
        Err(error) => session.set_error(error.to_string()),
    }
}

/// `n` and `N`: open the next or previous result of the saved set, counting from the one opened
/// last and wrapping at the ends. It is a visit like any other. With no results it says so.
pub(super) fn step_result(session: &mut Session, forward: bool) {
    let next = session.search_form().and_then(|form| {
        let total = form.hits.len();
        if total == 0 {
            return None;
        }
        Some(match (form.last_opened, forward) {
            (Some(at), true) => (at + 1) % total,
            (Some(at), false) => (at + total - 1) % total,
            (None, true) => 0,
            (None, false) => total - 1,
        })
    });
    match next {
        Some(at) => open_hit(session, at),
        None => session.set_status("no search results, press / to search"),
    }
}

/// Searches with a fresh random seed. Ctrl+R in the overlay does this for a new set of
/// results from the same needle.
///
/// The needle is checked first, so a bad needle is reported as that and never uses randomness.
pub(super) fn run_search(session: &mut Session, random: RandomFn) {
    let view = session.view();
    let library = session.library().open();
    let Some(form) = session.search_form_mut() else {
        return;
    };
    let needle = match needle::parse(view, &form.input) {
        Ok(needle) => needle,
        Err(message) => return form.set_error(message),
    };
    let library = match library {
        Ok(library) => library,
        Err(error) => return form.set_error(error.to_string()),
    };
    let mut seed = [0u8; 32];
    if let Err(error) = random(&mut seed) {
        return form.set_error(error.to_string());
    }
    form.search(&library, view, &needle, seed);
}

#[cfg(test)]
mod test {
    use bitbabel_core::LibraryConfig;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::app::library::LibraryChoice;
    use crate::app::test_util::*;
    use crate::config::TuiConfig;
    use crate::files::MemFiles;

    #[test]
    fn a_result_that_cannot_be_opened_is_an_error_on_the_status_line() {
        let choice = LibraryChoice::new(LibraryConfig::SMALL, None);
        let mut session = Session::new(choice, 18).unwrap();
        session.open_search();
        // A result with an index of the wrong length cannot come from a real search.
        let bad = Hit {
            index: PageIndex::zero(4),
            start: 0,
            end: 1,
        };
        if let Some(form) = session.search_form_mut() {
            form.hits.push(bad);
        }
        open_hit(&mut session, 0);

        let status = session.status().expect("a status should be set");
        assert!(status.error);
        assert_eq!(status.text, "expected 16 bytes, got 4");
        // The page did not change, and the overlay is still open to read.
        assert_eq!(session.index(), &PageIndex::zero(16));
        assert!(session.search_is_open());
    }

    #[test]
    fn opening_one_overlay_replaces_the_other() {
        use crate::app::prompt::PromptKind;

        let choice = LibraryChoice::new(LibraryConfig::SMALL, None);
        let mut session = Session::new(choice, 18).unwrap();
        session.open_search();
        assert!(session.search_is_open());
        assert!(session.prompt().is_none());

        session.open_prompt(PromptKind::GoTo);
        assert!(session.prompt().is_some());
        assert!(!session.search_is_open());
        // The form and its needle are still kept for the next time.
        assert!(session.search_form().is_some());

        // Closing the one that is not open does nothing.
        session.close_search();
        assert!(session.prompt().is_some());
        session.close_prompt();
        assert!(session.prompt().is_none());
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
            bookmarks: None,
        };
        let mut app = App::new(config, fake_random, Box::new(MemFiles::default())).unwrap();
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
        assert!(form.is_fresh(View::Text));

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
            bookmarks: None,
        };
        let mut app = App::new(config, failing_random, Box::new(MemFiles::default())).unwrap();
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
        assert!(!form_of(&app).is_fresh(View::Text));
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
            bookmarks: None,
        };
        let mut app = App::new(config, fake_random, Box::new(MemFiles::default())).unwrap();
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
        assert!(form_of(&app).is_fresh(View::Text));
        assert_eq!(form_of(&app).hits().len(), 10);
    }

    /// Searches for `needle` on a small library, closes the overlay and returns the app.
    fn with_results(needle: &str) -> App {
        let mut app = searching(needle);
        app.handle_key(press(KeyCode::Enter));
        app.handle_key(press(KeyCode::Esc));
        app
    }

    fn hit_index(app: &App, at: usize) -> bitbabel_core::PageIndex {
        form_of(app).hits()[at].index().clone()
    }

    #[test]
    fn n_opens_the_first_result_then_the_next_ones() {
        let mut app = with_results("hi");
        app.handle_key(press(KeyCode::Char('n')));
        assert_eq!(index_of(&app), hit_index(&app, 0));
        app.handle_key(press(KeyCode::Char('n')));
        assert_eq!(index_of(&app), hit_index(&app, 1));
        assert_eq!(status_of(&app).unwrap().text, "search result 2/10");
    }

    #[test]
    fn capital_n_steps_back_and_both_wrap() {
        let mut app = with_results("hi");
        app.handle_key(press(KeyCode::Char('N'))); // from nothing: the last
        assert_eq!(index_of(&app), hit_index(&app, 9));
        app.handle_key(press(KeyCode::Char('n'))); // wraps to the first
        assert_eq!(index_of(&app), hit_index(&app, 0));
        app.handle_key(press(KeyCode::Char('N'))); // wraps back to the last
        assert_eq!(index_of(&app), hit_index(&app, 9));
    }

    #[test]
    fn n_counts_from_the_last_result_opened_even_after_wandering() {
        let mut app = with_results("hi");
        app.handle_key(press(KeyCode::Char('n')));
        app.handle_key(press(KeyCode::Char('n')));
        app.handle_key(press(KeyCode::Char('r'))); // wander off to a random page
        app.handle_key(press(KeyCode::Char('l')));
        app.handle_key(press(KeyCode::Char('n')));
        assert_eq!(index_of(&app), hit_index(&app, 2));
    }

    #[test]
    fn a_result_opened_with_n_is_marked_and_is_a_visit() {
        let mut app = with_results("hi");
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('n')));
        assert_eq!(highlight_of(&app), Some(form_of(&app).hits()[0].range()));
        app.handle_key(press(KeyCode::Left));
        assert_eq!(index_of(&app), start);
    }

    #[test]
    fn opening_from_the_overlay_starts_n_from_that_row() {
        let mut app = searching("hi");
        app.handle_key(press(KeyCode::Enter));
        app.handle_key(press(KeyCode::Down));
        app.handle_key(press(KeyCode::Down));
        app.handle_key(press(KeyCode::Enter)); // opens row 3
        app.handle_key(press(KeyCode::Char('n')));
        assert_eq!(index_of(&app), hit_index(&app, 3));
    }

    #[test]
    fn a_new_search_replaces_the_set_and_starts_n_over() {
        let mut app = with_results("hi");
        app.handle_key(press(KeyCode::Char('n')));
        app.handle_key(press(KeyCode::Char('n')));
        app.handle_key(press(KeyCode::Char('/')));
        type_text(&mut app, "x");
        app.handle_key(press(KeyCode::Enter)); // searches "hix"
        app.handle_key(press(KeyCode::Esc));
        app.handle_key(press(KeyCode::Char('n')));
        assert_eq!(index_of(&app), hit_index(&app, 0));
    }

    #[test]
    fn n_with_no_results_says_to_search() {
        let mut app = medium_app();
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('n')));
        assert_eq!(index_of(&app), start);
        assert_eq!(
            status_of(&app).unwrap().text,
            "no search results, press / to search"
        );
    }

    #[test]
    fn ctrl_r_gives_a_new_set_for_the_same_needle() {
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: None,
            bookmarks: None,
        };
        let mut app = App::new(config, counting_random, Box::new(MemFiles::default())).unwrap();
        app.handle_key(press(KeyCode::Char('/')));
        type_text(&mut app, "hi");
        app.handle_key(press(KeyCode::Enter));
        let first = form_of(&app).hits().to_vec();

        app.handle_key(press(KeyCode::Down));
        let ctrl_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        app.handle_key(ctrl_r);
        let form = form_of(&app);
        assert_eq!(form.input(), "hi");
        assert_eq!(form.hits().len(), 10);
        assert_ne!(form.hits(), first.as_slice());
        // A new set starts at the top again.
        assert_eq!(form.selected(), 0);
        assert!(search_open(&app));
    }

    #[test]
    fn ctrl_r_with_a_bad_needle_is_an_inline_error() {
        let mut app = searching("a中");
        let ctrl_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        app.handle_key(ctrl_r);
        assert!(
            form_of(&app)
                .error()
                .unwrap()
                .contains("not in the BitBabel text table")
        );
    }

    #[test]
    fn plain_r_in_the_box_is_a_letter_not_a_new_set() {
        let mut app = searching("");
        app.handle_key(press(KeyCode::Char('r')));
        assert_eq!(form_of(&app).input(), "r");
    }

    #[test]
    fn results_are_stale_after_the_view_changes_even_with_the_same_text() {
        let mut app = searching("6869");
        app.handle_key(press(KeyCode::Enter));
        assert!(form_of(&app).is_fresh(View::Text));
        let before = form_of(&app).hits().to_vec();

        // The same text in the hex view is two bytes, `hi`, not four characters.
        app.handle_key(press(KeyCode::Esc));
        app.handle_key(press(KeyCode::Char('e')));
        app.handle_key(press(KeyCode::Char('/')));
        assert!(!form_of(&app).is_fresh(View::Hex));

        // So Enter searches again instead of opening a result of the old needle.
        app.handle_key(press(KeyCode::Enter));
        assert!(search_open(&app));
        assert!(form_of(&app).is_fresh(View::Hex));
        assert_ne!(form_of(&app).hits(), before.as_slice());
    }

    #[test]
    fn a_bad_needle_is_reported_even_when_the_random_source_fails() {
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: None,
            bookmarks: None,
        };
        let mut app = App::new(config, failing_random, Box::new(MemFiles::default())).unwrap();
        app.handle_key(press(KeyCode::Char('/')));
        type_text(&mut app, "a中");
        app.handle_key(press(KeyCode::Enter));
        let error = form_of(&app).error().unwrap().to_string();
        assert!(error.contains("not in the BitBabel text table"), "{error}");
    }

    #[test]
    fn counting_random_starts_at_one_on_every_test_thread() {
        let mut first = [0u8; 2];
        counting_random(&mut first).unwrap();
        assert_eq!(first, [1, 1]);
        let mut second = [0u8; 2];
        counting_random(&mut second).unwrap();
        assert_eq!(second, [2, 2]);
    }

    #[test]
    fn arrows_on_an_empty_result_list_stay_on_row_zero() {
        let mut app = searching("");
        app.handle_key(press(KeyCode::Down));
        app.handle_key(press(KeyCode::Up));
        app.handle_key(press(KeyCode::Down));
        assert_eq!(form_of(&app).selected(), 0);
    }

    #[test]
    fn a_character_typed_with_altgr_goes_into_the_needle() {
        let mut app = searching("");
        // Windows reports AltGr as Ctrl+Alt, and `@` on many layouts is AltGr+Q.
        let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
        app.handle_key(KeyEvent::new(KeyCode::Char('@'), altgr));
        assert_eq!(form_of(&app).input(), "@");
    }

    #[test]
    fn ctrl_or_alt_alone_never_types_into_the_needle() {
        let mut app = searching("");
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT));
        assert_eq!(form_of(&app).input(), "");
    }
}
