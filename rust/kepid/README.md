# KepID for Rust

Satellite orbits packed into UUIDv7: an SGP4 element set, the content of a TLE or an OMM, in a single 128-bit value that is also a valid UUIDv7. This is the Rust library; for what KepID is and why, start with the [main README](https://github.com/racum/kepid), and for every rule and constant, the [spec](https://github.com/racum/kepid/blob/main/docs/spec.md).

## Install

The library is a Rust crate with no dependencies:

```bash
cargo add kepid
```

For OMM support (a serde struct you can read and write with serde_json or any other serde format), turn on the `omm` feature:

```bash
cargo add kepid --features omm
```

Pro tip: if you read OMM JSON with serde_json, also turn on its `float_roundtrip` feature. Without it, a number can be parsed one bit off, and that's sometimes enough to change a KepID.

## Usage

Turning a TLE of the ISS into a KepID, and back:

```rust
fn main() -> Result<(), kepid::TleError> {
    let line1 = "1 25544U 98067A   26305.20860139  .00006903  00000+0  13267-3 0  9994";
    let line2 = "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592994";

    // TLE to KepID:
    let id = kepid::from_tle(line1, line2)?;
    println!("{id}");
    // Prints:
    // 01a1957a-ebef-73ab-bccf-878de5f3a90d

    // KepID to TLE (mind the placeholders for the uncarried values):
    let [new1, new2] = kepid::to_tle(id)?;
    println!("{new1}\n{new2}");
    // Prints:
    // 1 00000U          26305.20860148  .00000000  00000+0  13155-3 0  9992
    // 2 00000  51.6162 342.0483 0007474 334.2037  25.8607 15.49139459999994

    Ok(())
}
```

Notice that the TLE that comes back is not the one that went in: every element is rounded to its code, and the epoch to its 256 ms tick. That's the lossy part, and the reason it fits in 16 bytes!

OMM works the same way: with the `omm` feature on, `from_omm`, `to_omm` and `to_omm_for` mirror the TLE functions, using a serde `Omm` struct you can read and write with serde_json or any other serde format.

For a command-line tool, see [`kepid-cli`](https://crates.io/crates/kepid-cli).

## License

[MIT](https://github.com/racum/kepid/blob/main/LICENSE).
