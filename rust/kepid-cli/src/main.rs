mod card;
mod lint;
mod propagation;
#[cfg(test)]
mod tests;

use std::io::{IsTerminal, Read};
use std::process::ExitCode;

use kepid::{EncodeError, Fields, KepId, Omm, OmmError, TleError};

use lint::{Finding, Severity};

const USAGE: &str = "\
usage: kepid [options] <input>
       kepid [options] -i <file>
       kepid [options] -

The input is a KepID, a TLE (two lines; a literal \\n between them works too)
or an OMM (JSON); the format is detected. Use - to read standard input.

With no options, it shows the input in every format, with its propagation and
bit layout.

options:
  -o, --output <format>  print only this format: kepid, tle or omm, converted
                         through the KepID; or json, the whole analysis as
                         JSON (without the bit layout)
  -i, --input <file>     read the input from a file
  -h, --help             show this help
  -v, --version          show the version

The bit layout is in color on a terminal; set NO_COLOR to turn that off, or
FORCE_COLOR to keep it when piping (for example, into less -R).

Exit status: 0 when there are no errors (warnings included), 1 otherwise.";

/// The formats the CLI can print with `--output`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    KepId,
    Tle,
    Omm,
    /// The whole analysis, as JSON.
    Json,
}

/// Where the input comes from.
#[derive(Debug, PartialEq, Eq)]
enum Source {
    Text(String),
    Stdin,
    File(String),
}

/// What to do.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Version,
    Run { output: Option<Format>, source: Source },
}

