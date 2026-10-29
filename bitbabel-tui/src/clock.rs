//! The time, handed to the app so a test does not depend on the real clock.

use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Datelike};

/// Where the app reads the time: unix seconds.
pub type NowFn = fn() -> u64;

/// The system clock. A clock set before 1970 reads as 0, which the bookmark file can hold.
pub fn system_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// `time` as the bookmark list shows it: UTC, exactly 17 cells, like `2026-10-01 14:20Z`.
///
/// A time the column cannot hold reads `????-??-?? ??:??Z`: seconds too big for `i64`
/// (`u64::MAX`), outside what `chrono` maps, or past the year 9999. UTC only: the list is
/// shared between machines, so local time would read differently for everyone (§13.3a).
pub fn format_utc(time: u64) -> String {
    const UNKNOWN: &str = "????-??-?? ??:??Z";
    // `i64::try_from`, not `as`: `u64::MAX as i64` is negative, and a negative second is an
    // 18th-century date, not the placeholder.
    i64::try_from(time)
        .ok()
        .and_then(|secs| DateTime::from_timestamp(secs, 0))
        .filter(|when| when.year() <= 9999)
        .map_or_else(
            || UNKNOWN.to_string(),
            |when| format!("{}Z", when.format("%Y-%m-%d %H:%M")),
        )
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn known_times_get_their_utc_date_and_time() {
        assert_eq!(format_utc(0), "1970-01-01 00:00Z");
        // 2000-02-29: the leap day that only the century rule makes a leap day.
        assert_eq!(format_utc(951_782_400), "2000-02-29 00:00Z");
        // The time of the pinned golden file in `bitbabel-file`.
        assert_eq!(format_utc(1_790_812_800), "2026-10-01 00:00Z");
        assert_eq!(format_utc(1_790_812_955), "2026-10-01 00:02Z");
    }

    #[test]
    fn the_year_9999_is_the_last_the_column_holds() {
        assert_eq!(format_utc(253_402_300_799), "9999-12-31 23:59Z");
        assert_eq!(format_utc(253_402_300_800), "????-??-?? ??:??Z");
    }

    #[test]
    fn seconds_too_big_for_i64_are_the_placeholder_not_a_wrapped_date() {
        assert_eq!(format_utc(u64::MAX), "????-??-?? ??:??Z");
    }

    #[test]
    fn the_system_clock_is_after_2024() {
        assert!(system_now() > 1_700_000_000);
    }
}
