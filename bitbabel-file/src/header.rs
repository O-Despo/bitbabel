//! The settings a file is written with, and the one-line header that records them.
//!
//! ```text
//! BITBABEL1 size=medium key=canonical format=raw\n
//! ```
//!
//! Fields come in a fixed order, separated by single spaces, all lowercase, and the line ends
//! in `\n`. Each value has one spelling, so each [`Settings`] has exactly one header and
//! parsing a header then writing it back gives the same bytes. The key itself is never
//! written: `key=custom` only says the reader must supply one.

use bitbabel_core::LibraryConfig;

use crate::error::HeaderError;
use crate::format::IndexFormat;

/// The first word of every header. The digit is the format version.
const MAGIC: &str = "BITBABEL1";

/// Every header starts with this, whatever its version.
const MAGIC_PREFIX: &str = "BITBABEL";

/// Field names, in the order they must appear.
const FIELDS: [&str; 3] = ["size", "key", "format"];

/// Which key a file's library uses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum KeyMode {
    /// The public canonical key for the size, so anyone can decode the file.
    #[default]
    Canonical,
    /// A private key the reader must supply. It is never stored in the file.
    Custom,
}

impl KeyMode {
    fn name(self) -> &'static str {
        match self {
            KeyMode::Canonical => "canonical",
            KeyMode::Custom => "custom",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        [KeyMode::Canonical, KeyMode::Custom]
            .into_iter()
            .find(|mode| mode.name() == name)
    }
}

impl IndexFormat {
    fn name(self) -> &'static str {
        match self {
            IndexFormat::Raw => "raw",
            IndexFormat::Hex => "hex",
            IndexFormat::Base64 => "base64",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        [IndexFormat::Raw, IndexFormat::Hex, IndexFormat::Base64]
            .into_iter()
            .find(|format| format.name() == name)
    }
}

/// How a file is written: the library size, the key mode and the index-list format.
///
/// The default is a medium, canonical, raw file.
///
/// # Example
///
/// ```
/// use bitbabel_core::LibraryConfig;
/// use bitbabel_file::{IndexFormat, KeyMode, Settings};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let settings = Settings::new(LibraryConfig::SMALL, KeyMode::Custom, IndexFormat::Hex)?;
/// assert_eq!(settings.header(), "BITBABEL1 size=small key=custom format=hex\n");
///
/// let file = b"BITBABEL1 size=small key=custom format=hex\n0001\n";
/// let (parsed, payload) = Settings::parse_header(file)?;
/// assert_eq!(parsed, settings);
/// assert_eq!(payload, b"0001\n");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Settings {
    size: LibraryConfig,
    key_mode: KeyMode,
    format: IndexFormat,
}

impl Settings {
    /// # Errors
    ///
    /// Returns [`HeaderError::NotAPreset`] unless `size` is
    /// [`SMALL`](LibraryConfig::SMALL), [`MEDIUM`](LibraryConfig::MEDIUM) or
    /// [`LARGE`](LibraryConfig::LARGE): the header can only name presets.
    pub fn new(
        size: LibraryConfig,
        key_mode: KeyMode,
        format: IndexFormat,
    ) -> Result<Self, HeaderError> {
        if size.name().is_none() {
            return Err(HeaderError::NotAPreset(size));
        }
        Ok(Settings {
            size,
            key_mode,
            format,
        })
    }

    /// The library shape. Always a preset.
    pub fn size(&self) -> LibraryConfig {
        self.size
    }

    pub fn key_mode(&self) -> KeyMode {
        self.key_mode
    }

    pub fn format(&self) -> IndexFormat {
        self.format
    }

    /// The header line, ending in `\n`. Always ASCII.
    pub fn header(&self) -> String {
        // `new` only accepts presets, so the size always has a name.
        let size = self.size.name().unwrap_or_default();
        format!(
            "{MAGIC} size={size} key={} format={}\n",
            self.key_mode.name(),
            self.format.name()
        )
    }

    /// Reads the header at the start of `bytes`. Returns the settings and the bytes after the
    /// header's `\n`, which are the payload.
    ///
    /// # Errors
    ///
    /// - [`HeaderError::NotBabelFile`] if `bytes` does not start with `BITBABEL`.
    /// - [`HeaderError::UnsupportedVersion`] for a version other than `BITBABEL1`.
    /// - [`HeaderError::MissingNewline`] if there is no `\n`.
    /// - [`HeaderError::NotText`] if the line is not UTF-8.
    /// - [`HeaderError::FieldCount`] for missing or extra fields, including those made by
    ///   doubled or trailing spaces.
    /// - [`HeaderError::Field`] for a field that is out of order or unknown.
    /// - [`HeaderError::Value`] for a value that is not one of the lowercase names.
    pub fn parse_header(bytes: &[u8]) -> Result<(Self, &[u8]), HeaderError> {
        if !bytes.starts_with(MAGIC_PREFIX.as_bytes()) {
            return Err(HeaderError::NotBabelFile);
        }
        let newline = bytes
            .iter()
            .position(|&b| b == b'\n')
            .ok_or(HeaderError::MissingNewline)?;
        let line = std::str::from_utf8(&bytes[..newline]).map_err(|_| HeaderError::NotText)?;
        let payload = &bytes[newline + 1..];

        let mut words = line.split(' ');
        let magic = words.next().unwrap_or_default();
        if magic != MAGIC {
            return Err(HeaderError::UnsupportedVersion(magic.to_string()));
        }

        let words: Vec<&str> = words.collect();
        if words.len() != FIELDS.len() {
            return Err(HeaderError::FieldCount {
                expected: FIELDS.len(),
                found: words.len(),
            });
        }
        let mut values = [""; FIELDS.len()];
        for ((value, word), expected) in values.iter_mut().zip(&words).zip(FIELDS) {
            *value = word
                .strip_prefix(expected)
                .and_then(|rest| rest.strip_prefix('='))
                .ok_or_else(|| HeaderError::Field {
                    expected,
                    found: word.to_string(),
                })?;
        }
        let [size, key_mode, format] = values;

        let invalid = |field: &'static str, value: &str| HeaderError::Value {
            field,
            value: value.to_string(),
        };
        let settings = Settings {
            // Only exact lowercase preset names parse, so this is always a preset.
            size: size.parse().map_err(|_| invalid("size", size))?,
            key_mode: KeyMode::from_name(key_mode).ok_or_else(|| invalid("key", key_mode))?,
            format: IndexFormat::from_name(format).ok_or_else(|| invalid("format", format))?,
        };
        Ok((settings, payload))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn all_settings() -> Vec<Settings> {
        let mut all = Vec::new();
        for size in [
            LibraryConfig::SMALL,
            LibraryConfig::MEDIUM,
            LibraryConfig::LARGE,
        ] {
            for key_mode in [KeyMode::Canonical, KeyMode::Custom] {
                for format in [IndexFormat::Raw, IndexFormat::Hex, IndexFormat::Base64] {
                    all.push(Settings::new(size, key_mode, format).unwrap());
                }
            }
        }
        all
    }

