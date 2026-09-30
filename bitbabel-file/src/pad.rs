//! Bringing data to a whole number of pages and back.
//!
//! The data gets a `0x80` marker, then zeros up to the next page boundary. Padding is always
//! added, even to data that already fills whole pages, so the last `0x80` before the trailing
//! zeros always marks where the data ends and no length field is needed.

use crate::error::PadError;

/// Marks the end of the data.
const MARKER: u8 = 0x80;

/// `data`, then `0x80`, then zeros up to the next multiple of `page_len`.
///
/// The result is always longer than `data`: data that already fills whole pages gains one
/// full page.
///
/// # Errors
///
/// Returns [`PadError::ZeroPageLen`] if `page_len` is zero.
///
/// # Example
///
/// ```
/// use bitbabel_file::pad;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// assert_eq!(pad(b"hello", 8)?, b"hello\x80\0\0");
/// assert_eq!(pad(b"", 4)?, b"\x80\0\0\0");
/// # Ok(())
/// # }
/// ```
pub fn pad(data: &[u8], page_len: usize) -> Result<Vec<u8>, PadError> {
    if page_len == 0 {
        return Err(PadError::ZeroPageLen);
    }
    // At least one byte (the marker), at most one page.
    let pad_len = page_len - data.len() % page_len;
    let mut padded = Vec::with_capacity(data.len() + pad_len);
    padded.extend_from_slice(data);
    padded.push(MARKER);
    padded.resize(data.len() + pad_len, 0);
    Ok(padded)
}

/// The data inside `padded`. Inverse of [`pad`].
///
/// Strict: it rejects anything [`pad`] could not have produced.
///
/// # Errors
///
/// Returns [`PadError::ZeroPageLen`] if `page_len` is zero, [`PadError::Misaligned`] unless
/// `padded` is a non-zero multiple of `page_len` bytes, [`PadError::MissingMarker`] if the
/// last non-zero byte is not `0x80`, or [`PadError::PaddingTooLong`] if the marker and zeros
/// span more than one page.
///
/// # Example
///
/// ```
/// use bitbabel_file::{unpad, PadError};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// assert_eq!(unpad(b"hello\x80\0\0", 8)?, b"hello");
/// assert_eq!(unpad(b"hello\0\0\0", 8), Err(PadError::MissingMarker));
/// # Ok(())
/// # }
/// ```
pub fn unpad(padded: &[u8], page_len: usize) -> Result<Vec<u8>, PadError> {
    if page_len == 0 {
        return Err(PadError::ZeroPageLen);
    }
    if padded.is_empty() || !padded.len().is_multiple_of(page_len) {
        return Err(PadError::Misaligned {
            len: padded.len(),
            page_len,
        });
    }

    let marker_at = padded
        .iter()
        .rposition(|&b| b != 0)
        .ok_or(PadError::MissingMarker)?;
    if padded[marker_at] != MARKER {
        return Err(PadError::MissingMarker);
    }

    let pad_len = padded.len() - marker_at;
    if pad_len > page_len {
        return Err(PadError::PaddingTooLong { pad_len, page_len });
    }
    Ok(padded[..marker_at].to_vec())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn round_trips_around_page_boundaries() {
        let page_len = 8;
        for len in [0, 1, page_len - 1, page_len, page_len + 1, 3 * page_len] {
            let data: Vec<u8> = (0..len as u8).map(|b| b.wrapping_add(1)).collect();
            let padded = pad(&data, page_len).unwrap();
            assert_eq!(padded.len() % page_len, 0);
            assert!(padded.len() > data.len());
            assert!(padded.len() - data.len() <= page_len);
            assert_eq!(unpad(&padded, page_len).unwrap(), data);
        }
    }

    #[test]
    fn data_ending_in_marker_and_zeros_round_trips() {
        // Looks like padding already, which is why padding is always added.
        let data = b"ab\x80\0";
        let padded = pad(data, 4).unwrap();
        assert_eq!(padded, b"ab\x80\0\x80\0\0\0");
        assert_eq!(unpad(&padded, 4).unwrap(), data);
    }

    #[test]
    fn full_pages_gain_a_whole_page() {
        assert_eq!(pad(b"abcd", 4).unwrap(), b"abcd\x80\0\0\0");
    }

    #[test]
    fn rejects_zero_page_len() {
        assert_eq!(pad(b"a", 0), Err(PadError::ZeroPageLen));
        assert_eq!(unpad(b"a", 0), Err(PadError::ZeroPageLen));
    }

    #[test]
    fn rejects_misaligned_or_empty_input() {
        for bad in [&b""[..], b"ab\x80", b"abc\x80\0"] {
            assert_eq!(
                unpad(bad, 4),
                Err(PadError::Misaligned {
                    len: bad.len(),
                    page_len: 4
                })
            );
        }
    }

    #[test]
    fn rejects_missing_marker() {
        assert_eq!(unpad(b"abc\0", 4), Err(PadError::MissingMarker));
        assert_eq!(unpad(b"\0\0\0\0", 4), Err(PadError::MissingMarker));
        assert_eq!(unpad(b"abc\x81", 4), Err(PadError::MissingMarker));
    }

    #[test]
    fn rejects_padding_longer_than_a_page() {
        assert_eq!(
            unpad(b"ab\x80\0\0\0\0\0", 4),
            Err(PadError::PaddingTooLong {
                pad_len: 6,
                page_len: 4
            })
        );
    }

    #[test]
    fn error_messages_are_readable() {
        assert_eq!(
            PadError::Misaligned {
                len: 5,
                page_len: 4
            }
            .to_string(),
            "padded data must be a non-zero multiple of 4 bytes, got 5"
        );
    }
}
