//! Opening a bookmark file when the explorer starts.
//!
//! A bookmark file belongs to one library, so a session that has one is *locked* to it: the
//! library is the file's library. The rule here is that a flag you give must match the file,
//! and a flag you leave out is taken from the file. Every mismatch is an error that says what
//! to change, so nothing is guessed.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use bitbabel_core::{FINGERPRINT_LEN, Key, LibraryConfig, LibraryError, PageIndex};
use bitbabel_file::{BookmarkError, BookmarkFile, BookmarkKey, BookmarkSettings};

use crate::app::{LibraryChoice, fingerprint_groups};
use crate::files::Files;

/// The longest file name the title bar shows, the rest cut off with `…`. The title has room
/// for the library name (up to 42 characters) and `locked: ` and the count beside it.
const TITLE_NAME_CHARS: usize = 16;

/// Whether the session has a bookmark file. Once it has, the library is the file's and cannot
/// change for the rest of the session.
#[derive(Debug, Clone)]
pub enum Bookmarks {
    /// No file. Nothing is saved.
    Free,
    /// Bound to one file.
    Locked(LockedBookmarks),
}

impl Bookmarks {
    /// The text on the right of the title bar: `no bookmark file`, or `locked: favs (12)`.
    pub fn title(&self) -> String {
        match self {
            Bookmarks::Free => "no bookmark file".to_string(),
            Bookmarks::Locked(locked) => {
                let name = locked.name();
                let name = if name.chars().count() > TITLE_NAME_CHARS {
                    let kept: String = name.chars().take(TITLE_NAME_CHARS - 1).collect();
                    format!("{kept}…")
                } else {
                    name
                };
                format!("locked: {name} ({})", locked.file().bookmarks().len())
            }
        }
    }
}

/// A bookmark file that is open, and where it came from.
#[derive(Debug, Clone)]
pub struct LockedBookmarks {
    path: PathBuf,
    file: BookmarkFile,
}

impl LockedBookmarks {
    /// Reads the bookmark file at `path` for a session that was started with `size` and `key`
    /// (each `None` when its flag was not given). Returns the library the file is for, and the
    /// open file.
    ///
    /// # Errors
    ///
    /// [`StartError`], one for each way the file and the flags can disagree.
    pub fn open(
        files: &dyn Files,
        path: &Path,
        size: Option<LibraryConfig>,
        key: Option<Key>,
    ) -> Result<(LibraryChoice, LockedBookmarks), StartError> {
        let bytes = files
            .read(path, largest_file())
            .map_err(|error| StartError::Read {
                path: path.to_path_buf(),
                error,
            })?;
        let file = BookmarkFile::from_bytes(&bytes).map_err(|error| StartError::Parse {
            path: path.to_path_buf(),
            error,
        })?;
        let settings = file.settings();

        if let Some(given) = size
            && given != settings.size()
        {
            return Err(StartError::SizeMismatch {
                path: path.to_path_buf(),
                given,
                file: settings.size(),
            });
        }
        match (settings.key(), &key) {
            (BookmarkKey::Canonical, None) => {}
            (BookmarkKey::Canonical, Some(_)) => {
                return Err(StartError::KeyForCanonical(path.to_path_buf()));
            }
            (BookmarkKey::Custom(fingerprint), None) => {
                return Err(StartError::NeedsKey {
                    path: path.to_path_buf(),
                    fingerprint,
                });
            }
            (BookmarkKey::Custom(fingerprint), Some(given)) => {
                if !settings.key().matches_fingerprint(&given.fingerprint()) {
                    return Err(StartError::WrongKey {
                        path: path.to_path_buf(),
                        file: fingerprint,
                        given: given.fingerprint(),
                    });
                }
            }
        }
        let library = LibraryChoice::new(settings.size(), key);
        let locked = LockedBookmarks {
            path: path.to_path_buf(),
            file,
        };
        Ok((library, locked))
    }

