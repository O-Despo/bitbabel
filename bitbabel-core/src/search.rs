//! Finding where a value lives.
//!
//! The library is a bijection, so nothing is scanned: a search builds pages that contain the
//! needle and asks [`BabelLibrary::index_of`] where each one is.

use std::collections::HashSet;

use crate::error::SearchError;
use crate::library::BabelLibrary;
use crate::page::Page;

/// Domain-separation context for the filler stream. Changing it changes every seeded search
/// result, so bump the version instead of editing it.
const SEARCH_CONTEXT: &str = "v1 bitbabel search filler";

/// Where the needle goes in each page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Placement {
    /// At byte 0.
    Start,
    /// At an offset chosen from the seed, anywhere the needle fits.
    Random,
}

/// What fills the rest of each page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fill {
    /// One repeated byte.
    Pad(u8),
    /// Bytes chosen from the seed.
    Random,
}

/// How [`BabelLibrary::search`] builds its pages.
///
/// "Random" here is deterministic: every choice comes from `seed`, so the same seed, needle
/// and options always give the same results. Pass fresh random bytes as the seed for
/// different results each time.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchOptions {
    /// The most results to return. Fewer come back when there are fewer distinct pages to
    /// build, e.g. only one for [`raw`](Self::raw).
    pub limit: usize,
    pub placement: Placement,
    pub fill: Fill,
    pub seed: [u8; 32],
}

impl SearchOptions {
    /// The needle at the start, then zero bytes: exactly one page.
    pub fn raw() -> Self {
        SearchOptions {
            limit: 1,
            placement: Placement::Start,
            fill: Fill::Pad(0),
            seed: [0; 32],
        }
    }

    /// The needle at a random offset among random bytes, up to 10 pages.
    pub fn surrounded(seed: [u8; 32]) -> Self {
        SearchOptions {
            limit: 10,
            placement: Placement::Random,
            fill: Fill::Random,
            seed,
        }
    }

    /// Sets [`limit`](Self::limit).
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// One page that contains the needle, and where in it the needle is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchResult {
    page: Page,
    start: usize,
    end: usize,
}

impl SearchResult {
    /// The page, with its bytes and its index.
    pub fn page(&self) -> &Page {
        &self.page
    }

    /// Byte offset where the needle starts.
    pub fn start(&self) -> usize {
        self.start
    }

    /// Byte offset just past the needle, so `page().bytes()[start()..end()]` is the needle.
    pub fn end(&self) -> usize {
        self.end
    }
}

