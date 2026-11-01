use std::fmt;

/// Why a set of elements cannot be encoded as a profile 0 KepID.
#[derive(Debug, Clone, PartialEq)]
pub enum EncodeError {
    NotFinite(&'static str),
    EpochBeforeUnixEpoch,
    /// Past the last 256 ms tick the 40-bit epoch can hold (year 10889).
    /// The epoch, Unix microseconds.
    EpochAfterLastTick(i64),
    MeanMotionNotPositive(f64),
    /// Outside [0, 1): not a closed orbit.
    EccentricityOutOfRange(f64),
    /// Outside [0, 180] degrees.
    InclinationOutOfRange(f64),
    /// The perigee altitude, km, below the 250 km floor: a re-entering object.
    PerigeeBelowFloor(f64),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite(name) => write!(f, "{name} is not a finite number"),
            Self::EpochBeforeUnixEpoch => write!(f, "epoch is before 1970-01-01"),
            Self::EpochAfterLastTick(us) => {
                write!(f, "epoch {us} µs is past the last representable tick (year 10889)")
            }
            Self::MeanMotionNotPositive(n) => write!(f, "mean motion {n} rev/day is not positive"),
            Self::EccentricityOutOfRange(e) => {
                write!(f, "eccentricity {e} is outside [0, 1), not a closed orbit")
            }
            Self::InclinationOutOfRange(i) => {
                write!(f, "inclination {i} deg is outside [0, 180]")
            }
            Self::PerigeeBelowFloor(h) => {
                write!(f, "perigee altitude {h:.1} km is below the 250 km floor")
            }
        }
    }
}

impl std::error::Error for EncodeError {}

/// Why a KepID cannot be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    NotUuidV7(u8),
    BadVariant(u8),
    UnsupportedProfile(u8),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotUuidV7(v) => write!(f, "not a UUIDv7 (version nibble is {v})"),
            Self::BadVariant(v) => write!(f, "not an RFC 9562 UUID (variant bits are {v:02b})"),
            Self::UnsupportedProfile(p) => write!(f, "unsupported KepID profile {p}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Why a string is not a UUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// Not 32 (bare) or 36 (hyphenated) characters; carries the length found.
    Length(usize),
    /// 36 characters, but the hyphens are not at positions 8, 13, 18 and 23.
    Hyphens,
    NotHex,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(n) => write!(f, "expected 32 or 36 characters, found {n}"),
            Self::Hyphens => write!(f, "hyphens must be at positions 8, 13, 18 and 23"),
            Self::NotHex => write!(f, "contains a character that is not a hex digit"),
        }
    }
}

impl std::error::Error for ParseError {}
