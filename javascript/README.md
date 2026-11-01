# KepID for JavaScript

Satellite orbits packed into UUIDv7, in plain JavaScript: no dependencies, an ES module that runs in Node.js 18+, Deno, Bun and browsers. This is the JavaScript version of the KepID library; for what KepID is and why, start with the [main README](https://github.com/racum/kepid), and for every rule and constant, the [spec](https://github.com/racum/kepid/blob/main/docs/spec.md).

It produces the same KepIDs as the Rust and Python libraries, bit for bit, and writes the same TLEs and OMMs, character for character.

## Install

```bash
npm install @racum/kepid
```

There is also a single-file, minified build, `dist/kepid.min.js` (about 13 KB, 6 KB gzipped), with the same exports. Import it as `@racum/kepid/min`, or drop the file next to a web page:

```html
<script type="module">
  import { tleToKepId } from "./kepid.min.js";
</script>
```

To rebuild it after changing the source, run `npm run build` (it uses esbuild through npx; nothing is added to the dependencies).

## Usage

### TLE

Turning the ISS TLE from the main README into a KepID, and back:

```js
import { kepIdToTle, tleToKepId } from "@racum/kepid";

const line1 = "1 25544U 98067A   26305.20860139  .00006903  00000+0  13267-3 0  9994";
const line2 = "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592994";

// TLE to KepID (its text form, a string)
const kid = tleToKepId(line1, line2);
console.log(kid);
// Prints: 01a1957a-ebef-73ab-bccf-878de5f3a90d

// KepID to TLE, with placeholders for the catalog number, designator and
// the rest of what a KepID doesn't carry
const [new1, new2] = kepIdToTle(kid);
console.log(new1);
console.log(new2);
// Prints:
// 1 00000U          26305.20860148  .00000000  00000+0  13155-3 0  9992
// 2 00000  51.6162 342.0483 0007474 334.2037  25.8607 15.49139459999994

// Or for a given object
const [named1, named2] = kepIdToTle(kid, 25544);
```

Notice that the TLE that comes back is not the one that went in: every element is rounded to its code, and the epoch to its 256 ms tick. That's the lossy part, and the reason it fits in 16 bytes!

### OMM

An OMM is a plain object, the shape `JSON.parse` gives for CelesTrak's or Space-Track's JSON (numbers can be numbers or strings):

```js
import { kepIdToOmm, ommToKepId } from "@racum/kepid";

const omm = JSON.parse(`{
  "EPOCH": "2026-11-01T05:00:23.160096",
  "MEAN_MOTION": 15.49138562,
  "ECCENTRICITY": 0.0007056,
  "INCLINATION": 51.6332,
  "RA_OF_ASC_NODE": 342.0408,
  "ARG_OF_PERICENTER": 334.5041,
  "MEAN_ANOMALY": 25.5599,
  "NORAD_CAT_ID": 25544,
  "BSTAR": 0.00013267359
}`);

const kid = ommToKepId(omm);
console.log(kid);
// Prints: 01a1957a-ebef-73ab-bccf-878de5f3a90d

const back = kepIdToOmm(kid, 25544); // an object, ready for JSON.stringify
console.log(back.EPOCH, back.MEAN_MOTION);
// Prints: 2026-11-01T05:00:23.168000 15.491394590138162
```

The long tails of digits are just the exact centre of each code; don't read them as extra precision.

### Elements

Under the hood, both formats go through the elements: a plain object with the mean element set in TLE units.

```js
import { decode, encode } from "@racum/kepid";

const el = decode("01a1957a-ebef-73ab-bccf-878de5f3a90d");
console.log(el.epochUnixUs, el.meanMotion, el.inclination);
// Prints: 1793509223168000n 15.491394590138162 51.61619462448406

console.log(encode(el));
// Prints: 01a1957a-ebef-73ab-bccf-878de5f3a90d
```

Important: `epochUnixUs` is a `BigInt` of Unix microseconds, not a `Number`. The spec requires the epoch as an exact integer, and microseconds outgrow `Number`'s safe integers in the year 2255. To get a `Date`, use `new Date(Number(el.epochUnixUs / 1000n))`. When encoding, a safe-integer `Number` is accepted too.

`decode` takes a KepID's text form (hyphenated or not) or its 128-bit `BigInt`. To work with the elements themselves, there are `tleToElements`, `elementsToTle`, `ommToElements` and `elementsToOmm`, and `checkRange(el)` tells you whether an element set can be encoded without encoding it.

### Errors

| Error | When |
|---|---|
| `TleError` | Malformed TLE: wrong length, bad checksum, unreadable field… |
| `OmmError` | Malformed OMM: missing keyword, bad `EPOCH`, or a theory other than SGP4 |
| `EncodeError` | Valid input that can't be a KepID: perigee below 250 km, epoch before 1970… |
| `DecodeError` | Not a KepID: wrong version, variant or profile bits |

```js
import { TleError, tleToKepId } from "@racum/kepid";

const line1 = "1 25544U 98067A   26305.20860139  .00006903  00000+0  13267-3 0  9995";
const line2 = "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592994";

try {
  tleToKepId(line1, line2);
} catch (e) {
  if (e instanceof TleError) console.log(e.message);
}
// Prints: line 1 checksum is 5, expected 4
```

## License

[MIT](https://github.com/racum/kepid/blob/main/LICENSE).
