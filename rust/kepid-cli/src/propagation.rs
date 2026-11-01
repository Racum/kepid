//! Where the orbit a KepID describes puts the satellite at a given time: SGP4
//! from the decoded elements, then TEME to WGS-84 geodetic coordinates.
//!
//! This lives in the CLI only; the library never propagates.

use kepid::KepId;

/// WGS-84 equatorial radius, km, and flattening.
const WGS84_A: f64 = 6378.137;
const WGS84_F: f64 = 1.0 / 298.257223563;
const US_PER_MINUTE: f64 = 60_000_000.0;
/// Unix microseconds of J2000.0, 2000-01-01T12:00:00 (UTC is close enough here).
const J2000_UNIX_US: i64 = 946_728_000_000_000;

/// A propagated position, and the orbit's period.
#[derive(Debug)]
pub(crate) struct Position {
    /// Geodetic latitude, degrees.
    pub latitude: f64,
    /// Longitude, degrees east, in (-180, 180].
    pub longitude: f64,
    /// Altitude above the WGS-84 ellipsoid, km.
    pub altitude_km: f64,
    /// Speed in the inertial (TEME) frame, km/s.
    pub speed_km_s: f64,
    /// Period from the mean motion, minutes.
    pub period_minutes: f64,
}

/// Propagates the orbit of `id` to `unix_us`, in Unix microseconds.
pub(crate) fn propagate(id: KepId, unix_us: i64) -> Result<Position, String> {
    let el = kepid::decode(id).map_err(|err| err.to_string())?;
    // The KepID's own TLE: SGP4 reads exactly what any TLE consumer would.
    let [l1, l2] = kepid::to_tle(id).map_err(|err| err.to_string())?;
    let elements = sgp4::Elements::from_tle(None, l1.as_bytes(), l2.as_bytes()).map_err(|err| err.to_string())?;
    let constants = sgp4::Constants::from_elements(&elements).map_err(|err| err.to_string())?;
    let minutes = (unix_us - el.epoch_unix_us) as f64 / US_PER_MINUTE;
    let prediction = constants.propagate(sgp4::MinutesSinceEpoch(minutes)).map_err(|err| err.to_string())?;

    // TEME to Earth-fixed: a rotation by the Greenwich sidereal time (polar
    // motion, a few metres, is left out).
    let years = (unix_us - J2000_UNIX_US) as f64 / US_PER_MINUTE / 525_960.0;
    let gmst = sgp4::iau_epoch_to_sidereal_time(years);
    let [x, y, z] = prediction.position;
    let (s, c) = gmst.sin_cos();
    let (xe, ye) = (c * x + s * y, -s * x + c * y);

    let (latitude, altitude_km) = geodetic(xe, ye, z);
    let [vx, vy, vz] = prediction.velocity;
    Ok(Position {
        latitude: latitude.to_degrees(),
        longitude: ye.atan2(xe).to_degrees(),
        altitude_km,
        speed_km_s: (vx * vx + vy * vy + vz * vz).sqrt(),
        period_minutes: 1440.0 / el.mean_motion,
    })
}

/// Geodetic latitude (radians) and altitude (km) of an Earth-fixed position,
/// by fixed-point iteration on the WGS-84 ellipsoid.
fn geodetic(x: f64, y: f64, z: f64) -> (f64, f64) {
    let e2 = WGS84_F * (2.0 - WGS84_F);
    let p = x.hypot(y);
    let mut lat = z.atan2(p * (1.0 - e2));
    let mut n = WGS84_A;
    for _ in 0..10 {
        n = WGS84_A / (1.0 - e2 * lat.sin().powi(2)).sqrt();
        lat = (z + e2 * n * lat.sin()).atan2(p);
    }
    let alt = if lat.cos().abs() > 1e-9 { p / lat.cos() - n } else { z.abs() - n * (1.0 - e2) };
    (lat, alt)
}

/// The analysis section's lines.
pub(crate) fn lines(position: &Position) -> String {
    let p = position;
    format!(
        "- Latitude: {:.4}\n\
         - Longitude: {:.4}\n\
         - Altitude: {:.1} km\n\
         - Speed: {:.0} km/h or {:.0} m/s\n\
         - Orbit duration: {:.2} minutes\n",
        p.latitude,
        p.longitude,
        p.altitude_km,
        p.speed_km_s * 3600.0,
        p.speed_km_s * 1000.0,
        p.period_minutes
    )
}
