//! The settings a file is written with, and the one-line header that records them.
//!
//! ```text
//! BITBABEL1 size=medium key=canonical format=raw check=1f2e3d4c5b6a7980\n
//! ```
//!
//! Fields come in a fixed order, separated by single spaces, all lowercase, and the line ends
//! in `\n`. `check` is left out when the checksum is off. Each value has one spelling, so each
//! header has exactly one valid form and parsing it then writing it back gives the same bytes.
//! The key itself is never written: `key=custom` only says the reader must supply one.

use bitbabel_core::{Encoding, Hex, LibraryConfig};

use crate::check::CHECK_LEN;
use crate::error::HeaderError;
use crate::format::IndexFormat;

/// The first word of every header. The digit is the format version.
const MAGIC: &str = "BITBABEL1";

/// Every header starts with this, whatever its version.
const MAGIC_PREFIX: &str = "BITBABEL";

/// Field names, in the order they must appear. All but the last are required.
const FIELDS: [&str; 4] = ["size", "key", "format", "check"];

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

/// How a file is written: the library size, the key mode, the index-list format, and whether
/// it carries a checksum.
///
/// The default is a medium, canonical, raw file with a checksum.
///
/// # Example
///
/// ```
/// use bitbabel_core::LibraryConfig;
/// use bitbabel_file::{IndexFormat, KeyMode, Settings};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let settings = Settings::new(LibraryConfig::SMALL, KeyMode::Custom, IndexFormat::Hex)?;
/// assert!(settings.check());
/// assert!(!settings.with_check(false).check());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Settings {
    size: LibraryConfig,
    key_mode: KeyMode,
    format: IndexFormat,
    check: bool,
}

impl Settings {
    /// Settings with the checksum on. Turn it off with [`with_check`](Self::with_check).
    ///
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
            check: true,
        })
    }

    /// Sets whether files carry a checksum of their data.
    pub fn with_check(mut self, check: bool) -> Self {
        self.check = check;
        self
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

    /// Whether files carry a checksum.
    pub fn check(&self) -> bool {
        self.check
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            size: LibraryConfig::default(),
            key_mode: KeyMode::default(),
            format: IndexFormat::default(),
            check: true,
        }
    }
}

/// A header line: the settings, plus the checksum value when the settings ask for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Header {
    pub(crate) settings: Settings,
    /// `Some` exactly when `settings.check()` is true.
    pub(crate) check: Option<[u8; CHECK_LEN]>,
}

impl Header {
    /// The header line, ending in `\n`. Always ASCII.
    pub(crate) fn write(&self) -> String {
        let settings = &self.settings;
        // `Settings::new` only accepts presets, so the size always has a name.
        let size = settings.size.name().unwrap_or_default();
        let mut line = format!(
            "{MAGIC} size={size} key={} format={}",
            settings.key_mode.name(),
            settings.format.name()
        );
        if let Some(check) = self.check {
            line.push_str(" check=");
            line.push_str(&Hex::encode(&check));
        }
        line.push('\n');
        line
    }

