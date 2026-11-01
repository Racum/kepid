//! KepID: zero-line orbital elements packed into a UUIDv7.
//!
//! A KepID holds an SGP4 mean element set (the content of a TLE or an OMM) in
//! one 128-bit UUIDv7. It describes an orbit, not an object, and is not unique.
//! The format is defined in `docs/spec.md`.
//!
//! Encoding rounds each field to its code, no propagation and no fit; the only
//! adjustment is moving the mean argument of latitude to the rounded epoch.
//! TLEs are read and written as text ([`from_tle`], [`to_tle`],
//! [`Elements::from_tle`], [`Elements::to_tle`]). With the `omm` feature, OMMs
//! are read and written as a serde struct (`Omm`, `from_omm`, `to_omm`).

mod codec;
pub mod curve;
mod elements;
mod error;
mod kepid_id;
#[cfg(feature = "omm")]
mod omm;
pub mod profile0;
mod time;
mod tle;

pub use codec::{decode, drag_factor, encode, w_of_mean_motion};
pub use elements::{check_range, Elements};
pub use error::{DecodeError, EncodeError, ParseError};
pub use kepid_id::{Fields, KepId};
#[cfg(feature = "omm")]
pub use omm::{from_omm, to_omm, to_omm_for, Omm, OmmError};
pub use time::tle_epoch_unix_us;
pub use tle::{from_tle, to_tle, to_tle_for, TleError};
