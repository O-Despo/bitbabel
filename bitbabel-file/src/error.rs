use std::error::Error;
use std::fmt;

use bitbabel_core::{LibraryConfig, LibraryError, PageIndexError};

/// Errors from [`pad`](crate::pad) and [`unpad`](crate::unpad).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PadError {
    /// A page length of zero cannot hold anything.
    ZeroPageLen,
    /// Padded data must be a non-zero whole number of pages.
    Misaligned { len: usize, page_len: usize },
    /// The last non-zero byte was not the `0x80` marker, or there was none.
    MissingMarker,
    /// The marker and the zeros after it are longer than one page, which [`pad`](crate::pad)
    /// never produces.
    PaddingTooLong { pad_len: usize, page_len: usize },
}

impl fmt::Display for PadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PadError::ZeroPageLen => write!(f, "page length must be non-zero"),
            PadError::Misaligned { len, page_len } => write!(
                f,
                "padded data must be a non-zero multiple of {page_len} bytes, got {len}"
            ),
            PadError::MissingMarker => write!(f, "padding marker 0x80 not found"),
            PadError::PaddingTooLong { pad_len, page_len } => write!(
                f,
                "padding is {pad_len} bytes but must be at most one page ({page_len})"
            ),
        }
    }
}

impl Error for PadError {}

/// Errors from [`IndexFormat`](crate::IndexFormat) encoding or decoding a list of indices.
///
/// `position` counts indices from 0 in the list, not lines in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexFormatError {
    /// A page length of zero cannot hold an index.
    ZeroPageLen,
    /// A list always has at least one index, because padding always yields a page.
    Empty,
    /// An index to write was not `page_len` bytes.
    IndexLen {
        position: usize,
        expected: usize,
        actual: usize,
    },
    /// A raw list must be a whole number of indices.
    Misaligned { len: usize, page_len: usize },
    /// Every line of a Hex or Base64 list ends in `\n`, including the last.
    MissingFinalNewline,
    /// A line did not decode to an index of `page_len` bytes. The message includes the
    /// decoder's own.
    Line {
        position: usize,
        error: PageIndexError,
    },
    /// A line decoded, but is not written the way the encoder writes it (e.g. uppercase hex),
    /// so the list would have more than one valid encoding.
    NotCanonical { position: usize },
}

impl fmt::Display for IndexFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexFormatError::ZeroPageLen => write!(f, "page length must be non-zero"),
            IndexFormatError::Empty => write!(f, "index list is empty"),
            IndexFormatError::IndexLen {
                position,
                expected,
                actual,
            } => write!(f, "index {position} is {actual} bytes, expected {expected}"),
            IndexFormatError::Misaligned { len, page_len } => write!(
                f,
                "raw index list must be a multiple of {page_len} bytes, got {len}"
            ),
            IndexFormatError::MissingFinalNewline => {
                write!(f, "index list must end with a newline")
            }
            IndexFormatError::Line { position, error } => write!(f, "index {position}: {error}"),
            IndexFormatError::NotCanonical { position } => {
                write!(f, "index {position} is not written in canonical form")
            }
        }
    }
}

// No `source()`: `Display` already includes the inner message, and reporting it both ways
// would print it twice in an error chain.
impl Error for IndexFormatError {}

/// Errors from building [`Settings`](crate::Settings) or reading a file's header line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderError {
    /// The header can only name the presets, so custom shapes cannot be written to a file.
    NotAPreset(LibraryConfig),
    /// The bytes do not start with `BITBABEL`.
    NotBabelFile,
    /// The header is from a format version this crate does not read.
    UnsupportedVersion(String),
    /// The header line has no terminating `\n`.
    MissingNewline,
    /// The header line is not UTF-8.
    NotText,
    /// The header must have 3 fields, or 4 with a check. Doubled or trailing spaces count as
    /// empty fields.
    FieldCount { found: usize },
    /// A field is out of order or unknown.
    Field {
        expected: &'static str,
        found: String,
    },
    /// A field's value is not one of its lowercase names, or a check is not 16 lowercase hex
    /// digits.
    Value { field: &'static str, value: String },
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeaderError::NotAPreset(config) => write!(
                f,
                "only small, medium and large can be written to a file, got page_len {} and rounds {}",
                config.page_len(),
                config.rounds()
            ),
            HeaderError::NotBabelFile => write!(f, "not a bitbabel file"),
            HeaderError::UnsupportedVersion(version) => {
                write!(
                    f,
                    "unsupported file version {version:?}, expected \"BITBABEL1\""
                )
            }
            HeaderError::MissingNewline => write!(f, "header line has no terminating newline"),
            HeaderError::NotText => write!(f, "header line is not valid text"),
            HeaderError::FieldCount { found } => {
                write!(
                    f,
                    "header must have 3 fields, or 4 with a check, found {found}"
                )
            }
            HeaderError::Field { expected, found } => {
                write!(f, "expected header field {expected:?}, found {found:?}")
            }
            HeaderError::Value { field, value } => {
                write!(f, "invalid value {value:?} for header field {field:?}")
            }
        }
    }
}

impl Error for HeaderError {}

