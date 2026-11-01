//! Orbit Mean-Elements Messages (CCSDS OMM), as a serde struct. Needs the
//! `omm` feature.
//!
//! [`Omm`] uses the OMM keywords as field names, the way CelesTrak's and
//! Space-Track's JSON do, so any serde format can read and write it. Numbers
//! may arrive as JSON numbers (CelesTrak) or as strings (Space-Track); strings
//! are parsed exactly. With serde_json, enable its `float_roundtrip` feature:
//! without it, a JSON number can land one ulp off, which can change a KepID.

use std::fmt;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};

use crate::codec::{decode, encode};
use crate::error::{DecodeError, EncodeError};
use crate::time::{iso8601_unix_us, omm_epoch};
use crate::{Elements, KepId};

/// The placeholders written in the identity fields a KepID does not have,
/// the same as in a TLE: classification `U`, element set 999 and revolution
/// number 99999. The catalog number is left out unless the caller gives one.
const CLASSIFICATION: &str = "U";
const ELEMENT_SET: u32 = 999;
const REV_NUMBER: u32 = 99_999;

/// One OMM, with the keywords a KepID reads or writes.
///
/// Reading ignores keywords not listed here. The optional CCSDS header
/// keywords are checked when present: `MEAN_ELEMENT_THEORY` must be `SGP4`,
/// `REF_FRAME` must be `TEME` and `TIME_SYSTEM` must be `UTC`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct Omm {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_name: Option<String>,
    /// International designator, such as `1998-067A`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_frame: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_element_theory: Option<String>,
    /// ISO 8601 UTC, such as `2026-11-01T05:00:23.160096`.
    pub epoch: String,
    /// Rev/day.
    #[serde(deserialize_with = "number")]
    pub mean_motion: f64,
    #[serde(deserialize_with = "number")]
    pub eccentricity: f64,
    /// Degrees.
    #[serde(deserialize_with = "number")]
    pub inclination: f64,
    /// Degrees.
    #[serde(deserialize_with = "number")]
    pub ra_of_asc_node: f64,
    /// Degrees.
    #[serde(deserialize_with = "number")]
    pub arg_of_pericenter: f64,
    /// Degrees.
    #[serde(deserialize_with = "number")]
    pub mean_anomaly: f64,
    #[serde(default, deserialize_with = "optional_integer", skip_serializing_if = "Option::is_none")]
    pub ephemeris_type: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification_type: Option<String>,
    #[serde(default, deserialize_with = "optional_integer", skip_serializing_if = "Option::is_none")]
    pub norad_cat_id: Option<u32>,
    #[serde(default, deserialize_with = "optional_integer", skip_serializing_if = "Option::is_none")]
    pub element_set_no: Option<u32>,
    #[serde(default, deserialize_with = "optional_integer", skip_serializing_if = "Option::is_none")]
    pub rev_at_epoch: Option<u32>,
    /// 1/earth radii.
    #[serde(deserialize_with = "number")]
    pub bstar: f64,
    #[serde(default, deserialize_with = "optional_number", skip_serializing_if = "Option::is_none")]
    pub mean_motion_dot: Option<f64>,
    #[serde(default, deserialize_with = "optional_number", skip_serializing_if = "Option::is_none")]
    pub mean_motion_ddot: Option<f64>,
}

/// Why an OMM cannot be read or written.
#[derive(Debug, Clone, PartialEq)]
pub enum OmmError {
    /// `EPOCH` is not an ISO 8601 UTC date-time; carries it.
    Epoch(String),
    /// A header keyword with a value a KepID cannot carry, such as
    /// `MEAN_ELEMENT_THEORY` = `SGP4-XP`; carries the keyword and its value.
    Header(&'static str, String),
    /// The epoch is past year 9999, which ISO 8601 cannot write. Unix
    /// microseconds.
    EpochOutOfRange(i64),
    /// The OMM was read, but its elements cannot be encoded.
    Encode(EncodeError),
    /// The KepID cannot be decoded.
    Decode(DecodeError),
}

impl fmt::Display for OmmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Epoch(text) => write!(f, "EPOCH {text:?} is not an ISO 8601 UTC date-time"),
            Self::Header(key, value) => write!(f, "{key} is {value:?}, which a KepID cannot carry"),
            Self::EpochOutOfRange(us) => write!(f, "epoch {us} µs is past year 9999"),
            Self::Encode(err) => write!(f, "{err}"),
            Self::Decode(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for OmmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(err) => Some(err),
            Self::Decode(err) => Some(err),
            _ => None,
        }
    }
}

