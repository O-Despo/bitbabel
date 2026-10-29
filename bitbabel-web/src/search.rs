//! The results of a search: pages that contain the needle, found by building them, not by
//! scanning. The rules live here so they run in plain unit tests without a browser.

use std::ops::Range;

use bitbabel_core::{PageIndex, SearchOptions};
use bitbabel_explorer::{LibraryChoice, Segment, View, parse_needle, render};

use crate::explore::shorten;

/// How many results a search shows.
pub const RESULTS: usize = 10;

/// Bytes of the page kept on each side of the needle in a result row.
const CONTEXT: usize = 8;

/// One result: the page's index and where the needle sits in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    index: PageIndex,
    start: usize,
    end: usize,
    /// A few bytes around the needle as text, with the needle marked.
    context: Vec<Segment>,
}

impl Hit {
    /// The index of the page that holds the needle.
    pub fn index(&self) -> &PageIndex {
        &self.index
    }

    /// The bytes of the page that are the needle.
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// The index with the middle left out.
    pub fn index_short(&self) -> String {
        shorten(&self.index.to_base64())
    }

    /// The wall, shelf and volume of the page: `w4 s5 v31`.
    pub fn address(&self) -> String {
        let location = self.index.location();
        format!(
            "w{} s{} v{}",
            location.wall(),
            location.shelf(),
            location.volume()
        )
    }

    /// The needle and the bytes around it, ready to draw.
    pub fn context(&self) -> &[Segment] {
        &self.context
    }
}

/// The result of one search, and which result was opened last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Results {
    /// The text that was typed, in `view`.
    needle: String,
    view: View,
    hits: Vec<Hit>,
    /// The result opened last. `next` steps from here, whatever page is shown now.
    opened: Option<usize>,
    list_open: bool,
}

impl Results {
    /// Searches `library` for `text`, read in `view`, with `seed` choosing where the needle
    /// goes and what fills the rest.
    ///
    /// The needle is checked first, so a bad needle is reported as that and never uses the seed.
    ///
    /// # Errors
    ///
    /// A message for the screen: the needle is empty, is not valid in `view` or does not fit a
    /// page, or the seed is missing or not 32 bytes.
    pub fn run(
        library: &LibraryChoice,
        view: View,
        text: &str,
        seed: Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let needle = parse_needle(view, text)?;
        let seed = seed.map_err(|error| format!("cannot get random bytes: {error}"))?;
        let seed = <[u8; 32]>::try_from(seed.as_slice())
            .map_err(|_| format!("the search seed must be 32 bytes, got {}", seed.len()))?;
        let opened = library.open().map_err(|error| error.to_string())?;
        let options = SearchOptions::surrounded(seed).limit(RESULTS);
        let found = opened
            .search(&needle, &options)
            .map_err(|error| error.to_string())?;
        let hits = found
            .iter()
            .map(|result| Hit {
                index: result.page().index().clone(),
                start: result.start(),
                end: result.end(),
                context: context(result.page().bytes(), result.start(), result.end()),
            })
            .collect();
        Ok(Results {
            needle: text.to_string(),
            view,
            hits,
            opened: None,
            list_open: true,
        })
    }

    /// The text that was searched for.
    pub fn needle(&self) -> &str {
        &self.needle
    }

    /// The results.
    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }

    /// The result opened last, if any.
    pub fn opened(&self) -> Option<usize> {
        self.opened
    }

    /// Whether the list of results is showing.
    pub fn list_open(&self) -> bool {
        self.list_open
    }

    /// Whether these results are not for what is typed now: the same text means different
    /// bytes in another view.
    pub fn is_stale(&self, text: &str, view: View) -> bool {
        self.needle != text || self.view != view
    }

    /// Shows or hides the list.
    pub fn toggle_list(&mut self) {
        self.list_open = !self.list_open;
    }

    /// Records that result `at` was opened. The list stays as it is.
    pub fn mark_opened(&mut self, at: usize) {
        self.opened = Some(at);
    }

    /// The result after or before the one opened last, wrapping. Before any is opened, the
    /// first or the last. `None` when there are no results.
    pub fn neighbour(&self, forward: bool) -> Option<usize> {
        let total = self.hits.len();
        if total == 0 {
            return None;
        }
        Some(match (self.opened, forward) {
            (Some(at), true) => (at + 1) % total,
            (Some(at), false) => (at + total - 1) % total,
            (None, true) => 0,
            (None, false) => total - 1,
        })
    }
}

/// `bytes[start..end]` and a few bytes either side as one line of text, with the needle marked.
fn context(bytes: &[u8], start: usize, end: usize) -> Vec<Segment> {
    let from = start.saturating_sub(CONTEXT);
    let to = (end + CONTEXT).min(bytes.len());
    let marked = start - from..end - from;
    render(View::Text, &bytes[from..to], Some(&marked))
        .into_iter()
        .next()
        .map(|line| line.segments)
        .unwrap_or_default()
}

