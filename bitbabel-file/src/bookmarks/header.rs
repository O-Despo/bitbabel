//! The settings a bookmark file belongs to, and the one-line header that records them.
//!
//! Fields come in a fixed order, separated by single spaces, all lowercase, and the line ends
//! in `\n`. `fingerprint` is there exactly when `key=custom`. Each header has one valid
//! spelling, so parsing it then writing it back gives the same bytes.
//! The key itself is never written, only its fingerprint.

use bitbabel_core::{Encoding, FINGERPRINT_LEN, Hex, LibraryConfig};

use crate::error::BookmarkError;
use crate::grammar::{field_values, lowercase_hex};

/// The first word of every header. The digit is the format version.
const MAGIC: &str = "BABELMARKS1";

/// Every header starts with this, whatever its version.
const MAGIC_PREFIX: &str = "BABELMARKS";

/// The only index format in version 1. It is written so the format can grow later.
const FORMAT: &str = "base64";

/// Which key a bookmark file's library uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BookmarkKey {
    /// The public canonical key for the size.
    Canonical,
    /// A private key, named only by its fingerprint ([`Key::fingerprint`]).
    ///
    /// [`Key::fingerprint`]: bitbabel_core::Key::fingerprint
    Custom([u8; FINGERPRINT_LEN]),
}

impl BookmarkKey {
    /// The fingerprint of a custom key, or `None` for the canonical key.
    pub fn fingerprint(&self) -> Option<&[u8; FINGERPRINT_LEN]> {
        match self {
            BookmarkKey::Canonical => None,
            BookmarkKey::Custom(fingerprint) => Some(fingerprint),
        }
    }

    /// Whether this is a custom key with exactly this fingerprint.
    ///
    /// This is the wrong-key check: the file crate never sees the key, so the caller hashes
    /// its key with `Key::fingerprint` and asks here.
    pub fn matches_fingerprint(&self, fingerprint: &[u8; FINGERPRINT_LEN]) -> bool {
        self.fingerprint() == Some(fingerprint)
    }
}

/// The library a bookmark file belongs to: a preset size and a key.
///
/// # Example
///
/// ```
/// use bitbabel_core::LibraryConfig;
/// use bitbabel_file::{BookmarkKey, BookmarkSettings};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let settings = BookmarkSettings::new(LibraryConfig::SMALL, BookmarkKey::Canonical)?;
/// assert_eq!(
///     settings.header(),
///     "BABELMARKS1 size=small key=canonical format=base64\n"
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BookmarkSettings {
    size: LibraryConfig,
    key: BookmarkKey,
}

impl BookmarkSettings {
    /// # Errors
    ///
    /// Returns [`BookmarkError::NotAPreset`] unless `size` is
    /// [`SMALL`](LibraryConfig::SMALL), [`MEDIUM`](LibraryConfig::MEDIUM) or
    /// [`LARGE`](LibraryConfig::LARGE): the header can only name presets.
    pub fn new(size: LibraryConfig, key: BookmarkKey) -> Result<Self, BookmarkError> {
        if size.name().is_none() {
            return Err(BookmarkError::NotAPreset(size));
        }
        Ok(BookmarkSettings { size, key })
    }

    /// The library shape. Always a preset.
    pub fn size(&self) -> LibraryConfig {
        self.size
    }

    pub fn key(&self) -> BookmarkKey {
        self.key
    }

    /// The header line, ending in `\n`. Always ASCII.
    pub fn header(&self) -> String {
        // `new` only accepts presets, so the size always has a name.
        let size = self.size.name().unwrap_or_default();
        let mut line = format!("{MAGIC} size={size}");
        match self.key {
            BookmarkKey::Canonical => line.push_str(" key=canonical"),
            BookmarkKey::Custom(fingerprint) => {
                line.push_str(" key=custom fingerprint=");
                line.push_str(&Hex::encode(&fingerprint));
            }
        }
        line.push(' ');
        line.push_str("format=");
        line.push_str(FORMAT);
        line.push('\n');
        line
    }

