//! A whole bookmark file: its settings and its list of bookmarks.

use std::collections::HashMap;

use bitbabel_core::PageIndex;

use crate::bookmarks::header::BookmarkSettings;
use crate::bookmarks::line::{Bookmark, MAX_LABEL_CHARS};
use crate::error::{BookmarkError, LineError};

/// The most bookmarks one file may hold. Every change rewrites the whole file, so this keeps
/// a write to a few tens of megabytes at most.
pub const MAX_BOOKMARKS: usize = 10_000;

/// The longest header line, `\n` included: the magic word, a custom key's fingerprint and a
/// size name with room to spare.
const MAX_HEADER_BYTES: usize = 96;

/// The settings of a bookmark file and its bookmarks, in the order they were added.
///
/// Every bookmark's index is one page of the file's library, and no index appears twice.
/// Parsing a file then writing it back gives the same bytes.
///
/// # Example
///
/// ```
/// use bitbabel_core::{LibraryConfig, PageIndex};
/// use bitbabel_file::{BookmarkFile, BookmarkKey, BookmarkSettings};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let settings = BookmarkSettings::new(LibraryConfig::SMALL, BookmarkKey::Canonical)?;
/// let mut file = BookmarkFile::new(settings);
/// file.push(1_790_812_800, "first", PageIndex::from_u64(1, 16))?;
///
/// let bytes = file.to_bytes();
/// assert_eq!(BookmarkFile::from_bytes(&bytes)?, file);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkFile {
    settings: BookmarkSettings,
    bookmarks: Vec<Bookmark>,
}

impl BookmarkFile {
    /// A file with no bookmarks.
    pub fn new(settings: BookmarkSettings) -> Self {
        BookmarkFile {
            settings,
            bookmarks: Vec::new(),
        }
    }

    pub fn settings(&self) -> BookmarkSettings {
        self.settings
    }

    pub fn bookmarks(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    /// The position of the bookmark for `index`, if there is one.
    pub fn find(&self, index: &PageIndex) -> Option<usize> {
        self.bookmarks.iter().position(|b| b.index() == index)
    }

    /// The most bytes a file of this library can have, so a reader can refuse a bigger one
    /// before reading it all.
    pub fn max_bytes(settings: &BookmarkSettings) -> usize {
        let base64_len = settings.size().page_len().div_ceil(3) * 4;
        // time (a u64 has at most 20 digits), tab, label (up to 4 bytes a character), tab,
        // index, newline.
        let line = 20 + 1 + MAX_LABEL_CHARS * 4 + 1 + base64_len + 1;
        MAX_HEADER_BYTES + MAX_BOOKMARKS * line
    }

    /// Adds a bookmark at the end. `time` is unix seconds.
    ///
    /// # Errors
    ///
    /// - [`BookmarkError::Invalid`] for a label that [`Bookmark::new`] refuses.
    /// - [`BookmarkError::IndexLen`] if `index` is not one page of this library.
    /// - [`BookmarkError::DuplicateIndex`] if the file already has this index.
    /// - [`BookmarkError::TooManyBookmarks`] if the file already has [`MAX_BOOKMARKS`].
    ///
    /// The file is unchanged on an error.
    pub fn push(&mut self, time: u64, label: &str, index: PageIndex) -> Result<(), BookmarkError> {
        let bookmark = Bookmark::new(time, label, index)?;
        let expected = self.settings.size().page_len();
        let actual = bookmark.index().as_bytes().len();
        if actual != expected {
            return Err(BookmarkError::IndexLen { expected, actual });
        }
        if let Some(first) = self.find(bookmark.index()) {
            return Err(BookmarkError::DuplicateIndex {
                first_line: line_of(first),
                line: line_of(self.bookmarks.len()),
            });
        }
        if self.bookmarks.len() >= MAX_BOOKMARKS {
            return Err(BookmarkError::TooManyBookmarks { max: MAX_BOOKMARKS });
        }
        self.bookmarks.push(bookmark);
        Ok(())
    }

    /// Changes the label of the bookmark at `position`.
    ///
    /// # Errors
    ///
    /// [`BookmarkError::NoSuchBookmark`] if there is none there, or
    /// [`BookmarkError::Invalid`] for a bad label. The file is unchanged on an error.
    pub fn rename(&mut self, position: usize, label: &str) -> Result<(), BookmarkError> {
        let len = self.bookmarks.len();
        let old = self
            .bookmarks
            .get(position)
            .ok_or(BookmarkError::NoSuchBookmark { position, len })?;
        let renamed = Bookmark::new(old.time(), label, old.index().clone())?;
        self.bookmarks[position] = renamed;
        Ok(())
    }

    /// Takes out and returns the bookmark at `position`. Later ones move up by one.
    ///
    /// # Errors
    ///
    /// [`BookmarkError::NoSuchBookmark`] if there is none there.
    pub fn remove(&mut self, position: usize) -> Result<Bookmark, BookmarkError> {
        let len = self.bookmarks.len();
        if position >= len {
            return Err(BookmarkError::NoSuchBookmark { position, len });
        }
        Ok(self.bookmarks.remove(position))
    }

    /// The file: the header, then one line per bookmark.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut text = self.settings.header();
        for bookmark in &self.bookmarks {
            text.push_str(&bookmark.line());
            text.push('\n');
        }
        text.into_bytes()
    }