#[cfg(test)]
mod test {
    use bitbabel_core::{BabelLibrary, LibraryConfig};

    use super::*;

    fn small() -> LibraryChoice {
        LibraryChoice::new(LibraryConfig::SMALL, None)
    }

    fn seed() -> Result<Vec<u8>, String> {
        Ok(vec![5; 32])
    }

    fn search(text: &str, view: View) -> Result<Results, String> {
        Results::run(&small(), view, text, seed())
    }

    #[test]
    fn every_result_is_a_page_that_holds_the_needle_where_it_says() {
        let results = search("hi", View::Text).unwrap();
        assert!(!results.hits().is_empty());
        let library = BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
        for hit in results.hits() {
            let page = library.page_at(hit.index()).unwrap();
            assert_eq!(&page.bytes()[hit.range()], b"hi");
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_results_and_another_seed_does_not() {
        let again = search("hi", View::Text).unwrap();
        assert_eq!(search("hi", View::Text).unwrap(), again);
        let other = Results::run(&small(), View::Text, "hi", Ok(vec![6; 32])).unwrap();
        assert_ne!(other.hits(), again.hits());
    }

    #[test]
    fn results_start_with_the_list_open_and_nothing_opened() {
        let results = search("hi", View::Text).unwrap();
        assert!(results.list_open());
        assert_eq!(results.opened(), None);
        assert_eq!(results.needle(), "hi");
    }

    #[test]
    fn the_needle_is_read_in_the_current_view() {
        let results = search("6869", View::Hex).unwrap();
        let library = BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
        let hit = &results.hits()[0];
        let page = library.page_at(hit.index()).unwrap();
        assert_eq!(&page.bytes()[hit.range()], b"hi");
    }

    #[test]
    fn a_bad_needle_is_a_message_and_never_needs_the_seed() {
        let no_seed = |text, view| Results::run(&small(), view, text, Err("blocked".into()));
        assert_eq!(
            no_seed("", View::Text).unwrap_err(),
            "type something to search for"
        );
        assert!(no_seed("zz", View::Hex).unwrap_err().contains("hex"));
        assert!(
            no_seed("x", View::Text)
                .unwrap_err()
                .starts_with("cannot get random bytes")
        );
    }

    #[test]
    fn a_needle_longer_than_a_page_is_a_message() {
        let error = search(&"a".repeat(17), View::Text).unwrap_err();
        assert!(!error.is_empty());
    }

    #[test]
    fn a_seed_of_the_wrong_length_is_a_message() {
        let error = Results::run(&small(), View::Text, "hi", Ok(vec![1; 3])).unwrap_err();
        assert_eq!(error, "the search seed must be 32 bytes, got 3");
    }

    #[test]
    fn results_are_stale_when_the_text_or_the_view_changes() {
        let results = search("hi", View::Text).unwrap();
        assert!(!results.is_stale("hi", View::Text));
        assert!(results.is_stale("hit", View::Text));
        assert!(results.is_stale("hi", View::Hex));
    }

    #[test]
    fn neighbours_wrap_and_start_at_the_ends() {
        let mut results = search("hi", View::Text).unwrap();
        let total = results.hits().len();
        assert_eq!(results.neighbour(true), Some(0));
        assert_eq!(results.neighbour(false), Some(total - 1));
        results.mark_opened(total - 1);
        assert_eq!(results.neighbour(true), Some(0));
        results.mark_opened(0);
        assert_eq!(results.neighbour(false), Some(total - 1));
    }

    #[test]
    fn opening_a_result_leaves_the_list_as_it_was() {
        let mut results = search("hi", View::Text).unwrap();
        results.mark_opened(2);
        assert!(results.list_open());
        assert_eq!(results.opened(), Some(2));
        results.toggle_list();
        results.mark_opened(3);
        assert!(!results.list_open());
    }

    #[test]
    fn a_result_row_shows_the_needle_marked_among_the_bytes_around_it() {
        let results = search("hi", View::Text).unwrap();
        for hit in results.hits() {
            let marked: String = hit
                .context()
                .iter()
                .filter(|segment| segment.highlighted)
                .map(|segment| segment.text.as_str())
                .collect();
            assert_eq!(marked, "hi");
        }
    }

    #[test]
    fn a_long_needle_is_marked_whole_in_the_row() {
        let needle = "a".repeat(16);
        let results = search(&needle, View::Text).unwrap();
        let hit = &results.hits()[0];
        assert_eq!(hit.range().len(), 16);
        let marked: usize = hit
            .context()
            .iter()
            .filter(|segment| segment.highlighted)
            .map(|segment| segment.text.chars().count())
            .sum();
        assert_eq!(marked, 16);
    }

    #[test]
    fn a_hit_has_a_short_index_and_an_address() {
        let results = search("hi", View::Text).unwrap();
        let hit = &results.hits()[0];
        assert!(hit.index_short().len() <= 25);
        assert!(hit.address().starts_with('w'));
    }
}
