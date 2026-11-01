# KepID: profile 0

A **KepID** packs an SGP4 mean element set, as carried by a TLE or an OMM, into one 128-bit value that is a valid UUIDv7:

```
01a17b91-23e2-73ab-813c-86cddaf3a10c    ← the ISS, 2026-10-27 04:14:31.68 UTC
```

- **Lossy.** Each element is rounded to a code. Decoding returns an element set that SGP4 can propagate, close to the original but not equal to it.
- **Not unique.** Two element sets that round to the same codes get the same KepID. A KepID describes an orbit, not an object, and carries no identifier.
- **Sortable.** The first 40 bits are the epoch, so KepIDs sort by time, like any UUIDv7.

**A UUIDv7 in form, not in spirit.** A KepID has the version and variant bits of RFC 9562 [RFC9562] and a timestamp in the first 48 bits, so it can be stored, indexed and sorted wherever a UUIDv7 can. It is not a UUIDv7 in the usual sense:
- **Nothing is random.** The bits a UUIDv7 fills with random data carry orbital elements. The same element set always gives the same KepID, and two element sets that round alike collide by design.
- **The timestamp is the element set's epoch,** not the time the value was made, and only to 256 ms: the lowest 8 bits of the 48-bit millisecond timestamp carry the argument of perigee. Read as an ordinary UUIDv7 timestamp, a KepID is off by 0 to 255 ms (section 3).
- **It cannot be recognized from its bits.** Every value with the UUIDv7 version and variant and a 0 in bit 90 decodes to a valid orbit, so about half of all ordinary UUIDv7s also decode as KepIDs. Whether a value is a KepID MUST come from its context, such as a field name or a column type, never from the value itself.
- A KepID MUST NOT be used where a UUIDv7's uniqueness or creation time is relied on.

This document defines **profile 0**, the only profile so far: the bit layout, the conversions in both directions, every constant, and the rules for an encoder and a decoder. It is the normative reference.

Why the format looks the way it does (the data the curves were fitted to, the fitting method, the field widths, the accuracy and the resolution of every field) is explained in the companion document, [rationale.md](rationale.md), which is informative.

---

## 1. Contents of a KepID

| Element | Symbol | Unit | Stored as |
|---|---|---|---|
| Epoch (UTC) | t | Unix time, whole µs | 256 ms ticks |
| Mean motion (SGP4/Kozai, as in a TLE) | n | rev/day | w = R_floor / a |
| Eccentricity | e | – | s = e / (1 − w) |
| Inclination | i | degrees | i |
| Right ascension of the ascending node | Ω | degrees | Ω |
| Argument of perigee | ω | degrees | ω |
| Mean anomaly | M | degrees | λ = ω + M (mean argument of latitude) |
| Drag term | B* | 1 / Earth radii | D = B* × drag factor |

The elements are the ones a TLE carries, in the TEME frame. In an OMM [CCSDS-ODM] they are `EPOCH`, `MEAN_MOTION`, `ECCENTRICITY`, `INCLINATION`, `RA_OF_ASC_NODE`, `ARG_OF_PERICENTER`, `MEAN_ANOMALY` and `BSTAR`, with `REF_FRAME` = `TEME` and `MEAN_ELEMENT_THEORY` = `SGP4`, in the same units. Element sets of other theories, such as `SGP4-XP`, are not SGP4 mean elements, and a KepID cannot carry them faithfully.

**SGP4.** The elements are the mean elements of SGP4, the propagation model that TLEs, and OMMs with `MEAN_ELEMENT_THEORY` = `SGP4`, are made for. SGP4 is defined in Spacetrack Report No. 3 [SPACETRACK3], and its current public form in "Revisiting Spacetrack Report #3" [VALLADO2006]. For orbits with a period of 225 minutes or more, SGP4 switches to its deep-space branch, often called SDP4; in this document, "SGP4" covers both. KepID itself never runs SGP4: encoding and decoding only round the elements. It borrows two SGP4 constants for the drag factor (section 6.3): the WGS-72 equatorial radius, 6378.135 km, and the 78 km altitude of SGP4's atmospheric reference surface. A decoded element set is meant to be propagated with SGP4, like the element set it came from.

