//! Everything about the library being explored: the page and how it is shown.

use std::ops::Range;

use bitbabel_core::{Cursor, LibraryError, PageIndex};

use super::bookmark_list::BookmarkList;
use super::library::LibraryChoice;
use super::prompt::{Prompt, PromptKind};
use super::search::SearchForm;
use super::viewport::Viewport;
use crate::bookmarks::{Bookmarks, SaveError};
use crate::files::Files;
use crate::history::History;
use crate::render::{DisplayLine, View, render};

/// One session in one library.
#[derive(Debug)]
pub struct Session {
    library: LibraryChoice,
    /// The library and the current index.
    cursor: Cursor,
    /// The bytes of the current page.
    page: Vec<u8>,
    view: View,
    /// The page as display lines. Made again whenever the page or the view changes, and
    /// only then.
    lines: Vec<DisplayLine>,
    /// Which lines of the page are on screen.
    viewport: Viewport,
    /// The pages visited, with the current one marked.
    history: History,
    status: Option<Status>,
    /// What is open over the page, if anything.
    overlay: Overlay,
    /// The `B` list's selection and window, kept while a prompt opens over the list.
    bookmark_list: BookmarkList,
    bookmarks: Bookmarks,
    /// The search form. It keeps its needle and results when the overlay is closed.
    search: Option<SearchForm>,
    /// The bytes of the page to mark, set when a search result is opened.
    highlight: Option<Range<usize>>,
}

/// What is open over the page. At most one thing is, so two overlays cannot both take keys.
#[derive(Debug)]
enum Overlay {
    /// Only the page.
    None,
    /// A one-line prompt (go to, export).
    Prompt(Prompt),
    /// The search box. Its form is kept on the session, so it outlives the overlay.
    Search,
    /// The list of keys.
    Help,
    /// The `B` list of bookmarks. Its state is kept on the session, not here: a prompt
    /// opens over the list (§13.3b).
    Bookmarks,
}

/// A message for the status line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub text: String,
    /// Errors are drawn in red.
    pub error: bool,
}

impl Session {
    /// A session at page 0 of `library`, on the text view. `viewport` is how many page lines
    /// fit on screen.
    ///
    /// # Errors
    ///
    /// [`LibraryError`] if the library cannot be built.
    pub fn new(library: LibraryChoice, viewport: usize) -> Result<Self, LibraryError> {
        let opened = library.open()?;
        let first = PageIndex::zero(opened.page_len());
        let cursor = Cursor::new(opened, first.clone())?;
        let page = cursor.current()?.bytes().to_vec();
        let mut session = Session {
            library,
            cursor,
            page,
            view: View::Text,
            lines: Vec::new(),
            viewport: Viewport::new(viewport),
            history: History::new(first),
            status: None,
            overlay: Overlay::None,
            bookmark_list: BookmarkList::default(),
            bookmarks: Bookmarks::Free,
            search: None,
            highlight: None,
        };
        session.refresh_lines();
        Ok(session)
    }

    /// Whether this session has a bookmark file.
    pub fn bookmarks(&self) -> &Bookmarks {
        &self.bookmarks
    }

    pub(super) fn bookmarks_mut(&mut self) -> &mut Bookmarks {
        &mut self.bookmarks
    }

    /// Binds the session to a bookmark file.
    pub(super) fn set_bookmarks(&mut self, bookmarks: Bookmarks) {
        self.bookmarks = bookmarks;
    }

    /// Whether the `B` bookmark list is showing.
    pub fn bookmark_list_is_open(&self) -> bool {
        matches!(self.overlay, Overlay::Bookmarks)
    }

    /// The list's selection and window, for drawing.
    pub(crate) fn bookmark_list(&self) -> &BookmarkList {
        &self.bookmark_list
    }

    /// Opens the `B` list, clamping the selection and the window inside the bookmarks the
    /// session has.
    pub fn open_bookmark_list(&mut self) {
        self.bookmark_list
            .fit(self.viewport.rows(), self.bookmark_count());
        self.overlay = Overlay::Bookmarks;
    }

