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
    /// The header has the wrong number of fields. Doubled or trailing spaces count as empty
    /// fields.
    FieldCount { expected: usize, found: usize },
    /// A field is out of order or unknown.
    Field {
        expected: &'static str,
        found: String,
    },
    /// A field's value is not one of its lowercase names.
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
            HeaderError::FieldCount { expected, found } => {
                write!(f, "header must have {expected} fields, found {found}")
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
