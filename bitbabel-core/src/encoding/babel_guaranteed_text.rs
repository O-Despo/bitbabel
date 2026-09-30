use std::error::Error;
use std::fmt;

use super::Encoding;

/// A fixed 256-slot table that shows every byte as one distinct, visible character.
///
/// Random bytes are mostly invalid as UTF-8 or UTF-16, so [`Utf8`](crate::Utf8) and the
/// UTF-16 encodings replace most of them with `U+FFFD`. This encoding never does: byte `b`
/// is always shown as [`char_from_byte(b)`](Self::char_from_byte), so `N` bytes become exactly
/// `N` characters and [`decode`](Encoding::decode) gives the same bytes back.
///
/// # The table
///
/// - Bytes 32 to 126 are their ASCII characters, space through `~`.
/// - The other 161 bytes (0 to 31 and 127 to 255) get a curated palette, filled in
///   ascending byte order: 60 international Latin letters, 39 mathematics and logic symbols,
///   34 Greek letters, 16 typography marks, and 12 symbols. So byte 0 is `À`, byte 127 is `â`,
///   and byte 255 is `♦`.
/// - No character appears twice, and none is a control code or invisible.
///
/// A few characters the palette could have used are left out because they look identical to
/// ones it does use: `∆` (Greek `Δ` is in), `∑` and `∏` (Greek `Σ` and `Π`), and the micro
/// sign and ohm sign (Greek `μ` and `Ω`).
///
/// The table is frozen: changing it would change what every saved page looks like.
///
/// # Example
///
/// ```
/// use bitbabel_core::{BabelGuaranteedText, BabelLibrary, Encoding, LibraryConfig, PageIndex};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = LibraryConfig::MEDIUM;
/// let library = BabelLibrary::canonical(config)?;
/// let page = library.page_at(&PageIndex::from_u64(42, config.page_len()))?;
///
/// // One character per byte, so a MEDIUM page is exactly 3200 characters.
/// let text = page.encode_as::<BabelGuaranteedText>();
/// assert_eq!(text.chars().count(), 3200);
///
/// // And the text leads straight back to the page's index.
/// let bytes = BabelGuaranteedText::decode(&text)?;
/// assert_eq!(library.index_of(&bytes)?, *page.index());
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct BabelGuaranteedText;

impl BabelGuaranteedText {
    /// The character that shows `byte`.
    pub const fn char_from_byte(byte: u8) -> char {
        // A `u8` is at most 255, so this index is always in range.
        ALPHABET[byte as usize]
    }

    /// The byte that `ch` shows, or `None` if `ch` is not one of the 256 table characters.
    pub fn byte_from_char(ch: char) -> Option<u8> {
        match REVERSE.binary_search_by_key(&ch, |&(c, _)| c) {
            Ok(i) => REVERSE.get(i).map(|&(_, byte)| byte),
            Err(_) => None,
        }
    }
}

/// A character in the text is not one of the 256 that [`BabelGuaranteedText`] uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BabelGuaranteedTextDecodeError {
    /// `ch` is at `position` (a 0-based character index, not a byte offset).
    InvalidChar { ch: char, position: usize },
}

impl fmt::Display for BabelGuaranteedTextDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BabelGuaranteedTextDecodeError::InvalidChar { ch, position } => write!(
                f,
                "character {ch:?} at position {position} is not in the BabelGuaranteedText table"
            ),
        }
    }
}

impl Error for BabelGuaranteedTextDecodeError {}

impl Encoding for BabelGuaranteedText {
    type Output = String;
    type DecodeError = BabelGuaranteedTextDecodeError;

    fn encode(bytes: &[u8]) -> Self::Output {
        bytes.iter().map(|&b| Self::char_from_byte(b)).collect()
    }

    fn decode(value: &Self::Output) -> Result<Vec<u8>, Self::DecodeError> {
        value
            .chars()
            .enumerate()
            .map(|(position, ch)| {
                Self::byte_from_char(ch)
                    .ok_or(BabelGuaranteedTextDecodeError::InvalidChar { ch, position })
            })
            .collect()
    }
}

// The palette below is spread across the byte values that ASCII does not cover, in this
// order: Latin, mathematics, Greek, typography, symbols.

/// International Latin: Latin-1 letters with diacritics, minus thorn (no diacritic).
const LATIN: [char; 60] = [
    'À', 'Á', 'Â', 'Ã', 'Ä', 'Å', 'Æ', 'Ç', 'È', 'É', 'Ê', 'Ë', 'Ì', 'Í', 'Î', 'Ï', 'Ð', 'Ñ', 'Ò',
    'Ó', 'Ô', 'Õ', 'Ö', 'Ø', 'Ù', 'Ú', 'Û', 'Ü', 'Ý', 'ß', 'à', 'á', 'â', 'ã', 'ä', 'å', 'æ', 'ç',
    'è', 'é', 'ê', 'ë', 'ì', 'í', 'î', 'ï', 'ð', 'ñ', 'ò', 'ó', 'ô', 'õ', 'ö', 'ø', 'ù', 'ú', 'û',
    'ü', 'ý', 'ÿ',
];

