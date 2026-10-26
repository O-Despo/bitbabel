//! One-line prompts on the explore screen: a question, a line of typing and an inline error.
//! While one is open every key goes to it, so typing `q` into a path never quits.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use bitbabel_core::PageIndex;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use bitbabel_file::{BookmarkFile, MAX_LABEL_CHARS, is_label_char};

use super::effect::Effect;
use super::session::Session;
use super::{App, Mode, typed_char};
use crate::bookmarks::{Bookmarks, LockedBookmarks, library_mismatch};
use crate::clock::NowFn;
use crate::files::Files;

/// What a prompt asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    /// A file holding an index to go to.
    GoTo,
    /// A new file to save the current index in.
    Export,
    /// The label of a new bookmark.
    Bookmark,
    /// The file to keep bookmarks in, asked for the first time a session bookmarks a page. The
    /// label comes next, and nothing is written until that is confirmed.
    BookmarkFile,
    /// A new label for an existing bookmark, opened from the `B` list over the list itself.
    BookmarkRename,
}

/// What the first bookmark of a session carries from the file step to the label step.
#[derive(Debug, Clone)]
struct Pending {
    /// The file, with `.babelbookmark` added if the name had no extension.
    path: PathBuf,
    /// What was typed for the file, to show again if the user steps back.
    path_text: String,
    /// The file as it is on disk, or `None` if it does not exist yet.
    existing: Option<BookmarkFile>,
    /// A label typed and then stepped back from, to show again if the user comes forward.
    label: String,
}

/// An open prompt.
#[derive(Debug, Clone)]
pub struct Prompt {
    kind: PromptKind,
    input: String,
    /// Why the last Enter did not work. The prompt stays open.
    error: Option<String>,
    /// Set on the label step of the first bookmark, where the file has been chosen but not
    /// written.
    pending: Option<Pending>,
    /// The row the rename prompt edits, `Some` only for [`PromptKind::BookmarkRename`].
    rename_at: Option<usize>,
}

impl Prompt {
    pub(super) fn new(kind: PromptKind) -> Self {
        Prompt {
            kind,
            input: String::new(),
            error: None,
            pending: None,
            rename_at: None,
        }
    }

    /// The rename prompt for row `at`, prefilled with its old label (§13.3b). The old label
    /// is made of valid characters, so it needs no filtering.
    pub(super) fn renaming(at: usize, old: &str) -> Self {
        Prompt {
            kind: PromptKind::BookmarkRename,
            input: old.to_string(),
            error: None,
            pending: None,
            rename_at: Some(at),
        }
    }

    /// Whether this box only accepts characters a label may hold: the bookmark label and
    /// its rename. A new kind that forgets this method silently skips the filter (§13.3b).
    fn is_label_box(&self) -> bool {
        matches!(self.kind, PromptKind::Bookmark | PromptKind::BookmarkRename)
    }

    pub(super) fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    /// The box title.
    pub fn title(&self) -> &'static str {
        match self.kind {
            PromptKind::GoTo => "go to",
            PromptKind::Export => "export index",
            PromptKind::Bookmark => "bookmark",
            PromptKind::BookmarkFile => "bookmark file",
            PromptKind::BookmarkRename => "rename",
        }
    }

    /// The question.
    pub fn label(&self) -> String {
        match (&self.kind, &self.pending) {
            (PromptKind::GoTo, _) => "file holding the index (hex or base64):".to_string(),
            (PromptKind::Export, _) => "save the index to a new file:".to_string(),
            (PromptKind::BookmarkFile, _) => {
                "keep bookmarks in which file? (.babelbookmark is added):".to_string()
            }
            (PromptKind::Bookmark, Some(pending)) => format!(
                "label for this page in {} (this locks the session to this library):",
                pending.path.display()
            ),
            (PromptKind::Bookmark, None) => "label for this page (may be empty):".to_string(),
            (PromptKind::BookmarkRename, _) => match self.rename_at {
                Some(at) => format!("new label for bookmark {} (may be empty):", at + 1),
                None => "new label for this bookmark (may be empty):".to_string(),
            },
        }
    }

    /// The keys line at the bottom of the box.
    pub fn footer(&self) -> &'static str {
        match (&self.kind, &self.pending) {
            (PromptKind::BookmarkFile, _) => "enter next   esc cancel",
            // Both a first bookmark's label and a rename can step back: to the file step,
            // or to the list.
            (PromptKind::Bookmark, Some(_)) | (PromptKind::BookmarkRename, _) => {
                "enter save   esc back"
            }
            _ => "enter confirm   esc cancel",
        }
    }

    /// What has been typed.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Why the last Enter did not work, if it did not.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// Handles a key while a prompt is open.
pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Vec<Effect> {
    let Mode::Explore(session) = &mut app.mode else {
        return vec![];
    };
    match key.code {
        KeyCode::Esc => {
            let renaming = session
                .prompt()
                .is_some_and(|prompt| prompt.kind == PromptKind::BookmarkRename);
            if !step_back(session) {
                session.close_prompt();
                // Esc on a rename goes back to the list it opened over (§13.3b).
                if renaming {
                    session.open_bookmark_list();
                }
            }
        }
        KeyCode::Enter => submit(session, app.files.as_ref(), app.now),
        KeyCode::Backspace => {
            if let Some(prompt) = session.prompt_mut() {
                prompt.input.pop();
                prompt.error = None;
            }
        }
        _ => {
            if let (Some(c), Some(prompt)) = (typed_char(&key), session.prompt_mut()) {
                // A label box only takes what a label may hold, so what is typed is valid.
                let refused = prompt.is_label_box()
                    && (!is_label_char(c) || prompt.input.chars().count() >= MAX_LABEL_CHARS);
                if !refused {
                    prompt.input.push(c);
                    prompt.error = None;
                }
            }
        }
    }
    vec![]
}

/// Esc on the label step of a first bookmark goes back to the file step, with both texts kept.
/// Returns whether it did; any other prompt is just closed.
fn step_back(session: &mut Session) -> bool {
    let Some(prompt) = session.prompt_mut() else {
        return false;
    };
    // Only the label step goes back. The file step, which also remembers the label, closes.
    if prompt.kind != PromptKind::Bookmark {
        return false;
    }
    let Some(mut pending) = prompt.pending.take() else {
        return false;
    };
    pending.label = std::mem::take(&mut prompt.input);
    prompt.input = pending.path_text.clone();
    prompt.kind = PromptKind::BookmarkFile;
    prompt.pending = Some(pending);
    prompt.error = None;
    true
}

/// Enter: do what the prompt asks. A failure stays in the prompt as an inline error.
fn submit(session: &mut Session, files: &dyn Files, now: NowFn) {
    let Some(prompt) = session.prompt() else {
        return;
    };
    match prompt.kind {
        PromptKind::GoTo => go_to(session, files),
        PromptKind::Export => export(session, files),
        PromptKind::Bookmark => add_bookmark(session, files, now()),
        PromptKind::BookmarkFile => choose_bookmark_file(session, files),
        PromptKind::BookmarkRename => rename_bookmark(session, files),
    }
}

/// Adds a bookmark for the current page, labelled with what was typed, and saves the file.
/// On the label step of a first bookmark this also creates or opens the file and locks the
/// session to it. A problem stays in the prompt, which stays open.
fn add_bookmark(session: &mut Session, files: &dyn Files, now: u64) {
    let Some(prompt) = session.prompt() else {
        return;
    };
    let label = prompt.input.clone();
    let pending = prompt.pending.clone();
    let index = session.index().clone();
    if let Some(pending) = pending {
        return start_bookmarks(session, files, (now, &label, index), pending);
    }
    let Bookmarks::Locked(locked) = session.bookmarks_mut() else {
        session.set_prompt_error("no bookmark file");
        return;
    };
    match locked.add(files, now, &label, index) {
        Ok(count) => {
            session.close_prompt();
            session.set_status(format!("bookmarked ({count})"));
        }
        Err(error) => session.set_prompt_error(error.to_string()),
    }
}

/// The rename prompt's Enter: apply the label to a copy, write the file, and only then
/// return to the list, which shows the new label (§13.3b). A failure keeps the prompt open
/// with the inline error, over the unchanged list state.
fn rename_bookmark(session: &mut Session, files: &dyn Files) {
    let Some(prompt) = session.prompt() else {
        return;
    };
    let Some(at) = prompt.rename_at else {
        return;
    };
    let label = prompt.input.clone();
    let Bookmarks::Locked(locked) = session.bookmarks_mut() else {
        session.set_prompt_error("no bookmark file");
        return;
    };
    match locked.rename(files, at, &label) {
        Ok(()) => session.open_bookmark_list(),
        Err(error) => session.set_prompt_error(error.to_string()),
    }
}

/// The label step of a first bookmark: writes the file and locks the session to it. If the
/// write fails the session is still free and both steps keep what was typed.
fn start_bookmarks(
    session: &mut Session,
    files: &dyn Files,
    bookmark: (u64, &str, PageIndex),
    pending: Pending,
) {
    let (file, is_new) = match pending.existing {
        Some(file) => (file, false),
        None => match session.library().bookmark_settings() {
            Ok(settings) => (BookmarkFile::new(settings), true),
            Err(error) => return session.set_prompt_error(error.to_string()),
        },
    };
    match LockedBookmarks::start(files, pending.path, file, is_new, bookmark) {
        Ok((locked, count)) => {
            let name = locked.name();
            session.set_bookmarks(Bookmarks::Locked(locked));
            session.close_prompt();
            session.set_status(format!("bookmarked ({count}) · locked to {name}"));
        }
        Err(error) => session.set_prompt_error(error.to_string()),
    }
}

