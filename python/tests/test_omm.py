"""OMM reading and writing; mirrors the Rust library's tests."""

import json
import unittest

from kepid import OmmError, kepid_to_omm, omm_to_elements, omm_to_kepid

# CelesTrak's OMM for the ISS, fetched with the TLE in the README.
CELESTRAK = '{"OBJECT_NAME":"ISS (ZARYA)","OBJECT_ID":"1998-067A","EPOCH":"2026-11-01T05:00:23.160096","MEAN_MOTION":15.49138562,"ECCENTRICITY":0.0007056,"INCLINATION":51.6332,"RA_OF_ASC_NODE":342.0408,"ARG_OF_PERICENTER":334.5041,"MEAN_ANOMALY":25.5599,"EPHEMERIS_TYPE":0,"CLASSIFICATION_TYPE":"U","NORAD_CAT_ID":25544,"ELEMENT_SET_NO":999,"REV_AT_EPOCH":59299,"BSTAR":0.00013267359,"MEAN_MOTION_DOT":6.903e-5,"MEAN_MOTION_DDOT":0}'
ISS_ID = "01a1957a-ebef-73ab-bccf-878de5f3a90d"


def iss():
    return json.loads(CELESTRAK)


class Omm(unittest.TestCase):
    def test_reads_celestrak_json(self):
        el = omm_to_elements(iss())
        self.assertEqual(el.epoch_unix_us, 1_793_509_223_160_096)
        self.assertEqual((el.mean_motion, el.eccentricity, el.bstar), (15.49138562, 0.0007056, 0.00013267359))

    def test_reads_space_track_strings(self):
        strings = {k: str(v) if isinstance(v, (int, float)) else v for k, v in iss().items()}
        self.assertEqual(omm_to_elements(strings), omm_to_elements(iss()))

    def test_same_kepid_as_the_readme_tle(self):
        self.assertEqual(str(omm_to_kepid(iss())), ISS_ID)

    def test_kepid_round_trips_through_omm_and_json(self):
        kepid = omm_to_kepid(iss())
        omm = kepid_to_omm(kepid)
        self.assertNotIn("NORAD_CAT_ID", omm)
        self.assertEqual(omm["MEAN_ELEMENT_THEORY"], "SGP4")
        self.assertEqual(omm_to_kepid(omm), kepid)
        self.assertEqual(omm_to_kepid(json.loads(json.dumps(omm))), kepid)
        named = kepid_to_omm(kepid, 25544)
        self.assertEqual((named["NORAD_CAT_ID"], named["REV_AT_EPOCH"]), (25544, 99_999))

    def test_matches_the_rust_output(self):
        rust = '{"CENTER_NAME":"EARTH","REF_FRAME":"TEME","TIME_SYSTEM":"UTC","MEAN_ELEMENT_THEORY":"SGP4","EPOCH":"2026-11-01T05:00:23.168000","MEAN_MOTION":15.491394590138162,"ECCENTRICITY":0.0007474469871928292,"INCLINATION":51.61619462448406,"RA_OF_ASC_NODE":342.04833984375,"ARG_OF_PERICENTER":334.20374889489966,"MEAN_ANOMALY":25.860679448257883,"EPHEMERIS_TYPE":0,"CLASSIFICATION_TYPE":"U","ELEMENT_SET_NO":999,"REV_AT_EPOCH":99999,"BSTAR":0.0001315454013845452,"MEAN_MOTION_DOT":0.0,"MEAN_MOTION_DDOT":0.0}'
        self.assertEqual(json.dumps(kepid_to_omm(ISS_ID), separators=(",", ":")), rust)

    def test_refuses_other_theories_and_frames(self):
        with self.assertRaisesRegex(OmmError, "MEAN_ELEMENT_THEORY"):
            omm_to_kepid({**iss(), "MEAN_ELEMENT_THEORY": "SGP4-XP"})
        with self.assertRaisesRegex(OmmError, "REF_FRAME"):
            omm_to_kepid({**iss(), "REF_FRAME": "GCRF"})
        self.assertEqual(str(omm_to_kepid({**iss(), "MEAN_ELEMENT_THEORY": "sgp4", "REF_FRAME": "TEME"})), ISS_ID)
        with self.assertRaisesRegex(OmmError, "BSTAR"):
            omm_to_kepid({k: v for k, v in iss().items() if k != "BSTAR"})

    def test_epochs_follow_the_spec(self):
        def at(epoch):
            return omm_to_elements({**iss(), "EPOCH": epoch}).epoch_unix_us

        self.assertEqual(at("2026-11-01T05:00:23.160096Z"), 1_793_509_223_160_096)
        self.assertEqual(at("2026-11-01T05:00:23.16"), 1_793_509_223_160_000)
        self.assertEqual(at("2026-11-01T05:00:23"), 1_793_509_223_000_000)
        self.assertEqual(at("2026-11-01T05:00:23.1600965"), 1_793_509_223_160_097)
        self.assertEqual(at("2026-11-01T05:00:23.1600964999"), 1_793_509_223_160_096)
        self.assertEqual(at("1970-01-01T00:00:00"), 0)
        for bad in ["2026-11-01 05:00:23", "2026-02-29T00:00:00", "2026-11-01T24:00:00", "2026-11-01T05:00:60", "2026-305T05:00:23", "26-11-01T05:00:23"]:
            with self.assertRaises(OmmError, msg=bad):
                at(bad)


if __name__ == "__main__":
    unittest.main()
