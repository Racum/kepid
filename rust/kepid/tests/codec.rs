mod common;

use common::*;
use kepid::profile0::*;
use kepid::{decode, encode, DecodeError, Elements, EncodeError, Fields, KepId};

fn id(s: &str) -> KepId {
    s.parse().unwrap()
}

fn fields_of(el: &Elements) -> Fields {
    Fields::unpack(encode(el).unwrap())
}

// ---- happy paths -----------------------------------------------------------

#[test]
fn iss_encodes_to_the_spec_example() {
    assert_eq!(encode(&base()).unwrap(), id(ISS_ID));
}

#[test]
fn iss_decodes_to_reference_values() {
    let el = decode(id(ISS_ID)).unwrap();
    assert_eq!(el.epoch_unix_us, ISS_EPOCH_US);
    assert!(approx(el.mean_motion, 15.490749447697493, 1e-12));
    assert!(approx(el.eccentricity, 0.0006625520740656849, 1e-15));
    assert!(approx(el.inclination, 51.61619462448406, 1e-12));
    assert!(approx(el.raan, 6.943359375, 1e-12));
    assert!(approx(el.arg_perigee, 314.76816165016845, 1e-12));
    assert!(approx(el.mean_anomaly, 45.281856203811856, 1e-12));
    assert!(approx(el.bstar, 0.00011417386721999137, 1e-15));
}

#[test]
fn iss_reencodes_to_same_id() {
    let el = decode(id(ISS_ID)).unwrap();
    assert_eq!(encode(&el).unwrap(), id(ISS_ID));
}

#[test]
fn encode_sets_markers_and_profile() {
    let k = encode(&base()).unwrap();
    assert_eq!(k.version(), 7);
    assert_eq!(k.variant(), 0b10);
    assert_eq!(k.profile(), 0);
    assert_eq!(Fields::unpack(k).profile, 0);
    assert_eq!(k.validate(), Ok(()));
}

/// The decoded value lies inside the code cell of the input: within one step,
/// measured at either end (the step changes along the curve).
fn within(map: &kepid::curve::FieldMap, x: f64, q: f64, d: f64) -> bool {
    d <= map.step(x).max(map.step(q)) + 1e-9 * x.abs().max(1.0)
}

#[test]
fn decode_stays_within_a_step_of_the_input() {
    let mut rng = Rng::new(0x9e37_79b9_7f4a_7c15);
    for _ in 0..5000 {
        let x = rng.elements();
        let q = decode(encode(&x).unwrap()).unwrap();
        let (w, qw) = (kepid::w_of_mean_motion(x.mean_motion), kepid::w_of_mean_motion(q.mean_motion));
        assert!(within(&MEAN_MOTION, w, qw, (qw - w).abs()), "n {} -> {}", x.mean_motion, q.mean_motion);
        assert!(within(&INCLINATION, x.inclination, q.inclination, (q.inclination - x.inclination).abs()), "i {} -> {}", x.inclination, q.inclination);
        assert!(within(&RAAN, 0.0, 0.0, angle_diff(q.raan, x.raan)), "raan {} -> {}", x.raan, q.raan);
        let (s, qs) = (x.eccentricity / (1.0 - qw), q.eccentricity / (1.0 - qw));
        assert!(within(&ECCENTRICITY, s, qs, (qs - s).abs()), "e {} -> {}", x.eccentricity, q.eccentricity);
        let argp = x.arg_perigee.rem_euclid(360.0);
        assert!(within(&ARG_PERIGEE, argp, q.arg_perigee, angle_diff(q.arg_perigee, argp)), "argp {} -> {}", argp, q.arg_perigee);
        // λ also moves by the satellite's motion over the epoch rounding (at most 128 ms).
        let lam = (x.arg_perigee + x.mean_anomaly).rem_euclid(360.0);
        let shift = 360.0 * x.mean_motion * 128.0 / 86_400_000.0;
        let ql = q.mean_arg_latitude();
        assert!(within(&LAMBDA, lam, ql, (angle_diff(ql, lam) - shift).max(0.0)), "lambda {lam} -> {ql}");
        assert!((q.epoch_unix_us - x.epoch_unix_us).abs() <= TICK_US / 2);
    }
}

#[test]
fn encoding_is_idempotent_after_a_round_trip() {
    let mut rng = Rng::new(42);
    for _ in 0..5000 {
        let first = encode(&rng.elements()).unwrap();
        let again = encode(&decode(first).unwrap()).unwrap();
        assert_eq!(first, again);
    }
}

