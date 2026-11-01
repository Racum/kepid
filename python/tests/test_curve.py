"""Curves, quantizers and the helper functions of section 2."""

import math
import unittest

from kepid import profile0 as p
from kepid.codec import cbrt
from kepid.curve import forward, inverse, mod, round_half_away, wrap

FIELDS = {
    "ω": p.ARG_PERIGEE,
    "i": p.INCLINATION,
    "Ω": p.RAAN,
    "D": p.DRAG,
    "λ": p.LAMBDA,
    "w": p.MEAN_MOTION,
    "s": p.ECCENTRICITY,
}


class Functions(unittest.TestCase):
    def test_round_halves_away_from_zero(self):
        for x, want in [(0.5, 1.0), (1.5, 2.0), (2.5, 3.0), (-0.5, -1.0), (-2.5, -3.0), (0.49999999999999994, 0.0), (2.4, 2.0)]:
            self.assertEqual(round_half_away(x), want, x)

    def test_mod_is_euclidean(self):
        self.assertEqual(mod(-10.0, 360.0), 350.0)
        self.assertEqual(mod(370.0, 360.0), 10.0)

    def test_wrap_stays_in_half_open_range(self):
        for x, want in [(0.0, 0.0), (-0.0, 0.0), (360.0, 0.0), (-1e-18, 0.0), (-1e-14, 0.0), (-10.0, 350.0), (720.0, 0.0)]:
            got = wrap(x)
            self.assertEqual(got, want, x)
            self.assertEqual(math.copysign(1.0, got), 1.0, x)


class Curves(unittest.TestCase):
    def test_end_points_are_fixed(self):
        for name, field in FIELDS.items():
            with self.subTest(name):
                self.assertAlmostEqual(forward(field.layers, 0.0), 0.0, delta=1e-12)
                self.assertAlmostEqual(forward(field.layers, 1.0), 1.0, delta=1e-12)

    def test_monotonic_and_inverse_is_closed_form(self):
        for name, field in FIELDS.items():
            with self.subTest(name):
                prev = -1.0
                for j in range(2001):
                    z = j / 2000
                    t = forward(field.layers, z)
                    self.assertGreater(t, prev)
                    self.assertAlmostEqual(inverse(field.layers, t), z, delta=1e-9)
                    prev = t

    def test_every_code_reencodes_to_itself(self):
        for name, field in FIELDS.items():
            with self.subTest(name):
                for k in range(field.codes):
                    self.assertEqual(field.encode(field.decode(k)), k)


class CubeRoot(unittest.TestCase):
    def test_exact_cubes(self):
        for root in (1.0, 2.0, 3.0, 0.5, 0.25, 1024.0, 1 / 1024):
            self.assertEqual(cbrt(root * root * root), root)

    def test_close_to_the_true_cube_root(self):
        for j in range(-300, 301):
            x = 10 ** (j / 10)
            self.assertTrue(math.isclose(cbrt(x) ** 3, x, rel_tol=1e-15), x)

if __name__ == "__main__":
    unittest.main()
