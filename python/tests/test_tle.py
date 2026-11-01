"""TLE reading and writing; mirrors the Rust library's tests."""

import dataclasses
import unittest

from kepid import EncodeError, TleError, elements_to_tle, kepid_to_tle, tle_to_elements, tle_to_kepid
from kepid.epoch import tle_epoch_field, tle_epoch_unix_us
from kepid.tle import _angle, _read_catalog, _read_exponent, _write_catalog, _write_exponent

ISS = (
    "1 25544U 98067A   26305.20860139  .00006903  00000+0  13267-3 0  9994",
    "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592994",
)
ISS_ID = "01a1957a-ebef-73ab-bccf-878de5f3a90d"


class Tle(unittest.TestCase):
    def test_reads_the_iss(self):
        el = tle_to_elements(*ISS)
        self.assertEqual(el.epoch_unix_us, 1_793_509_223_160_096)
        self.assertEqual((el.mean_motion, el.eccentricity, el.inclination, el.raan), (15.49138562, 0.0007056, 51.6332, 342.0408))
        self.assertEqual((el.arg_perigee, el.mean_anomaly, el.bstar), (334.5041, 25.5599, 0.13267e-3))

    def test_readme_example(self):
        self.assertEqual(str(tle_to_kepid(*ISS)), ISS_ID)

    def test_writes_what_it_reads(self):
        l1, l2 = elements_to_tle(tle_to_elements(*ISS), 25544)
        self.assertEqual(l1, "1 25544U          26305.20860139  .00000000  00000+0  13267-3 0  9996")
        self.assertEqual(l2, "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562999995")

    def test_kepid_round_trips_through_text(self):
        kepid = tle_to_kepid(*ISS)
        l1, l2 = kepid_to_tle(kepid)
        self.assertEqual(l1, "1 00000U          26305.20860148  .00000000  00000+0  13155-3 0  9992")
        self.assertEqual(l2, "2 00000  51.6162 342.0483 0007474 334.2037  25.8607 15.49139459999994")
        self.assertEqual(tle_to_kepid(l1, l2), kepid)
        l1, l2 = kepid_to_tle(ISS_ID, 25544)
        self.assertTrue(l1.startswith("1 25544U ") and l2.startswith("2 25544 "))
        self.assertEqual(tle_to_kepid(l1, l2), kepid)

    def test_checksum_and_shape_errors(self):
        with self.assertRaisesRegex(TleError, "checksum is 5, expected 4"):
            tle_to_kepid(ISS[0].replace("9994", "9995"), ISS[1])
        with self.assertRaisesRegex(TleError, "has 60 characters"):
            tle_to_kepid(ISS[0][:60], ISS[1])
        with self.assertRaisesRegex(TleError, "does not start"):
            tle_to_kepid(ISS[1], ISS[0])
        with self.assertRaisesRegex(TleError, "different catalog numbers"):
            tle_to_kepid(ISS[0], "2 25545  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592995")
        self.assertEqual(str(tle_to_kepid(ISS[0] + "  \r", ISS[1])), ISS_ID)

    def test_below_the_floor_is_an_encode_error(self):
        low = dataclasses.replace(tle_to_elements(*ISS), mean_motion=16.5)
        with self.assertRaises(EncodeError):
            tle_to_kepid(*elements_to_tle(low))

    def test_alpha5_catalog_numbers(self):
        for n, text in [(0, "00000"), (99_999, "99999"), (100_000, "A0000"), (172_345, "H2345"), (189_999, "J9999"), (339_999, "Z9999")]:
            self.assertEqual(_write_catalog(n), text)
            self.assertEqual(_read_catalog(text), n)
        self.assertEqual(_write_catalog(230_000), "P0000")
        self.assertIsNone(_write_catalog(340_000))
        self.assertIsNone(_read_catalog("I0000"))
        self.assertEqual(_read_catalog(" 4444"), 4444)
        with self.assertRaisesRegex(TleError, "Alpha-5"):
            kepid_to_tle(ISS_ID, 340_000)

    def test_exponent_fields(self):
        for text, x in [(" 13267-3", 0.13267e-3), ("-11606-4", -0.11606e-4), (" 00000+0", 0.0), (" 10000+1", 1.0)]:
            self.assertEqual(_read_exponent(text), x)
            self.assertEqual(_write_exponent(x), text)
        self.assertIsNone(_read_exponent(" 12345 3"))
        self.assertEqual(_write_exponent(0.999996), " 10000+1")
        self.assertEqual(_write_exponent(1e-12), " 00000+0")
        self.assertIsNone(_write_exponent(2e9))

    def test_epoch_fields(self):
        self.assertEqual(tle_epoch_field(1_793_509_223_160_096), "26305.20860139")
        self.assertEqual(tle_epoch_field(0), "70001.00000000")
        new_year = tle_epoch_unix_us("27001.0")
        self.assertEqual(tle_epoch_field(new_year - 432), "27001.00000000")
        self.assertEqual(tle_epoch_field(new_year - 433), "26365.99999999")
        self.assertIsNone(tle_epoch_field(tle_epoch_unix_us("57001.0") - 1000))

    def test_angles_wrap_and_round(self):
        self.assertEqual(_angle(359.99996), "  0.0000")
        self.assertEqual(_angle(-10.0), "350.0000")


if __name__ == "__main__":
    unittest.main()
