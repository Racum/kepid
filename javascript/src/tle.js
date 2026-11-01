// Two-line element sets, read and written as text, with no SGP4 dependency.
//
// A TLE is fixed-column text; reading one is slicing and exact decimal
// parsing, and SGP4 only ever consumes the numbers. The output matches the
// Rust and Python libraries character for character.

import { decode, encode } from "./codec.js";
import { tleEpochField, tleEpochUnixUs } from "./epoch.js";
import { toExponentialExact, toFixedExact } from "./format.js";

const LINE_LEN = 69;
// The largest catalog number Alpha-5 can write, Z9999.
const MAX_CATALOG = 339999;
// Alpha-5 letters, for 10 to 33; I and O are skipped.
const ALPHA5 = "ABCDEFGHJKLMNPQRSTUVWXYZ";
// Placeholders for the identity fields a KepID does not have.
const CLASSIFICATION = "U";
const ELEMENT_SET = 999;
const REV_NUMBER = 99999;

/** The TLE cannot be read or written. */
export class TleError extends Error {
  name = "TleError";
}

/**
 * Reads the mean elements of a TLE. The identity fields (catalog number,
 * designator and so on) are checked but not kept: a KepID cannot hold them.
 *
 * The first derivative of mean motion, the second derivative and the
 * ephemeris type are checked only by the checksum: SGP4 does not use them.
 *
 * @param {string} line1
 * @param {string} line2
 * @returns {import("./codec.js").Elements}
 */
export function tleToElements(line1, line2) {
  const l1 = checkLine(line1, 1);
  const l2 = checkLine(line2, 2);
  const catalog = readCatalog(l1.slice(2, 7));
  if (catalog === null) throw new TleError("line 1: cannot read catalog number");
  if (readCatalog(l2.slice(2, 7)) !== catalog) throw new TleError("the two lines have different catalog numbers");

  const number = (text, line, name) => {
    text = text.trim();
    const value = /^[0-9.+-]+$/.test(text) ? Number(text) : NaN;
    if (Number.isNaN(value)) throw new TleError(`line ${line}: cannot read ${name}`);
    return value;
  };

  const eccentricity = l2.slice(26, 33);
  if (!/^\d{7}$/.test(eccentricity)) throw new TleError("line 2: cannot read eccentricity");
  const epoch = tleEpochUnixUs(l1.slice(18, 32));
  if (epoch === null) throw new TleError("line 1: cannot read epoch");
  const bstar = readExponent(l1.slice(53, 61));
  if (bstar === null) throw new TleError("line 1: cannot read B*");
  return {
    epochUnixUs: epoch,
    meanMotion: number(l2.slice(52, 63), 2, "mean motion"),
    eccentricity: Number(`0.${eccentricity}`),
    inclination: number(l2.slice(8, 16), 2, "inclination"),
    raan: number(l2.slice(17, 25), 2, "RAAN"),
    argPerigee: number(l2.slice(34, 42), 2, "argument of perigee"),
    meanAnomaly: number(l2.slice(43, 51), 2, "mean anomaly"),
    bstar,
  };
}

/**
 * Writes mean elements as a TLE, for the object with this NORAD catalog
 * number, or with catalog number 00000 when it is left out.
 *
 * The other identity fields are placeholders: classification `U`, a blank
 * designator, element set 999 and revolution number 99999. The derivatives
 * of mean motion are written as zero; SGP4 does not use them. Angles are
 * wrapped to [0, 360) and every field is rounded to its column width, so the
 * epoch moves by at most 432 µs.
 *
 * @param {import("./codec.js").Elements} el
 * @param {number} [catalog]
 * @returns {[string, string]}
 */
export function elementsToTle(el, catalog = 0) {
  const catalogText = writeCatalog(catalog);
  if (catalogText === null) throw new TleError(`catalog number ${catalog} is above 339999, the Alpha-5 limit`);
  const epochUs = typeof el.epochUnixUs === "bigint" ? el.epochUnixUs : BigInt(el.epochUnixUs);
  const epoch = tleEpochField(epochUs);
  if (epoch === null) throw new TleError(`epoch ${epochUs} µs is outside 1957-2056, the years a TLE can write`);
  const bstar = writeExponent(el.bstar);
  if (bstar === null) throw new TleError("line 1: cannot write B*");
  const eccentricity = Math.min(Math.max(Math.round(el.eccentricity * 1e7), 0), 9999999);
  const meanMotion = toFixedExact(el.meanMotion, 8).padStart(11);
  if (meanMotion.length !== 11) throw new TleError("line 2: cannot write mean motion");

  const line1 = `1 ${catalogText}${CLASSIFICATION}          ${epoch}  .00000000  00000+0 ${bstar} 0 ${String(ELEMENT_SET).padStart(4)}`;
  const line2 =
    `2 ${catalogText} ${angle(el.inclination)} ${angle(el.raan)} ${String(eccentricity).padStart(7, "0")} ` +
    `${angle(el.argPerigee)} ${angle(el.meanAnomaly)} ${meanMotion}${String(REV_NUMBER).padStart(5)}`;
  return [line1 + checksum(line1), line2 + checksum(line2)];
}

