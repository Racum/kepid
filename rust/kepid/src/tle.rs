//! Two-line element sets, read and written as text, with no SGP4 dependency.
//!
//! A TLE is fixed-column text; reading one is slicing and exact decimal
//! parsing, and SGP4 only ever consumes the numbers. Every number is parsed
//! from its decimal text, so a value always gets the nearest `f64`.

use std::fmt;

use crate::codec::{decode, encode};
use crate::error::{DecodeError, EncodeError};
use crate::time::{tle_epoch_field, tle_epoch_unix_us};
use crate::{Elements, KepId};

/// Both lines are exactly 69 characters (trailing whitespace is ignored).
const LINE_LEN: usize = 69;
/// The largest catalog number Alpha-5 can write, `Z9999`.
const MAX_CATALOG: u32 = 339_999;
/// Alpha-5 letters, for 10 to 33; I and O are skipped.
const ALPHA5: &[u8; 24] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

/// What a written TLE carries in the identity fields a KepID does not have:
/// classification `U`, a blank designator, element set 999 and revolution
/// number 99999. The catalog number is 00000 unless the caller gives one.
const CLASSIFICATION: char = 'U';
const ELEMENT_SET: u16 = 999;
const REV_NUMBER: u32 = 99_999;

/// Why a TLE cannot be read or written.
#[derive(Debug, Clone, PartialEq)]
pub enum TleError {
    /// Not 69 characters; carries the line (1 or 2) and the length found.
    Length { line: u8, len: usize },
    /// The line does not start with its number and a space.
    LineNumber { line: u8 },
    /// The last digit does not match the line's checksum.
    Checksum { line: u8, expected: u8, found: u8 },
    /// The two lines carry different catalog numbers.
    CatalogMismatch,
    /// A field that cannot be read; carries the line and the field's name.
    Field { line: u8, name: &'static str },
    /// The epoch is outside 1957-2056, which a two-digit TLE year cannot
    /// write. Unix microseconds.
    EpochOutOfRange(i64),
    /// A catalog number above 339999, the largest Alpha-5 can write.
    CatalogOutOfRange(u32),
    /// The TLE was read, but its elements cannot be encoded.
    Encode(EncodeError),
    /// The KepID cannot be decoded.
    Decode(DecodeError),
}

impl fmt::Display for TleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { line, len } => write!(f, "line {line} has {len} characters, expected 69"),
            Self::LineNumber { line } => write!(f, "line {line} does not start with \"{line} \""),
            Self::Checksum { line, expected, found } => {
                write!(f, "line {line} checksum is {found}, expected {expected}")
            }
            Self::CatalogMismatch => write!(f, "the two lines have different catalog numbers"),
            Self::Field { line, name } => write!(f, "line {line}: cannot read {name}"),
            Self::EpochOutOfRange(us) => {
                write!(f, "epoch {us} µs is outside 1957-2056, the years a TLE can write")
            }
            Self::CatalogOutOfRange(n) => write!(f, "catalog number {n} is above 339999, the Alpha-5 limit"),
            Self::Encode(err) => write!(f, "{err}"),
            Self::Decode(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for TleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(err) => Some(err),
            Self::Decode(err) => Some(err),
            _ => None,
        }
    }
}

impl Elements {
    /// Reads the mean elements of a TLE. The identity fields (catalog number,
    /// designator and so on) are checked but not kept: a KepID cannot hold them.
    ///
    /// The first derivative of mean motion, the second derivative and the
    /// ephemeris type are checked only by the checksum: SGP4 does not use them.
    pub fn from_tle(line1: &str, line2: &str) -> Result<Elements, TleError> {
        parse(line1, line2)
    }

    /// Writes these elements as a TLE with catalog number 00000.
    ///
    /// The other identity fields are placeholders: classification `U`, a
    /// blank designator, element set 999 and revolution number 99999. The
    /// derivatives of mean motion are written as zero; SGP4 does not use them.
    /// Angles are wrapped to [0, 360) and every field is rounded to its column
    /// width, so the epoch moves by at most 432 µs.
    pub fn to_tle(&self) -> Result<[String; 2], TleError> {
        write(self, 0)
    }

    /// Like [`Elements::to_tle`], for the object with this NORAD catalog
    /// number (0 to 339999; Alpha-5 above 99999).
    pub fn to_tle_for(&self, catalog: u32) -> Result<[String; 2], TleError> {
        write(self, catalog)
    }
}

/// Reads a TLE and encodes it: [`Elements::from_tle`], then [`encode`].
pub fn from_tle(line1: &str, line2: &str) -> Result<KepId, TleError> {
    encode(&Elements::from_tle(line1, line2)?).map_err(TleError::Encode)
}

