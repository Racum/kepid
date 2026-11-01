"""KepID: Satellite orbits packed into UUIDv7.

A KepID holds an SGP4 mean element set (the content of a TLE or an OMM) in
one 128-bit value shaped like a UUIDv7. It describes an orbit, not an object,
and is not unique. The format is defined in ``docs/spec.md``.

This package implements profile 0 from the spec text, with the standard
library only. It is the library alone: no command-line tool. Like the Rust
library, it reads and writes TLEs (text) and OMMs (dictionaries, as
``json.loads`` gives them).
"""

from .codec import DecodeError, Elements, EncodeError, check_range, decode, drag_factor, encode, w_of_mean_motion
from .epoch import tle_epoch_unix_us
from .omm import OmmError, elements_to_omm, kepid_to_omm, omm_to_elements, omm_to_kepid
from .tle import TleError, elements_to_tle, kepid_to_tle, tle_to_elements, tle_to_kepid

__all__ = [
    "DecodeError",
    "Elements",
    "EncodeError",
    "OmmError",
    "TleError",
    "check_range",
    "decode",
    "drag_factor",
    "elements_to_omm",
    "elements_to_tle",
    "encode",
    "kepid_to_omm",
    "kepid_to_tle",
    "omm_to_elements",
    "omm_to_kepid",
    "tle_epoch_unix_us",
    "tle_to_elements",
    "tle_to_kepid",
    "w_of_mean_motion",
]
