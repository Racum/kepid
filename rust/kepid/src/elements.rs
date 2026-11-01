use crate::codec::{semi_major_axis_km, w_of_mean_motion};
use crate::error::EncodeError;
use crate::profile0::{MAX_EPOCH_TICKS, RE, TICK_US};
use crate::time;

/// SGP4 mean elements, in the units a TLE uses.
///
/// This is the intermediate form between a source element set (TLE, OMM) and a
/// [`crate::KepId`]. Angles are degrees, mean motion is rev/day.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Elements {
    /// Element-set epoch, whole microseconds since 1970-01-01 UTC.
    ///
    /// An integer, so the epoch tick never depends on float rounding. TLE
    /// epochs are exact in microseconds; see [`crate::tle_epoch_unix_us`].
    pub epoch_unix_us: i64,
    /// Mean motion, rev/day.
    pub mean_motion: f64,
    pub eccentricity: f64,
    /// Inclination, degrees.
    pub inclination: f64,
    /// Right ascension of the ascending node, degrees (TEME).
    pub raan: f64,
    /// Argument of perigee, degrees.
    pub arg_perigee: f64,
    /// Mean anomaly, degrees.
    pub mean_anomaly: f64,
    /// SGP4 B* drag term, 1/Earth radii.
    pub bstar: f64,
}

impl Elements {
    /// Mean argument of latitude, `argp + M`, in `[0, 360)` degrees.
    pub fn mean_arg_latitude(&self) -> f64 {
        wrap_degrees(self.arg_perigee + self.mean_anomaly)
    }

    /// Keplerian semi-major axis from the mean motion, km.
    ///
    /// This is the plain two-body value, not SGP4's Kozai/Brouwer mean axis, so
    /// it is good for sanity checks and display only.
    pub fn semi_major_axis_km(&self) -> f64 {
        semi_major_axis_km(self.mean_motion)
    }

    pub fn perigee_altitude_km(&self) -> f64 {
        self.semi_major_axis_km() * (1.0 - self.eccentricity) - RE
    }

    pub fn apogee_altitude_km(&self) -> f64 {
        self.semi_major_axis_km() * (1.0 + self.eccentricity) - RE
    }

    pub fn period_minutes(&self) -> f64 {
        1440.0 / self.mean_motion
    }

    /// The epoch as `YYYY-MM-DDTHH:MM:SS.mmmZ`, or with six fractional
    /// digits when it is not a whole millisecond.
    pub fn epoch_iso8601(&self) -> String {
        time::iso8601(self.epoch_unix_us)
    }
}

/// Wraps an angle into `[0, 360)`.
///
/// `rem_euclid` alone can return exactly 360.0 (for a tiny negative input) or
/// -0.0, neither of which is in the range.
pub(crate) fn wrap_degrees(degrees: f64) -> f64 {
    let wrapped = degrees.rem_euclid(360.0);
    if wrapped >= 360.0 || wrapped == 0.0 {
        0.0
    } else {
        wrapped
    }
}

/// The nearest 256 ms tick to a non-negative epoch, halves rounding up.
/// Integer arithmetic only (`docs/spec.md`, section 7, step 2).
pub(crate) fn epoch_tick(epoch_unix_us: i64) -> u64 {
    ((epoch_unix_us as u64).saturating_add(TICK_US as u64 / 2)) / TICK_US as u64
}

/// Checks that an element set can be encoded (`docs/spec.md`, section 7,
/// step 1): finite values, a closed orbit, 0 <= i <= 180, an epoch inside the
/// 40-bit tick range, and a perigee at or above 250 km.
pub fn check_range(el: &Elements) -> Result<(), EncodeError> {
    let finite = [
        ("mean motion", el.mean_motion),
        ("eccentricity", el.eccentricity),
        ("inclination", el.inclination),
        ("RAAN", el.raan),
        ("argument of perigee", el.arg_perigee),
        ("mean anomaly", el.mean_anomaly),
        ("B*", el.bstar),
    ];
    for (name, value) in finite {
        if !value.is_finite() {
            return Err(EncodeError::NotFinite(name));
        }
    }
    if el.epoch_unix_us < 0 {
        return Err(EncodeError::EpochBeforeUnixEpoch);
    }
    if epoch_tick(el.epoch_unix_us) > MAX_EPOCH_TICKS {
        return Err(EncodeError::EpochAfterLastTick(el.epoch_unix_us));
    }
    if el.mean_motion <= 0.0 {
        return Err(EncodeError::MeanMotionNotPositive(el.mean_motion));
    }
    if !(0.0..1.0).contains(&el.eccentricity) {
        return Err(EncodeError::EccentricityOutOfRange(el.eccentricity));
    }
    if !(0.0..=180.0).contains(&el.inclination) {
        return Err(EncodeError::InclinationOutOfRange(el.inclination));
    }
    // One part in 1e9 of slack, so every decoded element set encodes again.
    if 1.0 - el.eccentricity < w_of_mean_motion(el.mean_motion) * (1.0 - 1e-9) {
        return Err(EncodeError::PerigeeBelowFloor(el.perigee_altitude_km()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::wrap_degrees;

    #[test]
    fn wrap_stays_in_half_open_range() {
        for (input, want) in [
            (0.0, 0.0),
            (-0.0, 0.0),
            (359.5, 359.5),
            (360.0, 0.0),
            (720.0, 0.0),
            (-360.0, 0.0),
            (-10.0, 350.0),
            (370.0, 10.0),
            (-1e-18, 0.0),
            (-1e-14, 0.0), // closer to 360.0 than to the next float below it
            (-1e-13, 360.0 - 1e-13),
        ] {
            let got = wrap_degrees(input);
            assert!((0.0..360.0).contains(&got), "{input} -> {got}");
            assert!(got.is_sign_positive(), "{input} -> {got}");
            assert!((got - want).abs() < 1e-12, "{input} -> {got}, want {want}");
        }
    }
}
