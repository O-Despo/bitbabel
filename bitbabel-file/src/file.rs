//! A whole file: its settings and the page indices that hold its data.
//!
//! ```text
//! encode:  data → pad → split into pages → index_of each → BabelFile → to_bytes
//! decode:  from_bytes → BabelFile → page_at each → join → unpad → data
//! ```

use bitbabel_core::{BabelLibrary, Key, PageIndex};

use crate::error::FileError;
use crate::header::{KeyMode, Settings};
use crate::pad::{pad, unpad};

/// Data stored as a list of page indices in one library, plus the settings that name it.
///
/// Built only by [`encode`](Self::encode) or [`from_bytes`](Self::from_bytes), so there is
/// always at least one index and every index is one page long.
///
/// # Example
///
/// ```
/// use bitbabel_file::{BabelFile, Settings};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let file = BabelFile::encode(b"hello", Settings::default(), None)?;
/// let bytes = file.to_bytes()?;
/// assert!(bytes.starts_with(b"BITBABEL1 size=medium key=canonical format=raw\n"));
///
/// let read = BabelFile::from_bytes(&bytes)?;
/// assert_eq!(read.decode(None)?, b"hello");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BabelFile {
    settings: Settings,
    indices: Vec<PageIndex>,
}

impl BabelFile {
    /// Pads `data`, splits it into pages and finds each page's index.
    ///
    /// `key` must be `None` for [`KeyMode::Canonical`] and `Some` for [`KeyMode::Custom`].
    ///
    /// # Errors
    ///
    /// Returns [`FileError::UnexpectedKey`] or [`FileError::MissingKey`] if `key` does not
    /// match the key mode.
    pub fn encode(data: &[u8], settings: Settings, key: Option<&Key>) -> Result<Self, FileError> {
        let library = library_for(settings, key)?;
        let indices = pad(data, library.page_len())?
            .chunks(library.page_len())
            .map(|page| library.index_of(page))
            .collect::<Result<_, _>>()?;
        Ok(BabelFile { settings, indices })
    }

    /// The original data: each index's page, joined and unpadded. Inverse of
    /// [`encode`](Self::encode).
    ///
    /// # Errors
    ///
    /// Returns [`FileError::UnexpectedKey`] or [`FileError::MissingKey`] if `key` does not
    /// match the key mode, or [`FileError::Pad`] if the joined pages are not padded. With a
    /// custom key, a wrong key usually gives [`FileError::Pad`] but can give wrong data.
    pub fn decode(&self, key: Option<&Key>) -> Result<Vec<u8>, FileError> {
        let library = library_for(self.settings, key)?;
        let mut padded = Vec::with_capacity(self.indices.len() * library.page_len());
        for index in &self.indices {
            padded.extend_from_slice(library.page_at(index)?.bytes());
        }
        Ok(unpad(&padded, library.page_len())?)
    }

    /// The file as written to disk: the header line, then the indices in the settings' format.
    ///
    /// # Errors
    ///
    /// Returns [`FileError::Index`] if the index list cannot be written. A `BabelFile` always
    /// holds a valid list, so this is not expected.
    pub fn to_bytes(&self) -> Result<Vec<u8>, FileError> {
        let mut bytes = self.settings.header().into_bytes();
        bytes.extend(
            self.settings
                .format()
                .encode(&self.indices, self.settings.size().page_len())?,
        );
        Ok(bytes)
    }

    /// Reads a file written by [`to_bytes`](Self::to_bytes). No key is needed.
    ///
    /// # Errors
    ///
    /// Returns [`FileError::Header`] if the header line is invalid, or [`FileError::Index`] if
    /// the payload is not a valid index list for the header's size and format.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FileError> {
        let (settings, payload) = Settings::parse_header(bytes)?;
        let indices = settings
            .format()
            .decode(payload, settings.size().page_len())?;
        Ok(BabelFile { settings, indices })
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    /// One index per page, in order. Never empty.
    pub fn indices(&self) -> &[PageIndex] {
        &self.indices
    }
}

