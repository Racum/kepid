"""Encoding and decoding, beyond the vectors."""

import dataclasses
import math
import random
import unittest
import uuid

from kepid import DecodeError, Elements, EncodeError, decode, encode, tle_epoch_unix_us, w_of_mean_motion
from kepid import profile0 as p

ISS_ID = "01a17b91-23e2-73ab-813c-86cddaf3a10c"
ISS = Elements(
    epoch_unix_us=1_793_074_471_558_560,
    mean_motion=15.49073103,
    eccentricity=0.0006948,
    inclination=51.6336,
    raan=6.9404,
    arg_perigee=314.9066,
    mean_anomaly=45.1358,
    bstar=0.00011132,
)


def with_(el: Elements, **changes) -> Elements:
    return dataclasses.replace(el, **changes)


def random_elements(rng: random.Random) -> Elements:
    """A random encodable element set, LEO to beyond GEO, perigee above the floor."""
    n = rng.uniform(11.0, 16.0) if rng.random() < 0.7 else rng.uniform(0.1, 11.0)
    e_max = min(1 - w_of_mean_motion(n), 0.99) * 0.999
    e = rng.uniform(0, min(e_max, 0.01)) if rng.random() < 0.6 else rng.uniform(0, e_max)
    return Elements(
        epoch_unix_us=rng.randrange(0, 4 * 10**15),
        mean_motion=n,
        eccentricity=e,
        inclination=rng.uniform(0, 180),
        raan=rng.uniform(-720, 720),
        arg_perigee=rng.uniform(-720, 720),
        mean_anomaly=rng.uniform(-720, 720),
        bstar=rng.uniform(-1e-3, 3e-3),
    )


def random_kepid(rng: random.Random) -> uuid.UUID:
    """Random bits with the version, variant and profile bits of profile 0."""
    x = rng.getrandbits(128)
    x &= ~(0xF << 76) & ~(0b11 << 62) & ~(1 << 37)
    x |= 0b0111 << 76 | 0b10 << 62
    return uuid.UUID(int=x)


class Iss(unittest.TestCase):
    def test_encodes_to_the_spec_example(self):
        self.assertEqual(str(encode(ISS)), ISS_ID)

    def test_decodes_to_the_rounded_epoch(self):
        el = decode(ISS_ID)
        self.assertEqual(el.epoch_unix_us, 1_793_074_471_680_000)
        self.assertAlmostEqual(el.mean_motion, 15.490749447697493, delta=1e-12)

    def test_accepts_every_kepid_form(self):
        want = decode(ISS_ID)
        self.assertEqual(decode(uuid.UUID(ISS_ID)), want)
        self.assertEqual(decode(uuid.UUID(ISS_ID).int), want)
        self.assertEqual(decode(ISS_ID.upper()), want)

    def test_the_tle_epoch_matches(self):
        self.assertEqual(tle_epoch_unix_us("26300.17675415"), ISS.epoch_unix_us)


class RoundTrips(unittest.TestCase):
    def test_encoding_is_idempotent_after_a_round_trip(self):
        rng = random.Random(42)
        for _ in range(3000):
            first = encode(random_elements(rng))
            self.assertEqual(encode(decode(first)), first)

    def test_every_value_with_valid_markers_decodes_to_a_valid_orbit_and_reencodes(self):
        rng = random.Random(0x5EED)
        for _ in range(5000):
            k = random_kepid(rng)
            el = decode(k)
            self.assertGreater(el.mean_motion, 0)
            self.assertTrue(0 <= el.eccentricity < 1, k)
            perigee = p.R_FLOOR / w_of_mean_motion(el.mean_motion) * (1 - el.eccentricity) - p.RE
            self.assertGreaterEqual(perigee, 250 - 1e-6, k)
            self.assertTrue(0 <= el.inclination <= 180)
            for angle in (el.raan, el.arg_perigee, el.mean_anomaly):
                self.assertTrue(0 <= angle < 360, k)
            self.assertEqual(encode(el), k)

    def test_moving_the_epoch_within_a_tick_moves_lambda_with_the_satellite(self):
        x = with_(ISS, epoch_unix_us=1_793_074_471_680_000)
        dt = 100_000
        later = with_(x, epoch_unix_us=x.epoch_unix_us + dt, mean_anomaly=x.mean_anomaly + 360 * x.mean_motion * dt / 86_400_000_000)
        self.assertEqual(encode(x), encode(later))