impl Elements {
    /// Reads the mean elements of an OMM. The identity keywords (catalog
    /// number, name and so on) are not kept: a KepID cannot hold them.
    pub fn from_omm(omm: &Omm) -> Result<Elements, OmmError> {
        for (key, value, expected) in [
            ("MEAN_ELEMENT_THEORY", &omm.mean_element_theory, "SGP4"),
            ("REF_FRAME", &omm.ref_frame, "TEME"),
            ("TIME_SYSTEM", &omm.time_system, "UTC"),
        ] {
            if let Some(value) = value {
                if !value.trim().eq_ignore_ascii_case(expected) {
                    return Err(OmmError::Header(key, value.clone()));
                }
            }
        }
        Ok(Elements {
            epoch_unix_us: iso8601_unix_us(&omm.epoch).ok_or_else(|| OmmError::Epoch(omm.epoch.clone()))?,
            mean_motion: omm.mean_motion,
            eccentricity: omm.eccentricity,
            inclination: omm.inclination,
            raan: omm.ra_of_asc_node,
            arg_perigee: omm.arg_of_pericenter,
            mean_anomaly: omm.mean_anomaly,
            bstar: omm.bstar,
        })
    }

    /// Writes these elements as an OMM with no catalog number.
    ///
    /// The header keywords say what the elements are (`EARTH`, `TEME`, `UTC`,
    /// `SGP4`), and the identity keywords are placeholders: classification
    /// `U`, element set 999 and revolution number 99999. The derivatives of
    /// mean motion are written as zero; SGP4 does not use them. Numbers keep
    /// their full precision; the epoch is written to the microsecond.
    pub fn to_omm(&self) -> Result<Omm, OmmError> {
        write(self, None)
    }

    /// Like [`Elements::to_omm`], for the object with this NORAD catalog
    /// number.
    pub fn to_omm_for(&self, catalog: u32) -> Result<Omm, OmmError> {
        write(self, Some(catalog))
    }
}

/// Reads an OMM and encodes it: [`Elements::from_omm`], then [`encode`].
pub fn from_omm(omm: &Omm) -> Result<KepId, OmmError> {
    encode(&Elements::from_omm(omm)?).map_err(OmmError::Encode)
}

/// Decodes a KepID and writes it as an OMM with no catalog number:
/// [`decode`], then [`Elements::to_omm`].
pub fn to_omm(id: KepId) -> Result<Omm, OmmError> {
    decode(id).map_err(OmmError::Decode)?.to_omm()
}

/// Like [`to_omm`], for the object with this NORAD catalog number. A KepID
/// describes an orbit, not an object, so the number comes from the caller.
pub fn to_omm_for(id: KepId, catalog: u32) -> Result<Omm, OmmError> {
    decode(id).map_err(OmmError::Decode)?.to_omm_for(catalog)
}

fn write(el: &Elements, catalog: Option<u32>) -> Result<Omm, OmmError> {
    let text = |s: &str| Some(s.to_string());
    let wrap = |degrees: f64| degrees.rem_euclid(360.0);
    Ok(Omm {
        object_name: None,
        object_id: None,
        center_name: text("EARTH"),
        ref_frame: text("TEME"),
        time_system: text("UTC"),
        mean_element_theory: text("SGP4"),
        epoch: omm_epoch(el.epoch_unix_us).ok_or(OmmError::EpochOutOfRange(el.epoch_unix_us))?,
        mean_motion: el.mean_motion,
        eccentricity: el.eccentricity,
        inclination: el.inclination,
        ra_of_asc_node: wrap(el.raan),
        arg_of_pericenter: wrap(el.arg_perigee),
        mean_anomaly: wrap(el.mean_anomaly),
        ephemeris_type: Some(0),
        classification_type: text(CLASSIFICATION),
        norad_cat_id: catalog,
        element_set_no: Some(ELEMENT_SET),
        rev_at_epoch: Some(REV_NUMBER),
        bstar: el.bstar,
        mean_motion_dot: Some(0.0),
        mean_motion_ddot: Some(0.0),
    })
}

