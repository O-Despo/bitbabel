//! Turns arbitrary bytes into a list of Library of Babel page indices, and back.
//!
//! This re-addresses data; it does not compress or encrypt it. The crate works on bytes only:
//! file IO, stdin and stdout, and key prompting belong to the caller.
//!
//! # How it fits together
//!
//! - [`pad`] and [`unpad`] bring data to a whole number of pages and back.

mod error;
mod pad;

pub use error::PadError;
pub use pad::{pad, unpad};
