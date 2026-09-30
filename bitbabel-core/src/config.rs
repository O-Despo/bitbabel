use std::str::FromStr;

use crate::error::ConfigNameError;
use crate::key::Key;

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
    /// The public canonical root mixed with the page length and round count, so every distinct
    /// configuration is a distinct universe. It is public by design: it is not a secret. For a
    /// private library, use [`BabelLibrary::from_config`](crate::BabelLibrary::from_config)
    /// with your own key.
    pub fn canonical_key(&self) -> Key {
        Key::canonical_root().for_config(self)
    }
}

/// Every preset and its name. The names are part of the future file header, so they are
/// lowercase and fixed.
const PRESETS: [(&str, LibraryConfig); 3] = [
    ("small", LibraryConfig::SMALL),
    ("medium", LibraryConfig::MEDIUM),
    ("large", LibraryConfig::LARGE),
];

impl LibraryConfig {
    /// The preset's name (`"small"`, `"medium"` or `"large"`), or `None` for a custom shape.
    pub fn name(&self) -> Option<&'static str> {
        PRESETS
            .iter()
            .find(|(_, preset)| preset == self)
            .map(|(name, _)| *name)
    }
}

/// Parses a preset name. Only the exact lowercase names are accepted, so each preset has one
/// spelling.
///
/// ```
/// use bitbabel_core::LibraryConfig;
///
/// assert_eq!("medium".parse(), Ok(LibraryConfig::MEDIUM));
/// assert!("Medium".parse::<LibraryConfig>().is_err());
/// ```
impl FromStr for LibraryConfig {
    type Err = ConfigNameError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        PRESETS
            .iter()
            .find(|(preset_name, _)| *preset_name == name)
            .map(|(_, preset)| *preset)
            .ok_or_else(|| ConfigNameError(name.to_string()))
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
    fn preset_names_round_trip() {
        for (name, config) in PRESETS {
            assert_eq!(config.name(), Some(name));
            assert_eq!(name.parse(), Ok(config));
        }
    }

    #[test]
    fn custom_shapes_have_no_name() {
        assert_eq!(LibraryConfig::new(18, 8).name(), None);
        assert_eq!(LibraryConfig::new(16, 9).name(), None);
    }

    #[test]
    fn unknown_or_miscased_names_are_rejected() {
        for bad in ["huge", "Medium", "SMALL", " small", ""] {
            assert_eq!(
                bad.parse::<LibraryConfig>(),
                Err(ConfigNameError(bad.to_string()))
            );
        }
        assert_eq!(
            "huge".parse::<LibraryConfig>().unwrap_err().to_string(),
            "unknown size \"huge\", expected small, medium or large"
        );
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