#[test]
fn every_value_with_valid_markers_decodes_to_a_valid_orbit_and_reencodes() {
    let mut rng = Rng::new(0x5eed);
    for _ in 0..20_000 {
        let k = KepId::from_u128(rng.kepid_bits());
        let el = decode(k).unwrap();
        assert!(el.mean_motion > 0.0 && (0.0..1.0).contains(&el.eccentricity), "{k}: {el:?}");
        assert!(el.perigee_altitude_km() >= PERIGEE_FLOOR_KM - 1e-6, "{k}: perigee {}", el.perigee_altitude_km());
        assert!((0.0..=180.0).contains(&el.inclination));
        for a in [el.raan, el.arg_perigee, el.mean_anomaly] {
            assert!((0.0..360.0).contains(&a), "{k}: angle {a}");
        }
        assert_eq!(encode(&el).unwrap(), k);
    }
}

#[test]
fn moving_the_epoch_within_a_tick_moves_lambda_with_the_satellite() {
    // The same orbit, stated 100 ms later with the mean anomaly advanced, encodes identically.
    let x = Elements { epoch_unix_us: ISS_EPOCH_US, ..base() };
    let dt = 100_000;
    let later = Elements {
        epoch_unix_us: ISS_EPOCH_US + dt,
        mean_anomaly: x.mean_anomaly + 360.0 * x.mean_motion * dt as f64 / 86_400_000_000.0,
        ..x
    };
    assert_eq!(encode(&x).unwrap(), encode(&later).unwrap());
}

// ---- exhaustive sweeps over each field --------------------------------------

/// Packs `f`, decodes, re-encodes and compares the raw fields.
fn assert_stable(f: Fields) {
    let again = Fields::unpack(encode(&decode(f.pack()).unwrap()).unwrap());
    assert_eq!(again, f);
}

fn iss_fields() -> Fields {
    Fields::unpack(id(ISS_ID))
}

#[test]
fn every_arg_perigee_code_is_stable() {
    for code in 0..=255u8 {
        assert_stable(Fields { arg_perigee: code, ..iss_fields() });
    }
}

#[test]
fn every_inclination_is_stable() {
    for raw in 0..4096u16 {
        assert_stable(Fields { inclination: raw, ..iss_fields() });
    }
}

#[test]
fn every_raan_is_stable() {
    for raw in 0..16_384u16 {
        assert_stable(Fields { raan: raw, ..iss_fields() });
    }
}

#[test]
fn every_drag_code_is_stable() {
    for raw in 0..1024u16 {
        assert_stable(Fields { drag: raw, ..iss_fields() });
    }
}

#[test]
fn every_lambda_is_stable() {
    for raw in 0..8192u16 {
        assert_stable(Fields { lambda: raw, ..iss_fields() });
    }
}

#[test]
fn every_mean_motion_is_stable() {
    for raw in 0..131_072u32 {
        assert_stable(Fields { mean_motion: raw, ..iss_fields() });
    }
}

#[test]
fn every_eccentricity_is_stable_at_several_altitudes() {
    for mean_motion in [0, 30_000, 90_000, 120_000, 131_071] {
        for raw in 0..128u8 {
            assert_stable(Fields { eccentricity: raw, mean_motion, ..iss_fields() });
        }
    }
}

#[test]
fn epoch_extremes_are_stable() {
    for ticks in [0, 1, 2, 1 << 20, MAX_TICKS - 1, MAX_TICKS] {
        assert_stable(Fields { epoch_ticks: ticks, ..iss_fields() });
    }
}

// ---- epoch rounding ----------------------------------------------------------

fn ticks_of(epoch_unix_us: i64) -> u64 {
    fields_of(&Elements { epoch_unix_us, ..base() }).epoch_ticks
}

#[test]
fn epoch_rounds_to_the_nearest_tick() {
    assert_eq!(ticks_of(1000 * TICK_US), 1000);
    assert_eq!(ticks_of(1000 * TICK_US + 127_999), 1000);
    assert_eq!(ticks_of(1000 * TICK_US + 128_000), 1001); // halves round up
    assert_eq!(ticks_of(1000 * TICK_US - 128_000), 1000);
    assert_eq!(ticks_of(1000 * TICK_US - 128_001), 999);
}

