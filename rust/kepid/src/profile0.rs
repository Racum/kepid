//! Constants that define KepID profile 0. See `docs/spec.md`, section 4 and Appendix A.

use crate::curve::{FieldMap, Kind, Layer};

/// Epoch resolution: one tick is 256 ms, in microseconds.
pub const TICK_US: i64 = 256_000;
/// Last representable epoch tick, 2^40 - 1.
pub const MAX_EPOCH_TICKS: u64 = (1 << 40) - 1;

/// Earth's gravitational parameter, km^3/s^2.
pub const MU: f64 = 398600.4418;
/// Earth's equatorial radius, km.
pub const RE: f64 = 6378.137;
/// Lowest perigee altitude an element set may have, km.
pub const PERIGEE_FLOOR_KM: f64 = 250.0;
/// Radius of the perigee floor, km.
pub const R_FLOOR: f64 = RE + PERIGEE_FLOOR_KM;
/// Circular mean motion at the perigee floor, rev/day:
/// `(86400 / TAU) * sqrt(MU / ((R_FLOOR * R_FLOOR) * R_FLOOR))`.
pub const N_FLOOR: f64 = 16.08849971962204;
/// The floor radius in SGP4 (WGS-72) Earth radii: `R_FLOOR / 6378.135`.
pub const C_SGP4: f64 = 1.039196724434337;
/// SGP4's atmospheric reference radius s in Earth radii: `1 + 78 / 6378.135`.
pub const S_SGP4: f64 = 1.0122292801892716;
/// Drag factor of a circular orbit at 500 km: `g(R_FLOOR / (RE + 500), 0)`.
pub const G_REF: f64 = 50284.883336077255;
/// Largest magnitude of the drag field D.
pub const D_MAX: f64 = 1.0;

/// Field offsets (bit 0 is the most significant) and widths.
pub const EPOCH_BITS: (u32, u32) = (0, 40);
pub const ARG_PERIGEE_BITS: (u32, u32) = (40, 8);
pub const VERSION_BITS: (u32, u32) = (48, 4);
pub const INCLINATION_BITS: (u32, u32) = (52, 12);
pub const VARIANT_BITS: (u32, u32) = (64, 2);
pub const RAAN_BITS: (u32, u32) = (66, 14);
pub const DRAG_BITS: (u32, u32) = (80, 10);
pub const PROFILE_BITS: (u32, u32) = (90, 1);
pub const LAMBDA_BITS: (u32, u32) = (91, 13);
pub const MEAN_MOTION_BITS: (u32, u32) = (104, 17);
pub const ECCENTRICITY_BITS: (u32, u32) = (121, 7);

const ARG_PERIGEE_LAYERS: [Layer; 2] = [
    (4.083428629e-2, 7.664452626e-1, 3.990766569e-2),
    (5.507395214e-2, 6.064244552e-2, 1.176634895e-1),
];

const INCLINATION_LAYERS: [Layer; 5] = [
    (1.082992940e-2, 2.953274111e-1, 2.777786713e-4),
    (1.630044140e-2, 2.363204369e-1, 2.777795907e-4),
    (6.892137273e-2, 5.603886736e-1, 6.659174209e-3),
    (4.314797970e-1, 3.784270310e-1, 1.173090567e-1),
    (2.925692630e-2, 6.592332912e-1, 1.631219892e-3),
];

const DRAG_LAYERS: [Layer; 4] = [
    (9.623760485e-1, 1.589264601e-9, 5.318080922e-2),
    (2.954822161e-1, 7.986615314e-1, 3.778745014e-2),
    (1.786089996e-1, 1.116401956e-2, 1.338212342e-2),
    (8.140581342e-2, 2.365399929e-11, 1.654625459e-2),
];

const LAMBDA_LAYERS: [Layer; 5] = [
    (2.506724280e-3, 5.001930575e-1, 4.161572285e-4),
    (1.616032485e-1, 5.781309497e-1, 8.902241343e-2),
    (5.978329411e-2, 4.579863973e-1, 2.595466093e-3),
    (2.874542339e-2, 9.376973790e-1, 9.133208180e-3),
    (1.792777376e-2, 9.882822534e-1, 6.059456682e-3),
];

const MEAN_MOTION_LAYERS: [Layer; 5] = [
    (7.894998790e-1, 8.748975979e-1, 6.707300842e-2),
    (3.912329768e-1, 8.693916408e-1, 1.117938012e-1),
    (1.953420518e-1, 9.114594208e-1, 4.088966496e-2),
    (2.833931871e-1, 9.338001081e-1, 1.497879339e-2),
    (3.233338923e-1, 2.450968642e-2, 3.173610361e-2),
];

const ECCENTRICITY_LAYERS: [Layer; 4] = [
    (4.492276748e-1, 4.502440907e-3, 6.406040192e-2),
    (6.128914601e-2, 8.756510763e-27, 2.929546956e-2),
    (8.952636190e-1, 1.000000000e0, 6.953619088e-1),
    (7.664520029e-2, 9.896597597e-1, 4.532361035e-2),
];

/// ω, argument of perigee: circular, 256 codes.
pub const ARG_PERIGEE: FieldMap = FieldMap { bits: 8, kind: Kind::Circular { lo: 0.0 }, layers: &ARG_PERIGEE_LAYERS };
/// i, inclination: [0, 180] degrees, 4096 codes.
pub const INCLINATION: FieldMap = FieldMap { bits: 12, kind: Kind::Interval { lo: 0.0, width: 180.0 }, layers: &INCLINATION_LAYERS };
/// Ω, right ascension of the ascending node: circular, uniform, 16384 codes.
pub const RAAN: FieldMap = FieldMap { bits: 14, kind: Kind::Circular { lo: 0.0 }, layers: &[] };
/// D = B* x drag factor: signed, |D| <= D_MAX, 1024 codes with an exact zero.
pub const DRAG: FieldMap = FieldMap { bits: 10, kind: Kind::Signed { max: D_MAX }, layers: &DRAG_LAYERS };
/// λ = ω + M, mean argument of latitude: circular from -180 degrees, 8192 codes.
pub const LAMBDA: FieldMap = FieldMap { bits: 13, kind: Kind::Circular { lo: -180.0 }, layers: &LAMBDA_LAYERS };
/// w = R_FLOOR / a, the mean motion: [0, 1], 131072 codes.
pub const MEAN_MOTION: FieldMap = FieldMap { bits: 17, kind: Kind::Interval { lo: 0.0, width: 1.0 }, layers: &MEAN_MOTION_LAYERS };
/// s = e / (1 - w), the eccentricity: [0, 1], 128 codes.
pub const ECCENTRICITY: FieldMap = FieldMap { bits: 7, kind: Kind::Interval { lo: 0.0, width: 1.0 }, layers: &ECCENTRICITY_LAYERS };
