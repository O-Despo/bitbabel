//! The bookmark list of a session: one bookmark file held in memory. All of its rules live
//! here, so they run in plain unit tests without a browser. Nothing is saved anywhere until the
//! user downloads the file.

use bitbabel_core::PageIndex;
use bitbabel_explorer::{LibraryChoice, library_mismatch};
use bitbabel_file::{Bookmark, BookmarkError, BookmarkFile, MAX_LABEL_CHARS, is_label_char};

/// The file name offered for the first bookmark of a session.
pub const DEFAULT_NAME: &str = "my.babelmarks";

/// How many characters of each index the preview shows.
const PREVIEW_INDEX: usize = 10;

/// What a label box keeps of what is typed: only the characters a label may have, and at most
/// [`MAX_LABEL_CHARS`] of them. What is kept is always a valid label.
pub fn clean_label(text: &str) -> String {
    text.chars()
        .filter(|&c| is_label_char(c))
        .take(MAX_LABEL_CHARS)
        .collect()
}

/// A bookmark file and the name it will be downloaded as.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Marks {
    name: String,
    file: BookmarkFile,
}

/// The session's bookmarks: no file yet, or one file for the open library.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Bookmarks {
    marks: Option<Marks>,
    /// Why the last change was refused, if it was.
    error: Option<String>,
}

impl Bookmarks {
    /// Why the last change was refused, if it was.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The file's name, once there is a file.
    pub fn name(&self) -> Option<&str> {
        self.marks.as_ref().map(|marks| marks.name.as_str())
    }

    /// The bookmarks, in file order.
    pub fn list(&self) -> &[Bookmark] {
        self.marks
            .as_ref()
            .map_or(&[], |marks| marks.file.bookmarks())
    }

    /// The file as text: the header and one line per bookmark.
    pub fn text(&self) -> Option<String> {
        let marks = self.marks.as_ref()?;
        Some(String::from_utf8_lossy(&marks.file.to_bytes()).into_owned())
    }

    /// The file's first line: `BABELMARKS1 size=medium key=canonical format=base64`.
    pub fn header(&self) -> Option<String> {
        Some(
            self.marks
                .as_ref()?
                .file
                .settings()
                .header()
                .trim_end()
                .to_string(),
        )
    }

    /// The file as the page previews it: the header, then each bookmark with its index cut to
    /// its first [`PREVIEW_INDEX`] characters, since a full index is thousands of characters.
    pub fn preview(&self) -> Option<String> {
        let marks = self.marks.as_ref()?;
        let mut text = marks.file.settings().header();
        for bookmark in marks.file.bookmarks() {
            let index = bookmark.index().to_base64();
            let cut: String = index.chars().take(PREVIEW_INDEX).collect();
            text.push_str(&format!(
                "{}\t{}\t{cut}…\n",
                bookmark.time(),
                bookmark.label()
            ));
        }
        Some(text)
    }

    /// Reads the file called `name` for a session in `library`. A file that cannot be read, is
    /// not a bookmark file or is for another library is refused with a message, and the list
    /// that was open stays.
    pub fn open(&mut self, library: &LibraryChoice, name: &str, bytes: Result<Vec<u8>, String>) {
        match Self::read(library, name, bytes) {
            Ok(marks) => {
                self.error = None;
                self.marks = Some(marks);
            }
            Err(message) => self.error = Some(message),
        }
    }

    fn read(
        library: &LibraryChoice,
        name: &str,
        bytes: Result<Vec<u8>, String>,
    ) -> Result<Marks, String> {
        let bytes = bytes.map_err(|error| format!("cannot read {name}: {error}"))?;
        let session = library
            .bookmark_settings()
            .map_err(|error| error.to_string())?;
        if bytes.len() > BookmarkFile::max_bytes(&session) {
            return Err(format!("{name} is too big for a bookmark file"));
        }
        let file = BookmarkFile::from_bytes(&bytes)
            .map_err(|error| format!("{name} is not a valid bookmark file: {error}"))?;
        match library_mismatch(name, &file.settings(), &session) {
            Some(message) => Err(message),
            None => Ok(Marks {
                name: name.to_string(),
                file,
            }),
        }
    }

    /// Adds a bookmark for `index`. With no file yet, one is started for `library` and called
    /// `name`, or [`DEFAULT_NAME`] if that is blank.
    ///
    /// # Errors
    ///
    /// A message for the popup: the page is already bookmarked, the file is full, or the
    /// library cannot be written to a file. Nothing changes then.
    pub fn add(
        &mut self,
        library: &LibraryChoice,
        name: &str,
        now: u64,
        label: &str,
        index: &PageIndex,
    ) -> Result<(), String> {
        // A new file is only kept once its first bookmark is in it.
        let mut started = None;
        let marks = match &mut self.marks {
            Some(marks) => marks,
            None => {
                let settings = library
                    .bookmark_settings()
                    .map_err(|error| error.to_string())?;
                let name = name.trim();
                started.insert(Marks {
                    name: if name.is_empty() { DEFAULT_NAME } else { name }.to_string(),
                    file: BookmarkFile::new(settings),
                })
            }
        };
        if let Some(at) = marks.file.find(index) {
            let existing = marks.file.bookmarks()[at].label();
            return Err(if existing.is_empty() {
                "this page is already bookmarked (with no label)".to_string()
            } else {
                format!("this page is already bookmarked as \"{existing}\"")
            });
        }
        // `push` leaves the file as it was on an error.
        marks
            .file
            .push(now, label, index.clone())
            .map_err(|error| match error {
                BookmarkError::TooManyBookmarks { max } => {
                    format!("the file already has {max} bookmarks")
                }
                error => error.to_string(),
            })?;
        if started.is_some() {
            self.marks = started;
        }
        self.error = None;
        Ok(())
    }