    /// Reads a file. Line 1 is the header and bookmark `n` (counting from 0) is line `n + 2`.
    ///
    /// # Errors
    ///
    /// - The header errors of [`BookmarkSettings::from_header`].
    /// - [`BookmarkError::MissingFinalNewline`] if the last line has no `\n`.
    /// - [`BookmarkError::WindowsLineEndings`] for a line that ends in `\r`, in the header or
    ///   after it.
    /// - [`BookmarkError::Line`] for any other bad line, with its number.
    /// - [`BookmarkError::DuplicateIndex`] for an index that appears twice.
    /// - [`BookmarkError::TooManyBookmarks`] for more than [`MAX_BOOKMARKS`] lines.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BookmarkError> {
        let (settings, body) = BookmarkSettings::from_header(bytes)?;
        let page_len = settings.size().page_len();

        let mut file = BookmarkFile::new(settings);
        if !body.is_empty() {
            let body = body
                .strip_suffix(b"\n")
                .ok_or(BookmarkError::MissingFinalNewline)?;
            for (position, text) in body.split(|&b| b == b'\n').enumerate() {
                if position >= MAX_BOOKMARKS {
                    return Err(BookmarkError::TooManyBookmarks { max: MAX_BOOKMARKS });
                }
                let bookmark =
                    Bookmark::from_line(text, page_len).map_err(|error| match error {
                        // The same error as in the header, so a caller shows one hint.
                        LineError::WindowsLineEndings => BookmarkError::WindowsLineEndings,
                        error => BookmarkError::Line {
                            line: line_of(position),
                            error,
                        },
                    })?;
                file.bookmarks.push(bookmark);
            }
        }
        file.check_no_duplicates()?;
        Ok(file)
    }

    /// Looks for an index that appears twice, with one pass over the list.
    fn check_no_duplicates(&self) -> Result<(), BookmarkError> {
        let mut seen: HashMap<&[u8], usize> = HashMap::with_capacity(self.bookmarks.len());
        for (position, bookmark) in self.bookmarks.iter().enumerate() {
            if let Some(first) = seen.insert(bookmark.index().as_bytes(), position) {
                return Err(BookmarkError::DuplicateIndex {
                    first_line: line_of(first),
                    line: line_of(position),
                });
            }
        }
        Ok(())
    }
}

/// The file line of the bookmark at `position`: the header is line 1.
fn line_of(position: usize) -> usize {
    position + 2
}

#[cfg(test)]
mod test {
    use bitbabel_core::LibraryConfig;

    use super::*;
    use crate::bookmarks::header::BookmarkKey;

    const PAGE_LEN: usize = 16;

    fn settings() -> BookmarkSettings {
        BookmarkSettings::new(LibraryConfig::SMALL, BookmarkKey::Canonical).unwrap()
    }