/// Mathematics and logic: fractions, arithmetic, calculus, relations, set theory, logic, arrows.
const MATH: [char; 39] = [
    '¼', '½', '¾', '±', '×', '÷', '∞', '∫', '∂', '∇', '√', '∝', '≠', '≈', '≤', '≥', '≡', '≅', '∈',
    '∉', '∋', '⊂', '⊃', '∪', '∩', '∅', '⊆', '⊇', '∀', '∃', '∧', '∨', '¬', '←', '→', '↑', '↓', '↔',
    '⇒',
];

/// Greek: 24 lowercase, then the 10 uppercase letters that don't look like Latin ones.
const GREEK: [char; 34] = [
    'α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ', 'λ', 'μ', 'ν', 'ξ', 'ο', 'π', 'ρ', 'σ', 'τ',
    'υ', 'φ', 'χ', 'ψ', 'ω', 'Γ', 'Δ', 'Θ', 'Λ', 'Ξ', 'Π', 'Σ', 'Φ', 'Ψ', 'Ω',
];

/// Smart typography: quotes, dashes, and editorial marks.
const TYPOGRAPHY: [char; 16] = [
    '«', '»', '‹', '›', '“', '”', '‘', '’', '„', '–', '—', '†', '‡', '•', '…', '‰',
];

/// Symbols: currency, legal marks, card suits.
const SYMBOLS: [char; 12] = ['€', '£', '¥', '©', '®', '™', '§', '¶', '♠', '♣', '♥', '♦'];

/// Number of bytes outside 32..=126, which the palette fills.
const PALETTE_LEN: usize = 161;

/// The five groups joined in order.
const PALETTE: [char; PALETTE_LEN] = {
    let groups: [&[char]; 5] = [&LATIN, &MATH, &GREEK, &TYPOGRAPHY, &SYMBOLS];
    let mut palette = ['\0'; PALETTE_LEN];
    let mut filled = 0;
    let mut group = 0;
    while group < groups.len() {
        let mut i = 0;
        while i < groups[group].len() {
            palette[filled] = groups[group][i];
            filled += 1;
            i += 1;
        }
        group += 1;
    }
    // Fails the build if the groups do not add up to exactly 161.
    assert!(filled == PALETTE_LEN);
    palette
};

/// Byte to character: ASCII where it is printable, the palette everywhere else.
const fn build_alphabet() -> [char; 256] {
    let mut alphabet = ['\0'; 256];
    let mut next_palette = 0;
    let mut byte = 0;
    while byte < 256 {
        if byte >= 32 && byte <= 126 {
            alphabet[byte] = byte as u8 as char;
        } else {
            alphabet[byte] = PALETTE[next_palette];
            next_palette += 1;
        }
        byte += 1;
    }
    assert!(next_palette == PALETTE_LEN);
    alphabet
}

/// Character to byte: every `(character, byte)` pair, sorted by character so it can be
/// binary searched. Built while compiling, so it needs no setup or allocation at runtime.
const fn build_reverse(alphabet: &[char; 256]) -> [(char, u8); 256] {
    let mut table = [('\0', 0u8); 256];
    let mut i = 0;
    while i < 256 {
        // Insertion sort: shift larger entries right, then drop the new one in.
        let entry = (alphabet[i], i as u8);
        let mut j = i;
        while j > 0 && table[j - 1].0 > entry.0 {
            table[j] = table[j - 1];
            j -= 1;
        }
        table[j] = entry;
        i += 1;
    }
    table
}

const ALPHABET_TABLE: [char; 256] = build_alphabet();

// Statics so every lookup reads one table in the binary instead of copying it.
static ALPHABET: [char; 256] = ALPHABET_TABLE;
static REVERSE: [(char, u8); 256] = build_reverse(&ALPHABET_TABLE);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn every_byte_maps_to_a_visible_character() {
        for byte in 0..=255u8 {
            let ch = BabelGuaranteedText::char_from_byte(byte);
            assert!(!ch.is_control(), "byte {byte} maps to a control character");
            assert!(
                !ch.is_whitespace() || ch == ' ',
                "byte {byte} maps to invisible whitespace"
            );
        }
    }

    #[test]
    fn every_byte_round_trips() {
        // A repeated character would make one of these fail, so this also proves no duplicates.
        for byte in 0..=255u8 {
            let ch = BabelGuaranteedText::char_from_byte(byte);
            assert_eq!(BabelGuaranteedText::byte_from_char(ch), Some(byte));
        }

        let all: Vec<u8> = (0..=255).collect();
        let text = BabelGuaranteedText::encode(&all);
        assert_eq!(text.chars().count(), 256);
        assert_eq!(BabelGuaranteedText::decode(&text).unwrap(), all);
    }

    #[test]
    fn printable_ascii_is_preserved() {
        for byte in 32..=126u8 {
            assert_eq!(BabelGuaranteedText::char_from_byte(byte), char::from(byte));
        }
    }

    #[test]
    fn decode_rejects_characters_outside_the_table() {
        assert_eq!(
            BabelGuaranteedText::decode(&"A\u{0}B".to_string()),
            Err(BabelGuaranteedTextDecodeError::InvalidChar {
                ch: '\0',
                position: 1
            })
        );
        assert_eq!(BabelGuaranteedText::decode(&String::new()), Ok(vec![]));
    }
}