**Not carried:** catalog number, international designator, element set number, revolution number, and the first and second derivatives of the mean motion (SGP4 does not use them). A decoder sets the derivatives to 0.

**Valid input:** any element set with:
- 0 ≤ e < 1;
- n > 0;
- 0 ≤ i ≤ 180°;
- a perigee at least **250 km** above the equatorial radius, that is a (1 − e) ≥ R_E + 250 km;
- an epoch from 1970-01-01T00:00Z up to the last 256 ms tick that fits in 40 bits (about the year 10,900).

Every other closed orbit can be encoded, from LEO out past the Moon's distance. The perigee floor is nominal: a is the two-body semi-major axis that goes with n (section 7, step 1), not SGP4's own. A UUIDv7 timestamp cannot be negative, so element sets from before 1970 cannot be encoded. What these limits exclude is described in the rationale.

## 2. Conventions

- **Requirements language.** The key words "MUST", "MUST NOT", "REQUIRED", "SHOULD", "SHOULD NOT" and "MAY" in this document are to be interpreted as described in BCP 14 [BCP14] (RFC 2119, RFC 8174) when, and only when, they appear in all capitals.
- **Normative text.** Formulas in code blocks are normative and are evaluated exactly as written. Formulas in running text, such as `360 · n · Δt`, explain the code blocks and are not normative.
- **Bit numbering.** Bits are numbered 0 to 127 from the most significant. The 128-bit value is big-endian; its text form is the usual UUID form, 32 lowercase hexadecimal digits grouped 8-4-4-4-12. Field widths are in bits, and a field's offset is the number of its most significant bit.
- **Arithmetic.** Arithmetic is IEEE 754 [IEEE754] binary64 with round-to-nearest-even.
  - Every formula MUST be evaluated exactly as written: left to right, honoring the parentheses, with no fused multiply-add and no reordering. Implementations MUST keep the compiler from doing either. For example: with GCC and Clang, use `-ffp-contract=off` and no fast-math; with MSVC, avoid `/fp:fast`; in Go, put explicit `float64(...)` conversions around products on architectures where it fuses.
  - Constants are the decimal literals given here, parsed to the nearest binary64.
  - The epoch is the exception: it is an integer, and everything done to it before step 6 of section 7 is exact integer arithmetic.
- **Functions:**
  - `floor(x)`
  - `round(x)`: to nearest, **halves away from zero**. JavaScript's `Math.round` is not this.
  - `sqrt(x)`
  - `cbrt(x)`: the cube root, for x > 0, computed by exactly this procedure and not by a library function:
    ```
    m = x
    p = 1
    while m < 1:  m = m * 8;  p = p / 2
    while m ≥ 8:  m = m / 8;  p = p * 2
    y = 1.5
    repeat 6 times:  y = (2 * y + m / (y * y)) / 3
    cbrt(x) = y * p
    ```
    The scaling by 8 and 2 is exact, and six Newton steps from 1.5 land within one unit in the last place of the true cube root for every m. The result is the one this procedure gives, even where it differs in the last bit from a correctly rounded cube root.
  - `abs(x)`
  - `copysign(x, y)`: the magnitude of x with the sign of y
  - `min`, `max`, and `clamp(x, lo, hi) = min(max(x, lo), hi)`
  - `mod(x, m)`: the Euclidean remainder, `fmod(x, m)` plus m when that is negative
  - `wrap(x)`: `mod(x, 360)`, with a result of exactly 360, or of −0, replaced by 0
- **Units.** Angles are in degrees, epochs in whole microseconds, distances in kilometres.

## 3. Bit layout

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                   epoch, 256 ms ticks [39:8]                  |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  epoch [7:0]  |   ω (8 bits)  |  ver  |        i (12 bits)    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|var|        Ω (14 bits)        |    D (10 bits)    |P| λ, 13 → |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   λ (cont.)   |           w (17 bits)           | s (7 bits)  |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

