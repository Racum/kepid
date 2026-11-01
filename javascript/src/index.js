// KepID: Satellite orbits packed into UUIDv7.
//
// A KepID holds an SGP4 mean element set (the content of a TLE or an OMM) in
// one 128-bit value shaped like a UUIDv7. It describes an orbit, not an
// object, and is not unique. The format is defined in docs/spec.md.
//
// This package implements profile 0 from the spec text, with no dependencies.
// It produces the same KepIDs as the Rust and Python libraries, bit for bit.

export { DecodeError, EncodeError, checkRange, decode, dragFactor, encode, wOfMeanMotion } from "./codec.js";
export { tleEpochUnixUs } from "./epoch.js";
export { OmmError, elementsToOmm, kepIdToOmm, ommToElements, ommToKepId } from "./omm.js";
export { TleError, elementsToTle, kepIdToTle, tleToElements, tleToKepId } from "./tle.js";