/**
 * Reads a TLE and encodes it. Throws {@link TleError} for a malformed TLE and
 * EncodeError for elements that cannot be encoded.
 *
 * @param {string} line1
 * @param {string} line2
 * @returns {string}
 */
export function tleToKepId(line1, line2) {
  return encode(tleToElements(line1, line2));
}

/**
 * Decodes a KepID and writes it as a TLE, for the object with this NORAD
 * catalog number (0 to 339999; Alpha-5 above 99999), or with 00000 when it is
 * left out. A KepID describes an orbit, not an object, so the number comes
 * from the caller.
 *
 * @param {import("./codec.js").KepIdLike} kepid
 * @param {number} [catalog]
 * @returns {[string, string]}
 */
export function kepIdToTle(kepid, catalog) {
  return elementsToTle(decode(kepid), catalog);
}

function checkLine(line, number) {
  line = line.trimEnd();
  if (line.length !== LINE_LEN || !/^[\x00-\x7f]*$/.test(line)) {
    throw new TleError(`line ${number} has ${[...line].length} characters, expected 69`);
  }
  if (line[0] !== String(number) || line[1] !== " ") throw new TleError(`line ${number} does not start with "${number} "`);
  const found = (line.charCodeAt(68) - 48 + 256) % 256;
  const expected = checksum(line.slice(0, 68));
  if (found !== expected) throw new TleError(`line ${number} checksum is ${found}, expected ${expected}`);
  return line;
}

/** Sum of the digits, with each minus sign counting 1, mod 10. */
function checksum(text) {
  let sum = 0;
  for (const c of text) sum += c >= "0" && c <= "9" ? Number(c) : c === "-" ? 1 : 0;
  return sum % 10;
}

/** Five digits, or Alpha-5: a letter for 10-33, then four digits. */
export function readCatalog(text) {
  text = text.replace(/^ +/, "");
  if (!text) return null;
  let head = 0;
  let digits = text;
  if (text[0] >= "A" && text[0] <= "Z") {
    if (!ALPHA5.includes(text[0])) return null;
    head = 10 + ALPHA5.indexOf(text[0]);
    digits = text.slice(1);
  }
  if (!/^\d+$/.test(digits) || (head && digits.length !== 4)) return null;
  return head * 10000 + Number(digits);
}

export function writeCatalog(catalog) {
  if (!Number.isInteger(catalog) || catalog < 0 || catalog > MAX_CATALOG) return null;
  if (catalog <= 99999) return String(catalog).padStart(5, "0");
  return ALPHA5[Math.floor(catalog / 10000) - 10] + String(catalog % 10000).padStart(4, "0");
}

/** An implied-decimal exponent field: ` 11132-3` is 0.11132e-3. */
export function readExponent(text) {
  const sign = { " ": "", "+": "", "-": "-" }[text[0]];
  const mantissa = text.slice(1, 6);
  const exponent = text.slice(6, 8);
  if (sign === undefined || !/^[0-9 ]{5}$/.test(mantissa) || !/^[+-]\d$/.test(exponent)) return null;
  return Number(`${sign}0.${mantissa.replaceAll(" ", "0")}e${exponent}`);
}

/** Sign, five digits, exponent. Values too small for exponent -9 are written as zero. */
export function writeExponent(x) {
  if (x === 0) return " 00000+0";
  const sign = x < 0 ? "-" : " ";
  const [digits, power] = toExponentialExact(Math.abs(x), 4);
  const exponent = power + 1;
  if (exponent < -9) return " 00000+0";
  if (exponent > 9) return null;
  return `${sign}${digits}${exponent < 0 ? "-" : "+"}${Math.abs(exponent)}`;
}

/** Four decimals in [0, 360), so 359.99996 writes as 0. */
export function angle(degrees) {
  let wrapped = degrees % 360;
  if (wrapped < 0) wrapped += 360;
  const text = toFixedExact(wrapped, 4).padStart(8);
  return text === "360.0000" ? "  0.0000" : text;
}
