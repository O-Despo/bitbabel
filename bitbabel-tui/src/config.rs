//! What the CLI hands to [`run`](crate::run).

use std::fmt;

use bitbabel_core::{Key, LibraryConfig};

/// The starting choices. Anything left as `None` is asked for on the setup screen, except that
/// a size with no key means the canonical library, like `encode` and `decode`.
#[derive(Clone, Default)]
pub struct TuiConfig {
    /// The library size, one of the presets.
    pub size: Option<LibraryConfig>,
    /// The private key, or `None` for the canonical library.
    pub key: Option<Key>,
}

// Hand-written to keep the `Key` out of the output (it prints as `<redacted>` anyway).
impl fmt::Debug for TuiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TuiConfig")
            .field("size", &self.size)
            .field("key", &self.key)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn default_asks_for_everything() {
        let config = TuiConfig::default();
        assert!(config.size.is_none());
        assert!(config.key.is_none());
    }

    #[test]
    fn debug_never_shows_the_key() {
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: Some(Key::from_bytes([0x77; 32])),
        };
        let shown = format!("{config:?}");
        assert!(shown.contains("redacted"));
        assert!(!shown.contains("77"));
    }
}