/// The file step of a first bookmark: finds out what is at the path, and if it can be used
/// goes on to the label step. Nothing is written. A problem stays in the prompt.
fn choose_bookmark_file(session: &mut Session, files: &dyn Files) {
    let Some(prompt) = session.prompt() else {
        return;
    };
    let typed = prompt.input.trim().to_string();
    let earlier_label = prompt
        .pending
        .as_ref()
        .map(|pending| pending.label.clone())
        .unwrap_or_default();
    if typed.is_empty() {
        return session.set_prompt_error("type a file name");
    }
    let settings = match session.library().bookmark_settings() {
        Ok(settings) => settings,
        Err(error) => return session.set_prompt_error(error.to_string()),
    };
    let path = with_bookmark_extension(&typed);
    let existing = match files.read(&path, BookmarkFile::max_bytes(&settings)) {
        Ok(bytes) => match BookmarkFile::from_bytes(&bytes) {
            Ok(file) => {
                if let Some(message) = library_mismatch(&path, &file.settings(), &settings) {
                    return session.set_prompt_error(message);
                }
                Some(file)
            }
            Err(error) => {
                let message = format!("{} is not a valid bookmark file: {error}", path.display());
                return session.set_prompt_error(message);
            }
        },
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            let message = format!("cannot read {}: {error}", path.display());
            return session.set_prompt_error(message);
        }
    };
    if let Some(prompt) = session.prompt_mut() {
        prompt.kind = PromptKind::Bookmark;
        prompt.input = earlier_label.clone();
        prompt.error = None;
        prompt.pending = Some(Pending {
            path,
            path_text: typed,
            existing,
            label: earlier_label,
        });
    }
}

/// `typed` as a path, with `.babelbookmark` added if the name has no extension. A name that
/// has one, such as `notes.txt`, is kept as typed.
fn with_bookmark_extension(typed: &str) -> PathBuf {
    let path = Path::new(typed);
    if path.extension().is_some() {
        path.to_path_buf()
    } else {
        PathBuf::from(format!("{typed}.babelbookmark"))
    }
}

/// The most an index file may hold. The largest index is 8,536 base64 characters, so this is
/// room for whitespace and a final newline and no more.
const INDEX_FILE_MAX: usize = 16 * 1024;

/// Visits the index in the file the prompt names. A problem stays in the prompt.
fn go_to(session: &mut Session, files: &dyn Files) {
    let path = session
        .prompt()
        .map(|prompt| prompt.input.trim().to_string())
        .unwrap_or_default();
    if path.is_empty() {
        session.set_prompt_error("type the name of a file");
        return;
    }
    let page_len = session.index().as_bytes().len();
    let index = match read_index(files, &path, page_len) {
        Ok(index) => index,
        Err(message) => {
            session.set_prompt_error(message);
            return;
        }
    };
    match session.visit(index) {
        Ok(()) => session.close_prompt(),
        Err(error) => session.set_prompt_error(error.to_string()),
    }
}

/// Reads an index of `page_len` bytes from the file at `path`.
///
/// All whitespace is stripped, so a trailing newline or wrapped lines are fine, and so is the
/// byte-order mark some Windows editors write. Then the length picks the decoder: hex is
/// `2 * page_len` characters and padded base64 is `4 * ceil(page_len / 3)`. Nothing else is
/// accepted: no `0x` prefix, no URL-safe base64 and no unpadded base64.
///
/// # Errors
///
/// A message for the prompt: the file could not be read, is too long, is not text, or does not
/// hold an index of the right length.
fn read_index(files: &dyn Files, path: &str, page_len: usize) -> Result<PageIndex, String> {
    let bytes = match files.read(Path::new(path), INDEX_FILE_MAX) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::FileTooLarge => {
            return Err(format!(
                "{path} is longer than {} KB",
                INDEX_FILE_MAX / 1024
            ));
        }
        Err(error) => return Err(format!("cannot read {path}: {error}")),
    };
    let text = String::from_utf8(bytes).map_err(|_| "index file is not text".to_string())?;
    let text: String = text
        .trim_start_matches('\u{feff}')
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    let hex_len = page_len * 2;
    let base64_len = page_len.div_ceil(3) * 4;
    let chars = text.chars().count();
    let index = if chars == hex_len {
        PageIndex::from_hex(&text, page_len)
    } else if chars == base64_len {
        PageIndex::from_base64(&text, page_len)
    } else {
        return Err(format!(
            "{chars} chars: not {hex_len} (hex) or {base64_len} (base64, with its = padding)"
        ));
    };
    index.map_err(|error| error.to_string())
}

/// Writes the current index, as base64 and a newline, to a file that must not exist yet.
/// `g` reads the same file back.
fn export(session: &mut Session, files: &dyn Files) {
    let path = session
        .prompt()
        .map(|prompt| prompt.input.trim().to_string())
        .unwrap_or_default();
    if path.is_empty() {
        session.set_prompt_error("type a file name");
        return;
    }
    let text = format!("{}\n", session.index().to_base64());
    match files.write_new(Path::new(&path), text.as_bytes()) {
        Ok(()) => {
            session.close_prompt();
            session.set_status(format!("index written to {path}"));
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            session.set_prompt_error(format!("{path} already exists, choose another name"));
        }
        Err(error) => session.set_prompt_error(format!("cannot write {path}: {error}")),
    }
}

#[cfg(test)]
mod test {
    use bitbabel_core::LibraryConfig;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::app::test_util::*;
    use crate::config::TuiConfig;
    use crate::files::MemFiles;