    /// Adds a bookmark for `index` and writes the whole file. Returns how many bookmarks the
    /// file has now. `time` is unix seconds.
    ///
    /// The file is written before the change is kept: if the write fails, the list is as it
    /// was, so what is on screen is always what is on disk.
    ///
    /// # Errors
    ///
    /// [`SaveError`] if the index is already bookmarked, the file is full, the label is
    /// refused, or the file could not be written.
    pub fn add(
        &mut self,
        files: &dyn Files,
        time: u64,
        label: &str,
        index: PageIndex,
    ) -> Result<usize, SaveError> {
        if let Some(at) = self.file.find(&index) {
            let existing = self.file.bookmarks()[at].label().to_string();
            return Err(SaveError::Duplicate { label: existing });
        }
        self.file
            .push(time, label, index)
            .map_err(|error| match error {
                BookmarkError::TooManyBookmarks { max } => SaveError::Full { max },
                other => SaveError::Refused(other),
            })?;
        let count = self.file.bookmarks().len();
        if let Err(error) = files.write_replace(&self.path, &self.file.to_bytes()) {
            // The bookmark just added is the last one, so this cannot be out of range.
            let _ = self.file.remove(count - 1);
            return Err(SaveError::Write {
                path: self.path.clone(),
                error,
            });
        }
        Ok(count)
    }

    /// The file's name without its extension: `favs` for `dir/favs.babelbookmark`.
    pub fn name(&self) -> String {
        let stem = self.path.file_stem().unwrap_or(self.path.as_os_str());
        stem.to_string_lossy().into_owned()
    }

    /// Where the file is.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The bookmarks in it.
    pub fn file(&self) -> &BookmarkFile {
        &self.file
    }
}

/// The most bytes any bookmark file can have. At startup the file's size is not known until
/// its header is read, so the biggest library's bound is used. It only stops a hostile file
/// from being read whole.
fn largest_file() -> usize {
    let largest = BookmarkSettings::new(LibraryConfig::LARGE, BookmarkKey::Canonical);
    // `LARGE` is a preset, so this cannot be an error; the fallback is a generous number.
    largest.map_or(128 << 20, |settings| BookmarkFile::max_bytes(&settings))
}

/// Why a bookmark could not be added. Every one of these is shown inline and the session
/// carries on.
#[derive(Debug)]
pub enum SaveError {
    /// The page is already bookmarked, under `label`.
    Duplicate { label: String },
    /// The file holds as many bookmarks as it may.
    Full { max: usize },
    /// The file crate refused the bookmark. The label prompt only takes valid characters, so
    /// this is not expected.
    Refused(BookmarkError),
    /// The file could not be written. The list was left as it was.
    Write { path: PathBuf, error: io::Error },
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Duplicate { label } if label.is_empty() => {
                write!(f, "this page is already bookmarked (with no label)")
            }
            SaveError::Duplicate { label } => {
                write!(f, "this page is already bookmarked as \"{label}\"")
            }
            SaveError::Full { max } => write!(f, "the file already has {max} bookmarks"),
            SaveError::Refused(error) => write!(f, "{error}"),
            SaveError::Write { path, error } => {
                write!(f, "cannot save {}: {error}", path.display())
            }
        }
    }
}

impl Error for SaveError {}

