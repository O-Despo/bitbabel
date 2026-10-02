//! A deterministic "Library of Babel": every index maps to exactly one page of bytes, and
//! every page of that length maps back to exactly one index.
//!
//! # How it fits together
//!
//! - [`PageIndex`] is an arbitrary-length big-endian index.
//! - [`BabelMachine`] is a keyed Feistel permutation (see [`FeistelBytes`]); the library runs
//!   an index through it to get a [`Page`], and runs a page backward to get its index.
//! - [`Key`] is the 256-bit key that names a library. It is mixed with the page length and
//!   round count before use, so a key plus a [`LibraryConfig`] names exactly one library.
//! - [`BabelLibrary`] ties a machine to a fixed page length; [`Cursor`] walks through it.
//!   [`BabelLibrary::search`] finds pages that contain a given value.
//! - [`LibraryConfig`] names ready-made shapes ([`SMALL`](LibraryConfig::SMALL),
//!   [`MEDIUM`](LibraryConfig::MEDIUM), [`LARGE`](LibraryConfig::LARGE)), each with a shared
//!   canonical key so everyone sees the same library.
//! - [`PageIndex::location`] gives a display address ([`Location`]): hexagon, wall, shelf and
//!   volume, after Borges' story.
//! - [`Encoding`] types ([`Hex`], [`Base64`], [`Utf8`], ...) render a page's bytes.
//!   [`BabelGuaranteedText`] shows each byte as exactly one visible character.
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

// cipher.rs is frozen, so newer clippy lints are allowed here instead of fixed.
#[allow(clippy::manual_is_multiple_of)]
mod cipher;
mod config;
mod cursor;
mod encoding;
mod error;
mod index;
mod key;
mod library;
mod location;
mod page;
mod search;

pub use cipher::{BabelMachine, FeistelBytes};
pub use config::LibraryConfig;
pub use cursor::Cursor;
pub use encoding::{
    BabelGuaranteedText, BabelGuaranteedTextDecodeError, Base64, Base64DecodeError, Encoding, Hex,
    HexDecodeError, Raw, Utf8, Utf16Be, Utf16Le,
};
pub use error::{
    CipherError, ConfigNameError, KeyError, LibraryError, LocationError, PageIndexError,
    SearchError,
};
pub use index::PageIndex;
pub use key::{FINGERPRINT_LEN, Key};
pub use library::BabelLibrary;
pub use location::Location;
pub use page::Page;
pub use search::{Fill, Placement, SearchOptions, SearchResult};