| Offset | Bits | Field | Content | Hex digits |
|---|---|---|---|---|
| 0 | 40 | `tick` | epoch in 256 ms ticks since 1970-01-01T00:00Z | 1–10 |
| 40 | 8 | `ω` | argument of perigee | 11–12 |
| 48 | 4 | `ver` | `0111` (UUID version 7) | 13 |
| 52 | 12 | `i` | inclination | 14–16 |
| 64 | 2 | `var` | `10` (RFC 9562 variant) | 17 |
| 66 | 14 | `Ω` | right ascension of the ascending node | 17–20 |
| 80 | 10 | `D` | drag | 21–23 |
| 90 | 1 | `P` | profile bit, `0` for profile 0 | 23 |
| 91 | 13 | `λ` | mean argument of latitude | 23–26 |
| 104 | 17 | `w` | mean motion | 27–31 |
| 121 | 7 | `s` | eccentricity | 31–32 |

Every field is one contiguous run of bits, and no field crosses the version or variant bits.

Which field owns each hex digit (`*` marks a digit shared by two fields):

```
TTTTTTTT-TTOO-VIII-*AAA-DD*LLLNNNN*E
```

T = tick, O = ω, V = version, I = i, A = Ω, D = drag, L = λ, N = w, E = s. Digit 17 holds the variant bits and the top of Ω. Digit 23 holds the end of D, the profile bit and the top of λ. Digit 31 holds the end of w and the top of s.

**The profile bit (bit 90)** selects the layout. Its position is the same in every profile. Value 1 is reserved, and a profile 0 decoder MUST refuse it. A future profile 1 is expected to start its layout with further selector bits of its own, so the profile space can grow as a prefix code, the way the UUID variant field grew (`0`, `10`, `110`, `111`).

**UUIDv7 readers** see bits 0–47 as a millisecond timestamp. For a KepID that timestamp is `tick × 256 + code(ω)`, which lies 0 to 255 ms after the encoded epoch.

## 4. Constants

### 4.1 Physical and derived constants

| Name | Value | Definition |
|---|---|---|
| `MU` | 398600.4418 | Earth's gravitational parameter, km³/s² (WGS-84 [WGS84]) |
| `RE` | 6378.137 | Earth's equatorial radius, km (WGS-84 [WGS84]) |
| `R_FLOOR` | 6628.137 | `RE + 250`, the perigee floor radius, km |
| `TAU` | 6.283185307179586 | 2π |
| `N_FLOOR` | 16.088499719622039 | `(86400 / TAU) × sqrt(MU / ((R_FLOOR × R_FLOOR) × R_FLOOR))`: circular mean motion at the floor, rev/day |
| `C` | 1.039196724434337 | `R_FLOOR / 6378.135`: floor radius in SGP4 (WGS-72) Earth radii |
| `S` | 1.0122292801892716 | `1 + 78 / 6378.135`: SGP4's atmospheric reference radius s, Earth radii |
| `G_REF` | 50284.883336077255 | `g(R_FLOOR / (RE + 500), 0)`: the drag factor of a circular orbit at 500 km (section 6.3) |
| `D_MAX` | 1 | largest magnitude of D |

The derived values are given to 17 significant digits. Use them as literals: they are the exact binary64 results of their definitions.

### 4.2 Field maps

| Field | Kind | Domain | Codes N | Curve |
|---|---|---|---|---|
| `ω` | circular | [0°, 360°), `lo` = 0 | 256 | 2 layers |
| `i` | interval | [0°, 180°], `lo` = 0, `W` = 180 | 4096 | 5 layers |
| `Ω` | circular | [0°, 360°), `lo` = 0 | 16384 | none (uniform) |
| `D` | signed | [−1, 1], `X` = D_MAX | 1024 | 4 layers |
| `λ` | circular | [−180°, 180°), `lo` = −180 | 8192 | 5 layers |
| `w` | interval | [0, 1], `lo` = 0, `W` = 1 | 131072 | 5 layers |
| `s` | interval | [0, 1], `lo` = 0, `W` = 1 | 128 | 4 layers |

The layers (a, c, g) of every curve are listed in Appendix A.

## 5. Curves

