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
            let fresh = session.search_form().is_some_and(SearchForm::is_fresh);
            // Results for this needle are already there; Enter on them opens one (next step).
            if !fresh {
                run_search(session, random);
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
