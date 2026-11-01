"""Encoding and decoding (``docs/spec.md``, sections 7 and 8)."""

from __future__ import annotations

import math
import uuid
from dataclasses import dataclass
from typing import Union

from . import profile0 as p
from .curve import clamp, wrap


@dataclass(frozen=True)
class Elements:
    """SGP4 mean elements, in the units a TLE uses (TEME frame).

    ``epoch_unix_us`` is a whole number of Unix microseconds, so the epoch
    tick never depends on float rounding; see :func:`tle_epoch_unix_us`.
    """

    epoch_unix_us: int
    #: Mean motion, rev/day.
    mean_motion: float
    eccentricity: float
    #: Inclination, degrees.
    inclination: float
    #: Right ascension of the ascending node, degrees.
    raan: float
    #: Argument of perigee, degrees.
    arg_perigee: float
    #: Mean anomaly, degrees.
    mean_anomaly: float
    #: SGP4 B* drag term, 1/Earth radii.
    bstar: float


class EncodeError(ValueError):
    """The element set cannot be encoded as a profile 0 KepID."""


class DecodeError(ValueError):
    """The value is not a profile 0 KepID."""


KepIdLike = Union[uuid.UUID, str, int]


# ---- shared formulas ---------------------------------------------------------


def cbrt(x: float) -> float:
    """The spec's cube root (section 2), not ``math.cbrt``: exact scaling by 8
    into [1, 8), then six Newton steps from 1.5."""
    m, scale = x, 1.0
    while m < 1:
        m = m * 8
        scale = scale / 2
    while m >= 8:
        m = m / 8
        scale = scale * 2
    y = 1.5
    for _ in range(6):
        y = (2 * y + m / (y * y)) / 3
    return y * scale


def w_of_mean_motion(n: float) -> float:
    """w = R_FLOOR / a = (n / N_FLOOR)^(2/3) (section 7, step 1)."""
    v = n / p.N_FLOOR
    return cbrt(clamp(v * v, 1e-30, 1e30))


def drag_factor(w: float, e: float) -> float:
    """drag(w, e) = g(w, e) / G_REF (section 6.3)."""
    q = p.C_SGP4 - p.S_SGP4 * w
    eta = (p.C_SGP4 * e) / q
    x = max(1 - eta * eta, 1e-12)
    w2 = w * w
    q2 = q * q
    g = ((w2 * w2) * w) / (q2 * q2) / (((x * x) * x) * math.sqrt(x))
    return g / p.G_REF


def _put(value: int, field: tuple) -> int:
    offset, width = field
    return (value & ((1 << width) - 1)) << (128 - offset - width)


def _get(x: int, field: tuple) -> int:
    offset, width = field
    return (x >> (128 - offset - width)) & ((1 << width) - 1)


# ---- encoding ------------------------------------------------------------------


def _validate(el: Elements) -> float:
    """Section 7, step 1. Returns w for step 3."""
    for name in ("mean_motion", "eccentricity", "inclination", "raan", "arg_perigee", "mean_anomaly", "bstar"):
        if not math.isfinite(getattr(el, name)):
            raise EncodeError(f"{name} is not a finite number")
    if not isinstance(el.epoch_unix_us, int):
        raise EncodeError("the epoch must be a whole number of microseconds")
    if el.epoch_unix_us < 0:
        raise EncodeError("epoch is before 1970-01-01")
    if _tick(el.epoch_unix_us) > p.MAX_EPOCH_TICKS:
        raise EncodeError("epoch is past the last representable tick (year 10889)")
    if el.mean_motion <= 0:
        raise EncodeError(f"mean motion {el.mean_motion} rev/day is not positive")
    if el.eccentricity < 0 or el.eccentricity >= 1:
        raise EncodeError(f"eccentricity {el.eccentricity} is outside [0, 1), not a closed orbit")
    if el.inclination < 0 or el.inclination > 180:
        raise EncodeError(f"inclination {el.inclination} deg is outside [0, 180]")
    w = w_of_mean_motion(el.mean_motion)
    if 1 - el.eccentricity < w * (1 - 1e-9):
        raise EncodeError("perigee is below the 250 km floor")
    return w


