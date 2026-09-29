mod base64;
mod hex;
mod raw;
mod text;

pub use base64::{Base64, Base64DecodeError};
pub use hex::{Hex, HexDecodeError};
pub use raw::Raw;
pub use text::{Utf8, Utf16Be, Utf16Le};

/// Converts babel bytes to and from another representation.
///
/// `encode` is total: it always produces a value, using lossy conversion where the
/// representation can't hold arbitrary bytes exactly (e.g. text). `decode` is fallible
/// only where the representation can be malformed independently of anything `encode`
/// would have produced (e.g. a hand-typed hex or base64 string).
pub trait Encoding {
    type Output;
    type DecodeError;

    fn encode(bytes: &[u8]) -> Self::Output;
    fn decode(value: &Self::Output) -> Result<Vec<u8>, Self::DecodeError>;
}
