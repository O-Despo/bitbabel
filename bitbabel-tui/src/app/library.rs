//! Which library the session is in. It is shown in every mode, because the same index in
//! another library is a different page and a wrong key gives no error.

use bitbabel_core::{Encoding, Hex, Key, LibraryConfig};

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
    let hex: Vec<char> = Hex::encode(&key.fingerprint()).chars().collect();
    let groups: Vec<String> = hex.chunks(4).map(|group| group.iter().collect()).collect();
    groups.join("-")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn label_names_size_and_key_separately() {
        let canonical = LibraryChoice::new(LibraryConfig::MEDIUM, None);
        assert_eq!(canonical.label(), "medium · canonical");
        let custom = LibraryChoice::new(LibraryConfig::SMALL, Some(Key::from_bytes([1; 32])));
        assert_eq!(custom.label(), "small · key a576-7c85-fdef-3c24");
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