    /// Reads the header at the start of `bytes`. Returns it and the bytes after its `\n`,
    /// which are the payload.
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
    /// - [`HeaderError::Value`] for a value that is not one of the lowercase names, or a check
    ///   that is not 16 lowercase hex digits.
    pub(crate) fn parse(bytes: &[u8]) -> Result<(Self, &[u8]), HeaderError> {
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
        if words.len() != FIELDS.len() && words.len() != FIELDS.len() - 1 {
            return Err(HeaderError::FieldCount { found: words.len() });
        }
        let values = words
            .iter()
            .zip(FIELDS)
            .map(|(word, expected)| {
                word.strip_prefix(expected)
                    .and_then(|rest| rest.strip_prefix('='))
                    .ok_or_else(|| HeaderError::Field {
                        expected,
                        found: word.to_string(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let invalid = |field: &'static str, value: &str| HeaderError::Value {
            field,
            value: value.to_string(),
        };
        let (size, key_mode, format) = (values[0], values[1], values[2]);
        let check = values
            .get(3)
            .map(|&check| parse_check(check).ok_or_else(|| invalid("check", check)))
            .transpose()?;
        let settings = Settings {
            // Only exact lowercase preset names parse, so this is always a preset.
            size: size.parse().map_err(|_| invalid("size", size))?,
            key_mode: KeyMode::from_name(key_mode).ok_or_else(|| invalid("key", key_mode))?,
            format: IndexFormat::from_name(format).ok_or_else(|| invalid("format", format))?,
            check: check.is_some(),
        };
        Ok((Header { settings, check }, payload))
    }
}

/// A check value, only if it is written exactly as [`Header::write`] writes it.
fn parse_check(text: &str) -> Option<[u8; CHECK_LEN]> {
    let bytes = Hex::decode(&text.to_string()).ok()?;
    let check = <[u8; CHECK_LEN]>::try_from(bytes).ok()?;
    // Hex decoding also accepts uppercase; only the lowercase spelling is valid.
    (Hex::encode(&check) == text).then_some(check)
}

#[cfg(test)]
mod test {
    use super::*;

    const CHECK: [u8; CHECK_LEN] = [0x1f, 0x2e, 0x3d, 0x4c, 0x5b, 0x6a, 0x79, 0x80];

    fn all_headers() -> Vec<Header> {
        let mut all = Vec::new();
        for size in [
            LibraryConfig::SMALL,
            LibraryConfig::MEDIUM,
            LibraryConfig::LARGE,
        ] {
            for key_mode in [KeyMode::Canonical, KeyMode::Custom] {
                for format in [IndexFormat::Raw, IndexFormat::Hex, IndexFormat::Base64] {
                    for check in [Some(CHECK), None] {
                        let settings = Settings::new(size, key_mode, format)
                            .unwrap()
                            .with_check(check.is_some());
                        all.push(Header { settings, check });
                    }
                }
            }
        }
        all
    }

    fn parse_err(line: &[u8]) -> HeaderError {
        Header::parse(line).unwrap_err()
    }

    #[test]
    fn writes_the_agreed_header() {
        let settings = Settings::default();
        assert!(settings.check());
        assert_eq!(
            Header {
                settings,
                check: Some(CHECK)
            }
            .write(),
            "BITBABEL1 size=medium key=canonical format=raw check=1f2e3d4c5b6a7980\n"
        );
        assert_eq!(
            Header {
                settings: settings.with_check(false),
                check: None
            }
            .write(),
            "BITBABEL1 size=medium key=canonical format=raw\n"
        );
    }

    #[test]
    fn every_header_round_trips_byte_for_byte() {
        for header in all_headers() {
            let line = header.write();
            let (parsed, payload) = Header::parse(line.as_bytes()).unwrap();
            assert_eq!(parsed, header);
            assert!(payload.is_empty());
            assert_eq!(parsed.write(), line);
        }
    }

    #[test]
    fn payload_is_everything_after_the_first_newline() {
        let mut file = b"BITBABEL1 size=small key=canonical format=raw\n".to_vec();
        file.extend_from_slice(b"\x00\n\xff\n");
        let (_, payload) = Header::parse(&file).unwrap();
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
            &b"BITBABEL1 size=medium key=canonical\n"[..],
            b"BITBABEL1 size=medium key=canonical format=raw check=1f2e3d4c5b6a7980 x=1\n",
            b"BITBABEL1  size=medium key=canonical format=raw check=1f2e3d4c5b6a7980\n",
            b"BITBABEL1 size=medium key=canonical format=raw check=1f2e3d4c5b6a7980 \n",
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
            parse_err(b"BITBABEL1 size=medium key=canonical format=raw sum=00\n"),
            HeaderError::Field {
                expected: "check",
                found: "sum=00".to_string()
            }
        );
        // A trailing space before the newline makes an empty fourth field.
        assert_eq!(
            parse_err(b"BITBABEL1 size=medium key=canonical format=raw \n"),
            HeaderError::Field {
                expected: "check",
                found: String::new()
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
            (
                b"BITBABEL1 size=medium key=canonical format=raw check=1F2E3D4C5B6A7980\n",
                "check",
                "1F2E3D4C5B6A7980",
            ),
            (
                b"BITBABEL1 size=medium key=canonical format=raw check=1f2e\n",
                "check",
                "1f2e",
            ),
            (
                b"BITBABEL1 size=medium key=canonical format=raw check=zz2e3d4c5b6a7980\n",
                "check",
                "zz2e3d4c5b6a7980",
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
        assert_eq!(
            HeaderError::FieldCount { found: 2 }.to_string(),
            "header must have 3 fields, or 4 with a check, found 2"
        );
    }
}