A curve maps a normalized value z ∈ [0, 1] to a fraction of the codes t ∈ [0, 1]. It is monotonic, with C(0) = 0 and C(1) = 1, and it has a closed-form inverse. Its slope is the local code density: steep where element sets are common, and never below a floor elsewhere.

**One layer.** A layer blends the identity with an algebraic sigmoid centred at c, of width g and share a:

```
ss(u) = u / (1 + abs(u))

L = ss(-c / g)
M = ss((1 - c) / g) - L

forward   h(z):  u = (z - c) / g
                 h = (1 - a) * z + (a * (ss(u) - L)) / M

inverse   h⁻¹(t): α = (1 - a) * g
                  β = a / M
                  τ = (t - (1 - a) * c) + β * L
                  B = (α + β) - abs(τ)
                  U = (sqrt(B * B + (4 * α) * abs(τ)) - B) / (2 * α)
                  z = clamp(c + g * copysign(U, τ), 0, 1)
```

The inverse is exact, not an approximation. Writing z = c + g·u, the layer becomes α·u + β·u/(1 + |u|) = τ. That has the sign of τ, and |u| is the positive root of α·U² + (α + β − |τ|)·U − |τ| = 0. Every layer keeps a share (1 − a) of uniform density, so the product of those shares is a hard floor on the density of the whole curve.

**A curve** with layers 1 … K:

```
C(z)   = h_K( … h_2( h_1(z) ) … )            (layer 1 first)
C⁻¹(t) = h_1⁻¹( h_2⁻¹( … h_K⁻¹(t) … ) )      (layer K first)
```

A field with no layers uses C(z) = z and C⁻¹(t) = t.

## 6. Quantizers

Each field turns a value x into a code k of N codes, and back. There are three kinds.

### 6.1 Interval fields (`i`, `w`, `s`)

Domain [lo, lo + W].

```
encode:  z = clamp((x - lo) / W, 0, 1)
         t = C(z)
         k = clamp(floor(t * N), 0, N - 1)

decode:  t = clamp((k + 0.5) / N, 0, 1)
         x = lo + W * C⁻¹(t)
```

### 6.2 Circular fields (`ω`, `Ω`, `λ`)

360° starting at lo.

```
encode:  z = mod(x - lo, 360) / 360
         t = C(z)
         k = mod(round(t * N), N)

decode:  t = k / N
         x = lo + 360 * C⁻¹(t)
```

### 6.3 The signed field (`D`)

Sign and magnitude, with an exact zero code k = N/2.

```
encode:  z = min(abs(x) / X, 1)
         t = copysign(C(z), x)
         k = clamp(round((t * N) / 2), -N/2, N/2 - 1) + N/2

decode:  t = clamp((k - N/2) / (N/2), -1, 1)
         x = X * copysign(C⁻¹(abs(t)) - C⁻¹(0), t)
```

Subtracting `C⁻¹(0)` makes the zero code decode to exactly zero, with no special case.

The two signs are not symmetric. The negative side has N/2 codes, and its last one decodes to −X up to rounding. The positive side has N/2 − 1, so its largest value is one code short of X, and larger positive values saturate there.

**The drag factor.** Here w and e are the decoded values, and C and S are the constants of section 4.1:

```
g(w, e):  q   = C - S * w
          η   = (C * e) / q
          x   = max(1 - η * η, 1e-12)
          w2  = w * w
          q2  = q * q
          g   = ((w2 * w2) * w) / (q2 * q2) / (((x * x) * x) * sqrt(x))

drag(w, e) = g(w, e) / G_REF
```

`g` is SGP4's own sensitivity of the along-track position to B*, written in w and e: a²·n²·(a − s)⁻⁴·(1 − η²)⁻³·⁵. D = 1 for B* = 1 on a circular orbit at 500 km.

## 7. Encoding

Input: the epoch as a whole number of Unix microseconds, `epoch_us` (UTC, see "Epoch input" below), and n, e, i, Ω, ω, M, B*. All values MUST be finite.

