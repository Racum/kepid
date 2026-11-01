"""KepID: zero-line orbital elements packed into a UUIDv7.

A KepID holds an SGP4 mean element set (the content of a TLE or an OMM) in
one 128-bit value shaped like a UUIDv7. It describes an orbit, not an object,
and is not unique. The format is defined in ``docs/spec.md``.

This package implements profile 0 from the spec text, with the standard
library only. It is the library alone: no command-line tool.
"""

from .codec import DecodeError, Elements, EncodeError, decode, drag_factor, encode, w_of_mean_motion
from .epoch import tle_epoch_unix_us

__all__ = [
    "DecodeError",
    "Elements",
    "EncodeError",
    "decode",
    "drag_factor",
    "encode",
    "tle_epoch_unix_us",
    "w_of_mean_motion",
]
