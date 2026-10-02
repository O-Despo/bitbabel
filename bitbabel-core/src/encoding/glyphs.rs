//! A display view of page bytes that remembers which bytes each piece came from.
//!
//! [`Encoding`](super::Encoding) turns bytes into one string, which loses where each
//! character came from. A viewer that wants to wrap lines or highlight a byte range needs the offsets, so
//! [`Glyphs`] gives a list of [`Glyph`]s instead.

use std::ops::Range;

use super::{BabelGuaranteedText, Base64, Encoding, Hex};

/// One displayed piece of a page: its text, and the bytes it came from.
///
/// The glyphs of one page cover its bytes exactly once, in order, with no gaps.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Glyph {
    bytes: Range<usize>,
    text: String,
}

impl Glyph {
    fn new(bytes: Range<usize>, text: impl Into<String>) -> Self {
        Glyph {
            bytes,
            text: text.into(),
        }
    }

    /// The byte range this glyph shows, as offsets into the page.
    pub fn bytes(&self) -> Range<usize> {
        self.bytes.clone()
    }

    /// The text to draw. It is never a control character, so it cannot move the cursor or
    /// change a terminal's state.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Splits page bytes into [`Glyph`]s for display.
///
/// | Type | One glyph per | Example for bytes `48 65 ff 0a` |
/// |---|---|---|
/// | [`BabelGuaranteedText`] | byte | `H` `e` and the table's characters for `ff` and `0a` |
/// | [`Hex`] | byte | `48` `65` `ff` `0a` |
/// | [`Base64`] | 3 bytes | `SGVm` `Cg==` |
///
/// # Example
///
/// ```
/// use bitbabel_core::{Glyphs, Hex};
///
/// let glyphs = Hex::glyphs(&[0x48, 0xff]);
/// assert_eq!(glyphs[1].text(), "ff");
/// assert_eq!(glyphs[1].bytes(), 1..2);
/// ```
pub trait Glyphs {
    /// The glyphs of `bytes`, in order. Empty input gives no glyphs.
    fn glyphs(bytes: &[u8]) -> Vec<Glyph>;
}

impl Glyphs for BabelGuaranteedText {
    fn glyphs(bytes: &[u8]) -> Vec<Glyph> {
        bytes
            .iter()
            .enumerate()
            .map(|(at, &byte)| Glyph::new(at..at + 1, BabelGuaranteedText::char_from_byte(byte)))
            .collect()
    }
}

impl Glyphs for Hex {
    fn glyphs(bytes: &[u8]) -> Vec<Glyph> {
        bytes
            .iter()
            .enumerate()
            .map(|(at, byte)| Glyph::new(at..at + 1, format!("{byte:02x}")))
            .collect()
    }
}

impl Glyphs for Base64 {
    fn glyphs(bytes: &[u8]) -> Vec<Glyph> {
        bytes
            .chunks(3)
            .enumerate()
            .map(|(at, group)| Glyph::new(at * 3..at * 3 + group.len(), Base64::encode(group)))
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn texts(glyphs: &[Glyph]) -> Vec<&str> {
        glyphs.iter().map(Glyph::text).collect()
    }

    fn ranges(glyphs: &[Glyph]) -> Vec<Range<usize>> {
        glyphs.iter().map(Glyph::bytes).collect()
    }

    /// The glyphs cover `len` bytes once each, in order.
    fn assert_tiles(glyphs: &[Glyph], len: usize) {
        let mut next = 0;
        for glyph in glyphs {
            assert_eq!(glyph.bytes().start, next);
            assert!(glyph.bytes().end > next);
            next = glyph.bytes().end;
        }
        assert_eq!(next, len);
    }

    #[test]
    fn text_is_one_table_character_per_byte() {
        let bytes = [0x48, 0x65, 0xff, 0x0a];
        let glyphs = BabelGuaranteedText::glyphs(&bytes);
        assert_eq!(ranges(&glyphs), [0..1, 1..2, 2..3, 3..4]);
        let expected: Vec<String> = bytes
            .iter()
            .map(|&b| BabelGuaranteedText::char_from_byte(b).to_string())
            .collect();
        assert_eq!(texts(&glyphs), expected);
        assert_eq!(glyphs[0].text(), "H");
    }

    #[test]
    fn hex_is_two_digits_per_byte() {
        let glyphs = Hex::glyphs(&[0x48, 0x65, 0xff, 0x0a]);
        assert_eq!(texts(&glyphs), ["48", "65", "ff", "0a"]);
        assert_eq!(ranges(&glyphs), [0..1, 1..2, 2..3, 3..4]);
    }

    #[test]
    fn base64_is_four_characters_per_three_bytes() {
        let glyphs = Base64::glyphs(&[0x48, 0x65, 0xff, 0x0a]);
        assert_eq!(texts(&glyphs), ["SGX/", "Cg=="]);
        assert_eq!(ranges(&glyphs), [0..3, 3..4]);
        // Joined, the glyphs are the whole page in base64.
        let joined: String = glyphs.iter().map(Glyph::text).collect();
        assert_eq!(joined, Base64::encode(&[0x48, 0x65, 0xff, 0x0a]));
    }

    #[test]
    fn no_glyph_text_is_ever_a_control_character() {
        let bytes: Vec<u8> = (0..=255).collect();
        for glyphs in [
            BabelGuaranteedText::glyphs(&bytes),
            Hex::glyphs(&bytes),
            Base64::glyphs(&bytes),
        ] {
            for glyph in glyphs {
                assert!(!glyph.text().is_empty());
                assert!(!glyph.text().chars().any(char::is_control));
            }
        }
    }

    #[test]
    fn glyphs_tile_every_input() {
        // A spread of bytes: every value, in several rotations and lengths.
        for shift in [0usize, 1, 7, 100] {
            for len in [0usize, 1, 2, 3, 4, 5, 63, 64, 65, 255] {
                let bytes: Vec<u8> = (0..len).map(|i| ((i + shift) * 37 % 256) as u8).collect();
                assert_tiles(&BabelGuaranteedText::glyphs(&bytes), len);
                assert_tiles(&Hex::glyphs(&bytes), len);
                assert_tiles(&Base64::glyphs(&bytes), len);
            }
        }
    }

    #[test]
    fn empty_input_has_no_glyphs() {
        assert!(BabelGuaranteedText::glyphs(&[]).is_empty());
        assert!(Hex::glyphs(&[]).is_empty());
        assert!(Base64::glyphs(&[]).is_empty());
    }
}
