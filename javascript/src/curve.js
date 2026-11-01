// Curves and quantizers (docs/spec.md, sections 2, 5 and 6).
//
// Every formula is evaluated in the order the spec writes it, so encoding is
// bit-exact with other conforming implementations. JavaScript numbers are
// IEEE 754 binary64, and engines never fuse a multiply and an add.

/** round(x): to nearest, halves away from zero. */
export function roundHalfAway(x) {
  const a = Math.abs(x);
  const f = Math.floor(a);
  const r = a - f >= 0.5 ? f + 1 : f;
  return copysign(r, x);
}

export function clamp(x, lo, hi) {
  return Math.min(Math.max(x, lo), hi);
}

/** The Euclidean remainder: fmod(x, m), plus m when that is negative. */
export function mod(x, m) {
  const r = x % m;
  return r < 0 ? r + m : r;
}

/** mod(x, 360), with a result of exactly 360, or of -0, replaced by 0. */
export function wrap(x) {
  const r = mod(x, 360);
  return r === 360 || r === 0 ? 0 : r;
}

export function copysign(x, y) {
  const negative = y < 0 || Object.is(y, -0);
  return negative ? -Math.abs(x) : Math.abs(x);
}

// ---- curves (section 5) ------------------------------------------------------

function ss(u) {
  return u / (1 + Math.abs(u));
}

function layerForward([a, c, g], z) {
  const bigL = ss(-c / g);
  const bigM = ss((1 - c) / g) - bigL;
  const u = (z - c) / g;
  return (1 - a) * z + (a * (ss(u) - bigL)) / bigM;
}

function layerInverse([a, c, g], t) {
  const bigL = ss(-c / g);
  const bigM = ss((1 - c) / g) - bigL;
  const alpha = (1 - a) * g;
  const beta = a / bigM;
  const tau = (t - (1 - a) * c) + beta * bigL;
  const b = (alpha + beta) - Math.abs(tau);
  const u = (Math.sqrt(b * b + (4 * alpha) * Math.abs(tau)) - b) / (2 * alpha);
  return clamp(c + g * copysign(u, tau), 0, 1);
}

/** C(z): layer 1 first. No layers is the identity. */
export function forward(layers, z) {
  for (const layer of layers) z = layerForward(layer, z);
  return z;
}

/** C⁻¹(t): the last layer's inverse first. */
export function inverse(layers, t) {
  for (let i = layers.length - 1; i >= 0; i--) t = layerInverse(layers[i], t);
  return t;
}

// ---- quantizers (section 6) --------------------------------------------------

/**
 * One field of the layout: its width, its kind and its curve. `kind` is
 * `{ interval: [lo, width] }`, `{ circular: lo }` or `{ signed: max }`.
 */
export class FieldMap {
  constructor(bits, kind, layers) {
    this.bits = bits;
    this.kind = kind;
    this.layers = layers;
    this.codes = 2 ** bits;
  }

  encode(x) {
    const n = this.codes;
    const { interval, circular, signed } = this.kind;
    if (interval) {
      const [lo, width] = interval;
      const z = clamp((x - lo) / width, 0, 1);
      const t = forward(this.layers, z);
      return clamp(Math.floor(t * n), 0, n - 1);
    }
    if (circular !== undefined) {
      const z = mod(x - circular, 360) / 360;
      const t = forward(this.layers, z);
      return mod(roundHalfAway(t * n), n);
    }
    const z = Math.min(Math.abs(x) / signed, 1);
    const t = copysign(forward(this.layers, z), x);
    return clamp(roundHalfAway((t * n) / 2), -n / 2, n / 2 - 1) + n / 2;
  }

  decode(k) {
    const n = this.codes;
    const { interval, circular, signed } = this.kind;
    if (interval) {
      const [lo, width] = interval;
      const t = clamp((k + 0.5) / n, 0, 1);
      return lo + width * inverse(this.layers, t);
    }
    if (circular !== undefined) {
      return circular + 360 * inverse(this.layers, k / n);
    }
    const t = clamp((k - n / 2) / (n / 2), -1, 1);
    return signed * copysign(inverse(this.layers, Math.abs(t)) - inverse(this.layers, 0), t);
  }
}
