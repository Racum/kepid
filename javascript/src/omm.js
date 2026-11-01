// Orbit Mean-Elements Messages (CCSDS OMM), as plain objects.
//
// An OMM is an object from OMM keywords to values, the shape JSON.parse gives
// for CelesTrak's and Space-Track's JSON. Numbers may be numbers (CelesTrak)
// or strings (Space-Track). The output matches the Rust library's `Omm` key
// for key.

import { decode, encode } from "./codec.js";
import { iso8601UnixUs, ommEpoch } from "./epoch.js";

// Placeholders for the identity fields a KepID does not have, as in a TLE.
const CLASSIFICATION = "U";
const ELEMENT_SET = 999;
const REV_NUMBER = 99999;
const NUMBER = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;

/** The OMM cannot be read or written. */
export class OmmError extends Error {
  name = "OmmError";
}

/**
 * Reads the mean elements of an OMM. The identity keywords (catalog number,
 * name and so on) are not kept: a KepID cannot hold them.
 *
 * The optional header keywords are checked when present:
 * `MEAN_ELEMENT_THEORY` must be `SGP4`, `REF_FRAME` must be `TEME` and
 * `TIME_SYSTEM` must be `UTC`.
 *
 * @param {Record<string, unknown>} omm
 * @returns {import("./codec.js").Elements}
 */
export function ommToElements(omm) {
  for (const [key, expected] of [["MEAN_ELEMENT_THEORY", "SGP4"], ["REF_FRAME", "TEME"], ["TIME_SYSTEM", "UTC"]]) {
    const value = omm[key];
    if (value !== undefined && value !== null && String(value).trim().toUpperCase() !== expected) {
      throw new OmmError(`${key} is ${JSON.stringify(value)}, which a KepID cannot carry`);
    }
  }
  const epochText = omm.EPOCH;
  const epoch = typeof epochText === "string" ? iso8601UnixUs(epochText) : null;
  if (epoch === null) throw new OmmError(`EPOCH ${JSON.stringify(epochText)} is not an ISO 8601 UTC date-time`);
  return {
    epochUnixUs: epoch,
    meanMotion: number(omm, "MEAN_MOTION"),
    eccentricity: number(omm, "ECCENTRICITY"),
    inclination: number(omm, "INCLINATION"),
    raan: number(omm, "RA_OF_ASC_NODE"),
    argPerigee: number(omm, "ARG_OF_PERICENTER"),
    meanAnomaly: number(omm, "MEAN_ANOMALY"),
    bstar: number(omm, "BSTAR"),
  };
}

/**
 * Writes mean elements as an OMM, for the object with this NORAD catalog
 * number, or with no `NORAD_CAT_ID` when it is left out.
 *
 * The header keywords say what the elements are (`EARTH`, `TEME`, `UTC`,
 * `SGP4`), and the identity keywords are placeholders: classification `U`,
 * element set 999 and revolution number 99999. The derivatives of mean
 * motion are written as zero; SGP4 does not use them. Numbers keep their full
 * precision; the epoch is written to the microsecond.
 *
 * @param {import("./codec.js").Elements} el
 * @param {number} [catalog]
 * @returns {Record<string, string | number>}
 */
export function elementsToOmm(el, catalog) {
  const epochUs = typeof el.epochUnixUs === "bigint" ? el.epochUnixUs : BigInt(el.epochUnixUs);
  const epoch = ommEpoch(epochUs);
  if (epoch === null) throw new OmmError(`epoch ${epochUs} µs is past year 9999`);
  const wrap = (degrees) => {
    const r = degrees % 360;
    return r < 0 ? r + 360 : r;
  };
  return {
    CENTER_NAME: "EARTH",
    REF_FRAME: "TEME",
    TIME_SYSTEM: "UTC",
    MEAN_ELEMENT_THEORY: "SGP4",
    EPOCH: epoch,
    MEAN_MOTION: el.meanMotion,
    ECCENTRICITY: el.eccentricity,
    INCLINATION: el.inclination,
    RA_OF_ASC_NODE: wrap(el.raan),
    ARG_OF_PERICENTER: wrap(el.argPerigee),
    MEAN_ANOMALY: wrap(el.meanAnomaly),
    EPHEMERIS_TYPE: 0,
    CLASSIFICATION_TYPE: CLASSIFICATION,
    ...(catalog === undefined || catalog === null ? {} : { NORAD_CAT_ID: catalog }),
    ELEMENT_SET_NO: ELEMENT_SET,
    REV_AT_EPOCH: REV_NUMBER,
    BSTAR: el.bstar,
    MEAN_MOTION_DOT: 0,
    MEAN_MOTION_DDOT: 0,
  };
}

/**
 * Reads an OMM and encodes it. Throws {@link OmmError} for a malformed OMM
 * and EncodeError for elements that cannot be encoded.
 *
 * @param {Record<string, unknown>} omm
 * @returns {string}
 */
export function ommToKepId(omm) {
  return encode(ommToElements(omm));
}

/**
 * Decodes a KepID and writes it as an OMM, for the object with this NORAD
 * catalog number, or with none when it is left out. A KepID describes an
 * orbit, not an object, so the number comes from the caller.
 *
 * @param {import("./codec.js").KepIdLike} kepid
 * @param {number} [catalog]
 */
export function kepIdToOmm(kepid, catalog) {
  return elementsToOmm(decode(kepid), catalog);
}

/** A number, or a string holding one. */
function number(omm, key) {
  const value = omm[key];
  if (typeof value === "number") return value;
  if (typeof value === "string" && NUMBER.test(value.trim())) return Number(value.trim());
  throw new OmmError(`${key} is missing or not a number: ${JSON.stringify(value)}`);
}
