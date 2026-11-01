use kepid::{encode, Elements, Fields, KepId};

use crate::lint::{lint_at, Finding, Severity};
use crate::{analyse, convert, detect, parse_args, Command, Format, Input, Source};

/// The spec's example: the ISS element set of `base`.
const ISS_ID: &str = "01a17b91-23e2-73ab-813c-86cddaf3a10c";
/// Its decoded epoch, the nearest 256 ms tick: 2026-10-27T04:14:31.680Z.
const ISS_EPOCH_US: i64 = 1_793_074_471_680_000;

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

/// The ISS element set of the spec's test vectors.
fn base() -> Elements {
    Elements {
        epoch_unix_us: 1_793_074_471_558_560,
        mean_motion: 15.49073103,
        eccentricity: 0.0006948,
        inclination: 51.6336,
        raan: 6.9404,
        arg_perigee: 314.9066,
        mean_anomaly: 45.1358,
        bstar: 0.00011132,
    }
}

/// Small deterministic generator, so sweeps are reproducible without a crate.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
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

const TLE: [&str; 2] = [
    "1 25544U 98067A   26305.20860139  .00006903  00000+0  13267-3 0  9994",
    "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592994",
];
const OMM: &str = r#"{ "EPOCH": "2026-11-01T05:00:23.160096", "MEAN_MOTION": 15.49138562, "ECCENTRICITY": 0.0007056, "INCLINATION": 51.6332, "RA_OF_ASC_NODE": 342.0408, "ARG_OF_PERICENTER": 334.5041, "MEAN_ANOMALY": 25.5599, "NORAD_CAT_ID": 25544, "BSTAR": 0.00013267359 }"#;
/// An hour after the epoch of `TODAY_ID`.
const NOW_TODAY: i64 = 1_793_509_223_168_000 + 3_600_000_000;
/// The KepID of `TLE` and `OMM`.
const TODAY_ID: &str = "01a1957a-ebef-73ab-bccf-878de5f3a90d";

