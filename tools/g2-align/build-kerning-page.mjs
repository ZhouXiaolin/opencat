// Build the kerning probe page. Shares the face-loading approach of
// build-text-page.mjs: real @font-face files next to the page, and an
// explicit document.fonts.load because @font-face is lazy under measureText.
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "../..");
const DIR = resolve(ROOT, "out/g2-align");
const FAMILY = "AlignFace";

const FACES = [300, 400, 500, 600, 700]
  .map((weight) => ({ weight, file: `assets/Inter-${weight}.ttf` }));

const css = FACES.map((f) => {
  writeFileSync(resolve(DIR, f.file.split("/").pop()), readFileSync(resolve(ROOT, f.file)));
  return `@font-face{font-family:"${FAMILY}";font-weight:${f.weight};font-style:normal;`
    + `src:url("${f.file.split("/").pop()}") format("truetype");}`;
}).join("\n");
writeFileSync(resolve(DIR, "faces.css"), css);

const script = readFileSync(resolve(import.meta.dir, "kerning-parity.js"), "utf8");
writeFileSync(
  resolve(DIR, "kerning-parity.html"),
  `<!doctype html><html><head><meta charset="utf-8">
<link rel="stylesheet" href="faces.css"></head>
<body style="margin:0"><canvas id="cv" width="800" height="400"></canvas>
<div id="result"></div>
<script>${script}</script></body></html>`,
);
console.log("wrote", resolve(DIR, "kerning-parity.html"));