    /// Gives the bookmark at `position` a new label.
    pub fn rename(&mut self, position: usize, label: &str) {
        let Some(marks) = &mut self.marks else {
            return;
        };
        self.error = marks
            .file
            .rename(position, label)
            .err()
            .map(|e| e.to_string());
    }

    /// Takes the bookmark at `position` out of the list.
    pub fn remove(&mut self, position: usize) {
        let Some(marks) = &mut self.marks else {
            return;
        };
        self.error = marks.file.remove(position).err().map(|e| e.to_string());
    }
}

/// The time of unix `seconds` in UTC, as `2026-10-29 14:05`.
pub fn when(seconds: u64) -> String {
    // Days since 1970-01-01 to a calendar date (the civil-from-days method).
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX / 2) + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    let minutes = seconds % 86_400 / 60;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        minutes / 60,
        minutes % 60
    )
}

#[cfg(test)]
mod test {
    use bitbabel_core::{Key, LibraryConfig};

    use super::*;

    fn small() -> LibraryChoice {
        LibraryChoice::new(LibraryConfig::SMALL, None)
    }

    fn medium() -> LibraryChoice {
        LibraryChoice::new(LibraryConfig::MEDIUM, None)
    }

    fn index(library: &LibraryChoice, n: u8) -> PageIndex {
        let mut bytes = vec![0; library.size().page_len()];
        bytes[0] = n;
        PageIndex::from_bytes(bytes)
    }

    /// A list with bookmarks "a" and "b" on pages 1 and 2.
    fn two() -> Bookmarks {
        let library = small();
        let mut marks = Bookmarks::default();
        marks
            .add(&library, "", 10, "a", &index(&library, 1))
            .unwrap();
        marks
            .add(&library, "", 20, "b", &index(&library, 2))
            .unwrap();
        marks
    }

    fn labels(marks: &Bookmarks) -> Vec<&str> {
        marks.list().iter().map(Bookmark::label).collect()
    }

    #[test]
    fn a_label_box_keeps_only_valid_characters_up_to_the_limit() {
        assert_eq!(clean_label("a\tb\nc\u{1b}d"), "abcd");
        assert_eq!(
            clean_label(&"x".repeat(300)).chars().count(),
            MAX_LABEL_CHARS
        );
        assert_eq!(clean_label("héllo wörld"), "héllo wörld");
    }

    #[test]
    fn the_first_bookmark_starts_a_file_with_the_name_given_or_the_default() {
        let library = small();
        let mut named = Bookmarks::default();
        named
            .add(&library, " favs.babelmarks ", 5, "x", &index(&library, 1))
            .unwrap();
        assert_eq!(named.name(), Some("favs.babelmarks"));
        let mut blank = Bookmarks::default();
        blank
            .add(&library, "  ", 5, "x", &index(&library, 1))
            .unwrap();
        assert_eq!(blank.name(), Some(DEFAULT_NAME));
    }

    #[test]
    fn the_name_is_only_used_for_the_first_bookmark() {
        let library = small();
        let mut marks = two();
        marks
            .add(&library, "other", 30, "c", &index(&library, 3))
            .unwrap();
        assert_eq!(marks.name(), Some(DEFAULT_NAME));
        assert_eq!(labels(&marks), ["a", "b", "c"]);
    }

    #[test]
    fn a_page_cannot_be_bookmarked_twice_and_nothing_changes() {
        let library = small();
        let mut marks = two();
        let before = marks.clone();
        let error = marks
            .add(&library, "", 30, "again", &index(&library, 1))
            .unwrap_err();
        assert_eq!(error, "this page is already bookmarked as \"a\"");
        assert_eq!(marks, before);
        let mut unlabeled = Bookmarks::default();
        unlabeled
            .add(&library, "", 1, "", &index(&library, 1))
            .unwrap();
        let error = unlabeled
            .add(&library, "", 2, "x", &index(&library, 1))
            .unwrap_err();
        assert_eq!(error, "this page is already bookmarked (with no label)");
    }

    #[test]
    fn a_bad_label_is_refused_and_no_file_is_started() {
        let library = small();
        let mut marks = Bookmarks::default();
        assert!(
            marks
                .add(&library, "", 1, "a\tb", &index(&library, 1))
                .is_err()
        );
        assert!(marks.name().is_none());
    }

