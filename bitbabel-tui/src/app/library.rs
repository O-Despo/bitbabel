//! Which library the session is in. It is shown in every mode, because the same index in
//! another library is a different page and a wrong key gives no error.

use bitbabel_core::{Key, LibraryConfig};

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
    // Read by the explore screen once it builds a cursor.
    #[allow(dead_code)]
    pub fn size(&self) -> LibraryConfig {
        self.size
    }

    /// The private key, or `None` for the canonical library.
    #[allow(dead_code)]
    pub fn key(&self) -> Option<&Key> {
        self.key.as_ref()
    }

    /// The indicator text: `medium · canonical`. The key is never shown.
    pub fn label(&self) -> String {
        let size = self.size.name().unwrap_or("custom size");
        let key = match self.key {
            Some(_) => "custom key",
            None => "canonical",
        };
        format!("{size} · {key}")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn label_names_size_and_key_separately() {
        let canonical = LibraryChoice::new(LibraryConfig::MEDIUM, None);
        assert_eq!(canonical.label(), "medium · canonical");
        let custom = LibraryChoice::new(LibraryConfig::SMALL, Some(Key::from_bytes([1; 32])));
        assert_eq!(custom.label(), "small · custom key");
    }

    #[test]
    fn label_never_shows_the_key() {
        let custom = LibraryChoice::new(LibraryConfig::SMALL, Some(Key::from_bytes([0x77; 32])));
        assert!(!custom.label().contains("77"));
    }
}