/// A detected input, as given.
#[derive(Debug)]
enum Input {
    KepId(KepId),
    /// The two lines, and the name line before them, if any.
    Tle { name: Option<String>, lines: [String; 2], catalog: Option<u32> },
    /// The OMM, and the JSON it came from, keys in their original order.
    Omm { omm: Box<Omm>, json: serde_json::Value },
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (output, source) = match parse_args(&args) {
        Ok(Command::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Command::Version) => {
            println!("kepid {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Ok(Command::Run { output, source }) => (output, source),
        Err(err) => {
            eprintln!("kepid: {err}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    // Colors on a terminal, or anywhere with FORCE_COLOR; NO_COLOR always wins.
    let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
    let color_on = |terminal: bool| !set("NO_COLOR") && (terminal || set("FORCE_COLOR"));
    let color = color_on(std::io::stdout().is_terminal());
    let now_us = now_unix_us();
    let result = read(source).and_then(|text| detect(&text)).and_then(|input| match output {
        Some(format) => convert(&input, format, color, now_us),
        None => analyse(&input, color, now_us),
    });
    match result {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        // An analysis that fails shows only its checks, with the errors.
        Err(problems) if output == Some(Format::Json) => {
            let checks: Vec<serde_json::Value> = problems.iter().map(|p| p.finding().to_string().into()).collect();
            println!("{}", pretty(&serde_json::json!({ "checks": checks }), 0, color));
            ExitCode::FAILURE
        }
        Err(problems) if output.is_none() => {
            let findings: Vec<Finding> = problems.iter().map(Problem::finding).collect();
            eprint!("{}", checks(&findings, color_on(std::io::stderr().is_terminal())));
            ExitCode::FAILURE
        }
        Err(problems) => {
            for problem in &problems {
                eprintln!("kepid: {}", problem.message);
            }
            ExitCode::FAILURE
        }
    }
}

/// Why the input cannot be shown: a check code and a message. Any problem
/// exits with 1; a run with none exits with 0, warnings included.
#[derive(Debug)]
struct Problem {
    code: &'static str,
    message: String,
}

impl Problem {
    fn new(code: &'static str, message: impl Into<String>) -> Vec<Problem> {
        vec![Problem { code, message: message.into() }]
    }

    fn finding(&self) -> Finding {
        Finding { severity: Severity::Error, code: self.code, message: self.message.clone() }
    }
}

fn encode_code(err: &EncodeError) -> &'static str {
    match err {
        EncodeError::NotFinite(_) => "not-finite",
        EncodeError::EpochBeforeUnixEpoch => "epoch-before-1970",
        EncodeError::EpochAfterLastTick(_) => "epoch-after-last-tick",
        EncodeError::MeanMotionNotPositive(_) => "mean-motion-not-positive",
        EncodeError::EccentricityOutOfRange(_) => "eccentricity-out-of-range",
        EncodeError::InclinationOutOfRange(_) => "inclination-out-of-range",
        EncodeError::PerigeeBelowFloor(_) => "perigee-below-floor",
    }
}

fn tle_problem(err: TleError) -> Vec<Problem> {
    let code = match &err {
        TleError::Length { .. } => "tle-line-length",
        TleError::LineNumber { .. } => "tle-line-number",
        TleError::Checksum { .. } => "tle-checksum",
        TleError::CatalogMismatch => "tle-catalog-mismatch",
        TleError::Field { .. } => "tle-field",
        TleError::Encode(err) => encode_code(err),
        _ => "tle",
    };
    Problem::new(code, err.to_string())
}

fn omm_problem(err: OmmError) -> Vec<Problem> {
    let code = match &err {
        OmmError::Epoch(_) => "omm-epoch",
        OmmError::Header(..) => "omm-header",
        OmmError::Encode(err) => encode_code(err),
        _ => "omm",
    };
    Problem::new(code, err.to_string())
}

/// The bold "Checks:" header and one line per finding.
fn checks(findings: &[Finding], color: bool) -> String {
    let mut out = format!("{}\n", paint("Checks:", BOLD, color));
    for finding in findings {
        out.push_str(&format!("  {finding}\n"));
    }
    out
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let (mut output, mut source) = (None, None);
    let mut set_source = |s: Source| {
        if source.replace(s).is_some() {
            return Err("give one input: a value, - or -i <file>".to_string());
        }
        Ok(())
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_string())),
            _ => (arg.as_str(), None),
        };
        let mut value = |name: &str| inline.clone().or_else(|| it.next().cloned()).ok_or(format!("{name} needs a value"));
        match flag {
            "-h" | "--help" => return Ok(Command::Help),
            "-v" | "--version" => return Ok(Command::Version),
            "-o" | "--output" => {
                output = Some(match value("--output")?.to_ascii_lowercase().as_str() {
                    "kepid" => Format::KepId,
                    "tle" => Format::Tle,
                    "omm" => Format::Omm,
                    "json" => Format::Json,
                    other => return Err(format!("unknown output format {other:?}; use kepid, tle, omm or json")),
                })
            }
            "-i" | "--input" => set_source(Source::File(value("--input")?))?,
            "-" => set_source(Source::Stdin)?,
            _ if flag.starts_with('-') && flag.len() > 1 && !flag.starts_with("- ") => {
                return Err(format!("unknown option {arg}"));
            }
            _ => set_source(Source::Text(arg.clone()))?,
        }
    }
    let source = source.ok_or("no input")?;
    Ok(Command::Run { output, source })
}

fn read(source: Source) -> Result<String, Vec<Problem>> {
    match source {
        Source::Text(text) => Ok(text),
        Source::File(path) => std::fs::read_to_string(&path).map_err(|err| Problem::new("unreadable-input", format!("{path}: {err}"))),
        Source::Stdin => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|err| Problem::new("unreadable-input", format!("standard input: {err}")))?;
            Ok(text)
        }
    }
}

/// Detects the input format: JSON is an OMM, two lines starting with `1 ` and
/// `2 ` are a TLE (a name line before them is skipped), anything else must be
/// a KepID.
fn detect(text: &str) -> Result<Input, Vec<Problem>> {
    let text = text.replace("\\n", "\n");
    let trimmed = text.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|err| Problem::new("invalid-json", format!("not valid JSON: {err}")))?;
        let object = match &value {
            serde_json::Value::Array(items) if items.len() == 1 => items[0].clone(),
            serde_json::Value::Array(items) => {
                return Err(Problem::new("several-omms", format!("expected one OMM, found {}", items.len())));
            }
            value => value.clone(),
        };
        let omm: Omm = serde_json::from_value(object).map_err(|err| Problem::new("invalid-omm", format!("not an OMM: {err}")))?;
        return Ok(Input::Omm { omm: Box::new(omm), json: value });
    }
    let lines: Vec<&str> = trimmed.lines().map(str::trim_end).filter(|l| !l.is_empty()).collect();
    if let Some(i) = lines.windows(2).position(|w| w[0].starts_with("1 ") && w[1].starts_with("2 ")) {
        let catalog = lines[i].get(2..7).and_then(read_catalog);
        let name = i.checked_sub(1).map(|n| lines[n].trim().to_string());
        return Ok(Input::Tle { name, lines: [lines[i].to_string(), lines[i + 1].to_string()], catalog });
    }
    trimmed
        .parse()
        .map(Input::KepId)
        .map_err(|_| Problem::new("unknown-format", "not a KepID, a TLE or an OMM"))
}