/// Decodes a KepID and writes it as a TLE with catalog number 00000:
/// [`decode`], then [`Elements::to_tle`].
pub fn to_tle(id: KepId) -> Result<[String; 2], TleError> {
    decode(id).map_err(TleError::Decode)?.to_tle()
}

/// Like [`to_tle`], for the object with this NORAD catalog number. A KepID
/// describes an orbit, not an object, so the number comes from the caller.
pub fn to_tle_for(id: KepId, catalog: u32) -> Result<[String; 2], TleError> {
    decode(id).map_err(TleError::Decode)?.to_tle_for(catalog)
}

fn parse(line1: &str, line2: &str) -> Result<Elements, TleError> {
    let l1 = check_line(line1, 1)?;
    let l2 = check_line(line2, 2)?;
    let catalog = read_catalog(&l1[2..7]).ok_or(TleError::Field { line: 1, name: "catalog number" })?;
    if read_catalog(&l2[2..7]) != Some(catalog) {
        return Err(TleError::CatalogMismatch);
    }

    let field = |line: u8, name| TleError::Field { line, name };
    let number = |text: &str, line, name| -> Result<f64, TleError> {
        let text = text.trim();
        let plain = text.bytes().all(|b| b.is_ascii_digit() || b == b'.' || b == b'-' || b == b'+');
        if text.is_empty() || !plain {
            return Err(field(line, name));
        }
        text.parse().map_err(|_| field(line, name))
    };

    let eccentricity = &l2[26..33];
    if !eccentricity.bytes().all(|b| b.is_ascii_digit()) {
        return Err(field(2, "eccentricity"));
    }
    let el = Elements {
        epoch_unix_us: tle_epoch_unix_us(&l1[18..32]).ok_or(field(1, "epoch"))?,
        mean_motion: number(&l2[52..63], 2, "mean motion")?,
        eccentricity: format!("0.{eccentricity}").parse().map_err(|_| field(2, "eccentricity"))?,
        inclination: number(&l2[8..16], 2, "inclination")?,
        raan: number(&l2[17..25], 2, "RAAN")?,
        arg_perigee: number(&l2[34..42], 2, "argument of perigee")?,
        mean_anomaly: number(&l2[43..51], 2, "mean anomaly")?,
        bstar: read_exponent(&l1[53..61]).ok_or(field(1, "B*"))?,
    };
    Ok(el)
}

/// Checks a line's length, number and checksum; returns it without the
/// trailing whitespace.
fn check_line(line: &str, number: u8) -> Result<&str, TleError> {
    let line = line.trim_end();
    if line.len() != LINE_LEN || !line.is_ascii() {
        return Err(TleError::Length { line: number, len: line.chars().count() });
    }
    let bytes = line.as_bytes();
    if bytes[0] != b'0' + number || bytes[1] != b' ' {
        return Err(TleError::LineNumber { line: number });
    }
    let found = bytes[68].wrapping_sub(b'0');
    let expected = checksum(&line[..68]);
    if found != expected {
        return Err(TleError::Checksum { line: number, expected, found });
    }
    Ok(line)
}

/// Sum of the digits, with each minus sign counting 1, mod 10.
fn checksum(text: &str) -> u8 {
    let sum: u32 = text
        .bytes()
        .map(|b| match b {
            b'0'..=b'9' => u32::from(b - b'0'),
            b'-' => 1,
            _ => 0,
        })
        .sum();
    (sum % 10) as u8
}