    /// Opens the rename prompt over the `B` list, prefilled with row `at`'s label. The list
    /// state stays on the session, so Enter or Esc returns to it (§13.3b).
    pub fn open_rename_prompt(&mut self, at: usize, old: &str) {
        self.overlay = Overlay::Prompt(Prompt::renaming(at, old));
    }

    /// Closes the `B` list. The selection stays where it was for the next time.
    pub fn close_bookmark_list(&mut self) {
        if matches!(self.overlay, Overlay::Bookmarks) {
            self.overlay = Overlay::None;
        }
    }

    /// How many bookmarks the list would show: the file's, or none without a file.
    pub fn bookmark_count(&self) -> usize {
        match &self.bookmarks {
            Bookmarks::Locked(locked) => locked.count(),
            Bookmarks::Free => 0,
        }
    }

    /// Moves the list selection one row down or up, keeping it in view.
    pub(crate) fn move_bookmark_selection(&mut self, down: bool) {
        self.bookmark_list
            .move_selected(down, self.viewport.rows(), self.bookmark_count());
    }

    /// Deletes the selected bookmark of the `B` list, writing its file. Without a file it
    /// does nothing; the list never opens without one.
    pub(crate) fn delete_selected_bookmark(&mut self, files: &dyn Files) -> Result<(), SaveError> {
        let at = self.bookmark_list.selected();
        let Bookmarks::Locked(locked) = &mut self.bookmarks else {
            return Ok(());
        };
        locked.remove(files, at)
    }

    /// The library of this session.
    pub fn library(&self) -> &LibraryChoice {
        &self.library
    }

    /// The index of the current page.
    pub fn index(&self) -> &PageIndex {
        self.cursor.index()
    }

    /// How the page is shown.
    pub fn view(&self) -> View {
        self.view
    }

    /// The bytes of the current page.
    #[cfg(test)]
    pub fn page(&self) -> &[u8] {
        &self.page
    }

    /// The page as display lines.
    pub fn lines(&self) -> &[DisplayLine] {
        &self.lines
    }

    /// The message on the status line, if any.
    pub fn status(&self) -> Option<&Status> {
        self.status.as_ref()
    }

    /// The open prompt, if one is open.
    pub fn prompt(&self) -> Option<&Prompt> {
        match &self.overlay {
            Overlay::Prompt(prompt) => Some(prompt),
            _ => None,
        }
    }

    pub(super) fn prompt_mut(&mut self) -> Option<&mut Prompt> {
        match &mut self.overlay {
            Overlay::Prompt(prompt) => Some(prompt),
            _ => None,
        }
    }

    /// Opens a prompt. It gets every key until it is closed, and it replaces any other overlay.
    pub fn open_prompt(&mut self, kind: PromptKind) {
        self.overlay = Overlay::Prompt(Prompt::new(kind));
    }

    /// Closes the open prompt, if one is open.
    pub fn close_prompt(&mut self) {
        if matches!(self.overlay, Overlay::Prompt(_)) {
            self.overlay = Overlay::None;
        }
    }

    /// The search form, if it has been opened. It is kept after the overlay is closed.
    pub fn search_form(&self) -> Option<&SearchForm> {
        self.search.as_ref()
    }

    pub(super) fn search_form_mut(&mut self) -> Option<&mut SearchForm> {
        self.search.as_mut()
    }

    /// Whether the search overlay is showing. It gets every key while it is.
    pub fn search_is_open(&self) -> bool {
        matches!(self.overlay, Overlay::Search)
    }

    /// Opens the search overlay, with the last needle and results if there were any. It
    /// replaces any other overlay.
    pub fn open_search(&mut self) {
        self.search.get_or_insert_with(SearchForm::default);
        self.overlay = Overlay::Search;
    }

    /// Closes the search overlay. The needle and results are kept.
    pub fn close_search(&mut self) {
        if matches!(self.overlay, Overlay::Search) {
            self.overlay = Overlay::None;
        }
    }

    /// Whether the help screen is showing. Any key closes it.
    pub fn help_is_open(&self) -> bool {
        matches!(self.overlay, Overlay::Help)
    }

    /// Opens the help screen. It replaces any other overlay.
    pub fn open_help(&mut self) {
        self.overlay = Overlay::Help;
    }

