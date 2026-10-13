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

    use super::*;
    use crate::app::library::LibraryChoice;

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
}
