use crate::cipher::{BabelMachine, FeistelBytes};
use crate::config::LibraryConfig;
use crate::error::LibraryError;
use crate::index::PageIndex;
use crate::key::Key;
use crate::page::Page;

/// Maps a [`PageIndex`] to its [`Page`] and back.
///
/// Both directions are pure and stateless: the same index always gives the same page, and
/// every page has exactly one index.
#[derive(Debug, Clone)]
pub struct BabelLibrary {
    machine: BabelMachine,
    page_len: usize,
}

impl BabelLibrary {
    /// # Errors
    ///
    /// Returns [`LibraryError::InvalidPageLen`] unless `page_len` is even and non-zero.
    pub fn new(machine: BabelMachine, page_len: usize) -> Result<Self, LibraryError> {
        if page_len == 0 || !page_len.is_multiple_of(2) {
            Err(LibraryError::InvalidPageLen(page_len))
        } else {
            Ok(BabelLibrary { machine, page_len })
        }
    }

    /// A library shaped by `config`, mapped with `key`. Use this for a private library.
    ///
    /// The key is mixed with `config` (see [`Key::for_config`]), so the same key at a different
    /// shape is a different, unrelated library.
    ///
    /// # Errors
    ///
    /// Returns [`LibraryError::InvalidPageLen`] unless `config.page_len()` is even and
    /// non-zero, or [`LibraryError::Cipher`] if `config.rounds()` is zero.
    pub fn from_config(config: LibraryConfig, key: Key) -> Result<Self, LibraryError> {
        let effective = key.for_config(&config);
        Self::new(
            BabelMachine::new(*effective.as_bytes(), config.rounds())?,
            config.page_len(),
        )
    }

    /// The shared library for `config`, using [`Key::canonical_root`]: everyone who builds the
    /// same configuration sees the same pages. Its machine runs on
    /// [`LibraryConfig::canonical_key`].
    ///
    /// # Errors
    ///
    /// The same as [`from_config`](Self::from_config).
    pub fn canonical(config: LibraryConfig) -> Result<Self, LibraryError> {
        Self::from_config(config, Key::canonical_root())
    }

    pub fn page_len(&self) -> usize {
        self.page_len
    }

    /// The page at `index`.
    ///
    /// # Errors
    ///
    /// Returns [`LibraryError::LenMismatch`] if `index` is not `page_len` bytes long.
    pub fn page_at(&self, index: &PageIndex) -> Result<Page, LibraryError> {
        let mut block = self.block_from(index.as_bytes())?;
        self.machine.forward(&mut block);
        Ok(Page::new(block.into_bytes(), index.clone()))
    }

    /// The index of the page whose contents are `page_bytes`. Inverse of [`page_at`](Self::page_at).
    ///
    /// # Errors
    ///
    /// Returns [`LibraryError::LenMismatch`] if `page_bytes` is not `page_len` bytes long.
    pub fn index_of(&self, page_bytes: &[u8]) -> Result<PageIndex, LibraryError> {
        let mut block = self.block_from(page_bytes)?;
        self.machine.backward(&mut block);
        Ok(PageIndex::from_bytes(block.into_bytes()))
    }

    /// Checks `bytes` is exactly one page long and wraps it as a cipher block.
    fn block_from(&self, bytes: &[u8]) -> Result<FeistelBytes, LibraryError> {
        if bytes.len() != self.page_len {
            return Err(LibraryError::LenMismatch {
                expected: self.page_len,
                actual: bytes.len(),
            });
        }
        Ok(FeistelBytes::new(bytes.to_vec())?)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn library(page_len: usize) -> BabelLibrary {
        BabelLibrary::new(BabelMachine::new([7u8; 32], 4).unwrap(), page_len).unwrap()
    }

    #[test]
    fn rejects_odd_and_zero_page_len() {
        let machine = || BabelMachine::new([0u8; 32], 4).unwrap();
        assert_eq!(
            BabelLibrary::new(machine(), 3).unwrap_err(),
            LibraryError::InvalidPageLen(3)
        );
        assert_eq!(
            BabelLibrary::new(machine(), 0).unwrap_err(),
            LibraryError::InvalidPageLen(0)
        );
    }

    #[test]
    fn same_index_yields_same_page() {
        let lib = library(8);
        let index = PageIndex::from_u64(42, 8);
        assert_eq!(lib.page_at(&index).unwrap(), lib.page_at(&index).unwrap());
    }

    #[test]
    fn page_at_rejects_wrong_index_len() {
        assert_eq!(
            library(8).page_at(&PageIndex::zero(4)).unwrap_err(),
            LibraryError::LenMismatch {
                expected: 8,
                actual: 4
            }
        );
    }

    #[test]
    fn index_of_inverts_page_at() {
        let lib = library(8);
        let index = PageIndex::from_u64(42, 8);
        let page = lib.page_at(&index).unwrap();
        assert_eq!(lib.index_of(page.bytes()).unwrap(), index);
    }

    #[test]
    fn index_of_rejects_wrong_len() {
        assert_eq!(
            library(8).index_of(&[1, 2]).unwrap_err(),
            LibraryError::LenMismatch {
                expected: 8,
                actual: 2
            }
        );
    }

    #[test]
    fn known_pages_are_stable() {
        use crate::encoding::{Encoding, Hex};

        // Recorded from the pre-refactor implementation; the permutation must not change.
        let lib = library(16);
        for (n, expected) in [
            (0u64, "9eff4aeda65315464ce30e87ec43b054"),
            (1, "1230340d572d8aeaf89a9a432728a901"),
            (42, "c5635fe324d1936fb599e2479d106207"),
            (65535, "bca7906b9b60b630d1ae750e29412180"),
        ] {
            let page = lib.page_at(&PageIndex::from_u64(n, 16)).unwrap();
            assert_eq!(Hex::encode(page.bytes()), expected);
        }
    }
}
