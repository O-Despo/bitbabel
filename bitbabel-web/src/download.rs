//! The link a `[ save ]` or `[ download ]` button points at: the file's text inside the link
//! itself, so no file API is needed.

/// A `data:` link holding `text`. Every byte that is not a letter, digit or one of `-._~` is
/// `%`-escaped, so labels with spaces, `#`, `%` or non-ASCII text survive.
pub fn data_link(text: &str) -> String {
    let mut link = String::from("data:text/plain;charset=utf-8,");
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            link.push(char::from(byte));
        } else {
            link.push_str(&format!("%{byte:02X}"));
        }
    }
    link
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn plain_text_is_left_alone() {
        assert_eq!(
            data_link("abc-1.2_3~"),
            "data:text/plain;charset=utf-8,abc-1.2_3~"
        );
    }

    #[test]
    fn base64_characters_and_whitespace_are_escaped() {
        assert_eq!(
            data_link("a+b/c=\n\t"),
            "data:text/plain;charset=utf-8,a%2Bb%2Fc%3D%0A%09"
        );
    }

    #[test]
    fn labels_with_hashes_percents_and_non_ascii_survive() {
        assert_eq!(
            data_link("# 50% é"),
            "data:text/plain;charset=utf-8,%23%2050%25%20%C3%A9"
        );
    }
}
