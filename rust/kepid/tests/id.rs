mod common;

use common::*;
use kepid::{Fields, KepId, ParseError};

fn id(s: &str) -> KepId {
    s.parse().unwrap()
}

#[test]
fn display_is_lowercase_hyphenated() {
    let k = id(ISS_ID);
    assert_eq!(k.to_string(), ISS_ID);
    assert_eq!(k.to_string().len(), 36);
}

#[test]
fn parses_uppercase_and_bare_hex() {
    let k = id(ISS_ID);
    assert_eq!(id(&ISS_ID.to_uppercase()), k);
    assert_eq!(id(&ISS_ID.replace('-', "")), k);
}

#[test]
fn parse_rejects_bad_lengths() {
    assert_eq!("".parse::<KepId>(), Err(ParseError::Length(0)));
    assert_eq!(ISS_ID[..35].parse::<KepId>(), Err(ParseError::Length(35)));
    assert_eq!(format!("{ISS_ID}0").parse::<KepId>(), Err(ParseError::Length(37)));
    assert_eq!(ISS_ID.replace('-', "")[..31].parse::<KepId>(), Err(ParseError::Length(31)));
}

#[test]
fn parse_rejects_non_hex() {
    let bad = ISS_ID.replace('0', "g");
    assert_eq!(bad.parse::<KepId>(), Err(ParseError::NotHex));
    assert_eq!(format!(" {}", &ISS_ID[1..]).parse::<KepId>(), Err(ParseError::NotHex));
}

#[test]
fn parse_rejects_misplaced_hyphens() {
    // Right length, hyphens one character off.
    assert_eq!("01a17b912-3e2-73ab-813c-86cddaf3a10c".parse::<KepId>(), Err(ParseError::Hyphens));
    // 36 characters with no hyphens at all.
    assert_eq!(format!("{}0000", ISS_ID.replace('-', "")).parse::<KepId>(), Err(ParseError::Hyphens));
    // Hyphens sprinkled into a bare id, or padding the ends.
    assert!("0-1a17b9123e273-ab813c86cddaf3a10c-".parse::<KepId>().is_err());
    assert!("--01a17b9123e273ab813c86cddaf3a10c".parse::<KepId>().is_err());
    assert!("01a17b91-23e273-ab-813c-86cddaf3a10c".parse::<KepId>().is_err());
    // A hyphen where a digit belongs in a 32-character string.
    let bare = ISS_ID.replace('-', "");
    let with_hyphen = format!("{}-{}", &bare[..8], &bare[9..]);
    assert_eq!(with_hyphen.len(), 32);
    assert_eq!(with_hyphen.parse::<KepId>(), Err(ParseError::NotHex));
}

#[test]
fn bytes_are_in_rfc_order() {
    let k = id(ISS_ID);
    let b = k.to_bytes();
    assert_eq!(b[0], 0x01);
    assert_eq!(b[1], 0xa1);
    assert_eq!(b[6] >> 4, 7, "version nibble is the high half of byte 6");
    assert_eq!(b[8] >> 6, 0b10, "variant is the top two bits of byte 8");
    assert_eq!(b[15], 0x0c);
    assert_eq!(KepId::from_bytes(b), k);
}

#[test]
fn u128_round_trips() {
    for v in [0, 1, u128::MAX, id(ISS_ID).as_u128()] {
        assert_eq!(KepId::from_u128(v).as_u128(), v);
    }
}

#[test]
fn accessors_read_markers_profile_and_epoch() {
    let k = id(ISS_ID);
    assert_eq!(k.version(), 7);
    assert_eq!(k.variant(), 0b10);
    assert_eq!(k.profile(), 0);
    assert_eq!(k.epoch_ticks(), 7_004_197_155);
    assert_eq!(k.epoch_unix_us(), ISS_EPOCH_US);
}

#[test]
fn ids_sort_by_epoch() {
    let at = |ticks: u64| Fields { epoch_ticks: ticks, ..Fields::unpack(id(ISS_ID)) }.pack();
    let mut ids = [at(30), at(2), at(MAX_TICKS), at(0), at(7)];
    ids.sort();
    let ticks: Vec<u64> = ids.iter().map(|k| k.epoch_ticks()).collect();
    assert_eq!(ticks, [0, 2, 7, 30, MAX_TICKS]);
}

#[test]
fn all_zero_fields_pack_to_just_the_markers() {
    assert_eq!(Fields::default().pack().to_string(), "00000000-0000-7000-8000-000000000000");
}

#[test]
fn all_one_fields_pack_to_all_ones() {
    let f = Fields {
        epoch_ticks: MAX_TICKS,
        arg_perigee: 255,
        inclination: 4095,
        raan: 16_383,
        drag: 1023,
        profile: 1,
        lambda: 8191,
        mean_motion: 131_071,
        eccentricity: 127,
    };
    assert_eq!(f.pack().to_string(), "ffffffff-ffff-7fff-bfff-ffffffffffff");
    assert_eq!(Fields::unpack(f.pack()), f);
}

#[test]
fn fields_sit_at_their_offsets_and_do_not_overlap() {
    let zero = Fields::default();
    let cases = [
        (Fields { epoch_ticks: MAX_TICKS, ..zero }, 0, 40),
        (Fields { arg_perigee: 255, ..zero }, 40, 8),
        (Fields { inclination: 4095, ..zero }, 52, 12),
        (Fields { raan: 16_383, ..zero }, 66, 14),
        (Fields { drag: 1023, ..zero }, 80, 10),
        (Fields { profile: 1, ..zero }, 90, 1),
        (Fields { lambda: 8191, ..zero }, 91, 13),
        (Fields { mean_motion: 131_071, ..zero }, 104, 17),
        (Fields { eccentricity: 127, ..zero }, 121, 7),
    ];
    let markers = zero.pack().as_u128();
    let mut union = 0u128;
    for (f, offset, width) in cases {
        assert_eq!(Fields::unpack(f.pack()), f, "{f:?}");
        let bits = f.pack().as_u128() & !markers;
        let expected = ((1u128 << width) - 1) << (128 - offset - width);
        assert_eq!(bits, expected, "{f:?} at {offset}/{width}");
        assert_eq!(union & bits, 0, "{f:?} overlaps");
        union |= bits;
    }
    // Every bit is a field or a marker.
    assert_eq!(union | (0xF << 76) | (0b11 << 62), u128::MAX);
}

#[test]
fn unpack_ignores_marker_bits() {
    let a = Fields::unpack(KepId::from_u128(id(ISS_ID).as_u128() & !(0xF << 76) & !(0b11 << 62)));
    assert_eq!(a, Fields::unpack(id(ISS_ID)));
}

#[test]
fn validate_checks_markers_in_order() {
    use kepid::DecodeError::*;
    let iss = id(ISS_ID).as_u128();
    assert_eq!(id(ISS_ID).validate(), Ok(()));
    assert_eq!(KepId::from_u128(iss & !(0xF << 76) | (1 << 76)).validate(), Err(NotUuidV7(1)));
    assert_eq!(KepId::from_u128(iss | (0b11 << 62)).validate(), Err(BadVariant(0b11)));
    assert_eq!(KepId::from_u128(iss | (1 << 37)).validate(), Err(UnsupportedProfile(1)));
    let wrong_both = KepId::from_u128(iss & !(0xF << 76) | (1 << 37));
    assert_eq!(wrong_both.validate(), Err(NotUuidV7(0)));
}
