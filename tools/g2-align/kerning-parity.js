// Kerning probe: Chrome applies GPOS pair kerning to canvas measureText,
// OpenCat's `surface.rs` sums hmtx advances only (its documented canvas 2D
// anchor). This page measures, for each pair, the pair width minus the two
// single advances — i.e. exactly the kerning Chrome added — so the numbers can
// be compared against a shap implementation and the deltas decided on data.
//
// Run: bun tools/g2-align/build-kerning-page.mjs && msedge --headless --dump-dom ...
const PAIRS = [
  "AV", "Av", "AW", "Ay", "AT", "Va", "WA", "Ta", "Ya",
  "LT", "LV", "LW", "LY", "Lo", "La", "Lu", "Le",
  "P!", "P.", "T.", "V.", "W.", "Y.", "r!", "r,", "v,",
  "oe", "oo", "ll", "ee", "nn", "oo",
  "0-", "1.", "2.", "3.", "5.", "0.", "0,", "0)",
  "g-y", "gla", "y-", "y.", "-0", "--",
  "Sports", "Strategy", "Action", "Genre", "Sold", "Jan", "Feb", "275", "12:34",
];

(async function main() {
  for (const weight of [300, 400, 500, 600, 700]) {
    await document.fonts.load(`${weight} 16px "AlignFace"`, "0AaWg§°±→Δ");
  }
  const ctx = document.getElementById("cv").getContext("2d");
  const out = [];
  for (const weight of [400]) {
    for (const size of [16, 32]) {
      for (const pair of PAIRS) {
        ctx.letterSpacing = "0px";
        ctx.font = `${weight} ${size}px "AlignFace"`;
        const whole = ctx.measureText(pair).width;
        const parts = [...pair]
          .map((c) => { ctx.font = `${weight} ${size}px "AlignFace"`; return ctx.measureText(c).width; })
          .reduce((a, b) => a + b, 0);
        out.push({ pair, weight, size, whole, parts, kern: whole - parts });
      }
    }
  }
  document.getElementById("result").textContent = JSON.stringify(out);
})();
