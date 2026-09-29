use crate::encoding::Encoding;
use crate::index::PageIndex;

/// One block of babel bytes and the index it came from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Page {
    bytes: Vec<u8>,
    index: PageIndex,
}

impl Page {
    pub(crate) fn new(bytes: Vec<u8>, index: PageIndex) -> Self {
        Page { bytes, index }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn index(&self) -> &PageIndex {
        &self.index
    }

    /// Renders the bytes through `E`, e.g. `page.encode_as::<Hex>()`.
    pub fn encode_as<E: Encoding>(&self) -> E::Output {
        E::encode(&self.bytes)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::encoding::Hex;

    #[test]
    fn encodes_through_the_chosen_encoding() {
        let page = Page::new(vec![0xab, 0x01], PageIndex::zero(2));
        assert_eq!(page.encode_as::<Hex>(), "ab01");
    }
}
