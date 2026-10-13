//! Everything about the library being explored: the page and how it is shown.

use bitbabel_core::{Cursor, LibraryError, PageIndex};

use super::library::LibraryChoice;
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
    /// The first visible line.
    scroll: usize,
    /// How many page lines fit on screen.
    viewport: usize,
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
        let cursor = Cursor::new(opened, first)?;
        let page = cursor.current()?.bytes().to_vec();
        let mut session = Session {
            library,
            cursor,
            page,
            view: View::Text,
            lines: Vec::new(),
            scroll: 0,
            viewport,
        };
        session.refresh_lines();
        Ok(session)
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

    /// The first visible line.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// How many page lines fit on screen.
    #[cfg(test)]
    pub fn viewport(&self) -> usize {
        self.viewport
    }

    /// The largest scroll position: the last line sits on the last row of the screen.
    fn max_scroll(&self) -> usize {
        self.lines.len().saturating_sub(self.viewport)
    }

    /// Tells the session how many page lines fit on screen, and keeps the scroll inside.
    pub fn set_viewport(&mut self, rows: usize) {
        self.viewport = rows;
        self.scroll = self.scroll.min(self.max_scroll());
    }

    /// Scrolls by `lines` (negative is up), stopping at the first and last line.
    pub fn scroll_by(&mut self, lines: isize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(lines)
            .min(self.max_scroll());
    }

    /// Scrolls half a screen down or up.
    pub fn scroll_half_page(&mut self, down: bool) {
        let half = (self.viewport / 2).max(1) as isize;
        self.scroll_by(if down { half } else { -half });
    }

    /// Scrolls to the first line.
    pub fn scroll_to_top(&mut self) {
        self.scroll = 0;
    }

    /// Scrolls so the last line is on the last row of the screen.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll = self.max_scroll();
    }

    /// Makes the display lines again. Called after every change of the page or the view.
    fn refresh_lines(&mut self) {
        self.lines = render(self.view, &self.page, None);
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
    fn scrolling_stops_at_both_ends() {
        let mut session = session(LibraryConfig::MEDIUM);
        session.scroll_by(-5);
        assert_eq!(session.scroll(), 0);
        session.scroll_by(1000);
        assert_eq!(session.scroll(), 50 - 18);
        session.scroll_to_top();
        assert_eq!(session.scroll(), 0);
        session.scroll_to_bottom();
        assert_eq!(session.scroll(), 32);
    }

    #[test]
    fn half_page_scrolls_half_the_viewport() {
        let mut session = session(LibraryConfig::MEDIUM);
        session.scroll_half_page(true);
        assert_eq!(session.scroll(), 9);
        session.scroll_half_page(false);
        assert_eq!(session.scroll(), 0);
    }

    #[test]
    fn a_page_that_fits_does_not_scroll() {
        let mut session = session(LibraryConfig::SMALL);
        session.scroll_by(5);
        session.scroll_to_bottom();
        assert_eq!(session.scroll(), 0);
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
}