    /// Reads the header at the start of `bytes`. Returns the settings and the bytes after its
    /// `\n`.
    ///
    /// # Errors
    ///
    /// - [`BookmarkError::NotBookmarkFile`] if `bytes` does not start with `BABELMARKS`.
    /// - [`BookmarkError::UnsupportedVersion`] for a version other than `BABELMARKS1`.
    /// - [`BookmarkError::MissingNewline`] if there is no `\n`.
    /// - [`BookmarkError::NotText`] if the line is not UTF-8.
    /// - [`BookmarkError::WindowsLineEndings`] if the line ends in `\r`.
    /// - [`BookmarkError::FieldCount`] for missing or extra fields, including those made by
    ///   doubled or trailing spaces.
    /// - [`BookmarkError::Field`] for a field that is out of order or unknown.
    /// - [`BookmarkError::Value`] for a value that is not one of the lowercase names, or a
    ///   fingerprint that is not 16 lowercase hex digits.
    /// - [`BookmarkError::MissingFingerprint`] or [`BookmarkError::UnexpectedFingerprint`]
    ///   when the fingerprint disagrees with `key=`.
    pub fn from_header(bytes: &[u8]) -> Result<(Self, &[u8]), BookmarkError> {
        if !bytes.starts_with(MAGIC_PREFIX.as_bytes()) {
            return Err(BookmarkError::NotBookmarkFile);
        }
        let newline = bytes
            .iter()
            .position(|&b| b == b'\n')
            .ok_or(BookmarkError::MissingNewline)?;
        let line = std::str::from_utf8(&bytes[..newline]).map_err(|_| BookmarkError::NotText)?;
        if line.ends_with('\r') {
            return Err(BookmarkError::WindowsLineEndings);
        }
        let rest = &bytes[newline + 1..];

        let mut words = line.split(' ');
        let magic = words.next().unwrap_or_default();
        if magic != MAGIC {
            return Err(BookmarkError::UnsupportedVersion(magic.to_string()));
        }

        // 3 fields for a canonical key, 4 with the fingerprint of a custom one.
        let words: Vec<&str> = words.collect();
        let names: &[&'static str] = match words.len() {
            3 => &["size", "key", "format"],
            4 => &["size", "key", "fingerprint", "format"],
            found => return Err(BookmarkError::FieldCount { found }),
        };
        let values = field_values(&words, names).map_err(|wrong| BookmarkError::Field {
            expected: wrong.expected,
            found: wrong.found,
        })?;

        let invalid = |field: &'static str, value: &str| BookmarkError::Value {
            field,
            value: value.to_string(),
        };
        let size = values[0].parse().map_err(|_| invalid("size", values[0]))?;
        let format = values[values.len() - 1];
        if format != FORMAT {
            return Err(invalid("format", format));
        }
        let key = match (values[1], values.get(2).filter(|_| values.len() == 4)) {
            ("canonical", None) => BookmarkKey::Canonical,
            ("canonical", Some(_)) => return Err(BookmarkError::UnexpectedFingerprint),
            ("custom", None) => return Err(BookmarkError::MissingFingerprint),
            ("custom", Some(text)) => BookmarkKey::Custom(
                lowercase_hex(text).ok_or_else(|| invalid("fingerprint", text))?,
            ),
            (other, _) => return Err(invalid("key", other)),
        };
        // `size` parsed as a preset name, so this cannot fail.
        Ok((BookmarkSettings::new(size, key)?, rest))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const FINGERPRINT: [u8; FINGERPRINT_LEN] = [0x3f, 0x9a, 0x1c, 0x07, 0xe2, 0xb8, 0x4d, 0x10];

    const SIZES: [LibraryConfig; 3] = [
        LibraryConfig::SMALL,
        LibraryConfig::MEDIUM,
        LibraryConfig::LARGE,
    ];

    fn parse_err(bytes: &[u8]) -> BookmarkError {
        BookmarkSettings::from_header(bytes).unwrap_err()
    }

    #[test]
    fn writes_the_agreed_header() {
        let canonical = BookmarkSettings::new(LibraryConfig::MEDIUM, BookmarkKey::Canonical);
        assert_eq!(
            canonical.unwrap().header(),
            "BABELMARKS1 size=medium key=canonical format=base64\n"
        );
        let custom = BookmarkSettings::new(LibraryConfig::MEDIUM, BookmarkKey::Custom(FINGERPRINT));
        assert_eq!(
            custom.unwrap().header(),
            "BABELMARKS1 size=medium key=custom fingerprint=3f9a1c07e2b84d10 format=base64\n"
        );
    }

