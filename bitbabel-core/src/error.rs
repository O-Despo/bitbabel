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
    /// The underlying cipher rejected its input.
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
            LibraryError::Cipher(_) => write!(f, "cipher error"),
        }
    }
}

impl Error for LibraryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            LibraryError::Cipher(err) => Some(err),
            _ => None,
        }
    }
}

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
    /// The hex text was malformed.
    Hex(HexDecodeError),
    /// The base64 text was malformed.
    Base64(Base64DecodeError),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::InvalidLength(len) => write!(f, "key must be 32 bytes, got {len}"),
            KeyError::Hex(_) => write!(f, "invalid hex key"),
            KeyError::Base64(_) => write!(f, "invalid base64 key"),
        }
    }
}

impl Error for KeyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            KeyError::Hex(err) => Some(err),
            KeyError::Base64(err) => Some(err),
            KeyError::InvalidLength(_) => None,
        }
    }
}

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