class Epoch(unittest.TestCase):
    def ticks(self, us: int) -> int:
        return encode(with_(ISS, epoch_unix_us=us)).int >> 88

    def test_rounds_to_the_nearest_tick_halves_up(self):
        base = 1000 * p.TICK_US
        self.assertEqual(self.ticks(base), 1000)
        self.assertEqual(self.ticks(base + 127_999), 1000)
        self.assertEqual(self.ticks(base + 128_000), 1001)
        self.assertEqual(self.ticks(base - 128_000), 1000)
        self.assertEqual(self.ticks(base - 128_001), 999)

    def test_half_tick_tle_epochs_round_up(self):
        for field, tick in [
            ("26300.03268000", 7_004_148_530),
            ("26300.19244000", 7_004_202_449),
            ("26298.88404000", 7_003_760_864),
            ("26300.31596000", 7_004_244_137),
        ]:
            us = tle_epoch_unix_us(field)
            self.assertEqual(us % p.TICK_US, p.TICK_US // 2, field)
            self.assertEqual(self.ticks(us), tick, field)

    def test_range(self):
        self.assertEqual(self.ticks(0), 0)
        self.assertEqual(self.ticks(p.MAX_EPOCH_TICKS * p.TICK_US + 127_999), p.MAX_EPOCH_TICKS)
        for bad in (-1, p.MAX_EPOCH_TICKS * p.TICK_US + 128_000):
            with self.assertRaises(EncodeError):
                encode(with_(ISS, epoch_unix_us=bad))

    def test_a_float_epoch_is_refused(self):
        with self.assertRaises(EncodeError):
            encode(with_(ISS, epoch_unix_us=1.7930744715585600e15))


class Validation(unittest.TestCase):
    def refused(self, **changes):
        with self.assertRaises(EncodeError):
            encode(with_(ISS, **changes))

    def test_closed_orbits_and_inclination(self):
        for changes in ({"mean_motion": 0.0}, {"mean_motion": -1.0}, {"eccentricity": -1e-9}, {"eccentricity": 1.0},
                        {"inclination": -1e-9}, {"inclination": 180.0000001}):
            with self.subTest(changes):
                self.refused(**changes)
        encode(with_(ISS, inclination=0.0))
        encode(with_(ISS, inclination=180.0))

    def test_non_finite_values(self):
        for name in ("mean_motion", "eccentricity", "inclination", "raan", "arg_perigee", "mean_anomaly", "bstar"):
            for bad in (math.nan, math.inf, -math.inf):
                with self.subTest(name=name, bad=bad):
                    self.refused(**{name: bad})

    def test_perigee_floor(self):
        limit = 1 - w_of_mean_motion(16.0)
        encode(with_(ISS, mean_motion=16.0, eccentricity=limit * (1 - 1e-6)))
        self.refused(mean_motion=16.0, eccentricity=limit * (1 + 1e-6))
        self.refused(mean_motion=16.2, eccentricity=0.0)


class Markers(unittest.TestCase):
    def test_wrong_version_variant_or_profile_is_refused(self):
        x = uuid.UUID(ISS_ID).int
        for bad in (x ^ (1 << 76), x ^ (1 << 63), x ^ (1 << 37)):
            with self.assertRaises(DecodeError):
                decode(bad)

    def test_not_a_kepid_type(self):
        with self.assertRaises(TypeError):
            decode(1 << 128)


class Drag(unittest.TestCase):
    def test_zero_b_star_is_exact(self):
        self.assertEqual(decode(encode(with_(ISS, bstar=0.0))).bstar, 0.0)

    def test_sign_is_kept(self):
        self.assertLess(decode(encode(with_(ISS, bstar=-1e-4))).bstar, 0)

    def test_saturation_is_asymmetric(self):
        top = p.DRAG.decode(p.DRAG.codes - 1)
        bottom = p.DRAG.decode(0)
        self.assertAlmostEqual(bottom, -p.D_MAX, delta=1e-12)
        self.assertLess(top, p.D_MAX - 1e-6)
        self.assertEqual(p.DRAG.encode(10.0), p.DRAG.codes - 1)
        self.assertEqual(p.DRAG.encode(-10.0), 0)


if __name__ == "__main__":
    unittest.main()