    #[test]
    fn a_file_downloaded_can_be_opened_again() {
        let library = small();
        let text = two().text().unwrap();
        assert!(text.starts_with("BABELMARKS1 size=small key=canonical format=base64\n"));
        let mut reopened = Bookmarks::default();
        reopened.open(&library, "two.babelmarks", Ok(text.into_bytes()));
        assert_eq!(reopened.name(), Some("two.babelmarks"));
        assert_eq!(labels(&reopened), ["a", "b"]);
        assert!(reopened.error().is_none());
    }

    #[test]
    fn a_file_for_another_library_is_refused_and_the_list_stays() {
        let text = two().text().unwrap();
        let mut marks = Bookmarks::default();
        marks.open(&medium(), "two.babelmarks", Ok(text.into_bytes()));
        assert!(marks.name().is_none());
        assert_eq!(
            marks.error().unwrap(),
            "two.babelmarks is a small library, but this session is medium"
        );

        let mut open = two();
        open.open(&small(), "x", Ok(b"nothing".to_vec()));
        assert_eq!(labels(&open), ["a", "b"]);
        assert!(open.error().is_some());
    }

    #[test]
    fn a_file_that_is_not_a_bookmark_file_or_cannot_be_read_says_so() {
        let library = small();
        let mut marks = Bookmarks::default();
        marks.open(&library, "x.txt", Ok(b"hello\n".to_vec()));
        assert!(
            marks
                .error()
                .unwrap()
                .starts_with("x.txt is not a valid bookmark file: ")
        );
        marks.open(&library, "x.txt", Err("denied".to_string()));
        assert_eq!(marks.error().unwrap(), "cannot read x.txt: denied");
    }

    #[test]
    fn rename_changes_the_label_and_keeps_the_time() {
        let mut marks = two();
        marks.rename(1, "second");
        assert_eq!(labels(&marks), ["a", "second"]);
        assert_eq!(marks.list()[1].time(), 20);
        assert!(marks.error().is_none());
    }

    #[test]
    fn rename_and_remove_past_the_list_or_with_a_bad_label_say_why() {
        let mut marks = two();
        marks.rename(5, "x");
        assert!(marks.error().is_some());
        marks.rename(0, "a\tb");
        assert_eq!(labels(&marks), ["a", "b"]);
        marks.remove(9);
        assert_eq!(labels(&marks), ["a", "b"]);
        assert!(marks.error().is_some());
    }

    #[test]
    fn remove_takes_the_bookmark_out() {
        let mut marks = two();
        marks.remove(0);
        assert_eq!(labels(&marks), ["b"]);
    }

    #[test]
    fn rename_and_remove_with_no_file_do_nothing() {
        let mut marks = Bookmarks::default();
        marks.rename(0, "x");
        marks.remove(0);
        assert_eq!(marks, Bookmarks::default());
    }

    #[test]
    fn a_private_library_writes_its_fingerprint_and_a_canonical_session_refuses_it() {
        let key = Key::from_slice(&[7; 32]).unwrap();
        let library = LibraryChoice::new(LibraryConfig::SMALL, Some(key));
        let mut marks = Bookmarks::default();
        marks
            .add(&library, "", 1, "x", &index(&library, 1))
            .unwrap();
        let text = marks.text().unwrap();
        assert!(text.contains(" key=custom fingerprint="));

        let mut canonical = Bookmarks::default();
        canonical.open(&small(), "p", Ok(text.into_bytes()));
        assert!(canonical.error().is_some());
        assert!(canonical.name().is_none());
    }

    #[test]
    fn times_are_utc_calendar_days_and_minutes() {
        assert_eq!(when(0), "1970-01-01 00:00");
        assert_eq!(when(951_782_400), "2000-02-29 00:00");
        assert_eq!(when(1_793_232_000), "2026-10-29 00:00");
        assert_eq!(when(1_793_232_000 - 1), "2026-10-28 23:59");
        assert_eq!(
            when(1_793_232_000 + 14 * 3600 + 5 * 60 + 59),
            "2026-10-29 14:05"
        );
    }

    #[test]
    fn the_header_is_the_files_first_line() {
        assert_eq!(Bookmarks::default().header(), None);
        assert_eq!(
            two().header().unwrap(),
            "BABELMARKS1 size=small key=canonical format=base64"
        );
    }

    #[test]
    fn the_preview_cuts_each_index() {
        let library = small();
        let mut marks = Bookmarks::default();
        assert_eq!(marks.preview(), None);
        marks
            .add(&library, "", 10, "a", &index(&library, 1))
            .unwrap();
        let preview = marks.preview().unwrap();
        let mut lines = preview.lines();
        assert_eq!(
            lines.next(),
            Some("BABELMARKS1 size=small key=canonical format=base64")
        );
        let line = lines.next().unwrap();
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(&fields[..2], ["10", "a"]);
        assert_eq!(fields[2].chars().count(), PREVIEW_INDEX + 1);
        assert!(fields[2].ends_with('…'));
    }
}
