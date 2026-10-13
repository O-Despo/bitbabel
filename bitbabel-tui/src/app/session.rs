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
}

impl Session {
    /// A session at page 0 of `library`, on the text view.
    ///
    /// # Errors
    ///
    /// [`LibraryError`] if the library cannot be built.
    pub fn new(library: LibraryChoice) -> Result<Self, LibraryError> {
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
        Session::new(LibraryChoice::new(size, None)).unwrap()
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
        let private = Session::new(LibraryChoice::new(
            LibraryConfig::SMALL,
            Some(bitbabel_core::Key::from_bytes([1; 32])),
        ))
        .unwrap();
        assert_ne!(private.page(), session(LibraryConfig::SMALL).page());
    }

    #[test]
    fn the_lines_show_every_byte_of_the_page() {
        let session = session(LibraryConfig::MEDIUM);
        assert_eq!(session.page().len(), 3200);
        assert_eq!(session.lines().len(), 50);
        assert_eq!(session.lines().last().unwrap().offset, 49 * 64);
    }
}