impl BabelLibrary {
    /// Pages that contain `needle`, placed and filled as `options` says, each with its index.
    ///
    /// Results are distinct. There can be fewer than `options.limit`: [`Placement::Start`]
    /// with [`Fill::Pad`] can only build one page, and [`Placement::Random`] with
    /// [`Fill::Pad`] one per offset.
    ///
    /// # Errors
    ///
    /// Returns [`SearchError::EmptyNeedle`] or [`SearchError::NeedleTooLong`] if the needle
    /// does not fit a page.
    ///
    /// # Example
    ///
    /// ```
    /// use bitbabel_core::{BabelLibrary, LibraryConfig, SearchOptions};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let library = BabelLibrary::canonical(LibraryConfig::SMALL)?;
    ///
    /// let found = library.search(b"hello", &SearchOptions::raw())?;
    /// assert_eq!(found[0].page().bytes(), b"hello\0\0\0\0\0\0\0\0\0\0\0");
    ///
    /// // Going to that index gives the same page back.
    /// assert_eq!(library.page_at(found[0].page().index())?, *found[0].page());
    /// # Ok(())
    /// # }
    /// ```
    pub fn search(
        &self,
        needle: &[u8],
        options: &SearchOptions,
    ) -> Result<Vec<SearchResult>, SearchError> {
        let page_len = self.page_len();
        if needle.is_empty() {
            return Err(SearchError::EmptyNeedle);
        }
        if needle.len() > page_len {
            return Err(SearchError::NeedleTooLong {
                needle_len: needle.len(),
                page_len,
            });
        }

        // Start + Pad has nothing to vary, so a second attempt would only repeat the first.
        let fixed = options.placement == Placement::Start && matches!(options.fill, Fill::Pad(_));
        let max_attempts = if fixed {
            1
        } else {
            // Headroom for duplicates, capped so a small space ends instead of looping.
            options.limit.saturating_mul(4).saturating_add(16)
        };

        let positions = (page_len - needle.len() + 1) as u64;
        let stream_key = blake3::derive_key(SEARCH_CONTEXT, &options.seed);
        let mut seen = HashSet::new();
        let mut results = Vec::new();

        for attempt in 0..max_attempts {
            if results.len() >= options.limit {
                break;
            }

            // One stream per attempt, over the needle too, so different needles never share
            // offsets or filler.
            let mut hasher = blake3::Hasher::new_keyed(&stream_key);
            hasher.update(&(needle.len() as u64).to_le_bytes());
            hasher.update(needle);
            hasher.update(&(attempt as u64).to_le_bytes());
            let mut stream = hasher.finalize_xof();

            let start = match options.placement {
                Placement::Start => 0,
                Placement::Random => {
                    let mut word = [0u8; 8];
                    stream.fill(&mut word);
                    // The modulo bias is at most positions / 2^64: negligible.
                    (u64::from_le_bytes(word) % positions) as usize
                }
            };
            let mut bytes = match options.fill {
                Fill::Pad(byte) => vec![byte; page_len],
                Fill::Random => {
                    let mut filler = vec![0u8; page_len];
                    stream.fill(&mut filler);
                    filler
                }
            };
            let end = start + needle.len();
            bytes[start..end].copy_from_slice(needle);

            let index = self.index_of(&bytes)?;
            if seen.insert(index.clone()) {
                results.push(SearchResult {
                    page: Page::new(bytes, index),
                    start,
                    end,
                });
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::LibraryConfig;

    /// 16-byte pages.
    fn library() -> BabelLibrary {
        BabelLibrary::canonical(LibraryConfig::SMALL).unwrap()
    }

    /// Every result holds the needle where it says, and lives at its index.
    fn assert_valid(library: &BabelLibrary, needle: &[u8], results: &[SearchResult]) {
        for result in results {
            let bytes = result.page().bytes();
            assert_eq!(&bytes[result.start()..result.end()], needle);
            assert_eq!(
                library.page_at(result.page().index()).unwrap(),
                *result.page()
            );
        }
        let distinct: HashSet<_> = results.iter().map(|r| r.page().index()).collect();
        assert_eq!(distinct.len(), results.len());
    }

    #[test]
    fn raw_gives_one_page_with_the_needle_first() {
        let lib = library();
        for limit in [1, 5] {
            let results = lib
                .search(b"hello", &SearchOptions::raw().limit(limit))
                .unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!((results[0].start(), results[0].end()), (0, 5));
            assert_eq!(results[0].page().bytes(), b"hello\0\0\0\0\0\0\0\0\0\0\0");
            assert_valid(&lib, b"hello", &results);
        }
    }

    #[test]
    fn pad_byte_is_used() {
        let options = SearchOptions {
            fill: Fill::Pad(b' '),
            ..SearchOptions::raw()
        };
        let results = library().search(b"hi", &options).unwrap();
        assert_eq!(results[0].page().bytes(), b"hi              ");
    }

    #[test]
    fn surrounded_gives_limit_distinct_pages() {
        let lib = library();
        let results = lib
            .search(b"hi", &SearchOptions::surrounded([1; 32]).limit(5))
            .unwrap();
        assert_eq!(results.len(), 5);
        assert_valid(&lib, b"hi", &results);
        assert!(results.iter().all(|r| r.end() <= 16));
    }

    #[test]
    fn start_with_random_fill_keeps_the_needle_first() {
        let lib = library();
        let options = SearchOptions {
            placement: Placement::Start,
            ..SearchOptions::surrounded([2; 32]).limit(5)
        };
        let results = lib.search(b"hi", &options).unwrap();
        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.start() == 0));
        assert_valid(&lib, b"hi", &results);
    }

    #[test]
    fn random_placement_with_padding_is_capped_by_offsets() {
        let lib = library();
        let needle = [7u8; 15]; // fits at offset 0 or 1 only
        let options = SearchOptions {
            fill: Fill::Pad(0),
            ..SearchOptions::surrounded([3; 32]).limit(10)
        };
        let results = lib.search(&needle, &options).unwrap();
        assert!(!results.is_empty() && results.len() <= 2);
        assert_valid(&lib, &needle, &results);
    }

    #[test]
    fn same_seed_is_deterministic_and_seeds_differ() {
        let lib = library();
        let a = lib
            .search(b"hi", &SearchOptions::surrounded([4; 32]))
            .unwrap();
        let b = lib
            .search(b"hi", &SearchOptions::surrounded([4; 32]))
            .unwrap();
        let c = lib
            .search(b"hi", &SearchOptions::surrounded([5; 32]))
            .unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn needle_filling_the_page_gives_that_page() {
        let lib = library();
        let needle = [9u8; 16];
        let results = lib
            .search(&needle, &SearchOptions::surrounded([6; 32]))
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!((results[0].start(), results[0].end()), (0, 16));
        assert_valid(&lib, &needle, &results);
    }

    #[test]
    fn needle_must_fit() {
        let lib = library();
        assert_eq!(
            lib.search(b"", &SearchOptions::raw()).unwrap_err(),
            SearchError::EmptyNeedle
        );
        assert_eq!(
            lib.search(&[0u8; 17], &SearchOptions::raw()).unwrap_err(),
            SearchError::NeedleTooLong {
                needle_len: 17,
                page_len: 16
            }
        );
    }

    #[test]
    fn zero_limit_is_empty() {
        let results = library()
            .search(b"hi", &SearchOptions::surrounded([0; 32]).limit(0))
            .unwrap();
        assert!(results.is_empty());
    }
}
