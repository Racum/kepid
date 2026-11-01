//! Minimal calendar handling, so the library needs no date dependency.
//!
//! Epochs are whole Unix microseconds (`docs/spec.md`, section 7,
//! "Epoch input"); nothing here goes through floating point.

const US_PER_DAY: i64 = 86_400_000_000;
/// One unit of a TLE epoch's 8-digit day fraction, 10⁻⁸ day, in microseconds.
const US_PER_TLE_DIGIT: i64 = 864;

/// Formats Unix microseconds as `YYYY-MM-DDTHH:MM:SS.mmmZ`, or with six
/// fractional digits when the epoch is not a whole millisecond.
pub(crate) fn iso8601(unix_us: i64) -> String {
    let days = unix_us.div_euclid(US_PER_DAY);
    let rem = unix_us.rem_euclid(US_PER_DAY);
    let (y, m, d) = civil_from_days(days);
    let (h, min, s, us) = (
        rem / 3_600_000_000,
        rem / 60_000_000 % 60,
        rem / 1_000_000 % 60,
        rem % 1_000_000,
    );
    if us % 1000 == 0 {
        format!("{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:{s:02}.{:03}Z", us / 1000)
    } else {
        format!("{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:{s:02}.{us:06}Z")
    }
}

/// Converts a TLE epoch field, `YYDDD.DDDDDDDD` (line 1, columns 19-32), to
/// Unix microseconds, exactly.
///
/// Years 57-99 are 1957-1999 and 00-56 are 2000-2056. Day 1 is 1 January.
/// The fraction may have up to 8 digits; each 10⁻⁸ day is exactly 864 µs.
/// Returns `None` for anything else.
pub fn tle_epoch_unix_us(field: &str) -> Option<i64> {
    let field = field.trim();
    let (whole, fraction) = field.split_once('.').unwrap_or((field, ""));
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if whole.len() != 5 || !digits(whole) || fraction.len() > 8 || !digits(fraction) {
        return None;
    }
    let yy: i64 = whole[..2].parse().ok()?;
    let day: i64 = whole[2..].parse().ok()?;
    let year = if yy < 57 { 2000 + yy } else { 1900 + yy };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    if !(1..=365 + i64::from(leap)).contains(&day) {
        return None;
    }
    let units: i64 = format!("{fraction:0<8}").parse().ok()?;
    Some((days_from_civil(year, 1, 1) + day - 1) * US_PER_DAY + units * US_PER_TLE_DIGIT)
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// (year, month, day) to days since 1970-01-01, proleptic Gregorian.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::{days_from_civil, iso8601, tle_epoch_unix_us};

    const DAY: i64 = 86_400_000_000;

    #[test]
    fn unix_epoch() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn leap_days() {
        assert_eq!(iso8601(19_782 * DAY), "2024-02-29T00:00:00.000Z");
        assert_eq!(iso8601(11_016 * DAY), "2000-02-29T00:00:00.000Z");
        assert_eq!(iso8601(47_541 * DAY), "2100-03-01T00:00:00.000Z"); // 2100 is not a leap year
    }

    #[test]
    fn iss_example() {
        assert_eq!(iso8601(1_793_074_471_424_000), "2026-10-27T04:14:31.424Z");
        assert_eq!(iso8601(1_793_074_471_558_560), "2026-10-27T04:14:31.558560Z");
    }

    #[test]
    fn last_representable_tick_matches_the_spec() {
        assert_eq!(iso8601(((1i64 << 40) - 1) * 256_000), "10889-08-02T05:31:50.400Z");
    }

    #[test]
    fn day_and_year_edges() {
        assert_eq!(iso8601(DAY - 1), "1970-01-01T23:59:59.999999Z");
        assert_eq!(iso8601(365 * DAY), "1971-01-01T00:00:00.000Z");
    }

    #[test]
    fn civil_round_trip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2024, 2, 29), 19_782);
        assert_eq!(days_from_civil(2100, 3, 1), 47_541);
    }

    #[test]
    fn tle_epochs_are_exact() {
        assert_eq!(tle_epoch_unix_us("70001.00000000"), Some(0));
        assert_eq!(tle_epoch_unix_us("26300.17675415"), Some(1_793_074_471_558_560));
        assert_eq!(tle_epoch_unix_us("26300.1767541"), Some(1_793_074_471_554_240));
        assert_eq!(tle_epoch_unix_us(" 26300 "), Some(1_793_059_200_000_000));
        // NORAD 53167 in the catalog: exactly half a tick, with no float in the way.
        let us = tle_epoch_unix_us("26300.03268000").unwrap();
        assert_eq!(us, 1_793_062_023_552_000);
        assert_eq!(us % 256_000, 128_000);
        assert_eq!(tle_epoch_unix_us("57001.0"), Some(days_from_civil(1957, 1, 1) * DAY));
        assert_eq!(tle_epoch_unix_us("56366.0"), Some((days_from_civil(2056, 1, 1) + 365) * DAY));
    }

    #[test]
    fn tle_epochs_reject_malformed_fields() {
        for bad in ["", "2630.5", "263000.5", "26300.123456789", "26000.5", "25366.5", "26x00.5", "26300.-1", "26300.1e3"] {
            assert_eq!(tle_epoch_unix_us(bad), None, "{bad}");
        }
    }
}
