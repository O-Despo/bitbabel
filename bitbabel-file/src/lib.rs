//! Turns arbitrary bytes into a list of Library of Babel page indices, and back.
//!
//! This re-addresses data; it does not compress or encrypt it. The crate works on bytes only:
//! file IO, stdin and stdout, and key prompting belong to the caller.
//!
//! # How it fits together
//!
//! - [`pad`] and [`unpad`] bring data to a whole number of pages and back.
//! - [`IndexFormat`] writes a list of page indices as raw bytes, hex lines or base64 lines.
//! - [`Settings`] records the size, [`KeyMode`] and format, and writes and reads them as the
//!   file's one-line header.

mod error;
mod format;
mod header;
mod pad;

pub use error::{HeaderError, IndexFormatError, PadError};
pub use format::IndexFormat;
pub use header::{KeyMode, Settings};
pub use pad::{pad, unpad};