    fn index(len: usize) -> PageIndex {
        PageIndex::from_bytes((0..len).map(|i| (i * 7 % 256) as u8).collect())
    }

    fn read(contents: &[u8], page_len: usize) -> Result<PageIndex, String> {
        let files = MemFiles::default().with("f", contents);
        read_index(&files, "f", page_len)
    }

    #[test]
    fn reads_hex_and_base64_by_length_at_every_size() {
        for len in [16, 3200, 6400] {
            let want = index(len);
            assert_eq!(read(want.to_string().as_bytes(), len), Ok(want.clone()));
            assert_eq!(read(want.to_base64().as_bytes(), len), Ok(want));
        }
    }

    #[test]
    fn the_lengths_are_the_documented_ones() {
        let lengths = |len| (index(len).to_string().len(), index(len).to_base64().len());
        assert_eq!(lengths(16), (32, 24));
        assert_eq!(lengths(3200), (6400, 4268));
        assert_eq!(lengths(6400), (12800, 8536));
    }

    #[test]
    fn whitespace_wrapping_and_a_bom_are_fine() {
        let want = index(16);
        let hex = want.to_string();
        let wrapped = format!("\u{feff}  {}\r\n{}\r\n\n", &hex[..10], &hex[10..]);
        assert_eq!(read(wrapped.as_bytes(), 16), Ok(want.clone()));
        let b64 = format!("{}\n", want.to_base64());
        assert_eq!(read(b64.as_bytes(), 16), Ok(want));
    }

    #[test]
    fn uppercase_hex_is_fine() {
        let want = index(16);
        let upper = want.to_string().to_uppercase();
        assert_eq!(read(upper.as_bytes(), 16), Ok(want));
    }

    #[test]
    fn a_wrong_length_says_what_lengths_are_expected() {
        assert_eq!(
            read(b"abcdefghijklmnopqrstuv", 16),
            Err("22 chars: not 32 (hex) or 24 (base64, with its = padding)".to_string())
        );
        assert_eq!(
            read(b"", 16),
            Err("0 chars: not 32 (hex) or 24 (base64, with its = padding)".to_string())
        );
    }

    #[test]
    fn forms_that_are_not_accepted_get_the_length_error_or_a_decode_error() {
        let hex = index(16).to_string();
        // A `0x` prefix makes it two characters too long.
        let prefixed = format!("0x{hex}");
        assert!(
            read(prefixed.as_bytes(), 16)
                .unwrap_err()
                .starts_with("34 chars")
        );
        // Unpadded base64 is shorter than 24.
        let unpadded = index(16).to_base64().trim_end_matches('=').to_string();
        assert!(
            read(unpadded.as_bytes(), 16)
                .unwrap_err()
                .starts_with("22 chars")
        );
        // Right length, but not hex digits.
        let bad = "g".repeat(32);
        assert_eq!(
            read(bad.as_bytes(), 16),
            Err("invalid index: invalid hex digit 'g'".to_string())
        );
    }

    #[test]
    fn a_file_that_is_not_text_a_missing_file_and_a_big_file_are_messages() {
        assert_eq!(
            read(&[0xff, 0xfe, 0x00], 16),
            Err("index file is not text".to_string())
        );
        let files = MemFiles::default().with("big", &vec![b'a'; INDEX_FILE_MAX + 1]);
        assert_eq!(
            read_index(&files, "big", 16),
            Err("big is longer than 16 KB".to_string())
        );
        let none = MemFiles::default();
        assert_eq!(
            read_index(&none, "nope", 16),
            Err("cannot read nope: No such file or directory".to_string())
        );
    }

    fn prompt_of(app: &App) -> Option<&Prompt> {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session.prompt()
    }

    #[test]
    fn x_opens_the_export_prompt_and_esc_closes_it() {
        let mut app = medium_app();
        assert!(prompt_of(&app).is_none());
        app.handle_key(press(KeyCode::Char('x')));
        assert_eq!(prompt_of(&app).unwrap().title(), "export index");
        app.handle_key(press(KeyCode::Esc));
        assert!(prompt_of(&app).is_none());
    }