    /// Closes the help screen.
    pub fn close_help(&mut self) {
        if matches!(self.overlay, Overlay::Help) {
            self.overlay = Overlay::None;
        }
    }

    /// Shows `message` inside the open prompt, which stays open.
    pub(super) fn set_prompt_error(&mut self, message: impl Into<String>) {
        if let Some(prompt) = self.prompt_mut() {
            prompt.set_error(message);
        }
    }

    /// Shows `text` on the status line.
    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            text: text.into(),
            error: false,
        });
    }

    /// Shows `text` on the status line as an error.
    pub fn set_error(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            text: text.into(),
            error: true,
        });
    }

    /// Shows the page at `index`. The scroll goes back to the top and the status clears.
    ///
    /// # Errors
    ///
    /// [`LibraryError::LenMismatch`] if `index` is not this library's length. Nothing changes.
    fn show(&mut self, index: PageIndex) -> Result<(), LibraryError> {
        let page = self.cursor.jump(index)?;
        self.page = page.bytes().to_vec();
        // Any change of page clears the mark; opening a search result sets it again after.
        self.highlight = None;
        self.status = None;
        self.viewport.go_top();
        self.refresh_lines();
        Ok(())
    }

    /// Goes to the page at `index` and records the visit in the history.
    ///
    /// # Errors
    ///
    /// [`LibraryError::LenMismatch`] if `index` is not this library's length. Nothing changes.
    pub fn visit(&mut self, index: PageIndex) -> Result<(), LibraryError> {
        self.show(index.clone())?;
        self.history.visit(index);
        Ok(())
    }

    /// Goes to the page at `index`, records the visit, marks `needle` in it and scrolls so the
    /// mark is in view. Changing the view or scrolling keeps the mark; any other change of page
    /// clears it.
    ///
    /// # Errors
    ///
    /// [`LibraryError::LenMismatch`] if `index` is not this library's length. Nothing changes.
    pub fn visit_with_highlight(
        &mut self,
        index: PageIndex,
        needle: Range<usize>,
    ) -> Result<(), LibraryError> {
        self.visit(index)?;
        let start = needle.start;
        self.highlight = Some(needle);
        self.refresh_lines();
        self.scroll_to_byte(start);
        Ok(())
    }

    /// Scrolls so the line holding `byte` is the first one shown, or as near as the page allows.
    fn scroll_to_byte(&mut self, byte: usize) {
        let line = self.lines.partition_point(|line| line.offset <= byte);
        self.viewport
            .go_line(line.saturating_sub(1), self.lines.len());
    }

    /// The bytes marked on the page, if any.
    #[cfg(test)]
    pub fn highlight(&self) -> Option<&Range<usize>> {
        self.highlight.as_ref()
    }

    /// Goes back one page in the history, or says there is none.
    pub fn back(&mut self) {
        match self.history.back().cloned() {
            Some(index) => self.show_from_history(index),
            None => self.set_status("this is the first page visited"),
        }
    }

    /// Goes forward one page in the history, or says there is none.
    pub fn forward(&mut self) {
        match self.history.forward().cloned() {
            Some(index) => self.show_from_history(index),
            None => self.set_status("this is the last page visited"),
        }
    }

    /// Shows a page the history moved to. It is not recorded again.
    fn show_from_history(&mut self, index: PageIndex) {
        if let Err(error) = self.show(index) {
            self.set_error(error.to_string());
        }
    }

    /// Goes to the next page or the one before it, wrapping at the ends.
    pub fn step(&mut self, forward: bool) {
        let mut index = self.cursor.index().clone();
        if forward {
            index.increment();
        } else {
            index.decrement();
        }
        // The new index has the old one's length, so this does not fail in practice.
        if let Err(error) = self.visit(index) {
            self.set_error(error.to_string());
        }
    }

    /// Shows the page in the next view, with the first visible line kept in view.
    pub fn cycle_view(&mut self) {
        let top = self
            .lines
            .get(self.viewport.top())
            .map_or(0, |line| line.offset);
        self.view = self.view.next();
        self.refresh_lines();
        self.scroll_to_byte(top);
    }

    /// The first visible line.
    pub fn scroll(&self) -> usize {
        self.viewport.top()
    }

    /// How many page lines fit on screen.
    pub fn viewport(&self) -> usize {
        self.viewport.rows()
    }

    /// Tells the session how many page lines fit on screen, and keeps the scroll inside.
    /// The bookmark list gets the same number of rows, so its window stays usable while a
    /// prompt is open over it.
    pub fn set_viewport(&mut self, rows: usize) {
        self.viewport.set_rows(rows, self.lines.len());
        self.bookmark_list.fit(rows, self.bookmark_count());
    }

    /// Scrolls by `lines` (negative is up), stopping at the first and last line.
    pub fn scroll_by(&mut self, lines: isize) {
        self.viewport.scroll_by(lines, self.lines.len());
    }

    /// Scrolls half a screen down or up.
    pub fn scroll_half_page(&mut self, down: bool) {
        self.viewport.scroll_half_page(down, self.lines.len());
    }

    /// Scrolls to the first line.
    pub fn scroll_to_top(&mut self) {
        self.viewport.go_top();
    }

    /// Scrolls so the last line is on the last row of the screen.
    pub fn scroll_to_bottom(&mut self) {
        self.viewport.go_bottom(self.lines.len());
    }

    /// Makes the display lines again. Called after every change of the page or the view.
    fn refresh_lines(&mut self) {
        self.lines = render(self.view, &self.page, self.highlight.as_ref());
    }
}

