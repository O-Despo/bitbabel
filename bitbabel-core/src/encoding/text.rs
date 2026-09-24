use std::convert::Infallible;

use super::Encoding;

/// Lossy UTF-8 text view of arbitrary bytes.
pub struct Utf8;

impl Encoding for Utf8 {
    type Output = String;
    type DecodeError = Infallible;

    fn encode(bytes: &[u8]) -> Self::Output {
        String::from_utf8_lossy(bytes).into_owned()
    }

    fn decode(value: &Self::Output) -> Result<Vec<u8>, Self::DecodeError> {
        Ok(value.as_bytes().to_vec())
    }
}

/// Lossy little-endian UTF-16 text view of arbitrary bytes. A trailing odd byte is dropped.
pub struct Utf16Le;

impl Encoding for Utf16Le {
    type Output = String;
    type DecodeError = Infallible;

    fn encode(bytes: &[u8]) -> Self::Output {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    }

    fn decode(value: &Self::Output) -> Result<Vec<u8>, Self::DecodeError> {
        Ok(value.encode_utf16().flat_map(u16::to_le_bytes).collect())
    }
}

/// Lossy big-endian UTF-16 text view of arbitrary bytes. A trailing odd byte is dropped.
pub struct Utf16Be;

impl Encoding for Utf16Be {
    type Output = String;
    type DecodeError = Infallible;

    fn encode(bytes: &[u8]) -> Self::Output {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    }

    fn decode(value: &Self::Output) -> Result<Vec<u8>, Self::DecodeError> {
        Ok(value.encode_utf16().flat_map(u16::to_be_bytes).collect())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn utf8_encode_never_panics_on_arbitrary_bytes() {
        for len in 0..64 {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 + 5) as u8).collect();
            let _ = Utf8::encode(&data);
        }
    }

    #[test]
    fn utf8_decode_round_trips_real_text() {
        let text = "hello, babel 📚".to_string();
        let bytes = Utf8::decode(&text).unwrap();
        assert_eq!(Utf8::encode(&bytes), text);
    }

    #[test]
    fn utf16_le_encode_never_panics_on_arbitrary_bytes() {
        for len in 0..64 {
            let data: Vec<u8> = (0..len).map(|i| (i * 53 + 7) as u8).collect();
            let _ = Utf16Le::encode(&data);
        }
    }

    #[test]
    fn utf16_decode_round_trips_real_text() {
        let text = "hello, babel 📚".to_string();

        let le_bytes = Utf16Le::decode(&text).unwrap();
        assert_eq!(Utf16Le::encode(&le_bytes), text);

        let be_bytes = Utf16Be::decode(&text).unwrap();
        assert_eq!(Utf16Be::encode(&be_bytes), text);
    }

    #[test]
    fn le_and_be_bytes_are_distinct_for_non_ascii() {
        let text = "\u{1234}".to_string();
        assert_ne!(Utf16Le::decode(&text).unwrap(), Utf16Be::decode(&text).unwrap());
    }
}
