//! One bookmark: a time, a label and a page index, written as one line of text.
//!
//! ```text
//! 1790812800\tlooks like a word\tn0aMceL9...\n
//! ```
//!
//! The three fields are split by position. Each line has one valid spelling, so parsing it then
//! writing it back gives the same bytes.

use bitbabel_core::PageIndex;

use crate::error::LineError;

/// The most characters a label may have.
pub const MAX_LABEL_CHARS: usize = 200;

/// Characters that are not drawn, or that change how text is ordered or broken. A label with
/// one of these can read differently from what it holds. A zero-width joiner (U+200D) is
/// allowed because emoji need it.
fn is_hidden(c: char) -> bool {
    matches!(
        c,
        '\u{00ad}' // soft hyphen
        | '\u{061c}' // Arabic letter mark
        | '\u{180e}' // Mongolian vowel separator
        | '\u{200b}' // zero-width space
        | '\u{200e}'..='\u{200f}' // left-to-right and right-to-left marks
        | '\u{2028}'..='\u{202e}' // separators, embeddings and overrides
        | '\u{2060}'..='\u{2064}' // word joiner and invisible operators
        | '\u{2066}'..='\u{2069}' // isolates
        | '\u{feff}' // zero-width no-break space
        | '\u{fff9}'..='\u{fffb}' // interlinear annotation
        | '\u{e0000}'..='\u{e007f}' // tag characters, which can smuggle invisible text
    )
}

/// A saved page: when it was saved, what it is called, and where it is.
///
/// [`new`](Self::new) does not check the index against a library; the list that holds the
/// bookmarks will.
///
/// # Example
///
/// ```
/// use bitbabel_core::PageIndex;
/// use bitbabel_file::Bookmark;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let bookmark = Bookmark::new(1_790_812_800, "a word", PageIndex::zero(3))?;
/// assert_eq!(bookmark.line(), "1790812800\ta word\tAAAA");
/// assert_eq!(Bookmark::from_line(bookmark.line().as_bytes(), 3)?, bookmark);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    time: u64,
    label: String,
    index: PageIndex,
}

impl Bookmark {
    /// `time` is unix seconds. The label may be empty.
    ///
    /// # Errors
    ///
    /// - [`LineError::InvalidLabel`] if the label has a control character (tab, newline,
    ///   escape and the rest), or one that is not drawn or that reorders text.
    /// - [`LineError::LabelTooLong`] for more than [`MAX_LABEL_CHARS`] characters.
    pub fn new(time: u64, label: &str, index: PageIndex) -> Result<Self, LineError> {
        // Checked before the copy, so a huge label is refused without being allocated twice.
        check_label(label)?;
        Ok(Bookmark {
            time,
            label: label.to_string(),
            index,
        })
    }

