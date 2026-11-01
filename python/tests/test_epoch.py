"""TLE epochs to exact Unix microseconds."""

import unittest

from kepid import tle_epoch_unix_us

DAY = 86_400_000_000


class TleEpoch(unittest.TestCase):
    def test_exact_values(self):
        self.assertEqual(tle_epoch_unix_us("70001.00000000"), 0)
        self.assertEqual(tle_epoch_unix_us("26300.17675415"), 1_793_074_471_558_560)
        self.assertEqual(tle_epoch_unix_us("26300.1767541"), 1_793_074_471_554_240)
        self.assertEqual(tle_epoch_unix_us(" 26300 "), 1_793_059_200_000_000)
        self.assertEqual(tle_epoch_unix_us("26300.03268000"), 1_793_062_023_552_000)

    def test_century_rule(self):
        self.assertEqual(tle_epoch_unix_us("57001.0"), -410_227_200_000_000)
        self.assertEqual(tle_epoch_unix_us("56366.0"), 2_745_446_400_000_000)

    def test_malformed_fields(self):
        for bad in ["", "2630.5", "263000.5", "26300.123456789", "26000.5", "25366.5", "26x00.5", "26300.-1", "26300.1e3", "26３00.5"]:
            self.assertIsNone(tle_epoch_unix_us(bad), bad)


if __name__ == "__main__":
    unittest.main()
