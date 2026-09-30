use std::error::Error;
use std::fmt;

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
