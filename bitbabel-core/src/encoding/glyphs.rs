//! A display view of page bytes that remembers which bytes each piece came from.
//!
//! [`Encoding`](super::Encoding) turns bytes into one string, which loses where each
//! character came from, and [`Utf8`] replaces bad bytes in a way that shifts every later
//! offset. A viewer that wants to wrap lines or highlight a byte range needs the offsets, so
//! [`Glyphs`] gives a list of [`Glyph`]s instead.

use std::ops::Range;

use super::{BabelGuaranteedText, Base64, Encoding, Hex, Utf8};

/// What shows in place of a byte or character that cannot be shown.
const REPLACEMENT: &str = "·";

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
/// | [`Utf8`] | character | `H` `e` `·` `·` |
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

/// Whether a character can be drawn as itself: `char::escape_debug` leaves it alone.
///
/// That keeps letters, symbols, CJK and emoji, and rejects control characters, combining
/// marks, zero-width and bidirectional controls, private use and unassigned code points.
/// `'`, `"` and `\` are drawn as themselves, though `escape_debug` would escape them. The
/// set follows the Unicode version of the Rust compiler, so it can shift slightly between
/// compilers. That only changes how a page is drawn, never any page.
fn is_printable(c: char) -> bool {
    matches!(c, '\'' | '"' | '\\') || c.escape_debug().eq(std::iter::once(c))
}

impl Glyphs for Utf8 {
    /// Every byte that is not valid UTF-8 is a `·` of its own, so offsets stay exact. A valid
    /// character that cannot be drawn is one `·` over all its bytes.
    fn glyphs(bytes: &[u8]) -> Vec<Glyph> {
        let mut glyphs = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            let (valid, bad_len) = match std::str::from_utf8(&bytes[at..]) {
                Ok(text) => (text, 0),
                Err(error) => {
                    let valid_len = error.valid_up_to();
                    // Safe to slice: `valid_up_to` ends on a character boundary.
                    let valid = std::str::from_utf8(&bytes[at..at + valid_len]).unwrap_or("");
                    // `None` is a sequence cut off by the end of the input.
                    let bad_len = error.error_len().unwrap_or(bytes.len() - at - valid_len);
                    (valid, bad_len)
                }
            };
            for c in valid.chars() {
                let end = at + c.len_utf8();
                let text = if is_printable(c) {
                    c.to_string()
                } else {
                    REPLACEMENT.to_string()
                };
                glyphs.push(Glyph::new(at..end, text));
                at = end;
            }
            for _ in 0..bad_len {
                glyphs.push(Glyph::new(at..at + 1, REPLACEMENT));
                at += 1;
            }
        }
        glyphs
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
    fn utf8_shows_text_and_marks_each_bad_byte() {
        let glyphs = Utf8::glyphs(&[0x48, 0x65, 0xff, 0x0a]);
        assert_eq!(texts(&glyphs), ["H", "e", "·", "·"]);
        assert_eq!(ranges(&glyphs), [0..1, 1..2, 2..3, 3..4]);
    }

    #[test]
    fn utf8_keeps_multibyte_characters_whole() {
        let glyphs = Utf8::glyphs("é中📚".as_bytes());
        assert_eq!(texts(&glyphs), ["é", "中", "📚"]);
        assert_eq!(ranges(&glyphs), [0..2, 2..5, 5..9]);
    }

    #[test]
    fn utf8_bad_bytes_do_not_shift_later_offsets() {
        // Two stray continuation bytes, then "ok". Lossy decoding would merge or move them.
        let glyphs = Utf8::glyphs(&[0x80, 0x80, b'o', b'k']);
        assert_eq!(texts(&glyphs), ["·", "·", "o", "k"]);
        assert_eq!(ranges(&glyphs), [0..1, 1..2, 2..3, 3..4]);
    }

    #[test]
    fn utf8_cut_off_sequence_is_one_dot_per_byte() {
        // The first three bytes of a four-byte character, at the end of the input.
        let glyphs = Utf8::glyphs(&[b'a', 0xf0, 0x9f, 0x93]);
        assert_eq!(texts(&glyphs), ["a", "·", "·", "·"]);
        assert_tiles(&glyphs, 4);
    }

    #[test]
    fn utf8_bad_start_then_valid_text() {
        // 0xe2 starts a three-byte character, but `a` is not a continuation byte.
        let glyphs = Utf8::glyphs(&[0xe2, b'a']);
        assert_eq!(texts(&glyphs), ["·", "a"]);
    }

    #[test]
    fn utf8_unprintable_characters_are_one_dot_over_all_their_bytes() {
        for (text, len) in [
            ("\u{0}", 1),
            ("\u{1b}", 1),
            ("\u{7f}", 1),
            ("\n", 1),
            ("\t", 1),
            ("\u{85}", 2),
            ("\u{a0}", 2),
            ("\u{301}", 2),
            ("\u{200b}", 3),
            ("\u{202e}", 3),
            ("\u{feff}", 3),
            ("\u{fe0f}", 3),
            ("\u{e000}", 3),
            ("\u{e0001}", 4),
        ] {
            let glyphs = Utf8::glyphs(text.as_bytes());
            assert_eq!(texts(&glyphs), ["·"], "{text:?}");
            assert_eq!(glyphs[0].bytes(), 0..len, "{text:?}");
        }
    }

    #[test]
    fn utf8_quotes_and_backslash_show_as_themselves() {
        let glyphs = Utf8::glyphs(b"'\"\\ ");
        assert_eq!(texts(&glyphs), ["'", "\"", "\\", " "]);
    }

    #[test]
    fn utf8_printable_characters_are_kept() {
        for text in ["a", "Z", "é", "中", "📚", "م", "~"] {
            assert_eq!(texts(&Utf8::glyphs(text.as_bytes())), [text]);
        }
    }

    #[test]
    fn no_glyph_text_is_ever_a_control_character() {
        let bytes: Vec<u8> = (0..=255).collect();
        for glyphs in [
            BabelGuaranteedText::glyphs(&bytes),
            Hex::glyphs(&bytes),
            Base64::glyphs(&bytes),
            Utf8::glyphs(&bytes),
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
                assert_tiles(&Utf8::glyphs(&bytes), len);
            }
        }
    }

    #[test]
    fn empty_input_has_no_glyphs() {
        assert!(BabelGuaranteedText::glyphs(&[]).is_empty());
        assert!(Hex::glyphs(&[]).is_empty());
        assert!(Base64::glyphs(&[]).is_empty());
        assert!(Utf8::glyphs(&[]).is_empty());
    }
}