/// Why a bookmark file could not be opened at startup. All of these are fatal: there is no
/// library to start in.
#[derive(Debug)]
pub enum StartError {
    /// The file could not be read. A path that does not exist is this, so a typo never makes a
    /// new file.
    Read { path: PathBuf, error: io::Error },
    /// The file is not a valid bookmark file. The message includes the parser's, with the line.
    Parse { path: PathBuf, error: BookmarkError },
    /// `--size` is not the size of the file's library.
    SizeMismatch {
        path: PathBuf,
        given: LibraryConfig,
        file: LibraryConfig,
    },
    /// The file is for a private key and none was given.
    NeedsKey {
        path: PathBuf,
        fingerprint: [u8; FINGERPRINT_LEN],
    },
    /// The key given is not the file's key.
    WrongKey {
        path: PathBuf,
        file: [u8; FINGERPRINT_LEN],
        given: [u8; FINGERPRINT_LEN],
    },
    /// The file is for the canonical library, so a key would put the session in another one.
    KeyForCanonical(PathBuf),
    /// The file's library could not be built. The presets always can be, so this is not
    /// expected.
    Library { path: PathBuf, error: LibraryError },
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartError::Read { path, error } => {
                write!(f, "cannot read {}: {error}", path.display())
            }
            StartError::Parse { path, error } => {
                write!(
                    f,
                    "{} is not a valid bookmark file: {error}",
                    path.display()
                )
            }
            StartError::SizeMismatch { path, given, file } => write!(
                f,
                "--size {} does not match {}, which is a {} library",
                size_name(*given),
                path.display(),
                size_name(*file)
            ),
            StartError::NeedsKey { path, fingerprint } => write!(
                f,
                "{} needs the key with fingerprint {}; pass --key-file",
                path.display(),
                fingerprint_groups(fingerprint)
            ),
            StartError::WrongKey { path, file, given } => write!(
                f,
                "{} is for the key with fingerprint {}, but the key given has {}",
                path.display(),
                fingerprint_groups(file),
                fingerprint_groups(given)
            ),
            StartError::Library { path, error } => {
                write!(f, "cannot open the library of {}: {error}", path.display())
            }
            StartError::KeyForCanonical(path) => write!(
                f,
                "{} is a canonical library, but a key was given \
                 (--key-file, --private or BITBABEL_KEY)",
                path.display()
            ),
        }
    }
}

// No `source()`: `Display` already includes the inner error's message.
impl Error for StartError {}

fn size_name(size: LibraryConfig) -> &'static str {
    size.name().unwrap_or("custom")
}

#[cfg(test)]
mod test {
    use bitbabel_core::PageIndex;

    use super::*;
    use crate::files::MemFiles;

    const PATH: &str = "favs.babelbookmark";

    fn key() -> Key {
        Key::from_bytes([1; 32])
    }

    fn file_bytes(size: LibraryConfig, bookmark_key: BookmarkKey) -> Vec<u8> {
        let settings = BookmarkSettings::new(size, bookmark_key).unwrap();
        let mut file = BookmarkFile::new(settings);
        file.push(1, "first", PageIndex::from_u64(1, size.page_len()))
            .unwrap();
        file.to_bytes()
    }

    fn open(
        bytes: &[u8],
        size: Option<LibraryConfig>,
        key: Option<Key>,
    ) -> Result<(LibraryChoice, LockedBookmarks), StartError> {
        let files = MemFiles::default().with(PATH, bytes);
        LockedBookmarks::open(&files, Path::new(PATH), size, key)
    }

    fn medium_canonical() -> Vec<u8> {
        file_bytes(LibraryConfig::MEDIUM, BookmarkKey::Canonical)
    }

    fn small_custom() -> Vec<u8> {
        file_bytes(
            LibraryConfig::SMALL,
            BookmarkKey::Custom(key().fingerprint()),
        )
    }

    #[test]
    fn with_no_flags_the_library_is_the_files() {
        let (library, locked) = open(&medium_canonical(), None, None).unwrap();
        assert_eq!(library.label(), "medium · canonical");
        assert_eq!(locked.path(), Path::new(PATH));
        assert_eq!(locked.file().bookmarks().len(), 1);

        let (library, _) = open(&small_custom(), None, Some(key())).unwrap();
        assert_eq!(library.label(), "small · key a576-7c85-fdef-3c24");
    }

    #[test]
    fn flags_that_match_the_file_are_accepted() {
        assert!(open(&medium_canonical(), Some(LibraryConfig::MEDIUM), None).is_ok());
        assert!(open(&small_custom(), Some(LibraryConfig::SMALL), Some(key())).is_ok());
    }