    #[test]
    fn every_header_round_trips_byte_for_byte() {
        for size in SIZES {
            for key in [BookmarkKey::Canonical, BookmarkKey::Custom(FINGERPRINT)] {
                let settings = BookmarkSettings::new(size, key).unwrap();
                let line = settings.header();
                let (parsed, rest) = BookmarkSettings::from_header(line.as_bytes()).unwrap();
                assert_eq!(parsed, settings);
                assert!(rest.is_empty());
                assert_eq!(parsed.header(), line);
            }
        }
    }

    #[test]
    fn the_rest_is_everything_after_the_first_newline() {
        let mut file = b"BABELMARKS1 size=small key=canonical format=base64\n".to_vec();
        file.extend_from_slice(b"1\tlabel\n");
        let (_, rest) = BookmarkSettings::from_header(&file).unwrap();
        assert_eq!(rest, b"1\tlabel\n");
    }

    #[test]
    fn rejects_custom_shapes() {
        let custom = LibraryConfig::new(18, 8);
        assert_eq!(
            BookmarkSettings::new(custom, BookmarkKey::Canonical),
            Err(BookmarkError::NotAPreset(custom))
        );
    }

    #[test]
    fn matches_only_the_same_custom_fingerprint() {
        let mut other = FINGERPRINT;
        other[0] ^= 1;
        let custom = BookmarkKey::Custom(FINGERPRINT);
        assert!(custom.matches_fingerprint(&FINGERPRINT));
        assert!(!custom.matches_fingerprint(&other));
        assert!(!BookmarkKey::Canonical.matches_fingerprint(&FINGERPRINT));
        assert_eq!(custom.fingerprint(), Some(&FINGERPRINT));
        assert_eq!(BookmarkKey::Canonical.fingerprint(), None);
    }

    #[test]
    fn rejects_other_files_and_versions() {
        assert_eq!(parse_err(b"hello\n"), BookmarkError::NotBookmarkFile);
        assert_eq!(parse_err(b""), BookmarkError::NotBookmarkFile);
        // A `.babel` header, and a UTF-8 byte order mark before the magic word.
        assert_eq!(
            parse_err(b"BITBABEL1 size=medium key=canonical format=raw\n"),
            BookmarkError::NotBookmarkFile
        );
        assert_eq!(
            parse_err(b"\xef\xbb\xbfBABELMARKS1 size=medium key=canonical format=base64\n"),
            BookmarkError::NotBookmarkFile
        );
        assert_eq!(
            parse_err(b"BABELMARKS2 size=medium key=canonical format=base64\n"),
            BookmarkError::UnsupportedVersion("BABELMARKS2".to_string())
        );
        assert_eq!(
            parse_err(b"BABELMARKS1 size=medium key=canonical format=base64"),
            BookmarkError::MissingNewline
        );
        assert_eq!(
            parse_err(b"BABELMARKS1 size=medium key=canonical format=base64\xff\n"),
            BookmarkError::NotText
        );
    }

    #[test]
    fn names_windows_line_endings() {
        assert_eq!(
            parse_err(b"BABELMARKS1 size=medium key=canonical format=base64\r\n"),
            BookmarkError::WindowsLineEndings
        );
    }

    #[test]
    fn rejects_wrong_field_counts() {
        for (header, found) in [
            ("BABELMARKS1\n", 0),
            ("BABELMARKS1 size=medium key=canonical\n", 2),
            (
                "BABELMARKS1 size=medium key=custom fingerprint=3f9a1c07e2b84d10 format=base64 x\n",
                5,
            ),
        ] {
            assert_eq!(
                parse_err(header.as_bytes()),
                BookmarkError::FieldCount { found },
                "{header:?}"
            );
        }
    }

    /// Doubled and trailing spaces make empty fields, and those are named by position.
    #[test]
    fn rejects_extra_spaces_and_fields() {
        let field = |expected, found: &str| BookmarkError::Field {
            expected,
            found: found.to_string(),
        };
        assert_eq!(
            parse_err(b"BABELMARKS1 size=medium  key=canonical format=base64\n"),
            field("key", "")
        );
        assert_eq!(
            parse_err(b"BABELMARKS1 size=medium key=canonical format=base64 \n"),
            field("fingerprint", "format=base64")
        );
        assert_eq!(
            parse_err(b"BABELMARKS1 size=medium key=canonical format=base64 extra=1\n"),
            field("fingerprint", "format=base64")
        );
    }

