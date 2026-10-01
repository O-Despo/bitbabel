//! How a list of page indices is written down.
//!
//! Raw is the indices back to back. Hex and Base64 put one index per line, and every line,
//! including the last, ends in `\n`. Decoding is strict: each list has exactly one valid
//! encoding, so uppercase hex, blank lines and a missing final newline are all rejected.

use bitbabel_core::{Base64, Encoding, Hex, PageIndex};

use crate::error::IndexFormatError;

/// The format of a [`PageIndex`] list. [`Raw`](Self::Raw) is the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum IndexFormat {
    /// The indices back to back, as binary. The reader splits every `page_len` bytes.
    #[default]
    Raw,
    /// One lowercase hex index per line.
    Hex,
    /// One standard padded base64 index per line.
    Base64,
}

impl IndexFormat {
    /// The format's name, as written in a file header.
    ///
    /// # Example
    ///
    /// ```
    /// use bitbabel_file::IndexFormat;
    ///
    /// assert_eq!(IndexFormat::Hex.name(), "hex");
    /// ```
    pub fn name(self) -> &'static str {
        match self {
            IndexFormat::Raw => "raw",
            IndexFormat::Hex => "hex",
            IndexFormat::Base64 => "base64",
        }
    }

    /// The format with this [`name`](Self::name). Only the exact lowercase name parses.
    ///
    /// # Example
    ///
    /// ```
    /// use bitbabel_file::IndexFormat;
    ///
    /// assert_eq!(IndexFormat::from_name("hex"), Some(IndexFormat::Hex));
    /// assert_eq!(IndexFormat::from_name("HEX"), None);
    /// ```
    pub fn from_name(name: &str) -> Option<Self> {
        [IndexFormat::Raw, IndexFormat::Hex, IndexFormat::Base64]
            .into_iter()
            .find(|format| format.name() == name)
    }

    /// Writes `indices`, each of which must be `page_len` bytes.
    ///
    /// # Errors
    ///
    /// Returns [`IndexFormatError::ZeroPageLen`] if `page_len` is zero,
    /// [`IndexFormatError::Empty`] if there are no indices, or [`IndexFormatError::IndexLen`]
    /// for the first index that is not `page_len` bytes.
    ///
    /// # Example
    ///
    /// ```
    /// use bitbabel_core::PageIndex;
    /// use bitbabel_file::IndexFormat;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let indices = [PageIndex::from_u64(1, 2), PageIndex::from_u64(0xabcd, 2)];
    ///
    /// assert_eq!(IndexFormat::Raw.encode(&indices, 2)?, [0x00, 0x01, 0xab, 0xcd]);
    /// assert_eq!(IndexFormat::Hex.encode(&indices, 2)?, b"0001\nabcd\n");
    /// assert_eq!(IndexFormat::Base64.encode(&indices, 2)?, b"AAE=\nq80=\n");
    /// # Ok(())
    /// # }
    /// ```
    pub fn encode(
        self,
        indices: &[PageIndex],
        page_len: usize,
    ) -> Result<Vec<u8>, IndexFormatError> {
        check_list(indices.len(), page_len)?;
        if let Some((position, index)) = indices
            .iter()
            .enumerate()
            .find(|(_, index)| index.as_bytes().len() != page_len)
        {
            return Err(IndexFormatError::IndexLen {
                position,
                expected: page_len,
                actual: index.as_bytes().len(),
            });
        }

        let mut out = Vec::new();
        for index in indices {
            match self {
                IndexFormat::Raw => out.extend_from_slice(index.as_bytes()),
                IndexFormat::Hex | IndexFormat::Base64 => {
                    out.extend_from_slice(self.line_for(index).as_bytes());
                    out.push(b'\n');
                }
            }
        }
        Ok(out)
    }

    /// Reads a list written by [`encode`](Self::encode). Inverse of it.
    ///
    /// # Errors
    ///
    /// Returns [`IndexFormatError::ZeroPageLen`] if `page_len` is zero,
    /// [`IndexFormatError::Empty`] if `payload` is empty, and otherwise:
    ///
    /// - Raw: [`IndexFormatError::Misaligned`] unless `payload` is a multiple of `page_len`.
    /// - Hex and Base64: [`IndexFormatError::MissingFinalNewline`],
    ///   [`IndexFormatError::Line`] for a line that does not decode to `page_len` bytes, or
    ///   [`IndexFormatError::NotCanonical`] for one that decodes but is not how
    ///   [`encode`](Self::encode) writes it (e.g. uppercase hex).
    pub fn decode(
        self,
        payload: &[u8],
        page_len: usize,
    ) -> Result<Vec<PageIndex>, IndexFormatError> {
        check_list(payload.len(), page_len)?;
        match self {
            IndexFormat::Raw => {
                if !payload.len().is_multiple_of(page_len) {
                    return Err(IndexFormatError::Misaligned {
                        len: payload.len(),
                        page_len,
                    });
                }
                Ok(payload
                    .chunks(page_len)
                    .map(|chunk| PageIndex::from_bytes(chunk.to_vec()))
                    .collect())
            }
            IndexFormat::Hex | IndexFormat::Base64 => {
                let body = payload
                    .strip_suffix(b"\n")
                    .ok_or(IndexFormatError::MissingFinalNewline)?;
                body.split(|&b| b == b'\n')
                    .enumerate()
                    .map(|(position, line)| self.parse_line(position, line, page_len))
                    .collect()
            }
        }
    }

    /// One index as a line of text, without the newline. Only for Hex and Base64.
    fn line_for(self, index: &PageIndex) -> String {
        match self {
            IndexFormat::Hex => Hex::encode(index.as_bytes()),
            IndexFormat::Base64 | IndexFormat::Raw => Base64::encode(index.as_bytes()),
        }
    }

    /// Parses one line and checks it is written exactly as [`line_for`](Self::line_for) would.
    fn parse_line(
        self,
        position: usize,
        line: &[u8],
        page_len: usize,
    ) -> Result<PageIndex, IndexFormatError> {
        // Any non-UTF-8 byte becomes U+FFFD, which neither decoder accepts.
        let text = String::from_utf8_lossy(line);
        let index = match self {
            IndexFormat::Hex => PageIndex::from_hex(&text, page_len),
            IndexFormat::Base64 | IndexFormat::Raw => PageIndex::from_base64(&text, page_len),
        }
        .map_err(|error| IndexFormatError::Line { position, error })?;

        if self.line_for(&index).as_bytes() != line {
            return Err(IndexFormatError::NotCanonical { position });
        }
        Ok(index)
    }
}

