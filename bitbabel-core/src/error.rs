use std::error::Error;
use std::fmt;

use crate::encoding::{Base64DecodeError, HexDecodeError};

/// Errors from building or driving the Feistel cipher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CipherError {
    /// A block must split into two equal halves, so its length must be even.
    OddBlockLen(usize),
    /// Zero rounds would make the permutation the identity.
    ZeroRounds,
}

impl fmt::Display for CipherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CipherError::OddBlockLen(len) => {
                write!(f, "block length must be even, got {len}")
            }
            CipherError::ZeroRounds => write!(f, "rounds must be at least 1"),
        }
    }
}

impl Error for CipherError {}

/// Errors from building or addressing a [`BabelLibrary`](crate::BabelLibrary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryError {
    /// `page_len` must be even and non-zero.
    InvalidPageLen(usize),
    /// An index or page was not exactly `page_len` bytes long.
    LenMismatch { expected: usize, actual: usize },
    /// The underlying cipher rejected its input. The message includes the cipher's own.
    Cipher(CipherError),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::InvalidPageLen(len) => {
                write!(f, "page length must be even and non-zero, got {len}")
            }
            LibraryError::LenMismatch { expected, actual } => {
                write!(f, "expected {expected} bytes, got {actual}")
            }
            LibraryError::Cipher(err) => write!(f, "cipher error: {err}"),
        }
    }
}

// No `source()`: `Display` already includes the inner message, and reporting it both ways
// would print it twice in an error chain.
impl Error for LibraryError {}

impl From<CipherError> for LibraryError {
    fn from(err: CipherError) -> Self {
        LibraryError::Cipher(err)
    }
}

/// Errors from building a [`Key`](crate::Key) from bytes or text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    /// A key is exactly 32 bytes.
    InvalidLength(usize),
    /// The hex text was malformed. The message includes the decoder's own.
    Hex(HexDecodeError),
    /// The base64 text was malformed. The message includes the decoder's own.
    Base64(Base64DecodeError),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::InvalidLength(len) => write!(f, "key must be 32 bytes, got {len}"),
            KeyError::Hex(err) => write!(f, "invalid key: {err}"),
            KeyError::Base64(err) => write!(f, "invalid key: {err}"),
        }
    }
}

// No `source()`, for the same reason as `LibraryError`.
impl Error for KeyError {}

impl From<HexDecodeError> for KeyError {
    fn from(err: HexDecodeError) -> Self {
        KeyError::Hex(err)
    }
}

impl From<Base64DecodeError> for KeyError {
    fn from(err: Base64DecodeError) -> Self {
        KeyError::Base64(err)
    }
}

/// Errors from reading a [`PageIndex`](crate::PageIndex) from text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageIndexError {
    /// The hex text was malformed. The message includes the decoder's own.
    Hex(HexDecodeError),
    /// The base64 text was malformed. The message includes the decoder's own.
    Base64(Base64DecodeError),
    /// The text decoded to the wrong number of bytes for this library.
    LenMismatch { expected: usize, actual: usize },
}

impl fmt::Display for PageIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PageIndexError::Hex(err) => write!(f, "invalid index: {err}"),
            PageIndexError::Base64(err) => write!(f, "invalid index: {err}"),
            PageIndexError::LenMismatch { expected, actual } => {
                write!(f, "index must be {expected} bytes, got {actual}")
            }
        }
    }
}

// No `source()`, for the same reason as `LibraryError`.
impl Error for PageIndexError {}

impl From<HexDecodeError> for PageIndexError {
    fn from(err: HexDecodeError) -> Self {
        PageIndexError::Hex(err)
    }
}

impl From<Base64DecodeError> for PageIndexError {
    fn from(err: Base64DecodeError) -> Self {
        PageIndexError::Base64(err)
    }
}

/// A size name was not one of the presets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigNameError(pub String);

impl fmt::Display for ConfigNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown size {:?}, expected small, medium or large",
            self.0
        )
    }
}

impl Error for ConfigNameError {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::encoding::{Base64, Encoding, Hex};

    #[test]
    fn library_error_includes_the_cipher_message() {
        let err = LibraryError::from(CipherError::ZeroRounds);
        assert_eq!(err.to_string(), "cipher error: rounds must be at least 1");
        assert!(err.source().is_none());
    }

    #[test]
    fn key_error_includes_the_decoder_message() {
        let hex = KeyError::from(Hex::decode(&"zz".to_string()).unwrap_err());
        assert_eq!(hex.to_string(), "invalid key: invalid hex digit 'z'");

        let b64 = KeyError::from(Base64::decode(&"!!".to_string()).unwrap_err());
        assert!(b64.to_string().starts_with("invalid key: invalid base64: "));
        assert!(b64.source().is_none());
    }
}
