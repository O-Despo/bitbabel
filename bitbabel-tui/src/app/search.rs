//! The search overlay: a needle box and ten results.
//!
//! Search builds pages that contain the needle and asks for each page's index (core does the
//! work). The needle is typed in the current view's encoding. The form stays on the session
//! when the overlay is closed, so the results are still there to step through.

use bitbabel_core::{BabelLibrary, PageIndex, SearchOptions};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

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
    /// The needle text the current results belong to. Editing the needle makes them stale.
    searched: Option<String>,
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

    /// Whether the results are for what is typed now.
    pub fn is_fresh(&self) -> bool {
        !self.hits.is_empty() && self.searched.as_deref() == Some(self.input.as_str())
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

    /// Searches `library` for the typed needle. A problem is kept as the error and the
    /// earlier results are left alone.
    fn search(&mut self, library: &BabelLibrary, view: View, seed: [u8; 32]) {
        let bytes = match needle::parse(view, &self.input) {
            Ok(bytes) => bytes,
            Err(message) => {
                self.error = Some(message);
                return;
            }
        };
        let options = SearchOptions::surrounded(seed).limit(RESULTS);
        match library.search(&bytes, &options) {
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
                self.searched = Some(self.input.clone());
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
    match key.code {
        KeyCode::Esc => session.close_search(),
        KeyCode::Enter => {
            // Results for this needle are already there: Enter opens the picked one. Otherwise
            // Enter searches.
            let picked = session
                .search_form()
                .filter(|form| form.is_fresh())
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
fn open_hit(session: &mut Session, at: usize) {
    let Some(form) = session.search_form_mut() else {
        return;
    };
    let Some(hit) = form.hits.get(at).cloned() else {
        return;
    };
    let total = form.hits.len();
    match session.visit_with_highlight(hit.index().clone(), hit.range()) {
        Ok(()) => {
            if let Some(form) = session.search_form_mut() {
                form.selected = at;
                form.last_opened = Some(at);
            }
            session.close_search();
            session.set_status(format!("search result {}/{total}", at + 1));
        }
        Err(error) => {
            if let Some(form) = session.search_form_mut() {
                form.set_error(error.to_string());
            }
        }
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
pub(super) fn run_search(session: &mut Session, random: RandomFn) {
    let view = session.view();
    let library = session.library().open();
    let mut seed = [0u8; 32];
    let seeded = random(&mut seed);
    let Some(form) = session.search_form_mut() else {
        return;
    };
    match (library, seeded) {
        (Ok(library), Ok(())) => form.search(&library, view, seed),
        (Err(error), _) => form.set_error(error.to_string()),
        (_, Err(error)) => form.set_error(error.to_string()),
    }
}
