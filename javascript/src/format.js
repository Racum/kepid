// Exact decimal formatting of numbers, rounding halves to even.
//
// Number.prototype.toFixed rounds an exact tie up (1.40625 → "1.4063"), while
// Rust and Python round it to even ("1.4062"). Ties are real here: uniform
// codes, such as RAAN's, decode to exact binary fractions. These helpers work
// on the exact binary value with BigInt, so the TLE text matches the other
// libraries character for character.

/** The exact value of a finite, non-negative number, as [numerator, denominator]. */
function exact(x) {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, x);
  const bits = view.getBigUint64(0);
  const exponent = Number((bits >> 52n) & 0x7ffn);
  const fraction = bits & ((1n << 52n) - 1n);
  const mantissa = exponent === 0 ? fraction : fraction | (1n << 52n);
  const power = (exponent === 0 ? 1 : exponent) - 1075;
  return power >= 0 ? [mantissa << BigInt(power), 1n] : [mantissa, 1n << BigInt(-power)];
}

/** round(num / den) with halves to even, for non-negative BigInts. */
function divideHalfEven(num, den) {
  const q = num / den;
  const r2 = (num % den) * 2n;
  return r2 > den || (r2 === den && q % 2n === 1n) ? q + 1n : q;
}

/** Like Rust's `{:.digits}` and Python's `f"{x:.{digits}f}"`. */
export function toFixedExact(x, digits) {
  const sign = x < 0 || Object.is(x, -0) ? "-" : "";
  const [num, den] = exact(Math.abs(x));
  const scaled = divideHalfEven(num * 10n ** BigInt(digits), den).toString().padStart(digits + 1, "0");
  const whole = scaled.slice(0, scaled.length - digits);
  return digits ? `${sign}${whole}.${scaled.slice(-digits)}` : `${sign}${whole}`;
}

/**
 * Scientific notation with `digits` digits after the point, like Rust's
 * `{:.digits e}`, for a positive number: the digits without the point, and
 * the power of ten of the first one.
 */
export function toExponentialExact(x, digits) {
  const [num, den] = exact(x);
  let power = Math.floor(Math.log10(x));
  // Math.log10 can be one off near a power of ten; settle it exactly.
  const atLeast = (k) => (k >= 0 ? num >= den * 10n ** BigInt(k) : num * 10n ** BigInt(-k) >= den);
  while (!atLeast(power)) power -= 1;
  while (atLeast(power + 1)) power += 1;
  const shift = digits - power;
  let mantissa = shift >= 0 ? divideHalfEven(num * 10n ** BigInt(shift), den) : divideHalfEven(num, den * 10n ** BigInt(-shift));
  if (mantissa === 10n ** BigInt(digits + 1)) {
    mantissa /= 10n;
    power += 1;
  }
  return [mantissa.toString(), power];
}
