use crate::error::LibraryError;
use crate::index::PageIndex;
use crate::library::BabelLibrary;
use crate::page::Page;

/// A movable position in a [`BabelLibrary`].
///
/// Moving past either end wraps around, as [`PageIndex`] does.
#[derive(Debug, Clone)]
pub struct Cursor {
    library: BabelLibrary,
    index: PageIndex,
}

impl Cursor {
    /// # Errors
    ///
    /// Returns [`LibraryError::LenMismatch`] if `start` is not `page_len` bytes long.
    pub fn new(library: BabelLibrary, start: PageIndex) -> Result<Self, LibraryError> {
        check_len(&library, &start)?;
        Ok(Cursor {
            library,
            index: start,
        })
    }

    /// The current position.
    pub fn index(&self) -> &PageIndex {
        &self.index
    }

    /// The page at the current position.
    pub fn current(&self) -> Result<Page, LibraryError> {
        self.library.page_at(&self.index)
    }

    /// Moves forward one page and returns it.
    pub fn next_page(&mut self) -> Result<Page, LibraryError> {
        self.index.increment();
        self.current()
    }

    /// Moves back one page and returns it.
    pub fn prev_page(&mut self) -> Result<Page, LibraryError> {
        self.index.decrement();
        self.current()
    }

    /// Moves to `index` and returns its page.
    ///
    /// # Errors
    ///
    /// Returns [`LibraryError::LenMismatch`] if `index` is not `page_len` bytes long; the
    /// cursor stays where it was.
    pub fn jump(&mut self, index: PageIndex) -> Result<Page, LibraryError> {
        check_len(&self.library, &index)?;
        self.index = index;
        self.current()
    }
}

fn check_len(library: &BabelLibrary, index: &PageIndex) -> Result<(), LibraryError> {
    if index.as_bytes().len() == library.page_len() {
        Ok(())
    } else {
        Err(LibraryError::LenMismatch {
            expected: library.page_len(),
            actual: index.as_bytes().len(),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::cipher::BabelMachine;

    fn library(page_len: usize) -> BabelLibrary {
        BabelLibrary::new(BabelMachine::new([7u8; 32], 4).unwrap(), page_len).unwrap()
    }

    #[test]
    fn next_then_prev_returns_to_start() {
        let mut cursor = Cursor::new(library(8), PageIndex::from_u64(5, 8)).unwrap();
        let start = cursor.current().unwrap();
        cursor.next_page().unwrap();
        assert_eq!(cursor.prev_page().unwrap(), start);
    }

    #[test]
    fn new_rejects_wrong_index_len() {
        assert_eq!(
            Cursor::new(library(8), PageIndex::zero(2)).unwrap_err(),
            LibraryError::LenMismatch {
                expected: 8,
                actual: 2
            }
        );
    }

    #[test]
    fn failed_jump_leaves_cursor_in_place() {
        let mut cursor = Cursor::new(library(8), PageIndex::from_u64(5, 8)).unwrap();
        assert!(cursor.jump(PageIndex::zero(2)).is_err());
        assert_eq!(cursor.index(), &PageIndex::from_u64(5, 8));
    }

    #[test]
    fn jump_moves_to_index() {
        let mut cursor = Cursor::new(library(8), PageIndex::zero(8)).unwrap();
        let target = PageIndex::from_u64(9, 8);
        let page = cursor.jump(target.clone()).unwrap();
        assert_eq!(page.index(), &target);
    }
}
