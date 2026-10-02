//! The source of random bytes. Core has no randomness, so the app is handed one.

use std::error::Error;
use std::fmt;

/// Fills the buffer with random bytes. The real one is [`os_random`]; tests pass a fake.
pub type RandomFn = fn(&mut [u8]) -> Result<(), RandomError>;

/// The random source failed. The app shows this inline and carries on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomError(String);

impl RandomError {
    /// An error carrying `message`, for fake sources in tests.
    pub fn new(message: impl Into<String>) -> Self {
        RandomError(message.into())
    }
}

impl fmt::Display for RandomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot get random bytes from the OS: {}", self.0)
    }
}

impl Error for RandomError {}

impl From<getrandom::Error> for RandomError {
    fn from(error: getrandom::Error) -> Self {
        RandomError(error.to_string())
    }
}

/// Random bytes from the operating system.
///
/// # Errors
///
/// [`RandomError`] if the OS cannot supply bytes.
pub fn os_random(buf: &mut [u8]) -> Result<(), RandomError> {
    getrandom::fill(buf)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn os_random_fills_the_buffer() {
        // 32 zero bytes from a working source are a 1 in 2^256 event.
        let mut buf = [0u8; 32];
        os_random(&mut buf).unwrap();
        assert_ne!(buf, [0u8; 32]);
    }

    #[test]
    fn display_says_where_it_came_from() {
        assert_eq!(
            RandomError::new("blocked").to_string(),
            "cannot get random bytes from the OS: blocked"
        );
    }
}
