use std::convert::Infallible;

use super::Encoding;

/// Identity encoding: the representation is just the bytes themselves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Raw;

impl Encoding for Raw {
    type Output = Vec<u8>;
    type DecodeError = Infallible;

    fn encode(bytes: &[u8]) -> Self::Output {
        bytes.to_vec()
    }

    fn decode(value: &Self::Output) -> Result<Vec<u8>, Self::DecodeError> {
        Ok(value.clone())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn round_trips() {
        let data = vec![1, 2, 3, 4, 5];
        let encoded = Raw::encode(&data);
        assert_eq!(encoded, data);
        let decoded = Raw::decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
}
