// Text-metric parity harness: Chrome/Edge (truth) side.
//
// Measures the exact same InkMetrics tuple that OpenCat's
// `text/surface.rs::surface_measure_text` returns, by driving a canvas 2D
// context the way g-canvaskit does (font shorthand → measureText). Prints one
// JSON line per case so the Rust parity test can consume stdout directly.
//
// Usage: msedge --headless --dump-dom "file:///.../text-parity.html"
// The page writes the JSON into #result and the DOM dump contains it.
const FAMILIES = ["AlignFace", "Noto Sans SC"];
const WEIGHTS = [300, 400, 500, 700];
const SIZES = [8, 10, 12, 16, 24, 32, 48];
const LETTER_SPACINGS = [0, 0.5, -0.5, 2];
const SAMPLES = [
  "0", "1", "10", "100", "1000", "275", "-12.5", "Sports", "Strategy", "Action",
  "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
  "Jan 2024", "2024-01-01", "12:34", "¥ 1,234", "12.3%", "1.2k", "Genre", "Sold",
  "a", "Mg", "Ag", "W", "iii", "x", "©", "€", "°", "±", "→", "Δ",
];

// Chrome exposes these on TextMetrics: width + actualBoundingBox{Left,Right,
// Ascent,Descent} + fontBoundingBox{Ascent,Descent}. fontdb/swash with
// USE_TYPO_METRICS gives the same OS/2 typo figures for the ascent/descent pair.
function measure(ctx, family, weight, size, ls, text) {
  ctx.font = `${weight} ${size}px "${family}"`;
  // `ctx.font` carries no letter-spacing. Chrome's canvas 2D takes it from
  // `ctx.letterSpacing` (px, supports negative) — the same per-character
  // spacing OpenCat appends in `surface.rs`. Setting it via the shorthand
  // only (as an earlier revision did) measures nothing but the glyphs.
  if ("letterSpacing" in ctx) ctx.letterSpacing = `${ls}px`;
  const m = ctx.measureText(text);
  return {
    text, family, weight, size, ls,
    width: m.width,
    ink_left: m.actualBoundingBoxLeft,
    ink_right: m.actualBoundingBoxRight,
    ink_ascent: m.actualBoundingBoxAscent,
    ink_descent: m.actualBoundingBoxDescent,
    font_ascent: m.fontBoundingBoxAscent,
    font_descent: m.fontBoundingBoxDescent,
  };
}

(async function main() {
  // Wait for the embedded faces so Chrome shapes with the very same font files
  // OpenCat embeds (otherwise every row compares two different typefaces).
  // @font-face is lazy: `measureText('... "AlignFace"')` silently falls back to
  // the system font unless the face was requested first. Force every face we
  // measure with, then assert they really loaded — a fallback shows up as
  // integral ink values and fontBoundingBox heights that match no face.
  for (const weight of [300, 400, 500, 600, 700]) {
    await document.fonts.load(`${weight} 16px "AlignFace"`, "0AaWg§°±→Δ");
  }
  const diag = {
    interLoaded: document.fonts.check('400 16px "AlignFace"'),
    interWeights: [300, 400, 500, 600, 700].map((w) =>
      document.fonts.check(`${w} 16px "AlignFace"`)),
    status: [...document.fonts].map((f) => `${f.family}/${f.weight}:${f.status}`),
  };
  document.getElementById("diag").textContent = JSON.stringify(diag);
  const canvas = document.getElementById("cv");
  const ctx = canvas.getContext("2d");
  const out = [];
  for (const family of FAMILIES) {
    for (const weight of WEIGHTS) {
      for (const size of SIZES) {
        for (const ls of LETTER_SPACINGS) {
          for (const text of SAMPLES) {
            if (weight !== 400 && ls === 0) continue;      // trim the matrix
            if (size !== 16 && ls !== 0) continue;         // trim the matrix
            if (weight !== 400 && size !== 16) continue;   // trim the matrix
            out.push(measure(ctx, family, weight, size, ls, text));
          }
        }
      }
    }
  }
  document.getElementById("result").textContent = JSON.stringify(out);
  document.title = "READY";
})();
