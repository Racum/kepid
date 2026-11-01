use kepid::drag_factor;
use kepid::profile0::*;
use kepid::{Fields, KepId};

/// Describes every field of a KepID: raw code, decoded value, bit position.
///
/// Reads the bits without validating them, so it also works on values that
/// [`kepid::decode`] would refuse.
pub(crate) fn explain(id: KepId) -> String {
    let f = Fields::unpack(id);
    let el = crate::unchecked_elements(&f);
    let w = MEAN_MOTION.decode(f.mean_motion);
    let d = DRAG.decode(f.drag.into());
    let profile = if f.profile == 0 { "profile 0" } else { "reserved" };

    let rows: [(&str, (u32, u32), String, String); 9] = [
        ("epoch", EPOCH_BITS, f.epoch_ticks.to_string(), format!("{} ({} ticks of 256 ms)", el.epoch_iso8601(), f.epoch_ticks)),
        ("argp", ARG_PERIGEE_BITS, f.arg_perigee.to_string(), format!("ω = {:.4} deg", el.arg_perigee)),
        ("i", INCLINATION_BITS, f.inclination.to_string(), format!("{:.4} deg", el.inclination)),
        ("raan", RAAN_BITS, f.raan.to_string(), format!("Ω = {:.4} deg", el.raan)),
        ("drag", DRAG_BITS, f.drag.to_string(), format!("D = {d:+.4e} -> B* = {:.4e} (factor {:.3e})", el.bstar, drag_factor(w, el.eccentricity))),
        ("profile", PROFILE_BITS, f.profile.to_string(), profile.into()),
        ("lambda", LAMBDA_BITS, f.lambda.to_string(), format!("λ = {:.4} deg (argp + M), M = {:.4} deg", el.mean_arg_latitude(), el.mean_anomaly)),
        ("n", MEAN_MOTION_BITS, f.mean_motion.to_string(), format!("w = {w:.8} -> n = {:.8} rev/day", el.mean_motion)),
        ("e", ECCENTRICITY_BITS, f.eccentricity.to_string(), format!("e = {:.7}", el.eccentricity)),
    ];

    let mut out = format!("{id}\n  version {}  variant {:02b}\n", id.version(), id.variant());
    for (name, (offset, bits), raw, meaning) in rows {
        out.push_str(&format!("  {name:<8} bits {offset:>3}..{:<3} raw {raw:>13}  {meaning}\n", offset + bits));
    }
    out.push_str(&format!(
        "  derived  perigee {:.1} km, apogee {:.1} km, period {:.2} min\n",
        el.perigee_altitude_km(),
        el.apogee_altitude_km(),
        el.period_minutes()
    ));
    out
}