/// A TLE catalog number: five digits, or Alpha-5 (a letter for 10-33, I and O
/// skipped, then four digits).
fn read_catalog(text: &str) -> Option<u32> {
    let text = text.trim_start();
    let (head, digits) = match text.chars().next()? {
        c @ 'A'..='Z' => (10 + "ABCDEFGHJKLMNPQRSTUVWXYZ".find(c)? as u32, &text[1..]),
        _ => (0, text),
    };
    digits.parse::<u32>().ok().map(|n| head * 10_000 + n)
}

impl Input {
    fn format_name(&self) -> &'static str {
        match self {
            Input::KepId(_) => "KepID",
            Input::Tle { .. } => "TLE",
            Input::Omm { .. } => "OMM",
        }
    }

    fn catalog(&self) -> Option<u32> {
        match self {
            Input::KepId(_) => None,
            Input::Tle { catalog, .. } => *catalog,
            Input::Omm { omm, .. } => omm.norad_cat_id,
        }
    }

    fn kepid(&self) -> Result<KepId, Vec<Problem>> {
        match self {
            Input::KepId(id) => Ok(*id),
            Input::Tle { lines: [l1, l2], .. } => kepid::from_tle(l1, l2).map_err(tle_problem),
            Input::Omm { omm, .. } => kepid::from_omm(omm).map_err(omm_problem),
        }
    }
}

/// One format, converted through the KepID.
fn convert(input: &Input, format: Format, color: bool, now_us: i64) -> Result<String, Vec<Problem>> {
    let id = input.kepid()?;
    kepid::decode(id).map_err(|_| not_a_kepid(id, now_us))?;
    Ok(match format {
        Format::KepId => format!("{id}\n"),
        Format::Tle => tle_text(id, input.catalog())?,
        Format::Omm => omm_text(&kepid_omm(id, input.catalog())?, color),
        Format::Json => format!("{}\n", pretty(&analyse_json(input, now_us)?, 0, color)),
    })
}

/// The errors of a value that is not a profile 0 KepID: its marker checks.
fn not_a_kepid(id: KepId, now_us: i64) -> Vec<Problem> {
    lint::lint_at(id, now_us)
        .into_iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| Problem { code: f.code, message: f.message })
        .collect()
}

/// Every format, with the bit layout. The input's own format is shown as
/// given; the others are inferred from the KepID.
fn analyse(input: &Input, color: bool, now_us: i64) -> Result<String, Vec<Problem>> {
    let id = input.kepid()?;
    let el = kepid::decode(id).map_err(|_| not_a_kepid(id, now_us))?;
    let bold = |text: &str| paint(text, BOLD, color);
    let mut out = String::new();
    out.push_str(&format!("{} {} (other formats inferred)\n\n", bold("Input format:"), input.format_name()));
    out.push_str(&format!("{} {id}\n\n", bold("KepID:")));
    if let Some(age) = old_epoch_age(id, now_us) {
        let warning = paint("Warning:", WARNING_BOLD, color);
        out.push_str(&format!("{warning}{}\n\n", paint(&format!(" epoch is {age} old!"), WARNING, color)));
    }
    let tle = match input {
        Input::Tle { name: Some(name), lines: [l1, l2], .. } => format!("{name}\n{l1}\n{l2}\n"),
        Input::Tle { name: None, lines: [l1, l2], .. } => format!("{l1}\n{l2}\n"),
        _ => tle_text(id, input.catalog())?,
    };
    out.push_str(&format!("{}\n{tle}\n", bold("TLE:")));
    let omm = match input {
        Input::Omm { json, .. } => format!("{}\n", pretty(json, 0, color)),
        _ => omm_text(&kepid_omm(id, input.catalog())?, color),
    };
    out.push_str(&format!("{}\n{omm}\n", bold("OMM:")));
    let propagated = propagation::propagate(id, now_us).map_or_else(|err| format!("- not available: {err}\n"), |p| propagation::lines(&p));
    out.push_str(&format!("{}\n{propagated}\n", bold("Propagation (now):")));
    out.push_str(&format!("{}\n{}", bold("Bit layout:"), card::card(id, &el, color)));
    let findings = lint::lint_at(id, now_us);
    if !findings.is_empty() {
        out.push_str(&format!("\n{}", checks(&findings, color)));
    }
    Ok(out)
}