#[test]
fn half_tick_tle_epochs_round_up() {
    // The four catalog element sets whose TLE epoch is exactly half a tick off.
    for (field, tick) in [
        ("26300.03268000", 7_004_148_530),
        ("26300.19244000", 7_004_202_449),
        ("26298.88404000", 7_003_760_864),
        ("26300.31596000", 7_004_244_137),
    ] {
        let us = kepid::tle_epoch_unix_us(field).unwrap();
        assert_eq!(us % TICK_US, TICK_US / 2, "{field}");
        assert_eq!(ticks_of(us), tick, "{field}");
    }
}

#[test]
fn epoch_range() {
    assert_eq!(ticks_of(0), 0);
    assert_eq!(rejects(Elements { epoch_unix_us: -1, ..base() }), EncodeError::EpochBeforeUnixEpoch);
    assert_eq!(ticks_of(MAX_TICKS as i64 * TICK_US), MAX_TICKS);
    assert_eq!(ticks_of(MAX_TICKS as i64 * TICK_US + 127_999), MAX_TICKS);
    let past = MAX_TICKS as i64 * TICK_US + 128_000;
    assert_eq!(rejects(Elements { epoch_unix_us: past, ..base() }), EncodeError::EpochAfterLastTick(past));
    assert!(matches!(rejects(Elements { epoch_unix_us: i64::MAX, ..base() }), EncodeError::EpochAfterLastTick(_)));
}

// ---- boundaries and validation ---------------------------------------------

fn rejects(x: Elements) -> EncodeError {
    encode(&x).expect_err("should be rejected")
}

#[test]
fn mean_motion_must_be_positive() {
    assert!(matches!(rejects(Elements { mean_motion: 0.0, ..base() }), EncodeError::MeanMotionNotPositive(_)));
    assert!(matches!(rejects(Elements { mean_motion: -1.0, ..base() }), EncodeError::MeanMotionNotPositive(_)));
    // Far beyond GEO still encodes.
    let far = Elements { mean_motion: 0.05, eccentricity: 0.5, ..base() };
    assert!(approx(decode(encode(&far).unwrap()).unwrap().mean_motion, 0.05, 1e-3));
}

#[test]
fn eccentricity_boundaries() {
    assert!(matches!(rejects(Elements { eccentricity: -1e-12, ..base() }), EncodeError::EccentricityOutOfRange(_)));
    assert!(matches!(rejects(Elements { eccentricity: 1.0, mean_motion: 0.5, ..base() }), EncodeError::EccentricityOutOfRange(_)));
    assert!(encode(&Elements { eccentricity: 0.0, ..base() }).is_ok());
    // 12-hour highly elliptical: high e with the perigee above the floor.
    assert!(encode(&Elements { eccentricity: 0.74, mean_motion: 2.006, ..base() }).is_ok());
}

#[test]
fn inclination_boundaries() {
    assert!(matches!(rejects(Elements { inclination: -1e-9, ..base() }), EncodeError::InclinationOutOfRange(_)));
    assert!(matches!(rejects(Elements { inclination: 180.0 + 1e-9, ..base() }), EncodeError::InclinationOutOfRange(_)));
    assert_eq!(fields_of(&Elements { inclination: 0.0, ..base() }).inclination, 0);
    assert_eq!(fields_of(&Elements { inclination: 180.0, ..base() }).inclination, 4095);
}

#[test]
fn perigee_floor() {
    let at = |e: f64| Elements { mean_motion: 16.0, eccentricity: e, ..base() };
    let limit = 1.0 - kepid::w_of_mean_motion(16.0);
    assert!(encode(&at(limit * (1.0 - 1e-6))).is_ok());
    assert!(matches!(rejects(at(limit * (1.0 + 1e-6))), EncodeError::PerigeeBelowFloor(_)));
    assert!(matches!(rejects(Elements { mean_motion: 16.2, eccentricity: 0.0, ..base() }), EncodeError::PerigeeBelowFloor(_)));
}

#[test]
fn non_finite_values_are_rejected() {
    let fields: [fn(&mut Elements) -> &mut f64; 7] = [
        |x| &mut x.mean_motion,
        |x| &mut x.eccentricity,
        |x| &mut x.inclination,
        |x| &mut x.raan,
        |x| &mut x.arg_perigee,
        |x| &mut x.mean_anomaly,
        |x| &mut x.bstar,
    ];
    for field in fields {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut x = base();
            *field(&mut x) = bad;
            assert!(matches!(rejects(x), EncodeError::NotFinite(_)));
        }
    }
}

