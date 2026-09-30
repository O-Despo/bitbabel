//! The keyed checksum a file carries over its original data.
//!
//! It is keyed from the library's effective key, so it catches a wrong key and corrupted
//! indices without letting anyone confirm a guess at a private file's contents. Under the
//! canonical key it is effectively public, which is harmless: anyone can decode those files.

use bitbabel_core::{Key, LibraryConfig};

/// Domain-separation context for the checksum key. Changing it changes every file's checksum,
/// so bump the version instead of editing it.
const CHECK_CONTEXT: &str = "v1 bitbabel-file check";

/// Bytes of checksum kept: enough to catch accidents, short enough for a header.
pub(crate) const CHECK_LEN: usize = 8;

/// The checksum of `data` in the library of shape `size`, keyed by `key` or, for `None`, the
/// canonical key.
pub(crate) fn checksum(data: &[u8], size: LibraryConfig, key: Option<&Key>) -> [u8; CHECK_LEN] {
    let effective = match key {
        Some(key) => key.for_config(&size),
        None => size.canonical_key(),
    };
    let check_key = blake3::derive_key(CHECK_CONTEXT, effective.as_bytes());
    let hash = blake3::keyed_hash(&check_key, data);

    let mut check = [0u8; CHECK_LEN];
    check.copy_from_slice(&hash.as_bytes()[..CHECK_LEN]);
    check
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn depends_on_data_key_and_size() {
        let small = LibraryConfig::SMALL;
        let key = Key::from_bytes([1; 32]);
        let base = checksum(b"hello", small, None);

        assert_eq!(base, checksum(b"hello", small, None));
        assert_ne!(base, checksum(b"hellp", small, None));
        assert_ne!(base, checksum(b"hello", LibraryConfig::MEDIUM, None));
        assert_ne!(base, checksum(b"hello", small, Some(&key)));
        assert_ne!(
            checksum(b"hello", small, Some(&key)),
            checksum(b"hello", small, Some(&Key::from_bytes([2; 32])))
        );
    }

    #[test]
    fn canonical_root_as_a_custom_key_matches_canonical() {
        // Both mean the same library, so they must give the same checksum.
        assert_eq!(
            checksum(b"hello", LibraryConfig::SMALL, Some(&Key::canonical_root())),
            checksum(b"hello", LibraryConfig::SMALL, None)
        );
    }
}
