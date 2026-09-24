use super::Encoding;

/// Lowercase hex-string encoding.
pub struct Hex;

#[derive(Debug, PartialEq, Eq)]
pub enum HexDecodeError {
    OddLength,
    InvalidDigit(char),
}

impl Encoding for Hex {
    type Output = String;
    type DecodeError = HexDecodeError;

    fn encode(bytes: &[u8]) -> Self::Output {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn decode(value: &Self::Output) -> Result<Vec<u8>, Self::DecodeError> {
        let chars: Vec<char> = value.chars().collect();
        if chars.len() % 2 != 0 {
            return Err(HexDecodeError::OddLength);
        }

        chars
            .chunks(2)
            .map(|pair| {
                let hi = pair[0].to_digit(16).ok_or(HexDecodeError::InvalidDigit(pair[0]))?;
                let lo = pair[1].to_digit(16).ok_or(HexDecodeError::InvalidDigit(pair[1]))?;
                Ok((hi as u8) << 4 | (lo as u8))
            })
            .collect()
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
        assert_eq!(Hex::decode(&"abc".to_string()), Err(HexDecodeError::OddLength));
    }

    #[test]
    fn rejects_invalid_digit() {
        assert_eq!(
            Hex::decode(&"zz".to_string()),
            Err(HexDecodeError::InvalidDigit('z'))
        );
    }
}
