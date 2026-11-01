// Encoding and decoding (docs/spec.md, sections 7 and 8).

import * as p from "./profile0.js";
import { clamp, wrap } from "./curve.js";

/**
 * SGP4 mean elements, in the units a TLE uses (TEME frame). The epoch is a
 * whole number of Unix microseconds, as a BigInt, so the epoch tick never
 * depends on float rounding.
 *
 * @typedef {object} Elements
 * @property {bigint} epochUnixUs
 * @property {number} meanMotion Rev/day.
 * @property {number} eccentricity
 * @property {number} inclination Degrees.
 * @property {number} raan Right ascension of the ascending node, degrees.
 * @property {number} argPerigee Argument of perigee, degrees.
 * @property {number} meanAnomaly Degrees.
 * @property {number} bstar SGP4 B* drag term, 1/Earth radii.
 */

/** A KepID: its text form (hyphenated or not) or its 128-bit value. @typedef {string | bigint} KepIdLike */

/** The element set cannot be encoded as a profile 0 KepID. */
export class EncodeError extends Error {
  name = "EncodeError";
}

/** The value is not a profile 0 KepID. */
export class DecodeError extends Error {
  name = "DecodeError";
}

// ---- shared formulas -----------------------------------------------------------

/**
 * The spec's cube root (section 2), not Math.cbrt: exact scaling by 8 into
 * [1, 8), then six Newton steps from 1.5.
 */
export function cbrt(x) {
  let m = x;
  let scale = 1;
  while (m < 1) {
    m = m * 8;
    scale = scale / 2;
  }
  while (m >= 8) {
    m = m / 8;
    scale = scale * 2;
  }
  let y = 1.5;
  for (let i = 0; i < 6; i++) y = (2 * y + m / (y * y)) / 3;
  return y * scale;
}

/** w = R_FLOOR / a = (n / N_FLOOR)^(2/3) (section 7, step 1). */
export function wOfMeanMotion(n) {
  const v = n / p.N_FLOOR;
  return cbrt(clamp(v * v, 1e-30, 1e30));
}

/** drag(w, e) = g(w, e) / G_REF (section 6.3). */
export function dragFactor(w, e) {
  const q = p.C_SGP4 - p.S_SGP4 * w;
  const eta = (p.C_SGP4 * e) / q;
  const x = Math.max(1 - eta * eta, 1e-12);
  const w2 = w * w;
  const q2 = q * q;
  const g = ((w2 * w2) * w) / (q2 * q2) / (((x * x) * x) * Math.sqrt(x));
  return g / p.G_REF;
}

function put(value, [offset, width]) {
  return (BigInt(value) & ((1n << BigInt(width)) - 1n)) << BigInt(128 - offset - width);
}

function get(x, [offset, width]) {
  return Number((x >> BigInt(128 - offset - width)) & ((1n << BigInt(width)) - 1n));
}

// ---- encoding ------------------------------------------------------------------

const FLOATS = ["meanMotion", "eccentricity", "inclination", "raan", "argPerigee", "meanAnomaly", "bstar"];

function epochOf(el) {
  const epoch = el.epochUnixUs;
  if (typeof epoch === "bigint") return epoch;
  if (Number.isSafeInteger(epoch)) return BigInt(epoch);
  throw new EncodeError("the epoch must be a whole number of microseconds (a BigInt, or a safe integer)");
}

/** Section 7, step 2: the nearest tick, halves rounding up, in integers. */
function tick(epochUs) {
  return (epochUs + 128000n) / 256000n;
}

/** Section 7, step 1. Returns the epoch and w for step 3. */
function validate(el) {
  for (const name of FLOATS) {
    if (typeof el[name] !== "number" || !Number.isFinite(el[name])) {
      throw new EncodeError(`${name} is not a finite number`);
    }
  }
  const epoch = epochOf(el);
  if (epoch < 0n) throw new EncodeError("epoch is before 1970-01-01");
  if (tick(epoch) > p.MAX_EPOCH_TICKS) throw new EncodeError("epoch is past the last representable tick (year 10889)");
  if (el.meanMotion <= 0) throw new EncodeError(`mean motion ${el.meanMotion} rev/day is not positive`);
  if (el.eccentricity < 0 || el.eccentricity >= 1) {
    throw new EncodeError(`eccentricity ${el.eccentricity} is outside [0, 1), not a closed orbit`);
  }
  if (el.inclination < 0 || el.inclination > 180) {
    throw new EncodeError(`inclination ${el.inclination} deg is outside [0, 180]`);
  }
  const w = wOfMeanMotion(el.meanMotion);
  if (1 - el.eccentricity < w * (1 - 1e-9)) throw new EncodeError("perigee is below the 250 km floor");
  return [epoch, w];
}

