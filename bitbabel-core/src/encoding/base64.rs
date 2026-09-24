use base64::{engine::general_purpose::STANDARD, DecodeError as EngineDecodeError, Engine};

use super::Encoding;

/// Standard base64 (with padding) encoding.
pub struct Base64;

#[derive(Debug)]
pub struct Base64DecodeError(pub EngineDecodeError);

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
