//! The small rules both header kinds (`.babel` and bookmark files) are written with.

use bitbabel_core::{Encoding, Hex};

/// A header word that is not the field expected at its position.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct WrongField {
    pub(crate) expected: &'static str,
    pub(crate) found: String,
}

/// The values of header words written `name=value`, where `names` are the field names in
/// order. Stops at the first word that is not the next field. `words` must have at most as
/// many words as `names`.
pub(crate) fn field_values<'a>(
    words: &[&'a str],
    names: &[&'static str],
) -> Result<Vec<&'a str>, WrongField> {
    words
        .iter()
        .zip(names)
        .map(|(word, &expected)| {
            word.strip_prefix(expected)
                .and_then(|rest| rest.strip_prefix('='))
                .ok_or_else(|| WrongField {
                    expected,
                    found: word.to_string(),
                })
        })
        .collect()
}

/// `N` bytes from exactly `2 * N` lowercase hex digits. Hex decoding also accepts uppercase,
/// so the text is written back and compared: a header has one valid spelling.
pub(crate) fn lowercase_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let bytes = Hex::decode(&text.to_string()).ok()?;
    let value = <[u8; N]>::try_from(bytes).ok()?;
    (Hex::encode(&value) == text).then_some(value)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn field_values_strips_the_names() {
        assert_eq!(
            field_values(&["size=small", "key=custom"], &["size", "key", "format"]),
            Ok(vec!["small", "custom"])
        );
    }

    #[test]
    fn field_values_names_the_first_wrong_word() {
        let wrong = |expected, found: &str| WrongField {
            expected,
            found: found.to_string(),
        };
        let names = ["size", "key"];
        assert_eq!(
            field_values(&["key=custom", "size=small"], &names),
            Err(wrong("size", "key=custom"))
        );
        // A name that only starts like the expected one, and a missing `=`.
        assert_eq!(
            field_values(&["sizes=small"], &names),
            Err(wrong("size", "sizes=small"))
        );
        assert_eq!(field_values(&["size"], &names), Err(wrong("size", "size")));
        assert_eq!(
            field_values(&["size=small", ""], &names),
            Err(wrong("key", ""))
        );
    }

    #[test]
    fn lowercase_hex_accepts_one_spelling_of_exactly_n_bytes() {
        assert_eq!(lowercase_hex::<2>("0aff"), Some([0x0a, 0xff]));
        assert_eq!(lowercase_hex::<2>("0AFF"), None);
        assert_eq!(lowercase_hex::<2>("0a"), None);
        assert_eq!(lowercase_hex::<2>("0aff00"), None);
        assert_eq!(lowercase_hex::<2>("0a-ff"), None);
        assert_eq!(lowercase_hex::<2>(""), None);
    }
}