/**
 * Checks that an element set can be encoded (section 7, step 1), without
 * encoding it. Throws {@link EncodeError} if it cannot.
 *
 * @param {Elements} el
 */
export function checkRange(el) {
  validate(el);
}

/**
 * Encodes mean elements as a profile 0 KepID (section 7), in its hyphenated
 * lower-case text form. Throws {@link EncodeError} for an element set that
 * step 1 refuses.
 *
 * @param {Elements} el
 * @returns {string}
 */
export function encode(el) {
  const [epoch, w] = validate(el);
  const t = tick(epoch);
  const dt = Number(t * 256000n - epoch);

  const kW = p.MEAN_MOTION.encode(w);
  const wHat = p.MEAN_MOTION.decode(kW);

  const kS = p.ECCENTRICITY.encode(el.eccentricity / (1 - wHat));
  const sHat = p.ECCENTRICITY.decode(kS);
  const eHat = (1 - wHat) * sHat;

  const kOmega = p.ARG_PERIGEE.encode(wrap(el.argPerigee));

  const lam = wrap((el.argPerigee + el.meanAnomaly) + ((360 * el.meanMotion) * dt) / 86400000000);
  const kLambda = p.LAMBDA.encode(lam);

  const kRaan = p.RAAN.encode(wrap(el.raan));
  const kI = p.INCLINATION.encode(el.inclination);

  const d = el.bstar * dragFactor(wHat, eHat);
  const kD = p.DRAG.encode(d);

  const x =
    put(t, p.EPOCH_BITS) |
    put(0b0111, p.VERSION_BITS) |
    put(0b10, p.VARIANT_BITS) |
    put(0, p.PROFILE_BITS) |
    put(kOmega, p.ARG_PERIGEE_BITS) |
    put(kI, p.INCLINATION_BITS) |
    put(kRaan, p.RAAN_BITS) |
    put(kD, p.DRAG_BITS) |
    put(kLambda, p.LAMBDA_BITS) |
    put(kW, p.MEAN_MOTION_BITS) |
    put(kS, p.ECCENTRICITY_BITS);
  return toText(x);
}

// ---- decoding ------------------------------------------------------------------

function toText(x) {
  const hex = x.toString(16).padStart(32, "0");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/** @param {KepIdLike} kepid */
function asBigInt(kepid) {
  if (typeof kepid === "bigint") {
    if (kepid >= 0n && kepid < 1n << 128n) return kepid;
  } else if (typeof kepid === "string") {
    const text = kepid.trim();
    const hyphenated = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
    if (hyphenated.test(text) || /^[0-9a-f]{32}$/i.test(text)) return BigInt(`0x${text.replaceAll("-", "")}`);
    throw new DecodeError(`${JSON.stringify(kepid)} is not a UUID`);
  }
  throw new TypeError("a KepID is its text form or a 128-bit BigInt");
}

/**
 * Decodes a profile 0 KepID into mean elements (section 8). Throws
 * {@link DecodeError} only for wrong version, variant or profile bits: every
 * other value decodes to a closed orbit above the floor.
 *
 * @param {KepIdLike} kepid
 * @returns {Elements}
 */
export function decode(kepid) {
  const x = asBigInt(kepid);
  if (get(x, p.VERSION_BITS) !== 0b0111) throw new DecodeError(`version is ${get(x, p.VERSION_BITS)}, expected 7`);
  if (get(x, p.VARIANT_BITS) !== 0b10) throw new DecodeError("variant bits are not 10");
  if (get(x, p.PROFILE_BITS) !== 0) throw new DecodeError("profile bit is 1: not a profile 0 KepID");

  const w = p.MEAN_MOTION.decode(get(x, p.MEAN_MOTION_BITS));
  const s = p.ECCENTRICITY.decode(get(x, p.ECCENTRICITY_BITS));
  const omega = wrap(p.ARG_PERIGEE.decode(get(x, p.ARG_PERIGEE_BITS)));
  const raan = p.RAAN.decode(get(x, p.RAAN_BITS));
  const lam = p.LAMBDA.decode(get(x, p.LAMBDA_BITS));
  const i = p.INCLINATION.decode(get(x, p.INCLINATION_BITS));
  const d = p.DRAG.decode(get(x, p.DRAG_BITS));

  const e = (1 - w) * s;
  return {
    epochUnixUs: BigInt(get(x, p.EPOCH_BITS)) * p.TICK_US,
    meanMotion: (p.N_FLOOR * w) * Math.sqrt(w),
    eccentricity: e,
    inclination: i,
    raan: wrap(raan),
    argPerigee: omega,
    meanAnomaly: wrap(lam - omega),
    bstar: d / dragFactor(w, e),
  };
}