    fn parse_err(line: &[u8]) -> HeaderError {
        Settings::parse_header(line).unwrap_err()
    }

    #[test]
    fn default_writes_the_agreed_header() {
        assert_eq!(
            Settings::default().header(),
            "BITBABEL1 size=medium key=canonical format=raw\n"
        );
    }

    #[test]
    fn every_setting_round_trips_byte_for_byte() {
        for settings in all_settings() {
            let header = settings.header();
            let (parsed, payload) = Settings::parse_header(header.as_bytes()).unwrap();
            assert_eq!(parsed, settings);
            assert!(payload.is_empty());
            assert_eq!(parsed.header(), header);
        }
    }

    #[test]
    fn payload_is_everything_after_the_first_newline() {
        let mut file = Settings::default().header().into_bytes();
        file.extend_from_slice(b"\x00\n\xff\n");
        let (_, payload) = Settings::parse_header(&file).unwrap();
        assert_eq!(payload, b"\x00\n\xff\n");
    }

    #[test]
    fn rejects_custom_shapes() {
        let custom = LibraryConfig::new(18, 8);
        assert_eq!(
            Settings::new(custom, KeyMode::Canonical, IndexFormat::Raw),
            Err(HeaderError::NotAPreset(custom))
        );
    }

    #[test]
    fn rejects_other_files_and_versions() {
        assert_eq!(parse_err(b"hello\n"), HeaderError::NotBabelFile);
        assert_eq!(parse_err(b""), HeaderError::NotBabelFile);
        assert_eq!(
            parse_err(b"BITBABEL2 size=medium key=canonical format=raw\n"),
            HeaderError::UnsupportedVersion("BITBABEL2".to_string())
        );
        assert_eq!(
            parse_err(b"BITBABEL1 size=medium key=canonical format=raw"),
            HeaderError::MissingNewline
        );
        assert_eq!(parse_err(b"BITBABEL1 size=\xff\n"), HeaderError::NotText);
    }

    #[test]
    fn rejects_wrong_spacing_and_field_counts() {
        for line in [
            &b"BITBABEL1  size=medium key=canonical format=raw\n"[..],
            b"BITBABEL1 size=medium key=canonical format=raw \n",
            b"BITBABEL1 size=medium key=canonical\n",
            b"BITBABEL1 size=medium key=canonical format=raw check=00\n",
        ] {
            assert!(matches!(parse_err(line), HeaderError::FieldCount { .. }));
        }
    }

    #[test]
    fn rejects_out_of_order_or_unknown_fields() {
        assert_eq!(
            parse_err(b"BITBABEL1 key=canonical size=medium format=raw\n"),
            HeaderError::Field {
                expected: "size",
                found: "key=canonical".to_string()
            }
        );
        assert_eq!(
            parse_err(b"BITBABEL1 size=medium keys=canonical format=raw\n"),
            HeaderError::Field {
                expected: "key",
                found: "keys=canonical".to_string()
            }
        );
    }

    #[test]
    fn rejects_values_that_are_not_the_exact_names() {
        for (line, field, value) in [
            (
                &b"BITBABEL1 size=Medium key=canonical format=raw\n"[..],
                "size",
                "Medium",
            ),
            (
                b"BITBABEL1 size=huge key=canonical format=raw\n",
                "size",
                "huge",
            ),
            (
                b"BITBABEL1 size=medium key=secret format=raw\n",
                "key",
                "secret",
            ),
            (
                b"BITBABEL1 size=medium key=canonical format=raw\r\n",
                "format",
                "raw\r",
            ),
        ] {
            assert_eq!(
                parse_err(line),
                HeaderError::Value {
                    field,
                    value: value.to_string()
                }
            );
        }
    }

    #[test]
    fn error_messages_are_readable() {
        assert_eq!(
            HeaderError::NotAPreset(LibraryConfig::new(18, 8)).to_string(),
            "only small, medium and large can be written to a file, got page_len 18 and rounds 8"
        );
        assert_eq!(
            parse_err(b"BITBABEL1 size=huge key=canonical format=raw\n").to_string(),
            "invalid value \"huge\" for header field \"size\""
        );
    }
}
