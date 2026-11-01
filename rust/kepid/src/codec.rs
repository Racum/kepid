use crate::elements::{check_range, epoch_tick, wrap_degrees, Elements};
use crate::error::{DecodeError, EncodeError};
use crate::kepid_id::{Fields, KepId};
use crate::profile0::*;

/// Encodes mean elements as a profile 0 KepID (`docs/spec.md`, section 7).
///
/// The epoch rounds to the nearest 256 ms tick, and the mean argument of
/// latitude moves by the satellite's motion over that rounding, so the decoded
/// element set is consistent with its decoded epoch. Every other field is
/// rounded on its own; nothing is propagated or fitted.
pub fn encode(el: &Elements) -> Result<KepId, EncodeError> {
    check_range(el)?;
    let tick = epoch_tick(el.epoch_unix_us);
    // Exact: |Δt| <= 128 000 µs, an integer well inside binary64.
    let dt_us = (tick as i64 * TICK_US - el.epoch_unix_us) as f64;

    // Order matters: e is scaled with the decoded w, drag uses the decoded w and e.
    let mean_motion = MEAN_MOTION.encode(w_of_mean_motion(el.mean_motion));
    let w = MEAN_MOTION.decode(mean_motion);
    let em = 1.0 - w;
    let eccentricity = ECCENTRICITY.encode(el.eccentricity / em);
    let e = em * ECCENTRICITY.decode(eccentricity);

    let shift = 360.0 * el.mean_motion * dt_us / 86_400_000_000.0;
    let lambda = wrap_degrees(el.arg_perigee + el.mean_anomaly + shift);

    let fields = Fields {
        epoch_ticks: tick,
        arg_perigee: ARG_PERIGEE.encode(wrap_degrees(el.arg_perigee)) as u8,
        inclination: INCLINATION.encode(el.inclination) as u16,
        raan: RAAN.encode(wrap_degrees(el.raan)) as u16,
        drag: DRAG.encode(el.bstar * drag_factor(w, e)) as u16,
        profile: 0,
        lambda: LAMBDA.encode(lambda) as u16,
        mean_motion,
        eccentricity: eccentricity as u8,
    };
    Ok(fields.pack())
}

/// Decodes a profile 0 KepID into mean elements (`docs/spec.md`, section 8).
///
/// Every value with the right version, variant and profile bits decodes to a
/// closed orbit whose perigee is at or above 250 km.
pub fn decode(id: KepId) -> Result<Elements, DecodeError> {
    id.validate()?;
    Ok(elements_from_fields(&Fields::unpack(id)))
}

pub(crate) fn elements_from_fields(f: &Fields) -> Elements {
    let w = MEAN_MOTION.decode(f.mean_motion);
    let e = (1.0 - w) * ECCENTRICITY.decode(f.eccentricity.into());
    let arg_perigee = wrap_degrees(ARG_PERIGEE.decode(f.arg_perigee.into()));
    Elements {
        epoch_unix_us: f.epoch_ticks as i64 * TICK_US,
        mean_motion: N_FLOOR * w * w.sqrt(),
        eccentricity: e,
        inclination: INCLINATION.decode(f.inclination.into()),
        raan: wrap_degrees(RAAN.decode(f.raan.into())),
        arg_perigee,
        mean_anomaly: wrap_degrees(LAMBDA.decode(f.lambda.into()) - arg_perigee),
        bstar: DRAG.decode(f.drag.into()) / drag_factor(w, e),
    }
}

/// w = R_FLOOR / a = (n / N_FLOOR)^(2/3): in (0, 1] for every orbit that
/// clears the perigee floor, where the floor reads e <= 1 - w.
pub fn w_of_mean_motion(mean_motion: f64) -> f64 {
    let v = mean_motion / N_FLOOR;
    cbrt((v * v).clamp(1e-30, 1e30))
}

/// Two-body semi-major axis from the mean motion in rev/day, km.
pub(crate) fn semi_major_axis_km(mean_motion: f64) -> f64 {
    R_FLOOR / w_of_mean_motion(mean_motion)
}

/// The spec's cube root (`docs/spec.md`, section 2): exact scaling by
/// 8 into [1, 8), then six Newton steps from 1.5. Not `f64::cbrt`, whose
/// last bit is up to the platform's maths library.
pub(crate) fn cbrt(x: f64) -> f64 {
    let (mut m, mut p) = (x, 1.0);
    while m < 1.0 {
        m *= 8.0;
        p /= 2.0;
    }
    while m >= 8.0 {
        m /= 8.0;
        p *= 2.0;
    }
    let mut y = 1.5;
    for _ in 0..6 {
        y = (2.0 * y + m / (y * y)) / 3.0;
    }
    y * p
}

/// SGP4's sensitivity of the along-track position to B*, written in w and e:
/// a² n² (a - s)⁻⁴ (1 - η²)⁻³·⁵, up to a constant.
pub(crate) fn drag_g(w: f64, e: f64) -> f64 {
    let q = C_SGP4 - S_SGP4 * w;
    let eta = C_SGP4 * e / q;
    let x = (1.0 - eta * eta).max(1e-12);
    let w2 = w * w;
    let q2 = q * q;
    w2 * w2 * w / (q2 * q2) / (x * x * x * x.sqrt())
}

/// The drag factor: D = B* x drag_factor(w, e), 1 for a circular orbit at 500 km.
pub fn drag_factor(w: f64, e: f64) -> f64 {
    drag_g(w, e) / G_REF
}

#[cfg(test)]
mod tests {
    use super::cbrt;

    #[test]
    fn exact_cubes() {
        for root in [1.0, 2.0, 3.0, 0.5, 0.25, 1024.0, 1.0 / 1024.0] {
            assert_eq!(cbrt(root * root * root), root);
        }
    }

    #[test]
    fn within_one_ulp_of_a_correct_cube_root() {
        for j in -300..=300 {
            let x = 10f64.powf(f64::from(j) / 10.0);
            let (a, b) = (cbrt(x), x.cbrt());
            assert!((a - b).abs() <= 2.0 * f64::EPSILON * b, "{x}: {a} vs {b}");
        }
    }
}