    #[test]
    fn typing_into_a_prompt_never_runs_a_command() {
        let mut app = medium_app();
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('x')));
        // `l` would step a page and `q` would quit if the prompt did not take them.
        type_text(&mut app, "lq.txt");
        assert_eq!(prompt_of(&app).unwrap().input(), "lq.txt");
        assert_eq!(index_of(&app), start);
        app.handle_key(press(KeyCode::Backspace));
        assert_eq!(prompt_of(&app).unwrap().input(), "lq.tx");
    }

    #[test]
    fn ctrl_c_still_quits_with_a_prompt_open() {
        let mut app = medium_app();
        app.handle_key(press(KeyCode::Char('x')));
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(ctrl_c), [Effect::Quit]);
    }

    /// An app on `files`, small size, with the export prompt open and `name` typed.
    fn export_to(files: MemFiles, name: &str) -> App {
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: None,
            bookmarks: None,
        };
        let mut app = App::new(config, fake_random, Box::new(files)).unwrap();
        app.handle_key(press(KeyCode::Char('x')));
        type_text(&mut app, name);
        app
    }

    #[test]
    fn export_writes_the_index_as_base64_and_a_newline() {
        let files = std::sync::Arc::new(MemFiles::default());
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: None,
            bookmarks: None,
        };
        let mut app = App::new(config, fake_random, Box::new(SharedFiles(files.clone()))).unwrap();
        app.handle_key(press(KeyCode::Char('l')));
        app.handle_key(press(KeyCode::Char('x')));
        type_text(&mut app, "idx.txt");
        app.handle_key(press(KeyCode::Enter));

        let expected = format!("{}\n", index_of(&app).to_base64());
        assert_eq!(files.get("idx.txt").unwrap(), expected.as_bytes());
        assert!(prompt_of(&app).is_none());
        assert_eq!(status_of(&app).unwrap().text, "index written to idx.txt");
    }

    #[test]
    fn export_refuses_an_existing_file_and_keeps_the_prompt_open() {
        let files = MemFiles::default().with("taken.txt", b"keep me");
        let mut app = export_to(files, "taken.txt");
        app.handle_key(press(KeyCode::Enter));
        let prompt = prompt_of(&app).unwrap();
        assert_eq!(
            prompt.error(),
            Some("taken.txt already exists, choose another name")
        );
        // The name is still there to edit, and typing clears the error.
        assert_eq!(prompt.input(), "taken.txt");
        app.handle_key(press(KeyCode::Char('2')));
        assert_eq!(prompt_of(&app).unwrap().error(), None);
    }

    #[test]
    fn export_with_no_name_says_so() {
        let mut app = export_to(MemFiles::default(), "  ");
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(prompt_of(&app).unwrap().error(), Some("type a file name"));
    }

    #[test]
    fn g_visits_the_index_that_x_exported() {
        let files = std::sync::Arc::new(MemFiles::default());
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::MEDIUM),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(SharedFiles(files)),
        )
        .unwrap();
        app.handle_key(press(KeyCode::Char('l')));
        let saved = index_of(&app);

        app.handle_key(press(KeyCode::Char('x')));
        type_text(&mut app, "here.txt");
        app.handle_key(press(KeyCode::Enter));

        app.handle_key(press(KeyCode::Char('r')));
        assert_ne!(index_of(&app), saved);

        app.handle_key(press(KeyCode::Char('g')));
        assert_eq!(prompt_of(&app).unwrap().title(), "go to");
        type_text(&mut app, "here.txt");
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(index_of(&app), saved);
        assert!(prompt_of(&app).is_none());
    }

    #[test]
    fn g_with_a_bad_file_keeps_the_prompt_open_and_the_page() {
        let files = MemFiles::default().with("short.txt", b"abc");
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(files),
        )
        .unwrap();
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('g')));
        type_text(&mut app, "short.txt");
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(index_of(&app), start);
        assert_eq!(
            prompt_of(&app).unwrap().error(),
            Some("3 chars: not 32 (hex) or 24 (base64, with its = padding)")
        );
        app.handle_key(press(KeyCode::Esc));
        assert!(prompt_of(&app).is_none());
    }

    #[test]
    fn a_went_to_page_is_a_visit_so_back_returns() {
        let mut want = bitbabel_core::PageIndex::zero(16).to_string();
        want.replace_range(31.., "7");
        let files = MemFiles::default().with("seven.txt", want.as_bytes());
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(files),
        )
        .unwrap();
        let start = index_of(&app);
        app.handle_key(press(KeyCode::Char('g')));
        type_text(&mut app, "seven.txt");
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(index_of(&app), bitbabel_core::PageIndex::from_u64(7, 16));
        app.handle_key(press(KeyCode::Left));
        assert_eq!(index_of(&app), start);
    }
    fn marks_in(bytes: Vec<u8>) -> bitbabel_file::BookmarkFile {
        bitbabel_file::BookmarkFile::from_bytes(&bytes).unwrap()
    }

    fn open_prompt_error(app: &App) -> Option<String> {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session.prompt().and_then(|p| p.error().map(str::to_string))
    }

    fn prompt_is_open(app: &App) -> bool {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session.prompt().is_some()
    }

    fn marked_files() -> std::sync::Arc<MemFiles> {
        std::sync::Arc::new(MemFiles::default().with(MARKS, &empty_marks()))
    }

    #[test]
    fn b_asks_for_a_label_then_saves_the_page_with_the_time() {
        let files = marked_files();
        let mut app = bookmarked_app(files.clone());
        app.handle_key(press(KeyCode::Char('b')));
        assert!(prompt_is_open(&app));
        type_text(&mut app, "looks like a word");
        app.handle_key(press(KeyCode::Enter));

        assert!(!prompt_is_open(&app));
        assert_eq!(status_of(&app).unwrap().text, "bookmarked (1)");
        let saved = marks_in(files.get(MARKS).unwrap());
        let mark = &saved.bookmarks()[0];
        assert_eq!(mark.label(), "looks like a word");
        assert_eq!(mark.time(), fixed_now());
        assert_eq!(mark.index(), &index_of(&app));
    }

    #[test]
    fn an_empty_label_is_fine_and_a_second_page_makes_two() {
        let files = marked_files();
        let mut app = bookmarked_app(files.clone());
        app.handle_key(press(KeyCode::Char('b')));
        app.handle_key(press(KeyCode::Enter));
        app.handle_key(press(KeyCode::Char('l')));
        app.handle_key(press(KeyCode::Char('b')));
        type_text(&mut app, "next");
        app.handle_key(press(KeyCode::Enter));

        assert_eq!(status_of(&app).unwrap().text, "bookmarked (2)");
        let saved = marks_in(files.get(MARKS).unwrap());
        assert_eq!(saved.bookmarks().len(), 2);
        assert_eq!(saved.bookmarks()[0].label(), "");
    }

    #[test]
    fn bookmarking_a_page_twice_is_refused_and_names_the_first_label() {
        let files = marked_files();
        let mut app = bookmarked_app(files.clone());
        app.handle_key(press(KeyCode::Char('b')));
        type_text(&mut app, "first");
        app.handle_key(press(KeyCode::Enter));
        let before = files.get(MARKS).unwrap();

        app.handle_key(press(KeyCode::Char('b')));
        type_text(&mut app, "again");
        app.handle_key(press(KeyCode::Enter));
        assert!(prompt_is_open(&app));
        assert_eq!(
            open_prompt_error(&app).unwrap(),
            "this page is already bookmarked as \"first\""
        );
        assert_eq!(files.get(MARKS).unwrap(), before);
    }

    #[test]
    fn esc_cancels_and_writes_nothing() {
        let files = marked_files();
        let before = files.get(MARKS).unwrap();
        let mut app = bookmarked_app(files.clone());
        app.handle_key(press(KeyCode::Char('b')));
        type_text(&mut app, "never saved");
        app.handle_key(press(KeyCode::Esc));
        assert!(!prompt_is_open(&app));
        assert_eq!(files.get(MARKS).unwrap(), before);
    }

    #[test]
    fn the_label_box_ignores_characters_a_label_cannot_hold_and_stops_at_200() {
        let mut app = bookmarked_app(marked_files());
        app.handle_key(press(KeyCode::Char('b')));
        // A zero-width space and an escape are ignored; other keys still type.
        type_text(&mut app, "a\u{200b}b\u{1b}c");
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        assert_eq!(session.prompt().unwrap().input(), "abc");

        type_text(&mut app, &"x".repeat(300));
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        assert_eq!(session.prompt().unwrap().input().chars().count(), 200);
    }

    #[test]
    fn a_failed_write_is_an_inline_error_and_leaves_the_list_as_it_was() {
        let files =
            std::sync::Arc::new(crate::files::FailingFiles::default().with(MARKS, &empty_marks()));
        let mut app = bookmarked_app(files.clone());
        files.fail_writes(true);
        app.handle_key(press(KeyCode::Char('b')));
        type_text(&mut app, "kept");
        app.handle_key(press(KeyCode::Enter));

        assert!(prompt_is_open(&app));
        assert_eq!(
            open_prompt_error(&app).unwrap(),
            "cannot save marks.babelbookmark: No space left on device"
        );
        assert_eq!(files.get(MARKS).unwrap(), empty_marks());

        // Once the disk works again the same Enter saves it: one bookmark, not a duplicate
        // error, because the failed attempt was rolled back.
        files.fail_writes(false);
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(status_of(&app).unwrap().text, "bookmarked (1)");
        assert_eq!(marks_in(files.get(MARKS).unwrap()).bookmarks().len(), 1);
    }

    #[test]
    fn b_is_text_inside_a_prompt() {
        let mut app = bookmarked_app(marked_files());
        app.handle_key(press(KeyCode::Char('g')));
        app.handle_key(press(KeyCode::Char('b')));
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        assert_eq!(session.prompt().unwrap().input(), "b");
        assert_eq!(session.prompt().unwrap().title(), "go to");
    }

    // --- the first bookmark of a session without a file ---

    fn session_of(app: &App) -> &Session {
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        session
    }

    fn is_locked(app: &App) -> bool {
        matches!(session_of(app).bookmarks(), Bookmarks::Locked(_))
    }

    /// A small canonical session with no bookmark file, over `files`.
    fn free_app<F: Files + 'static>(files: std::sync::Arc<F>) -> App {
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: None,
            bookmarks: None,
        };
        App::new(config, fake_random, Box::new(SharedFiles(files)))
            .unwrap()
            .with_clock(fixed_now)
    }

    /// `b`, then `path` and Enter: on the label step.
    fn choose_file(app: &mut App, path: &str) {
        app.handle_key(press(KeyCode::Char('b')));
        type_text(app, path);
        app.handle_key(press(KeyCode::Enter));
    }

    fn small_file_with(count: u64) -> Vec<u8> {
        use bitbabel_file::{BookmarkFile, BookmarkKey, BookmarkSettings};
        let settings = BookmarkSettings::new(LibraryConfig::SMALL, BookmarkKey::Canonical).unwrap();
        let mut file = BookmarkFile::new(settings);
        for n in 0..count {
            // Indices far from the first page, which a test bookmarks.
            let index = PageIndex::from_u64(1000 + n, 16);
            file.push(5, &format!("old {n}"), index).unwrap();
        }
        file.to_bytes()
    }

    #[test]
    fn b_without_a_file_asks_for_one_first() {
        let mut app = free_app(std::sync::Arc::new(MemFiles::default()));
        app.handle_key(press(KeyCode::Char('b')));
        let prompt = session_of(&app).prompt().unwrap();
        assert_eq!(prompt.title(), "bookmark file");
        assert!(prompt.label().starts_with("keep bookmarks in which file?"));
        assert_eq!(prompt.footer(), "enter next   esc cancel");
    }

    #[test]
    fn a_new_file_is_written_only_when_the_label_is_confirmed() {
        let files = std::sync::Arc::new(MemFiles::default());
        let mut app = free_app(files.clone());
        choose_file(&mut app, "favs");

        // On the label step: the name has its extension, and nothing exists yet.
        let prompt = session_of(&app).prompt().unwrap();
        assert_eq!(prompt.title(), "bookmark");
        assert!(
            prompt.label().contains("favs.babelbookmark"),
            "{}",
            prompt.label()
        );
        assert!(prompt.label().contains("locks the session"));
        assert_eq!(prompt.footer(), "enter save   esc back");
        assert_eq!(files.get("favs.babelbookmark"), None);
        assert!(!is_locked(&app));

        type_text(&mut app, "first find");
        app.handle_key(press(KeyCode::Enter));
        assert!(!prompt_is_open(&app));
        assert!(is_locked(&app));
        assert_eq!(
            status_of(&app).unwrap().text,
            "bookmarked (1) · locked to favs"
        );
        let saved = marks_in(files.get("favs.babelbookmark").unwrap());
        assert_eq!(saved.bookmarks()[0].label(), "first find");
        assert_eq!(saved.bookmarks()[0].time(), fixed_now());
        assert_eq!(saved.bookmarks()[0].index(), &index_of(&app));
        assert_eq!(saved.settings().size(), LibraryConfig::SMALL);

        // From now on `b` goes straight to the label.
        app.handle_key(press(KeyCode::Char('l')));
        app.handle_key(press(KeyCode::Char('b')));
        assert_eq!(session_of(&app).prompt().unwrap().title(), "bookmark");
        assert!(!session_of(&app).prompt().unwrap().label().contains("locks"));
    }

    #[test]
    fn a_typed_extension_is_kept_and_a_missing_one_is_added() {
        assert_eq!(
            with_bookmark_extension("favs"),
            PathBuf::from("favs.babelbookmark")
        );
        assert_eq!(
            with_bookmark_extension("dir/notes.txt"),
            PathBuf::from("dir/notes.txt")
        );
        assert_eq!(
            with_bookmark_extension("dir/favs.babelbookmark"),
            PathBuf::from("dir/favs.babelbookmark")
        );
    }

    #[test]
    fn esc_steps_back_from_the_label_to_the_file_and_then_closes_with_nothing_written() {
        let files = std::sync::Arc::new(MemFiles::default());
        let mut app = free_app(files.clone());
        choose_file(&mut app, "favs");
        type_text(&mut app, "half typed");

        app.handle_key(press(KeyCode::Esc));
        let prompt = session_of(&app).prompt().unwrap();
        assert_eq!(prompt.title(), "bookmark file");
        assert_eq!(prompt.input(), "favs");

        // Coming forward again shows the label that was typed.
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(session_of(&app).prompt().unwrap().input(), "half typed");

        app.handle_key(press(KeyCode::Esc));
        app.handle_key(press(KeyCode::Esc));
        assert!(!prompt_is_open(&app));
        assert!(!is_locked(&app));
        assert_eq!(files.get("favs.babelbookmark"), None);
    }

    #[test]
    fn an_empty_file_name_is_an_inline_error() {
        let mut app = free_app(std::sync::Arc::new(MemFiles::default()));
        app.handle_key(press(KeyCode::Char('b')));
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(open_prompt_error(&app).unwrap(), "type a file name");
        assert_eq!(session_of(&app).prompt().unwrap().title(), "bookmark file");
    }

    #[test]
    fn an_existing_file_of_this_library_gets_the_new_bookmark_after_its_own() {
        let files = std::sync::Arc::new(
            MemFiles::default().with("favs.babelbookmark", &small_file_with(2)),
        );
        let mut app = free_app(files.clone());
        choose_file(&mut app, "favs");
        type_text(&mut app, "new one");
        app.handle_key(press(KeyCode::Enter));

        assert_eq!(
            status_of(&app).unwrap().text,
            "bookmarked (3) · locked to favs"
        );
        let saved = marks_in(files.get("favs.babelbookmark").unwrap());
        let labels: Vec<_> = saved.bookmarks().iter().map(|b| b.label()).collect();
        assert_eq!(labels, ["old 0", "old 1", "new one"]);
    }

    #[test]
    fn a_file_of_another_library_is_an_inline_error_that_names_both() {
        let medium = {
            use bitbabel_file::{BookmarkFile, BookmarkKey, BookmarkSettings};
            let settings =
                BookmarkSettings::new(LibraryConfig::MEDIUM, BookmarkKey::Canonical).unwrap();
            BookmarkFile::new(settings).to_bytes()
        };
        let files = std::sync::Arc::new(MemFiles::default().with("other.babelbookmark", &medium));
        let mut app = free_app(files.clone());
        choose_file(&mut app, "other");
        assert_eq!(
            open_prompt_error(&app).unwrap(),
            "other.babelbookmark is a medium library, but this session is small"
        );
        // Still on the file step, so the name can be fixed.
        assert_eq!(session_of(&app).prompt().unwrap().title(), "bookmark file");
        assert_eq!(files.get("other.babelbookmark").unwrap(), medium);
    }

    #[test]
    fn a_file_that_is_not_a_bookmark_file_is_an_inline_error_and_is_not_touched() {
        let files = std::sync::Arc::new(MemFiles::default().with("notes.txt", b"hello\n"));
        let mut app = free_app(files.clone());
        choose_file(&mut app, "notes.txt");
        assert_eq!(
            open_prompt_error(&app).unwrap(),
            "notes.txt is not a valid bookmark file: not a bookmark file"
        );
        assert_eq!(files.get("notes.txt").unwrap(), b"hello\n");
    }

    #[test]
    fn a_page_already_in_the_file_is_refused_on_the_label_step_with_nothing_written() {
        use bitbabel_file::{BookmarkFile, BookmarkKey, BookmarkSettings};
        let settings = BookmarkSettings::new(LibraryConfig::SMALL, BookmarkKey::Canonical).unwrap();
        let mut file = BookmarkFile::new(settings);
        file.push(5, "already here", PageIndex::zero(16)).unwrap();
        let bytes = file.to_bytes();
        let files = std::sync::Arc::new(MemFiles::default().with("favs.babelbookmark", &bytes));
        let mut app = free_app(files.clone());
        // The session starts on page 0, which is the bookmarked one.
        choose_file(&mut app, "favs");
        type_text(&mut app, "again");
        app.handle_key(press(KeyCode::Enter));

        assert_eq!(
            open_prompt_error(&app).unwrap(),
            "this page is already bookmarked as \"already here\""
        );
        assert_eq!(session_of(&app).prompt().unwrap().input(), "again");
        assert!(!is_locked(&app));
        assert_eq!(files.get("favs.babelbookmark").unwrap(), bytes);
    }

    #[test]
    fn a_failed_first_write_keeps_both_steps_and_the_session_free_and_retrying_works() {
        let files = std::sync::Arc::new(crate::files::FailingFiles::default());
        let mut app = free_app(files.clone());
        files.fail_writes(true);
        choose_file(&mut app, "favs");
        type_text(&mut app, "kept");
        app.handle_key(press(KeyCode::Enter));

        assert_eq!(
            open_prompt_error(&app).unwrap(),
            "cannot save favs.babelbookmark: No space left on device"
        );
        assert_eq!(session_of(&app).prompt().unwrap().input(), "kept");
        assert!(!is_locked(&app));
        assert_eq!(files.get("favs.babelbookmark"), None);

        files.fail_writes(false);
        app.handle_key(press(KeyCode::Enter));
        assert!(is_locked(&app));
        assert_eq!(
            marks_in(files.get("favs.babelbookmark").unwrap())
                .bookmarks()
                .len(),
            1
        );
    }

    #[test]
    fn a_private_key_session_writes_a_file_for_its_fingerprint() {
        let files = std::sync::Arc::new(MemFiles::default());
        let key = bitbabel_core::Key::from_bytes([1; 32]);
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: Some(key.clone()),
            bookmarks: None,
        };
        let mut app = App::new(config, fake_random, Box::new(SharedFiles(files.clone())))
            .unwrap()
            .with_clock(fixed_now);
        choose_file(&mut app, "mine");
        app.handle_key(press(KeyCode::Enter));

        let text = String::from_utf8(files.get("mine.babelbookmark").unwrap()).unwrap();
        let fingerprint =
            <bitbabel_core::Hex as bitbabel_core::Encoding>::encode(&key.fingerprint());
        assert!(
            text.starts_with(&format!(
                "BABELMARKS1 size=small key=custom fingerprint={fingerprint} format=base64\n"
            )),
            "{text}"
        );
    }

    #[test]
    fn a_private_key_session_refuses_a_canonical_file() {
        let files =
            std::sync::Arc::new(MemFiles::default().with("pub.babelbookmark", &small_file_with(0)));
        let config = TuiConfig {
            size: Some(LibraryConfig::SMALL),
            key: Some(bitbabel_core::Key::from_bytes([1; 32])),
            bookmarks: None,
        };
        let mut app = App::new(config, fake_random, Box::new(SharedFiles(files)))
            .unwrap()
            .with_clock(fixed_now);
        choose_file(&mut app, "pub");
        assert_eq!(
            open_prompt_error(&app).unwrap(),
            "pub.babelbookmark is a canonical library, but this session uses a private key"
        );
    }
}