/// Errors from [`BabelFile`](crate::BabelFile).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileError {
    /// The file uses a custom key but none was supplied.
    MissingKey,
    /// The file uses the canonical key, so a supplied key would be ignored.
    UnexpectedKey,
    /// The header line was invalid. The message includes the header error's own.
    Header(HeaderError),
    /// The index list was invalid. The message includes the format error's own.
    Index(IndexFormatError),
    /// The data was not padded as expected, usually because of a wrong key. The message
    /// includes the padding error's own.
    Pad(PadError),
    /// The data does not match the file's checksum: a wrong key or a corrupted file.
    ChecksumMismatch,
    /// The library rejected a page or index. The file keeps every index one page long, so
    /// this is not expected.
    Library(LibraryError),
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileError::MissingKey => write!(f, "this file uses a custom key, but none was given"),
            FileError::UnexpectedKey => {
                write!(f, "this file uses the canonical key, but a key was given")
            }
            FileError::Header(err) => write!(f, "invalid header: {err}"),
            FileError::Index(err) => write!(f, "invalid index list: {err}"),
            FileError::Pad(err) => write!(f, "decoded data is not padded (wrong key?): {err}"),
            FileError::ChecksumMismatch => {
                write!(f, "checksum does not match: wrong key or corrupted file")
            }
            FileError::Library(err) => write!(f, "library error: {err}"),
        }
    }
}

// No `source()`, for the same reason as `IndexFormatError`.
impl Error for FileError {}

impl From<HeaderError> for FileError {
    fn from(err: HeaderError) -> Self {
        FileError::Header(err)
    }
}

impl From<IndexFormatError> for FileError {
    fn from(err: IndexFormatError) -> Self {
        FileError::Index(err)
    }
}

impl From<PadError> for FileError {
    fn from(err: PadError) -> Self {
        FileError::Pad(err)
    }
}

impl From<LibraryError> for FileError {
    fn from(err: LibraryError) -> Self {
        FileError::Library(err)
    }
}

/// Errors from the bookmark file: its header now, and its lines and list as they are added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookmarkError {
    /// The header can only name the presets, so custom shapes cannot be written to a file.
    NotAPreset(LibraryConfig),
    /// The bytes do not start with `BABELMARKS`.
    NotBookmarkFile,
    /// The header is from a format version this crate does not read.
    UnsupportedVersion(String),
    /// The header line has no terminating `\n`.
    MissingNewline,
    /// The header line is not UTF-8.
    NotText,
    /// A line ends in `\r\n`: the file was saved with Windows line endings.
    WindowsLineEndings,
    /// The header must have 3 fields, or 4 with a fingerprint. Doubled or trailing spaces
    /// count as empty fields.
    FieldCount { found: usize },
    /// A field is out of order or unknown.
    Field {
        expected: &'static str,
        found: String,
    },
    /// A field's value is not one of its lowercase names, or a fingerprint is not 16
    /// lowercase hex digits.
    Value { field: &'static str, value: String },
    /// `key=custom` needs a `fingerprint` field.
    MissingFingerprint,
    /// A `fingerprint` field only belongs to `key=custom`.
    UnexpectedFingerprint,
}

impl fmt::Display for BookmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BookmarkError::NotAPreset(config) => write!(
                f,
                "only small, medium and large can be written to a file, got page_len {} and rounds {}",
                config.page_len(),
                config.rounds()
            ),
            BookmarkError::NotBookmarkFile => write!(f, "not a bookmark file"),
            BookmarkError::UnsupportedVersion(version) => write!(
                f,
                "unsupported bookmark file version {version:?}, expected \"BABELMARKS1\""
            ),
            BookmarkError::MissingNewline => write!(f, "header line has no terminating newline"),
            BookmarkError::NotText => write!(f, "header line is not valid text"),
            BookmarkError::WindowsLineEndings => write!(
                f,
                "file has Windows line endings (\\r\\n); save it with Unix line endings"
            ),
            BookmarkError::FieldCount { found } => write!(
                f,
                "header must have 3 fields, or 4 with a fingerprint, found {found}"
            ),
            BookmarkError::Field { expected, found } => {
                write!(f, "expected header field {expected:?}, found {found:?}")
            }
            BookmarkError::Value { field, value } => {
                write!(f, "invalid value {value:?} for header field {field:?}")
            }
            BookmarkError::MissingFingerprint => {
                write!(f, "header says key=custom but has no fingerprint")
            }
            BookmarkError::UnexpectedFingerprint => {
                write!(f, "header has a fingerprint but says key=canonical")
            }
        }
    }
}

// No `source()`, for the same reason as `IndexFormatError`.
impl Error for BookmarkError {}

/// Errors from one bookmark line, or from building a [`Bookmark`](crate::Bookmark).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineError {
    /// The line is not UTF-8.
    NotText,
    /// The line ends in `\r`: the file was saved with Windows line endings.
    WindowsLineEndings,
    /// A line is `time TAB label TAB index`, so it has exactly three fields.
    FieldCount { found: usize },
    /// The time is not the plain digits of a `u64`.
    Time(String),
    /// The label has a control character, or one that reorders text.
    InvalidLabel(char),
    /// The label has more than `max` characters.
    LabelTooLong { len: usize, max: usize },
    /// The index is not canonical base64 of one page. The message includes the decoder's own.
    Index(PageIndexError),
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineError::NotText => write!(f, "line is not valid text"),
            LineError::WindowsLineEndings => write!(
                f,
                "line ends in a carriage return (file saved with Windows line endings?)"
            ),
            LineError::FieldCount { found } => write!(
                f,
                "a bookmark line must have 3 tab-separated fields, found {found}"
            ),
            LineError::Time(time) => {
                write!(
                    f,
                    "invalid time {time:?}, expected digits without leading zeros"
                )
            }
            LineError::InvalidLabel(c) => write!(
                f,
                "label contains a control or text-reordering character U+{:04X}",
                u32::from(*c)
            ),
            LineError::LabelTooLong { len, max } => {
                write!(f, "label has {len} characters, at most {max} are allowed")
            }
            LineError::Index(err) => write!(f, "{err}"),
        }
    }
}

// No `source()`, for the same reason as `IndexFormatError`.
impl Error for LineError {}
