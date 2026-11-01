// KepID online: the CLI's analysis in a page, without propagation.
// kepid.min.js is the JavaScript library's build (npm run build copies it here).

import { decode, kepIdToOmm, kepIdToTle, ommToKepId, tleToKepId } from "./kepid.min.js";

const ISS =
  "ISS (ZARYA)\n" +
  "1 25544U 98067A   26305.20860139  .00006903  00000+0  13267-3 0  9994\n" +
  "2 25544  51.6332 342.0408 0007056 334.5041  25.5599 15.49138562592994";

const OLD_EPOCH_MS = 7 * 86400000;
const UNKNOWN = "not a KepID, a TLE or an OMM";

// Offset and width of every field (docs/spec.md, section 3), with its class.
const BITS = {
  epoch: [0, 40, "f-epoch"],
  perigee: [40, 8, "f-perigee"],
  version: [48, 4, "f-marker"],
  inclination: [52, 12, "f-inclination"],
  variant: [64, 2, "f-marker"],
  raan: [66, 14, "f-raan"],
  drag: [80, 10, "f-drag"],
  profile: [90, 1, ""],
  lambda: [91, 13, "f-lambda"],
  motion: [104, 17, "f-motion"],
  eccentricity: [121, 7, "f-eccentricity"],
};

const input = document.getElementById("input");
const output = document.getElementById("output");

/** Creates an element with a class and children; text is never parsed as HTML. */
function el(tag, className, ...children) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  node.append(...children);
  return node;
}

// ---- reading the input (the same detection as the CLI) ------------------------

function detect(raw) {
  const text = raw.replaceAll("\\n", "\n").trim();
  if (text.startsWith("{") || text.startsWith("[")) return detectOmm(text);
  const lines = text.split(/\r?\n/).map((line) => line.trimEnd()).filter(Boolean);
  const i = lines.findIndex((line, k) => line.startsWith("1 ") && lines[k + 1]?.startsWith("2 "));
  if (i >= 0) {
    const [line1, line2] = [lines[i], lines[i + 1]];
    return {
      format: "TLE",
      kepid: tleToKepId(line1, line2),
      catalog: readCatalog(line1.slice(2, 7)),
      given: [...(i > 0 ? [lines[i - 1].trim()] : []), line1, line2].join("\n"),
    };
  }
  const hex = text.replaceAll("-", "").toLowerCase();
  if (!/^[0-9a-f]{32}$/.test(hex)) throw new Error(UNKNOWN);
  const kepid = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  decode(kepid);
  return { format: "KepID", kepid };
}

function detectOmm(text) {
  let json;
  try {
    json = JSON.parse(text);
  } catch (err) {
    throw new Error(`not valid JSON: ${err.message}`);
  }
  if (Array.isArray(json) && json.length !== 1) throw new Error(`expected one OMM, found ${json.length}`);
  const omm = Array.isArray(json) ? json[0] : json;
  const catalog = Number(omm.NORAD_CAT_ID);
  return {
    format: "OMM",
    kepid: ommToKepId(omm),
    catalog: Number.isInteger(catalog) && omm.NORAD_CAT_ID != null ? catalog : undefined,
    given: JSON.stringify(json, null, 4),
  };
}

/** A TLE catalog number: five digits, or Alpha-5 (a letter for 10-33, then four digits). */
function readCatalog(text) {
  const head = "ABCDEFGHJKLMNPQRSTUVWXYZ".indexOf(text.trim()[0]);
  return head >= 0 ? (10 + head) * 10000 + Number(text.trim().slice(1)) : Number(text);
}

// ---- writing the formats -------------------------------------------------------

/** The OMM keywords SGP4 needs, as in the CLI. */
function compactOmm(kepid, catalog) {
  const omm = kepIdToOmm(kepid, catalog);
  const keys = ["EPOCH", "MEAN_MOTION", "ECCENTRICITY", "INCLINATION", "RA_OF_ASC_NODE", "ARG_OF_PERICENTER", "MEAN_ANOMALY", "NORAD_CAT_ID", "BSTAR"];
  const picked = Object.fromEntries(keys.filter((key) => key in omm).map((key) => [key, omm[key]]));
  return JSON.stringify(picked, null, 4);
}

/** An age in the largest unit that fits at least once, rounded down. */
function sparseAge(ms) {
  const days = ms / 86400000;
  const units = [[365.2425, "year"], [365.2425 / 12, "month"], [7, "week"], [1, "day"]];
  const [length, unit] = units.find(([size]) => days >= size) ?? units[3];
  const count = Math.floor(days / length);
  return `${count} ${unit}${count === 1 ? "" : "s"}`;
}

// ---- the bit layout ------------------------------------------------------------

function code(value, [offset, width]) {
  return (value >> BigInt(128 - offset - width)) & ((1n << BigInt(width)) - 1n);
}