/// Checks shared by both directions. `len` is the list or payload length.
fn check_list(len: usize, page_len: usize) -> Result<(), IndexFormatError> {
    if page_len == 0 {
        Err(IndexFormatError::ZeroPageLen)
    } else if len == 0 {
        Err(IndexFormatError::Empty)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use bitbabel_core::PageIndexError;

    use super::*;

    const FORMATS: [IndexFormat; 3] = [IndexFormat::Raw, IndexFormat::Hex, IndexFormat::Base64];

    fn indices() -> Vec<PageIndex> {
        [0u64, 1, 0xdead_beef]
            .iter()
            .map(|&n| PageIndex::from_u64(n, 4))
            .collect()
    }

    #[test]
    fn every_format_round_trips() {
        for format in FORMATS {
            let payload = format.encode(&indices(), 4).unwrap();
            assert_eq!(format.decode(&payload, 4).unwrap(), indices());
        }
    }

    #[test]
    fn writes_the_agreed_layout() {
        assert_eq!(
            IndexFormat::Hex.encode(&indices(), 4).unwrap(),
            b"00000000\n00000001\ndeadbeef\n"
        );
        assert_eq!(
            IndexFormat::Base64.encode(&indices(), 4).unwrap(),
            b"AAAAAA==\nAAAAAQ==\n3q2+7w==\n"
        );
        assert_eq!(IndexFormat::Raw.encode(&indices(), 4).unwrap().len(), 12);
        assert_eq!(IndexFormat::default(), IndexFormat::Raw);
    }

    #[test]
    fn rejects_zero_page_len_and_empty_lists() {
        for format in FORMATS {
            assert_eq!(
                format.encode(&indices(), 0),
                Err(IndexFormatError::ZeroPageLen)
            );
            assert_eq!(format.decode(b"\n", 0), Err(IndexFormatError::ZeroPageLen));
            assert_eq!(format.encode(&[], 4), Err(IndexFormatError::Empty));
            assert_eq!(format.decode(b"", 4), Err(IndexFormatError::Empty));
        }
    }

    #[test]
    fn encode_rejects_an_index_of_the_wrong_length() {
        let mut list = indices();
        list.push(PageIndex::zero(2));
        assert_eq!(
            IndexFormat::Hex.encode(&list, 4),
            Err(IndexFormatError::IndexLen {
                position: 3,
                expected: 4,
                actual: 2
            })
        );
    }

    #[test]
    fn raw_rejects_a_partial_index() {
        assert_eq!(
            IndexFormat::Raw.decode(&[0; 6], 4),
            Err(IndexFormatError::Misaligned {
                len: 6,
                page_len: 4
            })
        );
    }

    #[test]
    fn text_requires_a_final_newline() {
        assert_eq!(
            IndexFormat::Hex.decode(b"00000000\n00000001", 4),
            Err(IndexFormatError::MissingFinalNewline)
        );
    }

    #[test]
    fn text_rejects_bad_lines_by_position() {
        let short = IndexFormat::Hex.decode(b"00000000\n0001\n", 4);
        assert_eq!(
            short,
            Err(IndexFormatError::Line {
                position: 1,
                error: PageIndexError::LenMismatch {
                    expected: 4,
                    actual: 2
                }
            })
        );

        for bad in [
            &b"00000000\n\n"[..],                  // blank line
            b"00000000\r\n",                       // CRLF
            b"0000zz00\n",                         // bad digit
            b"\xff\xff\xff\xff\xff\xff\xff\xff\n", // not UTF-8
        ] {
            assert!(matches!(
                IndexFormat::Hex.decode(bad, 4),
                Err(IndexFormatError::Line { .. })
            ));
        }
    }

    #[test]
    fn text_rejects_non_canonical_lines() {
        assert_eq!(
            IndexFormat::Hex.decode(b"00000000\nDEADBEEF\n", 4),
            Err(IndexFormatError::NotCanonical { position: 1 })
        );
    }

    #[test]
    fn error_messages_are_readable() {
        assert_eq!(
            IndexFormatError::NotCanonical { position: 2 }.to_string(),
            "index 2 is not written in canonical form"
        );
        let err = IndexFormat::Hex.decode(b"0000zz00\n", 4).unwrap_err();
        assert_eq!(
            err.to_string(),
            "index 0: invalid index: invalid hex digit 'z'"
        );
    }
}
