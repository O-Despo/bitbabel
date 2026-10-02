//! A display address for an index, after Borges' story: hexagon, wall, shelf and volume.
//!
//! A hexagon has four walls of shelves, each wall has 5 shelves and each shelf holds 32
//! volumes: 4 x 5 x 32 = 640 volumes per hexagon. With `n` the index as a big-endian number:
//!
//! ```text
//! volume  = n mod 32          (shown 1..=32)
//! shelf   = (n / 32) mod 5    (shown 1..=5)
//! wall    = (n / 160) mod 4   (shown 1..=4)
//! hexagon = n / 640
//! ```
//!
//! So consecutive indexes walk along a shelf. Five shelves is not a power of two, so this
//! divides instead of slicing bits: every index has exactly one address.
//!
//! This is a display mapping. It is not stored anywhere and it is not part of the frozen
//! canonical contract.

use std::fmt;

use crate::encoding::{Encoding, Hex};
use crate::index::PageIndex;

const VOLUMES_PER_SHELF: u32 = 32;
const SHELVES_PER_WALL: u32 = 5;
const WALLS_PER_HEXAGON: u32 = 4;
const VOLUMES_PER_WALL: u32 = VOLUMES_PER_SHELF * SHELVES_PER_WALL;
const VOLUMES_PER_HEXAGON: u32 = VOLUMES_PER_WALL * WALLS_PER_HEXAGON;

/// Where a page sits: the hexagon, then the wall, shelf and volume inside it.
///
/// # Example
///
/// ```
/// use bitbabel_core::PageIndex;
///
/// let location = PageIndex::from_u64(641, 16).location();
/// assert_eq!(location.hexagon_hex(), "1");
/// assert_eq!((location.wall(), location.shelf(), location.volume()), (1, 1, 2));
/// assert_eq!(location.to_string(), "hexagon 1 · wall 1 · shelf 1 · volume 2");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    /// Big-endian, with no leading zero bytes. Empty is hexagon 0.
    hexagon: Vec<u8>,
    wall: u8,
    shelf: u8,
    volume: u8,
}

impl Location {
    /// The hexagon number as big-endian bytes with no leading zeros. Empty is hexagon 0.
    pub fn hexagon(&self) -> &[u8] {
        &self.hexagon
    }

    /// The hexagon number as lowercase hex with no leading zeros, `0` for the first.
    pub fn hexagon_hex(&self) -> String {
        let hex = Hex::encode(&self.hexagon);
        let digits = hex.trim_start_matches('0');
        if digits.is_empty() {
            "0".to_string()
        } else {
            digits.to_string()
        }
    }

    /// The wall, 1 to 4.
    pub fn wall(&self) -> u8 {
        self.wall
    }

    /// The shelf, 1 to 5.
    pub fn shelf(&self) -> u8 {
        self.shelf
    }

    /// The volume, 1 to 32.
    pub fn volume(&self) -> u8 {
        self.volume
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "hexagon {} · wall {} · shelf {} · volume {}",
            self.hexagon_hex(),
            self.wall,
            self.shelf,
            self.volume
        )
    }
}

impl PageIndex {
    /// The display address of this index.
    ///
    /// Every index has exactly one location.
    pub fn location(&self) -> Location {
        // Long division of the big-endian bytes by 640. The remainder stays under 640, so
        // `remainder << 8 | byte` stays well inside a `u32`.
        let mut remainder = 0u32;
        let mut hexagon = Vec::with_capacity(self.as_bytes().len());
        for &byte in self.as_bytes() {
            let value = (remainder << 8) | u32::from(byte);
            hexagon.push((value / VOLUMES_PER_HEXAGON) as u8);
            remainder = value % VOLUMES_PER_HEXAGON;
        }
        let start = hexagon
            .iter()
            .position(|&byte| byte != 0)
            .unwrap_or(hexagon.len());
        hexagon.drain(..start);

        Location {
            hexagon,
            wall: (remainder / VOLUMES_PER_WALL) as u8 + 1,
            shelf: (remainder % VOLUMES_PER_WALL / VOLUMES_PER_SHELF) as u8 + 1,
            volume: (remainder % VOLUMES_PER_SHELF) as u8 + 1,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// `(hexagon, wall, shelf, volume)` of the index `n` in a 16-byte library.
    fn at(n: u64) -> (String, u8, u8, u8) {
        let location = PageIndex::from_u64(n, 16).location();
        (
            location.hexagon_hex(),
            location.wall(),
            location.shelf(),
            location.volume(),
        )
    }

    fn at_hex(hexagon: &str, wall: u8, shelf: u8, volume: u8) -> (String, u8, u8, u8) {
        (hexagon.to_string(), wall, shelf, volume)
    }

    #[test]
    fn pinned_examples() {
        assert_eq!(at(0), at_hex("0", 1, 1, 1));
        assert_eq!(at(1), at_hex("0", 1, 1, 2));
        assert_eq!(at(31), at_hex("0", 1, 1, 32));
        assert_eq!(at(32), at_hex("0", 1, 2, 1));
        assert_eq!(at(159), at_hex("0", 1, 5, 32));
        assert_eq!(at(160), at_hex("0", 2, 1, 1));
        assert_eq!(at(639), at_hex("0", 4, 5, 32));
        assert_eq!(at(640), at_hex("1", 1, 1, 1));
        assert_eq!(at(641), at_hex("1", 1, 1, 2));
        // 640 * 0x100 = 163840
        assert_eq!(at(163_840), at_hex("100", 1, 1, 1));
    }

    #[test]
    fn display_is_the_address_line() {
        assert_eq!(
            PageIndex::from_u64(640, 16).location().to_string(),
            "hexagon 1 · wall 1 · shelf 1 · volume 1"
        );
    }

    #[test]
    fn the_hexagon_has_no_leading_zeros() {
        let location = PageIndex::from_u64(640 * 5, 3200).location();
        assert_eq!(location.hexagon(), [5]);
        assert_eq!(PageIndex::zero(3200).location().hexagon(), [] as [u8; 0]);
    }

    #[test]
    fn consecutive_indexes_walk_the_volumes() {
        // Every index of a few hexagons has a distinct address, in order.
        let mut seen = std::collections::HashSet::new();
        let mut previous = None;
        let mut index = PageIndex::zero(16);
        for _ in 0..(640 * 3) {
            let location = index.location();
            if let Some(previous) = previous {
                assert_ne!(previous, location);
            }
            assert!(seen.insert(location.clone()));
            previous = Some(location);
            index.increment();
        }
    }

    #[test]
    fn the_last_hexagon_is_partial_for_every_size() {
        // 2^(8*len) mod 640 is 256 for every size: the last hexagon holds 256 volumes, which
        // is all of wall 1 and shelves 1 to 3 of wall 2.
        for len in [16usize, 3200, 6400] {
            let last = PageIndex::from_bytes(vec![0xff; len]);
            let location = last.location();
            assert_eq!(
                (location.wall(), location.shelf(), location.volume()),
                (2, 3, 32),
                "len {len}"
            );

            // The next index wraps to the very first address.
            let mut next = last.clone();
            next.increment();
            assert_eq!(next.location().hexagon(), [] as [u8; 0]);
            assert_eq!(next.location().volume(), 1);
        }
    }

    #[test]
    fn empty_index_has_the_first_address() {
        let location = PageIndex::zero(0).location();
        assert_eq!(
            location.to_string(),
            "hexagon 0 · wall 1 · shelf 1 · volume 1"
        );
    }
}
