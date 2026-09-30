use std::fmt;

/// An arbitrary-length, big-endian unsigned integer that addresses a page.
///
/// Its length is not tied to any integer width, so it can match the page size: a 3200-byte
/// page needs an index space far larger than `u64`.
///
/// `{:?}` shows a short hex preview (a MEDIUM index is 3200 bytes); `{:#?}` shows it all.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PageIndex(Vec<u8>);

/// Bytes shown in a `{:?}` preview before it is cut short.
const PREVIEW_LEN: usize = 8;

/// `Debug` for a byte buffer: hex, cut to [`PREVIEW_LEN`] bytes plus the total length unless
/// the alternate flag (`{:#?}`) asks for all of it.
pub(crate) struct HexPreview<'a>(pub(crate) &'a [u8]);

impl fmt::Debug for HexPreview<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() || self.0.len() <= PREVIEW_LEN {
            return self.0.iter().try_for_each(|b| write!(f, "{b:02x}"));
        }
        self.0[..PREVIEW_LEN]
            .iter()
            .try_for_each(|b| write!(f, "{b:02x}"))?;
        write!(f, "… ({} bytes)", self.0.len())
    }
}

impl PageIndex {
    /// An index of `len` zero bytes.
    pub fn zero(len: usize) -> Self {
        PageIndex(vec![0u8; len])
    }

    /// `value` as `len` big-endian bytes, zero-padded, or truncated to its least-significant
    /// `len` bytes if `len` is under 8.
    pub fn from_u64(value: u64, len: usize) -> Self {
        let mut bytes = vec![0u8; len];
        // Fill from the least-significant end; whichever side runs out first stops the copy.
        for (dst, src) in bytes.iter_mut().rev().zip(value.to_be_bytes().iter().rev()) {
            *dst = *src;
        }
        PageIndex(bytes)
    }

    /// Wraps big-endian `bytes`. Any length is valid; a library checks it when the index is used.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        PageIndex(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// Adds one, wrapping to all-zero on overflow.
    pub fn increment(&mut self) {
        for byte in self.0.iter_mut().rev() {
            let (next, carry) = byte.overflowing_add(1);
            *byte = next;
            if !carry {
                return;
            }
        }
    }

    /// Subtracts one, wrapping to all-`0xFF` on underflow.
    pub fn decrement(&mut self) {
        for byte in self.0.iter_mut().rev() {
            let (next, borrow) = byte.overflowing_sub(1);
            *byte = next;
            if !borrow {
                return;
            }
        }
    }
}

impl AsRef<[u8]> for PageIndex {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<PageIndex> for Vec<u8> {
    fn from(index: PageIndex) -> Self {
        index.0
    }
}

impl From<Vec<u8>> for PageIndex {
    fn from(bytes: Vec<u8>) -> Self {
        PageIndex(bytes)
    }
}

impl fmt::Debug for PageIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PageIndex")
            .field(&HexPreview(&self.0))
            .finish()
    }
}

impl fmt::LowerHex for PageIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl fmt::UpperHex for PageIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02X}"))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn from_u64_pads_and_truncates() {
        assert_eq!(PageIndex::from_u64(0x0102, 4).as_bytes(), &[0, 0, 1, 2]);
        assert_eq!(PageIndex::from_u64(0x0102, 1).as_bytes(), &[2]);
        assert_eq!(PageIndex::from_u64(7, 0).as_bytes(), &[] as &[u8]);
    }

    #[test]
    fn increment_wraps_at_all_ff() {
        let mut index = PageIndex::from_bytes(vec![0xFF, 0xFF]);
        index.increment();
        assert_eq!(index.as_bytes(), &[0x00, 0x00]);
    }

    #[test]
    fn increment_carries() {
        let mut index = PageIndex::from_bytes(vec![0x00, 0xFF]);
        index.increment();
        assert_eq!(index.as_bytes(), &[0x01, 0x00]);
    }

    #[test]
    fn decrement_wraps_at_zero() {
        let mut index = PageIndex::zero(2);
        index.decrement();
        assert_eq!(index.as_bytes(), &[0xFF, 0xFF]);
    }

    #[test]
    fn decrement_borrows() {
        let mut index = PageIndex::from_bytes(vec![0x01, 0x00]);
        index.decrement();
        assert_eq!(index.as_bytes(), &[0x00, 0xFF]);
    }

    #[test]
    fn empty_index_is_a_no_op() {
        let mut index = PageIndex::zero(0);
        index.increment();
        index.decrement();
        assert!(index.as_bytes().is_empty());
    }

    #[test]
    fn increment_handles_large_arbitrary_precision_index() {
        // A page-size relevant length (e.g. matching a 3200-byte page mode), far beyond u64.
        let mut index = PageIndex::zero(400);
        index.increment();
        let mut expected = vec![0u8; 400];
        expected[399] = 1;
        assert_eq!(index.into_bytes(), expected);
    }

    #[test]
    fn debug_previews_long_indexes() {
        let index = PageIndex::from_u64(0xabcd, 3200);
        assert_eq!(
            format!("{index:?}"),
            "PageIndex(0000000000000000… (3200 bytes))"
        );
        assert!(format!("{index:#?}").contains(&"00".repeat(3198)));
    }

    #[test]
    fn debug_shows_short_indexes_in_full() {
        let index = PageIndex::from_bytes(vec![0x0a, 0xff]);
        assert_eq!(format!("{index:?}"), "PageIndex(0aff)");
    }

    #[test]
    fn formats_as_hex() {
        let index = PageIndex::from_bytes(vec![0x0a, 0xff]);
        assert_eq!(format!("{index:x}"), "0aff");
        assert_eq!(format!("{index:X}"), "0AFF");
    }
}
