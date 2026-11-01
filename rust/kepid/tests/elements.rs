mod common;

use common::*;
use kepid::{check_range, decode, Elements, KepId};

#[test]
fn iss_derived_values() {
    let el = decode(ISS_ID.parse::<KepId>().unwrap()).unwrap();
    assert!(approx(el.semi_major_axis_km(), 6798.0, 2.0));
    assert!(approx(el.perigee_altitude_km(), 415.3, 0.5));
    assert!(approx(el.apogee_altitude_km(), 424.3, 0.5));
    assert!(approx(el.period_minutes(), 92.96, 0.01));
}

#[test]
fn apogee_is_never_below_perigee() {
    let x = Elements { eccentricity: 0.01, ..base() };
    assert!(x.apogee_altitude_km() > x.perigee_altitude_km());
    let c = Elements { eccentricity: 0.0, ..base() };
    assert_eq!(c.apogee_altitude_km(), c.perigee_altitude_km());
}

#[test]
fn mean_arg_latitude_wraps_into_a_turn() {
    let m = |argp: f64, ma: f64| Elements { arg_perigee: argp, mean_anomaly: ma, ..base() }.mean_arg_latitude();
    assert_eq!(m(10.0, 20.0), 30.0);
    assert_eq!(m(350.0, 20.0), 10.0);
    assert_eq!(m(180.0, 180.0), 0.0);
    assert_eq!(m(-10.0, 0.0), 350.0);
}

#[test]
fn mean_arg_latitude_stays_below_a_full_turn() {
    for tiny in [-1e-18, -1e-14, -0.0] {
        let m = Elements { arg_perigee: 0.0, mean_anomaly: tiny, ..base() }.mean_arg_latitude();
        assert!((0.0..360.0).contains(&m) && m.is_sign_positive(), "{tiny} gave {m}");
    }
}

#[test]
fn epoch_iso8601_of_the_iss_example() {
    assert_eq!(Elements { epoch_unix_us: ISS_EPOCH_US, ..base() }.epoch_iso8601(), "2026-10-27T04:14:31.680Z");
}

#[test]
fn check_range_accepts_the_base_case() {
    assert!(check_range(&base()).is_ok());
}

#[test]
fn the_perigee_floor_is_250_km() {
    let at = |h: f64| {
        let a = kepid::profile0::RE + h;
        let n = 86_400.0 / std::f64::consts::TAU * (kepid::profile0::MU / (a * a * a)).sqrt();
        Elements { mean_motion: n, eccentricity: 0.0, ..base() }
    };
    assert!(check_range(&at(250.001)).is_ok());
    assert!(check_range(&at(249.9)).is_err());
}
