// The conformance vectors of docs/spec.md, section 10.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { EncodeError, decode, encode } from "../src/index.js";

const FLOATS = { mean_motion: "meanMotion", eccentricity: "eccentricity", inclination: "inclination", raan: "raan", arg_perigee: "argPerigee", mean_anomaly: "meanAnomaly", bstar: "bstar" };

function rows() {
  const [head, ...lines] = readFileSync(new URL("../../testdata/profile0_vectors.csv", import.meta.url), "utf8").trim().split("\n");
  const names = head.split(",");
  return lines.map((line) => Object.fromEntries(line.split(",").map((value, i) => [names[i], value])));
}

function elements(row, prefix = "") {
  const el = { epochUnixUs: BigInt(row[`${prefix}epoch_unix_us`]) };
  for (const [csv, js] of Object.entries(FLOATS)) el[js] = Number(row[prefix + csv]);
  return el;
}

test("every vector encodes to its KepID", () => {
  for (const row of rows()) {
    if (row.kepid.startsWith("REFUSED")) assert.throws(() => encode(elements(row)), EncodeError, row.case);
    else assert.equal(encode(elements(row)), row.kepid, row.case);
  }
});

test("every vector decodes to its elements, bit for bit", () => {
  for (const row of rows().filter((r) => !r.kepid.startsWith("REFUSED"))) {
    assert.deepEqual(decode(row.kepid), elements(row, "dec_"), row.case);
  }
});

test("the file covers the cases the spec lists", () => {
  const cases = rows().map((r) => r.case);
  assert.equal(cases.length, 17);
  assert.ok(cases.some((c) => c.startsWith("rejected")));
  assert.ok(cases.some((c) => c.includes("half a tick")));
});