    #[test]
    fn rejects_fields_out_of_order() {
        assert_eq!(
            parse_err(b"BABELMARKS1 key=canonical size=medium format=base64\n"),
            BookmarkError::Field {
                expected: "size",
                found: "key=canonical".to_string()
            }
        );
        // `check` belongs to `.babel` headers.
        assert_eq!(
            parse_err(b"BABELMARKS1 size=medium key=canonical check=1f2e3d4c5b6a7980\n"),
            BookmarkError::Field {
                expected: "format",
                found: "check=1f2e3d4c5b6a7980".to_string()
            }
        );
        assert_eq!(
            parse_err(
                b"BABELMARKS1 size=medium key=custom format=base64 fingerprint=3f9a1c07e2b84d10\n"
            ),
            BookmarkError::Field {
                expected: "fingerprint",
                found: "format=base64".to_string()
            }
        );
    }

    #[test]
    fn rejects_bad_values() {
        let value = |field, value: &str| BookmarkError::Value {
            field,
            value: value.to_string(),
        };
        assert_eq!(
            parse_err(b"BABELMARKS1 size=huge key=canonical format=base64\n"),
            value("size", "huge")
        );
        // Names are lowercase only.
        assert_eq!(
            parse_err(b"BABELMARKS1 size=Medium key=canonical format=base64\n"),
            value("size", "Medium")
        );
        assert_eq!(
            parse_err(b"BABELMARKS1 size=medium key=secret format=base64\n"),
            value("key", "secret")
        );
        assert_eq!(
            parse_err(b"BABELMARKS1 size=medium key=canonical format=hex\n"),
            value("format", "hex")
        );
        for fingerprint in [
            "3F9A1C07E2B84D10",   // uppercase
            "3f9a1c07e2b84d",     // too short
            "3f9a1c07e2b84d1000", // too long
            "3f9a-1c07-e2b8-4d10",
            "",
        ] {
            let header = format!(
                "BABELMARKS1 size=medium key=custom fingerprint={fingerprint} format=base64\n"
            );
            assert_eq!(
                parse_err(header.as_bytes()),
                value("fingerprint", fingerprint),
                "{fingerprint:?}"
            );
        }
    }

    #[test]
    fn the_fingerprint_exists_exactly_when_the_key_is_custom() {
        assert_eq!(
            parse_err(b"BABELMARKS1 size=medium key=custom format=base64\n"),
            BookmarkError::MissingFingerprint
        );
        assert_eq!(
            parse_err(
                b"BABELMARKS1 size=medium key=canonical fingerprint=3f9a1c07e2b84d10 format=base64\n"
            ),
            BookmarkError::UnexpectedFingerprint
        );
    }

    #[test]
    fn error_messages_are_readable() {
        assert_eq!(
            BookmarkError::WindowsLineEndings.to_string(),
            "file has Windows line endings (\\r\\n); save it with Unix line endings"
        );
        assert_eq!(
            BookmarkError::MissingFingerprint.to_string(),
            "header says key=custom but has no fingerprint"
        );
        assert_eq!(
            parse_err(b"BABELMARKS1 size=huge key=canonical format=base64\n").to_string(),
            "invalid value \"huge\" for header field \"size\""
        );
    }

    /// Changing any one byte of a header must give an error or another valid header that
    /// writes back as exactly those bytes: there is only one spelling of each header.
    #[test]
    fn a_changed_header_never_parses_to_different_bytes() {
        let settings =
            BookmarkSettings::new(LibraryConfig::MEDIUM, BookmarkKey::Custom(FINGERPRINT)).unwrap();
        let original = settings.header().into_bytes();
        for at in 0..original.len() - 1 {
            for replacement in [b' ', b'=', b'0', b'A', b'a', b'\r', 0xff] {
                let mut changed = original.clone();
                changed[at] = replacement;
                if let Ok((parsed, rest)) = BookmarkSettings::from_header(&changed) {
                    assert_eq!(parsed.header().as_bytes(), &changed[..], "byte {at}");
                    assert!(rest.is_empty());
                }
            }
        }
    }
}
