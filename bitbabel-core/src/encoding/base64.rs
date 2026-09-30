use std::error::Error;
use std::fmt;

use base64::{DecodeError as EngineDecodeError, Engine, engine::general_purpose::STANDARD};

use super::Encoding;

/// Standard base64 (with padding) encoding.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Base64;

/// The input was not valid standard base64.
///
/// The underlying `base64` error is kept private so this crate's API does not depend on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base64DecodeError(EngineDecodeError);

impl fmt::Display for Base64DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid base64: {}", self.0)
    }
}

// No `source()`: `Display` already includes the engine's message.
impl Error for Base64DecodeError {}

impl Encoding for Base64 {
    type Output = String;
    type DecodeError = Base64DecodeError;

    fn encode(bytes: &[u8]) -> Self::Output {
        STANDARD.encode(bytes)
    }

    fn decode(value: &Self::Output) -> Result<Vec<u8>, Self::DecodeError> {
        STANDARD.decode(value).map_err(Base64DecodeError)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn round_trips() {
        let data = vec![1, 2, 3, 250, 255, 0, 128];
        let encoded = Base64::encode(&data);
        let decoded = Base64::decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(Base64::decode(&"not valid base64!!".to_string()).is_err());
    }
}