**Epoch input.** The epoch MUST reach the encoder as an exact integer, never through a floating-point day or Julian date. One element set in about 8,000 has its epoch exactly half a tick from a tick boundary, and there any float error picks the other tick, and so a different KepID.
- **TLE** (`YYDDD.DDDDDDDD`, line 1, columns 19–32). Read the two-digit year, the day of the year and up to 8 fraction digits as integers. Years 57–99 are 1957–1999 and 00–56 are 2000–2056; day 1 is 1 January. Pad the fraction on the right with zeros to 8 digits, giving k units of 10⁻⁸ day; each is exactly 864 µs.
  ```
  epoch_us = (days(year, 1, 1) + day - 1) * 86400000000 + k * 864
  ```
  `days(y, m, d)` is the number of days from 1970-01-01 in the proleptic Gregorian calendar.
- **OMM** (`EPOCH`, ISO 8601 [ISO8601] UTC). Use the date and time fields as integers, and the fraction of a second to 6 digits, padded on the right with zeros. A fraction with more than 6 digits rounds to the microsecond on its decimal digits, halves up.
- As with Unix time, every day is 86,400 s; leap seconds are not counted.

1. **Validate.** The encoder MUST refuse the element set if any of the following holds:
   - `n ≤ 0`, `e < 0`, `e ≥ 1`, `i < 0` or `i > 180`;
   - `epoch_us < 0`, or `tick` (step 2) is above 2⁴⁰ − 1;
   - the perigee is below the floor. Compute
     ```
     v = n / N_FLOOR
     w = cbrt(clamp(v * v, 1e-30, 1e30))
     ```
     and refuse if `1 - e < w * (1 - 1e-9)`. The tolerance of one part in 10⁹ lets every decoded element set encode again.

     Here w = R_FLOOR / a, with a the two-body semi-major axis: N_FLOOR is the circular mean motion at R_FLOOR, so w = (n / N_FLOOR)^(2/3). The clamp changes no result: any w below 10⁻¹⁰ falls in w's lowest code, and any w above 10¹⁰ is refused.

2. **Epoch.**
   In integers, with `div` the integer quotient (the remainder dropped):
   ```
   tick = (epoch_us + 128000) div 256000
   Δt   = tick * 256000 - epoch_us
   ```
   The tick is the nearest one, halves rounding up. Δt is an integer from −128 000 to 127 999 µs; from here on it is used as a binary64, which holds it exactly.

3. **Mean motion.** `k_w = encode_w(w)` with w from step 1. Then decode it, `ŵ = decode_w(k_w)`; the next steps need the decoded value.

4. **Eccentricity.**
   ```
   k_s = encode_s(e / (1 - ŵ))
   ŝ   = decode_s(k_s)
   ê   = (1 - ŵ) * ŝ
   ```

5. **Argument of perigee.** `k_ω = encode_ω(wrap(ω))`.

6. **Mean argument of latitude.** The mean anomaly is moved to the rounded epoch: a satellite travels 360 · n degrees a day, so the rounding Δt is worth 360 · n · Δt / 86 400 000 000 degrees.
   ```
   λ   = wrap((ω + M) + ((360 * n) * Δt) / 86400000000)
   k_λ = encode_λ(λ)
   ```

7. **Node and inclination.** `k_Ω = encode_Ω(wrap(Ω))` and `k_i = encode_i(i)`.

8. **Drag.**
   ```
   D   = B* * drag(ŵ, ê)
   k_D = encode_D(D)
   ```
   A |D| above D_MAX saturates at the largest code of its sign.

9. **Assemble.** Start from `tick × 2⁸⁸`. Set the version bits 48–51 to `0111`, the variant bits 64–65 to `10` and the profile bit 90 to `0`. Place each code with its most significant bit at the field's offset: for a field of width b at offset o, OR in `k << (128 − o − b)`.

The order of steps 3, 4 and 8 matters: the eccentricity is scaled with the decoded w, and the drag factor uses the decoded w and e. A decoder can then rebuild everything from the codes alone.

## 8. Decoding

