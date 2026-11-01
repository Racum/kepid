"""Exact epoch input (``docs/spec.md``, section 7, "Epoch input")."""

from __future__ import annotations

import datetime
from typing import Optional

_US_PER_DAY = 86_400_000_000
# One unit of a TLE epoch's 8-digit day fraction, 10⁻⁸ day, in microseconds.
_US_PER_TLE_DIGIT = 864
_UNIX_DAY_ZERO = datetime.date(1970, 1, 1).toordinal()


def _days(year: int, month: int, day: int) -> int:
    """Days from 1970-01-01, proleptic Gregorian."""
    return datetime.date(year, month, day).toordinal() - _UNIX_DAY_ZERO


def tle_epoch_unix_us(field: str) -> Optional[int]:
    """Converts a TLE epoch field, ``YYDDD.DDDDDDDD`` (line 1, columns 19-32),
    to Unix microseconds, exactly.

    Years 57-99 are 1957-1999 and 00-56 are 2000-2056. Day 1 is 1 January.
    The fraction may have up to 8 digits; each 10⁻⁸ day is exactly 864 µs.
    Returns ``None`` for anything else.
    """
    field = field.strip()
    whole, _, fraction = field.partition(".")
    if len(whole) != 5 or not whole.isascii() or not whole.isdigit():
        return None
    if len(fraction) > 8 or not fraction.isascii() or (fraction and not fraction.isdigit()):
        return None
    yy, day = int(whole[:2]), int(whole[2:])
    year = 2000 + yy if yy < 57 else 1900 + yy
    leap = year % 4 == 0 and (year % 100 != 0 or year % 400 == 0)
    if not 1 <= day <= 365 + leap:
        return None
    units = int(fraction.ljust(8, "0"))
    return (_days(year, 1, 1) + day - 1) * _US_PER_DAY + units * _US_PER_TLE_DIGIT