#[cfg(test)]
mod test {
    use bitbabel_core::{BabelLibrary, LibraryConfig};

    use super::*;

    fn session(size: LibraryConfig) -> Session {
        Session::new(LibraryChoice::new(size, None), 18).unwrap()
    }

    #[test]
    fn starts_at_page_zero_on_the_text_view() {
        let session = session(LibraryConfig::SMALL);
        assert_eq!(session.index(), &PageIndex::zero(16));
        assert_eq!(session.view(), View::Text);
        assert_eq!(session.lines().len(), 1);
        assert_eq!(session.page().len(), 16);
    }

    #[test]
    fn the_page_is_the_librarys_page_at_index_zero() {
        let session = session(LibraryConfig::SMALL);
        let library = BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
        let expected = library.page_at(&PageIndex::zero(16)).unwrap();
        assert_eq!(session.page(), expected.bytes());
    }

    #[test]
    fn a_private_key_gives_a_different_page() {
        let key = bitbabel_core::Key::from_bytes([1; 32]);
        let choice = LibraryChoice::new(LibraryConfig::SMALL, Some(key));
        let private = Session::new(choice, 18).unwrap();
        assert_ne!(private.page(), session(LibraryConfig::SMALL).page());
    }

    #[test]
    fn the_lines_show_every_byte_of_the_page() {
        let session = session(LibraryConfig::MEDIUM);
        assert_eq!(session.page().len(), 3200);
        assert_eq!(session.lines().len(), 50);
        assert_eq!(session.lines().last().unwrap().offset, 49 * 64);
    }

    #[test]
    fn a_resize_keeps_the_scroll_inside_the_page() {
        let mut session = session(LibraryConfig::MEDIUM);
        session.scroll_to_bottom();
        assert_eq!(session.scroll(), 32);
        session.set_viewport(40);
        assert_eq!(session.scroll(), 10);
        session.set_viewport(100);
        assert_eq!(session.scroll(), 0);
        assert_eq!(session.viewport(), 100);
    }

    fn number(n: u64, len: usize) -> PageIndex {
        PageIndex::from_u64(n, len)
    }

    #[test]
    fn step_moves_by_one_and_wraps_at_both_ends() {
        let mut session = session(LibraryConfig::SMALL);
        session.step(true);
        assert_eq!(session.index(), &number(1, 16));
        session.step(false);
        session.step(false);
        assert_eq!(session.index(), &PageIndex::from_bytes(vec![0xff; 16]));
        session.step(true);
        assert_eq!(session.index(), &PageIndex::zero(16));
    }

