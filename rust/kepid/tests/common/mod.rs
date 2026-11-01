#![allow(dead_code)]

use kepid::Elements;

/// The spec's example: the ISS element set below.
pub const ISS_ID: &str = "01a17b91-23e2-73ab-813c-86cddaf3a10c";
/// The ISS input epoch, 2026-10-27T04:14:31.558560Z.
pub const ISS_INPUT_EPOCH_US: i64 = 1_793_074_471_558_560;
/// Its decoded epoch, the nearest 256 ms tick: 2026-10-27T04:14:31.680Z.
pub const ISS_EPOCH_US: i64 = 1_793_074_471_680_000;
/// The last representable tick, 2^40 - 1.
pub const MAX_TICKS: u64 = (1 << 40) - 1;

/// The ISS element set of the spec's test vectors.
pub fn base() -> Elements {
    Elements {
        epoch_unix_us: ISS_INPUT_EPOCH_US,
        mean_motion: 15.49073103,
        eccentricity: 0.0006948,
        inclination: 51.6336,
        raan: 6.9404,
        arg_perigee: 314.9066,
        mean_anomaly: 45.1358,
        bstar: 0.00011132,
    }
}

pub fn approx(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// Difference of two angles in degrees, in [0, 180].
pub fn angle_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// Small deterministic generator, so sweeps are reproducible without a crate.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform in `[lo, hi)`.
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * (hi - lo)
    }

    /// A random 128-bit value with the profile 0 markers set.
    pub fn kepid_bits(&mut self) -> u128 {
        let v = (u128::from(self.next_u64()) << 64) | u128::from(self.next_u64());
        let v = v & !(0xF << 76) | (0x7 << 76);
        let v = v & !(0b11 << 62) | (0b10 << 62);
        v & !(1 << 37)
    }

    /// A random encodable element set, LEO to beyond GEO, perigee above the floor.
    pub fn elements(&mut self) -> Elements {
        let mean_motion = if self.range(0.0, 1.0) < 0.7 { self.range(11.0, 16.0) } else { self.range(0.1, 11.0) };
        let w = kepid::w_of_mean_motion(mean_motion);
        let e_max = (1.0 - w).min(0.99) * 0.999;
        let eccentricity = if self.range(0.0, 1.0) < 0.6 { self.range(0.0, e_max.min(0.01)) } else { self.range(0.0, e_max) };
        Elements {
            epoch_unix_us: self.range(0.0, 4.0e15) as i64,
            mean_motion,
            eccentricity,
            inclination: self.range(0.0, 180.0),
            raan: self.range(-720.0, 720.0),
            arg_perigee: self.range(-720.0, 720.0),
            mean_anomaly: self.range(-720.0, 720.0),
            bstar: self.range(-1e-3, 3e-3),
        }
    }
}
