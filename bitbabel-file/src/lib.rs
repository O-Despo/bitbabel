//! Turns arbitrary bytes into a list of Library of Babel page indices, and back.
//!
//! This re-addresses data; it does not compress or encrypt it. The crate works on bytes only:
//! file IO, stdin and stdout, and key prompting belong to the caller.
//!
//! # How it fits together
//!
//! - [`pad`] and [`unpad`] bring data to a whole number of pages and back.
//! - [`IndexFormat`] writes a list of page indices as raw bytes, hex lines or base64 lines.
//! - [`Settings`] records the size, [`KeyMode`], format and whether to add a checksum. A file
//!   writes them, with the checksum, as its one-line header.
//! - [`BookmarkFile`] is the other kind of file: a list of [`Bookmark`]s (time, label, index)
//!   in one library, named by [`BookmarkSettings`] and [`BookmarkKey`].
//! - [`BabelFile`] ties them together: [`encode`](BabelFile::encode) and
//!   [`decode`](BabelFile::decode) turn data into indices and back, and
//!   [`to_bytes`](BabelFile::to_bytes) and [`from_bytes`](BabelFile::from_bytes) write and read
//!   the file.

mod bookmarks;
mod check;
mod error;
mod file;
mod format;
mod grammar;
mod header;
mod pad;

pub use bookmarks::{
    Bookmark, BookmarkFile, BookmarkKey, BookmarkSettings, MAX_BOOKMARKS, MAX_LABEL_CHARS,
};
pub use error::{BookmarkError, FileError, HeaderError, IndexFormatError, LineError, PadError};
pub use file::BabelFile;
pub use format::IndexFormat;
pub use header::{KeyMode, Settings};
pub use pad::{pad, unpad};
