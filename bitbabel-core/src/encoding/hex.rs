use std::error::Error;
use std::fmt;

use super::Encoding;

/// Lowercase hex-string encoding.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Hex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HexDecodeError {
    OddLength,
    InvalidDigit(char),
}

impl fmt::Display for HexDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HexDecodeError::OddLength => write!(f, "hex string has an odd number of digits"),
            HexDecodeError::InvalidDigit(c) => write!(f, "invalid hex digit {c:?}"),
        }
    }
}

impl Error for HexDecodeError {}

impl Encoding for Hex {
    type Output = String;
    type DecodeError = HexDecodeError;

    fn encode(bytes: &[u8]) -> Self::Output {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn decode(value: &Self::Output) -> Result<Vec<u8>, Self::DecodeError> {
        fn digit(c: char) -> Result<u8, HexDecodeError> {
            c.to_digit(16)
                .map(|d| d as u8)
                .ok_or(HexDecodeError::InvalidDigit(c))
        }

        let mut chars = value.chars();
        let mut bytes = Vec::with_capacity(value.len() / 2);
        while let Some(hi) = chars.next() {
            let lo = chars.next().ok_or(HexDecodeError::OddLength)?;
            bytes.push(digit(hi)? << 4 | digit(lo)?);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn encode_known_bytes() {
        assert_eq!(Hex::encode(&[0x00, 0xff, 0x0a]), "00ff0a");
    }

    #[test]
    fn round_trips() {
        let data = vec![1, 2, 3, 250, 255, 0];
        let encoded = Hex::encode(&data);
        let decoded = Hex::decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn rejects_odd_length() {
        assert_eq!(
            Hex::decode(&"abc".to_string()),
            Err(HexDecodeError::OddLength)
        );
    }

    #[test]
    fn empty_string_decodes_to_no_bytes() {
        assert_eq!(Hex::decode(&String::new()), Ok(vec![]));
    }

    #[test]
    fn accepts_uppercase_digits() {
        assert_eq!(Hex::decode(&"0AfF".to_string()), Ok(vec![0x0a, 0xff]));
    }

    #[test]
    fn non_ascii_input_is_an_error_not_a_panic() {
        assert_eq!(
            Hex::decode(&"é1".to_string()),
            Err(HexDecodeError::InvalidDigit('é'))
        );
    }

    #[test]
    fn rejects_invalid_digit() {
        assert_eq!(
            Hex::decode(&"zz".to_string()),
            Err(HexDecodeError::InvalidDigit('z'))
        );
    }
}