def _tick(epoch_us: int) -> int:
    """Section 7, step 2: the nearest tick, halves rounding up, in integers."""
    return (epoch_us + 128000) // 256000


def encode(el: Elements) -> uuid.UUID:
    """Encodes mean elements as a profile 0 KepID (section 7).

    Raises :class:`EncodeError` for an element set that step 1 refuses.
    """
    w = _validate(el)
    tick = _tick(el.epoch_unix_us)
    dt = float(tick * 256000 - el.epoch_unix_us)

    k_w = p.MEAN_MOTION.encode(w)
    w_hat = p.MEAN_MOTION.decode(k_w)

    k_s = p.ECCENTRICITY.encode(el.eccentricity / (1 - w_hat))
    s_hat = p.ECCENTRICITY.decode(k_s)
    e_hat = (1 - w_hat) * s_hat

    k_omega = p.ARG_PERIGEE.encode(wrap(el.arg_perigee))

    lam = wrap((el.arg_perigee + el.mean_anomaly) + ((360 * el.mean_motion) * dt) / 86400000000)
    k_lambda = p.LAMBDA.encode(lam)

    k_raan = p.RAAN.encode(wrap(el.raan))
    k_i = p.INCLINATION.encode(el.inclination)

    d = el.bstar * drag_factor(w_hat, e_hat)
    k_d = p.DRAG.encode(d)

    x = (
        _put(tick, p.EPOCH_BITS)
        | _put(0b0111, p.VERSION_BITS)
        | _put(0b10, p.VARIANT_BITS)
        | _put(0, p.PROFILE_BITS)
        | _put(k_omega, p.ARG_PERIGEE_BITS)
        | _put(k_i, p.INCLINATION_BITS)
        | _put(k_raan, p.RAAN_BITS)
        | _put(k_d, p.DRAG_BITS)
        | _put(k_lambda, p.LAMBDA_BITS)
        | _put(k_w, p.MEAN_MOTION_BITS)
        | _put(k_s, p.ECCENTRICITY_BITS)
    )
    return uuid.UUID(int=x)


# ---- decoding ------------------------------------------------------------------


def _as_int(kepid: KepIdLike) -> int:
    if isinstance(kepid, uuid.UUID):
        return kepid.int
    if isinstance(kepid, str):
        return uuid.UUID(kepid).int
    if isinstance(kepid, int) and 0 <= kepid < 1 << 128:
        return kepid
    raise TypeError("a KepID is a uuid.UUID, its text form or a 128-bit integer")


def decode(kepid: KepIdLike) -> Elements:
    """Decodes a profile 0 KepID into mean elements (section 8).

    Raises :class:`DecodeError` only for wrong version, variant or profile
    bits: every other value decodes to a closed orbit above the floor.
    """
    x = _as_int(kepid)
    if _get(x, p.VERSION_BITS) != 0b0111:
        raise DecodeError(f"version is {_get(x, p.VERSION_BITS)}, expected 7")
    if _get(x, p.VARIANT_BITS) != 0b10:
        raise DecodeError("variant bits are not 10")
    if _get(x, p.PROFILE_BITS) != 0:
        raise DecodeError("profile bit is 1: not a profile 0 KepID")

    w = p.MEAN_MOTION.decode(_get(x, p.MEAN_MOTION_BITS))
    s = p.ECCENTRICITY.decode(_get(x, p.ECCENTRICITY_BITS))
    omega = p.ARG_PERIGEE.decode(_get(x, p.ARG_PERIGEE_BITS))
    raan = p.RAAN.decode(_get(x, p.RAAN_BITS))
    lam = p.LAMBDA.decode(_get(x, p.LAMBDA_BITS))
    i = p.INCLINATION.decode(_get(x, p.INCLINATION_BITS))
    d = p.DRAG.decode(_get(x, p.DRAG_BITS))

    e = (1 - w) * s
    omega = wrap(omega)
    return Elements(
        epoch_unix_us=_get(x, p.EPOCH_BITS) * 256000,
        mean_motion=(p.N_FLOOR * w) * math.sqrt(w),
        eccentricity=e,
        inclination=i,
        raan=wrap(raan),
        arg_perigee=omega,
        mean_anomaly=wrap(lam - omega),
        bstar=d / drag_factor(w, e),
    )