/// Reads a catalog number: five digits, or Alpha-5 (a letter for 10-33, then
/// four digits). Leading spaces are allowed in the digit form.
fn read_catalog(text: &str) -> Option<u32> {
    let text = text.trim_start();
    let (head, digits) = match text.as_bytes().first()? {
        b if b.is_ascii_uppercase() => (10 + ALPHA5.iter().position(|a| a == b)? as u32, &text[1..]),
        _ => (0, text),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || (head > 0 && digits.len() != 4) {
        return None;
    }
    Some(head * 10_000 + digits.parse::<u32>().ok()?)
}

fn write_catalog(catalog: u32) -> Option<String> {
    match catalog {
        0..=99_999 => Some(format!("{catalog:05}")),
        100_000..=MAX_CATALOG => {
            let letter = char::from(ALPHA5[(catalog / 10_000 - 10) as usize]);
            Some(format!("{letter}{:04}", catalog % 10_000))
        }
        _ => None,
    }
}

/// Reads an implied-decimal exponent field, ` 11132-3` = 0.11132e-3.
fn read_exponent(text: &str) -> Option<f64> {
    let b = text.as_bytes();
    let sign = match b[0] {
        b' ' | b'+' => "",
        b'-' => "-",
        _ => return None,
    };
    let (mantissa, exponent) = (&text[1..6], &text[6..8]);
    let mantissa_ok = mantissa.bytes().all(|b| b.is_ascii_digit() || b == b' ');
    let exponent_ok = matches!(b[6], b'+' | b'-') && b[7].is_ascii_digit();
    if !mantissa_ok || !exponent_ok {
        return None;
    }
    format!("{sign}0.{}e{exponent}", mantissa.replace(' ', "0")).parse().ok()
}

/// Writes an implied-decimal exponent field: sign, five digits, exponent.
/// Values too small for exponent -9 are written as zero.
fn write_exponent(x: f64) -> Option<String> {
    let sign = if x < 0.0 { '-' } else { ' ' };
    let text = format!("{:.4e}", x.abs());
    let (digits, exponent) = text.split_once('e')?;
    let exponent = exponent.parse::<i32>().ok()? + 1;
    let digits = digits.replace('.', "");
    match exponent {
        _ if x == 0.0 || exponent < -9 => Some(" 00000+0".to_string()),
        -9..=9 => Some(format!("{sign}{digits}{}{}", if exponent < 0 { '-' } else { '+' }, exponent.abs())),
        _ => None,
    }
}

/// Rounds an angle to four decimals in [0, 360), so 359.99996 writes as 0.
fn angle(degrees: f64) -> String {
    let wrapped = degrees.rem_euclid(360.0);
    let text = format!("{wrapped:8.4}");
    if text == "360.0000" { "  0.0000".to_string() } else { text }
}

fn write(el: &Elements, catalog: u32) -> Result<[String; 2], TleError> {
    let catalog_text = write_catalog(catalog).ok_or(TleError::CatalogOutOfRange(catalog))?;
    let epoch = tle_epoch_field(el.epoch_unix_us).ok_or(TleError::EpochOutOfRange(el.epoch_unix_us))?;
    let bstar = write_exponent(el.bstar).ok_or(TleError::Field { line: 1, name: "B*" })?;
    let eccentricity = (el.eccentricity * 1e7).round().clamp(0.0, 9_999_999.0) as u32;
    let mean_motion = format!("{:11.8}", el.mean_motion);
    if mean_motion.len() != 11 {
        return Err(TleError::Field { line: 2, name: "mean motion" });
    }

    let line1 = format!(
        "1 {catalog_text}{CLASSIFICATION}          {epoch}  .00000000  00000+0 {bstar} 0 {ELEMENT_SET:>4}"
    );
    let line2 = format!(
        "2 {catalog_text} {} {} {eccentricity:07} {} {} {mean_motion}{REV_NUMBER:>5}",
        angle(el.inclination),
        angle(el.raan),
        angle(el.arg_perigee),
        angle(el.mean_anomaly),
    );
    let finish = |line: String| {
        let digit = checksum(&line);
        format!("{line}{digit}")
    };
    Ok([finish(line1), finish(line2)])
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISS: [&str; 2] = [
        "1 25544U 98067A   26305.20860139  .00006903  00000+0  13267-3 0  9994",
        "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592994",
    ];

    #[test]
    fn reads_the_iss() {
        let el = Elements::from_tle(ISS[0], ISS[1]).unwrap();
        assert_eq!(el.epoch_unix_us, 1_793_509_223_160_096);
        assert_eq!(
            (el.mean_motion, el.eccentricity, el.inclination, el.raan),
            (15.49138562, 0.0007056, 51.6332, 342.0408)
        );
        assert_eq!((el.arg_perigee, el.mean_anomaly, el.bstar), (334.5041, 25.5599, 0.13267e-3));
    }

    #[test]
    fn readme_example() {
        let id = from_tle(ISS[0], ISS[1]).unwrap();
        assert_eq!(id.to_string(), "01a1957a-ebef-73ab-bccf-878de5f3a90d");
    }

    #[test]
    fn writes_what_it_reads() {
        let el = Elements::from_tle(ISS[0], ISS[1]).unwrap();
        let [l1, l2] = el.to_tle_for(25544).unwrap();
        // The designator is blank, the derivatives of mean motion are zero, and
        // the element set and revolution numbers are placeholders.
        assert_eq!(l1, "1 25544U          26305.20860139  .00000000  00000+0  13267-3 0  9996");
        assert_eq!(l2, "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562999995");
        assert_eq!(l2[..63], ISS[1][..63]);
    }

    #[test]
    fn kepid_round_trips_through_text() {
        let id = from_tle(ISS[0], ISS[1]).unwrap();
        let [l1, l2] = to_tle(id).unwrap();
        assert!(l1.starts_with("1 00000U          26305.20860"), "{l1}");
        assert!(l2.starts_with("2 00000 ") && l2[63..68] == *"99999", "{l2}");
        assert_eq!(from_tle(&l1, &l2).unwrap(), id);
        let [l1, l2] = to_tle_for(id, 25544).unwrap();
        assert!(l1.starts_with("1 25544U          26305.20860") && l2.starts_with("2 25544 "), "{l1}");
        assert_eq!(from_tle(&l1, &l2).unwrap(), id);
    }

    #[test]
    fn checksum_and_shape_errors() {
        let bad_sum = ISS[0].replace("9994", "9995");
        assert_eq!(
            Elements::from_tle(&bad_sum, ISS[1]),
            Err(TleError::Checksum { line: 1, expected: 4, found: 5 })
        );
        assert_eq!(Elements::from_tle(&ISS[0][..60], ISS[1]), Err(TleError::Length { line: 1, len: 60 }));
        assert_eq!(Elements::from_tle(ISS[1], ISS[0]), Err(TleError::LineNumber { line: 1 }));
        let other = "2 25545  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592995";
        assert_eq!(Elements::from_tle(ISS[0], other), Err(TleError::CatalogMismatch));
        // Trailing whitespace and CR are fine.
        assert!(Elements::from_tle(&format!("{}  \r", ISS[0]), ISS[1]).is_ok());
    }

    #[test]
    fn alpha5_catalog_numbers() {
        for (n, text) in [(0, "00000"), (99_999, "99999"), (100_000, "A0000"), (172_345, "H2345"), (189_999, "J9999")] {
            assert_eq!(write_catalog(n).as_deref(), Some(text));
            assert_eq!(read_catalog(text), Some(n));
        }
        assert_eq!(write_catalog(180_000).as_deref(), Some("J0000"), "I is skipped");
        assert_eq!(write_catalog(230_000).as_deref(), Some("P0000"), "O is skipped");
        assert_eq!(write_catalog(MAX_CATALOG).as_deref(), Some("Z9999"));
        assert_eq!(write_catalog(MAX_CATALOG + 1), None);
        assert_eq!(read_catalog("I0000"), None);
        assert_eq!(read_catalog(" 4444"), Some(4444));
    }

    #[test]
    fn exponent_fields() {
        for (text, x) in [(" 13267-3", 0.13267e-3), ("-11606-4", -0.11606e-4), (" 00000+0", 0.0), (" 10000+1", 1.0)] {
            assert_eq!(read_exponent(text), Some(x), "{text}");
            assert_eq!(write_exponent(x).as_deref(), Some(text));
        }
        assert_eq!(read_exponent(" 12345 3"), None);
        assert_eq!(write_exponent(0.999996).as_deref(), Some(" 10000+1"), "rounding carries");
        assert_eq!(write_exponent(1e-12).as_deref(), Some(" 00000+0"));
        assert_eq!(write_exponent(2e9), None);
    }

    #[test]
    fn epoch_fields() {
        assert_eq!(tle_epoch_field(1_793_509_223_160_096).as_deref(), Some("26305.20860139"));
        assert_eq!(tle_epoch_field(0).as_deref(), Some("70001.00000000"));
        // The last 432 µs of a year round into the next one.
        let new_year = tle_epoch_unix_us("27001.0").unwrap();
        assert_eq!(tle_epoch_field(new_year - 432).as_deref(), Some("27001.00000000"));
        assert_eq!(tle_epoch_field(new_year - 433).as_deref(), Some("26365.99999999"));
        assert_eq!(tle_epoch_field(tle_epoch_unix_us("57001.0").unwrap() - 1000), None);
        assert_eq!(tle_epoch_field(tle_epoch_unix_us("57001.0").unwrap()).as_deref(), Some("57001.00000000"));
    }

    #[test]
    fn angles_wrap_and_round() {
        assert_eq!(angle(359.99996), "  0.0000");
        assert_eq!(angle(-10.0), "350.0000");
        assert_eq!(angle(51.6332), " 51.6332");
    }

    #[test]
    fn catalog_must_fit_alpha5() {
        let el = Elements::from_tle(ISS[0], ISS[1]).unwrap();
        assert!(el.to_tle_for(MAX_CATALOG).is_ok());
        assert_eq!(el.to_tle_for(340_000), Err(TleError::CatalogOutOfRange(340_000)));
        let early = Elements { epoch_unix_us: tle_epoch_unix_us("57001.0").unwrap() - 1_000_000, ..el };
        assert!(matches!(early.to_tle(), Err(TleError::EpochOutOfRange(_))));
    }
}
