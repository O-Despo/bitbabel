//! The time, handed to the app so a test does not depend on the real clock.

use std::time::{SystemTime, UNIX_EPOCH};

/// Where the app reads the time: unix seconds.
pub type NowFn = fn() -> u64;

/// The system clock. A clock set before 1970 reads as 0, which the bookmark file can hold.
pub fn system_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_system_clock_is_after_2024() {
        assert!(system_now() > 1_700_000_000);
    }
}