    #[test]
    fn step_shows_that_pages_bytes_and_resets_the_scroll() {
        let mut session = session(LibraryConfig::MEDIUM);
        session.scroll_by(10);
        let before = session.page().to_vec();
        session.step(true);
        let library = bitbabel_core::BabelLibrary::canonical(LibraryConfig::MEDIUM).unwrap();
        let expected = library.page_at(&number(1, 3200)).unwrap();
        assert_eq!(session.page(), expected.bytes());
        assert_ne!(session.page(), before);
        assert_eq!(session.scroll(), 0);
    }

    #[test]
    fn step_clears_a_status_message() {
        let mut session = session(LibraryConfig::SMALL);
        session.set_error("bad");
        assert_eq!(
            session.status(),
            Some(&Status {
                text: "bad".to_string(),
                error: true
            })
        );
        session.step(true);
        assert!(session.status().is_none());
    }

    #[test]
    fn cycling_the_view_goes_round_and_keeps_the_same_bytes() {
        let mut session = session(LibraryConfig::MEDIUM);
        let page = session.page().to_vec();
        assert_eq!(session.lines().len(), 50);
        session.cycle_view();
        assert_eq!(session.view(), View::Hex);
        assert_eq!(session.lines().len(), 100);
        session.cycle_view();
        assert_eq!(session.view(), View::Base64);
        assert_eq!(session.lines().len(), 67);
        session.cycle_view();
        assert_eq!(session.view(), View::Utf8);
        session.cycle_view();
        assert_eq!(session.view(), View::Text);
        assert_eq!(session.page(), page);
    }

    #[test]
    fn cycling_the_view_keeps_the_top_byte_in_view() {
        let mut session = session(LibraryConfig::MEDIUM);
        session.scroll_by(10); // text line 10 starts at byte 0x280
        session.cycle_view(); // hex lines hold 32 bytes, so byte 0x280 is on line 20
        assert_eq!(session.lines()[session.scroll()].offset, 0x280);
    }

    #[test]
    fn the_view_change_does_not_ask_the_library_for_the_page_again() {
        // The bytes are kept, so a view change only renders them again.
        let mut session = session(LibraryConfig::SMALL);
        let before = session.index().clone();
        session.cycle_view();
        assert_eq!(session.index(), &before);
    }

    #[test]
    fn back_and_forward_follow_the_pages_visited() {
        let mut session = session(LibraryConfig::SMALL);
        session.step(true);
        session.step(true);
        session.back();
        assert_eq!(session.index(), &number(1, 16));
        session.back();
        assert_eq!(session.index(), &number(0, 16));
        session.forward();
        assert_eq!(session.index(), &number(1, 16));
    }

    #[test]
    fn back_at_the_start_and_forward_at_the_end_say_so_and_stay() {
        let mut session = session(LibraryConfig::SMALL);
        session.back();
        assert_eq!(session.index(), &number(0, 16));
        assert_eq!(
            session.status().unwrap().text,
            "this is the first page visited"
        );
        session.forward();
        assert_eq!(
            session.status().unwrap().text,
            "this is the last page visited"
        );
    }

    #[test]
    fn moving_in_the_history_does_not_record_visits() {
        let mut session = session(LibraryConfig::SMALL);
        session.step(true);
        session.back();
        // A new visit now drops the page that was ahead, instead of keeping it.
        session.visit(number(9, 16)).unwrap();
        session.forward();
        assert_eq!(session.index(), &number(9, 16));
        assert_eq!(
            session.status().unwrap().text,
            "this is the last page visited"
        );
    }

    #[test]
    fn visit_checks_the_index_length_and_changes_nothing_on_error() {
        let mut session = session(LibraryConfig::SMALL);
        assert!(session.visit(number(5, 4)).is_err());
        assert_eq!(session.index(), &number(0, 16));
        session.forward();
        assert_eq!(
            session.status().unwrap().text,
            "this is the last page visited"
        );
    }

    #[test]
    fn going_back_shows_that_pages_bytes_again() {
        let mut session = session(LibraryConfig::SMALL);
        let first = session.page().to_vec();
        session.step(true);
        assert_ne!(session.page(), first);
        session.back();
        assert_eq!(session.page(), first);
    }
}
