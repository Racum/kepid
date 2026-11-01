"""Two-line element sets, read and written as text, with no SGP4 dependency.

A TLE is fixed-column text; reading one is slicing and exact decimal parsing,
and SGP4 only ever consumes the numbers. Every number is parsed from its
decimal text, so a value always gets the nearest float. The output matches
the Rust library character for character.
"""

from __future__ import annotations

import math
import uuid
from typing import Optional, Tuple

from .codec import Elements, KepIdLike, decode, encode
from .epoch import tle_epoch_field, tle_epoch_unix_us

_LINE_LEN = 69
# The largest catalog number Alpha-5 can write, Z9999.
_MAX_CATALOG = 339_999
# Alpha-5 letters, for 10 to 33; I and O are skipped.
_ALPHA5 = "ABCDEFGHJKLMNPQRSTUVWXYZ"
# Placeholders for the identity fields a KepID does not have.
_CLASSIFICATION = "U"
_ELEMENT_SET = 999
_REV_NUMBER = 99_999
_NUMBER_CHARS = set("0123456789.-+")


class TleError(ValueError):
    """The TLE cannot be read or written."""


def tle_to_elements(line1: str, line2: str) -> Elements:
    """Reads the mean elements of a TLE. The identity fields (catalog number,
    designator and so on) are checked but not kept: a KepID cannot hold them.

    The first derivative of mean motion, the second derivative and the
    ephemeris type are checked only by the checksum: SGP4 does not use them.
    """
    l1 = _check_line(line1, 1)
    l2 = _check_line(line2, 2)
    catalog = _read_catalog(l1[2:7])
    if catalog is None:
        raise TleError("line 1: cannot read catalog number")
    if _read_catalog(l2[2:7]) != catalog:
        raise TleError("the two lines have different catalog numbers")

    def number(text: str, line: int, name: str) -> float:
        text = text.strip()
        if not text or not set(text) <= _NUMBER_CHARS:
            raise TleError(f"line {line}: cannot read {name}")
        try:
            return float(text)
        except ValueError:
            raise TleError(f"line {line}: cannot read {name}") from None

    eccentricity = l2[26:33]
    if not (eccentricity.isascii() and eccentricity.isdigit()):
        raise TleError("line 2: cannot read eccentricity")
    epoch = tle_epoch_unix_us(l1[18:32])
    if epoch is None:
        raise TleError("line 1: cannot read epoch")
    bstar = _read_exponent(l1[53:61])
    if bstar is None:
        raise TleError("line 1: cannot read B*")
    return Elements(
        epoch_unix_us=epoch,
        mean_motion=number(l2[52:63], 2, "mean motion"),
        eccentricity=float("0." + eccentricity),
        inclination=number(l2[8:16], 2, "inclination"),
        raan=number(l2[17:25], 2, "RAAN"),
        arg_perigee=number(l2[34:42], 2, "argument of perigee"),
        mean_anomaly=number(l2[43:51], 2, "mean anomaly"),
        bstar=bstar,
    )


def elements_to_tle(el: Elements, catalog: Optional[int] = None) -> Tuple[str, str]:
    """Writes mean elements as a TLE, for the object with this NORAD catalog
    number, or with catalog number 00000 when it is ``None``.

    The other identity fields are placeholders: classification ``U``, a blank
    designator, element set 999 and revolution number 99999. The derivatives
    of mean motion are written as zero; SGP4 does not use them. Angles are
    wrapped to [0, 360) and every field is rounded to its column width, so the
    epoch moves by at most 432 µs.
    """
    catalog = 0 if catalog is None else catalog
    catalog_text = _write_catalog(catalog)
    if catalog_text is None:
        raise TleError(f"catalog number {catalog} is above 339999, the Alpha-5 limit")
    epoch = tle_epoch_field(el.epoch_unix_us)
    if epoch is None:
        raise TleError(f"epoch {el.epoch_unix_us} µs is outside 1957-2056, the years a TLE can write")
    bstar = _write_exponent(el.bstar)
    if bstar is None:
        raise TleError("line 1: cannot write B*")
    eccentricity = min(max(_round_half_away(el.eccentricity * 1e7), 0), 9_999_999)
    mean_motion = f"{el.mean_motion:11.8f}"
    if len(mean_motion) != 11:
        raise TleError("line 2: cannot write mean motion")

    line1 = f"1 {catalog_text}{_CLASSIFICATION}          {epoch}  .00000000  00000+0 {bstar} 0 {_ELEMENT_SET:>4}"
    line2 = (
        f"2 {catalog_text} {_angle(el.inclination)} {_angle(el.raan)} {eccentricity:07} "
        f"{_angle(el.arg_perigee)} {_angle(el.mean_anomaly)} {mean_motion}{_REV_NUMBER:>5}"
    )
    return line1 + str(_checksum(line1)), line2 + str(_checksum(line2))