    fn index(n: u64) -> PageIndex {
        PageIndex::from_u64(n, PAGE_LEN)
    }

    fn file_of(count: u64) -> BookmarkFile {
        let mut file = BookmarkFile::new(settings());
        for n in 1..=count {
            file.push(n, &format!("label {n}"), index(n)).unwrap();
        }
        file
    }

    /// The bytes of a small canonical file with three bookmarks. If this changes, files on
    /// people's disks stop reading.
    const GOLDEN: &str = "BABELMARKS1 size=small key=canonical format=base64\n\
        1790812800\tlooks like a word\tAAAAAAAAAAAAAAAAAAAAAQ==\n\
        1790812955\t\tAAAAAAAAAAAAAAAAAAAAAg==\n\
        0\tnaïve – 日本語\tAAAAAAAAAAAAAAAAAAAAAw==\n";

    fn golden_file() -> BookmarkFile {
        let mut file = BookmarkFile::new(settings());
        file.push(1_790_812_800, "looks like a word", index(1))
            .unwrap();
        file.push(1_790_812_955, "", index(2)).unwrap();
        file.push(0, "naïve – 日本語", index(3)).unwrap();
        file
    }

    #[test]
    fn the_file_bytes_are_pinned() {
        assert_eq!(golden_file().to_bytes(), GOLDEN.as_bytes());
        let parsed = BookmarkFile::from_bytes(GOLDEN.as_bytes());
        assert_eq!(parsed.unwrap(), golden_file());
    }

    #[test]
    fn the_golden_file_with_a_custom_key() {
        let fingerprint = [0x3f, 0x9a, 0x1c, 0x07, 0xe2, 0xb8, 0x4d, 0x10];
        let settings =
            BookmarkSettings::new(LibraryConfig::SMALL, BookmarkKey::Custom(fingerprint)).unwrap();
        let mut file = BookmarkFile::new(settings);
        file.push(5, "x", index(1)).unwrap();
        let bytes =
            b"BABELMARKS1 size=small key=custom fingerprint=3f9a1c07e2b84d10 format=base64\n\
            5\tx\tAAAAAAAAAAAAAAAAAAAAAQ==\n";
        assert_eq!(file.to_bytes(), bytes);
        assert_eq!(BookmarkFile::from_bytes(bytes).unwrap(), file);
    }

    #[test]
    fn a_file_with_no_bookmarks_is_valid() {
        let file = BookmarkFile::new(settings());
        let bytes = file.to_bytes();
        assert_eq!(
            bytes,
            b"BABELMARKS1 size=small key=canonical format=base64\n"
        );
        assert_eq!(BookmarkFile::from_bytes(&bytes).unwrap(), file);
        assert!(file.bookmarks().is_empty());
    }

    #[test]
    fn push_keeps_order_and_find_locates_an_index() {
        let file = file_of(3);
        let labels: Vec<_> = file.bookmarks().iter().map(|b| b.label()).collect();
        assert_eq!(labels, ["label 1", "label 2", "label 3"]);
        assert_eq!(file.find(&index(2)), Some(1));
        assert_eq!(file.find(&index(9)), None);
    }

    #[test]
    fn push_refuses_a_wrong_length_index_a_bad_label_and_a_duplicate() {
        let mut file = file_of(2);
        let before = file.clone();

        assert_eq!(
            file.push(1, "x", PageIndex::zero(15)),
            Err(BookmarkError::IndexLen {
                expected: 16,
                actual: 15
            })
        );
        assert_eq!(
            file.push(1, "a\tb", index(9)),
            Err(BookmarkError::Invalid(LineError::InvalidLabel('\t')))
        );
        // The second bookmark is on line 3, and the new one would be line 4.
        assert_eq!(
            file.push(1, "again", index(2)),
            Err(BookmarkError::DuplicateIndex {
                first_line: 3,
                line: 4
            })
        );
        assert_eq!(file, before);
    }

