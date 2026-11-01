//! The bit layout card: every field's decoded value, and the 128 bits with
//! each field's bits in its own color.

use kepid::profile0::*;
use kepid::{Elements, Fields, KepId};

/// One field of the card: its label, its bits and its ANSI color, if any.
struct Field {
    label: &'static str,
    bits: (u32, u32),
    color: Option<&'static str>,
}

/// The UUID version and variant bits; the "UUID format" row shares their color.
const MARKER_COLOR: &str = "2;37";

const FIELDS: [Field; 10] = [
    Field { label: "KepID", bits: PROFILE_BITS, color: None },
    Field { label: "UUID format", bits: VERSION_BITS, color: Some(MARKER_COLOR) },
    Field { label: "Epoch", bits: EPOCH_BITS, color: Some("36") },
    Field { label: "Perigee (ω)", bits: ARG_PERIGEE_BITS, color: Some("35") },
    Field { label: "Inclination (i)", bits: INCLINATION_BITS, color: Some("33") },
    Field { label: "Ascension (Ω)", bits: RAAN_BITS, color: Some("34") },
    Field { label: "Drag (D)", bits: DRAG_BITS, color: Some("31") },
    Field { label: "Mean latitude (λ)", bits: LAMBDA_BITS, color: Some("32") },
    Field { label: "Mean motion (w)", bits: MEAN_MOTION_BITS, color: Some("96") },
    Field { label: "Eccentricity (s)", bits: ECCENTRICITY_BITS, color: Some("95") },
];

/// Width of the left column of the bit rows, `01a1 957a`.
const HEX_WIDTH: usize = 9;
/// Width of the right column of the bit rows, 32 bits with their spacing.
const BITS_WIDTH: usize = 43;
/// Width of the label column: the longest label.
const LABEL_WIDTH: usize = 17;
/// Width of the value column, so both sections are as wide as each other.
const VALUE_WIDTH: usize = HEX_WIDTH + BITS_WIDTH - LABEL_WIDTH;

/// Renders the card of a valid KepID, from its decoded elements. `color` turns
/// the ANSI colors on.
pub(crate) fn card(id: KepId, el: &Elements, color: bool) -> String {
    let paint = |text: &str, code: Option<&str>| match code {
        Some(code) if color => format!("\x1b[{code}m{text}\x1b[0m"),
        _ => text.to_string(),
    };
    let values = values(id, el);
    let line = |n: usize| "━".repeat(n);
    let thin = |n: usize| "─".repeat(n);

    let mut out = format!("┏{}┯{}┓\n", line(LABEL_WIDTH + 2), line(VALUE_WIDTH + 2));
    for (i, (field, value)) in FIELDS.iter().zip(&values).enumerate() {
        let label = paint(&pad(field.label, LABEL_WIDTH), field.color);
        out.push_str(&format!("┃ {label} │ {} ┃\n", pad(value, VALUE_WIDTH)));
        // The identification rows (KepID, UUID format) apart from the orbit.
        if i == 1 {
            out.push_str(&format!("┠{}┼{}┨\n", thin(LABEL_WIDTH + 2), thin(VALUE_WIDTH + 2)));
        }
    }
    // The label column's divider ends here and the hex column's begins.
    let mut middle: Vec<char> = thin(LABEL_WIDTH + VALUE_WIDTH + 5).chars().collect();
    middle[HEX_WIDTH + 2] = '┬';
    middle[LABEL_WIDTH + 2] = '┴';
    out.push_str(&format!("┠{}┨\n", middle.iter().collect::<String>()));

    let x = id.as_u128();
    let hex = format!("{x:032x}");
    for row in 0..4u32 {
        let h = &hex[row as usize * 8..row as usize * 8 + 8];
        let mut bits = String::new();
        for i in 0..32 {
            let bit = row * 32 + i;
            let digit = if x >> (127 - bit) & 1 == 1 { "1" } else { "0" };
            bits.push_str(&paint(digit, color_of(bit)));
            bits.push_str(match i {
                15 => "   ",
                7 | 23 => "  ",
                3 | 11 | 19 | 27 => " ",
                _ => "",
            });
        }
        out.push_str(&format!("┃ {} {} │ {bits} ┃\n", &h[..4], &h[4..]));
    }
    out.push_str(&format!("┗{}┷{}┛\n", line(HEX_WIDTH + 2), line(BITS_WIDTH + 2)));
    out
}

/// The decoded value of every field, with its code.
fn values(id: KepId, el: &Elements) -> [String; 10] {
    let f = Fields::unpack(id);
    [
        "0 (profile 0)".to_string(),
        "UUID version 7 (RFC 9562)".to_string(),
        el.epoch_iso8601(),
        format!("{:.4}° (code {})", el.arg_perigee, f.arg_perigee),
        format!("{:.4}° (code {})", el.inclination, f.inclination),
        format!("{:.4}° (code {})", el.raan, f.raan),
        format!("B* {:.4e} (code {})", el.bstar, f.drag),
        format!("{:.4}° (code {})", el.mean_arg_latitude(), f.lambda),
        format!("{:.8} rev/day (code {})", el.mean_motion, f.mean_motion),
        format!("e {:.7} (code {})", el.eccentricity, f.eccentricity),
    ]
}

/// The color of bit `bit`, counted from the most significant.
fn color_of(bit: u32) -> Option<&'static str> {
    let inside = |(offset, width): (u32, u32)| (offset..offset + width).contains(&bit);
    FIELDS.iter().find(|f| inside(f.bits)).map_or(Some(MARKER_COLOR), |f| f.color)
}

/// Pads to `width` characters (not bytes: the labels have Greek letters).
fn pad(text: &str, width: usize) -> String {
    let n = text.chars().count();
    format!("{text}{}", " ".repeat(width.saturating_sub(n)))
}
