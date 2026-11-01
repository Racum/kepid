import assert from "node:assert/strict";
import { test } from "node:test";

import { EncodeError, TleError, decode, elementsToTle, kepIdToTle, tleToElements, tleToKepId } from "../src/index.js";
import { angle, readCatalog, readExponent, writeCatalog, writeExponent } from "../src/tle.js";

const ISS = [
  "1 25544U 98067A   26305.20860139  .00006903  00000+0  13267-3 0  9994",
  "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592994",
];
const ISS_ID = "01a1957a-ebef-73ab-bccf-878de5f3a90d";

test("reads the ISS", () => {
  assert.deepEqual(tleToElements(...ISS), {
    epochUnixUs: 1793509223160096n,
    meanMotion: 15.49138562,
    eccentricity: 0.0007056,
    inclination: 51.6332,
    raan: 342.0408,
    argPerigee: 334.5041,
    meanAnomaly: 25.5599,
    bstar: 0.13267e-3,
  });
  assert.equal(tleToKepId(...ISS), ISS_ID);
});

test("writes what it reads", () => {
  assert.deepEqual(elementsToTle(tleToElements(...ISS), 25544), [
    "1 25544U          26305.20860139  .00000000  00000+0  13267-3 0  9996",
    "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562999995",
  ]);
});

test("a KepID round-trips through text", () => {
  const [l1, l2] = kepIdToTle(ISS_ID);
  assert.equal(l1, "1 00000U          26305.20860148  .00000000  00000+0  13155-3 0  9992");
  assert.equal(l2, "2 00000  51.6162 342.0483 0007474 334.2037  25.8607 15.49139459999994");
  assert.equal(tleToKepId(l1, l2), ISS_ID);
  const named = kepIdToTle(ISS_ID, 25544);
  assert.ok(named[0].startsWith("1 25544U ") && named[1].startsWith("2 25544 "));
  assert.equal(tleToKepId(...named), ISS_ID);
});

test("checksum and shape errors", () => {
  assert.throws(() => tleToKepId(ISS[0].replace("9994", "9995"), ISS[1]), { name: "TleError", message: "line 1 checksum is 5, expected 4" });
  assert.throws(() => tleToKepId(ISS[0].slice(0, 60), ISS[1]), /has 60 characters/);
  assert.throws(() => tleToKepId(ISS[1], ISS[0]), /does not start/);
  assert.throws(() => tleToKepId(ISS[0], "2 25545  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592995"), /different catalog/);
  assert.equal(tleToKepId(`${ISS[0]}  \r`, ISS[1]), ISS_ID);
  assert.throws(() => tleToKepId(...elementsToTle({ ...decode(ISS_ID), meanMotion: 16.5 })), EncodeError);
});

test("Alpha-5 catalog numbers", () => {
  for (const [n, text] of [[0, "00000"], [99999, "99999"], [100000, "A0000"], [172345, "H2345"], [189999, "J9999"], [339999, "Z9999"]]) {
    assert.equal(writeCatalog(n), text);
    assert.equal(readCatalog(text), n);
  }
  assert.equal(writeCatalog(230000), "P0000");
  assert.equal(writeCatalog(340000), null);
  assert.equal(readCatalog("I0000"), null);
  assert.equal(readCatalog(" 4444"), 4444);
  assert.throws(() => kepIdToTle(ISS_ID, 340000), TleError);
});

test("exponent fields and angles", () => {
  for (const [text, x] of [[" 13267-3", 0.13267e-3], ["-11606-4", -0.11606e-4], [" 00000+0", 0], [" 10000+1", 1]]) {
    assert.equal(readExponent(text), x);
    assert.equal(writeExponent(x), text);
  }
  assert.equal(readExponent(" 12345 3"), null);
  assert.equal(writeExponent(0.999996), " 10000+1");
  assert.equal(writeExponent(1e-12), " 00000+0");
  assert.equal(writeExponent(2e9), null);
  assert.equal(angle(359.99996), "  0.0000");
  assert.equal(angle(-10), "350.0000");
  assert.equal(angle(1.40625), "  1.4062", "an exact tie rounds to even");
});
