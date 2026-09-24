use crate::cipher::{BabelMachine, FeistelBytes};
use crate::encoding::Encoding;

/// An arbitrary-precision, big-endian unsigned counter used to address pages.
///
/// Its byte length is independent of any fixed integer width so it can scale with the
/// page size it's paired with (a 3200-byte page needs an index space far larger than
/// `u64` can express to stay meaningfully unique).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageIndex(Vec<u8>);

impl PageIndex {
    pub fn zero(len: usize) -> Self {
        PageIndex(vec![0u8; len])
    }

    /// Zero-padded (or, if `len` is smaller than 8, truncated to the least-significant
    /// `len` bytes) big-endian representation of `value`.
    pub fn from_u64(value: u64, len: usize) -> Self {
        let full = value.to_be_bytes();
        let mut bytes = vec![0u8; len];
        let copy_len = full.len().min(len);
        bytes[len - copy_len..].copy_from_slice(&full[full.len() - copy_len..]);
        PageIndex(bytes)
    }

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
            if *byte == 0xFF {
                *byte = 0;
            } else {
                *byte += 1;
                return;
            }
        }
    }

    /// Subtracts one, wrapping to all-`0xFF` on underflow.
    pub fn decrement(&mut self) {
        for byte in self.0.iter_mut().rev() {
            if *byte == 0 {
                *byte = 0xFF;
            } else {
                *byte -= 1;
                return;
            }
        }
    }
}

/// A single deterministically-generated block of babel bytes, tagged with the index it
/// came from.
pub struct Page {
    bytes: Vec<u8>,
    index: PageIndex,
}

impl Page {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn index(&self) -> &PageIndex {
        &self.index
    }

    /// Views this page's bytes through a specific `Encoding`, without `Page` itself
    /// knowing anything about the format.
    pub fn encode_as<E: Encoding>(&self) -> E::Output {
        E::encode(&self.bytes)
    }
}

/// Deterministically maps a [`PageIndex`] to a [`Page`] of babel bytes.
///
/// Every index of `page_len` bytes maps to exactly one page and back (via
/// `BabelMachine::forward`/`backward`, which are inverse bijections over a fixed-length
/// block), so generation is pure and stateless: the same index always yields the same page.
pub struct BabelLibrary {
    machine: BabelMachine,
    page_len: usize,
}

impl BabelLibrary {
    pub fn new(machine: BabelMachine, page_len: usize) -> Result<Self, &'static str> {
        if page_len % 2 == 0 {
            Ok(BabelLibrary { machine, page_len })
        } else {
            Err("page_len must be even.")
        }
    }

    pub fn page_len(&self) -> usize {
        self.page_len
    }

    pub fn page_at(&self, index: &PageIndex) -> Page {
        assert_eq!(
            index.as_bytes().len(),
            self.page_len,
            "index length must match page_len"
        );

        let mut block = FeistelBytes::new(index.as_bytes().to_vec())
            .expect("page_len is validated even in BabelLibrary::new");
        self.machine.forward(&mut block);

        Page {
            bytes: block.into_bytes(),
            index: index.clone(),
        }
    }
}

/// A stateful position within a [`BabelLibrary`], for moving forward/backward through
/// pages by index. All navigation happens on bytes (`PageIndex`); conversion to any
/// concrete type only happens if the caller asks a returned `Page` for one.
pub struct Cursor {
    library: BabelLibrary,
    index: PageIndex,
}

impl Cursor {
    pub fn new(library: BabelLibrary, start: PageIndex) -> Self {
        Cursor { library, index: start }
    }

    pub fn current(&self) -> Page {
        self.library.page_at(&self.index)
    }

    pub fn next(&mut self) -> Page {
        self.index.increment();
        self.current()
    }

    pub fn prev(&mut self) -> Page {
        self.index.decrement();
        self.current()
    }

    pub fn jump(&mut self, index: PageIndex) -> Page {
        self.index = index;
        self.current()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn library(page_len: usize) -> BabelLibrary {
        BabelLibrary::new(BabelMachine::new([7u8; 32], 4), page_len).unwrap()
    }

    #[test]
    fn rejects_odd_page_len() {
        assert!(BabelLibrary::new(BabelMachine::new([0u8; 32], 4), 3).is_err());
    }

    #[test]
    fn same_index_yields_same_page() {
        let lib = library(8);
        let index = PageIndex::from_u64(42, 8);
        assert_eq!(lib.page_at(&index).bytes(), lib.page_at(&index).bytes());
    }

    #[test]
    fn forward_and_backward_are_inverse_through_the_library() {
        let lib = library(8);
        let index = PageIndex::from_u64(42, 8);
        let page = lib.page_at(&index);

        let mut block = FeistelBytes::new(page.bytes().to_vec()).unwrap();
        BabelMachine::new([7u8; 32], 4).backward(&mut block);
        assert_eq!(block.into_bytes(), index.into_bytes());
    }

    #[test]
    fn cursor_next_then_prev_returns_to_start() {
        let mut cursor = Cursor::new(library(8), PageIndex::from_u64(5, 8));
        let start = cursor.current().bytes().to_vec();
        cursor.next();
        let back = cursor.prev().bytes().to_vec();
        assert_eq!(start, back);
    }

    #[test]
    fn increment_wraps_at_all_ff() {
        let mut index = PageIndex::from_bytes(vec![0xFF, 0xFF]);
        index.increment();
        assert_eq!(index.as_bytes(), &[0x00, 0x00]);
    }

    #[test]
    fn decrement_wraps_at_zero() {
        let mut index = PageIndex::zero(2);
        index.decrement();
        assert_eq!(index.as_bytes(), &[0xFF, 0xFF]);
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
    fn page_encodes_as_hex() {
        use crate::encoding::Hex;

        let lib = library(4);
        let page = lib.page_at(&PageIndex::zero(4));
        assert_eq!(page.encode_as::<Hex>(), Hex::encode(page.bytes()));
    }
}