    #[test]
    fn push_stops_at_the_limit() {
        let mut file = BookmarkFile::new(settings());
        for n in 0..MAX_BOOKMARKS as u64 {
            file.push(0, "", index(n)).unwrap();
        }
        assert_eq!(
            file.push(0, "", index(MAX_BOOKMARKS as u64)),
            Err(BookmarkError::TooManyBookmarks { max: MAX_BOOKMARKS })
        );
        assert_eq!(file.bookmarks().len(), MAX_BOOKMARKS);

        // The same count parses, and one more line does not.
        let bytes = file.to_bytes();
        assert_eq!(BookmarkFile::from_bytes(&bytes).unwrap(), file);
        let mut extra = bytes;
        extra.extend_from_slice(b"0\t\tAAAAAAAAAAAAAAAAAAAAAQ==\n");
        assert_eq!(
            BookmarkFile::from_bytes(&extra),
            Err(BookmarkError::TooManyBookmarks { max: MAX_BOOKMARKS })
        );
    }

    /// `max_bytes` assumes a header of at most `MAX_HEADER_BYTES`. Check it against the longest
    /// real ones, so a longer field cannot make the bound too small without a test failing.
    #[test]
    fn every_header_fits_in_the_header_bound() {
        for size in [
            LibraryConfig::SMALL,
            LibraryConfig::MEDIUM,
            LibraryConfig::LARGE,
        ] {
            for key in [BookmarkKey::Canonical, BookmarkKey::Custom([0xff; 8])] {
                let header = BookmarkSettings::new(size, key).unwrap().header();
                assert!(header.len() <= MAX_HEADER_BYTES, "{header:?}");
            }
        }
    }

    #[test]
    fn a_full_file_fits_in_max_bytes() {
        let mut file = BookmarkFile::new(settings());
        let label = "日".repeat(MAX_LABEL_CHARS);
        for n in 0..MAX_BOOKMARKS as u64 {
            file.push(u64::MAX, &label, index(n)).unwrap();
        }
        let len = file.to_bytes().len();
        let max = BookmarkFile::max_bytes(&settings());
        assert!(len <= max, "{len} > {max}");
        // The bound is not wildly loose: within a factor of two of the longest real file.
        assert!(max < len * 2);
    }

    #[test]
    fn rename_changes_only_the_label() {
        let mut file = file_of(2);
        file.rename(1, "new name").unwrap();
        let renamed = &file.bookmarks()[1];
        assert_eq!(renamed.label(), "new name");
        assert_eq!(renamed.time(), 2);
        assert_eq!(renamed.index(), &index(2));
        assert_eq!(file.bookmarks()[0].label(), "label 1");
    }

    #[test]
    fn rename_and_remove_out_of_range_are_errors_not_panics() {
        let mut file = file_of(2);
        let before = file.clone();
        let error = BookmarkError::NoSuchBookmark {
            position: 2,
            len: 2,
        };
        assert_eq!(file.rename(2, "x"), Err(error.clone()));
        assert_eq!(file.remove(2), Err(error));
        assert_eq!(
            file.rename(0, "bad\u{1b}"),
            Err(BookmarkError::Invalid(LineError::InvalidLabel('\u{1b}')))
        );
        assert_eq!(file, before);
        assert!(BookmarkFile::new(settings()).remove(0).is_err());
    }

    #[test]
    fn remove_returns_the_bookmark_and_closes_the_gap() {
        let mut file = file_of(3);
        let removed = file.remove(1).unwrap();
        assert_eq!(removed.label(), "label 2");
        let labels: Vec<_> = file.bookmarks().iter().map(|b| b.label()).collect();
        assert_eq!(labels, ["label 1", "label 3"]);
        // The removed index can be bookmarked again.
        file.push(9, "back", index(2)).unwrap();
    }

