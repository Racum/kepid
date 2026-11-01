use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use kepid::profile0::DRAG;
use kepid::{Fields, KepId};

/// Mean altitude below which a zero B* is implausible, km.
const NO_DRAG_BELOW_KM: f64 = 450.0;
/// Mean altitude above which a large B* is implausible, km.
const HIGH_DRAG_ABOVE_KM: f64 = 800.0;
/// B* that counts as large.
const HIGH_BSTAR: f64 = 1e-3;
/// How far past "now" an epoch may be before it looks wrong.
const FUTURE_SLACK_US: i64 = 86_400_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Severity {
    /// Odd, but not evidence of anything.
    Info,
    /// Physically unlikely; the value may not be a KepID.
    Warning,
    /// Cannot be a profile 0 KepID.
    Error,
}

/// One result of [`lint`]. Findings are hints, not proof either way.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Finding {
    pub severity: Severity,
    /// Stable machine-readable identifier.
    pub code: &'static str,
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} [{}] {}", self.severity, self.code, self.message)
    }
}

fn finding(severity: Severity, code: &'static str, message: String) -> Finding {
    Finding {
        severity,
        code,
        message,
    }
}

/// Checks a KepID for structural and physical problems, using the system clock.
///
/// Meant to hint that a UUID may be an ordinary timestamp-plus-random UUIDv7
/// rather than a KepID. An empty result does not prove it is a KepID.
pub(crate) fn lint(id: KepId) -> Vec<Finding> {
    let now_us = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as i64);
    lint_at(id, now_us)
}

/// Like [`lint`], with an explicit "now" in Unix microseconds.
pub(crate) fn lint_at(id: KepId, now_unix_us: i64) -> Vec<Finding> {
    let mut out = Vec::new();

    if id.version() != 7 {
        out.push(finding(
            Severity::Error,
            "not-uuidv7",
            format!("version nibble is {}, expected 7", id.version()),
        ));
    }
    if id.variant() != 0b10 {
        out.push(finding(
            Severity::Error,
            "bad-variant",
            format!("variant bits are {:02b}, expected 10", id.variant()),
        ));
    }
    let fields = Fields::unpack(id);
    if fields.profile != 0 {
        out.push(finding(
            Severity::Error,
            "unsupported-profile",
            "profile bit (bit 90) is 1, which is reserved (a random UUIDv7 sets it half the time)".into(),
        ));
    }
    if !out.is_empty() {
        return out;
    }

    let el = crate::unchecked_elements(&fields);

    // Every decodable value is a closed orbit above the 250 km perigee floor,
    // so what is left are hints of physical implausibility.
    if fields.drag == 0 || u32::from(fields.drag) == DRAG.codes() - 1 {
        out.push(finding(
            Severity::Warning,
            "drag-saturated",
            format!("drag is at the saturation limit (B* = {:.3e}), a re-entering object or not a KepID", el.bstar),
        ));
    }

    let mean_altitude = (el.perigee_altitude_km() + el.apogee_altitude_km()) / 2.0;
    if el.bstar == 0.0 && mean_altitude < NO_DRAG_BELOW_KM {
        out.push(finding(
            Severity::Warning,
            "no-drag-at-low-altitude",
            format!("B* is 0 at a mean altitude of {mean_altitude:.0} km"),
        ));
    }
    if el.bstar > HIGH_BSTAR && mean_altitude > HIGH_DRAG_ABOVE_KM {
        out.push(finding(
            Severity::Warning,
            "high-drag-at-altitude",
            format!(
                "B* is {:.2e} at a mean altitude of {mean_altitude:.0} km",
                el.bstar
            ),
        ));
    }

    if el.epoch_unix_us > now_unix_us.saturating_add(FUTURE_SLACK_US) {
        out.push(finding(
            Severity::Warning,
            "epoch-in-future",
            format!("epoch {} is in the future", el.epoch_iso8601()),
        ));
    }

    if fields.inclination == 0 && fields.raan == 0 {
        out.push(finding(
            Severity::Info,
            "zero-plane",
            "inclination and RAAN are both exactly 0".into(),
        ));
    }

    out
}
