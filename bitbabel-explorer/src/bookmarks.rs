//! Whether a bookmark file belongs to the library that is open.

use bitbabel_core::LibraryConfig;
use bitbabel_file::{BookmarkKey, BookmarkSettings};

use crate::fingerprint_groups;

/// Why the file called `name`, with settings `file`, cannot be used in a session in the library
/// `session`, as a message for the screen, or `None` if it can. A session is locked to one
/// library, so the file must be for exactly it.
pub fn library_mismatch(
    name: &str,
    file: &BookmarkSettings,
    session: &BookmarkSettings,
) -> Option<String> {
    if file.size() != session.size() {
        return Some(format!(
            "{name} is a {} library, but this session is {}",
            size_name(file.size()),
            size_name(session.size())
        ));
    }
    match (file.key(), session.key()) {
        (BookmarkKey::Canonical, BookmarkKey::Canonical) => None,
        (BookmarkKey::Canonical, BookmarkKey::Custom(_)) => Some(format!(
            "{name} is a canonical library, but this session uses a private key"
        )),
        (BookmarkKey::Custom(fingerprint), BookmarkKey::Canonical) => Some(format!(
            "{name} is for the key with fingerprint {}, but this session is canonical",
            fingerprint_groups(&fingerprint)
        )),
        (BookmarkKey::Custom(theirs), BookmarkKey::Custom(ours)) if theirs == ours => None,
        (BookmarkKey::Custom(theirs), BookmarkKey::Custom(ours)) => Some(format!(
            "{name} is for the key with fingerprint {}, but this session's key has {}",
            fingerprint_groups(&theirs),
            fingerprint_groups(&ours)
        )),
    }
}

fn size_name(size: LibraryConfig) -> &'static str {
    size.name().unwrap_or("custom")
}

#[cfg(test)]
mod test {
    use super::*;

    fn settings(size: LibraryConfig, key: BookmarkKey) -> BookmarkSettings {
        BookmarkSettings::new(size, key).unwrap()
    }

    #[test]
    fn the_same_library_is_accepted() {
        let medium = settings(LibraryConfig::MEDIUM, BookmarkKey::Canonical);
        assert_eq!(library_mismatch("a", &medium, &medium), None);
    }

    #[test]
    fn a_different_size_names_both() {
        let medium = settings(LibraryConfig::MEDIUM, BookmarkKey::Canonical);
        let small = settings(LibraryConfig::SMALL, BookmarkKey::Canonical);
        assert_eq!(
            library_mismatch("a.babelmarks", &medium, &small).unwrap(),
            "a.babelmarks is a medium library, but this session is small"
        );
    }

    #[test]
    fn keys_must_match_and_the_message_names_them() {
        let canonical = settings(LibraryConfig::SMALL, BookmarkKey::Canonical);
        let ours = settings(LibraryConfig::SMALL, BookmarkKey::Custom([1; 8]));
        let theirs = settings(LibraryConfig::SMALL, BookmarkKey::Custom([2; 8]));
        assert_eq!(library_mismatch("f", &ours, &ours), None);
        assert!(
            library_mismatch("f", &canonical, &ours)
                .unwrap()
                .contains("canonical library, but this session uses a private key")
        );
        assert!(
            library_mismatch("f", &ours, &canonical)
                .unwrap()
                .contains("0101-0101-0101-0101, but this session is canonical")
        );
        let message = library_mismatch("f", &theirs, &ours).unwrap();
        assert!(message.contains("0202-0202-0202-0202"));
        assert!(message.contains("0101-0101-0101-0101"));
    }

    #[test]
    fn the_size_is_checked_before_the_key() {
        let file = settings(LibraryConfig::MEDIUM, BookmarkKey::Custom([1; 8]));
        let session = settings(LibraryConfig::SMALL, BookmarkKey::Canonical);
        assert!(
            library_mismatch("f", &file, &session)
                .unwrap()
                .contains("medium library")
        );
    }
}
