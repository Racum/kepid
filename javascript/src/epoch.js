// Exact epoch input and output (docs/spec.md, section 7, "Epoch input").
// Epochs are whole Unix microseconds, as BigInts; nothing here goes through
// floating point.

const US_PER_DAY = 86400000000n;
// One unit of a TLE epoch's 8-digit day fraction, 10⁻⁸ day, in microseconds.
const US_PER_TLE_DIGIT = 864n;

function isLeap(year) {
  return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
}

/** Floor division for BigInts (`/` truncates toward zero). */
function floorDiv(a, b) {
  const q = a / b;
  return a % b !== 0n && (a < 0n) !== (b < 0n) ? q - 1n : q;
}

/** (year, month, day) to days since 1970-01-01, proleptic Gregorian. */
export function daysFromCivil(y, m, d) {
  y = m <= 2 ? y - 1 : y;
  const era = Math.floor(y / 400);
  const yoe = y - era * 400;
  const mp = m > 2 ? m - 3 : m + 9;
  const doy = Math.floor((153 * mp + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146097 + doe - 719468;
}

/** Days since 1970-01-01 to [year, month, day], proleptic Gregorian. */
export function civilFromDays(days) {
  const z = days + 719468;
  const era = Math.floor(z / 146097);
  const doe = z - era * 146097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
  const mp = Math.floor((5 * doy + 2) / 153);
  const d = doy - Math.floor((153 * mp + 2) / 5) + 1;
  const m = mp < 10 ? mp + 3 : mp - 9;
  return [yoe + era * 400 + (m <= 2 ? 1 : 0), m, d];
}

/**
 * Converts a TLE epoch field, `YYDDD.DDDDDDDD` (line 1, columns 19-32), to
 * Unix microseconds, exactly.
 *
 * Years 57-99 are 1957-1999 and 00-56 are 2000-2056. Day 1 is 1 January. The
 * fraction may have up to 8 digits; each 10⁻⁸ day is exactly 864 µs. Returns
 * `null` for anything else.
 *
 * @param {string} field
 * @returns {bigint | null}
 */
export function tleEpochUnixUs(field) {
  const match = /^(\d{2})(\d{3})(?:\.(\d{0,8}))?$/.exec(field.trim());
  if (!match) return null;
  const yy = Number(match[1]);
  const day = Number(match[2]);
  const year = yy < 57 ? 2000 + yy : 1900 + yy;
  if (day < 1 || day > 365 + (isLeap(year) ? 1 : 0)) return null;
  const units = BigInt((match[3] ?? "").padEnd(8, "0"));
  return BigInt(daysFromCivil(year, 1, 1) + day - 1) * US_PER_DAY + units * US_PER_TLE_DIGIT;
}

/**
 * Formats Unix microseconds as a TLE epoch field, `YYDDD.DDDDDDDD`, rounding
 * to the nearest 10⁻⁸ day (864 µs; a tie rounds up). Returns `null` outside
 * 1957-2056, the years a two-digit TLE year covers.
 *
 * @param {bigint} unixUs
 */
export function tleEpochField(unixUs) {
  const units = floorDiv(unixUs + US_PER_TLE_DIGIT / 2n, US_PER_TLE_DIGIT);
  const unitsPerDay = US_PER_DAY / US_PER_TLE_DIGIT;
  const days = floorDiv(units, unitsPerDay);
  const fraction = units - days * unitsPerDay;
  const [year] = civilFromDays(Number(days));
  if (year < 1957 || year > 2056) return null;
  const day = Number(days) - daysFromCivil(year, 1, 1) + 1;
  return `${String(year % 100).padStart(2, "0")}${String(day).padStart(3, "0")}.${String(fraction).padStart(8, "0")}`;
}

const ISO8601 = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d*))?Z?$/;

/**
 * Converts an ISO 8601 UTC date-time, `YYYY-MM-DDThh:mm:ss[.f…][Z]` (an OMM
 * `EPOCH`), to Unix microseconds, exactly.
 *
 * The fraction is read to 6 digits; more digits round to the microsecond on
 * their decimal digits, halves up. Returns `null` for anything else,
 * including leap seconds.
 *
 * @param {string} text
 * @returns {bigint | null}
 */
export function iso8601UnixUs(text) {
  const match = ISO8601.exec(text.trim());
  if (!match) return null;
  const [y, m, d, h, min, s] = match.slice(1, 7).map(Number);
  const fraction = match[7] ?? "";
  const monthDays = [31, isLeap(y) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  if (m < 1 || m > 12 || d < 1 || d > monthDays[m - 1] || h > 23 || min > 59 || s > 59) return null;
  let us = BigInt(fraction.slice(0, 6).padEnd(6, "0"));
  if (fraction.length > 6 && fraction[6] >= "5") us += 1n;
  return BigInt(daysFromCivil(y, m, d)) * US_PER_DAY + BigInt((h * 60 + min) * 60 + s) * 1000000n + us;
}

/**
 * Formats Unix microseconds as `YYYY-MM-DDThh:mm:ss.ffffff`, the form
 * CelesTrak uses for OMM `EPOCH`. Returns `null` outside years 0-9999.
 *
 * @param {bigint} unixUs
 */
export function ommEpoch(unixUs) {
  const days = floorDiv(unixUs, US_PER_DAY);
  const rem = unixUs - days * US_PER_DAY;
  const [y, m, d] = civilFromDays(Number(days));
  if (y < 0 || y > 9999) return null;
  const seconds = Number(rem / 1000000n);
  const us = rem % 1000000n;
  const pad = (v, n) => String(v).padStart(n, "0");
  const clock = `${pad(Math.floor(seconds / 3600), 2)}:${pad(Math.floor(seconds / 60) % 60, 2)}:${pad(seconds % 60, 2)}`;
  return `${pad(y, 4)}-${pad(m, 2)}-${pad(d, 2)}T${clock}.${pad(us, 6)}`;
}
