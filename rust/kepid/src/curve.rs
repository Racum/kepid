//! The layered curves and the three quantizers of profile 0
//! (`docs/spec.md`, sections 5 and 6).
//!
//! Every formula is evaluated in the order the spec writes it, so encoding is
//! bit-exact across implementations.

/// One curve layer: share `a`, centre `c`, width `g`.
pub type Layer = (f64, f64, f64);

fn ss(u: f64) -> f64 {
    u / (1.0 + u.abs())
}

fn layer_forward(&(a, c, g): &Layer, z: f64) -> f64 {
    let l = ss(-c / g);
    let m = ss((1.0 - c) / g) - l;
    let u = (z - c) / g;
    (1.0 - a) * z + a * (ss(u) - l) / m
}

/// Closed-form inverse: with z = c + g u the layer reads
/// `alpha u + beta u / (1 + |u|) = tau`, so |u| is the positive root of
/// `alpha U^2 + (alpha + beta - |tau|) U - |tau| = 0` and u has the sign of tau.
fn layer_inverse(&(a, c, g): &Layer, t: f64) -> f64 {
    let l = ss(-c / g);
    let m = ss((1.0 - c) / g) - l;
    let alpha = (1.0 - a) * g;
    let beta = a / m;
    let tau = t - (1.0 - a) * c + beta * l;
    let at = tau.abs();
    let b = alpha + beta - at;
    let u = ((b * b + 4.0 * alpha * at).sqrt() - b) / (2.0 * alpha);
    (c + g * u.copysign(tau)).clamp(0.0, 1.0)
}

/// C(z): the layers applied in order. No layers is the identity.
pub fn forward(layers: &[Layer], z: f64) -> f64 {
    layers.iter().fold(z, |z, l| layer_forward(l, z))
}

/// C⁻¹(t): the layer inverses applied in reverse order.
pub fn inverse(layers: &[Layer], t: f64) -> f64 {
    layers.iter().rev().fold(t, |t, l| layer_inverse(l, t))
}

/// How a field turns its value into a code.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// `[lo, lo + width]`; codes are floor(t N), decoded at the code centre.
    Interval { lo: f64, width: f64 },
    /// 360 degrees starting at `lo`; codes are round(t N) mod N.
    Circular { lo: f64 },
    /// Sign and magnitude over `|x| <= max`, with an exact zero code N/2.
    Signed { max: f64 },
}

/// One field of the layout: its width, its kind and its curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldMap {
    pub bits: u32,
    pub kind: Kind,
    pub layers: &'static [Layer],
}

impl FieldMap {
    /// Number of codes, 2^bits.
    pub fn codes(&self) -> u32 {
        1 << self.bits
    }

    /// Value to code. Values outside the domain saturate at the edge codes.
    pub fn encode(&self, x: f64) -> u32 {
        let n = f64::from(self.codes());
        let k = match self.kind {
            Kind::Interval { lo, width } => {
                let z = ((x - lo) / width).clamp(0.0, 1.0);
                (forward(self.layers, z) * n).floor().clamp(0.0, n - 1.0)
            }
            Kind::Circular { lo } => {
                let z = (x - lo).rem_euclid(360.0) / 360.0;
                (forward(self.layers, z) * n).round().rem_euclid(n)
            }
            Kind::Signed { max } => {
                let z = (x.abs() / max).min(1.0);
                let t = forward(self.layers, z).copysign(x);
                (t * n / 2.0).round().clamp(-n / 2.0, n / 2.0 - 1.0) + n / 2.0
            }
        };
        k as u32
    }

    /// Code to value.
    pub fn decode(&self, code: u32) -> f64 {
        let (n, k) = (f64::from(self.codes()), f64::from(code));
        match self.kind {
            Kind::Interval { lo, width } => lo + width * inverse(self.layers, ((k + 0.5) / n).clamp(0.0, 1.0)),
            Kind::Circular { lo } => lo + 360.0 * inverse(self.layers, k / n),
            Kind::Signed { max } => {
                let t = ((k - n / 2.0) / (n / 2.0)).clamp(-1.0, 1.0);
                // Subtracting C⁻¹(0) makes the zero code decode to exactly zero.
                max * (inverse(self.layers, t.abs()) - inverse(self.layers, 0.0)).copysign(t)
            }
        }
    }

    /// Size of one code at `x`, in the field's units (for display and tests).
    pub fn step(&self, x: f64) -> f64 {
        let (n, eps) = (f64::from(self.codes()), 1e-7);
        let (z, width) = match self.kind {
            Kind::Interval { lo, width } => (((x - lo) / width).clamp(0.0, 1.0), width),
            Kind::Circular { lo } => ((x - lo).rem_euclid(360.0) / 360.0, 360.0),
            Kind::Signed { max } => ((x.abs() / max).min(1.0), 2.0 * max),
        };
        let (z0, z1) = ((z - eps).max(0.0), (z + eps).min(1.0));
        let slope = (forward(self.layers, z1) - forward(self.layers, z0)) / (z1 - z0);
        width / (n * slope)
    }
}