    #[test]
    fn a_size_that_differs_from_the_file_is_an_error_that_names_both() {
        let error = open(&medium_canonical(), Some(LibraryConfig::SMALL), None).unwrap_err();
        assert!(matches!(error, StartError::SizeMismatch { .. }));
        assert_eq!(
            error.to_string(),
            "--size small does not match favs.babelbookmark, which is a medium library"
        );
    }

    #[test]
    fn a_private_file_without_a_key_says_which_key_it_needs() {
        let error = open(&small_custom(), None, None).unwrap_err();
        assert!(matches!(error, StartError::NeedsKey { .. }));
        assert_eq!(
            error.to_string(),
            "favs.babelbookmark needs the key with fingerprint a576-7c85-fdef-3c24; \
             pass --key-file"
        );
    }

    #[test]
    fn the_wrong_key_names_both_fingerprints() {
        let other = Key::from_bytes([2; 32]);
        let error = open(&small_custom(), None, Some(other.clone())).unwrap_err();
        assert!(matches!(error, StartError::WrongKey { .. }));
        let text = error.to_string();
        assert!(
            text.contains(&fingerprint_groups(&key().fingerprint())),
            "{text}"
        );
        assert!(
            text.contains(&fingerprint_groups(&other.fingerprint())),
            "{text}"
        );
    }

    #[test]
    fn a_key_for_a_canonical_file_is_an_error() {
        let error = open(&medium_canonical(), None, Some(key())).unwrap_err();
        assert!(matches!(error, StartError::KeyForCanonical(_)));
        let text = error.to_string();
        assert!(
            text.contains("canonical library, but a key was given"),
            "{text}"
        );
        // It names the environment variable too, which the user may not have typed.
        assert!(text.contains("BITBABEL_KEY"), "{text}");
    }

    #[test]
    fn the_size_is_checked_before_the_key() {
        let error = open(&small_custom(), Some(LibraryConfig::LARGE), None).unwrap_err();
        assert!(matches!(error, StartError::SizeMismatch { .. }));
    }

    #[test]
    fn a_missing_file_is_an_error_and_is_not_created() {
        let files = MemFiles::default();
        let error =
            LockedBookmarks::open(&files, Path::new("typo.babelbookmark"), None, None).unwrap_err();
        assert!(matches!(error, StartError::Read { .. }));
        assert_eq!(
            error.to_string(),
            "cannot read typo.babelbookmark: No such file or directory"
        );
        assert_eq!(files.get("typo.babelbookmark"), None);
    }

    #[test]
    fn a_file_that_does_not_parse_says_why_and_where() {
        let mut bytes = medium_canonical();
        bytes.extend_from_slice(b"7\tbad\tnot-base64\n");
        let error = open(&bytes, None, None).unwrap_err();
        assert!(matches!(error, StartError::Parse { .. }));
        let text = error.to_string();
        assert!(text.starts_with("favs.babelbookmark is not a valid bookmark file: line 3: "));

        let error = open(b"hello\n", None, None).unwrap_err();
        assert_eq!(
            error.to_string(),
            "favs.babelbookmark is not a valid bookmark file: not a bookmark file"
        );
    }

    #[test]
    fn the_read_limit_covers_a_full_file_of_every_size() {
        for size in [
            LibraryConfig::SMALL,
            LibraryConfig::MEDIUM,
            LibraryConfig::LARGE,
        ] {
            for key in [
                BookmarkKey::Canonical,
                BookmarkKey::Custom([0xff; FINGERPRINT_LEN]),
            ] {
                let settings = BookmarkSettings::new(size, key).unwrap();
                assert!(BookmarkFile::max_bytes(&settings) <= largest_file());
            }
        }
    }

    #[test]
    fn a_header_only_file_is_a_valid_empty_list() {
        let settings = BookmarkSettings::new(LibraryConfig::SMALL, BookmarkKey::Canonical).unwrap();
        let bytes = BookmarkFile::new(settings).to_bytes();
        let (_, locked) = open(&bytes, None, None).unwrap();
        assert!(locked.file().bookmarks().is_empty());
    }
}