fn args(list: &[&str]) -> Result<Command, String> {
    parse_args(&list.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

#[test]
fn arguments() {
    let run = |output, source| Ok(Command::Run { output, source });
    assert_eq!(args(&[TODAY_ID]), run(None, Source::Text(TODAY_ID.into())));
    assert_eq!(args(&["-"]), run(None, Source::Stdin));
    assert_eq!(args(&["-i", "tle.txt"]), run(None, Source::File("tle.txt".into())));
    assert_eq!(args(&["--input=tle.txt"]), run(None, Source::File("tle.txt".into())));
    assert_eq!(args(&["-o", "TLE", "-"]), run(Some(Format::Tle), Source::Stdin));
    assert_eq!(args(&["--output=omm", "x"]), run(Some(Format::Omm), Source::Text("x".into())));
    assert_eq!(args(&["-h"]), Ok(Command::Help));
    assert_eq!(args(&["--version", "x"]), Ok(Command::Version));
    assert!(args(&[]).unwrap_err().contains("no input"));
    assert!(args(&["a", "b"]).unwrap_err().contains("one input"));
    assert!(args(&["-", "-i", "f"]).unwrap_err().contains("one input"));
    assert!(args(&["-o", "xml", "x"]).unwrap_err().contains("unknown output format"));
    assert_eq!(args(&["-o", "json", "x"]), run(Some(Format::Json), Source::Text("x".into())));
    assert!(args(&["-o"]).unwrap_err().contains("needs a value"));
    assert!(args(&["-a", "x"]).unwrap_err().contains("unknown option"));
    assert!(args(&["--frobnicate", "x"]).unwrap_err().contains("unknown option"));
}

#[test]
fn detects_every_format() {
    assert!(matches!(detect(TODAY_ID), Ok(Input::KepId(_))));
    assert!(matches!(detect(&format!("  {TODAY_ID}\n")), Ok(Input::KepId(_))));
    let tle = format!("{}\n{}\n", TLE[0], TLE[1]);
    assert!(matches!(detect(&tle), Ok(Input::Tle { catalog: Some(25544), .. })));
    // A literal \n, as a shell passes it inside single quotes, and a name line.
    assert!(matches!(detect(&format!("{}\\n{}", TLE[0], TLE[1])), Ok(Input::Tle { .. })));
    assert!(matches!(detect(&format!("ISS (ZARYA)\r\n{}\r\n{}\r\n", TLE[0], TLE[1])), Ok(Input::Tle { .. })));
    assert!(matches!(detect(OMM), Ok(Input::Omm { .. })));
    assert!(matches!(detect(&format!("[{OMM}]")), Ok(Input::Omm { .. })));
    assert_eq!(codes_of(detect(&format!("[{OMM}, {OMM}]"))), ["several-omms"]);
    assert_eq!(codes_of(detect("{ not json")), ["invalid-json"]);
    assert_eq!(codes_of(detect("hello")), ["unknown-format"]);
    assert_eq!(codes_of(detect(r#"{"EPOCH": "2026-11-01T05:00:23"}"#)), ["invalid-omm"]);
}

#[test]
fn every_input_gives_the_same_kepid() {
    let tle = format!("{}\n{}", TLE[0], TLE[1]);
    for text in [TODAY_ID, tle.as_str(), OMM] {
        assert_eq!(convert(&detect(text).unwrap(), Format::KepId, false, NOW_TODAY).unwrap(), format!("{TODAY_ID}\n"));
    }
}

#[test]
fn conversions_go_through_the_kepid() {
    let tle_in = detect(&format!("{}\n{}", TLE[0], TLE[1])).unwrap();
    assert_eq!(
        convert(&tle_in, Format::Tle, false, NOW_TODAY).unwrap(),
        "1 25544U          26305.20860148  .00000000  00000+0  13155-3 0  9992\n\
         2 25544  51.6162 342.0483 0007474 334.2037  25.8607 15.49139459999994\n"
    );
    let from_id = convert(&detect(TODAY_ID).unwrap(), Format::Tle, false, NOW_TODAY).unwrap();
    assert!(from_id.starts_with("1 00000U "), "no catalog number: {from_id}");
    let omm = convert(&detect(OMM).unwrap(), Format::Omm, false, NOW_TODAY).unwrap();
    assert_eq!(
        omm,
        "{\n    \"EPOCH\": \"2026-11-01T05:00:23.168000\",\n    \"MEAN_MOTION\": 15.491394590138162,\n    \"ECCENTRICITY\": 0.0007474469871928292,\n    \"INCLINATION\": 51.61619462448406,\n    \"RA_OF_ASC_NODE\": 342.04833984375,\n    \"ARG_OF_PERICENTER\": 334.20374889489966,\n    \"MEAN_ANOMALY\": 25.860679448257883,\n    \"NORAD_CAT_ID\": 25544,\n    \"BSTAR\": 0.0001315454013845452\n}\n"
    );
    let parsed: kepid::Omm = serde_json::from_str(&omm).unwrap();
    assert_eq!(kepid::from_omm(&parsed).unwrap().to_string(), TODAY_ID);
}

#[test]
fn analysis_shows_the_input_as_given() {
    let text = analyse(&detect(&format!("{}\n{}", TLE[0], TLE[1])).unwrap(), false, NOW_TODAY).unwrap();
    assert!(text.starts_with("Input format: TLE (other formats inferred)\n\nKepID: 01a1957a-ebef-73ab-bccf-878de5f3a90d\n"), "{text}");
    assert!(text.contains(TLE[0]) && text.contains(TLE[1]), "the input TLE, unchanged");
    assert!(text.contains("\"EPOCH\": \"2026-11-01T05:00:23.168000\""), "the OMM, from the KepID");
    assert!(text.contains("Bit layout:\n┏"));
    let omm = analyse(&detect(OMM).unwrap(), false, NOW_TODAY).unwrap();
    assert!(omm.contains("\"BSTAR\": 0.00013267359"), "the input OMM, unchanged");
    assert!(omm.contains("1 25544U          26305.20860148"), "the TLE, from the KepID");
}

#[test]
fn analysis_reflects_complete_inputs() {
    // Every keyword of the input OMM, in its order, prettified.
    let celestrak = r#"[{"OBJECT_NAME":"ISS (ZARYA)","OBJECT_ID":"1998-067A","EPOCH":"2026-11-01T05:00:23.160096","MEAN_MOTION":15.49138562,"ECCENTRICITY":0.0007056,"INCLINATION":51.6332,"RA_OF_ASC_NODE":342.0408,"ARG_OF_PERICENTER":334.5041,"MEAN_ANOMALY":25.5599,"EPHEMERIS_TYPE":0,"CLASSIFICATION_TYPE":"U","NORAD_CAT_ID":25544,"ELEMENT_SET_NO":999,"REV_AT_EPOCH":59299,"BSTAR":0.00013267359,"MEAN_MOTION_DOT":6.903e-5,"MEAN_MOTION_DDOT":0}]"#;
    let text = analyse(&detect(celestrak).unwrap(), false, NOW_TODAY).unwrap();
    let start = text.find("OMM:\n").unwrap() + 5;
    let echoed = &text[start..text.find("\n\nPropagation (now):").unwrap()];
    assert!(echoed.starts_with("[\n    {\n        \"OBJECT_NAME\": \"ISS (ZARYA)\",\n        \"OBJECT_ID\": \"1998-067A\","), "{echoed}");
    assert!(echoed.contains("        \"REV_AT_EPOCH\": 59299,\n        \"BSTAR\": 0.00013267359,\n        \"MEAN_MOTION_DOT\": 0.00006903,"), "{echoed}");
    let back: serde_json::Value = serde_json::from_str(echoed).unwrap();
    assert_eq!(back, serde_json::from_str::<serde_json::Value>(celestrak).unwrap(), "the same JSON, only prettified");
    // The TLE with its name line.
    let tle = analyse(&detect(&format!("ISS (ZARYA)\n{}\n{}", TLE[0], TLE[1])).unwrap(), false, NOW_TODAY).unwrap();
    assert!(tle.contains(&format!("TLE:\nISS (ZARYA)\n{}\n{}\n", TLE[0], TLE[1])), "{tle}");
}

/// The bit layout card of a valid KepID.
fn card(id: KepId, color: bool) -> String {
    crate::card::card(id, &kepid::decode(id).unwrap(), color)
}

/// The check codes of a failed step.
fn codes_of<T: std::fmt::Debug>(result: Result<T, Vec<crate::Problem>>) -> Vec<&'static str> {
    result.unwrap_err().iter().map(|p| p.code).collect()
}

#[test]
fn errors_become_checks() {
    let analyse_text = |text: &str| detect(text).and_then(|input| analyse(&input, false, NOW_TODAY));
    // Not a KepID: only its marker errors, no other section.
    assert_eq!(codes_of(analyse_text("0192f3a1-7c4e-4b2d-9a41-5d3e8f1c2b60")), ["not-uuidv7", "unsupported-profile"]);
    // A TLE that cannot be read, and one that cannot be encoded.
    let bad_sum = format!("{}\n{}", TLE[0].replace("9994", "9995"), TLE[1]);
    assert_eq!(codes_of(analyse_text(&bad_sum)), ["tle-checksum"]);
    let low = format!("{}\n{}", TLE[0], "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 16.49138562592995");
    assert_eq!(codes_of(analyse_text(&low)), ["perigee-below-floor"]);
    // OMMs: a wrong theory, and an epoch before 1970.
    let xp = OMM.replacen('{', r#"{ "MEAN_ELEMENT_THEORY": "SGP4-XP","#, 1);
    assert_eq!(codes_of(analyse_text(&xp)), ["omm-header"]);
    assert_eq!(codes_of(analyse_text(&OMM.replace("2026-11-01", "1965-01-01"))), ["epoch-before-1970"]);
    // The same problems go through -o.
    assert_eq!(codes_of(detect(&bad_sum).and_then(|input| convert(&input, Format::Tle, false, NOW_TODAY))), ["tle-checksum"]);
    // The output: only the checks segment.
    let problems = analyse_text(&bad_sum).unwrap_err();
    let findings: Vec<_> = problems.iter().map(crate::Problem::finding).collect();
    assert_eq!(crate::checks(&findings, false), "Checks:\n  Error [tle-checksum] line 1 checksum is 5, expected 4\n");
}

#[test]
fn card_lines_line_up() {
    let text = card(id(TODAY_ID), false);
    let widths: Vec<usize> = text.lines().map(|l| l.chars().count()).collect();
    assert_eq!(widths.len(), 18);
    assert!(widths.iter().all(|&w| w == widths[0]), "{widths:?}\n{text}");
    for needle in ["┃ KepID             │ 0 (profile 0)", "┃ UUID format       │ UUID version 7 (RFC 9562)           ┃\n┠───────────────────┼─────────────────────────────────────┨\n┃ Epoch", "2026-11-01T05:00:23.168Z", "51.6162° (code", "342.0483°", "15.49139459 rev/day", "e 0.0007474", "B* 1.3155e-4", "┬", "┴", "┷", "┃ 01a1 957a │ 0000 0001  1010 0001   1001 0101  0111 1010 ┃"] {
        assert!(text.contains(needle), "missing {needle:?} in\n{text}");
    }
}

#[test]
fn card_colors_each_field() {
    let text = card(id(TODAY_ID), true);
    assert!(text.contains("\x1b[36mEpoch"), "{text}");
    assert!(text.contains("┃ KepID             │ 0 (profile 0)"), "the profile row has no color: {text}");
    // Bits 88-91: the last two of D (red), the profile bit (no color), the first of λ (green).
    assert!(text.contains("\x1b[31m1\x1b[0m\x1b[31m0\x1b[0m0\x1b[32m0\x1b[0m"), "{text}");
    assert!(text.contains("\x1b[2;37m0\x1b[0m\x1b[2;37m1\x1b[0m\x1b[2;37m1\x1b[0m\x1b[2;37m1\x1b[0m"), "the version bits are dim white");
    let plain: String = text.split("\x1b[").map(|part| part.split_once('m').map_or(part, |(_, rest)| rest)).collect();
    assert_eq!(plain, card(id(TODAY_ID), false));
}

#[test]
fn card_and_lint_never_panic() {
    let mut rng = Rng::new(99);
    for _ in 0..2000 {
        let v = (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64());
        let _ = lint_at(KepId::from_u128(v), NOW);
        // Any bits with valid markers are a valid KepID, and get a card.
        let valid = Fields { profile: 0, ..Fields::unpack(KepId::from_u128(v)) }.pack();
        assert!(!card(valid, true).is_empty());
    }
    let extreme = Fields { profile: 0, ..Fields::unpack(KepId::from_u128(u128::MAX)) }.pack();
    assert!(!card(extreme, false).is_empty());
    assert!(!card(Fields::unpack(KepId::from_u128(0)).pack(), false).is_empty());
}

#[test]
fn colors_headers_and_json() {
    let text = analyse(&detect(OMM).unwrap(), true, NOW_TODAY).unwrap();
    for header in ["Input format:", "KepID:", "TLE:", "OMM:", "Bit layout:"] {
        assert!(text.contains(&format!("\x1b[1m{header}\x1b[0m")), "{header} is not bold");
    }
    assert!(text.contains("\x1b[34m\"EPOCH\"\x1b[0m: \x1b[32m\"2026-11-01T05:00:23.160096\"\x1b[0m"), "{text}");
    assert!(text.contains("\x1b[34m\"BSTAR\"\x1b[0m: \x1b[33m0.00013267359\x1b[0m"), "{text}");
    let plain = analyse(&detect(OMM).unwrap(), false, NOW_TODAY).unwrap();
    assert!(!plain.contains('\x1b'));
    let stripped: String = text.split("\x1b[").map(|part| part.split_once('m').map_or(part, |(_, rest)| rest)).collect();
    assert_eq!(stripped, plain);
}

#[test]
fn propagation_matches_sgp4_and_the_iss() {
    use crate::propagation::propagate;
    let id = id(TODAY_ID);
    let at_epoch = propagate(id, 1_793_509_223_168_000).unwrap();
    // The ISS: about 51.6° inclination, 410-425 km up, about 7.66 km/s, 92.9 minutes.
    assert!(at_epoch.latitude.abs() <= 51.7, "{at_epoch:?}");
    assert!((405.0..430.0).contains(&at_epoch.altitude_km), "{at_epoch:?}");
    assert!((7.6..7.72).contains(&at_epoch.speed_km_s), "{at_epoch:?}");
    assert!((at_epoch.period_minutes - 92.954).abs() < 0.01, "{at_epoch:?}");
    // λ is 0.06° at the epoch: just past the ascending node, so near the equator, going north.
    assert!(at_epoch.latitude.abs() < 0.1, "{at_epoch:?}");
    let a_minute_later = propagate(id, 1_793_509_223_168_000 + 60_000_000).unwrap();
    assert!(a_minute_later.latitude > at_epoch.latitude + 3.0, "{a_minute_later:?}");
    let text = analyse(&detect(TODAY_ID).unwrap(), false, NOW_TODAY).unwrap();
    let section = &text[text.find("Propagation (now):\n").unwrap()..text.find("Bit layout:").unwrap()];
    for line in ["- Latitude: ", "\n- Longitude: ", "\n- Altitude: ", " km\n- Speed: ", " km/h or ", " m/s\n- Orbit duration: 92.95 minutes\n"] {
        assert!(section.contains(line), "missing {line:?} in\n{section}");
    }
}

#[test]
fn json_output_is_the_analysis() {
    use crate::analyse_json;
    let celestrak = format!("[{OMM}]");
    let json = analyse_json(&detect(&celestrak).unwrap(), 1_793_509_223_168_000).unwrap();
    let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["input", "kepid", "warning", "elements", "tle", "omm", "propagation", "checks"]);
    assert_eq!(json["input"], "OMM");
    assert_eq!(json["kepid"], TODAY_ID);
    assert_eq!(json["tle"][0], "1 25544U          26305.20860148  .00000000  00000+0  13155-3 0  9992");
    assert_eq!(json["tle"].as_array().unwrap().len(), 2);
    // The input OMM itself (out of its one-element array), every keyword kept.
    assert_eq!(json["omm"], serde_json::from_str::<serde_json::Value>(OMM).unwrap());
    let p = &json["propagation"];
    assert!((405_000.0..430_000.0).contains(&p["altitude"].as_f64().unwrap()), "altitude in m: {p}");
    assert!((7_600.0..7_720.0).contains(&p["speed"].as_f64().unwrap()), "speed in m/s: {p}");
    assert!((p["orbit_duration"].as_f64().unwrap() - 5577.2).abs() < 1.0, "orbit duration in s: {p}");
    assert!(p["latitude"].as_f64().unwrap().abs() < 0.1 && p["longitude"].is_f64());
    assert_eq!(json["checks"], serde_json::json!([]));
    assert!(json["warning"].is_null(), "a fresh epoch has no warning");
    // A TLE input keeps its own two lines, without the name line.
    let tle = analyse_json(&detect(&format!("ISS\n{}\n{}", TLE[0], TLE[1])).unwrap(), NOW_TODAY).unwrap();
    assert_eq!(tle["tle"], serde_json::json!([TLE[0], TLE[1]]));
    assert_eq!(tle["omm"]["EPOCH"], "2026-11-01T05:00:23.168000");
}

#[test]
fn old_epochs_get_a_warning() {
    use crate::sparse_age;
    const DAY: i64 = 86_400_000_000;
    assert_eq!(sparse_age(8 * DAY), "1 week");
    assert_eq!(sparse_age(30 * 7 * DAY), "6 months");
    assert_eq!(sparse_age(59 * DAY), "1 month");
    assert_eq!(sparse_age(400 * DAY), "1 year");
    assert_eq!(sparse_age(800 * DAY), "2 years");
    assert_eq!(sparse_age(20 * DAY), "2 weeks");

    let epoch = 1_793_509_223_168_000;
    let at = |now| analyse(&detect(TODAY_ID).unwrap(), false, now).unwrap();
    assert!(!at(epoch + 7 * DAY).contains("Warning:"), "exactly a week is fine");
    assert!(!at(epoch - 30 * DAY).contains("Warning:"), "no age warning for a future epoch");
    let text = at(epoch + 3 * 30 * DAY);
    assert!(text.contains("KepID: 01a1957a-ebef-73ab-bccf-878de5f3a90d\n\nWarning: epoch is 2 months old!\n\nTLE:"), "{text}");
    let colored = analyse(&detect(TODAY_ID).unwrap(), true, epoch + 3 * 30 * DAY).unwrap();
    assert!(colored.contains("\x1b[1;33mWarning:\x1b[0m\x1b[33m epoch is 2 months old!\x1b[0m"), "{colored}");
}

#[test]
fn json_warning_for_old_epochs() {
    let json = crate::analyse_json(&detect(TODAY_ID).unwrap(), 1_793_509_223_168_000 + 400 * 86_400_000_000).unwrap();
    assert_eq!(json["warning"], "epoch is 1 year old");
}

#[test]
fn json_elements_raw_and_parsed() {
    let json = crate::analyse_json(&detect(TODAY_ID).unwrap(), NOW_TODAY).unwrap();
    let (raw, parsed) = (&json["elements"]["raw"], &json["elements"]["parsed"]);
    let keys = ["profile", "epoch", "arg_perigee", "inclination", "raan", "drag", "lambda", "mean_motion", "eccentricity"];
    assert_eq!(raw.as_object().unwrap().keys().collect::<Vec<_>>(), keys);
    assert_eq!(parsed.as_object().unwrap().keys().collect::<Vec<_>>(), keys);
    // The same codes as the bit layout card.
    assert_eq!(raw, &serde_json::json!({"profile": 0, "epoch": 7005895403u64, "arg_perigee": 239, "inclination": 939, "raan": 15567, "drag": 542, "lambda": 3557, "mean_motion": 124754, "eccentricity": 13}));
    assert_eq!(parsed["epoch"], 1_793_509_223_168_000i64);
    assert_eq!(parsed["raan"], 342.04833984375);
    assert!((parsed["inclination"].as_f64().unwrap() - 51.61619462448406).abs() < 1e-12);
    // w and s, the coded variables: both in (0, 1].
    let (w, s) = (parsed["mean_motion"].as_f64().unwrap(), parsed["eccentricity"].as_f64().unwrap());
    assert!((0.9..1.0).contains(&w) && (0.0..0.1).contains(&s), "{parsed}");
    let decoded = kepid::decode(id(TODAY_ID)).unwrap();
    assert!(((1.0 - w) * s - decoded.eccentricity).abs() < 1e-15, "e = (1 - w) s");
}

#[test]
fn checks_use_the_same_clock_as_the_analysis() {
    let epoch = 1_793_509_223_168_000;
    let at = |now| analyse(&detect(TODAY_ID).unwrap(), false, now).unwrap();
    assert!(at(epoch - 2 * 86_400_000_000).contains("epoch-in-future"), "two days before the epoch");
    assert!(!at(epoch).contains("epoch-in-future"));
    let json = crate::analyse_json(&detect(TODAY_ID).unwrap(), epoch - 2 * 86_400_000_000).unwrap();
    assert!(json["checks"][0].as_str().unwrap().contains("epoch-in-future"), "{json}");
}