    /// Unix seconds.
    pub fn time(&self) -> u64 {
        self.time
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn index(&self) -> &PageIndex {
        &self.index
    }

    /// The line, without its `\n`: the form [`from_line`](Self::from_line) reads.
    pub fn line(&self) -> String {
        format!("{}\t{}\t{}", self.time, self.label, self.index.to_base64())
    }

    /// Reads one line, without its `\n`. `page_len` is the page length of the library.
    ///
    /// # Errors
    ///
    /// - [`LineError::NotText`] if the line is not UTF-8.
    /// - [`LineError::WindowsLineEndings`] if it ends in `\r`.
    /// - [`LineError::FieldCount`] unless it has exactly three tab-separated fields.
    /// - [`LineError::Time`] unless the time is the digits of a `u64`, with no leading zeros.
    /// - [`LineError::InvalidLabel`] and [`LineError::LabelTooLong`], as for
    ///   [`new`](Self::new).
    /// - [`LineError::Index`] if the index is not canonical base64 of `page_len` bytes.
    pub fn from_line(line: &[u8], page_len: usize) -> Result<Self, LineError> {
        let line = std::str::from_utf8(line).map_err(|_| LineError::NotText)?;
        if line.ends_with('\r') {
            return Err(LineError::WindowsLineEndings);
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [time, label, index] = fields[..] else {
            return Err(LineError::FieldCount {
                found: fields.len(),
            });
        };

        let value: u64 = time
            .parse()
            .map_err(|_| LineError::Time(time.to_string()))?;
        // `parse` also accepts "+5" and "007"; only how `line` writes it is valid.
        if value.to_string() != time {
            return Err(LineError::Time(time.to_string()));
        }
        // The base64 decoder is strict (padding required, spare bits zero, standard alphabet
        // only), so an index that decodes has only one spelling. A test pins this.
        let index = PageIndex::from_base64(index, page_len).map_err(LineError::Index)?;
        Bookmark::new(value, label, index)
    }
}

fn check_label(label: &str) -> Result<(), LineError> {
    if let Some(found) = label.chars().find(|&c| c.is_control() || is_hidden(c)) {
        return Err(LineError::InvalidLabel(found));
    }
    let len = label.chars().count();
    if len > MAX_LABEL_CHARS {
        return Err(LineError::LabelTooLong {
            len,
            max: MAX_LABEL_CHARS,
        });
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    const PAGE_LEN: usize = 6;

    fn index() -> PageIndex {
        PageIndex::from_bytes(vec![1, 2, 3, 4, 5, 255])
    }

    fn parse(line: &str) -> Result<Bookmark, LineError> {
        Bookmark::from_line(line.as_bytes(), PAGE_LEN)
    }

    #[test]
    fn writes_the_agreed_line() {
        let bookmark = Bookmark::new(1_790_812_800, "looks like a word", index()).unwrap();
        assert_eq!(bookmark.line(), "1790812800\tlooks like a word\tAQIDBAX/");
        let empty = Bookmark::new(0, "", index()).unwrap();
        assert_eq!(empty.line(), "0\t\tAQIDBAX/");
    }

    #[test]
    fn lines_round_trip_byte_for_byte() {
        for (time, label) in [
            (0, ""),
            (1, "a"),
            (u64::MAX, "naïve – 日本語 🙂"),
            (1_790_812_800, "has spaces  and trailing "),
        ] {
            let bookmark = Bookmark::new(time, label, index()).unwrap();
            let line = bookmark.line();
            let parsed = Bookmark::from_line(line.as_bytes(), PAGE_LEN).unwrap();
            assert_eq!(parsed, bookmark);
            assert_eq!(parsed.time(), time);
            assert_eq!(parsed.label(), label);
            assert_eq!(parsed.index(), &index());
            assert_eq!(parsed.line(), line);
        }
    }

    #[test]
    fn rejects_the_wrong_number_of_fields() {
        for (line, found) in [
            ("", 1),
            ("1", 1),
            ("1\tAQIDBAX/", 2),
            ("1\ta\tb\tAQIDBAX/", 4),
        ] {
            assert_eq!(
                parse(line),
                Err(LineError::FieldCount { found }),
                "{line:?}"
            );
        }
    }

    #[test]
    fn the_time_has_one_spelling() {
        for time in [
            "",
            "x",
            "-1",
            "+5",
            "007",
            "1.5",
            " 5",
            "18446744073709551616",
        ] {
            let line = format!("{time}\tlabel\tAQIDBAX/");
            assert_eq!(
                parse(&line),
                Err(LineError::Time(time.to_string())),
                "{time:?}"
            );
        }
        assert!(parse("0\tlabel\tAQIDBAX/").is_ok());
        assert!(parse("18446744073709551615\tlabel\tAQIDBAX/").is_ok());
    }

    #[test]
    fn labels_may_not_hold_control_hidden_or_reordering_characters() {
        for bad in [
            '\t',
            '\n',
            '\r',
            '\u{1b}',
            '\0',
            '\u{7f}',
            '\u{85}', // control
            '\u{202e}',
            '\u{2066}',
            '\u{200f}',
            '\u{2028}',
            '\u{061c}', // reordering
            '\u{00ad}',
            '\u{200b}',
            '\u{2060}',
            '\u{feff}',
            '\u{fffa}', // not drawn
            '\u{e0041}',
            '\u{e007f}', // tag characters
        ] {
            let label = format!("a{bad}b");
            assert_eq!(
                Bookmark::new(1, &label, index()),
                Err(LineError::InvalidLabel(bad)),
                "{bad:?}"
            );
        }
        // Ordinary non-ASCII text, a zero-width joiner in an emoji, and the characters just
        // outside each rejected range stay allowed.
        for ok in [
            "日本語 é 👨\u{200d}👩",
            "a\u{2027}b",
            "a\u{202f}b",
            "a\u{e0080}b",
        ] {
            assert!(Bookmark::new(1, ok, index()).is_ok(), "{ok:?}");
        }
    }

    #[test]
    fn a_label_has_at_most_200_characters() {
        let long = "é".repeat(MAX_LABEL_CHARS);
        assert!(Bookmark::new(1, &long, index()).is_ok());
        assert_eq!(
            Bookmark::new(1, &format!("{long}x"), index()),
            Err(LineError::LabelTooLong {
                len: MAX_LABEL_CHARS + 1,
                max: MAX_LABEL_CHARS
            })
        );
    }

    #[test]
    fn a_parsed_label_is_checked_too() {
        assert_eq!(
            parse("1\ta\u{1b}[2Jb\tAQIDBAX/"),
            Err(LineError::InvalidLabel('\u{1b}'))
        );
    }

    #[test]
    fn rejects_windows_line_endings_and_other_bytes() {
        assert_eq!(
            parse("1\tlabel\tAQIDBAX/\r"),
            Err(LineError::WindowsLineEndings)
        );
        assert_eq!(
            Bookmark::from_line(b"1\t\xff\tAQIDBAX/", PAGE_LEN),
            Err(LineError::NotText)
        );
    }

    #[test]
    fn rejects_bad_indices() {
        // Not base64, and right text but the wrong length for the library.
        for bad in ["!!!!", "AQID", ""] {
            let line = format!("1\tlabel\t{bad}");
            assert!(matches!(parse(&line), Err(LineError::Index(_))), "{bad:?}");
        }
    }

    /// Each index has one spelling because the decoder refuses every other one. This test is
    /// what `from_line` relies on instead of re-encoding and comparing.
    #[test]
    fn only_the_canonical_spelling_of_an_index_decodes() {
        let line = |index: &str| Bookmark::from_line(format!("1\tl\t{index}").as_bytes(), 5);
        assert!(line("AQIDBAU=").is_ok());
        for other in [
            "AQIDBAU",   // padding left off
            "AQIDBAV=",  // spare bits set
            "AQIDBAU==", // extra padding
            "AQIDBAU= ", // trailing space
            "AQIDBA-_",  // url-safe alphabet
        ] {
            assert!(matches!(line(other), Err(LineError::Index(_))), "{other:?}");
        }
    }

    #[test]
    fn error_messages_are_readable() {
        assert_eq!(
            LineError::InvalidLabel('\u{1b}').to_string(),
            "label contains a control, hidden or text-reordering character U+001B"
        );
        assert_eq!(
            LineError::FieldCount { found: 2 }.to_string(),
            "a bookmark line must have 3 tab-separated fields, found 2"
        );
    }
}
