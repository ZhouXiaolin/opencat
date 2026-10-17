// Build a JSX composition that renders one transpiled G2 example through the
// engine's useChart path (Phase 0.2 engine-side single-frame export).
//
// The example is the byte-identical transpiled source both sides run: the same
// file g2-probe.mjs evals on the truth side. Here it is inlined into a
// `useChart(ref, build)` body, where `Chart` is the engine's managed class and
// the container is the shim element — the same substitution g2-probe makes via
// `createManagedChartClass`. Nothing about the example itself changes, so a
// diff is attributable to the renderer, not to a different spec.
//
// The static-normalization prelude (walkSpec: animate:false, interaction:{},
// tooltip/slider/scrollbar off, timingKeyframe→view) and the seeded
// Math.random are the same ones g2-rs applies, so both sides execute the same
// deterministic static chart.
//
// Usage: bun tools/g2-align/build-engine-jsx.mjs <transpiled.js> <out.jsx> [WxH]

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const [examplePath, outPath, dims = "960x540"] = process.argv.slice(2);
if (!examplePath || !outPath) {
  console.error("usage: build-engine-jsx.mjs <transpiled.js> <out.jsx> [WxH]");
  process.exit(1);
}
const [W, H] = dims.split("x").map(Number);

const example = readFileSync(resolve(examplePath), "utf8");

// Static-normalization: disable animation/interaction on the spec the example
// sets, mirroring g2-rs's walkSpec so both sides render one deterministic
// final frame. Applied by wrapping Chart.render to rewrite options() once.
const STATIC_PRELUDE = `
function walkSpec(value, visit, seen) {
  seen = seen || new WeakSet();
  if (!value || typeof value !== 'object') return;
  if (seen.has(value)) return;
  seen.add(value);
  if (Array.isArray(value)) { value.forEach(function (v) { walkSpec(v, visit, seen); }); return; }
  visit(value);
  Object.keys(value).forEach(function (k) { walkSpec(value[k], visit, seen); });
}
function staticizeChart(chart) {
  if (chart.__staticized) return;
  chart.__staticized = true;
  var spec = chart.options();
  walkSpec(spec, function (node) {
    if (!node || typeof node !== 'object' || Array.isArray(node)) return;
    var looks = 'children' in node || 'marks' in node || 'encode' in node
      || ('type' in node && ('data' in node || 'style' in node));
    if (!looks) return;
    node.animate = false;
    if (node.type === 'timingKeyframe') {
      node.type = 'view';
      if (Array.isArray(node.children) && node.children.length > 1) node.children = [node.children[0]];
    }
    if (node.interaction && Object.keys(node.interaction).length > 0) node.interaction = {};
    if (node.tooltip) node.tooltip = false;
    if (node.slider) node.slider = false;
    if (node.scrollbar) node.scrollbar = false;
  });
  chart.options(spec);
}
`;

// Seed=42 mulberry32 so any Math.random the example uses is identical to the
// truth side's seeded prelude.
const SEED_PRELUDE = `
(function () {
  var seed = 42;
  Math.random = function () {
    seed |= 0; seed = (seed + 0x6D2B79F5) | 0;
    var t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
})();
`;

const jsx = `import { useChart, useRef } from 'opencat/runtime';

export const meta = { width: ${W}, height: ${H}, fps: 30, duration: 1 };
${SEED_PRELUDE}
globalThis.__ocG2Staticize = staticizeChart;
globalThis.__ocG2Log = function (level, msg) { try { console.log('[G2]', level, msg); } catch (e) {} };
${STATIC_PRELUDE}
export default function Scene() {
  const chart = useRef(null);
  useChart(chart, function (Chart, node, t, ctx) {
    ${example.split("\n").join("\n    ")}
  });
  // A <canvas> node takes its layout box from CSS, not from the HTML width /
  // height attributes; without an explicit size the node lays out at 0 and the
  // DrawOp stream is clipped away entirely.
  // A white root: G2 views render on a transparent canvas, so without a
  // backdrop the compositor flattens the chart onto black. The truth side
  // renders the same view onto a white page, so both sides get white.
  return (
    <div class="absolute inset-0 w-[${W}px] h-[${H}px] bg-white">
      <canvas ref={chart} id="chart" class="absolute inset-0 w-[${W}px] h-[${H}px]" width={${W}} height={${H}} />
    </div>
  );
}
`;

writeFileSync(resolve(outPath), jsx);
console.log("wrote", resolve(outPath));
