//! The parts of a BitBabel explorer that don't draw anything, shared by the terminal and web
//! front ends: how page bytes are laid out in a view, how a typed search needle becomes bytes,
//! the pages visited so far, and which library (size and key) is open. There is no terminal, browser or file IO here.

mod history;
mod library;
mod needle;
mod render;

pub use history::History;
pub use library::{LibraryChoice, SIZES, fingerprint_groups, fingerprint_text};
pub use needle::parse as parse_needle;
pub use render::{DisplayLine, Segment, View, render};