/// The analysis as JSON: the same sections without the bit layout, with
/// plain numbers (altitude in m, speed in m/s, orbit duration in s).
fn analyse_json(input: &Input, now_us: i64) -> Result<serde_json::Value, Vec<Problem>> {
    let id = input.kepid()?;
    kepid::decode(id).map_err(|_| not_a_kepid(id, now_us))?;
    let tle = match input {
        Input::Tle { lines, .. } => lines.clone(),
        _ => kepid::to_tle_for(id, input.catalog().unwrap_or(0)).map_err(tle_problem)?,
    };
    let omm = match input {
        Input::Omm { json: serde_json::Value::Array(items), .. } => items[0].clone(),
        Input::Omm { json, .. } => json.clone(),
        _ => omm_value(&kepid_omm(id, input.catalog())?),
    };
    let propagation = match propagation::propagate(id, now_us) {
        Ok(p) => serde_json::json!({
            "latitude": p.latitude,
            "longitude": p.longitude,
            "altitude": p.altitude_km * 1000.0,
            "speed": p.speed_km_s * 1000.0,
            "orbit_duration": p.period_minutes * 60.0,
        }),
        Err(_) => serde_json::Value::Null,
    };
    let checks: Vec<serde_json::Value> = lint::lint_at(id, now_us).iter().map(|f| f.to_string().into()).collect();
    Ok(serde_json::json!({
        "input": input.format_name(),
        "kepid": id.to_string(),
        "warning": old_epoch_age(id, now_us).map(|age| format!("epoch is {age} old")),
        "elements": elements_json(id),
        "tle": tle,
        "omm": omm,
        "propagation": propagation,
        "checks": checks,
    }))
}

/// Every field of the KepID: `raw`, the integer codes from its bits, and
/// `parsed`, what the curves resolve them to (the coded variables of the
/// layout: w for mean motion, s for eccentricity, D for drag; the epoch in
/// Unix microseconds).
fn elements_json(id: KepId) -> serde_json::Value {
    use kepid::profile0::*;
    let f = Fields::unpack(id);
    serde_json::json!({
        "raw": {
            "profile": f.profile,
            "epoch": f.epoch_ticks,
            "arg_perigee": f.arg_perigee,
            "inclination": f.inclination,
            "raan": f.raan,
            "drag": f.drag,
            "lambda": f.lambda,
            "mean_motion": f.mean_motion,
            "eccentricity": f.eccentricity,
        },
        "parsed": {
            "profile": f.profile,
            "epoch": id.epoch_unix_us(),
            "arg_perigee": ARG_PERIGEE.decode(f.arg_perigee.into()),
            "inclination": INCLINATION.decode(f.inclination.into()),
            "raan": RAAN.decode(f.raan.into()),
            "drag": DRAG.decode(f.drag.into()),
            "lambda": LAMBDA.decode(f.lambda.into()),
            "mean_motion": MEAN_MOTION.decode(f.mean_motion),
            "eccentricity": ECCENTRICITY.decode(f.eccentricity.into()),
        },
    })
}

fn now_unix_us() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_micros() as i64)
}

fn tle_text(id: KepId, catalog: Option<u32>) -> Result<String, Vec<Problem>> {
    let [l1, l2] = kepid::to_tle_for(id, catalog.unwrap_or(0)).map_err(tle_problem)?;
    Ok(format!("{l1}\n{l2}\n"))
}

fn kepid_omm(id: KepId, catalog: Option<u32>) -> Result<Omm, Vec<Problem>> {
    match catalog {
        Some(catalog) => kepid::to_omm_for(id, catalog),
        None => kepid::to_omm(id),
    }
    .map_err(omm_problem)
}

