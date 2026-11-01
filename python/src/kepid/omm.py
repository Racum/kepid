"""Orbit Mean-Elements Messages (CCSDS OMM), as plain dictionaries.

An OMM is a mapping from OMM keywords to values, the shape ``json.loads``
gives for CelesTrak's and Space-Track's JSON. Numbers may be numbers
(CelesTrak) or strings (Space-Track); both are read exactly. Python's ``json``
parses numbers exactly, so no extra care is needed there. The output matches
the Rust library's ``Omm`` key for key.
"""

import re
import uuid
from collections.abc import Mapping
from typing import Any

from .codec import Elements, KepIdLike, decode, encode
from .epoch import iso8601_unix_us, omm_epoch

# Placeholders for the identity fields a KepID does not have, as in a TLE.
_CLASSIFICATION = "U"
_ELEMENT_SET = 999
_REV_NUMBER = 99_999
_NUMBER = re.compile(r"[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?", re.ASCII)


class OmmError(ValueError):
    """The OMM cannot be read or written."""


def omm_to_elements(omm: Mapping[str, Any]) -> Elements:
    """Reads the mean elements of an OMM. The identity keywords (catalog
    number, name and so on) are not kept: a KepID cannot hold them.

    The optional header keywords are checked when present:
    ``MEAN_ELEMENT_THEORY`` must be ``SGP4``, ``REF_FRAME`` must be ``TEME``
    and ``TIME_SYSTEM`` must be ``UTC``.
    """
    for key, expected in (("MEAN_ELEMENT_THEORY", "SGP4"), ("REF_FRAME", "TEME"), ("TIME_SYSTEM", "UTC")):
        value = omm.get(key)
        if value is not None and str(value).strip().upper() != expected:
            raise OmmError(f"{key} is {value!r}, which a KepID cannot carry")
    epoch_text = omm.get("EPOCH")
    epoch = iso8601_unix_us(epoch_text) if isinstance(epoch_text, str) else None
    if epoch is None:
        raise OmmError(f"EPOCH {epoch_text!r} is not an ISO 8601 UTC date-time")
    return Elements(
        epoch_unix_us=epoch,
        mean_motion=_number(omm, "MEAN_MOTION"),
        eccentricity=_number(omm, "ECCENTRICITY"),
        inclination=_number(omm, "INCLINATION"),
        raan=_number(omm, "RA_OF_ASC_NODE"),
        arg_perigee=_number(omm, "ARG_OF_PERICENTER"),
        mean_anomaly=_number(omm, "MEAN_ANOMALY"),
        bstar=_number(omm, "BSTAR"),
    )


def elements_to_omm(el: Elements, catalog: int | None = None) -> dict[str, Any]:
    """Writes mean elements as an OMM, for the object with this NORAD catalog
    number, or with no ``NORAD_CAT_ID`` when it is ``None``.

    The header keywords say what the elements are (``EARTH``, ``TEME``,
    ``UTC``, ``SGP4``), and the identity keywords are placeholders:
    classification ``U``, element set 999 and revolution number 99999. The
    derivatives of mean motion are written as zero; SGP4 does not use them.
    Numbers keep their full precision; the epoch is written to the microsecond.
    """
    epoch = omm_epoch(el.epoch_unix_us)
    if epoch is None:
        raise OmmError(f"epoch {el.epoch_unix_us} µs is past year 9999")
    omm: dict[str, Any] = {
        "CENTER_NAME": "EARTH",
        "REF_FRAME": "TEME",
        "TIME_SYSTEM": "UTC",
        "MEAN_ELEMENT_THEORY": "SGP4",
        "EPOCH": epoch,
        "MEAN_MOTION": el.mean_motion,
        "ECCENTRICITY": el.eccentricity,
        "INCLINATION": el.inclination,
        "RA_OF_ASC_NODE": el.raan % 360.0,
        "ARG_OF_PERICENTER": el.arg_perigee % 360.0,
        "MEAN_ANOMALY": el.mean_anomaly % 360.0,
        "EPHEMERIS_TYPE": 0,
        "CLASSIFICATION_TYPE": _CLASSIFICATION,
    }
    if catalog is not None:
        omm["NORAD_CAT_ID"] = catalog
    omm.update({
        "ELEMENT_SET_NO": _ELEMENT_SET,
        "REV_AT_EPOCH": _REV_NUMBER,
        "BSTAR": el.bstar,
        "MEAN_MOTION_DOT": 0.0,
        "MEAN_MOTION_DDOT": 0.0,
    })
    return omm


def omm_to_kepid(omm: Mapping[str, Any]) -> uuid.UUID:
    """Reads an OMM and encodes it. Raises :class:`OmmError` for a malformed
    OMM and :class:`EncodeError` for elements that cannot be encoded."""
    return encode(omm_to_elements(omm))


def kepid_to_omm(kepid: KepIdLike, catalog: int | None = None) -> dict[str, Any]:
    """Decodes a KepID and writes it as an OMM, for the object with this NORAD
    catalog number, or with none when it is ``None``. A KepID describes an
    orbit, not an object, so the number comes from the caller."""
    return elements_to_omm(decode(kepid), catalog)


def _number(omm: Mapping[str, Any], key: str) -> float:
    """A number, or a string holding one, parsed exactly."""
    match value := omm.get(key):
        case bool():
            pass
        case int() | float():
            return float(value)
        case str() if _NUMBER.fullmatch(value.strip()):
            return float(value.strip())
    raise OmmError(f"{key} is missing or not a number: {value!r}")

