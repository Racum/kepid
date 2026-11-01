mod common;

use common::*;
use kepid::{encode, lint_at, Elements, Fields, Finding, KepId, Severity};

const NOW: i64 = ISS_EPOCH_US + 3_600_000_000;

fn id(s: &str) -> KepId {
    s.parse().unwrap()
}

fn lint_elements(x: Elements) -> Vec<Finding> {
    lint_at(encode(&x).unwrap(), NOW)
}

fn codes(findings: &[Finding]) -> Vec<&'static str> {
    findings.iter().map(|f| f.code).collect()
}

#[test]
fn iss_is_clean() {
    assert_eq!(lint_at(id(ISS_ID), NOW), vec![]);
}

#[test]
fn wrong_version_and_variant_are_errors() {
    let found = lint_at(id("0192f3a1-7c4e-4b2d-4a41-5d3e8f1c2b60"), NOW);
    // This UUID also has bit 90 set, so all three marker checks fire.
    assert_eq!(codes(&found), ["not-uuidv7", "bad-variant", "unsupported-profile"]);
    assert!(found.iter().all(|f| f.severity == Severity::Error));
}

#[test]
fn reserved_profile_is_an_error_and_stops_the_checks() {
    let f = Fields { profile: 1, drag: 1023, ..Fields::unpack(id(ISS_ID)) };
    let found = lint_at(f.pack(), NOW);
    assert_eq!(codes(&found), ["unsupported-profile"]);
    assert_eq!(found[0].severity, Severity::Error);
}

#[test]
fn saturated_drag_is_flagged() {
    for bstar in [0.5, -0.5] {
        let found = lint_elements(Elements { bstar, ..base() });
        assert_eq!(codes(&found), ["drag-saturated"], "B* {bstar}");
        assert_eq!(found[0].severity, Severity::Warning);
    }
}

#[test]
fn zero_drag_at_low_altitude_is_flagged() {
    let found = lint_elements(Elements { bstar: 0.0, ..base() });
    assert_eq!(codes(&found), ["no-drag-at-low-altitude"]);
}

#[test]
fn zero_drag_at_high_altitude_is_fine() {
    assert!(lint_elements(Elements { mean_motion: 13.0, bstar: 0.0, ..base() }).is_empty());
}

#[test]
fn high_drag_at_high_altitude_is_flagged() {
    let found = lint_elements(Elements { mean_motion: 13.5, bstar: 1e-2, ..base() });
    assert_eq!(codes(&found), ["high-drag-at-altitude"]);
}

#[test]
fn high_drag_at_low_altitude_is_fine() {
    assert!(lint_elements(Elements { bstar: 5e-3, ..base() }).is_empty());
}

#[test]
fn epoch_in_the_future_has_a_one_day_slack() {
    let x = encode(&base()).unwrap();
    let epoch = x.epoch_unix_us();
    assert!(lint_at(x, epoch).is_empty());
    assert!(lint_at(x, epoch - 86_400_000_000).is_empty(), "exactly one day ahead");
    let found = lint_at(x, epoch - 86_400_000_000 - 1);
    assert_eq!(codes(&found), ["epoch-in-future"]);
}

#[test]
fn zero_inclination_and_raan_is_informational() {
    let f = Fields { inclination: 0, raan: 0, ..Fields::unpack(id(ISS_ID)) };
    let found = lint_at(f.pack(), NOW);
    assert_eq!(codes(&found), ["zero-plane"]);
    assert_eq!(found[0].severity, Severity::Info);
    let only_one = Fields { inclination: 0, raan: 5, ..Fields::unpack(id(ISS_ID)) };
    assert!(lint_at(only_one.pack(), NOW).is_empty());
}

#[test]
fn severities_are_ordered() {
    assert!(Severity::Info < Severity::Warning && Severity::Warning < Severity::Error);
}

#[test]
fn random_v7_uuids_are_often_flagged() {
    let mut rng = Rng::new(7);
    let mut flagged = 0;
    let total = 2000;
    for _ in 0..total {
        let ts = 0x0190_0000_0000u128 + u128::from(rng.next_u64() % 0x0010_0000_0000);
        let rand = (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64());
        let v = (ts << 80) | (0x7 << 76) | ((rand >> 64 & 0xFFF) << 64) | (0b10 << 62) | (rand & ((1 << 62) - 1));
        if !lint_at(KepId::from_u128(v), NOW).is_empty() {
            flagged += 1;
        }
    }
    // Measured ~67%: every value with the right markers is a valid orbit, so only
    // the profile bit (half of them) and implausible drag catch random UUIDv7.
    assert!(flagged * 100 > total * 50, "only {flagged} of {total} random UUIDv7 were flagged");
}

#[test]
fn display_mentions_severity_and_code() {
    let found = lint_at(id("0192f3a1-7c4e-4b2d-9a41-5d3e8f1c2b60"), NOW);
    let text = found[0].to_string();
    assert!(text.contains("Error") && text.contains("not-uuidv7"), "{text}");
}

#[test]
fn explain_describes_every_field() {
    let text = kepid::explain(id(ISS_ID));
    for needle in [ISS_ID, "2026-10-27T04:14:31.680Z", "profile 0", "51.6162 deg", "n = 15.49074945 rev/day", "e = 0.0006626", "B* = 1.1417e-4", "Ω = 6.9434 deg", "perigee"] {
        assert!(text.contains(needle), "missing {needle:?} in\n{text}");
    }
}

#[test]
fn explain_never_panics_on_arbitrary_bits() {
    let mut rng = Rng::new(99);
    for v in [0u128, u128::MAX] {
        assert!(!kepid::explain(KepId::from_u128(v)).is_empty());
    }
    for _ in 0..2000 {
        let v = (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64());
        let _ = kepid::explain(KepId::from_u128(v));
        let _ = lint_at(KepId::from_u128(v), NOW);
    }
}
