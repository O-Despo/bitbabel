//! A deterministic "Library of Babel": every index maps to exactly one page of bytes, and
//! every page of that length maps back to exactly one index.
//!
//! # How it fits together
//!
//! - [`PageIndex`] is an arbitrary-length big-endian index.
//! - [`BabelMachine`] is a keyed Feistel permutation (see [`FeistelBytes`]); the library runs
//!   an index through it to get a [`Page`], and runs a page backward to get its index.
//! - [`BabelLibrary`] ties a machine to a fixed page length; [`Cursor`] walks through it.
//! - [`Encoding`] types ([`Hex`], [`Base64`], [`Utf8`], ...) render a page's bytes.
//!
//! # Example
//!
//! ```
//! use bitbabel_core::{BabelLibrary, BabelMachine, Cursor, Hex, PageIndex};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let machine = BabelMachine::new([7u8; 32], 4)?;
//! let library = BabelLibrary::new(machine, 16)?;
//!
//! let mut cursor = Cursor::new(library.clone(), PageIndex::zero(16))?;
//! let page = cursor.next_page()?;
//! println!("{}", page.encode_as::<Hex>());
//!
//! // Every page can be traced back to where it lives.
//! assert_eq!(library.index_of(page.bytes())?, *page.index());
//! # Ok(())
//! # }
//! ```

mod cipher;
mod cursor;
mod encoding;
mod error;
mod index;
mod library;
mod page;

pub use cipher::{BabelMachine, FeistelBytes};
pub use cursor::Cursor;
pub use encoding::{
    Base64, Base64DecodeError, Encoding, Hex, HexDecodeError, Raw, Utf8, Utf16Be, Utf16Le,
};
pub use error::{CipherError, LibraryError};
pub use index::PageIndex;
pub use library::BabelLibrary;
pub use page::Page;
