//! The TUI's errors. Only startup and terminal failures are fatal; everything inside the
//! explorer is shown inline.

use std::error::Error;
use std::fmt;
use std::io;

/// Why [`run`](crate::run) failed.
#[derive(Debug)]
pub enum TuiError {
    /// Setting up, drawing on or reading from the terminal failed.
    Terminal(io::Error),
}

impl fmt::Display for TuiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TuiError::Terminal(error) => write!(f, "terminal error: {error}"),
        }
    }
}

// No `source()`: `Display` already includes the inner error's message.
impl Error for TuiError {}

impl From<io::Error> for TuiError {
    fn from(error: io::Error) -> Self {
        TuiError::Terminal(error)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn display_includes_the_inner_message() {
        let error = TuiError::from(io::Error::other("no tty"));
        assert_eq!(error.to_string(), "terminal error: no tty");
        assert!(error.source().is_none());
    }
}
