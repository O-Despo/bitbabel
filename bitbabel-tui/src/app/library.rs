//! Which library the session is in. It is shown in every mode, because the same index in
//! another library is a different page and a wrong key gives no error.

use bitbabel_core::{
    BabelLibrary, Encoding, FINGERPRINT_LEN, Hex, Key, LibraryConfig, LibraryError,
};
use bitbabel_file::{BookmarkError, BookmarkKey, BookmarkSettings};

/// A size and a key: the pair that names one library.
#[derive(Debug, Clone)]
pub struct LibraryChoice {
    size: LibraryConfig,
    /// `None` is the canonical library.
    key: Option<Key>,
}

impl LibraryChoice {
    /// The library of `size`, canonical when `key` is `None`.
    pub fn new(size: LibraryConfig, key: Option<Key>) -> Self {
        LibraryChoice { size, key }
    }

    /// The library shape.
    pub fn size(&self) -> LibraryConfig {
        self.size
    }

    /// The private key, or `None` for the canonical library.
    #[cfg(test)]
    pub fn key(&self) -> Option<&Key> {
        self.key.as_ref()
    }

    /// Whether this is the canonical library: everyone sees its pages, so a bookmark label
    /// there names no secret (§13.3a, review finding 4).
    pub fn is_canonical(&self) -> bool {
        self.key.is_none()
    }

    /// The bookmark file settings that name this library: its size, and `Canonical` or the
    /// private key's fingerprint.
    ///
    /// # Errors
    ///
    /// [`BookmarkError::NotAPreset`] if the size is not a preset. A bookmark file header can
    /// only name the presets.
    pub fn bookmark_settings(&self) -> Result<BookmarkSettings, BookmarkError> {
        let key = match &self.key {
            Some(key) => BookmarkKey::Custom(key.fingerprint()),
            None => BookmarkKey::Canonical,
        };
        BookmarkSettings::new(self.size, key)
    }

    /// The library itself, ready to map indexes to pages.
    ///
    /// # Errors
    ///
    /// [`LibraryError`] if the size is not a valid shape. The presets always are.
    pub fn open(&self) -> Result<BabelLibrary, LibraryError> {
        let key = self.key.clone().unwrap_or_else(Key::canonical_root);
        BabelLibrary::from_config(self.size, key)
    }

    /// The indicator text: `medium · canonical`, or `medium · key 3f9a-1c07-e2b8-4d10`. The
    /// key itself is never shown, only its fingerprint.
    pub fn label(&self) -> String {
        let size = self.size.name().unwrap_or("custom size");
        match &self.key {
            Some(key) => format!("{size} · key {}", fingerprint_text(key)),
            None => format!("{size} · canonical"),
        }
    }
}

/// A key's fingerprint as 16 hex characters in groups of four: `3f9a-1c07-e2b8-4d10`.
pub fn fingerprint_text(key: &Key) -> String {
    fingerprint_groups(&key.fingerprint())
}

/// Fingerprint bytes as 16 hex characters in groups of four. A bookmark file holds these bytes
/// without the key they came from.
pub fn fingerprint_groups(fingerprint: &[u8; FINGERPRINT_LEN]) -> String {
    let hex: Vec<char> = Hex::encode(fingerprint).chars().collect();
    let groups: Vec<String> = hex.chunks(4).map(|group| group.iter().collect()).collect();
    groups.join("-")
}

#[cfg(test)]
mod test {
    use bitbabel_core::PageIndex;

    use super::*;

    #[test]
    fn label_names_size_and_key_separately() {
        let canonical = LibraryChoice::new(LibraryConfig::MEDIUM, None);
        assert_eq!(canonical.label(), "medium · canonical");
        let custom = LibraryChoice::new(LibraryConfig::SMALL, Some(Key::from_bytes([1; 32])));
        assert_eq!(custom.label(), "small · key a576-7c85-fdef-3c24");
    }

    #[test]
    fn open_gives_the_canonical_library_without_a_key() {
        let opened = LibraryChoice::new(LibraryConfig::SMALL, None)
            .open()
            .unwrap();
        let expected = BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
        let index = PageIndex::zero(16);
        assert_eq!(
            opened.page_at(&index).unwrap(),
            expected.page_at(&index).unwrap()
        );
    }

    #[test]
    fn open_with_a_key_is_a_different_library() {
        let key = Key::from_bytes([1; 32]);
        let private = LibraryChoice::new(LibraryConfig::SMALL, Some(key))
            .open()
            .unwrap();
        let canonical = BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
        let index = PageIndex::zero(16);
        assert_ne!(
            private.page_at(&index).unwrap(),
            canonical.page_at(&index).unwrap()
        );
    }

    #[test]
    fn label_never_shows_the_key() {
        let key = Key::from_bytes([0x77; 32]);
        let label = LibraryChoice::new(LibraryConfig::SMALL, Some(key)).label();
        assert!(!label.contains("7777"));
    }

    #[test]
    fn fingerprint_is_four_groups_of_four() {
        let text = fingerprint_text(&Key::from_bytes([1; 32]));
        assert_eq!(text, "a576-7c85-fdef-3c24");
    }
}
