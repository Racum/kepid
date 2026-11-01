"""Curves and quantizers (``docs/spec.md``, sections 2, 5 and 6).

Every formula is evaluated in the order the spec writes it, so encoding is
bit-exact with other conforming implementations.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Sequence, Tuple, Union

Layer = Tuple[float, float, float]


# ---- the functions of section 2 ---------------------------------------------


def round_half_away(x: float) -> float:
    """round(x): to nearest, halves away from zero (not Python's round)."""
    a = abs(x)
    f = math.floor(a)
    r = f + 1 if a - f >= 0.5 else f
    return math.copysign(r, x)


def clamp(x: float, lo: float, hi: float) -> float:
    return min(max(x, lo), hi)


def mod(x: float, m: float) -> float:
    """The Euclidean remainder: fmod(x, m), plus m when that is negative."""
    r = math.fmod(x, m)
    return r + m if r < 0 else r


def wrap(x: float) -> float:
    """mod(x, 360), with a result of exactly 360, or of -0, replaced by 0."""
    r = mod(x, 360.0)
    return 0.0 if r == 360.0 or r == 0.0 else r


# ---- curves (section 5) -----------------------------------------------------


def _ss(u: float) -> float:
    return u / (1 + abs(u))


def _layer_forward(layer: Layer, z: float) -> float:
    a, c, g = layer
    big_l = _ss(-c / g)
    big_m = _ss((1 - c) / g) - big_l
    u = (z - c) / g
    return (1 - a) * z + (a * (_ss(u) - big_l)) / big_m


def _layer_inverse(layer: Layer, t: float) -> float:
    a, c, g = layer
    big_l = _ss(-c / g)
    big_m = _ss((1 - c) / g) - big_l
    alpha = (1 - a) * g
    beta = a / big_m
    tau = (t - (1 - a) * c) + beta * big_l
    b = (alpha + beta) - abs(tau)
    u = (math.sqrt(b * b + (4 * alpha) * abs(tau)) - b) / (2 * alpha)
    return clamp(c + g * math.copysign(u, tau), 0.0, 1.0)


def forward(layers: Sequence[Layer], z: float) -> float:
    """C(z): layer 1 first. No layers is the identity."""
    for layer in layers:
        z = _layer_forward(layer, z)
    return z


def inverse(layers: Sequence[Layer], t: float) -> float:
    """C⁻¹(t): the last layer's inverse first."""
    for layer in reversed(layers):
        t = _layer_inverse(layer, t)
    return t


# ---- quantizers (section 6) -------------------------------------------------


@dataclass(frozen=True)
class Interval:
    """Domain [lo, lo + width]; codes are floor(t N), decoded at the code centre."""

    lo: float
    width: float


@dataclass(frozen=True)
class Circular:
    """360 degrees starting at lo; codes are mod(round(t N), N)."""

    lo: float


@dataclass(frozen=True)
class Signed:
    """Sign and magnitude over |x| <= max, with an exact zero code N/2."""

    max: float


Kind = Union[Interval, Circular, Signed]


@dataclass(frozen=True)
class FieldMap:
    """One field of the layout: its width, its kind and its curve."""

    bits: int
    kind: Kind
    layers: Tuple[Layer, ...]

    @property
    def codes(self) -> int:
        return 1 << self.bits

    def encode(self, x: float) -> int:
        n = self.codes
        kind = self.kind
        if isinstance(kind, Interval):
            z = clamp((x - kind.lo) / kind.width, 0.0, 1.0)
            t = forward(self.layers, z)
            return int(clamp(math.floor(t * n), 0, n - 1))
        if isinstance(kind, Circular):
            z = mod(x - kind.lo, 360.0) / 360.0
            t = forward(self.layers, z)
            return int(mod(round_half_away(t * n), n))
        z = min(abs(x) / kind.max, 1.0)
        t = math.copysign(forward(self.layers, z), x)
        return int(clamp(round_half_away((t * n) / 2), -n / 2, n / 2 - 1) + n / 2)

    def decode(self, k: int) -> float:
        n = self.codes
        kind = self.kind
        if isinstance(kind, Interval):
            t = clamp((k + 0.5) / n, 0.0, 1.0)
            return kind.lo + kind.width * inverse(self.layers, t)
        if isinstance(kind, Circular):
            t = k / n
            return kind.lo + 360 * inverse(self.layers, t)
        t = clamp((k - n / 2) / (n / 2), -1.0, 1.0)
        return kind.max * math.copysign(inverse(self.layers, abs(t)) - inverse(self.layers, 0.0), t)
