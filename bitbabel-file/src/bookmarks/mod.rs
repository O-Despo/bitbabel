//! The bookmark file: a list of page indices with labels and times, kept in one library.
//!
//! ```text
//! BABELMARKS1 size=medium key=custom fingerprint=3f9a1c07e2b84d10 format=base64\n
//! ```
//!
//! This is a different kind of file from `.babel`. It has its own magic word, so neither
//! reader mistakes the other's files for its own. Like `.babel`, it works on bytes only.

mod header;
mod line;

pub use header::{BookmarkKey, BookmarkSettings};
pub use line::{Bookmark, MAX_LABEL_CHARS};
