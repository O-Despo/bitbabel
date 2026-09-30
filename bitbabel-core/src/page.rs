use std::fmt;

use crate::encoding::Encoding;
use crate::index::{HexPreview, PageIndex};

/// One block of babel bytes and the index it came from.
///
/// `{:?}` shows a short hex preview of both; `{:#?}` shows them in full.
#[derive(Clone, PartialEq, Eq, Hash)]
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

impl fmt::Debug for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Page")
            .field("index", &HexPreview(self.index.as_bytes()))
            .field("bytes", &HexPreview(&self.bytes))
            .finish()
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

    #[test]
    fn debug_previews_both_fields() {
        let page = Page::new(vec![0xee; 3200], PageIndex::zero(3200));
        assert_eq!(
            format!("{page:?}"),
            "Page { index: 0000000000000000… (3200 bytes), bytes: eeeeeeeeeeeeeeee… (3200 bytes) }"
        );
        assert!(format!("{page:#?}").contains(&"ee".repeat(3200)));
    }
}
