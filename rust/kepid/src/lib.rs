//! KepID: zero-line orbital elements packed into a UUIDv7.
//!
//! A KepID holds an SGP4 mean element set (the content of a TLE or an OMM) in
//! one 128-bit UUIDv7. It describes an orbit, not an object, and is not unique.
//! The format is defined in `docs/spec.md`.
//!
//! Encoding rounds each field to its code, no propagation and no fit; the only
//! adjustment is moving the mean argument of latitude to the rounded epoch.
//! TLE/OMM conversion is not implemented yet, apart from the exact TLE epoch
//! ([`tle_epoch_unix_us`]).

mod codec;
pub mod curve;
mod elements;
mod error;
mod explain;
mod kepid_id;
mod lint;
pub mod profile0;
mod time;

pub use codec::{decode, drag_factor, encode, w_of_mean_motion};
pub use elements::{check_range, Elements};
pub use error::{DecodeError, EncodeError, ParseError};
pub use explain::explain;
pub use kepid_id::{Fields, KepId};
pub use lint::{lint, lint_at, Finding, Severity};
pub use time::tle_epoch_unix_us;
