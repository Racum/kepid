//! The conformance vectors of `docs/spec.md`, section 10.

use kepid::{decode, encode, Elements, KepId};

const VECTORS: &str = include_str!("../../../docs/profile0_vectors.csv");

struct Row<'a> {
    case: &'a str,
    input: Elements,
    expected: &'a str,
    decoded: Option<Elements>,
}

fn elements(c: &[&str]) -> Elements {
    let f = |i: usize| -> f64 { c[i].parse().unwrap_or_else(|_| panic!("bad number {:?}", c[i])) };
    Elements {
        epoch_unix_us: c[0].parse().unwrap_or_else(|_| panic!("bad epoch {:?}", c[0])),
        mean_motion: f(1),
        eccentricity: f(2),
        inclination: f(3),
        raan: f(4),
        arg_perigee: f(5),
        mean_anomaly: f(6),
        bstar: f(7),
    }
}

fn rows() -> Vec<Row<'static>> {
    VECTORS
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let c: Vec<&str> = line.split(',').collect();
            assert_eq!(c.len(), 19, "{line}");
            let refused = c[10].starts_with("REFUSED");
            Row {
                case: c[0],
                input: elements(&c[2..10]),
                expected: c[10],
                decoded: (!refused).then(|| elements(&c[11..19])),
            }
        })
        .collect()
}

fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1e-300) || (a - b).abs() <= 1e-15
}

#[test]
fn there_are_vectors() {
    assert!(rows().len() >= 15);
}

#[test]
fn every_vector_encodes_to_its_kepid() {
    for r in rows() {
        match encode(&r.input) {
            Ok(id) => assert_eq!(id.to_string(), r.expected, "{}", r.case),
            Err(e) => assert!(r.expected.starts_with("REFUSED"), "{}: refused with {e}, expected {}", r.case, r.expected),
        }
    }
}

#[test]
fn every_vector_decodes_to_its_elements() {
    for r in rows() {
        let Some(want) = r.decoded else { continue };
        let got = decode(r.expected.parse::<KepId>().unwrap()).unwrap();
        assert_eq!(got.epoch_unix_us, want.epoch_unix_us, "{}: epoch", r.case);
        let pairs = [
            ("n", got.mean_motion, want.mean_motion),
            ("e", got.eccentricity, want.eccentricity),
            ("i", got.inclination, want.inclination),
            ("raan", got.raan, want.raan),
            ("argp", got.arg_perigee, want.arg_perigee),
            ("M", got.mean_anomaly, want.mean_anomaly),
            ("bstar", got.bstar, want.bstar),
        ];
        for (name, g, w) in pairs {
            assert!(close(g, w), "{}: {name} decoded {g}, expected {w}", r.case);
        }
    }
}

#[test]
fn every_decoded_vector_encodes_back_to_its_kepid() {
    for r in rows() {
        if let Some(d) = r.decoded {
            assert_eq!(encode(&d).unwrap().to_string(), r.expected, "{}", r.case);
        }
    }
}