1. **Check.** The decoder MUST refuse the value unless bits 48–51 are `0111`, bits 64–65 are `10` and bit 90 is `0`. It MUST NOT refuse a value for any other reason.
2. **Epoch.** `epoch_us = tick × 256000`, a whole number of milliseconds.
3. **Fields.** Decode every code to its value: `w`, `s`, `ω`, `Ω`, `λ`, `i`, `D`.
4. **Elements:**
   ```
   n  = (N_FLOOR * w) * sqrt(w)
   e  = (1 - w) * s
   i  = i
   Ω  = wrap(Ω)
   ω  = wrap(ω)
   M  = wrap(λ - ω)
   B* = D / drag(w, e)
   ```
   The first and second derivatives of n are 0.

Decoding uses only +, −, ×, ÷, sqrt, abs and copysign: no trigonometry and no cube root.

## 9. Guarantees

- **Every ID decodes to a valid orbit.** Every 128-bit value with the right version, variant and profile bits decodes to a closed orbit (0 ≤ e < 1, n > 0) whose perigee is at or above the 250 km floor, because e = (1 − w)·s with s ≤ 1. A decoder never has to refuse a value it can parse.
- **Re-encoding is exact.** Encoding a decoded element set returns the same KepID.
- **The epoch is consistent.** The decoded element set describes the satellite at its decoded epoch, because section 7, step 6 carries the position to the rounded tick.
- **Encoding is bit-exact** across implementations that follow section 2: every operation is exact integer arithmetic, fully specified by IEEE 754, or a procedure defined there.
- **Decoding** produces binary64 values that may differ between implementations in the last digits. Conforming decoders agree to a relative tolerance of 10⁻¹².
- **Saturation.** Drag effects beyond D_MAX saturate. In practice that means a B* of the order of 0.03 or more at 250 km, or of 1 or more at 500 km, values typical of re-entering objects.
- **Small B\*.** A B* whose effect is below the drag resolution decodes as 0. This happens at high altitude, where drag is negligible.
- **Resolution.** Every field has a worst-case step that no part of its range exceeds. The rationale lists it, with the typical steps and the accuracy.

## 10. Test vectors

[`profile0_vectors.csv`](profile0_vectors.csv), next to this document, has one row per case: the input element set, the expected KepID, and the decoded element set.

Columns:
- `case` and `epoch_utc`;
- the input `epoch_unix_us`, `mean_motion`, `eccentricity`, `inclination`, `raan`, `arg_perigee`, `mean_anomaly` and `bstar`;
- `kepid`, or `REFUSED: reason`;
- the decoded values with the `dec_` prefix.

The cases cover real element sets (ISS, a sun-synchronous satellite, a Starlink shell, negative B*, eccentric debris, GEO, GPS, a 12-hour highly elliptical orbit, a retrograde orbit, an orbit beyond GEO) and edge cases:
- e = 0 with B* = 0;
- i = 0 and i = 180;
- Ω and λ just below 360°;
- a saturated drag;
- a perigee just above and just below the floor;
- an epoch exactly half a tick off.

The ISS, epoch 2026-10-27T04:14:31.558560Z:

| | epoch_unix_us | n | e | i | Ω | ω | M | B* |
|---|---|---|---|---|---|---|---|---|
| input | 1793074471558560 | 15.49073103 | 0.0006948 | 51.6336 | 6.9404 | 314.9066 | 45.1358 | 0.00011132 |
| decoded | 1793074471680000 | 15.490749447697493 | 0.0006625520740656849 | 51.61619462448406 | 6.943359375 | 314.76816165016845 | 45.281856203811856 | 0.00011417386721999137 |

```
KepID: 01a17b91-23e2-73ab-813c-86cddaf3a10c
```

## 11. Conformance

- **An encoder** conforms if it implements section 7 as written. For every row of the test vectors it MUST produce the listed KepID, or refuse the rows marked `REFUSED`.
- **A decoder** conforms if it implements section 8 as written. For every listed KepID it MUST produce the listed values to a relative tolerance of 10⁻¹², and the epoch exactly.
- An implementation MAY reject input it cannot represent, such as a non-finite value, before section 7 starts. It MUST NOT produce a KepID for an element set that section 7, step 1 refuses.

## 12. References

