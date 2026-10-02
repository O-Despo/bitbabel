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
//! divides instead of slicing bits: every index has exactly one address, and every address
//! within the library has exactly one index.
//!
//! This is a display mapping. It is not stored anywhere and it is not part of the frozen
//! canonical contract.

use std::fmt;

use crate::encoding::{Encoding, Hex};
use crate::error::LocationError;
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
/// assert_eq!(location.index(16).unwrap(), PageIndex::from_u64(641, 16));
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
    /// A location from its parts. `hexagon` is a big-endian number of any length.
    ///
    /// # Errors
    ///
    /// [`LocationError`] if `wall`, `shelf` or `volume` is out of range.
    pub fn new(hexagon: &[u8], wall: u8, shelf: u8, volume: u8) -> Result<Self, LocationError> {
        if !(1..=WALLS_PER_HEXAGON as u8).contains(&wall) {
            return Err(LocationError::Wall(wall));
        }
        if !(1..=SHELVES_PER_WALL as u8).contains(&shelf) {
            return Err(LocationError::Shelf(shelf));
        }
        if !(1..=VOLUMES_PER_SHELF as u8).contains(&volume) {
            return Err(LocationError::Volume(volume));
        }
        let start = hexagon
            .iter()
            .position(|&byte| byte != 0)
            .unwrap_or(hexagon.len());
        Ok(Location {
            hexagon: hexagon[start..].to_vec(),
            wall,
            shelf,
            volume,
        })
    }

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

    /// The index at this location in a library whose indexes are `len` bytes.
    ///
    /// # Errors
    ///
    /// [`LocationError::OutsideLibrary`] if the address is past the library's last page. The
    /// last hexagon is only partly filled: for every size, `2^(8·len) mod 640` is 256.
    pub fn index(&self, len: usize) -> Result<PageIndex, LocationError> {
        let outside = LocationError::OutsideLibrary { len };
        if self.hexagon.len() > len {
            return Err(outside);
        }
        let within = (u32::from(self.wall) - 1) * VOLUMES_PER_WALL
            + (u32::from(self.shelf) - 1) * VOLUMES_PER_SHELF
            + (u32::from(self.volume) - 1);

        // hexagon * 640 + within, from the least significant byte up.
        let mut bytes = vec![0u8; len];
        let mut carry = within;
        for (place, out) in bytes.iter_mut().rev().enumerate() {
            let hexagon_byte = self
                .hexagon
                .iter()
                .rev()
                .nth(place)
                .copied()
                .map_or(0, u32::from);
            let value = hexagon_byte * VOLUMES_PER_HEXAGON + carry;
            *out = (value & 0xff) as u8;
            carry = value >> 8;
        }
        if carry != 0 {
            return Err(outside);
        }
        Ok(PageIndex::from_bytes(bytes))
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
    /// Every index has exactly one location, and [`Location::index`] gives it back.
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
    fn small_indexes_round_trip_in_every_size() {
        for len in [16, 3200, 6400] {
            for n in (0..2000u64).chain([u64::MAX - 1, u64::MAX]) {
                let index = PageIndex::from_u64(n, len);
                assert_eq!(index.location().index(len).unwrap(), index, "{n} at {len}");
            }
        }
    }

    #[test]
    fn varied_indexes_round_trip_in_every_size() {
        for len in [16usize, 3200, 6400] {
            for shift in 0..40usize {
                let bytes: Vec<u8> = (0..len)
                    .map(|i| ((i * 131 + shift * 17) % 256) as u8)
                    .collect();
                let index = PageIndex::from_bytes(bytes);
                assert_eq!(index.location().index(len).unwrap(), index);
            }
        }
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

            // The next volume in that hexagon is past the library.
            let past = Location::new(location.hexagon(), 2, 4, 1).unwrap();
            assert_eq!(past.index(len), Err(LocationError::OutsideLibrary { len }));
            assert_eq!(location.index(len).unwrap(), last);
        }
    }

    #[test]
    fn a_hexagon_longer_than_the_index_is_outside_the_library() {
        let location = Location::new(&[1, 0, 0], 1, 1, 1).unwrap();
        assert_eq!(
            location.index(2),
            Err(LocationError::OutsideLibrary { len: 2 })
        );
        // 65536 * 640 does not fit in 3 bytes either; it does in 4.
        assert_eq!(
            location.index(3),
            Err(LocationError::OutsideLibrary { len: 3 })
        );
        assert!(location.index(4).is_ok());
    }

    #[test]
    fn new_rejects_out_of_range_parts() {
        assert_eq!(Location::new(&[], 0, 1, 1), Err(LocationError::Wall(0)));
        assert_eq!(Location::new(&[], 5, 1, 1), Err(LocationError::Wall(5)));
        assert_eq!(Location::new(&[], 1, 0, 1), Err(LocationError::Shelf(0)));
        assert_eq!(Location::new(&[], 1, 6, 1), Err(LocationError::Shelf(6)));
        assert_eq!(Location::new(&[], 1, 1, 0), Err(LocationError::Volume(0)));
        assert_eq!(Location::new(&[], 1, 1, 33), Err(LocationError::Volume(33)));
        assert!(Location::new(&[], 4, 5, 32).is_ok());
    }

    #[test]
    fn new_ignores_leading_zero_bytes() {
        assert_eq!(
            Location::new(&[0, 0, 7], 1, 1, 1).unwrap(),
            Location::new(&[7], 1, 1, 1).unwrap()
        );
    }

    #[test]
    fn empty_index_has_the_first_address() {
        let location = PageIndex::zero(0).location();
        assert_eq!(
            location.to_string(),
            "hexagon 0 · wall 1 · shelf 1 · volume 1"
        );
        assert_eq!(location.index(0).unwrap(), PageIndex::zero(0));
    }
}
