"""Exact epoch input and output (``docs/spec.md``, section 7, "Epoch input")."""

import datetime
import re

_US_PER_DAY = 86_400_000_000
# One unit of a TLE epoch's 8-digit day fraction, 10⁻⁸ day, in microseconds.
_US_PER_TLE_DIGIT = 864
_UNIX_DAY_ZERO = datetime.date(1970, 1, 1).toordinal()


def _days(year: int, month: int, day: int) -> int:
    """Days from 1970-01-01, proleptic Gregorian."""
    return datetime.date(year, month, day).toordinal() - _UNIX_DAY_ZERO


def tle_epoch_unix_us(field: str) -> int | None:
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


def tle_epoch_field(unix_us: int) -> str | None:
    """Formats Unix microseconds as a TLE epoch field, ``YYDDD.DDDDDDDD``,
    rounding to the nearest 10⁻⁸ day (864 µs; a tie rounds up).

    Returns ``None`` outside 1957-2056, the years a two-digit TLE year covers.
    """
    units = (unix_us + _US_PER_TLE_DIGIT // 2) // _US_PER_TLE_DIGIT
    units_per_day = _US_PER_DAY // _US_PER_TLE_DIGIT
    days, fraction = divmod(units, units_per_day)
    year = datetime.date.fromordinal(days + _UNIX_DAY_ZERO).year if days + _UNIX_DAY_ZERO >= 1 else 0
    if not 1957 <= year <= 2056:
        return None
    day = days - _days(year, 1, 1) + 1
    return f"{year % 100:02}{day:03}.{fraction:08}"


_ISO8601 = re.compile(r"(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d*))?Z?", re.ASCII)


def iso8601_unix_us(text: str) -> int | None:
    """Converts an ISO 8601 UTC date-time, ``YYYY-MM-DDThh:mm:ss[.f…][Z]`` (an
    OMM ``EPOCH``), to Unix microseconds, exactly.

    The fraction is read to 6 digits; more digits round to the microsecond on
    their decimal digits, halves up (``docs/spec.md``, section 7, "Epoch
    input"). Returns ``None`` for anything else, including leap seconds.
    """
    match = _ISO8601.fullmatch(text.strip())
    if not match:
        return None
    y, m, d, h, minute, s = (int(g) for g in match.groups()[:6])
    fraction = match.group(7) or ""
    try:
        days = _days(y, m, d)
    except ValueError:
        return None
    if h > 23 or minute > 59 or s > 59:
        return None
    us = int(fraction[:6].ljust(6, "0"))
    if len(fraction) > 6 and fraction[6] >= "5":
        us += 1
    return days * _US_PER_DAY + ((h * 60 + minute) * 60 + s) * 1_000_000 + us


def omm_epoch(unix_us: int) -> str | None:
    """Formats Unix microseconds as ``YYYY-MM-DDThh:mm:ss.ffffff``, the form
    CelesTrak uses for OMM ``EPOCH``. Returns ``None`` outside years 1-9999."""
    days, rem = divmod(unix_us, _US_PER_DAY)
    ordinal = days + _UNIX_DAY_ZERO
    if not 1 <= ordinal <= datetime.date.max.toordinal():
        return None
    date = datetime.date.fromordinal(ordinal)
    seconds, us = divmod(rem, 1_000_000)
    h, rest = divmod(seconds, 3600)
    return f"{date.isoformat()}T{h:02}:{rest // 60:02}:{rest % 60:02}.{us:06}"
