/// The shape of a library: how long its pages (and therefore indexes) are, and how many
/// Feistel rounds map one to the other.
///
/// Use a preset ([`SMALL`](Self::SMALL), [`MEDIUM`](Self::MEDIUM), [`LARGE`](Self::LARGE)) or
/// build your own with [`new`](Self::new). Values are checked when a
/// [`BabelLibrary`](crate::BabelLibrary) is built from them, not here.
///
/// # Example
///
/// ```
/// use bitbabel_core::{BabelLibrary, LibraryConfig, PageIndex};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let library = BabelLibrary::canonical(LibraryConfig::SMALL)?;
/// let page = library.page_at(&PageIndex::zero(LibraryConfig::SMALL.page_len()))?;
/// assert_eq!(page.bytes().len(), 16);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LibraryConfig {
    page_len: usize,
    rounds: u8,
}

/// Domain-separation context for [`LibraryConfig::canonical_key`]. Changing it changes every
/// canonical library, so bump the version instead of editing it if the derivation ever changes.
const CANONICAL_KEY_CONTEXT: &str = "everything you will every do is already here";

impl LibraryConfig {
    /// 16 bytes: an index is 32 hex or 24 base64 characters, small enough to copy by hand,
    /// and it fits a `u128`.
    pub const SMALL: Self = Self::new(16, 8);

    /// 3200 bytes: one page of the original Library of Babel (40 lines of 80) when shown
    /// one byte per character.
    pub const MEDIUM: Self = Self::new(3200, 8);

    /// 6400 bytes: 3200 UTF-16 code units, so about 3200 characters through [`Utf16Le`](crate::Utf16Le)
    /// or [`Utf16Be`](crate::Utf16Be).
    pub const LARGE: Self = Self::new(6400, 8);

    /// A custom configuration.
    ///
    /// This only stores the values. [`BabelLibrary::from_config`](crate::BabelLibrary::from_config)
    /// rejects an odd or zero `page_len` and zero `rounds`.
    pub const fn new(page_len: usize, rounds: u8) -> Self {
        LibraryConfig { page_len, rounds }
    }

    /// Bytes per page, and per index.
    pub const fn page_len(&self) -> usize {
        self.page_len
    }

    /// Feistel rounds. Part of the mapping: changing it changes every page.
    pub const fn rounds(&self) -> u8 {
        self.rounds
    }

    /// The shared key for this configuration, so everyone using it sees the same library.
    ///
    /// Derived from the page length and round count, so every distinct configuration is a
    /// distinct universe. It is public by design: it is not a secret. For a private library,
    /// use [`BabelLibrary::from_config`](crate::BabelLibrary::from_config) with your own key.
    pub fn canonical_key(&self) -> [u8; 32] {
        let mut material = [0u8; 9];
        let (len_bytes, rounds_byte) = material.split_at_mut(8);
        len_bytes.copy_from_slice(&(self.page_len as u64).to_le_bytes()); // `to_le_bytes` makes layout identical on every platform
        rounds_byte.copy_from_slice(&[self.rounds]);
        blake3::derive_key(CANONICAL_KEY_CONTEXT, &material)
    }
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self::MEDIUM
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::BabelLibrary;

    #[test]
    fn presets_have_the_agreed_values() {
        assert_eq!(LibraryConfig::SMALL, LibraryConfig::new(16, 8));
        assert_eq!(LibraryConfig::MEDIUM, LibraryConfig::new(3200, 8));
        assert_eq!(LibraryConfig::LARGE, LibraryConfig::new(6400, 8));
        assert_eq!(LibraryConfig::default(), LibraryConfig::MEDIUM);
    }

    #[test]
    fn presets_are_valid() {
        for config in [
            LibraryConfig::SMALL,
            LibraryConfig::MEDIUM,
            LibraryConfig::LARGE,
        ] {
            let library = BabelLibrary::canonical(config).unwrap();
            assert_eq!(library.page_len(), config.page_len());
        }
    }

    #[test]
    fn canonical_key_is_deterministic_and_distinct() {
        let keys = [
            LibraryConfig::SMALL.canonical_key(),
            LibraryConfig::MEDIUM.canonical_key(),
            LibraryConfig::LARGE.canonical_key(),
            LibraryConfig::new(16, 9).canonical_key(),
            LibraryConfig::new(18, 8).canonical_key(),
        ];
        assert_eq!(keys[0], LibraryConfig::SMALL.canonical_key());
        for (i, a) in keys.iter().enumerate() {
            for b in &keys[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
