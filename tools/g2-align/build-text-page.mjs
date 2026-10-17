// Build the text-metric parity page (engine side of Phase 0.1).
//
// Writes the engine's own font files next to the page and references them from
// a real stylesheet (not base64). Chrome happily applies file:// @font-face
// when the font and the page share a directory, and this keeps the HTML small
// enough that `--dump-dom` returns the whole result div. Run:
//   bun tools/g2-align/build-text-page.mjs
// then open out/g2-align/text-parity.html with msedge --headless --dump-dom.
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "../..");
const DIR = resolve(ROOT, "out/g2-align");

const FACES = [
  { file: "assets/Inter-300.ttf", weight: 300 },
  { file: "assets/Inter-400.ttf", weight: 400 },
  { file: "assets/Inter-500.ttf", weight: 500 },
  { file: "assets/Inter-600.ttf", weight: 600 },
  { file: "assets/Inter-700.ttf", weight: 700 },
];

const css = FACES.map((f) => {
  writeFileSync(resolve(DIR, f.file.split("/").pop()), readFileSync(resolve(ROOT, f.file)));
  return `@font-face{font-family:"AlignFace";font-weight:${f.weight};font-style:normal;`
    + `src:url("${f.file.split("/").pop()}") format("truetype");}`;
}).join("\n");
writeFileSync(resolve(DIR, "faces.css"), css);

const script = readFileSync(resolve(import.meta.dir, "text-parity.js"), "utf8");
writeFileSync(
  resolve(DIR, "text-parity.html"),
  `<!doctype html><html><head><meta charset="utf-8">
<link rel="stylesheet" href="faces.css"></head>
<body style="margin:0"><canvas id="cv" width="800" height="400"></canvas>
<div id="result"></div><div id="diag"></div>
<script>${script}</script></body></html>`,
);

console.log("wrote", resolve(DIR, "text-parity.html"));