- **[BCP14]** S. Bradner, "Key words for use in RFCs to Indicate Requirement Levels", RFC 2119, March 1997; and B. Leiba, "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", RFC 8174, May 2017.
- **[RFC9562]** K. Davis, B. Peabody, P. Leach, "Universally Unique IDentifiers (UUIDs)", RFC 9562, May 2024.
- **[IEEE754]** "IEEE Standard for Floating-Point Arithmetic", IEEE 754-2019.
- **[SPACETRACK3]** F. R. Hoots, R. L. Roehrich, "Models for Propagation of NORAD Element Sets", Spacetrack Report No. 3, December 1980.
- **[VALLADO2006]** D. A. Vallado, P. Crawford, R. Hujsak, T. S. Kelso, "Revisiting Spacetrack Report #3", AIAA 2006-6753, AIAA/AAS Astrodynamics Specialist Conference, August 2006.
- **[CCSDS-ODM]** "Orbit Data Messages", CCSDS 502.0-B-3, Blue Book, April 2023 (also ISO 26900:2024).
- **[WGS84]** "Department of Defense World Geodetic System 1984", NIMA TR8350.2, third edition.
- **[ISO8601]** "Date and time — Representations for information interchange", ISO 8601.

## Appendix A. Curve layers

Each curve is a list of layers (a, c, g), applied in the order listed (section 5).

**ω**

| layer | a | c | g |
|---|---|---|---|
| 1 | 4.083428629e-2 | 7.664452626e-1 | 3.990766569e-2 |
| 2 | 5.507395214e-2 | 6.064244552e-2 | 1.176634895e-1 |

**i**

| layer | a | c | g |
|---|---|---|---|
| 1 | 1.082992940e-2 | 2.953274111e-1 | 2.777786713e-4 |
| 2 | 1.630044140e-2 | 2.363204369e-1 | 2.777795907e-4 |
| 3 | 6.892137273e-2 | 5.603886736e-1 | 6.659174209e-3 |
| 4 | 4.314797970e-1 | 3.784270310e-1 | 1.173090567e-1 |
| 5 | 2.925692630e-2 | 6.592332912e-1 | 1.631219892e-3 |

**D**

| layer | a | c | g |
|---|---|---|---|
| 1 | 9.623760485e-1 | 1.589264601e-9 | 5.318080922e-2 |
| 2 | 2.954822161e-1 | 7.986615314e-1 | 3.778745014e-2 |
| 3 | 1.786089996e-1 | 1.116401956e-2 | 1.338212342e-2 |
| 4 | 8.140581342e-2 | 2.365399929e-11 | 1.654625459e-2 |

**λ**

| layer | a | c | g |
|---|---|---|---|
| 1 | 2.506724280e-3 | 5.001930575e-1 | 4.161572285e-4 |
| 2 | 1.616032485e-1 | 5.781309497e-1 | 8.902241343e-2 |
| 3 | 5.978329411e-2 | 4.579863973e-1 | 2.595466093e-3 |
| 4 | 2.874542339e-2 | 9.376973790e-1 | 9.133208180e-3 |
| 5 | 1.792777376e-2 | 9.882822534e-1 | 6.059456682e-3 |

**w**

| layer | a | c | g |
|---|---|---|---|
| 1 | 7.894998790e-1 | 8.748975979e-1 | 6.707300842e-2 |
| 2 | 3.912329768e-1 | 8.693916408e-1 | 1.117938012e-1 |
| 3 | 1.953420518e-1 | 9.114594208e-1 | 4.088966496e-2 |
| 4 | 2.833931871e-1 | 9.338001081e-1 | 1.497879339e-2 |
| 5 | 3.233338923e-1 | 2.450968642e-2 | 3.173610361e-2 |

**s**

| layer | a | c | g |
|---|---|---|---|
| 1 | 4.492276748e-1 | 4.502440907e-3 | 6.406040192e-2 |
| 2 | 6.128914601e-2 | 8.756510763e-27 | 2.929546956e-2 |
| 3 | 8.952636190e-1 | 1.000000000e0 | 6.953619088e-1 |
| 4 | 7.664520029e-2 | 9.896597597e-1 | 4.532361035e-2 |
