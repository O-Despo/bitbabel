use std::fmt;

use crate::config::LibraryConfig;
use crate::encoding::{Base64, Encoding, Hex};
use crate::error::KeyError;

/// Domain-separation context for [`Key::canonical_root`]. Changing it changes every canonical
/// library, so bump the version instead of editing it if the derivation ever changes.
const CANONICAL_KEY_CONTEXT: &str = "v1 everything you will every do is already here";

/// Domain-separation context for [`Key::for_config`]. Changing it changes every library, so bump
/// the version instead of editing it if the derivation ever changes.
const UNIVERSE_CONTEXT: &str = "v1 bitbabel universe key";

/// Length of a key in bytes: BLAKE3's keyed mode takes exactly 256 bits.
const KEY_LEN: usize = 32;

/// A 256-bit key. Together with a [`LibraryConfig`] it names one library.
///
/// A key is not used directly: [`for_config`](Self::for_config) mixes in the page length and
/// round count first, so the same key never gives related libraries at different shapes.
///
/// `Debug` hides the bytes so a key does not end up in logs or panic messages.
///
/// # Example
///
/// ```
/// use bitbabel_core::{Key, LibraryConfig};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let key = Key::from_hex(&"ab".repeat(32))?;
/// assert_ne!(key.for_config(&LibraryConfig::SMALL), key.for_config(&LibraryConfig::MEDIUM));
/// # Ok(())
/// # }
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct Key([u8; KEY_LEN]);

impl Key {
    /// Wraps 32 bytes as a key.
    pub const fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Key(bytes)
    }

    /// Wraps `bytes` as a key.
    ///
    /// # Errors
    ///
    /// Returns [`KeyError::InvalidLength`] unless `bytes` is exactly 32 bytes long.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, KeyError> {
        let array =
            <[u8; KEY_LEN]>::try_from(bytes).map_err(|_| KeyError::InvalidLength(bytes.len()))?;
        Ok(Key(array))
    }

    /// Parses a key from 64 hex digits.
    ///
    /// # Errors
    ///
    /// Returns [`KeyError::Hex`] for a malformed string, or [`KeyError::InvalidLength`] if it
    /// does not decode to 32 bytes.
    pub fn from_hex(text: &str) -> Result<Self, KeyError> {
        Self::from_slice(&Hex::decode(&text.to_string())?)
    }

    /// Parses a key from standard padded base64.
    ///
    /// # Errors
    ///
    /// Returns [`KeyError::Base64`] for a malformed string, or [`KeyError::InvalidLength`] if
    /// it does not decode to 32 bytes.
    pub fn from_base64(text: &str) -> Result<Self, KeyError> {
        Self::from_slice(&Base64::decode(&text.to_string())?)
    }

    pub const fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// The fixed, public root key of the canonical libraries. It is not a secret: it exists so
    /// everyone who builds the same configuration sees the same pages.
    pub fn canonical_root() -> Self {
        Key(blake3::derive_key(CANONICAL_KEY_CONTEXT, b""))
    }

    /// The key that actually drives a library of shape `config`: this key mixed with the page
    /// length and round count.
    ///
    /// Without this, one key at rounds 8 and rounds 9 would give related libraries (the
    /// 9-round mapping is the 8-round one plus a further round). The material is fixed-width,
    /// so no two (key, shape) pairs can produce the same input bytes.
    pub fn for_config(&self, config: &LibraryConfig) -> Key {
        let mut material = [0u8; KEY_LEN + 8 + 1];
        let (key_bytes, shape) = material.split_at_mut(KEY_LEN);
        let (len_bytes, rounds_byte) = shape.split_at_mut(8);
        key_bytes.copy_from_slice(&self.0);
        len_bytes.copy_from_slice(&(config.page_len() as u64).to_le_bytes()); // `to_le_bytes` makes layout identical on every platform
        rounds_byte.copy_from_slice(&[config.rounds()]);
        Key(blake3::derive_key(UNIVERSE_CONTEXT, &material))
    }
}

// Hand-written so the key never ends up in logs or panic messages.
impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Key").field(&"<redacted>").finish()
    }
}

impl From<[u8; KEY_LEN]> for Key {
    fn from(bytes: [u8; KEY_LEN]) -> Self {
        Key(bytes)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{BabelLibrary, PageIndex};

    #[test]
    fn from_slice_requires_exactly_32_bytes() {
        assert!(Key::from_slice(&[1u8; 32]).is_ok());
        assert_eq!(
            Key::from_slice(&[1u8; 31]).unwrap_err(),
            KeyError::InvalidLength(31)
        );
        assert_eq!(
            Key::from_slice(&[1u8; 33]).unwrap_err(),
            KeyError::InvalidLength(33)
        );
    }

    #[test]
    fn hex_and_base64_round_trip() {
        let key = Key::from_bytes([0xa5; 32]);
        assert_eq!(Key::from_hex(&Hex::encode(key.as_bytes())).unwrap(), key);
        assert_eq!(
            Key::from_base64(&Base64::encode(key.as_bytes())).unwrap(),
            key
        );
    }

    #[test]
    fn hex_and_base64_reject_bad_input() {
        assert_eq!(
            Key::from_hex("abcd").unwrap_err(),
            KeyError::InvalidLength(2)
        );
        assert!(matches!(
            Key::from_hex(&"zz".repeat(32)),
            Err(KeyError::Hex(_))
        ));
        assert!(matches!(
            Key::from_base64("not base64!!"),
            Err(KeyError::Base64(_))
        ));
    }

    #[test]
    fn debug_redacts_key() {
        let shown = format!("{:?}", Key::from_bytes([0x77; 32]));
        assert!(shown.contains("redacted"));
        assert!(!shown.contains("77"));
    }

    #[test]
    fn for_config_is_deterministic() {
        let key = Key::from_bytes([7; 32]);
        assert_eq!(
            key.for_config(&LibraryConfig::SMALL),
            key.for_config(&LibraryConfig::SMALL)
        );
    }

    #[test]
    fn for_config_separates_shapes_and_keys() {
        let key = Key::from_bytes([7; 32]);
        let other = Key::from_bytes([8; 32]);
        let base = key.for_config(&LibraryConfig::new(16, 8));

        assert_ne!(base, key.for_config(&LibraryConfig::new(16, 9)));
        assert_ne!(base, key.for_config(&LibraryConfig::new(18, 8)));
        assert_ne!(base, other.for_config(&LibraryConfig::new(16, 8)));
        assert_ne!(base, key); // never the raw key
    }

    #[test]
    fn same_key_at_different_rounds_gives_different_pages() {
        let key = Key::from_bytes([7; 32]);
        let index = PageIndex::zero(16);
        let page = |rounds| {
            BabelLibrary::from_config(LibraryConfig::new(16, rounds), key.clone())
                .unwrap()
                .page_at(&index)
                .unwrap()
        };
        assert_ne!(page(8).bytes(), page(9).bytes());
    }

    #[test]
    fn canonical_root_is_stable() {
        assert_eq!(Key::canonical_root(), Key::canonical_root());
    }
}