/// The library `settings` names, checking `key` matches the key mode.
fn library_for(settings: Settings, key: Option<&Key>) -> Result<BabelLibrary, FileError> {
    let library = match (settings.key_mode(), key) {
        (KeyMode::Canonical, None) => BabelLibrary::canonical(settings.size())?,
        (KeyMode::Custom, Some(key)) => BabelLibrary::from_config(settings.size(), key.clone())?,
        (KeyMode::Canonical, Some(_)) => return Err(FileError::UnexpectedKey),
        (KeyMode::Custom, None) => return Err(FileError::MissingKey),
    };
    Ok(library)
}

#[cfg(test)]
mod test {
    use bitbabel_core::LibraryConfig;

    use super::*;
    use crate::error::HeaderError;
    use crate::format::IndexFormat;

    fn small(key_mode: KeyMode, format: IndexFormat) -> Settings {
        Settings::new(LibraryConfig::SMALL, key_mode, format).unwrap()
    }

    fn key(byte: u8) -> Key {
        Key::from_bytes([byte; 32])
    }

    #[test]
    fn one_page_holds_the_canonical_index_of_the_padded_data() {
        let file =
            BabelFile::encode(b"hello", small(KeyMode::Canonical, IndexFormat::Hex), None).unwrap();
        let library = BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
        let expected = library.index_of(&pad(b"hello", 16).unwrap()).unwrap();
        assert_eq!(file.indices(), [expected]);
    }

    #[test]
    fn page_count_follows_padding() {
        let settings = small(KeyMode::Canonical, IndexFormat::Raw);
        for (len, pages) in [(0, 1), (15, 1), (16, 2), (17, 2), (32, 3)] {
            let file = BabelFile::encode(&vec![7; len], settings, None).unwrap();
            assert_eq!(file.indices().len(), pages, "{len} bytes");
        }
    }

    #[test]
    fn key_must_match_the_key_mode() {
        let canonical = small(KeyMode::Canonical, IndexFormat::Raw);
        let custom = small(KeyMode::Custom, IndexFormat::Raw);
        assert_eq!(
            BabelFile::encode(b"hi", canonical, Some(&key(1))),
            Err(FileError::UnexpectedKey)
        );
        assert_eq!(
            BabelFile::encode(b"hi", custom, None),
            Err(FileError::MissingKey)
        );

        let file = BabelFile::encode(b"hi", custom, Some(&key(1))).unwrap();
        assert_eq!(file.decode(None), Err(FileError::MissingKey));
    }

    #[test]
    fn custom_keys_give_different_indices() {
        let custom = small(KeyMode::Custom, IndexFormat::Raw);
        let a = BabelFile::encode(b"hi", custom, Some(&key(1))).unwrap();
        let b = BabelFile::encode(b"hi", custom, Some(&key(2))).unwrap();
        assert_ne!(a.indices(), b.indices());
    }

    #[test]
    fn wrong_key_does_not_give_the_data_back() {
        let custom = small(KeyMode::Custom, IndexFormat::Raw);
        let file = BabelFile::encode(b"secret", custom, Some(&key(1))).unwrap();
        assert_ne!(file.decode(Some(&key(2))), Ok(b"secret".to_vec()));
    }

    #[test]
    fn from_bytes_reports_header_and_index_errors() {
        assert_eq!(
            BabelFile::from_bytes(b"hello"),
            Err(FileError::Header(HeaderError::NotBabelFile))
        );
        assert!(matches!(
            BabelFile::from_bytes(b"BITBABEL1 size=small key=canonical format=raw\n\x01\x02"),
            Err(FileError::Index(_))
        ));
    }

    #[test]
    fn decode_rejects_indices_whose_pages_are_not_padded() {
        // Index 0 of the canonical small library is a random-looking page, not padding.
        let mut bytes = small(KeyMode::Canonical, IndexFormat::Raw)
            .header()
            .into_bytes();
        bytes.extend([0u8; 16]);
        let file = BabelFile::from_bytes(&bytes).unwrap();
        assert!(matches!(file.decode(None), Err(FileError::Pad(_))));
    }

    #[test]
    fn error_messages_include_the_inner_message() {
        let err = BabelFile::from_bytes(b"hello").unwrap_err();
        assert_eq!(err.to_string(), "invalid header: not a bitbabel file");
    }
}
