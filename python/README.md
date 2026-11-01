# KepID for Python

Satellite orbits packed into UUIDv7, in pure Python: no dependencies, just the standard library. This is the Python version of the KepID library; for what KepID is and why, start with the [main README](../README.md), and for every rule and constant, the [spec](../docs/spec.md).

It produces the same KepIDs as the Rust library, bit for bit, and writes the same TLEs and OMMs, character for character.

## Install

Python 3.11 or newer:

```bash
pip install kepid
```

## Usage

### TLE

Turning the ISS TLE from the main README into a KepID, and back:

```python
from kepid import kepid_to_tle, tle_to_kepid

line1 = "1 25544U 98067A   26305.20860139  .00006903  00000+0  13267-3 0  9994"
line2 = "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592994"

# TLE to KepID (a standard uuid.UUID)
kid = tle_to_kepid(line1, line2)
print(kid)
# Prints: 01a1957a-ebef-73ab-bccf-878de5f3a90d

# KepID to TLE, with placeholders for the catalog number, designator and
# the rest of what a KepID doesn't carry
new1, new2 = kepid_to_tle(kid)
print(new1)
print(new2)
# Prints:
# 1 00000U          26305.20860148  .00000000  00000+0  13155-3 0  9992
# 2 00000  51.6162 342.0483 0007474 334.2037  25.8607 15.49139459999994

# Or for a given object
new1, new2 = kepid_to_tle(kid, catalog=25544)
```

Notice that the TLE that comes back is not the one that went in: every element is rounded to its code, and the epoch to its 256 ms tick. That's the lossy part, and the reason it fits in 16 bytes!

### OMM

An OMM is a plain dictionary, the shape `json.loads` gives for CelesTrak's or Space-Track's JSON (numbers can be numbers or strings):

```python
import json
from kepid import kepid_to_omm, omm_to_kepid

omm = json.loads("""{
    "EPOCH": "2026-11-01T05:00:23.160096",
    "MEAN_MOTION": 15.49138562,
    "ECCENTRICITY": 0.0007056,
    "INCLINATION": 51.6332,
    "RA_OF_ASC_NODE": 342.0408,
    "ARG_OF_PERICENTER": 334.5041,
    "MEAN_ANOMALY": 25.5599,
    "NORAD_CAT_ID": 25544,
    "BSTAR": 0.00013267359
}""")

kid = omm_to_kepid(omm)
print(kid)
# Prints: 01a1957a-ebef-73ab-bccf-878de5f3a90d

back = kepid_to_omm(kid, catalog=25544)  # a dict, ready for json.dumps
print(back["EPOCH"], back["MEAN_MOTION"])
# Prints: 2026-11-01T05:00:23.168000 15.491394590138162
```

The long tails of digits are just the exact centre of each code; don't read them as extra precision.

### Elements

Under the hood, both formats go through `Elements`, the mean element set in TLE units, with the epoch as whole Unix microseconds:

```python
from kepid import decode, encode

el = decode("01a1957a-ebef-73ab-bccf-878de5f3a90d")
print(el.epoch_unix_us, el.mean_motion, el.inclination)
# Prints: 1793509223168000 15.491394590138162 51.61619462448406

print(encode(el))
# Prints: 01a1957a-ebef-73ab-bccf-878de5f3a90d
```

`decode` takes a `uuid.UUID`, its text form or a 128-bit integer. To work with the elements themselves, there are `tle_to_elements`, `elements_to_tle`, `omm_to_elements` and `elements_to_omm`, and `check_range(el)` tells you whether an element set can be encoded without encoding it.

### Errors

Every error is a `ValueError`, so one `except` can catch them all, or you can be specific:

| Error | When |
|---|---|
| `TleError` | Malformed TLE: wrong length, bad checksum, unreadable field… |
| `OmmError` | Malformed OMM: missing keyword, bad `EPOCH`, or a theory other than SGP4 |
| `EncodeError` | Valid input that can't be a KepID: perigee below 250 km, epoch before 1970… |
| `DecodeError` | Not a KepID: wrong version, variant or profile bits |

```python
from kepid import TleError, tle_to_kepid

try:
    tle_to_kepid(line1.replace("9994", "9995"), line2)
except TleError as e:
    print(e)
# Prints: line 1 checksum is 5, expected 4
```

## License

[MIT](../LICENSE).
