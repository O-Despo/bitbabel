//! Turns what the user typed into the search needle: the bytes to look for.
//!
//! The needle is written in the current view's encoding, so what you type matches what you see:
//!
//! | View | You type | Needle bytes |
//! |---|---|---|
//! | text | `hello` | each character's byte from the BitBabel text table |
//! | utf8 | `héllo` | the UTF-8 bytes of the text |
//! | hex | `68656c6c6f` | the digits, two per byte |
//! | base64 | `aGVsbG8=` | the decoded bytes (padding required) |

use bitbabel_core::{BabelGuaranteedText, Base64, Encoding, Hex};

use crate::render::View;

/// The bytes `text` stands for in `view`.
///
/// # Errors
///
/// A message for the search box: the text is empty, has a character the view cannot write, or
/// is not valid hex or base64. It never panics and never changes anything.
pub fn parse(view: View, text: &str) -> Result<Vec<u8>, String> {
    if text.is_empty() {
        return Err("type something to search for".to_string());
    }
    match view {
        View::Text => text
            .chars()
            .map(|c| {
                BabelGuaranteedText::byte_from_char(c).ok_or_else(|| {
                    format!(
                        "{c:?} is not in the BitBabel text table: esc, then e for the utf8 view"
                    )
                })
            })
            .collect(),
        View::Utf8 => Ok(text.as_bytes().to_vec()),
        View::Hex => parse_hex(text),
        View::Base64 => parse_base64(text),
    }
}

fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    // A bad digit is reported before the length, so `g` says `g` and not "odd".
    if let Some(bad) = text.chars().find(|c| !c.is_ascii_hexdigit()) {
        return Err(format!("invalid hex digit {bad:?}"));
    }
    if !text.len().is_multiple_of(2) {
        return Err("hex needle needs an even number of digits".to_string());
    }
    Hex::decode(&text.to_string()).map_err(|error| error.to_string())
}

fn parse_base64(text: &str) -> Result<Vec<u8>, String> {
    if !text.chars().count().is_multiple_of(4) {
        return Err(
            "base64 needle must be padded to a multiple of 4 characters (add =)".to_string(),
        );
    }
    Base64::decode(&text.to_string()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn text_uses_the_table_character_for_each_byte() {
        // ASCII is itself in the table.
        assert_eq!(parse(View::Text, "hello"), Ok(b"hello".to_vec()));
        // `À` is byte 0, and `♦` is byte 255.
        assert_eq!(parse(View::Text, "À♦"), Ok(vec![0x00, 0xff]));
    }

    #[test]
    fn text_names_the_character_that_is_not_in_the_table() {
        assert_eq!(
            parse(View::Text, "a中"),
            Err(
                "'中' is not in the BitBabel text table: esc, then e for the utf8 view".to_string()
            )
        );
    }

    #[test]
    fn utf8_is_the_text_bytes() {
        assert_eq!(
            parse(View::Utf8, "héllo"),
            Ok(vec![0x68, 0xc3, 0xa9, 0x6c, 0x6c, 0x6f])
        );
        assert_eq!(parse(View::Utf8, "中"), Ok("中".as_bytes().to_vec()));
    }

    #[test]
    fn hex_decodes_either_case() {
        assert_eq!(parse(View::Hex, "68656c6c6f"), Ok(b"hello".to_vec()));
        assert_eq!(parse(View::Hex, "FFa0"), Ok(vec![0xff, 0xa0]));
    }

    #[test]
    fn hex_reports_a_bad_digit_before_an_odd_length() {
        assert_eq!(
            parse(View::Hex, "g"),
            Err("invalid hex digit 'g'".to_string())
        );
        assert_eq!(
            parse(View::Hex, "6865z"),
            Err("invalid hex digit 'z'".to_string())
        );
        assert_eq!(
            parse(View::Hex, "68656"),
            Err("hex needle needs an even number of digits".to_string())
        );
    }

    #[test]
    fn base64_needs_padding_and_valid_characters() {
        assert_eq!(parse(View::Base64, "aGVsbG8="), Ok(b"hello".to_vec()));
        assert_eq!(
            parse(View::Base64, "aGVsbG8"),
            Err("base64 needle must be padded to a multiple of 4 characters (add =)".to_string())
        );
        assert!(
            parse(View::Base64, "aGVs!G8=")
                .unwrap_err()
                .starts_with("invalid base64")
        );
    }

    #[test]
    fn an_empty_needle_is_an_error_in_every_view() {
        for view in [View::Text, View::Hex, View::Base64, View::Utf8] {
            assert_eq!(
                parse(view, ""),
                Err("type something to search for".to_string())
            );
        }
    }
}
