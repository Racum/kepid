import assert from "node:assert/strict";
import { test } from "node:test";

import { OmmError, kepIdToOmm, ommToElements, ommToKepId } from "../src/index.js";

// CelesTrak's OMM for the ISS, fetched with the TLE in the README.
const CELESTRAK = '{"OBJECT_NAME":"ISS (ZARYA)","OBJECT_ID":"1998-067A","EPOCH":"2026-11-01T05:00:23.160096","MEAN_MOTION":15.49138562,"ECCENTRICITY":0.0007056,"INCLINATION":51.6332,"RA_OF_ASC_NODE":342.0408,"ARG_OF_PERICENTER":334.5041,"MEAN_ANOMALY":25.5599,"EPHEMERIS_TYPE":0,"CLASSIFICATION_TYPE":"U","NORAD_CAT_ID":25544,"ELEMENT_SET_NO":999,"REV_AT_EPOCH":59299,"BSTAR":0.00013267359,"MEAN_MOTION_DOT":6.903e-5,"MEAN_MOTION_DDOT":0}';
const ISS_ID = "01a1957a-ebef-73ab-bccf-878de5f3a90d";
const iss = () => JSON.parse(CELESTRAK);

test("reads CelesTrak JSON and Space-Track strings", () => {
  const el = ommToElements(iss());
  assert.equal(el.epochUnixUs, 1793509223160096n);
  assert.equal(el.bstar, 0.00013267359);
  const strings = Object.fromEntries(Object.entries(iss()).map(([k, v]) => [k, typeof v === "number" ? String(v) : v]));
  assert.deepEqual(ommToElements(strings), el);
  assert.equal(ommToKepId(iss()), ISS_ID);
});

test("a KepID round-trips through OMM and JSON", () => {
  const omm = kepIdToOmm(ISS_ID);
  assert.ok(!("NORAD_CAT_ID" in omm));
  assert.equal(ommToKepId(omm), ISS_ID);
  assert.equal(ommToKepId(JSON.parse(JSON.stringify(omm))), ISS_ID);
  const named = kepIdToOmm(ISS_ID, 25544);
  assert.equal(named.NORAD_CAT_ID, 25544);
  assert.equal(named.REV_AT_EPOCH, 99999);
});

test("matches the Rust output", () => {
  const rust = '{"CENTER_NAME":"EARTH","REF_FRAME":"TEME","TIME_SYSTEM":"UTC","MEAN_ELEMENT_THEORY":"SGP4","EPOCH":"2026-11-01T05:00:23.168000","MEAN_MOTION":15.491394590138162,"ECCENTRICITY":0.0007474469871928292,"INCLINATION":51.61619462448406,"RA_OF_ASC_NODE":342.04833984375,"ARG_OF_PERICENTER":334.20374889489966,"MEAN_ANOMALY":25.860679448257883,"EPHEMERIS_TYPE":0,"CLASSIFICATION_TYPE":"U","ELEMENT_SET_NO":999,"REV_AT_EPOCH":99999,"BSTAR":0.0001315454013845452,"MEAN_MOTION_DOT":0.0,"MEAN_MOTION_DDOT":0.0}';
  assert.deepEqual(kepIdToOmm(ISS_ID), JSON.parse(rust));
});

test("refuses other theories and frames", () => {
  assert.throws(() => ommToKepId({ ...iss(), MEAN_ELEMENT_THEORY: "SGP4-XP" }), /MEAN_ELEMENT_THEORY/);
  assert.throws(() => ommToKepId({ ...iss(), REF_FRAME: "GCRF" }), OmmError);
  assert.equal(ommToKepId({ ...iss(), MEAN_ELEMENT_THEORY: "sgp4", REF_FRAME: "TEME" }), ISS_ID);
  const { BSTAR, ...noBstar } = iss();
  assert.throws(() => ommToKepId(noBstar), /BSTAR/);
});

test("epochs follow the spec", () => {
  const at = (EPOCH) => ommToElements({ ...iss(), EPOCH }).epochUnixUs;
  assert.equal(at("2026-11-01T05:00:23.160096Z"), 1793509223160096n);
  assert.equal(at("2026-11-01T05:00:23.16"), 1793509223160000n);
  assert.equal(at("2026-11-01T05:00:23"), 1793509223000000n);
  assert.equal(at("2026-11-01T05:00:23.1600965"), 1793509223160097n, "a half rounds up");
  assert.equal(at("2026-11-01T05:00:23.1600964999"), 1793509223160096n);
  assert.equal(at("1970-01-01T00:00:00"), 0n);
  for (const bad of ["2026-11-01 05:00:23", "2026-02-29T00:00:00", "2026-11-01T24:00:00", "2026-11-01T05:00:60", "2026-305T05:00:23", "26-11-01T05:00:23"]) {
    assert.throws(() => at(bad), OmmError, bad);
  }
});
