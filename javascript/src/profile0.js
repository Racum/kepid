// Constants of KepID profile 0 (docs/spec.md, sections 3 and 4, and Appendix A).

import { FieldMap } from "./curve.js";

// Epoch: one tick is 256 ms; the field holds 40 bits of ticks.
export const TICK_US = 256000n;
export const MAX_EPOCH_TICKS = (1n << 40n) - 1n;

// Physical and derived constants (section 4.1), as the spec's literals.
export const MU = 398600.4418;
export const RE = 6378.137;
export const R_FLOOR = 6628.137;
export const TAU = 6.283185307179586;
export const N_FLOOR = 16.088499719622039;
export const C_SGP4 = 1.039196724434337;
export const S_SGP4 = 1.0122292801892716;
export const G_REF = 50284.883336077255;
export const D_MAX = 1.0;

// Field offsets (bit 0 is the most significant) and widths (section 3).
export const EPOCH_BITS = [0, 40];
export const ARG_PERIGEE_BITS = [40, 8];
export const VERSION_BITS = [48, 4];
export const INCLINATION_BITS = [52, 12];
export const VARIANT_BITS = [64, 2];
export const RAAN_BITS = [66, 14];
export const DRAG_BITS = [80, 10];
export const PROFILE_BITS = [90, 1];
export const LAMBDA_BITS = [91, 13];
export const MEAN_MOTION_BITS = [104, 17];
export const ECCENTRICITY_BITS = [121, 7];

// Curve layers [a, c, g], Appendix A.
const ARG_PERIGEE_LAYERS = [
  [4.083428629e-2, 7.664452626e-1, 3.990766569e-2],
  [5.507395214e-2, 6.064244552e-2, 1.176634895e-1],
];
const INCLINATION_LAYERS = [
  [1.082992940e-2, 2.953274111e-1, 2.777786713e-4],
  [1.630044140e-2, 2.363204369e-1, 2.777795907e-4],
  [6.892137273e-2, 5.603886736e-1, 6.659174209e-3],
  [4.314797970e-1, 3.784270310e-1, 1.173090567e-1],
  [2.925692630e-2, 6.592332912e-1, 1.631219892e-3],
];
const DRAG_LAYERS = [
  [9.623760485e-1, 1.589264601e-9, 5.318080922e-2],
  [2.954822161e-1, 7.986615314e-1, 3.778745014e-2],
  [1.786089996e-1, 1.116401956e-2, 1.338212342e-2],
  [8.140581342e-2, 2.365399929e-11, 1.654625459e-2],
];
const LAMBDA_LAYERS = [
  [2.506724280e-3, 5.001930575e-1, 4.161572285e-4],
  [1.616032485e-1, 5.781309497e-1, 8.902241343e-2],
  [5.978329411e-2, 4.579863973e-1, 2.595466093e-3],
  [2.874542339e-2, 9.376973790e-1, 9.133208180e-3],
  [1.792777376e-2, 9.882822534e-1, 6.059456682e-3],
];
const MEAN_MOTION_LAYERS = [
  [7.894998790e-1, 8.748975979e-1, 6.707300842e-2],
  [3.912329768e-1, 8.693916408e-1, 1.117938012e-1],
  [1.953420518e-1, 9.114594208e-1, 4.088966496e-2],
  [2.833931871e-1, 9.338001081e-1, 1.497879339e-2],
  [3.233338923e-1, 2.450968642e-2, 3.173610361e-2],
];
const ECCENTRICITY_LAYERS = [
  [4.492276748e-1, 4.502440907e-3, 6.406040192e-2],
  [6.128914601e-2, 8.756510763e-27, 2.929546956e-2],
  [8.952636190e-1, 1.000000000e0, 6.953619088e-1],
  [7.664520029e-2, 9.896597597e-1, 4.532361035e-2],
];

// Field maps (section 4.2).
export const ARG_PERIGEE = new FieldMap(8, { circular: 0 }, ARG_PERIGEE_LAYERS);
export const INCLINATION = new FieldMap(12, { interval: [0, 180] }, INCLINATION_LAYERS);
export const RAAN = new FieldMap(14, { circular: 0 }, []);
export const DRAG = new FieldMap(10, { signed: D_MAX }, DRAG_LAYERS);
export const LAMBDA = new FieldMap(13, { circular: -180 }, LAMBDA_LAYERS);
export const MEAN_MOTION = new FieldMap(17, { interval: [0, 1] }, MEAN_MOTION_LAYERS);
export const ECCENTRICITY = new FieldMap(7, { interval: [0, 1] }, ECCENTRICITY_LAYERS);