    #[test]
    fn from_bytes_reports_the_header_and_the_line_number() {
        assert_eq!(
            BookmarkFile::from_bytes(b"hello\n"),
            Err(BookmarkError::NotBookmarkFile)
        );

        let mut bytes = GOLDEN.as_bytes().to_vec();
        bytes.extend_from_slice(b"7\tbad\tnot-base64\n");
        // The header is line 1, so the fourth bookmark is on line 5.
        assert!(matches!(
            BookmarkFile::from_bytes(&bytes),
            Err(BookmarkError::Line {
                line: 5,
                error: LineError::Index(_)
            })
        ));

        // An empty line in the middle is a line with one field.
        let bytes = b"BABELMARKS1 size=small key=canonical format=base64\n\n";
        assert_eq!(
            BookmarkFile::from_bytes(bytes),
            Err(BookmarkError::Line {
                line: 2,
                error: LineError::FieldCount { found: 1 }
            })
        );
    }

    #[test]
    fn from_bytes_requires_the_final_newline() {
        let bytes = &GOLDEN.as_bytes()[..GOLDEN.len() - 1];
        assert_eq!(
            BookmarkFile::from_bytes(bytes),
            Err(BookmarkError::MissingFinalNewline)
        );
    }

    #[test]
    fn from_bytes_rejects_an_index_of_the_wrong_library_size() {
        // 24 characters is 16 bytes; a medium file needs 3200.
        let bytes = b"BABELMARKS1 size=medium key=canonical format=base64\n\
            1\tx\tAAAAAAAAAAAAAAAAAAAAAQ==\n";
        assert!(matches!(
            BookmarkFile::from_bytes(bytes),
            Err(BookmarkError::Line {
                line: 2,
                error: LineError::Index(_)
            })
        ));
    }

    #[test]
    fn from_bytes_rejects_duplicate_indices() {
        let mut bytes = GOLDEN.as_bytes().to_vec();
        bytes.extend_from_slice(b"9\tcopy\tAAAAAAAAAAAAAAAAAAAAAg==\n");
        // The index is first on line 3 and again on line 5.
        assert_eq!(
            BookmarkFile::from_bytes(&bytes),
            Err(BookmarkError::DuplicateIndex {
                first_line: 3,
                line: 5
            })
        );
    }

    #[test]
    fn windows_line_endings_are_named() {
        let bytes: Vec<u8> = GOLDEN
            .bytes()
            .flat_map(|b| {
                if b == b'\n' {
                    vec![b'\r', b'\n']
                } else {
                    vec![b]
                }
            })
            .collect();
        assert_eq!(
            BookmarkFile::from_bytes(&bytes),
            Err(BookmarkError::WindowsLineEndings)
        );

        // Only a bookmark line with the `\r`, and a good header: the same error.
        let mut bytes = GOLDEN.as_bytes()[..GOLDEN.len() - 1].to_vec();
        bytes.extend_from_slice(b"\r\n");
        assert_eq!(
            BookmarkFile::from_bytes(&bytes),
            Err(BookmarkError::WindowsLineEndings)
        );
    }

    /// Changing any one byte of a file must give an error, or a file that writes back as
    /// exactly the changed bytes: there is one spelling of each file.
    #[test]
    fn a_changed_file_never_parses_to_different_bytes() {
        for at in 0..GOLDEN.len() {
            for replacement in [b' ', b'\t', b'\n', b'\r', b'0', b'A', b'=', b'+', 0xff] {
                let mut changed = GOLDEN.as_bytes().to_vec();
                changed[at] = replacement;
                if let Ok(file) = BookmarkFile::from_bytes(&changed) {
                    assert_eq!(file.to_bytes(), changed, "byte {at} -> {replacement:#x}");
                }
            }
        }
    }

    #[test]
    fn error_messages_are_readable() {
        assert_eq!(
            BookmarkError::Line {
                line: 5,
                error: LineError::FieldCount { found: 1 }
            }
            .to_string(),
            "line 5: a bookmark line must have 3 tab-separated fields, found 1"
        );
        assert_eq!(
            BookmarkError::DuplicateIndex {
                first_line: 3,
                line: 5
            }
            .to_string(),
            "line 5 has the same index as line 3"
        );
        assert_eq!(
            BookmarkError::NoSuchBookmark {
                position: 4,
                len: 2
            }
            .to_string(),
            "no bookmark at position 4, the file has 2"
        );
    }
}