#[test]
fn drag_keeps_its_sign_and_zero_is_exact() {
    let q = |bstar: f64| decode(encode(&Elements { bstar, ..base() }).unwrap()).unwrap().bstar;
    assert_eq!(q(0.0), 0.0);
    assert_eq!(fields_of(&Elements { bstar: 0.0, ..base() }).drag, 512);
    assert!(q(-2e-4) < 0.0);
    assert!(approx(q(-2e-4), -2e-4, 2e-5));
    assert!(approx(q(5e-4), 5e-4, 5e-5));
}

#[test]
fn drag_saturates_instead_of_failing() {
    assert_eq!(fields_of(&Elements { bstar: 0.5, ..base() }).drag, 1023);
    assert_eq!(fields_of(&Elements { bstar: -0.5, ..base() }).drag, 0);
}

#[test]
fn small_drag_at_high_altitude_decodes_as_zero() {
    let geo = Elements { mean_motion: 1.0027, eccentricity: 3e-4, bstar: 1e-4, ..base() };
    assert_eq!(decode(encode(&geo).unwrap()).unwrap().bstar, 0.0);
}

#[test]
fn angles_wrap() {
    let raan = |r: f64| fields_of(&Elements { raan: r, ..base() }).raan;
    assert_eq!(raan(360.0), raan(0.0));
    assert_eq!(raan(-10.0), raan(350.0));
    assert_eq!(raan(359.99999), 0);
    let argp = |a: f64| fields_of(&Elements { arg_perigee: a, ..base() }).arg_perigee;
    assert_eq!(argp(-45.0), argp(315.0));
    let lam = |m: f64| fields_of(&Elements { mean_anomaly: m, ..base() }).lambda;
    assert_eq!(lam(45.1358 + 360.0), lam(45.1358));
}

#[test]
fn decoded_angles_stay_below_a_full_turn() {
    let mut rng = Rng::new(3);
    for _ in 0..2000 {
        let el = decode(KepId::from_u128(rng.kepid_bits())).unwrap();
        for a in [el.raan, el.arg_perigee, el.mean_anomaly] {
            assert!((0.0..360.0).contains(&a) && a.is_sign_positive(), "{a}");
        }
    }
}

// ---- decode refusals -------------------------------------------------------

#[test]
fn decode_rejects_wrong_markers_and_reserved_profile() {
    let iss = id(ISS_ID).as_u128();
    assert_eq!(decode(KepId::from_u128(iss & !(0xF << 76) | (0x4 << 76))), Err(DecodeError::NotUuidV7(4)));
    assert_eq!(decode(KepId::from_u128(iss & !(0b11 << 62))), Err(DecodeError::BadVariant(0)));
    assert_eq!(decode(KepId::from_u128(iss | (1 << 37))), Err(DecodeError::UnsupportedProfile(1)));
}

// ---- constants and curves --------------------------------------------------

#[test]
fn derived_constants_match_their_definitions() {
    let tau = std::f64::consts::TAU;
    assert_eq!(R_FLOOR, 6628.137);
    assert_eq!(N_FLOOR, 86_400.0 / tau * (MU / (R_FLOOR * R_FLOOR * R_FLOOR)).sqrt());
    assert_eq!(C_SGP4, R_FLOOR / 6378.135);
    assert_eq!(S_SGP4, 1.0 + 78.0 / 6378.135);
    assert_eq!(kepid::drag_factor(R_FLOOR / (RE + 500.0), 0.0), 1.0);
    assert!(approx(kepid::w_of_mean_motion(N_FLOOR), 1.0, 1e-15));
}

#[test]
fn curves_are_monotonic_and_invert_exactly() {
    for map in [ARG_PERIGEE, INCLINATION, RAAN, DRAG, LAMBDA, MEAN_MOTION, ECCENTRICITY] {
        assert_eq!(kepid::curve::forward(map.layers, 0.0), 0.0);
        assert!(approx(kepid::curve::forward(map.layers, 1.0), 1.0, 1e-12));
        let mut prev = -1.0;
        for j in 0..=10_000 {
            let z = f64::from(j) / 10_000.0;
            let t = kepid::curve::forward(map.layers, z);
            assert!(t >= prev, "not monotonic at {z}");
            prev = t;
            let back = kepid::curve::inverse(map.layers, t);
            assert!(approx(kepid::curve::forward(map.layers, back), t, 1e-12), "inverse off at {z}");
        }
    }
}
