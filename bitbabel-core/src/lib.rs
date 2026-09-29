//! A deterministic "Library of Babel": every index maps to exactly one page of bytes, and
//! every page of that length maps back to exactly one index.
//!
//! # How it fits together
//!
//! - [`PageIndex`] is an arbitrary-length big-endian index.
//! - [`BabelMachine`] is a keyed Feistel permutation (see [`FeistelBytes`]); the library runs
//!   an index through it to get a [`Page`], and runs a page backward to get its index.
//! - [`BabelLibrary`] ties a machine to a fixed page length; [`Cursor`] walks through it.
//! - [`LibraryConfig`] names ready-made shapes ([`SMALL`](LibraryConfig::SMALL),
//!   [`MEDIUM`](LibraryConfig::MEDIUM), [`LARGE`](LibraryConfig::LARGE)), each with a shared
//!   canonical key so everyone sees the same library.
//! - [`Encoding`] types ([`Hex`], [`Base64`], [`Utf8`], ...) render a page's bytes.
//!
//! # Example
//!
//! ```
//! use bitbabel_core::{BabelLibrary, Cursor, Hex, LibraryConfig, PageIndex};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = LibraryConfig::SMALL;
//! let library = BabelLibrary::canonical(config)?;
//!
//! let mut cursor = Cursor::new(library.clone(), PageIndex::zero(config.page_len()))?;
//! let page = cursor.next_page()?;
//! println!("{}", page.encode_as::<Hex>());
//!
//! // Every page can be traced back to where it lives.
//! assert_eq!(library.index_of(page.bytes())?, *page.index());
//! # Ok(())
//! # }
//! ```

mod cipher;
mod config;
mod cursor;
mod encoding;
mod error;
mod index;
mod library;
mod page;

pub use cipher::{BabelMachine, FeistelBytes};
pub use config::LibraryConfig;
pub use cursor::Cursor;
pub use encoding::{
    Base64, Base64DecodeError, Encoding, Hex, HexDecodeError, Raw, Utf8, Utf16Be, Utf16Le,
};
pub use error::{CipherError, LibraryError};
pub use index::PageIndex;
pub use library::BabelLibrary;
pub use page::Page;