function classOfBit(bit) {
  for (const [offset, width, className] of Object.values(BITS)) {
    if (bit >= offset && bit < offset + width) return className;
  }
  return "";
}

/** Four decimals, with exact ties to even as in the CLI (toFixed rounds them up). */
function degrees(x) {
  const scaled = x * 1e4;
  const tie = scaled % 1 === 0.5;
  const value = tie ? (Math.floor(scaled) % 2 === 0 ? Math.floor(scaled) : Math.ceil(scaled)) / 1e4 : x;
  return `${value.toFixed(4)}°`;
}

function row(label, className, value) {
  return el("tr", "", el("th", className, label), el("td", "", value));
}

function bitLayout(kepid) {
  const hex = kepid.replaceAll("-", "");
  const value = BigInt(`0x${hex}`);
  const e = decode(kepid);
  const lambda = (e.argPerigee + e.meanAnomaly) % 360;
  const codeOf = (name) => code(value, BITS[name]);

  const identity = el(
    "tbody", "",
    row("KepID", "", "0 (profile 0)"),
    row("UUID format", "f-marker", "UUID version 7 (RFC 9562)"),
  );
  const fields = el(
    "tbody", "",
    row("Epoch", "f-epoch", new Date(Number(e.epochUnixUs / 1000n)).toISOString()),
    row("Perigee (ω)", "f-perigee", `${degrees(e.argPerigee)} (code ${codeOf("perigee")})`),
    row("Inclination (i)", "f-inclination", `${degrees(e.inclination)} (code ${codeOf("inclination")})`),
    row("Ascension (Ω)", "f-raan", `${degrees(e.raan)} (code ${codeOf("raan")})`),
    row("Drag (D)", "f-drag", `B* ${e.bstar.toExponential(4)} (code ${codeOf("drag")})`),
    row("Mean latitude (λ)", "f-lambda", `${degrees(lambda)} (code ${codeOf("lambda")})`),
    row("Mean motion (w)", "f-motion", `${e.meanMotion.toFixed(8)} rev/day (code ${codeOf("motion")})`),
    row("Eccentricity (s)", "f-eccentricity", `e ${e.eccentricity.toFixed(7)} (code ${codeOf("eccentricity")})`),
  );

  const bits = el("tbody");
  for (let r = 0; r < 4; r++) {
    const cell = el("td", "bits");
    for (let i = 0; i < 32; i++) {
      const bit = r * 32 + i;
      const digit = (value >> BigInt(127 - bit)) & 1n;
      cell.append(el("span", classOfBit(bit), String(digit)));
      if (i === 15) cell.append("   ");
      else if (i === 7 || i === 23) cell.append("  ");
      else if (i % 4 === 3 && i !== 31) cell.append(" ");
    }
    const part = hex.slice(r * 8, r * 8 + 8);
    bits.append(el("tr", "", el("th", "", `${part.slice(0, 4)} ${part.slice(4)}`), cell));
  }

  // Two tables, as the label and hex columns have different widths.
  return el("div", "card", el("table", "", identity, fields), el("table", "bit-map", bits));
}

// ---- the page ------------------------------------------------------------------

function section(title, content, copyText) {
  const heading = el("div", "section-title", el("h2", "", title));
  if (copyText !== undefined) {
    const button = el("button", "copy", "Copy");
    button.type = "button";
    button.addEventListener("click", async () => {
      await navigator.clipboard.writeText(copyText);
      button.textContent = "Copied";
      setTimeout(() => { button.textContent = "Copy"; }, 1500);
    });
    heading.append(button);
  }
  return el("section", "", heading, content);
}

function render() {
  output.replaceChildren();
  if (!input.value.trim()) return;

  let found;
  try {
    found = detect(input.value);
  } catch (err) {
    const message = err instanceof TypeError ? UNKNOWN : err.message;
    output.append(section("Checks", el("ul", "checks", el("li", "", message))));
    return;
  }

  const { format, kepid, catalog, given } = found;
  const tle = format === "TLE" ? given : kepIdToTle(kepid, catalog).join("\n");
  const omm = format === "OMM" ? given : compactOmm(kepid, catalog);

  output.append(section("Input format", el("div", "", `${format} (other formats inferred)`)));
  output.append(section("KepID", el("pre", "", kepid), kepid));

  const ageMs = Date.now() - Number(decode(kepid).epochUnixUs / 1000n);
  if (ageMs > OLD_EPOCH_MS) {
    output.append(el("div", "warning", el("strong", "", "Warning:"), ` epoch is ${sparseAge(ageMs)} old!`));
  }

  output.append(section("TLE", el("pre", "", tle), tle));
  output.append(section("OMM", el("pre", "", omm), omm));
  output.append(section("Bit layout", bitLayout(kepid)));
}

input.addEventListener("input", render);
document.getElementById("example").addEventListener("click", () => {
  input.value = ISS;
  input.scrollLeft = 0;
  render();
});
render();
