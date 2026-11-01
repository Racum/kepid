import assert from "node:assert/strict";
import { test } from "node:test";

import { DecodeError, EncodeError, checkRange, decode, encode, tleEpochUnixUs } from "../src/index.js";
import { cbrt } from "../src/codec.js";
import { toExponentialExact, toFixedExact } from "../src/format.js";

const ISS_ID = "01a1957a-ebef-73ab-bccf-878de5f3a90d";

test("decode accepts text, bare hex and BigInt; encode round-trips", () => {
  const el = decode(ISS_ID);
  assert.equal(encode(el), ISS_ID);
  assert.deepEqual(decode(ISS_ID.replaceAll("-", "").toUpperCase()), el);
  assert.deepEqual(decode(BigInt(`0x${ISS_ID.replaceAll("-", "")}`)), el);
  assert.equal(el.epochUnixUs, 1793509223168000n);
});

test("decode refuses what is not a profile 0 KepID", () => {
  assert.throws(() => decode("0192f3a1-7c4e-4b2d-9a41-5d3e8f1c2b60"), DecodeError);
  assert.throws(() => decode("not a uuid"), DecodeError);
  assert.throws(() => decode(42), TypeError);
});

test("checkRange and encode refuse impossible orbits", () => {
  const el = decode(ISS_ID);
  assert.doesNotThrow(() => checkRange(el));
  assert.throws(() => checkRange({ ...el, meanMotion: 16.5 }), /250 km floor/);
  assert.throws(() => encode({ ...el, epochUnixUs: -1n }), EncodeError);
  assert.throws(() => encode({ ...el, eccentricity: NaN }), /not a finite number/);
  assert.equal(encode({ ...el, epochUnixUs: Number(el.epochUnixUs) }), ISS_ID, "a safe integer epoch works too");
  assert.throws(() => encode({ ...el, epochUnixUs: 1.5 }), EncodeError);
});

test("the spec's cube root", () => {
  for (const x of [1, 8, 27, 0.001, 1e30, 1e-30]) assert.ok(Math.abs(cbrt(x) / Math.cbrt(x) - 1) < 3e-16, String(x));
});

test("TLE epochs are exact", () => {
  assert.equal(tleEpochUnixUs("70001.00000000"), 0n);
  assert.equal(tleEpochUnixUs("26300.17675415"), 1793074471558560n);
  assert.equal(tleEpochUnixUs(" 26300 "), 1793059200000000n);
  assert.equal(tleEpochUnixUs("26300.03268000") % 256000n, 128000n, "exactly half a tick");
  for (const bad of ["", "2630.5", "26300.123456789", "26000.5", "25366.5", "26x00.5", "26300.1e3"]) assert.equal(tleEpochUnixUs(bad), null, bad);
});

test("exact formatting rounds ties to even, like Rust and Python", () => {
  assert.equal(toFixedExact(1.40625, 4), "1.4062");
  assert.equal((1.40625).toFixed(4), "1.4063", "what toFixed would write");
  assert.equal(toFixedExact(0.125, 2), "0.12");
  assert.equal(toFixedExact(15.491394590138162, 8), "15.49139459");
  assert.deepEqual(toExponentialExact(0.13267e-3, 4), ["13267", -4]);
  assert.deepEqual(toExponentialExact(0.999996, 4), ["10000", 0]);
  assert.deepEqual(toExponentialExact(1000, 4), ["10000", 3]);
});