def tle_to_kepid(line1: str, line2: str) -> uuid.UUID:
    """Reads a TLE and encodes it. Raises :class:`TleError` for a malformed
    TLE and :class:`EncodeError` for elements that cannot be encoded."""
    return encode(tle_to_elements(line1, line2))


def kepid_to_tle(kepid: KepIdLike, catalog: Optional[int] = None) -> Tuple[str, str]:
    """Decodes a KepID and writes it as a TLE, for the object with this NORAD
    catalog number (0 to 339999; Alpha-5 above 99999), or with 00000 when it
    is ``None``. A KepID describes an orbit, not an object, so the number
    comes from the caller."""
    return elements_to_tle(decode(kepid), catalog)


def _check_line(line: str, number: int) -> str:
    line = line.rstrip()
    if len(line) != _LINE_LEN or not line.isascii():
        raise TleError(f"line {number} has {len(line)} characters, expected 69")
    if line[0] != str(number) or line[1] != " ":
        raise TleError(f'line {number} does not start with "{number} "')
    found = (ord(line[68]) - ord("0")) % 256
    expected = _checksum(line[:68])
    if found != expected:
        raise TleError(f"line {number} checksum is {found}, expected {expected}")
    return line


def _checksum(text: str) -> int:
    """Sum of the digits, with each minus sign counting 1, mod 10."""
    return sum(int(c) if c.isdigit() else 1 if c == "-" else 0 for c in text) % 10


def _read_catalog(text: str) -> Optional[int]:
    """Five digits, or Alpha-5: a letter for 10-33, then four digits."""
    text = text.lstrip(" ")
    if not text:
        return None
    head, digits = 0, text
    if "A" <= text[0] <= "Z":
        if text[0] not in _ALPHA5:
            return None
        head, digits = 10 + _ALPHA5.index(text[0]), text[1:]
    if not digits or not (digits.isascii() and digits.isdigit()) or (head and len(digits) != 4):
        return None
    return head * 10_000 + int(digits)


def _write_catalog(catalog: int) -> Optional[str]:
    if 0 <= catalog <= 99_999:
        return f"{catalog:05}"
    if 100_000 <= catalog <= _MAX_CATALOG:
        return f"{_ALPHA5[catalog // 10_000 - 10]}{catalog % 10_000:04}"
    return None


def _read_exponent(text: str) -> Optional[float]:
    """An implied-decimal exponent field: `` 11132-3`` is 0.11132e-3."""
    sign = {" ": "", "+": "", "-": "-"}.get(text[0])
    mantissa, exponent = text[1:6], text[6:8]
    if sign is None or not set(mantissa) <= set("0123456789 "):
        return None
    if exponent[0] not in "+-" or not (exponent[1].isascii() and exponent[1].isdigit()):
        return None
    return float(f"{sign}0.{mantissa.replace(' ', '0')}e{exponent}")


def _write_exponent(x: float) -> Optional[str]:
    """Sign, five digits, exponent. Values too small for exponent -9 are
    written as zero."""
    sign = "-" if x < 0 else " "
    digits, _, exponent = f"{abs(x):.4e}".partition("e")
    exponent = int(exponent) + 1
    if x == 0 or exponent < -9:
        return " 00000+0"
    if exponent > 9:
        return None
    return f"{sign}{digits.replace('.', '')}{'-' if exponent < 0 else '+'}{abs(exponent)}"


def _angle(degrees: float) -> str:
    """Four decimals in [0, 360), so 359.99996 writes as 0."""
    text = f"{degrees % 360.0:8.4f}"
    return "  0.0000" if text == "360.0000" else text


def _round_half_away(x: float) -> int:
    """Rounds halves away from zero, like Rust's ``f64::round``."""
    whole = math.floor(x)
    return whole + 1 if x - whole >= 0.5 else whole

