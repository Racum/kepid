"""The conformance vectors of ``docs/spec.md``, section 10."""

import csv
import math
import unittest
from pathlib import Path

from kepid import Elements, EncodeError, decode, encode

VECTORS = Path(__file__).resolve().parents[2] / "testdata" / "profile0_vectors.csv"
FLOATS = ["mean_motion", "eccentricity", "inclination", "raan", "arg_perigee", "mean_anomaly", "bstar"]


def elements(row: dict, prefix: str = "") -> Elements:
    return Elements(
        epoch_unix_us=int(row[prefix + "epoch_unix_us"]),
        **{name: float(row[prefix + name]) for name in FLOATS},
    )


def rows():
    with VECTORS.open(newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


class Vectors(unittest.TestCase):
    def test_every_vector_encodes_to_its_kepid(self):
        for row in rows():
            with self.subTest(row["case"]):
                if row["kepid"].startswith("REFUSED"):
                    with self.assertRaises(EncodeError):
                        encode(elements(row))
                else:
                    self.assertEqual(str(encode(elements(row))), row["kepid"])

    def test_every_vector_decodes_to_its_elements(self):
        for row in rows():
            if row["kepid"].startswith("REFUSED"):
                continue
            with self.subTest(row["case"]):
                got, want = decode(row["kepid"]), elements(row, "dec_")
                self.assertEqual(got.epoch_unix_us, want.epoch_unix_us)
                for name in FLOATS:
                    g, w = getattr(got, name), getattr(want, name)
                    self.assertTrue(math.isclose(g, w, rel_tol=1e-12, abs_tol=1e-300), f"{name}: {g} vs {w}")

    def test_the_file_covers_the_cases_the_spec_lists(self):
        cases = [row["case"] for row in rows()]
        self.assertEqual(len(cases), 17)
        self.assertTrue(any(c.startswith("rejected") for c in cases))
        self.assertTrue(any("half a tick" in c for c in cases))


if __name__ == "__main__":
    unittest.main()