/// ANSI styles for the analysis: bold headers, and JSON keys, strings,
/// numbers and literals in their own colors.
const BOLD: &str = "1";
const JSON_KEY: &str = "34";
const JSON_STRING: &str = "32";
const JSON_NUMBER: &str = "33";
const JSON_LITERAL: &str = "35";
const WARNING: &str = "33";
const WARNING_BOLD: &str = "1;33";

/// Epochs older than this get a warning in the analysis.
const OLD_EPOCH_US: i64 = 7 * 86_400_000_000;

/// How old the epoch of `id` is at `now_us`, in words, when it is older than a
/// week; `None` otherwise (including epochs in the future).
fn old_epoch_age(id: KepId, now_us: i64) -> Option<String> {
    let age_us = now_us - id.epoch_unix_us();
    (age_us > OLD_EPOCH_US).then(|| sparse_age(age_us))
}

/// An age in the largest unit that fits at least once, rounded down: "3
/// weeks", "5 months", "1 year".
fn sparse_age(age_us: i64) -> String {
    const DAY: f64 = 86_400_000_000.0;
    let days = age_us as f64 / DAY;
    let (count, unit) = [(365.2425, "year"), (365.2425 / 12.0, "month"), (7.0, "week"), (1.0, "day")]
        .into_iter()
        .map(|(length, unit)| ((days / length).floor() as i64, unit))
        .find(|&(count, _)| count >= 1)
        .unwrap_or((0, "day"));
    format!("{count} {unit}{}", if count == 1 { "" } else { "s" })
}

fn paint(text: &str, code: &str, color: bool) -> String {
    if color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// The OMM keywords SGP4 needs, as indented JSON, highlighted when `color`
/// is on.
fn omm_text(omm: &Omm, color: bool) -> String {
    format!("{}\n", pretty(&omm_value(omm), 0, color))
}

/// The OMM keywords SGP4 needs, in order, as a JSON object.
fn omm_value(omm: &Omm) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("EPOCH".into(), omm.epoch.clone().into());
    for (key, value) in [
        ("MEAN_MOTION", omm.mean_motion),
        ("ECCENTRICITY", omm.eccentricity),
        ("INCLINATION", omm.inclination),
        ("RA_OF_ASC_NODE", omm.ra_of_asc_node),
        ("ARG_OF_PERICENTER", omm.arg_of_pericenter),
        ("MEAN_ANOMALY", omm.mean_anomaly),
    ] {
        map.insert(key.into(), value.into());
    }
    if let Some(catalog) = omm.norad_cat_id {
        map.insert("NORAD_CAT_ID".into(), catalog.into());
    }
    map.insert("BSTAR".into(), omm.bstar.into());
    serde_json::Value::Object(map)
}

/// Any JSON value, indented by four spaces per level, keys in their original
/// order, highlighted when `color` is on.
fn pretty(value: &serde_json::Value, depth: usize, color: bool) -> String {
    use serde_json::Value;
    let indent = |d: usize| "    ".repeat(d);
    let text = |v: &Value| serde_json::to_string(v).unwrap_or_default();
    match value {
        Value::Object(map) if map.is_empty() => "{}".into(),
        Value::Array(items) if items.is_empty() => "[]".into(),
        Value::Object(map) => {
            let rows: Vec<String> = map
                .iter()
                .map(|(key, v)| {
                    let key = paint(&text(&Value::String(key.clone())), JSON_KEY, color);
                    format!("{}{key}: {}", indent(depth + 1), pretty(v, depth + 1, color))
                })
                .collect();
            format!("{{\n{}\n{}}}", rows.join(",\n"), indent(depth))
        }
        Value::Array(items) => {
            let rows: Vec<String> = items.iter().map(|v| format!("{}{}", indent(depth + 1), pretty(v, depth + 1, color))).collect();
            format!("[\n{}\n{}]", rows.join(",\n"), indent(depth))
        }
        Value::String(_) => paint(&text(value), JSON_STRING, color),
        Value::Number(_) => paint(&text(value), JSON_NUMBER, color),
        Value::Bool(_) | Value::Null => paint(&text(value), JSON_LITERAL, color),
    }
}
