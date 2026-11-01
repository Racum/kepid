use std::fmt;
use std::str::FromStr;

use crate::error::{DecodeError, ParseError};
use crate::profile0::*;

/// A KepID: a 128-bit UUIDv7-shaped value carrying SGP4 mean elements.
///
/// Wrapping and printing never validate; use [`KepId::validate`] or
/// [`crate::decode`] for that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KepId(u128);

impl KepId {
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }

    pub const fn as_u128(self) -> u128 {
        self.0
    }

    /// Builds a KepID from its 16 bytes in RFC 9562 (big-endian) order.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(u128::from_be_bytes(bytes))
    }

    /// The 16 bytes in RFC 9562 (big-endian) order.
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    /// The UUID version nibble. A KepID always has 7.
    pub const fn version(self) -> u8 {
        get(self.0, VERSION_BITS) as u8
    }

    /// The two UUID variant bits. A KepID always has `0b10`.
    pub const fn variant(self) -> u8 {
        get(self.0, VARIANT_BITS) as u8
    }

    /// The profile bit (bit 90). Profile 0 is the only one defined.
    pub const fn profile(self) -> u8 {
        get(self.0, PROFILE_BITS) as u8
    }

    /// Epoch in 256 ms ticks since 1970-01-01 UTC, without a full decode.
    pub const fn epoch_ticks(self) -> u64 {
        get(self.0, EPOCH_BITS) as u64
    }

    /// Epoch in Unix microseconds, without a full decode.
    pub const fn epoch_unix_us(self) -> i64 {
        self.epoch_ticks() as i64 * TICK_US
    }

    /// Checks the version, variant and profile bits. Any value that passes
    /// decodes to a valid orbit.
    pub fn validate(self) -> Result<(), DecodeError> {
        if self.version() != 7 {
            return Err(DecodeError::NotUuidV7(self.version()));
        }
        if self.variant() != 0b10 {
            return Err(DecodeError::BadVariant(self.variant()));
        }
        if self.profile() != 0 {
            return Err(DecodeError::UnsupportedProfile(self.profile()));
        }
        Ok(())
    }
}

impl fmt::Display for KepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let x = self.0;
        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            (x >> 96) as u32,
            (x >> 80) as u16,
            (x >> 64) as u16,
            (x >> 48) as u16,
            (x & 0xffff_ffff_ffff) as u64
        )
    }
}

impl FromStr for KepId {
    type Err = ParseError;

    /// Accepts 36 characters with hyphens at positions 8, 13, 18 and 23, or 32
    /// bare hex digits.
    fn from_str(s: &str) -> Result<Self, ParseError> {
        let chars: Vec<char> = s.chars().collect();
        let digits: Vec<char> = match chars.len() {
            32 => chars,
            36 => {
                if [8, 13, 18, 23].iter().any(|&i| chars[i] != '-') {
                    return Err(ParseError::Hyphens);
                }
                chars.into_iter().filter(|c| *c != '-').collect()
            }
            n => return Err(ParseError::Length(n)),
        };
        let mut value: u128 = 0;
        for c in digits {
            let d = c.to_digit(16).ok_or(ParseError::NotHex)?;
            value = (value << 4) | u128::from(d);
        }
        Ok(Self(value))
    }
}

/// The raw codes of a profile 0 KepID, before any scaling.
///
/// Layout (offset, width): epoch 0/40, arg_perigee 40/8, inclination 52/12,
/// raan 66/14, drag 80/10, profile 90/1, lambda 91/13, mean_motion 104/17,
/// eccentricity 121/7. The version (48/4) and variant (64/2) markers are implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fields {
    pub epoch_ticks: u64,
    /// ω code.
    pub arg_perigee: u8,
    pub inclination: u16,
    /// Ω code.
    pub raan: u16,
    /// D code; 512 is exactly zero drag.
    pub drag: u16,
    pub profile: u8,
    /// λ = ω + M code.
    pub lambda: u16,
    /// w = R_floor / a code.
    pub mean_motion: u32,
    /// s = e / (1 - w) code.
    pub eccentricity: u8,
}

impl Fields {
    /// Reads the fields without validating any marker bits.
    pub fn unpack(id: KepId) -> Self {
        let x = id.0;
        Self {
            epoch_ticks: get(x, EPOCH_BITS) as u64,
            arg_perigee: get(x, ARG_PERIGEE_BITS) as u8,
            inclination: get(x, INCLINATION_BITS) as u16,
            raan: get(x, RAAN_BITS) as u16,
            drag: get(x, DRAG_BITS) as u16,
            profile: get(x, PROFILE_BITS) as u8,
            lambda: get(x, LAMBDA_BITS) as u16,
            mean_motion: get(x, MEAN_MOTION_BITS) as u32,
            eccentricity: get(x, ECCENTRICITY_BITS) as u8,
        }
    }

    /// Lays the fields out, inserting the UUIDv7 version and variant bits.
    pub fn pack(&self) -> KepId {
        KepId(
            put(self.epoch_ticks.into(), EPOCH_BITS)
                | put(self.arg_perigee.into(), ARG_PERIGEE_BITS)
                | put(0b0111, VERSION_BITS)
                | put(self.inclination.into(), INCLINATION_BITS)
                | put(0b10, VARIANT_BITS)
                | put(self.raan.into(), RAAN_BITS)
                | put(self.drag.into(), DRAG_BITS)
                | put(self.profile.into(), PROFILE_BITS)
                | put(self.lambda.into(), LAMBDA_BITS)
                | put(self.mean_motion.into(), MEAN_MOTION_BITS)
                | put(self.eccentricity.into(), ECCENTRICITY_BITS),
        )
    }
}

/// Reads the `(offset, width)` bits, where bit 0 is the most significant.
const fn get(x: u128, (offset, size): (u32, u32)) -> u128 {
    (x >> (128 - offset - size)) & ((1 << size) - 1)
}

/// Places `value` in the `(offset, width)` bits, where bit 0 is the most significant.
const fn put(value: u128, (offset, size): (u32, u32)) -> u128 {
    (value & ((1 << size) - 1)) << (128 - offset - size)
}