/// Accepts a number, or a string holding one (parsed exactly).
struct Number;

impl Visitor<'_> for Number {
    type Value = f64;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a number, or a string holding one")
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<f64, E> {
        Ok(v)
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<f64, E> {
        Ok(v as f64)
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<f64, E> {
        Ok(v as f64)
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<f64, E> {
        v.trim().parse().map_err(|_| E::invalid_value(de::Unexpected::Str(v), &self))
    }
}

/// Accepts a whole number, or a string holding one.
struct Integer;

impl Visitor<'_> for Integer {
    type Value = u32;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a whole number, or a string holding one")
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<u32, E> {
        u32::try_from(v).map_err(|_| E::invalid_value(de::Unexpected::Unsigned(v), &self))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<u32, E> {
        u32::try_from(v).map_err(|_| E::invalid_value(de::Unexpected::Signed(v), &self))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<u32, E> {
        if v.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&v) {
            Ok(v as u32)
        } else {
            Err(E::invalid_value(de::Unexpected::Float(v), &self))
        }
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<u32, E> {
        v.trim().parse().map_err(|_| E::invalid_value(de::Unexpected::Str(v), &self))
    }
}

fn number<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    d.deserialize_any(Number)
}

fn optional_number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    #[derive(Deserialize)]
    struct Wrap(#[serde(deserialize_with = "number")] f64);
    Ok(Option::<Wrap>::deserialize(d)?.map(|Wrap(v)| v))
}

fn optional_integer<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    #[derive(Deserialize)]
    struct Wrap(#[serde(deserialize_with = "integer")] u32);
    Ok(Option::<Wrap>::deserialize(d)?.map(|Wrap(v)| v))
}

fn integer<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    d.deserialize_any(Integer)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CelesTrak's OMM for the ISS, fetched with the TLE in the README.
    const CELESTRAK: &str = r#"{"OBJECT_NAME":"ISS (ZARYA)","OBJECT_ID":"1998-067A","EPOCH":"2026-11-01T05:00:23.160096","MEAN_MOTION":15.49138562,"ECCENTRICITY":0.0007056,"INCLINATION":51.6332,"RA_OF_ASC_NODE":342.0408,"ARG_OF_PERICENTER":334.5041,"MEAN_ANOMALY":25.5599,"EPHEMERIS_TYPE":0,"CLASSIFICATION_TYPE":"U","NORAD_CAT_ID":25544,"ELEMENT_SET_NO":999,"REV_AT_EPOCH":59299,"BSTAR":0.00013267359,"MEAN_MOTION_DOT":6.903e-5,"MEAN_MOTION_DDOT":0}"#;

    fn iss() -> Omm {
        serde_json::from_str(CELESTRAK).unwrap()
    }

    #[test]
    fn reads_celestrak_json() {
        let omm = iss();
        assert_eq!(omm.norad_cat_id, Some(25544));
        let el = Elements::from_omm(&omm).unwrap();
        assert_eq!(el.epoch_unix_us, 1_793_509_223_160_096);
        assert_eq!((el.mean_motion, el.eccentricity, el.bstar), (15.49138562, 0.0007056, 0.00013267359));
        assert_eq!((el.inclination, el.raan, el.arg_perigee, el.mean_anomaly), (51.6332, 342.0408, 334.5041, 25.5599));
    }

    #[test]
    fn reads_space_track_strings() {
        let text = CELESTRAK
            .replace("15.49138562", "\"15.49138562\"")
            .replace("0.00013267359", "\"0.00013267359\"")
            .replace("25544", "\"25544\"")
            .replace("\"EPHEMERIS_TYPE\":0", "\"EPHEMERIS_TYPE\":\"0\"");
        let omm: Omm = serde_json::from_str(&text).unwrap();
        assert_eq!(omm, iss());
    }

    #[test]
    fn same_kepid_as_the_readme_tle() {
        // The OMM's B* has more digits than the TLE's 13267-3, but both round
        // to the same code.
        assert_eq!(from_omm(&iss()).unwrap().to_string(), "01a1957a-ebef-73ab-bccf-878de5f3a90d");
    }

    #[test]
    fn kepid_round_trips_through_omm_and_json() {
        let id = from_omm(&iss()).unwrap();
        let omm = to_omm(id).unwrap();
        assert_eq!(omm.norad_cat_id, None);
        assert_eq!(from_omm(&omm).unwrap(), id);
        let json = serde_json::to_string(&omm).unwrap();
        assert!(!json.contains("NORAD_CAT_ID") && json.contains("\"MEAN_ELEMENT_THEORY\":\"SGP4\""), "{json}");
        let back: Omm = serde_json::from_str(&json).unwrap();
        assert_eq!(back, omm);
        assert_eq!(from_omm(&back).unwrap(), id);
        let named = to_omm_for(id, 25544).unwrap();
        assert_eq!((named.norad_cat_id, named.rev_at_epoch), (Some(25544), Some(99_999)));
    }

    #[test]
    fn refuses_other_theories_and_frames() {
        let xp = Omm { mean_element_theory: Some("SGP4-XP".into()), ..iss() };
        assert_eq!(Elements::from_omm(&xp), Err(OmmError::Header("MEAN_ELEMENT_THEORY", "SGP4-XP".into())));
        let gcrf = Omm { ref_frame: Some("GCRF".into()), ..iss() };
        assert!(matches!(Elements::from_omm(&gcrf), Err(OmmError::Header("REF_FRAME", _))));
        let ok = Omm { mean_element_theory: Some("sgp4".into()), ref_frame: Some("TEME".into()), ..iss() };
        assert!(Elements::from_omm(&ok).is_ok());
    }

    #[test]
    fn epochs_follow_the_spec() {
        let at = |epoch: &str| Elements::from_omm(&Omm { epoch: epoch.into(), ..iss() }).map(|el| el.epoch_unix_us);
        assert_eq!(at("2026-11-01T05:00:23.160096Z"), Ok(1_793_509_223_160_096));
        assert_eq!(at("2026-11-01T05:00:23.16"), Ok(1_793_509_223_160_000));
        assert_eq!(at("2026-11-01T05:00:23"), Ok(1_793_509_223_000_000));
        assert_eq!(at("2026-11-01T05:00:23.1600965"), Ok(1_793_509_223_160_097), "a half rounds up");
        assert_eq!(at("2026-11-01T05:00:23.1600964999"), Ok(1_793_509_223_160_096));
        assert_eq!(at("1970-01-01T00:00:00"), Ok(0));
        for bad in ["2026-11-01 05:00:23", "2026-02-29T00:00:00", "2026-11-01T24:00:00", "2026-11-01T05:00:60", "2026-305T05:00:23", "26-11-01T05:00:23"] {
            assert_eq!(at(bad), Err(OmmError::Epoch(bad.into())), "{bad}");
        }
    }

    #[test]
    fn writes_the_epoch_to_the_microsecond() {
        let el = Elements { epoch_unix_us: 1_793_509_223_168_000, ..Elements::from_omm(&iss()).unwrap() };
        assert_eq!(el.to_omm().unwrap().epoch, "2026-11-01T05:00:23.168000");
        let far = Elements { epoch_unix_us: 253_402_300_800_000_000, ..el };
        assert_eq!(far.to_omm(), Err(OmmError::EpochOutOfRange(253_402_300_800_000_000)));
    }
}
