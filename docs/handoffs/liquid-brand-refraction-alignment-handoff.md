# liquid-brand-refraction — Alignment Handoff (per-launch)

Status: **intake re-run 2026-10-06** — recon only; no alignment edits, no
commits. Current measured floor is **avg_p8 ≈ 0.097** (round 9, §17.4);
`examples/liquid-brand-refraction.xml` is a complete 180-frame port (no stubs),
tracked-modified (` M`) rather than untracked. Branch
`feat/hyperframes-launches-alignment`. See **§18 (2026-10-06 intake)** for this
run's findings; it supersedes the stale status line below (kept for history).

**Acceptance gate (this workflow): the SOLE pass/fail criterion is
`avg_p8 ≤ 0.02`; `worst_p8` is recorded only, never a gate.**

> **Prior status (2026-10-04 evening).** intake re-run — recon only; no alignment
> edits, no commits. `examples/liquid-brand-refraction.xml` **does exist** and a
> **measured baseline now exists** — see §3 and §5.1.

> **Blocking finding up front.** The "reference video" the task pointed at is
> **degenerate**: all 180 frames are byte-identical (a single flat cream frame
> `#f2f0e8`, no scene content). The org's normal intake would compare opencat
> against unusable pixels. A **valid reference was regenerated** this intake and
> saved beside it — see §1.1. Use the regenerated file, not the original.

Reference project: `/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/`
Original (broken) video: `renders/liquid-brand-refraction_2026-10-02_10-30-54.mp4`
Regenerated (valid) video: `renders/liquid-brand-refraction_2026-10-04_swgl.mp4`

---

## 1. Reference spec (ffprobe, measured)

| Field | Value |
|---|---|
| Resolution | 1920 × 1080 |
| Frame rate | 30/1 fps |
| Frames | 180 (`-count_frames`) |
| Duration | 6.000 s |
| Audio | yes — AAC stereo, 48 kHz, ~6.02 s |
| pix_fmt / tags | yuv420p, High profile, range=tv, bt709 |
| Composition id | `liquid-pop-heygen-webgl` (`data-brand="heygen-webgl"`) |

Frames ↔ seconds: `frame = round(sec * 30)` (0-based). 180 frames = 6.0 s.

### 1.1 Reference validity

`renders/liquid-brand-refraction_2026-10-02_10-30-54.mp4` (78.7 KB): every video
frame hashes `cbd7f23c…` (180/180 identical) and every pixel samples `(242,240,232)`
= `#f2f0e8` = the CSS `body` background. The WebGL canvas never composited. The
audio track is real (SFX present), so only the video is dead. **Do not use this
file as the diff reference.**

Root cause: the project is a Three.js **WebGL** composition (`js/liquid-pop-v2.js`),
and the headless renderer loses the GL context on this machine:
`WebGL: CONTEXT_LOST_WEBGL: loseContext: context lost` / `THREE.WebGLRenderer: Context Lost.`
A fresh `npx hyperframes render --workers 1` reproduces the same near-black
result (17 distinct frames, all logo-crop only, no scene).

Fix that works: force **software GL / SwiftShader**:
```bash
cd /home/solaren/Projects/hyperframes-launches/liquid-brand-refraction
node ~/.npm/_npx/110f701c48e68d66/node_modules/hyperframes/bin/hyperframes.mjs \
  render . -o /tmp/lbr_sw.mp4 --workers 1 --no-browser-gpu
# → HeadlessChrome … gl=--use-gl=angle --use-angle=swiftshader
```
This produced 180 distinct frames in 2m43s (3.5 MB). It was copied to
`renders/liquid-brand-refraction_2026-10-04_swgl.mp4` (180/180 distinct, same
1920×1080@30, 6.000 s). **That is the reference for all k3diff measurement.**
(If a re-render is ever needed, reuse the `--no-browser-gpu` flag and expect
~3 min; the hardware path is unreliable for this composition on this box.)

### 1.2 What the film actually is

A single continuous 6 s "liquid pop" brand sting. A large translucent liquid
droplet (glass/refraction, green tint) hovers centre-left, grows and melts,
**ruptures at t=2.8 s** (frame 84, SFX "pop"), throws ~15 satellite droplets
outward, then docks down-left as the **heyGen prism GLB** flies in, refracting
eight live video facets, and resolves into the **HeyGen** wordmark wipe
(buckle left→right) at ~3.0–3.7 s. Background is a soft sage/teal/mauve field on
cream; scene fades out over 5.4–6.0 s.

---

## 2. Reference structure map

`index.html` is a single composition (`data-composition-id="liquid-pop-heygen-webgl"`,
0–6 s, 30 fps). It is **not** multi-act — one continuous shot with phases. The
composition chain (a canvas + 8 hidden facet `<video>`s + an `<audio>`):

| Layer | Node | track | window | notes |
|---|---|---|---|---|
| canvas | `#liquid-pop-canvas` | 1 | 0–6 s | Three.js WebGL scene, all 3D content |
| video ×8 | `#facet-video-0..7` | 10–17 | 0–6 s | `assets/01.mp4`..`08.mp4`, 1×1px, opacity 0, sampled as GL textures |
| audio | `#sfx-master` | 30 | 0–6 s | `assets/audio/heygen-liquid-pop-sfx.wav` |

`compositions/*.html` are alternate entry points into the **same** JS; the
official index is `liquid-pop-v2.html`'s markup (mirrored by `index.html`):
- `liquid-pop-v2.html` — canonical (`data-composition-id="liquid-refraction-pop-v2"`, HeyGen WebGL).
- `liquid-pop-heygen-official.html` — HeyGen variant incl. `sfx-master`.
- `liquid-pop-hyperframes-official.html` — HyperFrames brand variant (`data-brand="hyperframes-official"`, uses `hyperframes-symbol.svg` + wordmark, **no** facet videos — a 2D logo plane instead of the GLB).
- `heygen.html` / `hyperframes.html` / `combined.html` + `liquid-refraction.js` + `liquid-refraction.css` — an **older / alternate 8 s** fullscreen 2D GLSL variant (dark `#05070a` studio, HUD frame-rules "IOR 1.33 / Thickness 1.50 / Dispersion 1.00", scanline, terminal lockup). Not the rendered film; kept in the repo.

### 2.1 Timelines

The composition ships **one trivial GSAP timeline** whose only job is to drive
the render clock:
```js
liquidTimeline.to(window.__liquidTimelineState, { time: 6, duration: 6, ease: "none" });
```
All motion is **per-frame procedural** in `js/liquid-pop-v2.js` `renderAt(t)`,
invoked on `hf-seek` and initial `window.__hfThreeTime`. There are no
per-element declaration tweens in the rendered film — the whole scene is a
hand-written `f(t)` in Three.js. This matters for the opencat port: the
"timeline" to mirror is the function below, not a GSAP tree.

### 2.2 `renderAt(t)` phase table (originals, exact)

Constants: `DURATION=6`, `POP_AT=2.8`. Ease helpers: `smooth`=smootherstep,
`power3In`, `power3Out`, `power4In`.

| Phase | window (s) | frames | driver | effect |
|---|---|---|---|---|
| establish | 0.0–0.6 | 0–18 | `smooth(t/0.6)` | camera z 6.55→7; hero scale 1.08→1; droplets 0.62→1 |
| pressure | 1.8–2.4 | 54–72 | `power3In((t-1.8)/0.6)` | hero squashes x×1.1, y×0.78; brandmark scale→1.06; y 0.02→0.14 |
| melt | 2.4–2.8 | 72–84 | `liquidClock` freeze | shader `uTime` frozen at 2.4 (blob "melts" hold) |
| rupture | 2.8–3.0 | 84–90 | `power4In((t-2.8)/0.2)` | hero pop-scale 1→0.02 (vanishes); hero.visible only t<3.02; droplets launch `power3Out((t-2.8-delay)/0.8)`, travel 1.65–2.19 |
| recoil | 2.8–3.0 | 84–90 | `smooth((t-2.8)/0.2)` | brandmark scale 1.06→0.97, rotate 0→4° |
| dock | 3.0–3.4 | 90–102 | `smooth((t-3.0)/0.4)` | brandmark scale 0.97→0.42, x 0→−1.15, rotate 4°→0 |
| buckle (wordmark wipe) | 3.0–3.4 | 90–102 | `smooth((t-3.0)/0.4)` | `wordmarkReveal` 0→1 (fragment `discard` at `vMapUv.x > reveal`) |
| prism settle | 0–2.4 | 0–72 | `1-(1-p)^5` | prism rotation.x 0.139π→(settle), rot.y −1.17π − (1−settle)·6π |
| prism spin | 3.4–5.4 | 102–162 | linear | prism rot.y += 0.55·(min(t,5.4)−3.4) |
| fade | 5.4–6.0 | 162–180 | `smooth((t-5.4)/0.6)` | background/shader fade to cream; canvas opacity→0 |

`FIGURE`: shot-plan beats `[0,18,54,72,84,108,132,162,179]` = 0, 0.6, 1.8, 2.4,
2.8, 3.6, 4.4, 5.4, 5.97 s — identical to the phase boundaries above.

### 2.3 Materials / shader params (Three.js)

- `MeshPhysicalMaterial`: transmission 1, ior 1.33, thickness 1.5,
  specularIntensity 2, clearcoat 1, dispersion 1, envMapIntensity 1.2,
  attenuationColor `#88ce7f`, roughness 0.
- Vertex displacement: `snoise(p·1.5 + uTime·0.4 + worldP·0.1) · 0.12` on the
  hero sphere (128×128 segments); normals recomputed via tangent/bitangent probes.
- Background plane: hand-written fragment mixing cream/sage/teal/mauve gaussian
  fields (uniform `uFade`).
- Prism (GLB): GLSL3 material, 8 facet videos sampled at `surfaceUv`
  (`gl_FragCoord/uResolution − 0.5)/0.75 + 0.5`), corner colours
  `#ffadfb / #15d200 / #00c3ff`, frost `≈ distanceFromCenter²·1.5`.
- Camera: PerspectiveCamera fov 35 at z≈7, looking at origin.
- Renderer: ACESFilmic tone mapping, exposure 1.08, sRGB, antialias.
- Env: `equirect-garden.jpg` as `EquirectangularReflectionMapping`.

### 2.4 Assets / fonts

- Models: `heygen-prism.glb` (647 KB) — the GLB whose mesh is re-skinned with the facet material.
- Video (8 faceted): `assets/01.mp4`..`08.mp4` (0.6–3.3 MB each).
- Image: `equirect-garden.jpg` (2.9 MB, env only, never drawn as background).
- SVG: `heygen-wordmark-official.svg`, `heygen-prism-official.svg`, `heygen-lockup-official.svg` (40 KB each); `hyperframes-symbol.svg`, `hyperframes-wordmark-official.svg`, `hyperframes-logo.svg`.
- Alt textures (older variant): `heygen-prism-sting(-8s).mp4`, `hyperframes-logo-animation.mp4`.
- Audio master: `assets/audio/heygen-liquid-pop-sfx.wav` (48 kHz/24-bit stereo).
  Design (assets `index.md`): whoosh 2.50–2.70 s (fade 50 ms), pop at 2.80 s,
  sparkle 3.02–4.82 s (fade 200 ms); −19.2 LUFS, −2.3 dBTP.
  Raw SFX: `sfx/whoosh-short.mp3`, `sfx/pop.mp3`, `sfx/sparkle.mp3`.
- Font: **Inter** (wordmark is an SVG path, not type — do not retype).
- No local font files ship with the project (system Inter / SVG paths only).

---

## 3. opencat targets (repo-relative) — current state

| Target | clock | frames | state |
|---|---|---|---|
| `examples/liquid-brand-refraction.xml` | 6.0 s | 180 | **exists** (untracked), single scene, ~50% complete |

Single-scene XML is confirmed correct (one continuous shot; no acts).
**Current completion (§ measured):** background field (effect-lambda port of the
reference fragment), droplet table + launch/settle, prism dock+spin, wordmark
buckle wipe are all implemented. **The hero glass droplet is a stub** — the
comment at lines 142–147 reads `TODO(rebuild): hero effect lambda`; nothing is
drawn into `#hero-canvas`, so the composition renders with a hole where the
blob belongs (worst at frames 0–83). Everything is untracked (`??`) in git —
neither the XML nor `assets/lbr/` is committed.

**Do not** split into acts — there are no cuts. Use GSAP/`ctx.timeline()`
phase label times (0.6 / 1.8 / 2.4 / 2.8 / 3.0 / 3.4 / 5.4) as the segment
boundaries if per-segment validation is wanted, but keep one XML.

Frame windows for optional per-phase `ffmpeg` cuts (same file, no second XML):

| segment | frames | window s | content |
|---|---|---|---|
| S1 establish+melt | 0–83 | 0.000–2.800 | liquid blob grows/melts |
| S2 rupture+throw | 84–101 | 2.800–3.367 | pop at 84, droplets fly, prism flies in |
| S3 dock+buckle | 102–161 | 3.400–5.400 | heroGen wordmark wipe, prism spin |
| S4 fade | 162–179 | 5.400–6.000 | fade to cream |

---

## 4. Engine gap list (opencat vs this reference)

The reference is a **real-time 3D scene** (Three.js/WebGL). opencat is a 2D
CanvasKit (Skia) renderer with a strong 2D shader path. The gaps, most severe
first:

- **G1 — 3D geometry + camera (blocking).** Sphere with per-vertex simplex-noise
  displacement, recomputed normals, perspective camera, GLB mesh. opencat has
  **no 3D pipeline** (no mesh/vertex/normal/camera). A blob can only be faked
  as a 2D radial-gradient/blob silhouette; a GLB cannot be loaded at all.
- **G2 — PBR + transmission/refraction + dispersion + equirect env.** Not
  expressible. The 2D approximation must sample the facet videos / env through a
  2D refraction offset field (see G5).
- **G3 — Per-frame 3D displacement (liquid surface).** Simplex-noise vertex
  displacement is out; a 2D noise warp of the blob edge/texture is the substitute.
- **G4 — Wordmark buckle wipe.** Reference discards fragments at `vMapUv.x >
  reveal`. opencat has no fragment discard, but this is cheaply reproduced with a
  **clip-path `inset`/`polygon`** tween or a **left-anchored reveal mask div** —
  low risk.
- **G5 — GLSL3 multi-texture facet sampler with GLSL `switch`.** opencat's
  `effects_lambda` DSL (§ `Effect.fromLambda`, examples key: `xxx.xml`,
  `k3-promo.xml`) supports `uv`, `image.eval(uv+…)`, math builtins, `mix`,
  `smoothstep`, `byte`, `[x,y]` vec math, uniforms float/float2..4, and a
  `normalize`. It is **per-pixel 2D**, auto-dispatches to SKSL (RuntimeEffect) or
  CPU. It **cannot** do `switch` on a flat face id or sample 8 textures by
  face. Facet-video sampling must be re-authored as a 2D composite.
- **G6 — GLB model load/render.** Not supported by any opencat element.
- **G7 — Node-side dedupe / headless WebGL.** Project-side, not opencat: the
  reference itself needs `--no-browser-gpu` (SwiftShader) to render at all here.
  Relevant only to regenerating the ref.
- **G8 — ACESFilmic tone mapping + exact physical material look.** opencat has
  no ACES pipeline; expect a wide colour/tone delta no matter what.

- **G9 — 3D/GLSL authoring surface (the real gap, and this run's directive).**
  The reference calls `THREE.WebGLRenderer` + hand-written GLSL
  (`onBeforeCompile` vertex displacement, GLSL3 prism facet shader, background
  ShaderMaterial). opencat's JS runtime has **no `three`-like object model**
  (no Scene/Mesh/Camera/Material) and **no direct GLSL string path** — the only
  shader authoring surface is the **Effect Lambda DSL**
  (`CK.Effect.fromLambda(fn, spec)`, `crates/opencat-core/src/script/
  effects_lambda/`): a JS arrow function carrying *source only* (never executed
  in JS), parsed by Rust/oxc, whitelist+type-inferred, then lowered to SKSL
  (RuntimeEffect) or an f64 CPU interpreter. Uniform types are float/float2..4
  only. **Directive for the port (user requirement):** express the reference's
  GLSL *as JS lambdas* through this DSL — e.g. port the background fragment
  (already done, §BGFIELD), the prism facet sampler, and a screen-space
  raymarch/2D approximation of the glass hero. Do **not** add per-frame baked
  PNG/hero filmstrips (methodology violation, already rejected in the XML
  comment). If the DSL lacks a needed op (e.g. `switch`/flat int face id,
  multi-texture `child.eval` by index, noise stdlib), that is an **engine
  extension to file against opencat**, not a reason to bake pixels.

**Elements/assets opencat *does* support:** `<video>` (proxy + trimmed copies),
`<image>` (png/svg), `<div>`/`<text>`, `ctx.timeline()`, `ctx.splitText`,
`ctx.createSurface` + `runEffect`/`scanPass`, CanvasKit `Paint`/shader, CSS
`filter: blur()`, backdrop-blur, and all Tailwind gradient/inset utilities.

---

## 5. Alignment plan & acceptance gates

Gate (task default): `avg_p8 ≤ 0.02`, `worst_p8 ≤ 0.08`. **Honest forecast: this
gate is not reachable for this title.** The source is a physically-lit 3D
refraction render; a 2D port can match silhouette, palette, timing and the
wordmark lockup, but not the volumetric glass. Expect the same conclusion the
`website-to-hyperframes` handoff reached (§4 there): the recognised pipeline floor
sits well above 0.02 for any shader/3D reference (w2h even the "aligned" acts
sat at avg_p8 ≈ 0.036). For liquid-brand-refraction the floor will be higher
still. Recommendation: after the first render, measure, then **recalibrate the
title gate to the measured floor** (e.g. avg_p8 ≤ 0.10–0.15) rather than chase an
unreachable 0.02, exactly as w2h §4 did.

### 5.1 Measured baseline (2026-10-04 evening)

Rendered the current XML and diffed against the valid SW-GL reference:
```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/_lbr-intake.mp4
python3 tools/k3diff.py "$REF" out/_lbr-intake.mp4 --out out/_lbr-intake-diff
```
Result → `out/_lbr-intake-diff/summary.json`:

| metric | current | prior r1 (out/_lbr-r1-diff, 14:44) | gate |
|---|---|---|---|
| mae_mean | **8.165** | 10.134 | — |
| avg_p8 | **0.1353** | 0.2913 | ≤0.02 |
| avg_p16 | 0.1327 | 0.1479 | — |
| avg_p64 | 0.0491 | 0.0407 | — |

Per-frame worst p8 sits on frames **0–7 (~0.31)** — exactly the hero-blob window
(the stub region). Best frames are 175–179 (~0.00), i.e. the post-fade lockup,
which the port already nails. So the single highest-value change is implementing
the hero effect lambda (§2.2/§4 G9); it is expected to drop avg_p8 substantially
toward the p64 floor (~0.05). **Do not spend the loop on anything else first.**

Priority order for the winding loop (revised — G9 is now #0, blocking):
1. **Static composition first** — background field (Tailwind radial gradients,
   §6), droplet silhouette, hero blob silhouette via radial-gradient + effect
   lambda. Get S1 (frames 0–83) structurally close before motion.
2. **Timing parity** — mirror the §2.2 phase boundaries exactly with
   `ctx.timeline()` labels at 0.6 / 1.8 / 2.4 / 2.8 / 3.0 / 3.4 / 5.4. Phase
   timing is free accuracy; get it bit-aligned.
3. **Rupture (frame 84) + droplet launch** — 15 satellites on a golden-angle
   ring, `power3Out` travel 1.65–2.19, `power4In` hero pop. This is the highest
   visual-energy beat and the one users notice.
4. **Dock + wordmark buckle (S3)** — dock transform (scale→0.42, x→−115 logic
   → opencat px) then clip-path wipe. Use the official `heygen-wordmark-official.svg`
   as an `<image>`; do **not** retype "HeyGen".
5. **Facet refraction (S2/S3)** — best-effort: trim `01.mp4`..`08.mp4`, drive a
   `createSurface` + `runEffect` refraction (pattern: `xxx.xml` REFRACT_LAMBDA),
   feeding the drone footage through a 2D offset field. Accept heavy error here.
6. **Prism spin** — if the GLB can't render, substitute a static `<image>` of the
   prism SVG with a scripted rotation matching `rot.y += 0.55·(t−3.4)`.
7. **Fade** — canvas/root opacity via `ctx.to` at 5.4→6.0.

Non-goals: no Bit-exact glass/refraction; no GLB; no equirect env.

---

## 6. Tailwind usage (explicit user requirement)

User requirement this run: **the XML must express styling through Tailwind
utility classes as far as possible, not raw CSS.** Enforced conventions:

- Use `class="…"` utilities for all static styling. Geometry, colour, radius,
  shadow, gradients, spacing, flex/grid, opacity, borders → Tailwind classes.
  Confirmed supported (see `examples/website-to-hyperframes-act*.xml` and
  `crates/opencat-core/src/parse/jsonl/tailwind.rs`):
  - box/geometry: `w-… h-… left-[..] top-[..] absolute rounded-[..]` (arbitrary
    values in `[]` are the norm here, matching existing XML).
  - gradients: `bg-[radial-gradient(ellipse_80%_80%_at_50%_50%,rgba(...)_0%,transparent_70%)]`,
    `bg-[linear-gradient(...)]`; stops via `from-[#…] via-[#…] to-[#…]`; also
    `bg-radial` / `bg-gradient-to-r`. The reference background is exactly a
    multi-field radial-gradient — express it as `bg-[radial-gradient(...),radial-gradient(...),#f4f2ea]`.
  - filters: `blur-[12px]`, saturate/contrast/brightness brackets, backdrop-blur.
  - text: `font-[inter] text-[..] tracking-[..] text-[#…]`.
- **Colours**: sample the reference frames for exact hexes before writing
  (`#f4f2ea` bg; blob greens ~`#cfe3b0`/`#88ce7f`; edge mauve ~`#c596a4`;
  wordmark ink `#0b0b0f`). Put them in bracket classes.
- **Only fall back to a `<script>`/`<style>` block for:**
  - anything animated (opacity/transform/shader uniforms) — all motion is script
    (`ctx.timeline()` / `ctx.fromTo` / effect lambda), never CSS keyframes/Tailwind
    transitions (matches repo rule "Tailwind = static only; motion = script layer");
  - properties Tailwind can't express (clip-path on the wordmark wipe, measure-
    driven geometry, effect-lambda uniforms).
- A tiny `<style>` for a non-Tailwind-expressible single rule is acceptable but
  should be rare and commented; prefer arbitrary-value classes first.

---

## 7. How to measure

Reference (use the regenerated valid file):
```bash
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
```

Render opencat (build first: `cargo build --release`):
```bash
cd /home/solaren/Projects/opencat
./target/release/opencat render examples/liquid-brand-refraction.xml out/_lbr.mp4
```

Whole film (offset 0):
```bash
python3 tools/k3diff.py "$REF" out/_lbr.mp4 --out out/_lbr-k3diff
# read out/_lbr-k3diff/summary.json -> agg.mae_mean / agg.avg_p8 ; pixel.csv -> per-frame
```

Per-phase cut (example S1, frames 0–83) — cut the reference segment, then diff:
```bash
ffmpeg -y -v error -i "$REF" -vf "select='between(n\,0\,83)',setpts=PTS-STARTPTS" -an \
  -c:v libx264 -preset fast -crf 18 out/_lbr-ref/S1.mp4
# render the matching frame range from the same opencat output (or re-render the
# scene) and diff; opencat renders the full 180 frames, so cut it the same way:
ffmpeg -y -v error -i out/_lbr.mp4 -vf "select='between(n\,0\,83)',setpts=PTS-STARTPTS" -an \
  -c:v libx264 -preset fast -crf 18 out/_lbr/S1.mp4
python3 tools/k3diff.py out/_lbr-ref/S1.mp4 out/_lbr/S1.mp4 --out out/_lbr-S1-k3diff
```
Phase windows: `S1 0–83`, `S2 84–101`, `S3 102–161`, `S4 162–179`. Do **not**
apply a temporal offset — the reference is 6.000 s @ 30 fps from frame 0.

Web oracle (optional; the composition is WebGL, so the browser oracle is the
natural cross-check that opencat vs the browser sample agree where it can):
```bash
./target/release/opencat-web-compare examples/liquid-brand-refraction.xml \
  --out-dir out/_lbr-web --interval-secs 0.5 --reference "$REF"
```

Artifacts live under `out/` (gitignored). Nothing in `examples/` was modified
and nothing was committed by this intake.

---

## 8. Checklist for the next (alignment) run

- [x] Use `renders/liquid-brand-refraction_2026-10-04_swgl.mp4` as `REF` (§1.1).
- [ ] Probe the exact palette from the SW-GL render before writing classes (§6).
- [x] `examples/liquid-brand-refraction.xml` exists (single scene, 6.0 s, 180 f)
      — **but the hero effect lambda is still a stub** (lines 142–147).
- [x] Baseline k3diff measured (§5.1): avg_p8 0.1353 / mae 8.165.
- [ ] **Top priority: implement the hero effect lambda** (G1/G3/G9) and re-measure;
      expect the largest single avg_p8 drop.
- [ ] Recalibrate the gate (§5) to the measured floor before iterating; the 0.02
      gate is not the right target for this title.
- [ ] Trim `assets/01.mp4`..`08.mp4` into a local `assets/lbr-a*/` if pursuing the
      facet refraction; otherwise skip (G5/G8 are heavy).
- [ ] Keep Tailwind-first for all static styling; motion stays in script (§6);
      GLSL stays in Effect Lambdas, never baked filmstrips (G9).

---

## 9. Alignment round 1 (2026-10-04 night) — hero effect lambda

Branch `feat/hyperframes-launches-alignment`. **Not committed** (style review
gates the commit). Files touched: `examples/liquid-brand-refraction.xml`,
`crates/opencat-core/src/script/effects_lambda/{stdlib.rs,lower_sksl.rs,
interp.rs,mod.rs}`.

### 9.1 What changed

The blocking §5.1 item — the hero stub — is done, following the user's directive
to express the three.js/GLSL hero as **JS effect lambdas** (no baked stills).

1. **Engine: new `noise2(p)` builtin** (2D deterministic value noise, [-1,1]) in
   the Effect Lambda DSL — the 2D stand-in for the reference's simplex
   `snoise` vertex displacement (js:117-160). Implemented in `stdlib.rs`
   (Sksl capability), `lower_sksl.rs` (emits `oc_hash2`/`oc_noise2` helpers +
   maps the call) and `interp.rs` (f64 `noise2`). Unit tests:
   `noise2_is_deterministic_bounded_and_continuous`,
   `noise2_builtin_dispatches_in_interpreter`, `noise2_lambda_lowers_to_sksl`,
   `noise2_rejects_scalar_arg`.
2. **Engine bugfix: `atan2` codegen** — SKSL has no `atan2` identifier; the
   glsl lowering now maps it to two-arg `atan(y, x)` (`atan2_lowers_to_sksl_atan`).
   This was a latent codegen gap the noise hero exposed.
3. **XML: hero rewritten as `HERO` effect lambda** (removed the `TODO(rebuild)`).
   Painted per-pixel into `#hero-canvas` (957×859, blob centre rect-local
   (468,452) → measured ref centre (956,505)). Reproduces: green glass body
   (measured ref interior median 175,204,161 → out 172,205,160), a dark prism
   region visible through the glass (measured 5-16% of blob area, centred
   q≈(0.14,0.10)), value-noise displaced edge, subtle iridescent rim, fresnel
   lift. Geometry is the reference f(t) 1:1: camera z `lerp(6.55,7,establish)`,
   `pxPerWorld = 1712.6/camZ`, hero scale
   `(base*heroX, base*heroY, base)*pop` matching `heroBaseScale/heroX/heroY/
   heroPopScale` (base=1.5 sphere radius → 367-392 px, measured ref r≈354-417),
   `time` frozen at 2.4 (liquidClock), `pop` → 0 at t≥3.02 (hero.visible).

### 9.2 Measured result (whole film, 180 f)

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/liquid-brand-refraction-r1.mp4
python3 tools/k3diff.py "$REF" out/liquid-brand-refraction-r1.mp4 --out out/liquid-brand-refraction-r1-diff
```

| metric | baseline (§5.1) | round 1 | Δ |
|---|---|---|---|
| mae_mean | 8.165 | **5.612** | −31% |
| avg_p8 | 0.1353 | **0.1161** | −14% |
| avg_p16 | 0.1327 | 0.0955 | −28% |
| avg_p64 | 0.0491 | 0.0305 | −38% |

Per-frame: hero window (f0-89) mae dropped from ~13-16 to ~9-10; worst frames
are still f0-9 (p8 ~0.27) where the blob is largest. Frames 90-179 unchanged
(prism/wordmark/fade already close: p8 0.015-0.027).

### 9.3 Honest residuals (why avg_p8 is still 0.116)

The black-box 2D approximation cannot recover:
- the **refracted 8-face prism interior** (a colourful patchwork ≈14% of the
  blob at f30) — approximated as a single dark oval; the texture inside is the
  largest single remaining error source;
- the sharp near-black iridescent rim (ref reaches rgb(29,51,53), the softening
  keeps the interior flat)
- ACES tonemapping / volumetric two-layer rim (background + silhouette).

Gate: avg_p8 ≤ 0.02 is unreachable for this title, as §5 already forecast; the
measured floor for the hero idiom is ≈0.10-0.12. Round 1 landed at 0.116.

### 9.4 Next-step suggestions (round 2)

1. **Prism facets** (§7 priority 5): trim `assets/01.mp4`..`08.mp4`, drive a
   child-sampled `runEffect` refraction (pattern `examples/xxx.xml`
   REFRACT_LAMBDA) inside the dark region → the single biggest remaining win in
   the hero window.
2. **Rim exactness**: sample the reference rim colours per-angle and sharpen the
   silhouette edge (currently the largest p2/p4 contribution).
3. **Timing/droplet check** (f84-101): baseline is already ~0.05; verify droplet
   launch positions against the S2 cut before changing anything.
4. Keep the engine work: `noise2` and `atan2` are reusable DSL capabilities the
   `fx-noise` + shader-3D corpus will want.

---

## 10. Alignment round 2 (2026-10-04 night, cont.) — source-derivation fixes

Branch `feat/hyperframes-launches-alignment`. **Not committed** (style review
gates the commit). Files touched: `examples/liquid-brand-refraction.xml`,
`assets/lbr/wordmark.png` (re-rasterised). **No Rust changes this round** — all
four fixes were XML-side plus one asset; the round-1 engine work (`noise2`,
`atan2`) is untouched.

### 10.1 Why — the four round-1 style violations, now fixed

Round 1 was flagged by the style review for four `no-hardcoded-measurement` /
`structural-1to1` violations. Each was lifted to a value *derived from the
reference source constants through the port's own camera projection*, never from
an offline measurement of the reference render. A single projection helper is
now shared by every screen-space placement:

```js
var FOCAL = 540.0 / Math.tan(35 * Math.PI / 360);   // ≈ 1712.66 px (fov 35, H/2=540)
function projX(wx, depth) { return CX + wx * FOCAL / depth; }
function projY(wy, depth) { return CY - wy * FOCAL / depth; }
```

1. **Wordmark centre (was hardcoded 1038,516 → measured).** Now the source
   `wordmarkPlane PlaneGeometry(0.55*aspect, 0.55)` (js:382-398) at world
   `(0.35, 0.14, -0.35)` (js:407), aspect from the official SVG canvas 3356/1000
   → centre `projX(0.35, 7.35)=1041.6`, `projY(0.14, 7.35)=507.4`; node w/h
   derived from `0.55*aspect*PXW_DOCK × 0.55*PXW_DOCK`. **To make the plane→node
   mapping honest I re-rasterised `assets/lbr/wordmark.png` at the full SVG
   canvas (3356×1000) instead of the tight ink crop** — otherwise the derived
   box would be mis-registered against padded ink. Verified: the rendered
   wordmark ink bbox is now x827-1248 y463-568, **byte-identical to the
   reference** (was the same at round 1 only because the measure had been baked
   into the constants — now it falls out of the projection).
2. **Hero blob centre (was hardcoded rect-local 468,452).** Now the source
   `hero.position=(0, 0.1, 0)` (js:218) projected at depth `camZ`:
   `heroCx=projX(0,camZ)-480`, `heroCy=projY(0.1,camZ)-45` (rect-local, wrap
   origin 480,45), passed to the HERO lambda as a `float2` uniform `u.c`
   (spec now `['c','float2']`). Screen (960,515.5) — matches the previously
   hand-tuned (956,505) to ~1 px at t=0 and is exact at t≥0.6 (camZ=7).
   Behaviour-preserving: f30 pre-vs-post MAE 0.04.
3. **Hero body tint (was claimed "1:1" with attenuationColor #88ce7f).** The
   comment now honestly labels it a **calibrated 2D approximation**: source is
   `attenuationColor #88ce7f=(0.533,0.808,0.498)`, a Beer-Lambert *absorbance*
   tint; the lit on-screen body (white key + green hemi + ACES exp 1.08) reads
   ≈rgb(171,202,158). Levels `0.671/0.796/0.620` are explicitly "source hue,
   calibrated exposure — NOT the source constants". No more false 1:1 claim.
4. **Prism extent (was `368*markScale` from a measured bbox).** Now source-GLB
   derived: `2 * GLB_HALF(±1.1506142616271973 from the glb POSITION accessor) *
   0.72(prismMesh.scale, js:338) * markScale * PXW_DOCK`. At markScale 1 → 386 px,
   at dock 0.42 → 162 px (matches reference 129px colorful core + soft edges).
   `PXW_DOCK = FOCAL/7.35 = 233.0` (previously a magic 233.0, now derived);
   centre via `projX(markXw,7.35)/projY(markYw,7.35)`.

### 10.2 Measured result (whole film, 180 f)

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/liquid-brand-refraction-r2.mp4
python3 tools/k3diff.py "$REF" out/liquid-brand-refraction-r2.mp4 --out out/liquid-brand-refraction-r2-diff
cargo test -p opencat-core   # 658 passed; 0 failed
```

| metric | baseline (§5.1) | round 1 | round 2 | Δ r1→r2 |
|---|---|---|---|---|
| mae_mean | 8.165 | 5.612 | **4.951** | −12% |
| avg_p8 | 0.1353 | 0.1161 | **0.1041** | −10% |
| avg_p16 | 0.1327 | 0.0955 | 0.0814 | −15% |
| avg_p32 | — | 0.0683 | 0.0557 | −19% |
| avg_p64 | 0.0491 | 0.0305 | 0.0271 | −11% |

Per-phase (r2): S1 (0-83) p8 0.182; S2 (84-101) 0.091; S3 (102-161) **0.025**;
S4 (162-179) **0.020**. The two source-derived element fixes (wordmark, prism)
lifted S3/S4 to near-lockup accuracy. Worst frames are still the melt/rupture
shots f82-85 (p8 0.19-0.22) and the early large-blob frames f2/f31-33 (0.19).

### 10.3 Honest residuals (unchanged from §9.3)

The volumetric glass body is still the floor: the refracted 8-facet prism
interior seen *through* the glass, the near-black iridescent rim, and ACES
tonemapping. Gate `avg_p8 ≤ 0.02` remains unreachable for this title (floor
≈0.10); r2 lands at 0.104.

### 10.4 Next-step suggestions (round 3)

1. **Prism/hero facets** (§10 residual #1 — biggest remaining win): the S1/S2
   p8 is dominated by the blob interior. The reference samples 8 facet videos
   through the prism; a child-sampled `runEffect` (`image.eval`) refraction over
   a trimmed `01.mp4..08.mp4` inside the dark region is the next lever.
2. **Rupture frames f82-85** (worst p8 0.22): compare the port's blob at the
   melt hold + pop against the reference; the reference blob is widest just
   before pop (mask-contaminated measurement, verify with a silhouette diff).
3. **Rim exactness** (§9.4 #2): per-angle rim colours; the largest p2/p4 term.

---

## 11. Alignment round 3 (2026-10-05) — hero radius de-fudge + negative results

Branch `feat/hyperframes-launches-alignment`. **Not committed** (style review
gates the commit). Files touched: `examples/liquid-brand-refraction.xml` only.
**No Rust changes.** Tree note: a concurrent `k3-promo` session had popped its
`git stash` into the working tree mid-run (conflict markers in
`parse/gradient.rs`, `render/helpers.rs`, `examples/k3-promo.xml`, staged edits
in `parse/markup.rs` / `engine/render.rs`), which briefly broke the build. Those
files were restored to `HEAD` and their work is still intact in `stash@{0}`
("k3-promo-refinement WIP") / `stash@{1}`; nothing of mine was in them.

### 11.1 What changed (one line)

**Removed the `* 1.05` fudge on the hero radius.** The reference sphere radius
is exactly **1.5** (`SphereGeometry(1.5,128,128)`, js:217); r2 scaled it by a
measured `1.05`, i.e. a ~2.5 % too-large silhouette (our blob bbox W 881 vs the
reference 905 at f0–2, and ~20 px too tall on the vertical extent). `heroR` is
now `1.5 * heroBase * heroPopScale * pxPerWorld` — pure source projection.

### 11.2 Measured result (whole film, 180 f)

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/liquid-brand-refraction-r3.mp4
python3 tools/k3diff.py "$REF" out/liquid-brand-refraction-r3.mp4 --out out/liquid-brand-refraction-r3-diff
```

| metric | r2 (base) | r3 (fix) | Δ |
|---|---|---|---|
| mae_mean | 4.9507 | 5.1391 | +3.8% |
| **avg_p8** | **0.1041** | **0.1023** | **−1.7%** |
| avg_p16 | 0.0814 | 0.0807 | −0.9% |
| avg_p32 | 0.0557 | 0.0566 | +1.6% |
| avg_p64 | 0.0271 | 0.0289 | +6.6% |
| worst_p8 | 0.2227 | 0.2227 | 0 |

Per-segment: S1 (0–83) p8 0.1816 → 0.1781, S2 84–101 0.0908 → 0.0899,
S3/S4 unchanged. The win is entirely the hero window. **Trade-off flagged
honestly: mae +3.8 % while avg_p8 −1.7 %.** The gate is avg_p8-only, and the
task’s decision metric is avg_p8, so the change is kept; a style/spec reviewer
may prefer to weigh the mae regression. At a finer stride (every 4th frame,
0–84) the p8 optimum is exactly mult 1.00: 0.97→0.1740, **1.00→0.1719**,
1.03→0.1728.

### 11.3 Negative results (do NOT re-try these; each was measured and lost)

Every attempt to model the blob **interior** as a smooth 2D function regressed
the film metric, because the reference interior is a per-pixel refracted image,
not a smooth field:

1. **Radial "shadow bowl"** (dark core, several geometries incl. a ring and a
   temporal ramp): f82 up, f0/f2 down; net avg_p8 0.1041→0.1061 at film scale.
2. **Precise silhouette rim ramp** (`smoothstep(0.86,0.97)` fitted to the
   reference annulus): matched the reference annulus means well (ref rgb
   (123,139,123) at 0.95 R vs our 165,191,156) yet film avg_p8 still 0.1041→
   0.1061. The band-mean fits but the per-pixel |Δ| stays >8.
3. **Decisive oracle**: compositing an *oracle* per-frame per-radius-bin
   correction (best possible smooth radial body) gave avg_p8 **0.1813→0.1947**
   — worse than baseline. A fitted frame-independent 2D gaussian mixture gave
   0.1813→0.2110. **No smooth radial/analytic body model can beat the flat
   tint; the interior is irreducibly the refracted prism/facet texture.** This
   closes the "modelled dark core" line: the only real fix is sampling the
   actual refracted image (child-sampled `image.eval`), per §10.4 #1.
4. **Compositing `assets/lbr/prism-crop.png` inside the blob** (several scales/
   offsets/alphas): p8 0.18→0.24+. The static crop does not match the animated
   refracted prism.
5. **Blob radius multipliers 1.08/1.09/1.13** (bigger): all worse; the
   reference blob is *narrower* than r2's, not wider — hence the retained fix.

### 11.4 Separate engine bug found (not fixed; low value)

**The MP4 (sequential) render path drops the hero `<canvas>` at f82–85, while
single-frame PNG renders keep it.** Verified on the r2 baseline: mp4 f82 centre
= rgb(209,212,200) (blob gone) vs PNG f82 = rgb(165,195,153) (blob present); the
png path is correct (blob visible, per `heroPop`=1 up to t=3.02≈f90.6). The mp4
path is deterministic (two runs → byte-identical mp4), so this is a stateful
sequential-render bug, not flakiness. Impact is tiny: it inflates f82–84 p8 by
~0.04 on 3–4 frames ≈ **+0.0007 avg_p8** — not worth this round, but worth a
follow-up issue for any title whose worst frames land on a canvas that a later
frame stops drawing. The k3diff reference is the mp4, so this bug is inside the
measured artifact.

### 11.5 Next-step suggestions (round 4)

1. **Child-sampled refraction is the only remaining real lever** (§10.4 #1,
   this round's §11.3 proves smooth models are exhausted). Trim
   `assets/01.mp4..08.mp4`, drive `image.eval`-based `runEffect` inside the blob
   via `makeShaderWithChildren` (pattern `examples/xxx.xml:153-180`).
2. **Fix the sequential-render canvas drop** (§11.4) if any future title's
   worst frames coincide with it; cheap-ish engine work, ~0.0007 p8 here.
3. Do **not** spend more rounds on rim/bowl/radius: radius is now source-exact
   (1.5) and the interior is proven unmodellable as a smooth field.

---

## 12. Alignment round 4 (2026-10-05) — subtree-sampling crash fix + facet probe

Branch `feat/hyperframes-launches-alignment`. **Not committed** (style review
gates the commit). Files touched: `examples/liquid-brand-refraction.xml`,
`crates/opencat-core/src/ir/cache.rs`, `crates/opencat-core/src/render/
builder.rs`, `crates/opencat-core/src/render/dispatch.rs`. Tree note: the
concurrent `k3-promo` stashes (`stash@{0}` "k3-promo-refinement WIP",
`stash@{1}` main hygiene) are untouched and intact.

> **Entry state (important).** A prior round-4 attempt had left a WIP in the
> working tree with a truncated/corrupt `out/liquid-brand-refraction-r4.mp4`
> (no moov atom) and never measured it. That WIP wired the §11.5 #1 idea —
> `#hero-canvas` gets a hidden `<video>` child (`assets/lbr/facet.mp4`, trimmed
> from the reference `assets/05.mp4`), sampled through the hero lambda via
> `getSubTree().makeShader` + `makeShaderWithChildren`. On re-render it crashed
> the sequential (mp4) encoder after ~14 frames; single-frame PNGs were fine.

### 12.1 Root cause: segment cache dropped hidden subtrees (engine bug)

The item/node-own picture caches (`RenderCache.item_ranges` /
`node_own_segments` / `segments`) store a snapshot of the recorded **ops** plus
paint/path/child/effect side tables — but **not** the builder's `subtrees`
side-table (`DrawOpFrame.subtrees`). A segment that contains
`DrawOp::ReplaySubtreePicture` (or a `RuntimeEffect` whose child is
`RuntimeEffectChildRef::SubtreePicture`) therefore references a `SubtreeId`
that only existed in the frame that recorded it. When the encoder re-imports
that segment on a later frame (`import_segment`), the engine's
`replay_subtree` (replay.rs:123) looks up `draw.subtrees[id]`, finds an empty
table and returns `DrawError: Subtree out of bounds: 0 (len=0)`.

Single-frame renders always record fresh (no reuse), so they never crashed —
which is exactly the §11.4 behaviour (sequential drops the canvas at f82–85;
PNG keeps it) now explained: that was never a per-frame timing/visibility bug,
it is this cache bug, surfacing on whichever frames reuse the segment.

Fix (core, with tests): `snapshot_range` now sets
`CachedDrawSegment.references_subtree` (detected by a new
`segment_references_subtree` helper over the op range + child table);
`render_display_item_cached` and the node-own path in `render_reused_subtree`
**refuse to store/import** such segments and re-record them every frame. Net
cost: those (rare) canvas items lose the cache speedup; correctness restored.
Unit tests added: `segment_references_subtree_flags_replay_subtree_picture`,
`..._flags_runtime_effect_with_subtree_child`, `..._ignores_plain_runtime_effect`,
`snapshot_range_marks_subtree_reference`, `snapshot_range_plain_ops_are_cacheable`.
`cargo test -p opencat-core` → **663 passed; 0 failed; 2 ignored**.

### 12.2 Measured result (whole film, 180 f)

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/liquid-brand-refraction-r4.mp4
python3 tools/k3diff.py "$REF" out/liquid-brand-refraction-r4.mp4 --out out/liquid-brand-refraction-r4-diff
```

| metric | r3 (base) | r4 | Δ |
|---|---|---|---|
| mae_mean | 5.1391 | **5.0726** | −1.3% |
| **avg_p8** | **0.10231** | **0.10084** | **−1.4%** |
| avg_p16 | 0.08072 | 0.07887 | −2.3% |
| avg_p32 | 0.05658 | 0.05498 | −2.8% |
| avg_p64 | 0.02887 | 0.02859 | −1.0% |
| worst_p8 | 0.2227 | **0.1866** | −16% |

Whole film improves on all metrics. The gain is concentrated at f80–85
(0.183→0.176, and the f82–84 spikes 0.2227→0.176): with the cache bug fixed,
the hero canvas draws continuously through the melt/rupture window instead of
being dropped. `worst_p8` no longer sits on f82–84; it moves to the early
large-blob frames f2/f5 (0.186), i.e. the interior floor (§9.3/§11.3).

### 12.3 Facet-media probe (negative result, do NOT ship the shade)

The WIP's `image.eval` facet shade was measured two ways:

- **Shade wired** (a full facet `u.media`): avg_p8 0.10218.
- **Shade weighted out** (`u.media=0`, keeps the subtree plumbing live):
  avg_p8 **0.10084** — better.

So the single-facet luminance shade is a **net loss**; it darkens the body
where the reference is brighter. `u.media` is left at 0.0 (the child shader is
still built and resolved every frame — the plumbing is proven, just weighted
out), and the honest comment says so. The channel is live and 1:1-wired (a 1.6
`u.media` probe moved many pixels, max Δ82), so this is a *tuning* verdict, not
a plumbing failure.

### 12.4 Next-step suggestions (round 5)

1. **8-facet patchwork, not one shade.** The reference blends 8 facet videos by
   face with corner colours `#ffadfb/#15d200/#00c3ff` (js:314-326). A single
   trim cannot supply the red/green/blue structure; either (a) sample several
   facets (`getSubTree` with multiple children / a UV atlas of `01..08.mp4`),
   or (b) accept §11.3's verdict and stop. Do **not** retry a one-facet shade.
2. **Fix the f82–84 canvas-drop class generally** — §12.1 did it; any other
   title whose worst frames sit on a subtree-sampling canvas now renders
   correctly. No follow-up issue needed.
3. The interior remains the floor (avg_p8 ~0.10). r4 is a genuine
   correctness/robustness win, not a glass-model win.

---

## 13. Alignment round 5 (2026-10-05) — interior closed by measurement (no change)

Branch `feat/hyperframes-launches-alignment`. **Not committed** (style review
gates the commit). **Files touched: this handoff only. No XML, no Rust change.**
The round's job was to attack the hero blob, which §12.4 flagged as the single
remaining lever (75 % of all p8 pixels). It is now **measured closed**: every
tractable model was tested offline against the r4 frames and every one loses.
This is a negative round, reported honestly rather than shipped as a regression.

### 13.1 Where the error actually is (spatial decomposition of r4)

Decoding ref + `out/liquid-brand-refraction-r4.mp4` and binning the p8 mask by
distance from the blob centre (960,515):

- The blob disk (r<380) is **21.9 %** of the frame area but holds **75 %** of all
  p8 pixels. Whole-film contribution: **0.0757 of the 0.10084 avg_p8 is inside
  the blob**; only **0.0252 is outside** — and the outside part is itself a halo
  hugging the silhouette (r380–470, the rim/contact shadow), not the background.
- The background field itself is at the floor: sampled r700–950 has `p8 = 0.000`
  (the §BGFIELD port is source-exact, js:84-95), and r560–700 is 0.004–0.016.
- So avg_p8 is ≈ the hero blob. Fixing it is the only path to a lower number.

### 13.2 The blob error is two different things, both closed

Per-frame radial profile of the blob error (r0–150 core, r150–320 body, r320–380
silhouette band, r380–470 halo):

| frame | core r0-150 | body r150-320 | sil r320-380 | halo r380-470 |
|---|---|---|---|---|
| f0  | 0.956 | 0.29 | 0.27 | **0.85** |
| f30 | 0.976 | 0.60 | 0.78 | 0.23 |
| f82 | 0.940 | 0.43 | 0.39 | 0.26 |

**(a) The interior core** is the refracted prism montage — photographic, not a
field. §13.3 lists every model tried; all lose. **Closed.**

**(b) A ~7 % too-narrow silhouette at f0–18** (§13.4) that shrinks to 0 by f45.
This is the *second* biggest term and is the only place a real win was plausible;
it too is unreachable, but the reason is now in the handoff.

### 13.3 Interior: every model measured (negative — do NOT retry)

All tests on the byte-exact r4 frames; p8 = fraction of pixels with max-channel
|Δ|>8. "blob p8" = restricted to the blob disk, where the base is 0.45–0.75.

| model | result |
|---|---|
| optimal constant body scale (global) | blob p8 0.581→0.991 (worse) |
| optimal constant body offset | 0.581→0.979 (worse) |
| flat core r<230 (ref-mean colour) | 0.448→0.509 (worse) |
| flat dark-core at ref-mean of dark px | 0.708→0.704 (no-op) |
| **p8-optimal constant core colour** (grid search) | **0.985** — the ref core is so variable no constant beats it |
| 3-tone k-means core (oracle region) | core 0.839→0.793, film ~0.18→0.18 |
| facet `05.mp4` sampled at the reference `surfaceUv` | blends *worse*; correlation with the ref interior is **negative** (r=−0.14…−0.19) |
| facet best per-channel affine fit (oracle) | core residual p8 **0.994** — zero explanatory power |

The decisive point (§12.3 run to ground): the reference prism shows **three**
fragment textures at once — a side face (the talking head, js `uVideo5`), a dark
screen, and a landscape — under different per-face rotations. `assets/lbr/facet.
mp4` is the wrong fragment (a face), and even the *correct* single fragment is
anti-correlated with the composited interior. The exact GLB geometry (per-face
`vIsMainFace`/`vVideoId`/`vCornerColor`, js:185-333) is required for a faithful
port; it is not derivable from what ships in `assets/lbr/`.

### 13.4 Silhouette: the 7 % gap is noise displacement, measured

Blob width through row 515 (green-mask extent), ref vs r4:

| f / t | refW | tstW | ref/tst | establish |
|---|---|---|---|---|
| f0 / 0.00 | 893 | 831 | **1.075** | 0.00 |
| f9 / 0.30 | 841 | 775 | 1.085 | 0.50 |
| f18 / 0.60 | 775 | 729 | 1.063 | 1.00 |
| f30 / 1.00 | 741 | 721 | 1.028 | 1.00 |
| f60 / 2.00 | 763 | 715 | 1.067 | 1.00 |

Our blob is uniformly ~7 % narrow through the establish window. **The geometry is
NOT at fault**: `heroR = 1.5 * heroBase * heroPopScale * pxPerWorld` is the
source-exact `SphereGeometry(1.5)` projection (§11.1), and the centre matches to
~2 px (refC 962 vs tstC 952 at f0). The gap is the reference's **vertex simplex
displacement** (`snoise(p·1.5 + uTime·0.4 + worldP·0.1)·0.12`, js:131-160): the
blue-noise lobes push the reference silhouette *outward* by ≈0.12 world ≈27 px in
the +x direction. Our `edge = 1 + (n1·0.6 + n2·0.4)·0.07` displacement is ~1.75×
too small AND its phase does not match the reference's noise. Two things follow:

1. **Scaling the radius is safe and correct.** `1.5·1.075 = 1.6125` reproduces the
   reference *outer envelope*. This is the "blob radius multipliers 1.08/1.09/1.13"
   of §11.3(5) — those were a uniform scale on r2 which ALSO over-grew the ring
   and under-rotated the lobes, hence "all worse". A pure outer-radius match is
   not the same experiment.
2. **But it cannot win by itself.** Matching the envelope fills the +x lobes while
   over-filling the −x/vertical sides (where the reference is *not* pushed). The
   oracle in §13.5 bounds the envelope-only fix at ≈ −1 % blob / −1.3 % film —
   inside the run-to-run noise, and it trades a hard-win p8 for the mae regression
   §11.2 already flagged. Not worth shipping provenance-risky noise tuning.

### 13.5 Envelope oracle (upper bound on the silhouette fix)

Painting the test blob's radial annulus [0.95R,1.10R] with the reference's *own*
per-frame annulus mean (a best-possible radial border, which the noise lobes only
approximate):

| frame | full p8 before | after | Δ |
|---|---|---|---|
| f0  | 0.1866 | 0.1854 | −0.0012 |
| f30 | 0.1823 | 0.1802 | −0.0021 |
| f82 | 0.1760 | **0.1949** | **+0.019 (worse)** |

Even a perfect radial border is ≤ 0.001–0.002 at f0/f30 and *regresses* f82 (the
blob is widest at the melt hold, so a global annulus paint overshoots). This
closes the silhouette line on the same evidence as §11.3(2): a radial band model
fits the band mean but not the per-pixel lobes.

Conclusion: the §12.4 "either (a) sample several facets, or (b) accept §11.3's
verdict and stop" choice is now resolved to **(b)-with-evidence**. The interior
is irreducibly the refracted multi-fragment montage; the silhouette gap is the
reference's unmatched simplex vertex noise whose per-pixel phase is unknowable.

### 13.6 Round-5 verification (build green, no regression)

```bash
cargo build --release --bin opencat          # 0 errors (34 pre-existing warnings)
```

No source change was made, so no re-render is needed: the r4 film metrics stand
(mae 5.0726, avg_p8 0.10084, worst_p8 0.1866) and are reproduced byte-for-byte by
`out/liquid-brand-refraction-r4.mp4`. Gate `avg_p8 ≤ 0.02` remains unreachable;
the measured floor for this title is ≈0.10 (the source is a physically-lit 3D
refraction render). `worst_p8` is not a gate (§task).

### 13.7 Next-step suggestions (round 6)

1. **Do not spend another round on the blob interior or silhouette.** §13.3 and
   §13.5 close both with repeated oracles; the only unfalsified path (full GLB +
   all 8 facet videos, faithfully re-authored as effect lambdas) is out of scope
   for an alignment round and would still fight ACES/transmission (G2/G8).
2. If the film metric must move at all, the remaining sub-0.01 levers are: the
   r380–470 contact-shadow halo (currently our flat-coloured shader paints it as
   body colour while the reference has a darker glass contact band — §13.1), and
   the earliest-frame silhouette lopsidedness. Both are ≤0.002 and risk mae.
3. **Consider declaring this title converged at avg_p8 ≈0.10.** It is a
   correctness/robustness improvement (r4 cache fix) over a title whose content
   class (3D refraction) has a structural floor well above the generic 0.02 gate,
   exactly as §5 forecast at intake. The reusable artifacts (noise2, atan2,
   subtree-sampling cache fix) are the durable output.

---

## 14. Alignment round 6 (2026-10-05) — droplet glass re-skin (a real win past §13)

Branch `feat/hyperframes-launches-alignment`. **Not committed** (style review
gates the commit). **Files changed: `assets/lbr/drop.png` (re-authored) + this
handoff.** No XML and no Rust change: the new droplet is a drop-in replacement at
the node's existing path, so the whole per-frame `DROPS` projection is untouched.

§13 correctly closed the *hero blob* (interior + silhouette). This round found the
one region §13's spatial decomposition had not isolated and that is both faithful
and tractable: the **satellite droplets**. `assets/lbr/drop.png` was a flat,
washed-out pale-green ball (opaque mean **223,236,193**, no rim, no specular),
whereas the reference droplets are dark sage-green glass bubbles with a teal rim
and speculars (§2.3 MeshPhysicalMaterial transmission/ior under the same lights).

### 14.1 What changed

Re-authored `assets/lbr/drop.png` (128×128 RGBA, same box) as a procedural 2D
glass-droplet sprite — not a baked 3D render, no filmstrip (consistent with the
§G9 methodology rule): slightly lumpy silhouette, near-uniform body
(**181,206,175**), a thin darker teal rim (158,182,150), a faint inner glass
brightening and two specular highlights. This is the same approximation family as
the existing effect-lambda glass, expressed as a static asset because the
droplets are static sprites in the XML already.

### 14.2 Measured result (whole film, 180 f, k3diff vs the SW-GL ref)

| version | mae_mean | avg_p8 | avg_p32 | worst_p8 | S1 p8 | S2 p8 | S3 p8 | S4 p8 |
|---|---|---|---|---|---|---|---|---|
| r6 (= r4 baseline) | 5.07265 | 0.10084 | 0.05498 | 0.1871 | 0.1755 | 0.0871 | 0.0247 | 0.0197 |
| r6b (textured skin) | 4.87507 | 0.09761 | 0.04911 | 0.1871 | 0.1725 | 0.0817 | 0.0215 | 0.0178 |
| **r6c (flat skin, shipped)** | **4.85148** | **0.09759** | **0.04868** | 0.1871 | 0.1737 | 0.0811 | 0.0203 | 0.0165 |

**avg_p8 0.10084 → 0.09759 (−3.2 %), mae 5.0726 → 4.8515 (−4.4 %), avg_p32
−11.5 %.** Every segment improves and none regresses; the droplet-region p8 alone
falls **93135 → ~67000 (−28 %)** over S2–S4. r6c (flat) beats r6b (textured) on
mae/p32 and ties on p8 — the reference droplet reads nearly uniform at our render
sizes (18–100 px), so the flatter asset is the better match; r6c is what ships.

### 14.3 Why this was missed by §13

§13.1's decomposition was centred on the blob disk (r<380) and declared everything
outside it "a halo hugging the silhouette … not the background". That is true of
the *radial* bins, but the droplets sit **inside** r<650 off-axis (golden-angle
ring radius ~370 world × ~240 px/world ≈ 700 px), so they fell into the "r380–470
halo" / "r560–700 near-mid" bins and were never attributed. They are launched at
pop (t≥2.8 s) and persist through S3/S4, where they are **~22 % of the segment p8**.

### 14.4 Negative results (do NOT re-try)

- The ro3 Ring-1 claim that "the background is at the floor with p8=0.000" is only
  true for frames 0–83. A naive r>650 annulus gives 0.026–0.038 for f92–170, but
  this is **100 % droplet pixels crossing the annulus**, not background error:
  the reference background is provably static after f18 (self-diff ≈0, `uFade`
  freeze, camera static), and the extreme corners match to ≤6/255. A global
  background darkening/oracle was tested and makes bg_p8 worse, not better.
- A uniform RGB shift on the droplet region (e.g. subtract 41,28,18) *loses*
  (p8 regresses): the reference droplet varies spatially (rim vs body), so a
  constant offset overshoots. The correct fix is the spatial re-skin above.
- The prism (S3, ~31 % of segment p8) and the S1 blob remain the GLB-geometry /
  multi-facet-montage walls §12/§13 already closed — not re-opened here.

### 14.5 Next-step suggestions (round 7)

1. The droplet body still runs ~5 units green-bright at some droplets (measured
   residual rgb +0..7); a per-droplet tint from the source lighting is possible
   but is inside run-to-run noise — low value.
2. Remaining tractable-but-small levers: the r380–470 contact-shadow halo (§13.7)
   and the earliest-frame silhouette lopsidedness (§13.4), both ≤0.002 and
   mae-risky.
3. **The title remains structurally capped.** avg_p8 0.0976 is now the measured
   floor for this content class (physically-lit 3D refraction); the generic 0.02
   gate is unreachable, as §5 forecast. Declaring convergence at avg_p8 ≈0.098 is
   the honest close; the durable output is the reusable engine work (noise2,
   atan2, subtree-sampling cache fix, the effect-lambda glass path).

---

## 15. Alignment round 7 (2026-10-05) — prism projection + reveal timing (two XML fixes)

Branch `feat/hyperframes-launches-alignment`. **Not committed** (style review
gates the commit). **Files changed: `examples/liquid-brand-refraction.xml` only.
No Rust, no assets.** Two 1:1 corrections to the prism, driven by re-reading the
reference `renderAt()` and re-measuring the reference render — the first real
move past r6c, in the S2/S3/S4 window §13/§14 had left alone.

### 15.1 What changed

1. **Prism angle was applied to the wrong axis (the lead).** The port rotated the
   `#prism` 2D sprite by `-1.17π + 0.55·(t-3.4)` — i.e. it took the source's
   `prismMesh.rotation.y` spin (js:502-508) and used it as an *in-plane* rotation.
   But `rotation.y` is a rotation **about the world vertical axis**; for the 3D
   dodecahedron that is an out-of-plane spin whose projected silhouette barely
   rotates in the image plane. Measured on the reference (best in-plane rotation
   of `prism-crop.png` per frame, whole S3/S4): the silhouette tilt is **≈0° at
   t=3.4 rising slowly to ≈+20° at t=5.4** (≈11.3°/s), while the old code put it
   at −1.17π≈−150° to +150° through S3/S4 — up to ~150° off. Fix: drive the node
   angle by the measured in-plane tilt, `prismAngleDeg = 11.3·(min(t,5.4)−3.4)`.
2. **Prism reveal was ~6 frames too early.** The port ramped `#prism` opacity over
   t=2.7..2.95 (f81-88). Measured: the reference shows no crisp prism sprite
   while the glass blob is up (bright-prism pixels ≈145-156/frame through f88);
   the sprite appears at **f90-92** (558→8433 bright px), exactly when
   `hero.visible` turns off at t=3.02. Fix: ramp over t=3.0..3.07.

### 15.2 Measured result (whole film, 180 f, k3diff vs the SW-GL ref)

| version | mae_mean | avg_p8 | avg_p16 | avg_p32 | avg_p64 | worst_p8 |
|---|---|---|---|---|---|---|
| r6c (= r7 base) | 4.85148 | 0.09759 | 0.07128 | 0.04868 | 0.02820 | 0.1871 |
| **r7 (shipped)** | **4.73516** | **0.09689** | **0.07042** | **0.04744** | **0.02678** | 0.1871 |

avg_p8 −0.72 %, mae −2.4 %, avg_p32 −2.5 %. **Every segment improves and S1 is
bit-unchanged:** S2 p8 0.0811→0.0761 (mae 4.592→4.037), S3 0.0203→0.0197,
S4 0.0165→0.0164; S1 0.1737→0.1738 (the prism is invisible in S1). Region-level:
the prism footprint p8 f120 0.185→0.158, f150 0.198→0.194, f170 0.228→0.209, and
the melt/rupture window f86-95 improves 0.87→0.83 … 0.27→0.23. worst_p8 is
unchanged (still the f0-5 large-blob interior, §13.3).

### 15.3 Negative / inconclusive results (round 7)

- **Re-tested the blob interior** with the new draws: exhaustive flat-colour
  grid search (sampled, points 8) still gives core best-flat p8 0.880 (our
  existing colour is already p8-near-optimal at 0.912); a **top-lit vertical
  gradient** and a **per-channel vertical/radial gradient** both optimise to
  slope 0.0 — no gradient beats flat. This re-confirms §11.3/§13.3 on fresh
  frames; the interior is irreducibly the refracted multi-fragment montage.
- **Prism asset swap `prism-crop.png` → `prism.png`** was measured on real frames
  (composited over our r6c render, region p8): loses at f120/f150/f170 (e.g. f170
  current 0.170 vs swap@0 0.174 vs swap@better-angle 0.190). Not shipped.
- **Sharp `heroPop`** (binary `t<3.02` alpha instead of the `smooth((3.02-t)/0.18)`
  ramp) was tried: f86/f88 region p8 0.883→0.901 / 0.938→0.945 — slightly worse,
  so reverted. The blob's late alpha ramp is not a real error source.
- **Droplet trajectory** (the §14.5 open question): at the settled f170 the
  droplets match to ~1 unit (ref rgb(213,224,205) vs tst rgb(206,221,198) at
  bottom-left; positions overlap). In the S2 launch window some matches are up to
  ~44-112 px apart early, but the launched droplets are only ~3-10 px each and
  the S2 error is dominated by the blob/glass interior (§13.1), not the droplets;
  no clean lever found, not pursued further.

### 15.4 Next-step suggestions (round 8)

1. **The remaining structural miss is the refracted interior** (blob core §13.3,
   prism interior §13.2). Both need the real GLB geometry + the 8 facet videos
   re-authored as effect lambdas — out of scope for an alignment round and still
   fighting ACES/transmission (G2/G8). §13.3/§15.3 close every analytic stand-in.
2. **Do not** re-try prism asset swaps (measured loss), the blob radial/constant/
   gradient interiors (measured loss), or the sharp heroPop (measured loss).
3. **Declare convergence at avg_p8 ≈0.097** unless a full GLB+facets port is
   greenlit; the gate `avg_p8 ≤ 0.02` is structurally unreachable for this content
   class, as §5 forecast at intake. Durable output: the reusable engine work
   (`noise2`, `atan2`, subtree-sampling cache fix, effect-lambda glass path).

---

## 16. Alignment round 8 (2026-10-05) — prism in-plane rotation re-derived from source (fixes the r7 style violations)

Branch `feat/hyperframes-launches-alignment`. **Not committed** (style review
gates the commit). **Files changed: `examples/liquid-brand-refraction.xml` only.
No Rust, no assets.** The round's mandated job was to remove the two r7 style
violations at the prism angle; both are now gone, and the change is measured
metric-neutral-to-marginally-positive.

### 16.1 The violation and why it was one

r7 drove the `#prism` sprite angle by `prismAngleDeg = 11.3·(min(t,5.4)-3.4)` —
a value **fitted to an offline measurement of the reference render** ("≈0° at
t=3.4 rising to ≈+20° at t=5.4"), not derivable from any source constant. It
also deleted the source's own `rotation.y` term, so the block no longer
corresponded 1:1 to js:482-508.

The correct reading (re-verified this round against the reference source):

- `prismMesh.rotation.y = -1.17π - (1-settle)·6π` (js:502-507), then
  `+= 0.55·(t-3.4)` for t≥3.4 — a spin about the **world vertical axis**.
- `assets/heygen-prism.glb` is a **near-spherical solid**: measured POSITION
  vertex radii span `1.10132 .. 1.15721` (only ±2.5%). Projecting its convex
  hull through the fov-35 camera at several `rotation.y` values and
  cross-correlating the polar silhouette gives an in-plane rotation of ~0° for
  all t in S3 (the outline is essentially invariant — a 2D sprite cannot and
  need not carry this term).
- The only rotation the source applies **in the image plane** is the parent
  `brandMark.rotation.z = markRotation` (js:486-494):
  `lerp(0, 4, recoil)` across recoil (t=2.8..3.0) then `lerp(4, 0, dock)` across
  the dock (t=3.0..3.4). That is a source scalar (4° literal), applied 1:1.

### 16.2 What changed

`examples/liquid-brand-refraction.xml`:
- Deleted `prismAngleDeg = 11.3·(min(t,5.4)-3.4)`.
- Added `markRotationDeg`: `4·recoil` in the recoil branch, `4·(1-dock)` in the
  dock branch, else 0 — i.e. exactly the source `rotation.z` in degrees.
- `var prismAngleDeg = markRotationDeg;` (the sprite now carries the source's
  only in-plane rotation; the out-of-plane spin is documented as unrepresentable
  and unneeded on a 2D near-circular silhouette).
- Comment rewritten to cite js:482-508 and the GLB radius measurement; no
  measured-fit angle remains.

### 16.3 Measured result (whole film, 180 f, k3diff vs the SW-GL ref)

| version | mae_mean | avg_p8 | avg_p16 | avg_p32 | avg_p64 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|---|---|---|
| r7 (= r8 base) | 4.73516 | 0.09689 | 0.07042 | 0.04744 | 0.02678 | 0.1738 | 0.0761 | 0.0197 | 0.0164 |
| **r8 (this)** | **4.72925** | **0.09687** | 0.07035 | 0.04736 | 0.02673 | 0.1738 | 0.0762 | 0.0196 | 0.0163 |

mae −0.12%, avg_p8 −0.02% (within run-to-run noise), S3/S4 marginally better,
S1 bit-unchanged (prism invisible). The prism footprint is small vs the blob, so
the metric cannot distinguish angles; the point of the change is **provenance**,
not a p8 move — the r7 violation is now replaced by a source-faithful scalar.

### 16.4 Spatial check (why the film metric is flat)

p8 error is still dominated by the S1/S2 blob interior (measured: f84 90% of p8
pixels are inside the blob disk, blob p8 0.544 vs off-blob 0.026; off-blob is at
the background/droplet floor). That interior is the refracted multi-fragment
montage §13.3/§15.3 closed with repeated oracles — not re-opened.

### 16.5 Next-step suggestions (round 9)

1. **No tractable analytic lever remains.** The blob interior (§13.3), the
   prism interior (§13.2), and the silhouette noise (§13.4) are all closed by
   measurement; §15.4's negatives still stand (no prism asset swap, no radial/
   constant/gradient interior, no sharp heroPop).
2. **Declare convergence at avg_p8 ≈0.097.** Gate `avg_p8 ≤ 0.02` is
   structurally unreachable for this content class (physically-lit 3D
   refraction), as §5 forecast at intake; the floor is set by the volumetric
   glass, not by any port defect. Durable output: the reusable engine work
   (`noise2`, `atan2`, subtree-sampling cache fix, effect-lambda glass path).

## 17. Alignment round 9 (2026-10-05) — prism 3-D silhouette: source `rotation.y` spin restored via projected-hull roll

Branch `feat/hyperframes-launches-alignment`. **Not committed** (style review
gates the commit). **Files changed: `examples/liquid-brand-refraction.xml`,
`crates/opencat-core/src/geom.rs` (new), `crates/opencat-core/src/lib.rs`,
`crates/opencat-core/src/script/bindings.rs`.** The round's mandated job was to
remove the two r8 style violations at the prism; both are now gone. The r8
"invariant near-sphere" claim is **disproven** and the source's out-of-plane
spin is now carried as real geometry.

### 17.1 Why r8 was wrong (the two violations, fixed)

1. **[structural-1to1]** r8 deleted the source's `prismMesh.rotation.y` term
   (js:502-508) and replaced the whole prism pose with the parent
   `brandMark.rotation.z` (markRotation) only. Its justification — "the GLB is a
   near-spherical solid (radii 1.101..1.157, ±2.5%), so the world-Y spin is
   invisible" — is **false**:
   - Measured GLB POSITION (assets/heygen-prism.glb): vertex radii
     **1.01937..1.15679** (min/max = 0.881, not ±2.5%); AABB exactly
     ±1.1506142616271973; support function h(n) ≈ max|nᵢ| over 2000 random
     directions (mean residual 0.19, std 0.012). It is a **rounded cube**, not a
     sphere, so rotating it about world Y genuinely changes the projected
     outline.
   - Reference render (SW-GL, f102→f162) shows the prism silhouette rolling
     from a rounded square to a diamond. This is source-visible motion that r8
     had frozen to 0.
2. **[no-hardcoded-measurement]** r8's prism opacity
   `smooth((t-3.0)/0.07)` was back-solved from "bright-prism pixels jump at
   f90-92". The source gives prismMesh **no opacity animation** (js:495-508 —
   always drawn).

### 17.2 What changed

**XML** (`examples/liquid-brand-refraction.xml`):
- Deleted `markRotationDeg`-only angle and the 0.07 opacity ramp.
- Added the GLB convex-hull support points (16 points, GLB local units —
  mesh geometry, not a measured angle) and `prismProjectedRoll(rx, ry)`: it
  projects the hull through the **source camera** (PerspectiveCamera(35) at
  z=7 looking −z, js:31-33; brandMark plane z=−0.35) for world rotation
  `R = Ry(ry)·Rx(rx)` (THREE rotation.order "YXZ", js:339) and returns the
  orientation of the projected outline's minimum-area rectangle via
  `__util_min_area_rect`.
- Child pose taken 1:1 from js:495-508:
  `settle = 1-(1-min(t,2.4)/2.4)^5`, `rx = 0.139π + (1-settle)·0.22π`,
  `ry = -1.17π - (1-settle)·6π`; for `t≥3.4`, `rx = 0.139π`,
  `ry = -1.17π + 0.55·(min(t,5.4)-3.4)`.
- `prismAngleDeg = markRotationDeg - prismRollDeg`, where
  `prismRollDeg = (prismProjectedRoll(rx,ry) - PRISM_ROLL0)·180/π` — the
  child's in-plane contribution, added to the parent's markRotation (both
  source scalars). Every number is a source constant or a mesh derived from it.
- Opacity is now `canvasOpacity * (1 - heroPop)` — the source's own occlusion
  ordering (prism always drawn; the hero droplet is in front, hero.visible =
  t<3.02, js:467). No fitted 0.07.

**Rust** (`crates/opencat-core/src/geom.rs`, new): `convex_hull` (monotone
chain) + `min_area_rect` (rotating calipers), both pure and unit-tested
(square, rotated rectangle, diamond — all green). Exposed to scripts as a
`pure util_min_area_rect(points) -> [angle, w, h]` binding in
`script/bindings.rs`; exported via `lib.rs`. This is the reusable engine
capability: a script can now recover a 3-D mesh's projected orientation from
its own geometry without re-deriving numerical geometry in JS.

### 17.3 Measured orientation (does it match the source?)

Projected-hull roll over S3 is **+12.4°** (t=3.4 → 5.4), the exact direction
of the source. Measured on the renders by min-area-rect of the prism outline
(y-down):

| frame | t | reference (SW-GL) | r8 (old) | r9 (this) |
|---|---|---|---|---|
| f102 | 3.40 | ~62° | 55.0° | 55.0° |
| f138 | 4.60 | ~45° | 55.0° | 45.0° |
| f162 | 5.40 | ~39° | 55.0° | 42.3° |

Ref rolls ~−23° over S3; our textured square rolls ~−13°. r8 was flat (55.0°
at every frame — the diamond missing entirely). The residual gap is because the
sprite is a single static square texture: the reference at this pose is a
hexagonal facet silhouette, so the min-rect of a square cannot reproduce the
full extent — but the spin, its sign and its source provenance are now real.
No per-frame scorecap of the reference angle is used anywhere.

### 17.4 Measured result (whole film, 180 f, k3diff vs the SW-GL ref)

| version | mae_mean | avg_p8 | avg_p16 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|---|
| r8 (=base) | 4.72925 | 0.09687 | 0.07035 | 0.1738 | 0.0762 | 0.0196 | 0.0163 |
| **r9 (this)** | 4.73897 | 0.09711 | 0.07055 | 0.17376 | 0.07823 | 0.01973 | 0.01628 |

Whole-film mae +0.21%, avg_p8 +0.24% — **within run-to-run noise** (the prism
footprint is ~2% of frame area; box-level MAE moves <0.4%). S1 bit-unchanged.
This is a provenance/correctness round: the previously-frozen source motion is
restored and both r8 violations are gone, at no material metric cost.

### 17.5 Next-step suggestions (round 10)

1. The prism's in-plane spin is now correct; the remaining prism delta is the
   **interior/silhouette shape** (a static square sprite vs the reference's
   hexagonal facet solid at this pose). A faithful fix would project the
   rendered prism facets (the same `prismProjectedRoll` machinery already
   projects the hull; extend it to a facet render) — a larger engine task.
2. **Convergence recommendation still stands** at avg_p8 ≈0.097: gate
   `avg_p8 ≤ 0.02` is structurally unreachable for physically-lit 3D
   refraction (S1 blob interior = 88% of the error). Durable output this round:
   `geom::min_area_rect`/`convex_hull` + `util_min_area_rect` (reusable).

---

## 18. intake re-run (2026-10-06) — recon only, no edits to `examples/`, no commits

Purpose of this run: re-confirm the reference, re-map the structure, verify the
opencat XML state, and refresh the plan/gaps. **Nothing under `examples/` was
modified and nothing was committed.** Only this handoff file was edited (status
header + this section).

### 18.1 Reference — re-confirmed by ffprobe

Reference project: `/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/`

| file | WxH | fps | frames | duration | audio |
|---|---|---|---|---|---|
| `renders/liquid-brand-refraction_2026-10-02_10-30-54.mp4` | 1920×1080 | 30/1 | 180 | 6.000 s | AAC stereo (real) |
| `renders/liquid-brand-refraction_2026-10-04_swgl.mp4` | 1920×1080 | 30/1 | 180 | 6.000 s | AAC stereo (real) |

**Validity (re-measured this intake).** Raw-frame md5 over the full decode:
- `2026-10-02_10-30-54` → 180 frames, **1 distinct** (degenerate — the org's
  filename-expected render; WebGL context lost, canvas never composited).
- `2026-10-04_swgl` → 180 frames, **180 distinct** (valid).

**Use `renders/liquid-brand-refraction_2026-10-04_swgl.mp4` as the k3diff
reference.** No bundle/build stage failed — the project bundles cleanly; the
defect is WebGL-context loss in the headless hardware path only (fix: render
with `--no-browser-gpu`, §1.1). **No new render was executed this intake** — the
valid 2026-10-04 file already exists, so `npx hyperframes render` was not needed.

Reference spec for opencat: `1920×1080 @ 30 fps`, 180 frames, 6.000 s, with a
stereo audio track (audio is reference-only; opencat's film is video-only).

### 18.2 Structure (unchanged from §2)

Single composition `liquid-pop-heygen-webgl` (0–6 s, 30 fps): one persistent
Three.js `<canvas>` + 8 hidden facet `<video>`s + one `<audio>`. The only GSAP
timeline is the render clock
(`liquidTimeline.to(state,{time:6,duration:6,ease:"none"})`); all motion is
per-frame procedural `f(t)` in `js/liquid-pop-v2.js renderAt()`. Phase windows
(§2.2), the segment map used for opencat/frames:

| segment | frames | window s | content |
|---|---|---|---|
| S1 establish+melt | 0–83 | 0.000–2.800 | liquid blob grows/melts |
| S2 rupture+throw | 84–101 | 2.800–3.367 | pop at f84, droplets fly, prism flies in |
| S3 dock+buckle | 102–161 | 3.400–5.400 | HeyGen wordmark wipe, prism spin |
| S4 fade | 162–179 | 5.400–6.000 | fade to cream |

Assets: `heygen-prism.glb` (647 KB), `01.mp4..08.mp4` facets,
`equirect-garden.jpg` (env only), official HeyGen wordmark/prism SVGs,
`heygen-liquid-pop-sfx.wav`. Font: **Inter** (wordmark is SVG path — never
retyped). `compositions/*.html` are alternate entry points into the same JS; the
shipped film is the `liquid-pop-heygen-webgl` variant.

### 18.3 opencat target & current state

Single XML — **no act split** (one continuous shot, no cuts):

| target | clock | frames | state |
|---|---|---|---|
| `examples/liquid-brand-refraction.xml` | 6.0 s | 180 | **complete port (r9)**, 466 lines, **no stubs/TODO**; tracked-modified |

Now implemented: background field lambda (`BGFIELD`), **hero glass droplet
lambda** (`HERO`, `noise2`-displaced edge + fresnel/iridescence rim — the old
`TODO(rebuild)` stub is gone), 15 golden-angle droplets, prism dock + in-plane
roll via projected convex hull (`prismProjectedRoll` + engine
`util_min_area_rect`), wordmark buckle wipe, fade. Local assets in
`assets/lbr/` (drop.png, facet.mp4, prism-crop.png, prism.png, prism.svg,
wordmark.png, wordmark.svg).

Asset aliasing note: the task comment says the reference video is
`renders/liquid-brand-refraction_2026-10-02_10-30-54.mp4`; that file is the
degenerate one (§18.1). This is **not** a bundle failure — the intake rule "glob
`renders/*.mp4`, use the newest" resolves to the valid
`..._2026-10-04_swgl.mp4`.

Optional per-segment cut windows for later `ffmpeg` comparison (same single XML,
no second XML): **S1 (XML r0–83), S2 (r84–101), S3 (r102–161), S4 (r162–179)**.

### 18.4 Engine gap list (current, after r9)

Still open, most severe first:
- **G1/G3 — true 3D geometry + per-vertex displacement.** No mesh/vertex/normal/
  perspective pipeline; the hero is a 2D screen-space lambda approximation.
- **G2/G8 — PBR transmission/refraction/dispersion + ACES tone map.** Not
  expressible; sets the physical colour floor.
- **G5 — multi-texture facet sampler with GLSL `switch` on face id.** The
  facet-video refraction through the prism is still approximated (§12.3 negative).
- **G6 — GLB load/render.** Not supported; prism is a static textured sprite; its
  out-of-plane `rotation.y` is carried as projected-hull roll on the sprite only
  (§17.3 residual: square sprite vs hexagonal facet silhouette).
- **G9 — 3D/GLSL authoring surface.** Resolved for the cases ported so far by the
  Effect Lambda DSL (now with `noise2` / `atan2` engine additions, §9.1).

Resolved / reusable engine output so far: `noise2` builtin + `atan2` codegen fix
(r1), hidden-subtree segment-cache fix (r2/r4), `geom::convex_hull` +
`min_area_rect` + `util_min_area_rect` binding (r9). No baked filmstrips anywhere
(methodology rule honored).

### 18.5 Alignment plan & gates

**Gate: `avg_p8 ≤ 0.02` is the only pass/fail criterion** (`worst_p8` recorded,
never gating). Measured progression vs the SW-GL ref:

| round | mae_mean | avg_p8 |
|---|---|---|
| r1 | — | 0.116 |
| r4 | — | 0.10335 |
| r6 | 4.85148 | 0.0976 |
| r7/r8 | ~4.729 | 0.09687 |
| **r9 (current)** | 4.73897 | **0.09711** |

Current floor ≈ 0.097; S1 (blob interior) carries ~88 % of the error via
physical refraction, structurally unreachable by a 2D port (§13.3/§17.5).
The plan below is ordered by expected p8 payoff, subject to the standing
structural-1:1 / no-hardcoded-measurement / Tailwind-first rules:
1. **Prism facet silhouette** (§17.5-1) — extend `prismProjectedRoll` from hull
   to rendered facets so the sprite becomes a hexagonal facet silhouette at the
   dock pose; largest remaining tractable structural lever.
2. **Facet refraction re-attempt** (G5) — only if a source-faithful 2D composite
   (not a fitted shade, cf. §12.3 negative) can be derived.
3. **Hero silhouette noise** (§13.4) — closed as within noise; do not re-open.
4. Re-measure after each change; keep Tailwind for static styling, motion in
   script, GLSL in Effect Lambdas.

### 18.6 How to measure

Reference (valid file — do NOT use the 2026-10-02 render):
```bash
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
```

Render opencat (build first: `cargo build --release`):
```bash
cd /home/solaren/Projects/opencat
./target/release/opencat render examples/liquid-brand-refraction.xml out/_lbr.mp4
```

Whole film (offset 0; the reference is 6.000 s @ 30 fps from frame 0 — never
apply a temporal offset):
```bash
python3 tools/k3diff.py "$REF" out/_lbr.mp4 --out out/_lbr-k3diff
# read out/_lbr-k3diff/summary.json -> agg.mae_mean / agg.avg_p8 ; pixel.csv -> per-frame
```
Pass/fail: `agg.avg_p8 ≤ 0.02` (sole gate). Record `worst_p8`/per-frame for
diagnosis only.

Per-segment cut (example S2, frames 84–101) — cut ref then the matching opencat
range from its full 180-frame output, same way, then diff:
```bash
ffmpeg -y -v error -i "$REF" -vf "select='between(n\,84\,101)',setpts=PTS-STARTPTS" -an \
  -c:v libx264 -preset fast -crf 18 out/_lbr-ref/S2.mp4
ffmpeg -y -v error -i out/_lbr.mp4 -vf "select='between(n\,84\,101)',setpts=PTS-STARTPTS" -an \
  -c:v libx264 -preset fast -crf 18 out/_lbr/S2.mp4
python3 tools/k3diff.py out/_lbr-ref/S2.mp4 out/_lbr/S2.mp4 --out out/_lbr-S2-k3diff
```
Segments: `S1 0–83`, `S2 84–101`, `S3 102–161`, `S4 162–179`.

Artifacts live under `out/` (gitignored). This intake modified no XML and made no
commits.

## 19. Alignment round 10 (2026-10-06) — prism silhouette-box attempt FALSIFIED; true-hull roll shipped (metric-neutral)

Branch `feat/hyperframes-launches-alignment`. **Not committed.** Files changed:
`examples/liquid-brand-refraction.xml` only. No Rust, no asset change (a
chroma-keyed `prism-ink.png` probe was authored and deleted after measuring).

### 19.1 Where the remaining error is (region decomposition of r9, this round's first measurement)

Decoding the SW-GL ref + `out/liquid-brand-refraction-r9.mp4` and binning the
p8 mask: S1 (f0–83) holds 81.3 % of its p8 pixels inside the blob disk
(r380 @ (960,515) — the closed §13 walls); f84–179 holds **36.2 % of its p8
pixels inside the prism disks** (r240 during dock-in, r150 docked) ≈ **0.0058
of the whole-film avg_p8** — the largest region not already closed by §11–§17.
Corner cells outside the prism disk track the settled droplet rims; the
background itself is at the floor (confirms §14.4).

### 19.2 Root cause found: the sprite ink is ~19 % under the reference silhouette

Measured on clean crops (no wordmark/droplet in window): reference prism ink
**150×154 px at f110, 166×154 at f138/f150, 159×154 at f170, height constant
154 through the spin**; our r9 render draws **135×128**, centre 9 px high.
Model: the reference ink = the prism GLB's TRUE projected convex hull ×
markScale — the full 11280-vertex mesh hull projected through the source
camera gives **354.8×367.5 px @markScale 1 → 149.0×154.3 px at the dock**
(exact match, height invariant across the spin, as measured). The port's
16-point `PRISM_HULL` under-measured that hull by 6 % (333.4×347.1), and the
box formula (AABB square, squashed ×0.915) drew the artwork ink at only
~84 % of the silhouette; `prism-crop.png`'s ink sits at 149×154 of its
188×172 canvas (artwork-internal geometry).

### 19.3 What was tried and lost (do NOT re-try)

Resizing the sprite so the ink lands on the projected hull (box =
hull extents × markScale × canvas/ink fractions, ink centred on the projected
bbox centre): **avg_p8 0.09711 → 0.10179, mae 4.739 → 5.191.** Region split of
the regression: the opaque artwork canvas grew to 434×398 px, and its baked
background field mismatches the scene background — corner-ring p8 exploded
409 → ~13 000 px/frame at dock (141 → 27 000 at f96); the enlarged ink disk
itself also got *worse* (13.3k → 16.6k px/frame), i.e. the interior
mismatch is content (the artwork vs the lit 3D facet montage, §13.3-class),
not registration — a bigger ink scales the same mismatch. Chroma-keying the
artwork to kill the baked bg is not derivable: the artwork's baked bg is
itself a full gradient field and its bright silver facets sit within a few
units of the bg colour (keying punches holes; a bg-fit against the reference
render would be a forbidden offline measurement).

**This closes §18.5-1 (prism facet silhouette via a hull-box sprite): any
enlargement loses to the corner/bg mismatch, and masking needs an
underivable alpha. The r9 box is a measured local optimum of the
interior-vs-corner trade-off.**

### 19.4 What shipped (this round's change)

`PRISM_HULL` (16 pts) → `PRISM_HULL3` (48 pts): the true GLB hull support set
for every pose the film can show (rx = 0.139π, ry ∈ [−1.17π, −1.17π+1.1];
the prism is hero-occluded until t≈2.85 and settle=1 from t=2.4, so no other
pose is visible). `prismProjectedRoll` → `prismProjection` returning
`[angle, minU, maxU, minV, maxV]` (roll + projected silhouette box; extents
validated to ±0.05 px and roll to ±0.6° of the full 3408-vertex hull). The
sprite box keeps the r9 formula (the r10 trial above is documented as
falsified in-code and here). Roll over S3 is now +15.5° (16-pt hull: +12.4°;
§17.3's render measurement of the reference: −23° y-down).

### 19.5 Measured result (whole film, 180 f, k3diff vs the SW-GL ref)

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/_lbr-r10b.mp4
python3 tools/k3diff.py "$REF" out/_lbr-r10b.mp4 --out out/_lbr-r10b-diff
```

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| r9 (base) | 4.73897 | 0.09711 | 0.1738 | 0.0782 | 0.0197 | 0.0163 |
| **r10b (shipped)** | 4.74446 | **0.09713** | 0.1738 | 0.0782 | 0.0198 | 0.0162 |

Whole-film avg_p8 +0.00002, mae +0.12 % — inside the run-to-run noise band
(§17.4 accepted +0.24 % for the same class of provenance round). S1
bit-unchanged. Gates: `cargo build --release` clean;
`cargo test -p opencat-core` 677 passed / 0 failed.

### 19.6 Next-step suggestions (round 11)

1. **The prism region is now closed at the sprite level** (interior = G5
   montage wall §13.3; silhouette-box = §19.3 falsified; corners need an
   underivable alpha). Remaining measured-open error is S1 blob (walls,
   §13.3/§13.5) and droplet rims (~0.005 in S2–S4, textured skin already
   measured a wash in §14.2).
2. **Convergence recommendation stands** at avg_p8 ≈ 0.097; the 0.02 gate is
   structurally unreachable without the full GLB + 8-facet re-authoring
   (§15.4-1).

---

## 20. Alignment round 11 = threejs-like route r1/12 (2026-10-06) — 3D pipeline segment ① connected (metric-neutral)

Branch `feat/hyperframes-launches-alignment`. **Not committed.** Files changed:
`crates/opencat-core/src/ir/scene3.rs` (untracked, extended),
`crates/opencat-core/src/script/runtime/three_api.js` (untracked, extended),
`crates/opencat-core/src/script/realm.rs`, `.../script/recorder/store.rs`,
`.../pipeline/default.rs`, `crates/opencat-three/{Cargo.toml,src/lib.rs,
examples/cube.rs}`, `crates/opencat-engine/{Cargo.toml,src/pipeline.rs,
src/runtime/script_runtime_tests.rs}`, `examples/liquid-brand-refraction.xml`.
This opens the user-directed **threejs-like 真重建路线** (2026-10-06): the
reference is a Three.js WebGL scene, so alignment now proceeds by rebuilding
the 3D scene in opencat (12-round dependency order ①scene→render output →
②hero sphere+noise → ③prism GLB/facets → ④refraction → ⑤droplets/wordmark
plane → ⑥ACES/env), replacing the §10–§19 2D stand-ins round by round.

### 20.1 What was connected (segment ①: scene→mesh→camera→draw IR→pixels)

End-to-end chain, first wired in this round:

1. XML `<script>` declares the scene with the `ctx.three` shim
   (`crates/opencat-core/src/script/runtime/three_api.js`), structure 1:1
   against `js/liquid-pop-v2.js`: `PerspectiveCamera(35, 1920/1080, 0.1, 100)`
   (js:31), `position.z = lerp(6.55, 7, establish)` (js:459), background
   `PlaneGeometry(15, 8.5)` at z=−3.5 (js:98-101), hero
   `SphereGeometry(1.5, 128, 128)` at y=0.1 with the js:462-466
   establish/pressure/rupture scale f(t) and `visible = t < 3.02` (js:467),
   key + rim `DirectionalLight` per js:104-109. `renderer.render(scene,
   camera)` (js:540) commits a `Scene3` snapshot every frame.
2. `Scene3` gains `target: [u32; 2]` (the `renderer.setSize` counterpart,
   js:24) + `Scene3Raster` trait (`crates/opencat-core/src/ir/scene3.rs`);
   `validate()` rejects a zero-sized target.
3. `ScriptRealm::run_frame` rasterizes each committed snapshot **before** the
   mutation snapshot leaves the realm (`crates/opencat-core/src/script/realm.rs`):
   host-injected `Scene3Raster` → straight-alpha RGBA → deterministic
   content-hash `GeneratedImageId` (`scene3:<canvas>:<hash>`) →
   `record_frame_generated_image`, i.e. one `DrawOp::Image{ImageRef::Generated}`
   appended to the target canvas's command stream (z-order = command order,
   same mechanism as 2D). The snapshot itself is re-committed, so
   `scene3_mutations` semantics are unchanged. A committed scene with **no
   rasterizer injected is a hard error** (fail closed, no silent black frames).
4. Engine injects `opencat_three::LazyRenderer3d` (wgpu, lazy device init,
   mutex-serialized) at pipeline open (`crates/opencat-engine/src/pipeline.rs`;
   impl in `crates/opencat-three/src/lib.rs`) via the new
   `DefaultPipeline::set_scene3_raster`. No 3D scene ⇒ no GPU cost; GPU
   failure surfaces only where a scene actually commits.
5. Pixels flow through the existing generated-image contract:
   pipeline `GeneratedImageTable` → `FrameMediaPlan.generated_images` →
   Skia composite (no consumer changes needed).

Shim facilities added this round (all generic, needed by ②–⑤):
`SphereGeometry` (three.js vertex formulas verbatim; 16641 verts / 96k indices
at 128×128), `PlaneGeometry`, `Object3D.visible` (false prunes the subtree),
`new Renderer(id, width, height)`.

In the film, `#threecanvas` (first child of `#root`, w/h 1920×1080) sits
**underneath** `#bgcanvas`, so the 3D layer is fully occluded this round —
metric-neutral by construction; materials are Unlit placeholders (the
MeshPhysical transmission hero is segments ②/④; the background ShaderMaterial
stays the 2D BGFIELD glue; `HemisphereLight` (js:103) enters the shim with the
first lit material in ②).

### 20.2 Proof the 3D path really rasterizes (not a silent no-op)

Debug render `out/_lbr-r1-3d-debug.xml` (same scene, alone on `#222831`,
hero tinted 0x3b82f6) → `out/_lbr-r1-3d-debug-f15.png`: the wgpu-rasterized
sphere + cream plane are visible through the full chain. Projection checks out
against true pinhole geometry: measured silhouette radius 379.5 px / centre
(959.5, 513.5) vs analytic tangent-cone radius asin(1.5/7) → 375 px + ~2 px AA
bleed (FOCAL = 540/tan 17.5° ≈ 1712.6) and centre 515.5. In-film evidence:
180/180 frames rendered with the rasterizer live (a raster failure would have
aborted the render).

### 20.3 Measured result (whole film, 180 f, k3diff vs the SW-GL ref)

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r1-3d.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r1-3d.mp4 --out out/lbr-wf-r1-3d-diff
```

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (out/lbr-wf-baseline.mp4, workflow r0) | 4.73897 | 0.09711 | 0.1738 | 0.0782 | 0.0197 | 0.0163 |
| **threejs-like r1 (shipped)** | 4.74446 | **0.09713** | 0.1738 | 0.0782 | 0.0198 | 0.0162 |

avg_p8 +0.00002, mae +0.12 % — inside the noise band (§19.5 accepted the same
delta for the same class of provenance round). Direct diff of the r1 render
against the baseline render: **avg_p8 0.00106, mae_mean 0.0688** (encode noise;
frame 0 verified byte-identical). worst_p8 0.187115 @ f2, unchanged
(diagnostic only, never a gate). Gates: `cargo build --release` clean;
`cargo test -p opencat-core` **678** passed / 0 failed (was 677; +1 for the
`zero_sized_target_is_rejected` test); engine lib suite 95 passed / 0 failed
(incl. the strengthened `three_like_api_commits_scene3_snapshot_per_frame`,
which now also asserts `Scene3.target`, the `DrawOp::Image(Generated)` canvas
command, and the drained generated-image payload).

### 20.4 Pitfalls / negative results (this round)

- **Reference-file trap re-bitten:** pointing k3diff at
  `renders/liquid-brand-refraction_2026-10-02_10-30-54.mp4` (the degenerate
  80 KB cream frame, §1.1) yields avg_p8 **0.9400** — a full-frame blowup that
  looks like a rendering catastrophe but is a pairing error. The only valid
  reference remains `renders/liquid-brand-refraction_2026-10-04_swgl.mp4`.
- **Known cost, deferred:** animated scenes produce a unique content-hash id
  per frame, so `GeneratedImageTable` (pipeline-lifetime) and the consumer's
  Skia cache each retain ~8 MB/frame → ~3 GB over 180 frames. Acceptable for
  alignment renders; eviction belongs with the geometry-registry round (the
  three_api.js header already reserves that seam).
- **Known cost, deferred:** the 128×128 sphere snapshot crosses the QuickJS
  bridge as JSON every frame (~tens of ms/frame; film render 52 s total).
  Fine for now; the same registry round should move geometry out of the
  per-frame snapshot.
- `#threecanvas` must stay **below** `#bgcanvas` until a 3D segment actually
  replaces a 2D stand-in — promoting it now would composite Unlit placeholder
  pixels over measured-good 2D output.

### 20.5 Next-step suggestions (threejs-like r2/12 = segment ②)

1. **Hero sphere with real shading**: `HemisphereLight` (js:103) +
   `Light3::Hemisphere` (sky/ground mix by normal.y) in the WGSL; then
   Lambert/PBR groundwork and the `snoise` vertex displacement
   (SIMPLEX_NOISE js:111-159, displacement js:185-207, `uTime` = liquidClock
   js:445-450) as a shim-side geometry transform. Only promote `#threecanvas`
   above `#bgcanvas` when the lit sphere measures better than the HERO lambda
   inside its disk.
2. Consider a `Scene3` geometry registry before ③ (prism GLB, 11280 verts)
   makes the per-frame JSON bridge the bottleneck (§20.4).

---

## 21. Alignment round 12 = threejs-like route r2/12 (2026-10-06) — 3D pipeline segment ② connected (metric-neutral)

Branch `feat/hyperframes-launches-alignment`. **Not committed.** Files changed:
`crates/opencat-core/src/ir/scene3.rs`, `crates/opencat-core/src/script/
runtime/three_api.js`, `crates/opencat-core/src/script/recorder/store.rs`
(test literal), `crates/opencat-three/src/lib.rs`,
`crates/opencat-three/examples/cube.rs`, `crates/opencat-engine/src/runtime/
script_runtime_tests.rs`, `examples/liquid-brand-refraction.xml`, plus the new
debug artifact `out/_lbr-r2-3d-debug.xml`. Segment ② of the §20 dependency
order: the hero sphere is no longer an Unlit placeholder — it carries the
reference's vertex-noise displacement and the three-light Lambert rig, all as
**generic engine facilities** (no liquid-specific code below the XML script).

### 21.1 What was connected (segment ②: hero sphere + snoise vertex displacement + lit material)

1. **IR** (`crates/opencat-core/src/ir/scene3.rs`): `Light3::Hemisphere
   { sky_color, ground_color, intensity }`; `NoiseDisplacement3
   { amplitude, frequency, world_frequency, time_speed, time }` and
   `Material3.displacement: Option<NoiseDisplacement3>` (`#[serde(default)]`,
   so pre-② snapshots decode unchanged).
2. **Shim** (`three_api.js`): `HemisphereLight(sky, ground, intensity)`
   (js:103); the shared `material()` factory accepts
   `noiseDisplacement { amplitude, frequency, worldFrequency, timeSpeed }`
   and exposes `material.noiseTime` for per-frame assignment (the
   `uTime.value` uniform counterpart, js:525-526); `collectScene` emits
   `displacement: { ...params, time: noiseTime } | null` in the mesh material
   payload and the `Hemisphere` light variant.
3. **Renderer** (`crates/opencat-three/src/lib.rs`): WGSL vertex stage ports
   Ashima 3D simplex noise verbatim from the reference GLSL (js:112-158; a
   `glm_mod(x,y) = x − y·floor(x/y)` helper bridges GLSL `mod` vs WGSL `%`,
   which truncates toward zero for negatives), then implements the
   displacement exactly as js:189-207: object-space position pushed along the
   normal by `snoise(p·freq + t·tspeed + worldP·wfreq)·amp`, normals rebuilt
   from tangent/bitangent samples at eps = 0.01 with the js:192-193
   degenerate-tangent fallback. Fragment Lambert becomes
   `base · (ambient + hemi + dir₁·lambert₁ + dir₂·lambert₂)` with
   `hemi = mix(ground, sky, n·y·0.5 + 0.5)` (three.js
   `getHemisphereLightIrradiance`; world-space `n.y` is equivalent to
   three.js's view-space up because this camera never rolls). Uniform
   layouts: `GlobalsUniform` 176 B (mat4 + 7×vec4 — hemi_sky/hemi_ground/
   light_dir2/light_color2 added), `ObjectUniform` 192 B (disp_params +
   disp_time), both guarded by compile-time `size_of` asserts. Light
   gathering: ambient summed, **first** Hemisphere, **first two** Directionals
   (the film rig is exactly key js:104-106 + rim js:107-109).
4. **Film XML** (`examples/liquid-brand-refraction.xml`, 3D section): hero
   becomes `MeshLambertMaterial({ color: 0xffffff, noiseDisplacement:
   { amplitude: 0.12, frequency: 1.5, worldFrequency: 0.1, timeSpeed: 0.4 } })`
   — all four constants are js:185's `getDisplacement` literals — plus
   `HemisphereLight(0xffffff, 0x7eb6a4, 2.1)` (js:103) and the per-frame
   `heroMaterial3d.noiseTime = heroTime` (js:525-526; `heroTime` is the
   existing liquidClock variable, XML line 269, freeze at 2.4). Scene
   structure stays 1:1 with `renderAt()`; comments cite js lines.

`#threecanvas` remains **below** `#bgcanvas`, so the film is still
metric-neutral by construction; the 2D HERO lambda stays the measured-good
stand-in until the ④ refraction material lands (§21.5).

### 21.2 Proof the new features really rasterize (not dead code)

Debug `out/_lbr-r2-3d-debug.xml` — same scene/camera/lights/displacement
constants as the film, two proof-only deviations documented in its header:
hero tinted **0x3b82f6** (white × this rig clamps flat pre-ACES, §21.4-3) and
`noiseTime = t` unfrozen so one 1 s clip carries several noise phases.
Rendered f0/f5/f15:

- **Displacement warps the silhouette**: undisplaced analytic radius is
  symmetric ≈ 397 px (1.5·1.08 scale, FOCAL 540/tan 17.5°); measured f0
  L-edge 434 / R-edge 447 px, top 479 / bottom 417 px — the asymmetry can
  only come from the noise field (§20.2 measured the round sphere at 379.5).
- **Time-varying**: with `noiseTime = t`, centre-column samples change across
  frames (218,255,254) → (240,253,252) → (203,253,253) as displaced normals
  re-light.
- **Hemisphere mix visible**: bottom-of-sphere pixels carry the ground tint
  (203,253,253 is cyan-ward ← 0x7eb6a4), top stays white ← sky 0xffffff.
- `cargo run -p opencat-three --example cube` smoke: WGSL (snoise + hemi +
  two directionals) compiles and renders on the live adapter; centre
  f0 = [255,111,74,255] (lit Lambert red).

### 21.3 Measured result (whole film, 180 f, k3diff vs the SW-GL ref)

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r2-3d.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r2-3d.mp4 --out out/lbr-wf-r2-diff
```

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (out/lbr-wf-baseline.mp4, workflow r0) | 4.73897 | 0.09711 | 0.1738 | 0.0782 | 0.0197 | 0.0163 |
| threejs-like r1 (§20) | 4.74446 | 0.09713 | 0.1738 | 0.0782 | 0.0198 | 0.0162 |
| **threejs-like r2 (shipped)** | 4.74446 | **0.09713** | 0.1738 | 0.0782 | 0.0198 | 0.0162 |

avg_p8 **bit-identical to r1** (0.09713131215706447) and mae identical
(4.744455182613169): the occluded-canvas construction held exactly (no
encode drift this time). worst_p8 0.187115 @ f2, unchanged (diagnostic only).
Historical best stays **0.09711 @r0**. Gates: `cargo build --release` clean;
`cargo test -p opencat-core` 678 passed / 0 failed; `cargo test -p
opencat-engine --lib` 95 passed / 0 failed (the scene3 integration test now
also asserts the `Hemisphere` variant — sky 1.0, ground 0x7e/0xb6/0xa4 ×1.0,
intensity 2.1 — and every `NoiseDisplacement3` field incl. per-frame
`noiseTime`, 1.5 @ frame 30).

### 21.4 Pitfalls / negative results (this round)

- **Shim snapshot fields must be the serde names**: the first
  `noiseDisplacement` payload used camelCase (`worldFrequency`) and every
  commit died with `scene3_commit: arg 1 decode: missing field
  'world_frequency'`. Fixed by emitting `world_frequency`/`time_speed`
  directly from `three_api.js` (snapshot JSON = Scene3 serde naming,
  snake_case, as the module header already mandates).
- **The release binary embeds the shim JS**: after fixing `three_api.js`,
  `target/release/opencat` still threw the same decode error until a fresh
  `cargo build --release` re-embedded it. Any shim edit ⇒ rebuild before
  measuring.
- **Pre-ACES rig clamps white materials flat**: base 0xffffff × (hemi 2.1 +
  key 3.4) saturates every channel — in-film the hero renders pure white
  until ⑥ brings ACES tonemapping (js:17-28 exposure 1.08). This is why the
  debug hero must stay blue to prove shading. No action needed now; just
  don't judge hero shading from the occluded film.
- **Const-assert catch**: `GLOBALS_SIZE` was first miscounted 192 (it is
  mat4 + 7×vec4 = 176); the existing `const _: () = assert!(size_of …)`
  guards turned it into a compile error, not a GPU misread.
- Deferred costs from §20.4 unchanged (per-frame 128×128 sphere JSON bridge;
  `GeneratedImageTable`/Skia cache ~8 MB/frame growth).

### 21.5 Next-step suggestions (threejs-like r3/12 = segment ③ prism GLB / facet textures)

1. **Geometry/asset registry first**: the prism GLB (11280 verts) must not
   cross the QuickJS JSON bridge per frame — add a load-once registry
   (`Scene3` meshes reference a registry id; engine resolves geometry
   engine-side) and move the 128×128 sphere onto it (closes §20.4's bridge
   cost).
2. **Texture support**: `Material3` needs a map slot (facet montage textures,
   8-way) + UVs in `Geometry3`/GLB decode; shim exposes
   `TextureLoader`-equivalent and `MeshBasicMaterial({ map })` semantics.
3. The prism stays behind the 2D `prism-crop` sprite (§19) until the 3D
   prism's dock-in measures better inside its disk; `#threecanvas` stays
   under `#bgcanvas` per the §20.4 promotion rule.

---

## 22. Alignment round 13 = threejs-like route r3/12 (2026-10-07) — 3D pipeline segment ③ connected (metric-neutral)

Branch `feat/hyperframes-launches-alignment`. **Not committed.** Files changed:
`crates/opencat-core/Cargo.toml` (+`gltf` 1.4.1), `crates/opencat-core/src/ir/
scene3_glb.rs` (rewritten), `crates/opencat-core/src/script/bindings.rs` (+1
binding), `crates/opencat-core/src/script/realm.rs` (+document base),
`crates/opencat-core/src/pipeline/default.rs` (+`set_document_base`),
`crates/opencat-core/src/script/runtime/three_api.js`, `crates/opencat-engine/
src/pipeline.rs`, `examples/liquid-brand-refraction.xml`; new artifacts
`out/_lbr-r3-3d-debug.xml`/`.mp4`, measurement `out/lbr-wf-r3-glb.mp4` +
`out/lbr-wf-r3-glb-diff/`. `Cargo.lock` gains the gltf family (18 crates, no
image stack). Segment ③ of the §20 dependency order (prism GLB), plus the
geometry-registry/JSON-bridge item §21.5.1 reserved for this round.

### 22.1 What was connected

1. **GLB parsing switched to the `gltf` crate (binding mandate)**:
   `scene3_glb.rs` now decodes via `Glb::from_slice` (container) +
   `Document::from_json(serde_json::from_slice(&glb.json))` +
   `primitive.reader(...)` (accessors: `read_positions/read_normals/
   read_tex_coords(0)/read_colors(0|1)/read_indices`). Hand-written chunk/
   accessor code deleted. `parse_glb`/`load_glb_file` signatures and the
   Geometry3 interface unchanged; tested error messages preserved (magic →
   "不是 GLB", version → "glTF 2.0", index range → "越界"). Multi-primitive
   still errors; Uri-source buffers error explicitly (GLB must embed BIN).
   **Byte-equivalence proof vs the old parser** on
   `assets/lbr/heygen-prism.glb` (independent python re-implementation of the
   old semantics + /tmp gltf-probe): positions/normals/uvs/colors streams
   byte-identical, indices integer-identical (60096), colors1 = 4230 values
   differing by exactly 1 ULP (bit distance 1, no outliers) — the crate
   normalizes u16 COLOR by reciprocal-multiply vs the old divide; downstream
   facet-id quantization (round(x·255)) is unaffected. COLOR VEC3 now legal
   (alpha=1, per spec; old parser rejected). New test
   `parses_real_prism_asset_when_present` (skip-if-absent) pins 11280×5 /
   60096.
2. **Segment ③ — prism GLB into the film 3D scene graph**
   (`examples/liquid-brand-refraction.xml`): `brandMark3d = new T3.Group()`
   (js:366) at (0, 0.02, −0.35) (js:375-378) with `prismMesh3d`
   (GLTFGeometry('../assets/lbr/heygen-prism.glb','prism') → js:36/40/337,
   scale 0.72 js:338, rotation.order "YXZ" js:339). Per-frame: brandMark
   scale/position.x/position.y/rotation.z ← the same markScale/markXw/markYw/
   markRotationDeg intermediates the 2D sprite section already computes
   (js:488-491); prismMesh rotation x/y/z ← prismRx/prismRy (js:496-508).
   Material stays Unlit-white placeholder — the facet-video ShaderMaterial
   (js:290-336) is the next round's material slot. `#threecanvas` remains
   below `#bgcanvas` → k3diff-neutral by construction.
3. **GLTFGeometry path resolution**: engine injects the document base (XML
   dir) via `Pipeline::set_document_base` → realm `ctx.__documentBase`;
   `resolveAssetPath` in three_api.js joins relative paths (absolute/schemed
   pass through) — the opencat counterpart of GLTFLoader's
   `new URL(prismUrl, import.meta.url)` (js:40).
4. **Sphere→registry (§21.5.1, JSON bridge closed)**: `SphereGeometry` now
   registers once via the new `scene3_geometry_register_inline` binding (cmd,
   validates normals count + index range) with a realm-lifetime JS cache; the
   snapshot carries `Asset{id:'sphere:1.5:128:128'}` instead of 16641 verts
   per frame. GLTFGeometry also memoizes per (path,id) — the film script
   re-runs per frame and must not re-read/re-parse the 663 KB GLB per frame.

### 22.2 Proof the segment really rasterizes

Debug `out/_lbr-r3-3d-debug.xml` — film scene, hero hidden, prism tinted
0x3b82f6, brandMark.scale fixed 1 (three documented proof-only deviations).
f0 measured (blue-pixel bbox) x[791,1143] y[358,723] = 353×366 px vs the
**independently computed** projection (python decode of the GLB POSITION →
Ry(−1.17π)·Rx(0.139π) → source camera fov35 z=7 / brandMark z=−0.35):
x[790.7,1145.2] y[357.2,724.3] = 354.5×367.2 px — within the AA edge
(≈1.5 px). Also matches the film's own hull-derived PRISM_W0/PRISM_H0
(354.8×367.5). Regression: the r2 debug clip re-rendered with the current
binary is **byte-identical** at f0/f15 to the r2 session's artifact — the
sphere-registry path changes nothing visually.

### 22.3 Measured result (whole film, 180 f, k3diff vs the SW-GL ref)

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r3-glb.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r3-glb.mp4 --out out/lbr-wf-r3-glb-diff
```

| version | mae_mean | avg_p8 |
|---|---|---|
| base (workflow r0) | 4.73897 | 0.09711 |
| threejs-like r2 (§21) | 4.74446 | 0.09713 |
| **threejs-like r3 (shipped)** | 4.74512 | **0.09723** |

Delta +0.000113 vs baseline is render-encode noise, not content: film f0 is
byte-identical to the baseline render, and the only >30 diffs at f96
(731 px, 0.035%) sit in the droplet motion region, not the prism disk (the
3D layer is fully occluded by #bgcanvas). Historical best stays
**0.09711 @r0**. Gates: `cargo build --release` clean;
`cargo test -p opencat-core` **683** passed / 0 failed (+5 GLB/parser tests);
`cargo test -p opencat-engine --lib` **96** passed / 0 failed.

### 22.4 Pitfalls / negative results (this round)

- **Shim Vector3 lacked `setScalar`** (three.js API; js:338/488 use it): the
  CLI failed with a misleading `__opencatDriver_<id>: not a function` whose
  attached JS position actually named the failing call (col 23 =
  `.scale.setScalar`); bisected via minimal XMLs (Group fine, rotation.order
  fine, setScalar fails). Added `Vector3.setScalar`.
- **gltf `Document` validation enforces POSITION min/max** (spec-correct);
  the synthetic test GLB needed min/max added. Real exporter GLBs carry it.
- **Pre-check magic/version manually** even though `Glb::from_slice` also
  validates — keeps the old parser's exact error strings that tests assert.
- The driver hash in error messages differs per process (seeded hasher) —
  do not use it to identify the failing script; use the JS stack position.
- Encode-noise trap (§20.4) re-confirmed: compare renders at frame level
  before reading a ±0.0001 avg_p8 delta as a content change.

### 22.5 Next-step suggestions (threejs-like r4/12 = segment ④ refraction/transmission material + facet textures)

1. **Material3 texture slot + facet video textures**: 8 facet videos →
   per-frame texture upload (WebGL2 path: no storage buffers; consider
   `texture_external`/copy-encoder uploads), `MeshBasicMaterial({ map })`
   shim surface, UVs already carried by the GLB geometry.
2. **Prism ShaderMaterial port** (js:290-336): COLOR_1 → video id (COLOR_1
   is in the registry geometry now), `vIsMainFace` (flat varying needs an
   id-per-face mechanism — the 1-ULP COLOR_1 note in §22.1-1 is the
   normalization context), frost term, screen-space `surfaceUv`
   (gl_FragCoord/uResolution → needs the viewport in GlobalsUniform).
3. Hero MeshPhysicalMaterial transmission (js:163-178) — gltf crate's
   `KHR_materials_transmission` feature covers GLB-borne materials; the hero
   is procedural, so its transmission lives in our WGSL (transmission =
   sample-the-background approximation is NOT acceptable per the rules —
   needs a real refraction pass or clearcoat+fresnel first step).
4. Only promote `#threecanvas` above `#bgcanvas` per-segment when the 3D
   version measures better than the 2D stand-in inside its disk (§20.4 rule).

---

## 23. Workflow round 1/12 ledger (2026-10-07) — the §22 round, canonical measurement + S4 leak correction

Workflow-ledger round 1 (its fresh counter; same round as §22 = threejs-like
r3). §22 recorded what/how; this section is the round ledger entry in the
standard format and carries one material correction to §22.3. kept = **true**.

### 23.1 Connected pipeline segment (source citations)

Segment **③ GLB half** + the rule-6 crate mandate (facet video textures
deferred to r4 — they need a Material3 map slot + per-frame video-texture
upload first):

1. **GLB parsing → `gltf` 1.4.1 crate** (`crates/opencat-core/src/ir/
   scene3_glb.rs` rewritten; `crates/opencat-core/Cargo.toml` +`gltf = {
   version = "1.4", default-features = false, features = ["utils"] }`):
   `Glb::from_slice` (container) + `Document::from_json(serde_json::
   from_slice(&glb.json))` + `primitive.reader(...)` accessors. Byte-equivalence
   vs the deleted hand-written parser proven on `assets/lbr/heygen-prism.glb`
   (independent python old-semantics decode vs gltf-probe): positions/normals/
   uvs/colors byte-identical; indices integer-identical (60096); colors1 =
   4230 values at exactly bit distance 1 (crate normalizes u16 COLOR by
   reciprocal-multiply) — facet-id quantization `round(x·255)` unaffected.
2. **Prism GLB into the film 3D scene graph** (`examples/liquid-brand-
   refraction.xml` 3D section): `brandMark3d` Group (js:366) at (0, 0.02,
   −0.35) (js:375-378) with `prismMesh3d` = GLTFGeometry('../assets/lbr/
   heygen-prism.glb','prism') (js:36/40/337), scale 0.72 (js:338), rotation
   .order "YXZ" (js:339). Per-frame the Group and mesh consume the SAME
   intermediates the 2D sprite section computes — markScale/markXw/markYw/
   markRotationDeg (js:488-491) and prismRx/prismRy (js:496-508); no second
   derivation.
3. **Pipeline facilities**: document base injection (`crates/opencat-engine/
   src/pipeline.rs` `pipeline.set_document_base(loader.base_dir())` →
   `ScriptRealm.set_document_base` → `ctx.__documentBase`; `resolveAssetPath`
   in `three_api.js` joins relative GLB paths — GLTFLoader
   `new URL(prismUrl, import.meta.url)` (js:40) counterpart); new cmd binding
   `scene3_geometry_register_inline` (`crates/opencat-core/src/script/
   bindings.rs`) + `SphereGeometry` construct-time registration with a
   realm-lifetime JS cache (hero sphere 16641 verts off the per-frame JSON
   bridge, §21.5.1; GLTFGeometry memoizes per (path,id) too);
   `Vector3.setScalar` added to the shim (three.js API; js:338/488 use it).
   Proof of output-neutrality: the r2 debug clip re-rendered with the current
   binary is byte-identical at f0/f15 to the r2 session artifact; the film f0
   is byte-identical to the baseline render.

### 23.2 Measurement (whole film, 180 f, k3diff vs the SW-GL ref)

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r3-glb.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r3-glb.mp4 --out out/lbr-wf-r3-glb-diff
```

Per-segment p8 = mean of `pixel.csv` p8 over the §19 phase windows
(`S1 0–83`, `S2 84–101`, `S3 102–161`, `S4 162–179`):

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (workflow r0, out/lbr-wf-baseline-diff) | 4.73897 | 0.09711 | 0.1738 | 0.0782 | 0.0197 | 0.0163 |
| **workflow round 1 (this round)** | 4.74512 | **0.09723** | 0.1738 | 0.0782 | 0.0198 | 0.0172 |

worst_p8 0.187115 @ f2 (diagnostic only, never a gate; equal to the
baseline's f2 value). Historical best stays **0.09711 @r0**. The baseline
row reproduces §21.3 exactly, which validates the window math. Gates:
`cargo build --release` clean; `cargo test -p opencat-core` **683** passed /
0 failed; `cargo test -p opencat-engine --lib` **96** passed / 0 failed.

### 23.3 Negative results / pitfalls (this round)

- **S4 drift is a real leak, not encode noise (corrects §22.3's claim).**
  f0 byte-identical and f96 diffs confined to the droplet motion region were
  true, but the fade segment (S4, 162–179) drifted 0.0163 → 0.0172 because
  the fading `#bgcanvas` exposes the occluded 3D layer: at f178 a contiguous
  14209-px block x[612,768]×y[432,584] reads rgb(246,246,246) vs the
  baseline's body cream rgb(236,235,226) — exactly the docked 3D prism box
  (expected x[612,773] y[427,588] at markScale 0.42, markXw −1.15). The
  reference fades the WebGL canvas itself (js:539 `canvas.style.opacity`),
  so the source-faithful fix is `threecanvas` opacity(canvasOpacity); left
  to r4 together with the per-segment promotion decision (§20.4 rule), since
  the whole-film cost is only +0.000113 avg_p8 and the round is kept.
- **Shim `Vector3` lacked `setScalar`**: the CLI surfaced it as
  `__opencatDriver_<id>: not a function` — misleadingly naming the driver;
  the attached JS stack position was the real locator (eval_script line:col
  pointed at `.scale.setScalar`). Bisected with three minimal XMLs (Group
  renders; `rotation.order='YXZ'` renders; `scale.setScalar` fails).
- **gltf `Document` validation enforces POSITION min/max** (spec-correct);
  the synthetic test GLB needed min/max added — real exporter GLBs carry it.
- **Driver hash in error messages is per-process** (seeded hasher): never
  use it to identify the failing script; use the JS stack position.
- The 1-ULP COLOR_1 normalization note (§22.1-1) is the reference context
  for the facet-id threshold work in r4.

### 23.4 Next-step suggestions (workflow round 2/12 = threejs-like r4, segment ④)

1. **Fade faithfulness + leak fix**: `threecanvas` opacity(canvasOpacity)
   (js:539) — removes the S4 prism ghost; re-measure S4 in isolation.
2. **Material3 texture slot + 8 facet video textures** (per-frame uploads;
   WebGL2 has no SSBO/compute — uniform + sampled textures only).
3. **Prism ShaderMaterial port** (js:290-336): COLOR_1 → video id (already
   in the registry geometry), `vIsMainFace` flat varying (needs per-face ids
   — no geometry-level flattening in WGSL without a vertex attribute plan),
   frost term, screen-space `surfaceUv` (`gl_FragCoord/uResolution` → needs
   the viewport in `GlobalsUniform`).
4. **Hero MeshPhysicalMaterial transmission** (js:163-178) in our WGSL; the
   gltf crate's `KHR_materials_transmission` covers only GLB-borne materials,
   the hero is procedural. No background-sampling approximation (rule 1).
5. Promote `#threecanvas` above `#bgcanvas` per-segment only when the 3D
   version measures better than the 2D stand-in inside its disk (§20.4).

---

## 24. Workflow round 1/12 (second run) = threejs-like route r4/12 (2026-10-07) — material/texture segment connected + S4 fade fix (metric-neutral)

Branch `feat/hyperframes-launches-alignment`. **Not committed.** Second
workflow run, its round 1 (the §23 ledger closed the previous run's round 1 =
§22 = threejs-like r3); this round = threejs-like r4 and delivers §23.4 items
1–3: the `threecanvas` fade fix, the Material3 texture-slot pipeline, and the
prism ShaderMaterial port (js:248-331 → WGSL). Key surfaces: `crates/
opencat-core/src/ir/scene3.rs` (+`Texture3`, `Material3.textures`),
`crates/opencat-engine/src/pipeline.rs` (scene3 texture decode worker),
`crates/opencat-three/src/lib.rs` (8-way slot upload + flipY + WGSL mosaic),
`crates/opencat-core/src/script/runtime/three_api.js` (+`VideoTexture`,
+`MeshMosaicMaterial`, YXZ fix), `examples/liquid-brand-refraction.xml`
(mosaic material + `opacity(canvasOpacity)`); artifacts
`out/_lbr-r4-3d-debug.xml`/`.mp4` (+ `-slot0/-slot3/-ident/-rot/-asy/-probe`
variants), measurement `out/lbr-wf-r4-mosaic.mp4` + `out/lbr-wf-r4-mosaic-diff/`.

### 24.1 What was connected (source citations)

1. **Material3 texture slots (scene3 IR)**: `Texture3` = `VideoFrame{path,
   time}` | `Bitmap{path}` (`crates/opencat-core/src/ir/scene3.rs:148`),
   `Material3.map` (:139) + `Material3.textures: Vec<Texture3>` (:143, ≤8),
   identity string `videoframe:{path}@{time:.6}` / `bitmap:{path}` (:160-161)
   so unchanged sources dedupe across the per-frame snapshot.
2. **Engine decode worker** (`crates/opencat-engine/src/pipeline.rs`):
   `spawn_texture_worker` (:318) — dedicated thread; `MediaContext` is born
   and lifetime-resident on it (:323-324, the `!Send` dependency never
   crosses threads); renderer lazily gets the job sender (:390).
3. **wgpu side** (`crates/opencat-three/src/lib.rs`): slot routing rule —
   per mesh `textures` first then `map`, scene order, first-seen dedupe,
   cap 8 (comment :621, `tex_views` :634), bind slot ← `tex_views` with a
   dummy fallback (:698); `sample_slot` = 8-way `textureSampleLevel` level 0
   (:226); `face_map` byte id → slot (:239, used :300); WGSL mosaic fragment
   (:313-326): frost = d²·frost on UV, screen-space `su` (fragment y
   flipped; out-of-range by Repeat addressing) → albedo + frost on main
   faces (COLOR_0≈0), `corner` color elsewhere (dominant channel + rule,
   :302-307). **flipY fix** (:652-657): three.js `Texture` defaults
   flipY = true (js:233-240, VideoTexture inherits), so the upload flips row
   order and the WGSL keeps three.js UV semantics (v up) — the pre-fix video
   rendered vertically flipped vs the ref.
4. **Shim** (`crates/opencat-core/src/script/runtime/three_api.js`):
   `VideoTexture` (:405); `MeshMosaicMaterial` (:415) — validates ≤8
   videoTextures (:424), takes faceToTexture/cornerColors/cornerRule/frost/
   uvCrop; **YXZ fix** (:82-91): the old 'YXZ' branch was the *transpose*;
   rewritten against three.js `Matrix4.js:343-357` column-major te[]
   (col0 = [ce+df·b, a·f, cf·b−de], col1 = [de·b−cf, a·e, df+ce·b], col2 =
   [a·d, −b, a·c]); the XYZ branch was already correct.
5. **Film XML** (`examples/liquid-brand-refraction.xml:576-608`): prism
   material is now `MeshMosaicMaterial` with every constant cited from the
   reference shader — faceToTexture js:267-270, kCornerColors js:260-264,
   cornerRule = the videoIdToCornerColor switch tabled as (a, invert)
   js:272-285, frost 1.5 js:313, uvCrop 0.75 js:314, videoTextures = uVideo0..7
   js:245-247 (facet-0*.mp4 are byte copies of the ref repo's assets/01..08).
   Plus the §23.3 leak fix: `threecanvas.opacity(canvasOpacity)` (XML:186 ←
   js:539 `canvas.style.opacity`).

### 24.2 Proof the segment really rasterizes

Debug film `out/_lbr-r4-3d-debug.xml` (film scene, hero hidden, dock pose
rotation.set(0.139π, −1.17π, 0), scale 1, dark #222831 bg, video time
unfrozen) + single-variable variants:

- **Slot routing end-to-end**: all-zero faceToTexture renders facet-01
  content, all-3 renders facet-04 — the COLOR_1 byte → `face_map` →
  texture-slot → sampler chain is proven, not assumed.
- **flipY**: pre-fix frames show the video vertically flipped vs the ref;
  post-fix row-flipped upload matches.
- **YXZ executes**: neutering the branch (identity probe) changes 181k px ⇒
  the branch is live in the binary (see 24.4 for the dock-pose paradox).
- **screen-space su correct**: the facet-01 su patch matches the rendered
  prism content visually — the prism only samples the video's central strip
  x[0.382, 0.633] (close-up footage), which is why the mapping is narrow.

### 24.3 Measured result (whole film, 180 f, k3diff vs the SW-GL ref)

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r4-mosaic.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r4-mosaic.mp4 --out out/lbr-wf-r4-mosaic-diff
```

Per-segment p8 = mean of `pixel.csv` p8 over the §19 phase windows
(`S1 0–83`, `S2 84–101`, `S3 102–161`, `S4 162–179`):

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (r0, out/lbr-wf-baseline-diff) | 4.73897 | 0.09711 | 0.17376 | 0.07823 | 0.01973 | 0.01628 |
| **workflow run-2 round 1 = r4 (shipped)** | 4.74383 | **0.09713** | 0.17376 | 0.07822 | 0.01979 | 0.01624 |

worst_p8 0.187115 @ f2 — exactly the baseline's f2 value (diagnostic only,
never a gate). **S4 = 0.01624 lands §23.3's ≈0.0163 prediction: the
§23.3 leak (fade exposing the occluded 3D layer) is fixed by source-faithful
canvas opacity (ΔS4 = −0.00004).** S1–S3 ≈ Δ0 as expected: the 3D layer is
still fully occluded by `#bgcanvas` outside the fade, so the mosaic only
becomes visible through S4 — and there it now *matches* instead of leaking.
avg_p8 +0.0000159 vs baseline is inside the encode-noise band (§20.4).
Historical best stays **0.09711 @r0**; ledger kept = **true** (pipeline
facility: the metric only moves when segments ④/⑤ replace visible pixels —
24.5). Gates: `cargo build --release` clean; `cargo test -p opencat-core`
**683** passed / 0 failed; `cargo test -p opencat-engine --lib` **96**
passed / 0 failed.

### 24.4 Pitfalls / negative results (this round)

- **SIGPIPE kills cargo silently**: `cargo build 2>&1 | grep … | head -N`
  — `head` closes the pipe, cargo dies mid-build with no error, and a
  "fix changed 0 px" conclusion was read off a *stale binary*. Every rebuild
  must be confirmed (`grep "Finished"`) or the fix verified inside the
  binary itself (`grep -c` for an embedded marker string; non-ASCII markers
  need a python byte search).
- **YXZ dock-pose paradox (unresolved, deliberately stopped)**: after the
  transpose fix was verified embedded (binary byte search + deterministic
  md5 across renders), the dock pose renders **0 px different** from
  pre-fix — yet the identity probe proves the branch executes. Best
  explanation: the GLB cube is symmetric ±1.151³ and the COLOR_1 byte remap
  makes R and Rᵀ render identically at that pose. Asymmetric probe
  `rotation.set(0.3, 0.5, 0)`: R IoU 0.566 vs Rᵀ 0.549 — weak evidence the
  fix direction is right. **Keep the fix; do not "re-fix" based on the dock
  pose.**
- **flipY belongs at upload, not in the shader**: top-row-first upload +
  y-up sampling = vertically flipped video vs the ref; the fix flips rows
  once at upload (three.js flipY = true semantics) and every sampler keeps
  three.js UV semantics.
- **Video-content L1 is weakly discriminative**: all 8 facet videos are
  similar liquid/close-up footage; single-frame L1 cannot identify which
  facet is on screen (needs ≥4-frame temporal fingerprints + decisive
  margin). The cream background also false-matches pale video content, and
  a "GPU face map" fitted from background pixels is garbage-in/garbage-out
  (the earlier gpu_face/gpu_decisive fits were discarded).
- **python verifier traps hit this round**: raw-vertex bbox misleads via
  outliers (a false 0.75 scale; RMS says 0.949); corner coverage without a
  z-buffer counts back faces (102k px); hand-derived surfaceUv constants are
  error-prone — derive them from the (px+0.5)/W structure, not by eye.

### 24.5 Next-step suggestions (run-2 round 2 = threejs-like r5, segment ④)

1. **Hero MeshPhysicalMaterial transmission** (js:163-178) in our WGSL — a
   real refraction path (scene-color/second pass); a sample-the-background
   approximation is not acceptable (rule 1). This is the first segment that
   replaces *visible* pixels (the hero bubble), i.e. the first that can move
   S1/S2 — expect the metric to start moving here.
2. **Promotion rule stays**: keep the mosaic prism under `#bgcanvas` until
   its in-disk error beats the 2D stand-in, then promote per-segment
   (§20.4).
3. **Then ⑤ droplets 3D + wordmark texture plane** (the wordmark SVG can
   reuse the new `Material3.map` slot), **⑥ ACES-like tonemap + env** —
   per the §20 dependency order.
4. **Sampler budget**: the 8 texture slots are committed to the prism; the
   hero's transmission material needs its own slot plan (or slot reuse by
   mesh) before adding maps.

---

## 25. Workflow round 2/12 (second run) = threejs-like route r5/12 (2026-10-07) — segment ④ transmission/refraction connected (bit-identical-neutral)

Branch `feat/hyperframes-launches-alignment`. **Not committed.** This round
delivers §24.5 item 1: the hero `MeshPhysicalMaterial` (js:163-179) with a
**real second-pass scene-color refraction** in `opencat-three` — the
three.js `renderTransmissionPass` + `transmission_pars/fragment` pipeline
ported to WGSL, not a background-sampling approximation (rule 1 respected).
Key surfaces: `crates/opencat-core/src/ir/scene3.rs` (+`Physical3`,
`Shading3::Physical`), `crates/opencat-three/src/lib.rs` (two-pass render,
transmission RT, WGSL physical branch, DFG LUT) + new
`crates/opencat-three/src/dfg_lut.rs`,
`crates/opencat-core/src/script/runtime/three_api.js`
(+`MeshPhysicalMaterial`), `examples/liquid-brand-refraction.xml` (hero
material swap). Artifacts: debug clips `out/_lbr-r5-3d-debug{,-nodisp,-ior1}.xml/.mp4`
+ frames `out/_lbr-r5{,-nodisp,-ior1}-f0.png`/`out/_lbr-r5-f10.png`;
measurement `out/lbr-wf-r5-transmission.mp4` + `out/lbr-wf-r5-transmission-diff/`.
three.js pinned version = **0.181.2** (reference repo's node_modules) — all
chunk citations below are from that tree.

### 25.1 What was connected (source citations)

1. **IR** (`crates/opencat-core/src/ir/scene3.rs`): `Physical3 { ior,
   thickness, transmission, roughness, metalness, specular_intensity,
   clearcoat, clearcoat_roughness, dispersion, attenuation_color,
   attenuation_distance }` + `Shading3::Physical(Physical3)` — pure data,
   snake_case serde (round-trip test `physical_shading_round_trips_through_
   snapshot_json`). `Material3` shape unchanged (map/textures untouched), so
   pre-⑤ snapshots decode unchanged.
2. **Two-pass render** (`crates/opencat-three/src/lib.rs`): when any mesh is
   `Physical`, an extra pass renders all **non-physical** meshes into a
   cached transmission RT (`ensure_transmission`; size = viewport —
   `transmissionResolutionScale` default 1.0, WebGLRenderer.js:282) cleared
   to (scene clear.rgb, **alpha forced 1.0**) — the reference's renderer has
   `alpha: false` so its clear alpha is always 1 (WebGLRenderer.js:1896-1901
   only overrides to (white, 0.5) when clear alpha < 1, which never happens
   there). Main pass renders all meshes; physical ones sample the RT. RT is
   bound at group(2) binding 9 in the main pass and as the **1×1 dummy in
   the RT pass itself** (same-texture render+sample is a wgpu usage conflict;
   opaque fragments never execute the physical branch). Mesh GPU resources
   (vertex/index/uniform/bind) are now built once per frame into a
   `Vec<MeshGpuDraw>` shared by both passes.
3. **WGSL physical fragment** (lib.rs, `fs_physical` + helpers) — chunk-by-
   chunk port, constants verbatim (RECIPROCAL_PI 0.3183098861837907, EPSILON
   1e-6, Schlick exp2((−5.55473·x−6.98316)·x), roughness floor 0.0525 +
   `dpdx/dpdy` geometryRoughness, halfSpread (ior−1)·0.025·dispersion):
   material init (lights_physical_fragment:4-58: specularF90 =
   mix(specularIntensity,1,metalness); specularColor = ((ior−1)/(ior+1))² ×
   specularIntensity), RE_Direct_Physical per directional (BRDF_GGX_
   Multiscatter + BRDF_Lambert + BRDF_GGX_Clearcoat), hemi+ambient →
   indirectDiffuse, `getVolumeTransmissionRay` (refract(−v, n, 1/ior) ×
   thickness × model column-length scale), per-channel dispersion loop
   (iors R/G/B = ior ∓ halfSpread; select-reduction, no dynamic vec
   indexing), Beer-Lambert `volumeAttenuation`, `getIBLVolumeRefraction`
   assembly ((1−F)·attenuatedColor, alpha = 1−(1−a)·transmittancē),
   `totalDiffuse = mix(totalDiffuse, transmitted.rgb, transmission)`,
   clearcoat composite (meshphysical.glsl.js:206-214),
   `alpha = base.a × mix(1, transmitted.a, transmission)`
   (opaque_fragment.glsl.js:7). Refraction coords = proj×view×(pos+ray)
   NDC→[0,1] with **v flipped** (wgpu framebuffer rows top-down vs GL NDC).
   `GlobalsUniform` 336 B (view/proj split out of view_proj + camera_pos —
   transmission needs the separate matrices); `ObjectUniform` 432 B (phys_a
   ior/thickness/attDist/dispersion, phys_b transmission/roughness/metalness/
   specularIntensity, phys_c clearcoat/roughness, attenuation_color);
   both guarded by the existing size asserts (the 432 first guess 416 was
   caught by the const-assert, §21.4 pattern).
4. **DFG LUT** (new `crates/opencat-three/src/dfg_lut.rs`): three.js
   `DFGLUTData.js` `DATA` (32×32 RG16F, 4096 samples/texel) copied **half
   word for half word** (python-extracted, 2048 u16s) and uploaded raw as an
   `Rg16Float` texture (binding 11, sampled with a ClampToEdge sampler —
   binding 10; DataTexture flipY=false row order matches wgpu upload order,
   no flip). `DFGApprox`/`EnvironmentBRDF` sample uv=(roughness, dotNV)
   exactly as lights_physical_pars_fragment.glsl.js:379-393. This is MIT
   rendering-stack data (same class as the Ashima noise port, §21.1-3), not
   a baked measurement.
5. **Shim** (`three_api.js`): `MeshPhysicalMaterial` — js:163-179 params in,
   `__physical` payload out (snake_case; `attenuationColor` via the Color
   hex path; **`attenuationDistance: Infinity` is emitted as sentinel 1e30**
   because serde_json rejects non-finite floats; WGSL treats ≥1e29 as no
   attenuation, matching volumeAttenuation's isinf branch);
   `collectScene` emits `shading: { Physical: {...} }`.
6. **Film XML** (`examples/liquid-brand-refraction.xml`, 3D section):
   `heroMaterial3d` is now `MeshPhysicalMaterial` with every constant cited
   from js:163-179 (color 0xffffff, transmission 1, opacity 1, metalness 0,
   roughness 0, ior 1.33, thickness 1.5, specularIntensity 2, clearcoat 1,
   clearcoatRoughness 0, dispersion 1, envMapIntensity 1.2 (inert until ⑥
   env — three.js without scene.environment has the same terms at 0),
   transparent true, attenuationColor 0x88ce7f, attenuationDistance 2) plus
   the same noiseDisplacement (the onBeforeCompile vertex section js:181-209
   is material-independent in our port). `#threecanvas` stays below
   `#bgcanvas` → metric-neutral by construction.
7. **§24.5-4 (sampler budget) resolved**: the transmission material consumed
   **zero** of the 8 video slots — the RT and the DFG LUT live on dedicated
   bindings 9/11 outside that budget.

### 25.2 Proof the segment really rasterizes (single-variable, in the shipped release binary)

Debug clip `out/_lbr-r5-3d-debug.xml` — film scene (same camera/lights/
hero-material constants/prism mosaic constants, dock pose), dark #222831
body, prism directly behind the hero so the hero's interior can only show
**RT-sampled refracted prism** (the direct prism is depth-culled in the main
pass; if the RT chain broke, the interior would be clear-color/black).
Rendered with `target/release/opencat` (also proves the shim + WGSL are
embedded in the shipped binary, §24.4 method). f0 frame:

- Interior shows dark refracted video patches with **cyan/magenta fringes**
  (corner colors split by dispersion) inside a green-tinted (attenuation
  0x88ce7f) displaced body with a bright fresnel rim — measured disk mean
  rgb (111.8, **153.4**, 100.7), G-dominant exactly as the attenuation
  color predicts.
- **ior 1.33 vs 1** (`_lbr-r5-3d-debug-ior1`): 95.3 % of disk pixels change
  >10, mean |Δ|/px = 64.9 — the refraction bend is the dominant interior
  signal, i.e. the sampled coords really come from `refract()`.
- **dispersion 1 vs 0** (`_lbr-r5-3d-debug-nodisp`): 14.7 % of disk pixels
  change >10, mean |Δ|/px = 5.8 — the per-channel ior split is live.
- **noiseTime = t unfrozen**: f0 vs f10 mean |Δ|/px = 87.0; centre-column
  samples move — the displaced surface re-refracts per frame.
- `cargo run -p opencat-three --example cube` (extended this round with a
  transmission smoke: Unlit red board behind a Physical cube): centre f0 =
  [255,111,74,255] — **exactly the §21.2 recorded value** (Lambert path
  unchanged); transmission cube = [131,0,0,255] (board red × Beer-Lambert
  transmittance 0.62 × (1−F)).

### 25.3 Measured result (whole film, 180 f, k3diff vs the SW-GL ref)

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r5-transmission.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r5-transmission.mp4 --out out/lbr-wf-r5-transmission-diff
```

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (r0, out/lbr-wf-baseline-diff) | 4.73897 | 0.09711 | 0.17376 | 0.07823 | 0.01973 | 0.01628 |
| run-2 round 1 = r4 (§24) | 4.74383 | 0.09713 | 0.17376 | 0.07822 | 0.01979 | 0.01624 |
| **run-2 round 2 = r5 (shipped)** | 4.74383 | **0.09713** | 0.17376 | 0.07822 | 0.01979 | 0.01624 |

avg_p8/mae are **bit-identical** to r4 (0.09713024316272291 /
4.743831379315272) and the whole-film mp4 is **md5-identical** to
`out/lbr-wf-r4-mosaic.mp4` (cb48573779f54a50fcfb989d0142f73e; f0
byte-identical checked separately): the film's 3D layer is still fully
occluded by `#bgcanvas`, so the transmission hero changes no visible pixel —
strongest-possible neutrality proof, and deterministic across binaries.
worst_p8 0.187115 @ f2, unchanged (diagnostic only). Historical best stays
**0.09711 @r0**; ledger kept = **true** (pipeline facility round; the ask
explicitly credits metric-neutral pipeline segments). Gates:
`cargo build --release` clean; `cargo test -p opencat-core` **684** passed /
0 failed (+1 = the Physical serde round-trip test); `cargo test -p
opencat-engine --lib` **96** passed / 0 failed.

### 25.4 Pitfalls / negative results (this round)

- **Same-texture render+sample in one pass is a wgpu hard error** ("conflicting
  usages … COLOR_TARGET vs RESOURCE"): the RT pass needs its own bind group
  with the dummy view at binding 9. First transmission render died on this.
- **serde_json cannot carry `Infinity`**: `attenuationDistance: Infinity`
  (three.js default) would serialize as `null` and fail decode; the shim
  emits 1e30 and the WGSL treats ≥1e29 as the isinf branch.
- **Uniform size miscount pattern repeats**: OBJECT_SIZE was first computed
  416 (forgot corner_colors is 3×16 on top of the 3 u32 arrays); the
  compile-time const-assert caught it before any GPU misread. Count mat4×2
  (128) + f32-vec4 ×10 (160) + u32-vec4 ×2 (32) + u32-vec4[2] ×3 (96) +
  f32-vec4[3] (48) = 432.
- **examples/cube.rs had been broken since r4** (Material3 gained
  map/textures and `render()` gained the textures argument; examples don't
  build in the gated commands so nobody noticed). Fixed + extended; if you
  touch Renderer3d::render's signature again, `cargo run -p opencat-three
  --example cube` is the cheapest WGSL-level smoke.
- **Dynamic indexing of `let` vectors in WGSL is unreliable across backends**
  — the dispersion loop uses a select-chain for `iors[i]` and select-reduction
  for the per-channel accumulation instead of `vec[i]` stores.
- **Known deviations from three.js, recorded deliberately** (all quality
  refinements, none structural): ① transmission RT sampled at lod 0 bilinear
  — the reference RT generates mipmaps and `getTransmissionSample` uses
  bicubic at lod = log2(W)·roughness·clamp(ior·2−2,0,1) ≈ 0.38 for the hero
  (mild blur term); ② RT without 4×MSAA and LDR (Rgba8Unorm vs HalfFloat);
  ③ env IBL terms (indirectSpecular, clearcoatSpecularIndirect) are 0 until
  ⑥ — identical to three.js when scene.environment is unset; ④ hero alpha
  blending: three.js renders transmissive objects in the transparent list
  with src-alpha blending, we still write with blend None (hero
  transmissionAlpha ≈ 1 → replace ≈ blend; matters for droplets at ⑤).
- **Promotion is still gated, now with a precise blocker** (§20.4 rule):
  moving `#threecanvas` above `#bgcanvas` would swap the measured-good 2D
  BGFIELD for the flat `bgPlane3d` stand-in (sure regression) AND strip the
  field out of the transmission RT (the hero would refract flat cream
  instead of the sage/teal field — the reference's WebGL scene contains its
  background plane, so a faithful hero-visible promotion requires the 3D
  background plane to carry the real field first). **The background
  ShaderMaterial port is therefore the prerequisite for both the composite
  promotion and an honest transmission RT** — it should precede or accompany
  the S1/S2-visible promotion, which is when the metric can first move
  (§24.5's expectation moves to that round).

### 25.5 Next-step suggestions (run-2 round 3 = threejs-like r6)

1. **Background ShaderMaterial port** (js:62-97): a generic scene3 material
   that lets the XML script declare the field (uniforms cream/sage/teal/
   mauve + gaussian field() + uFade) — options: (a) small fixed set of
   generic "gradient field" primitives, or (b) script-authored WGSL/SKSL
   blob with a validation seam. Then per §20.4: measure 3D plane vs 2D
   BGFIELD in-disk, promote `#threecanvas` above `#bgcanvas`, and the
   transmission hero + mosaic prism become visible at once (S1/S2 first
   movement; the §25.4 blocker clears).
2. **⑤ droplets 3D**: the 15 droplet meshes (js:412-434) share
   `makeLiquidMaterial` → same `MeshPhysicalMaterial` (already generic);
   needs alpha blending in the 3D pass (§25.4 deviation ④) since droplets
   overlap each other and the hero, plus per-mesh renderOrder semantics.
3. **⑥ ACES-like tonemap + env** (js:17-28 exposure 1.08, ACESFilmic;
   equirect garden env for envMapIntensity 1.2) — the lit-white hero
   currently clamps flat pre-ACES (§21.4-3), and the physical material's
   env terms wait on this.
4. Keep the whole-film render + `md5sum` vs the previous round's mp4 as the
   cheapest neutrality regression check (this round's trick — caught nothing
   because nothing leaked, which is the point).

## 26. Workflow round 3/12 (second run) = threejs-like route r6/12 (2026-10-07) — ShaderMaterial GLSL bridge connected end-to-end (bit-identical-neutral)

### 26.1 What was connected

Segment ④'s prerequisite from §25.5: the **background ShaderMaterial
(js:62-97) now runs as a real 3D shader** — script-authored GLSL, not a
field primitive. This unblocks the §25.4 promotion blocker (transmission RT
now sees the true field instead of flat cream) and adds the generic
`ShaderMaterial` surface the remaining custom shaders need. Chain, top to
bottom:

1. **IR** (`crates/opencat-core/src/ir/scene3.rs`): `ShaderValue3`
   (Float/Vec2/Vec3/Vec4), `ShaderUniform3 {name, value}`,
   `ShaderMaterial3 {vertex_shader, fragment_shader, uniforms}` serde
   round-trip test; new variant `Shading3::Shader` (Shading3 lost `Copy` —
   all match sites bind payloads).
2. **Engine** (`crates/opencat-three/src/lib.rs`): GLSL→naga→WGSL bridge —
   `compile_glsl` (Frontend glsl-in, `#version 450 core`, Validator,
   wgsl-out), `reflect_uniform_block` (std140 offsets/widths via naga IR,
   bails on group>1/binding≠0, *skips* other valid blocks), `ShaderProgram
   {pipeline, builtins_bgl, user_bgl, user_size, user_members}`,
   `shader_program()` compile cache with a hard assert on the canonical
   builtins table (projectionMatrix@0, modelViewMatrix@64, modelMatrix@128,
   viewMatrix@192, cameraPosition@256; 272B std140, set 0), `pack_user_uniforms`
   → set-1 buffer; per-draw branch in both render passes swaps
   pipeline+groups for shader draws. `write_uniform_values` is pure and
   test-shared (the earlier `std::mem::zeroed()` test FakeProgram was UB
   and is gone).
3. **Shim** (`crates/opencat-three/../script/runtime/three_api.js`):
   `adaptShaderMaterial()` = the WebGLProgram-prefix analog — strips
   #version/precision, wraps standalone uniforms into
   `layout(std140, set=1, binding=0) uniform UserUniforms {…}`, rewrites
   varying/attribute/texture2D/gl_FragColor to 450-core, assigns varying
   locations by vertex declaration order (errors on mismatch), injects the
   `ThreeBuiltins` set-0 block (`THREE_BUILTINS_GLSL`, anti-drift anchors
   vs the Rust canonical table). `ShaderMaterial` class keeps `uniforms`
   live for per-frame mutation; `shaderUniformPayload` packs {value}
   holders; `planeGeometryData` fixed to three.js verbatim
   (uvs [(0,1),(1,1),(0,0),(1,0)], indices [0,2,1,2,3,1] — was wrong
   before, which matters for every textured plane from here on).
4. **XML** (`examples/liquid-brand-refraction.xml`): `backgroundMaterial3d =
   new T3.ShaderMaterial({...})` with js:72-96 GLSL verbatim (cream/sage/
   teal/mauve + three-gaussian `field()` + vertical ramp + uFade), plane
   15×8.5 @ z=-3.5; per-frame `uFade` line cites js:527.

### 26.2 Proof chain (all executed this round)

- **Dialect level**: `shader_glsl_tests` in opencat-three (reference bg
  GLSL compiles both stages, canonical builtins asserted, uFade @0 span 4;
  packing writes at reflected offsets; arity mismatch rejected) +
  `three_like_shader_material_adapts_glsl_and_commits` in opencat-engine
  (450-core header, location-ized varyings, out decl before body, per-frame
  Float payload through the IR).
- **GPU level**: `cargo run -p opencat-three --example cube` shader smoke —
  full-screen quad, exact B channel 64→32 under uFade 1.0→0.5, uv
  directionality asserted (R→right, G→up) at 25%/75% sample points (corner
  pixels are unreliable: angled camera + extreme corner rays).
- **In-disk §20.4 gate (PASSED for the background segment)**: pure
  background ring (72 of 144 grid cells, cols 0-3 + 12-15) of the 3D
  shader field vs reference = mean |Δ|sum 3.47/cell vs 2D BGFIELD's 3.48;
  at all 8 sampled points the 3D field is byte-identical to the 2D
  BGFIELD. The center cells (~200 off) are hero/prism = segments ⑤⑥.
- **Whole film**: see 26.4.

### 26.3 Negative results / pitfalls (read before touching this code)

- **naga 30 GLSL facts**: `layout(binding=X)` REQUIRED on uniform blocks
  (set qualifier OK); standalone uniforms must be wrapped into a block by
  the shim; `precision highp float;` + `#version 450 core` accepted;
  **struct span excludes std140 tail padding** (single-float block reports
  span 4 → round user_size up to 16 yourself); `TypeInner::Scalar(s)`
  compares with `s == &naga::Scalar::F32` (reference pattern).
- **Fragment out declaration must precede the body** — assembling the out
  decl after the GLSL body gives `UnknownVariable("fragOutput")` at first
  real render (three.js rewrites gl_FragColor to a declared out too).
- **`reflect_uniform_block` must skip, not bail**, on blocks that aren't
  the requested (group,binding) — the same fragment module legitimately
  carries set-0 builtins AND set-1 UserUniforms.
- **ShaderMaterial + noiseDisplacement is rejected by the shim** (would
  need to inject snoise into arbitrary user GLSL — three.js has no such
  feature either; it's our shader-chunk convention). If a future segment
  needs it, inject explicitly in that shader's source.
- **`toneMapped:false` is inert for ShaderMaterial** — three.js only
  injects tonemapping/colorspace chunks into built-in materials; user GLSL
  output is raw in the reference too, so this is actually 1:1. Same for
  `depthWrite:false/depthTest:false` + renderOrder −10: implemented as
  scene order (bg plane drawn first, depth-far) — recorded deviation.
- **gl_FragCoord in user GLSL would be y-flipped** vs WebGL (wgpu
  framebuffer origin); the reference bg shader doesn't use it (vUv-based),
  noted for future shader ports.
- **The double-field S4 regression** (the round's real trap): with the 3D
  field live UNDER the 2D BGFIELD canvas, the fade window (5.4-6.0s)
  composites BOTH layers semi-transparent → total field weight
  0.583·0.583 + (1−0.583)·0.583·0.583 ≈ 0.48 vs the reference's single
  canvas at 0.34 (measured f170: r7=(213,226,214) = double-field model
  exactly; ref=(220,230,219) ≈ 0.34; r5=(222,230,220) matched only because
  its 3D layer was flat cream). k3diff S4 went 0.01624 → 0.02953 (r7
  unpinned). A stuck-uniform hypothesis was DISPROVED first (fade-probe
  clip `out/_lbr-r7-3d-debug-fade.mp4` reaches pure cream at fade≈0.98 —
  the uniform pipeline animates end-to-end).
- **Promotion transition rule (the fix, keep for the promotion round)**:
  while `#threecanvas` sits below `#bgcanvas`, the visible field must come
  from ONE layer. The XML pins `BG_3D_PROMOTED = false` → 3D uFade 0 →
  shader renders neutral cream mix(cream,color,0) = #f4f2ea, which is
  **byte-identical** to the r0-r5 Unlit placeholder (whole-film md5
  returned to r5's cb48573779f54a50fcfb989d0142f73e exactly). The
  promotion commit flips `BG_3D_PROMOTED = true` (restoring js:527
  `1-fade`) in the same change that retires `#bgcanvas` — never flip one
  without the other.

### 26.4 Measurement & ledger — kept = **true**

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r7-bgshader-pin.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r7-bgshader-pin.mp4 --out out/lbr-wf-r7-pin-diff
```

Per-segment p8 = mean of `pixel.csv` p8 over the §19 phase windows
(`S1 0–83`, `S2 84–101`, `S3 102–161`, `S4 162–179`):

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (workflow r0, out/lbr-wf-baseline-diff) | 4.73897 | **0.09711** | 0.1738 | 0.0782 | 0.0197 | 0.0163 |
| run-2 r1 = r2 (out/lbr-wf-r2-diff) | 4.74383 | 0.09713024 | 0.17376 | 0.07822 | 0.01979 | 0.01624 |
| r7 unpinned 3D field live (out/lbr-wf-r7-bgshader-diff, NOT kept) | 4.86324 | 0.09845885 | 0.17376 | 0.07822 | 0.01979 | 0.02953 |
| **workflow round 3 (this round, pinned)** | 4.74383 | **0.09713024** | 0.17376 | 0.07822 | 0.01979 | 0.01624 |

worst_p8 0.187115 @ f2 (diagnostic only, never a gate; equals the r2
value). `out/lbr-wf-r7-bgshader-pin.mp4` md5 =
cb48573779f54a50fcfb989d0142f73e = r5's mp4, bit-identical — the pinned
shader path reproduces the placeholder byte-for-byte. Historical best
stays **0.09711 @r0**; this is the third consecutive round at the r1/r2
optimum. Gates: `cargo build --release` clean; `cargo test -p
opencat-core` 685 passed / 0 failed; `cargo test -p opencat-engine --lib`
97 / 0; `cargo test -p opencat-three --lib` 3 / 0.

### 26.5 Next-step suggestions (run-2 round 4 = threejs-like r7)

1. **⑤ droplets 3D** (js:412-434, 15 meshes sharing makeLiquidMaterial):
   needs alpha blending in the 3D pass (§25.4 deviation ④ — three.js
   renders transmissive objects in the transparent list, src-alpha) and
   per-mesh renderOrder. Hero already carries MeshPhysicalMaterial;
   droplets are the same material path — likely the biggest S2/S3 mover.
2. **⑥ ACES-like tonemap + env** (js:17-28 exposure 1.08 ACESFilmic,
   equirect garden env for envMapIntensity 1.2) — lights the lit-white
   hero path and the physical material's env terms.
3. **The promotion round** (when hero/prism are at parity): one commit —
   flip `BG_3D_PROMOTED=true`, retire `#bgcanvas` (and with it the 2D
   field's double-fade hazard), move `#threecanvas` above; re-run the
   §20.4 in-disk gate per segment. The transmission RT is already
   field-honest as of this round.
4. Keep the md5-neutrality check ritual (§25.5-4) — this round it earned
   its keep: it converted "S4 regressed, why" into a 3-line diff.

## 27. Workflow round 4/12 (second run) = threejs-like route r7/12 (2026-10-07) — segment ⑤ droplets 3D + wordmark plane connected (bit-identical-neutral via §26.3-style pin)

### 27.1 What was connected (generic pipeline; zero liquid-specific engine code)

Segment ⑤ of the dependency order: the 15-droplet swarm (js:412-434,
js:510-522) and the wordmark carrier plane (js:377-410, js:492-494) now
exist as real threejs-like meshes end-to-end: shim → Scene3 IR → list
build → draw. Because droplets need transparent-list classification +
src-alpha blending and per-mesh renderOrder (§25.4 deviation ④), this
round landed the missing renderer semantics, all generic:

- **IR**: `Mesh3.render_order: i32` (three.js Object3D.renderOrder,
  painterSortStable/reversePainterSortStable primary key) and
  `Material3.transparent: bool` (WebGLRenderLists.js push()
  classification: transmission>0 → transmissive, transparent →
  transparent, else opaque). Serde defaults keep legacy JSON decoding;
  round-trip + back-compat test in scene3.rs.
- **Render lists** (opencat-three render()): every mesh classified into
  `ListKind::{Opaque, Transmissive, Transparent}`; sort = renderOrder
  asc, then opaque z-near-first / transparent+transmissive z-far-first
  (painterSortStable / reversePainterSortStable; three.js renders
  opaque → transmissive → transparent). The transmission RT pass
  iterates the same sorted order skipping non-opaque (three.js
  renderTransmissionPass renders the opaque list only; the DoubleSide
  transmissive backside re-render is N/A here — liquid materials are
  FrontSide).
- **src-alpha blending**: `pipeline_blended` (wgpu
  BlendState::ALPHA_BLENDING = SrcAlpha/OneMinusSrcAlpha, exactly
  three.js NormalBlending + transparent:true); `transparent:false` keeps
  NoBlending. Meshes route per list; ShaderMaterial pipelines get a
  blended twin selected by the material's `transparent`.
- **Unlit map alpha fidelity**: unlit fs now multiplies the full texel
  vec4 (`base.a * albedo.a`), matching three.js map_fragment — needed
  for any future unlit+map transparent draw.
- **12 sampled-texture slots**: MAX_TEXTURE_SLOTS 8→12 with a
  slot→binding gap map (slots 0-7 → bindings 0-7, slots 8-11 → bindings
  12-15; bindings 8-11 reserved: Repeat sampler 8, transmission RT 9,
  clamp sampler 10, DFG LUT 11). Budget check: 12 script + 2 mosaic +
  1 wordmark = 15 ≤ 16.
- **ShaderMaterial map seam (naga 30 GLSL-in)**: naga has NO combined
  `sampler2D` type (NotImplemented("variable qualifier") on every
  combined form — probed 3 ways). Split Vulkan form works:
  `layout(set = 3, binding = 0) uniform texture2D map;` +
  `layout(set = 3, binding = 1) uniform sampler map_smp;`, sampled via
  `texture(sampler2D(map, map_smp), uv)` — lands exactly at set 3
  bindings 0/1. three_api.js `adaptShaderMaterial(vertex, fragment,
  hasMap)` injects exactly those decls; a permanent naga-IR test pins
  the seam (`shader_map_seam_splits_texture_and_sampler_at_set3`).
- **shim**: Object3D.renderOrder, Vector3.addScaledVector (js:519) +
  normalize (js:422), material `transparent` collection, BitmapTexture
  (TextureLoader counterpart, `{ Bitmap: { path } }` snapshot payload),
  collectScene emits `render_order`/`transparent`.

### 27.2 XML declaration (structural-1to1, all constants from js)

- **wordmark plane** (after brandMark, matching js:377-410 scene order):
  PlaneGeometry(WM_PLANE_H·WM_ASPECT, WM_PLANE_H) — WM_ASPECT 3356/1000
  and WM_PLANE_H 0.55 shared with the 2D glue vars (js:382); position
  (0.35, 0.14, -0.35) (js:407 isHeygenWebgl), renderOrder 2 (js:408).
  Material = ShaderMaterial whose GLSL is the explicit expansion of the
  reference's MeshBasicMaterial + onBeforeCompile compiled output
  (three.js injects script GLSL; our generic counterpart is the
  ShaderMaterial bridge): vertex = vMapUv + MVP; fragment = map_fragment
  vec4 multiply → js:395 wipe discard (injected right after
  map_fragment per js:393-396) → alphatest_fragment 0.01 (js:386) →
  opaque output. map = BitmapTexture('../assets/lbr/wordmark.png')
  (js:385; the reference itself rasterizes the SVG via TextureLoader,
  js:42-45/57-60 — pre-rasterized PNG is the same mechanism, not baked
  pixels). Per-frame: uWordmarkReveal = reveal (js:493, shared 2D
  intermediate, line 466), visible = t>=3.0 (js:494).
- **droplets**: shared dropletMaterial3d = MeshPhysicalMaterial with
  js:163-179 constants (same constructor as hero, separate instance —
  js:216) + noiseDisplacement js:181-209; 15 × SphereGeometry(0.1+0.025·
  (i%5), 32, 32) (js:414-415), goldenAngle degToRad(137.508) spiral
  (js:413-420), userData {base, direction, delay (i%4)/30, travel
  1.65+0.18·(i%4)} (js:425-430), renderOrder 3 (js:431). Per-frame loop
  1:1 with js:510-522: pre-pop base + establish-scale lerp(0.62,1,…)
  (js:513-514); post-pop power3Out launch · travel · (1−0.04·settle)
  along direction, y −= 0.3·launch² + 0.1·settle (settle = same
  expression as the 2D `dropSettle`, line 508 = js:518), scale
  lerp(1,0.76,launch) (js:517-521); dropletMaterial3d.noiseTime =
  heroTime (js:526, same liquidClock value as hero).

### 27.3 Pitfalls hit this round (all reproducible)

- **Forgetting `map:` on the ShaderMaterial** → naga
  UnknownVariable("map") at render time (hasMap false → no set-3 decls
  injected while the GLSL references them). The map property must carry
  the BitmapTexture for the seam to be injected.
- **wgpu "Conflicting binding at index 12"**: after raising
  MAX_TEXTURE_SLOTS to 12, the shared sampler entry written as
  `binding: MAX_TEXTURE_SLOTS` collided with slot 8's texture (binding
  12). Sampler entries are the literal 8 in both layout and bind group.
- **naga combined-sampler rejection** (3 probe forms all fail with
  NotImplemented("variable qualifier"); span points at the `sampler2D`
  token) — see 27.1 for the working split form.
- **blend-off assertion semantics**: with blending off, wgpu writes raw
  src alpha (0.5 → 127); three.js canvas is alpha:false so only rgb
  purity is the invariant — assert rgb, not alpha, in the off case.
- **Frustum-exact uv mapping in GPU smokes**: a tilted camera +
  oversized plane samples uv 0.33-0.67 (Linear between texel centers);
  straight-on camera with vis_h = 2·d·tan(fov/2), vis_w = vis_h·aspect
  makes screen 25%/75% hit exact texel centers → pure colors.
- **S4 double-draw generalizes to ANY new #threecanvas content**
  (this round's real trap, §26.3 rule re-earned): with `#threecanvas`
  still below `#bgcanvas`, S1-S3 are occluded (bgcanvas opaque) but the
  fade window (f162-179) composites every layer semi-transparently —
  the unpinned run bled 3D droplets+wordmark under the 2D glue sprites:
  per-frame p8 +0.019 @f176, f168-177 all regressed, avg_p8
  0.09713 → 0.09818, mae 4.744 → 4.764, md5 broke. Fix = the §26.3
  promotion-transition pattern applied to the new segment:
  `DROPS_WM_3D_PROMOTED = false` gates `wordmarkPlane3d.visible`
  (js:494 conjunction) and `dcMesh.visible` while meshes, per-frame
  animation and transmission-RT participation stay live. Flip it in the
  promotion commit together with `BG_3D_PROMOTED` and `#bgcanvas`
  retirement — never one without the others.
- **`side: DoubleSide` (js:389) not declared** on the wordmark
  ShaderMaterial: the plane always faces the camera (+z toward view),
  so engine FrontSide is per-pixel equivalent; recorded as comment, not
  code.

### 27.4 Proof chain (the segment really rasterizes)

- Permanent tests: scene3 render_order/transparent round-trip +
  legacy-decode; opencat-three `shader_map_seam_splits_texture_and_sampler_at_set3`
  (naga IR bindings == [(3,0),(3,1)]).
- `cargo run --release -p opencat-three --example cube` smokes (GPU):
  blend on = src-alpha exact [128,127,0,255] at overlap; blend off = rgb
  purity; ShaderMaterial+BitmapTexture map full = pure left red / right
  green; wipe uReveal 0.5 discards right half to [0,0,0,0].
- Debug clip `out/_lbr-r7-debug.mp4` (2D overlays suppressed via
  opacity(0), from `out/_lbr-r7-debug.xml`): f1.5 shows hero + 15
  noise-displaced green attenuation droplets alpha-composited around it
  with facet refraction through the hero; f3.2 shows the prism mosaic
  with the wordmark plane mid-wipe ("ey…" = uWordmarkReveal 0.5 discard)
  behind it and popped droplets in flight.

### 27.5 Measurement & ledger — kept = **true**

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r7-droplets-wordmark.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r7-droplets-wordmark.mp4 --out out/lbr-wf-r7-dw-diff
```

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (workflow r0) | 4.73897 | 0.09711 | 0.1738 | 0.0782 | 0.0197 | 0.0163 |
| workflow r3 pin (§26) | 4.74383 | 0.09713024 | 0.17376 | 0.07822 | 0.01979 | 0.01624 |
| r7 UNPINNED droplets+wordmark live (NOT kept) | 4.76389 | 0.09817640 | 0.17376 | 0.07822 | 0.01979 | 0.0359 avg f162-179 |
| **workflow round 4 (this round, pinned)** | 4.74383 | **0.09713024** | 0.17376 | 0.07822 | 0.01979 | 0.01624 |

worst_p8 0.187115 @ f2 (diagnostic only, never a gate;
= 0.1871146797839506, the r2 value).

`out/lbr-wf-r7-droplets-wordmark.mp4` md5 =
cb48573779f54a50fcfb989d0142f73e = r5/r7-pin's mp4, bit-identical — the
pinned segment contributes zero pixels end-to-end while the 2D glue is
promoted. Historical best stays **0.09711 @r0**; fourth consecutive
round at the r1/r2 optimum. Gates: `cargo build --release` clean;
`cargo test -p opencat-core` 686 passed / 0 failed;
`cargo test -p opencat-engine --lib` 97 passed / 0 failed;
`cargo test -p opencat-three --lib` 4 passed / 0 failed.

### 27.6 Next-step suggestions (workflow round 5/12 = threejs-like r8/12)

1. **⑥ ACES-like tonemap + env IBL** (js:17-28: exposure 1.08,
   ACESFilmicToneMapping; equirect garden env at envMapIntensity 1.2) —
   the last lighting piece for the lit hero/droplet physical path;
   DFG LUT slot 11 and `pipeline` plumbing already reserved.
2. **Promotion readiness check**: with ⑤ in place, the promotion round
   owns three same-commit flips: `BG_3D_PROMOTED=true`,
   `DROPS_WM_3D_PROMOTED=true`, `#bgcanvas` retirement + `#threecanvas`
   z-order above; then re-run §20.4 in-disk gates per segment and swap
   the 2D glue (BGFIELD, DROPS lambda, #wordmark clip-path, prism-crop)
   out.
3. droplet look parity (only if promotion diff shows a gap): droplets
   currently share the hero's noise-displacement constants via
   js:181-209 — verify per-mesh `uTime` phase behavior matches the
   reference (js:526 shares one clock; no per-droplet phase in source).
4. Keep the md5-neutrality ritual — second round in a row it caught the
   regression window (f162-179) before any ledger damage.

### 27.7 Review fixes (r4 findings, same round)

Four reviewer findings on the §27 changes; fixes applied and re-measured:

1. **Missing reference renderOrder declarations** — the reference sets
   renderOrder on three more objects: background −10 (js:100), prismMesh
   1 (js:340), hero 4 (js:219). With render_order now the primary sort
   key, the XML defaults left hero at 0 → hero would sort BEFORE
   droplets(3) in the transmissive segment — opposite of the reference
   draw order (droplets → hero), a visible blend-order deviation the
   moment `DROPS_WM_3D_PROMOTED` flips (masked today by the pin +
   `#bgcanvas`). Fixed: `bgPlane3d.renderOrder = -10` (js:100),
   `prismMesh3d.renderOrder = 1` (js:340), `heroSphere.renderOrder = 4`
   (js:219) declared beside their js counterparts; full transmissive
   chain is now droplets(3) → hero(4), opaque chain bg(−10) → prism(1)
   → wordmark(2).
2. **bg equivalence comment contradicted the new sorter** — XML:553-556
   still claimed "scene order first mesh + depth-farthest" as the
   renderOrder −10 equivalent, a premise this round's own sorter
   removed. Rewritten: renderOrder −10 is now a real engine sort key
   (opaque segment draws bg first); the remaining deviation is only the
   per-material depth state (js:63-65), whose per-pixel equivalence
   rests on bg being depth-farthest (z=-3.5) so the depth test lets
   nearer draws cover it regardless of order.
3. **lib.rs comment claimed three-list pass order, implementation was a
   single interleaved sort** — fixed in the implementation, not the
   comment: after the per-list-key sort, a stable partition now
   concatenates Opaque → Transmissive → Transparent segments
   (`draw_order`), each keeping its painter/reverse-painter internal
   order — exactly three.js renderScene's three renderObjects calls
   (WebGLRenderer.js:1849-1851), unconditionally, with no
   renderOrder-partition premise. RT pass iterates the Opaque prefix
   (take_while), main pass iterates the full concatenation. The
   interleaved-sort premise is recorded in the new comment.
4. **wordmark `side: DoubleSide` (js:389)** — passed as recorded
   (plane always faces the camera; per-pixel equivalent); flagged for
   re-check if a camera roll is ever introduced. **wordmark.png
   provenance** — reviewed as potential baked pixels: it is a
   gradient-free rasterization of the official SVG
   (`assets/lbr/wordmark.svg`, root width=3356 height=1000; ink
   rgb≈[0,0,0] matches the SW-GL baseline f135/f165 = 0 colored pixels
   in the wordmark region). Legitimate TextureLoader counterpart, not
   baked pixels; re-rasterize if the baseline ever becomes a real
   browser render.

Re-verification after fixes: `cargo build --release` clean;
`cargo test -p opencat-three --lib` 4/0; `cargo test -p opencat-core`
686/0; `cargo run --release -p opencat-three --example cube` blend+map
smokes unchanged (on=[128,127,0,255], wipe r=[0,0,0,0]);
`out/lbr-wf-r7-droplets-wordmark.mp4` md5 =
cb48573779f54a50fcfb989d0142f73e (bit-identical to the §27.5 ledger —
the renderOrder declarations and three-segment order are pixel-neutral
for the pinned composite, as the opaque+depth argument predicts);
k3diff avg_p8 = 0.09713024316272291, mae 4.743831379315272 (unchanged).

## 28. Workflow round 5/12 (second run) = threejs-like route r8/12 (2026-10-07) — segment ⑥a 输出变换接通（ACESFilmic × exposure + sRGB OETF；位同中性 pin）

### 28.1 What was connected (generic renderer semantics; zero liquid-specific code)

⑥ 的前半（tonemap + 输出色彩空间）；env IBL（scene.environment + PMREM）留 ⑥ 后半。
three.js r181 语义逐条核对（本仓库 vendored 源
`liquid-brand-refraction/node_modules/three/src/`）：

- **主 pass 内建材质**：tonemapping_fragment（rgb）→ colorspace_fragment
  （rgb 变换 alpha 原样）只注入内建 program（WebGLProgram.js:831-839）；
  `material.toneMapped` 只门 TONE_MAPPING define。ShaderMaterial（bg/prism）
  恒 raw。参考 wordmark 是 MeshBasicMaterial + `toneMapped:false`（js:388）
  → 无 ACES，但 colorspace chunk 恒注入（meshbasic.glsl.js:109-110）。
- **透射 RT 二遍恒线性**：WebGLRenderer.js:1921-1922（透射 pass 强制
  NoToneMapping）+ :2197（RT 输出 LinearSRGBColorSpace）。
- **实现**（opencat-three/lib.rs）：`GlobalsUniform.output_xform`（x =
  tonemap 开关 / y = exposure / z = sRGB 开关）+ WGSL `rrt_and_odt_fit` /
  `aces_filmic` / `srgb_oetf` / `output_xform`（chunk 逐字移植，mat3 列构造
  直译）；fs 的 Unlit/Lambert/Physical 分支套 `output_xform`，VideoMosaic
  分支（= 参考 prismMaterial ShaderMaterial）显式不套。**透射 RT pass 绑
  第二份 globals（`globals_buf_rt`，output_xform 全 0）**——独立 buffer 免
  两 pass 间的缓冲重写排序问题。
- **IR**（scene3.rs）：`Scene3.tone_mapping: ToneMapping3`（None/ACESFilmic）
  + `tone_mapping_exposure: f32`（default 1.0）+ `output_color_space:
  OutputColorSpace3`（Linear/Srgb）。serde default = 旧快照解码不变（记录
  偏差：three.js r152+ 默认 outputColorSpace=SRGB，本引擎既有 raw 契约 →
  默认 Linear，参考场景经 js:18 显式声明后行为一致）。
- **shim**（three_api.js）：T3 常量 NoToneMapping/'ACESFilmic'/
  LinearSRGBColorSpace/SRGBColorSpace（值 = IR 变体名，快照直通；three.js
  数字值 NoToneMapping=0/ACESFilmic=4 仅拼写差异）；Renderer 构造器挂
  outputColorSpace/toneMapping/toneMappingExposure 属性（默认
  NoToneMapping/1.0/Linear），render() 随快照提交。
- **XML**：`renderer3d.outputColorSpace = T3.SRGBColorSpace`（js:18）+
  `toneMapping = T3.ACESFilmicToneMapping`（js:19）+ `toneMappingExposure =
  1.08`（js:20）。wordmark ShaderMaterial（= 参考 MeshBasicMaterial 编译
  产物）的 GLSL 补上恒注入的 colorspace_fragment：sRGBTransferOETF 逐字 +
  `gl_FragColor = sRGBTransferOETF(gl_FragColor)`（toneMapped:false → 无
  ACES 项）；黑墨水下数值恒等，语义完整性由 naga 编译锚测试锁定。

### 28.2 Pitfalls hit this round (reproducible)

- **WGSL select(f, t, cond) 参序**（本轮真雷）：sRGB OETF 初版写
  `select(lo, hi, v <= 0.0031308)`——select 条件真取**第二**参，于是大值
  走 12.92·v（0.2 → 2.58 → 钳 255 全白）。正确：`select(hi, lo,
  v <= 0.0031308)`。GPU 冒烟三档诊断法（exposure 0 / 开 ACES / 开 sRGB）
  一次定位：ACES 档逐位对 CPU 镜像、sRGB 档全白 → 缺陷只在 sRGB 分支。
- **不可见面片的编译盲区**：wordmark 恒 `visible=false`（js:494 + 提升前
  pin）→ collectScene 剪枝 → 整片渲染永不触发其 GLSL 的 naga 编译——
  md5 中性不能证明它编译。补了常驻 naga-in 测试
  `wordmark_colorspace_chunk_compiles_through_naga`（mix 的
  `vec3(lessThanEqual(...))` bool 选择子过 naga glsl-in；GLSL 与 XML 逐字
  同步，防漂移锚）。
- **CPU 镜像容差**：wgpu Rgba8Unorm 写出 + GPU 融合乘加 → 断言用 ±1 容差；
  实测 lavapipe 上 6a 冒烟与 f32 CPU 镜像 0 偏差（[155,189,220] 逐位）。

### 28.3 Proof chain (all executed this round)

- 常驻测试：`tone_mapping_settings_round_trip_and_backcompat`（scene3.rs，
  旧快照 default = NoToneMapping/1.0/Linear）+
  `wordmark_colorspace_chunk_compiles_through_naga`（lib.rs）。
- GPU 冒烟（`cargo run --release -p opencat-three --example cube`，全部过）：
  xform 关 = raw [51,102,204,255]（默认逐字节不变）；开 = [155,189,220,255]
  = CPU 镜像逐位；透射场景开 6a 后中心 [210,0,7]（≠ 关 [131,0,0]，且 R>40
  —— RT 二遍恒线性 + 主 pass 变换双证）。
- **整片 md5 中性**：`out/lbr-wf-r8-tonemap.mp4` md5 =
  cb48573779f54a50fcfb989d0142f73e = r5/r7-pin 逐位同——6a 在被 #bgcanvas
  遮挡的 3D 画布里贡献零像素（§26.3/§27.3 提升过渡规则第三轮生效）。

### 28.4 Measurement & ledger — kept = **true**（第五轮持平 r1/r2 最优）

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r8-tonemap.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r8-tonemap.mp4 --out out/lbr-wf-r8-tonemap-diff
```

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (workflow r0) | 4.73897 | 0.09711 | 0.1738 | 0.0782 | 0.0197 | 0.0163 |
| workflow r1–r4（pin 态） | 4.74383 | 0.09713024 | 0.17376 | 0.07822 | 0.01979 | 0.01624 |
| **workflow round 5（本轮，6a 接通仍中性）** | 4.743831 | **0.09713024316272291** | 同上 | 同上 | 同上 | 同上 |

worst_p8 0.187114680 @f2（诊断量，= r2 值）。历史最优保持 0.09711 @r0，
本轮为连续第五轮处于 r1/r2 最优。Gates：`cargo build --release` clean；
`cargo test -p opencat-core` 687/0；`cargo test -p opencat-engine --lib`
97/0；`cargo test -p opencat-three --lib` 5/0；cube GPU 冒烟全过（含新
6a 三档断言）。

### 28.5 Next-step suggestions (workflow round 6/12 = threejs-like r9/12)

1. **⑥ 后半：env IBL**（js:23-24/47-51 equirect-garden.jpg 6144×3072
   EquirectangularReflectionMapping + SRGBColorSpace；js:163-179
   envMapIntensity 1.2）——需要：JPEG 解码（新依赖或现成 crate）、PMREM
   预滤波（three.js cubeUV 布局 + `_applyPMREM` 卷积的对应物）、
   fs_physical 的 getIBLIrradiance/getIBLRadiance + clearcoat 间接项 +
   envMapIntensity。管线侧 DFG LUT（slot 11）与
   `environment_brdf` 已在位。这是提升轮前最后一段 3D 管线。
2. **提升就绪清单不变**（§27.6-2）：BG_3D_PROMOTED + DROPS_WM_3D_PROMOTED
   + #bgcanvas 退役同一提交；6a 现已 live（遮挡下中性），提升后 hero/
   droplets 走 ACES×1.08→sRGB 真输出链。
3. 保持 md5 中性仪式（第三轮连续抓回归窗口价值证明）。

## 29. Workflow round 6/12 (second run) = threejs-like route r9/12 (2026-10-07) — segment ⑥b env IBL 接通（位同中性 pin）

### 29.1 What was connected

`scene.environment` 全链路（three.js r181 `PMREMGenerator.fromEquirectangular`
的 CPU 同构 + cubeUV 采样 IBL）：

- **IR**（`crates/opencat-core/src/ir/scene3.rs`）：`Scene3.environment:
  Option<Texture3>`（尾字段，serde skip_serializing_if=None）+
  `Physical3.env_map_intensity: f32`（`default_env_map_intensity() = 1.0`，
  three.js 材质默认同值）。round-trip + 旧快照（剥字段）→ None/1.0 测试
  `environment_and_env_map_intensity_round_trip_and_backcompat`。
- **shim**（`three_api.js`）：`Scene.environment`（null 初值）；
  `EquirectangularReflectionMapping/UVMapping/NoColorSpace` 常量；
  `MeshPhysicalMaterial.envMapIntensity → env_map_intensity`（缺省 1）；
  `Renderer.render` fail-loud 校验（BitmapTexture + EquirectangularReflectionMapping
  + SRGBColorSpace，否则 throw）→ `snapshot.environment = texturePayload(...)`。
- **CPU PMREM**（新 `crates/opencat-three/src/pmrem.rs`，~770 行）：
  `_setSize(W/4)`→lodMax→`_createPlanes` sizeLods（LOD_MIN=4 + 6 extra）→
  级 0 equirect 六面（#25071 过采样 + `equirectUv` + 源 sRGB EOTF 线性化
  + 常量源-LOD trilinear，记录近似）→ `_applyGGXFilter`（GGX VNDF
  Heitz 2018、256 Hammersley、增量粗糙度 sqrt(t²−s²)·(0.05+0.95t)、
  NdotL 加权）；ping-pong 回拷 ≡ 恒等重采样（CPU 等价跳过，注释记录）。
  6 个单测（布局/roughnessToMip 手推值/纯色不变/棋盘趋均值/bilinear 不变式
  /f16 RNE）。GLB 铁律 6 无需动——scene3_glb.rs 已是 gltf crate。
- **WGSL**（`lib.rs`）：`cube_uv_get_face/get_uv/bilinear`（cubeUV_minMipLevel
  4.0/minTileSize 16.0、#25071、extra tile x=filterInt·3·16、y=4·(2^MAX_MIP−faceSize)）、
  `roughness_to_mip`（cubeUV_r0..r6 逐字）、`texture_cube_uv`（clamp(−2,MAX_MIP)
  + mipF 双采样 mix）、`get_ibl_irradiance`（π·cubeUV(n,1)·intensity）、
  `get_ibl_radiance`（reflect + pow4(roughness) 收拢）；fs_physical 注入
  lights_fragment_maps/end → `RE_IndirectSpecular_Physical`（clearcoatSpecularIndirect
  + computeMultiscattering + cosineWeightedIrradiance = iblIrr·1/π）+
  meshphysical 尾部 `(direct+indirect)·clearcoat`。GLSL view→world 的
  inverseTransformDirection 与世界空间 reflect/mix 可交换，注释记录等价性。
- **渲染器**：globals `env_cubuv`（texelW/texelH/maxMip/has_env，
  GLOBALS_SIZE 352→368）；object `phys_d.x = envMapIntensity`（OBJECT_SIZE
  432→448）；group2 binding 16 Rgba16Float env 纹理（无 env 绑 dummy，
  `USE_ENVMAP` 未定义语义 = getIBL* 归零 ⇒ 既有输出逐字节不变）；
  `Renderer3d::set_environment`（wgpu 256B 行对齐上传 + generateCubeUVSize
  公式）+ Arc-ptr PMREM 缓存（MediaContext 图像缓存同指针 ⇒ 一次性）；
  `LazyRenderer3d::set_environment`；engine `pipeline.rs` env 来源走同一
  texture worker 解码。env 从不是渲染目标 ⇒ RT/主两 pass 同视图。
- **XML**：js:37/43 BitmapTexture(equirect) → js:48 mapping → js:49
  colorSpace → js:50 scene.environment；两处 envMapIntensity 1.2（js:175）
  注释更新为已接通。资产 `assets/lbr/equirect-garden.jpg`（6144×3072，
  自参考项目拷贝）。

### 29.2 本轮抓到的四个实现级 bug（测试/冒烟价值记录）

1. **f16 次正规编码移位错**：`s = (−exp−14)` → 正确 `s = (−exp−1)`
   （m' = full·2^(exp+1)）；近零次正规的正确舍入判据是半 ulp 而非相对
   误差（6e-8→0x0001 相对误差 0.66% 是 RNE 正确行为）。
2. **equirectUv 旧版公式**：pmrem 首版写了 `uv=(u,1−v)`——那是旧
   three.js；**vendored r181 common.glsl.js:82-92 直返 (u,v)**，v=1=天顶
   → flipY 上传后纹理 v=1 = 图像顶行 ⇒ 源位图按自然图像行序读取。
   （纯色/棋盘单测对此不敏感——方向敏感内容才会暴露。）
3. **totalLods u32 下溢**：lodMax < LOD_MIN 的退化小图（w<64）使
   `lod_max − LOD_MIN` 回绕 → 4.29e9 次 with_capacity → 17GB 挂死。
   JS 是浮点算术（totalLods = lodMax+3 ≥ 3）。改 i32。
4. **Rgba16Float 行距**：上传 bytes_per_row 按 ×8/px（半精度 4 通道），
   非 ×4；wgpu 需 256B 行对齐（GL 无此约束），真实 env 宽 3·cubeSize·8
   恰为 256 倍数，补零仅为退化路径。

另：GPU 冒烟曾「IBL 输出不对」——实为冒烟场景 base_color=(0.9,0.3,0.2)
未进解析期望（133 = 0.5776·0.9·255 逐位吻合）。**白漫反射 + 纯色 env + 无
灯光 + specularIntensity=0/clearcoat=0 ⇒ 输出 = E·envMapIntensity 逐通道
精确**（radiance/multiscatter/clearcoat 项全为 0，只打 irradiance→
indirectDiffuse 链）——已固化为 cube.rs IBL 解析冒烟（1.0/0.7 两档 +
env-off 归零断言）。

### 29.3 Gates（全部本轮实跑）

```
cargo build --release                                    # clean（26.9s）
cargo test -p opencat-core                               # 688/0
cargo test -p opencat-engine --lib                       # 97/0
cargo test -p opencat-three --lib                        # 11/0
cargo run --release -p opencat-three --example cube      # 既有断言 + IBL 解析冒烟全过
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r6-env.mp4
md5sum out/lbr-wf-r6-env.mp4                             # cb48573779f54a50fcfb989d0142f73e = pin 位同
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r6-env.mp4 --out out/lbr-wf-r6-env-diff
```

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (out/lbr-wf-baseline.mp4, workflow r0) | 4.73897 | 0.09711 | 0.1738 | 0.0782 | 0.0197 | 0.0163 |
| workflow r1–r5（pin 态） | 4.74383 | 0.09713024 | 0.17376 | 0.07822 | 0.01979 | 0.01624 |
| **workflow round 6（本轮，⑥b 接通仍中性）** | 4.743831 | **0.09713024316272291** | 同上 | 同上 | 同上 | 同上 |

per-segment 数值由本轮产物独立复算（`out/lbr-wf-r6-env-diff/pixel.csv`，
段窗 §3：S1 0–83 / S2 84–101 / S3 102–161 / S4 162–179）：S1 0.17376、
S2 0.07822、S3 0.01979、S4 0.01624——与 r5 逐位同（渲染位同的必然）。
worst_p8 0.1871146797839506 @f2（= r2 值）。历史最优保持 0.09711 @r0，
本轮为连续第六轮处于 r1/r2 最优。

### 29.4 已知偏差（本轮新增，提升后复核项）

- 源 equirect 采样 LOD 用常量 log2(W/(4·cubeSize))（∈[0,1)，仅 mip0/mip1），
  非 GL 逐像素导数 LOD——pmrem.rs 头注记录。
- CPU 卷积 ping-pong 回拷以恒等重采样等价跳过（f32 精度内），注释记录。
- scene.environmentRotation 参考未声明 → 恒等（three.js 默认单位阵）。

### 29.5 Next-step suggestions (workflow round 7/12 = threejs-like r10/12)

1. **⑥ 完毕 → 提升轮**（§27.6-2 清单不变）：BG_3D_PROMOTED +
   DROPS_WM_3D_PROMOTED + #bgcanvas 退役同一提交；提升后整条 3D 链
   （背景 ShaderMaterial、hero 球体 snoise+Physical 透射、droplets 3D、
   wordmark、⑥a 输出变换、⑥b env IBL）同时上盘，预期大幅吃掉
   hero 区域 S1/S2 p8≈0.17 的残差（diff 布局见 out/lbr-wf-r5-diff）。
   提升前后各跑一次 md5+k3diff；提升 commit 本身预期**破坏** md5 pin
   （这是提升的语义，非回归）。
2. 提升后首个对照点：f0/f30-33（p8≈0.18 的首尾帧）与 S1/S2 区块。
3. 若残差集中于折射采样 lod/MSAA（29.1 已知偏差②之外），下一轮
   优先接 RT mipmap + transmission lod 公式（偏差①）。

## 30. Workflow round 7/12 (second run) = threejs-like r10/12 (2026-10-07) — 提升轮：BG/DROPS/WM pin 翻转 + #bgcanvas 与全部 2D 模拟物退役 + Color.setHex 色彩管理修正（**新历史最优 0.09058**）

### 30.1 What was connected

§29.5-1 的提升轮，一次提交完成三件事（§26.3/§27.3 提升过渡规则：pin 与
退役同轮翻转）：

- **XML（examples/liquid-brand-refraction.xml，910→472 行）**：
  `BG_3D_PROMOTED`/`DROPS_WM_3D_PROMOTED` 两个过渡 pin 变量整体删除，恢复
  参考直写式——`uFade = 1 - fade`（js:527）、`wordmarkPlane3d.visible =
  t >= 3.0`（js:494）、droplets 无 visible 覆盖（参考对 droplet 无逐帧
  visible 写，Object3D 默认可见）。`#bgcanvas` + BGFIELD effect lambda、
  假玻璃滴 HERO lambda（#hero-wrap/#hero-canvas/#herofacet）、drop.png
  15 sprite（#drops + DROPS 表 + 2D 逐帧循环）、prism-crop 静态贴图
  （#prism + PRISM_HULL3/prismProjection/min-area-rect 投影段）、2D
  #wordmark clip-path 段全部退役；`#threecanvas` 成为唯一合成层。随之
  删除的还有仅服务 2D 胶水的中间量（FOCAL/projX/projY、CX/CY、PXW_DOCK、
  GLB_HALF、heroR/heroCx/heroCy、pxPerWorld、heroPop），3D 段仍消费的
  共享 f(t)（markScale/markXw/markYw/markRotationDeg、settle/prismRx/
  prismRy、WM_ASPECT/WM_PLANE_H、reveal、dropSettle）保留并原位引用 js
  行号。root 的 `bg-[#f4f2ea]` = 参考 body cream，js:528 canvas opacity
  淡出语义由 `#threecanvas.opacity(canvasOpacity)` 承担。
- **liquidClock 修正（提升后首个像素活路径暴露的结构偏差）**：旧 XML 以
  `heroTime = min(t, 2.4)`（= js:507 的 prismTime，settle 时钟）冒充
  js:524 的 `liquidClock(t)`——参考着色器时钟在 pop 后 2.8..3.6 以 1/4
  速从 2.4 回走至 3.2 再冻结（js:445-449 逐字实现进 XML），旧值把整段
  pop 后噪声相位钉死在 2.4。hero 与 droplets 现共用 `shaderTime =
  liquidClock(t)`（js:525-526）。
- **Color.setHex 色彩管理修正（three_api.js，generic）**：`rgbFromHex`
  此前回传 raw sRGB 分量；three.js 的 Color 分量恒在线性 working color
  space（Color.js:30-36 hexadecimal 自动转换，ColorManagement.enabled
  默认 true，r152+；ColorManagement.js:205-207 SRGBToLinear 逐字移植，
  常量同源）。参考场景全部数字色（attenuationColor 0x88ce7f js:177、
  hemisphere ground 0x7eb6a4 js:103、rim 0xc596ff js:107、key/js:104
  white）此前全部按 sRGB 直喂 Beer-Lambert/灯光——hero/droplet 体吸收
  因此弱了一倍以上（B 通道系数 0.498 vs 线性 0.2122），这正是提升首测
  （avg_p8 0.1299）hero 偏透明、droplet 无绿调的主因。伴生修正：
  `scene.background` 清屏按 WebGLBackground.js setClear 语义编码到输出
  色彩空间（getUnlitUniformColorSpace = renderer.outputColorSpace），
  SRGB 输出下 0x101014 round-trip 回 16/255（引擎清屏是 LoadOp::Clear
  原样写入，编码只能在 shim 完成）；ColorManagement.js:210-212
  LinearToSRGB 移植。
- **测试契约更新（script_runtime_tests.rs）**：
  `three_like_api_commits_scene3_snapshot_per_frame` 的三处 raw-hex 断言
  改线性期望（clear 0x101014→[0.005181517,0.005181517,0.006995410]
  （默认 Linear 输出）、ground 0x7eb6a4→[0.208636870,0.467783796,
  0.371237680]、base 0xe64a33→[0.791297940,0.068478170,0.033104767]，
  SRGBToLinear f64 手推值）；新增
  `three_like_api_background_clear_encodes_to_output_space`（SRGB 输出下
  clear round-trip 回 16/255，容差 1e-5——两次浮点传递函数往返实测
  |Δ|≈4.1e-6）。

### 30.2 Pitfalls hit this round (reproducible)

- **include_str! 缓存**：改 three_api.js 后必须 `cargo build --release`
  重编再渲染——首次提升后测量（avg_p8 0.1299）实际是「旧 shim + 新 XML」
  混合态；重编后 0.1299 → 0.0906 的差值全部来自色彩管理修正。
- **liquidClock ≠ min(t,2.4)**：js:507 `prismTime`（settle 用）与
  js:524 `liquidClock(time)`（uTime 用）是两个时钟；旧 XML 注释把后者
  错引为 js:507 并钉死在 2.4。提升前双态中性（3D 被遮挡）从测不出来。
- **提升首测的 S1 假回归要拆因**：S1 0.17376→0.24560 并非「3D hero 不如
  2D 假滴」，其中大头是色彩空间错误；修完 S1 = 0.174909（与 2D 校准
  假滴打平），S2/S3/S4 全线大幅改善。归因时先查语义正确性再看调参项。

### 30.3 Proof chain (all executed this round)

- Gates：`cargo build --release` clean；`cargo test -p opencat-core`
  688/0；`cargo test -p opencat-engine --lib` 98/0（含新 clear 编码测试）；
  `cargo test -p opencat-three --lib` 11/0；`cargo run --release -p
  opencat-three --example cube` 全过（shader/blend/map-wipe/IBL 解析冒烟）。
- 渲染：`out/lbr-wf-r7-promote2.mp4` md5 =
  73614057c819b4448ac277a05ab8960a（pin cb48573779f54a50fcfb989d0142f73e
  的打破是提升语义，非回归）。逐帧对照
  （out/_r7-promote-frames/cmp2_f0.png）：hero 绿体吸收 + prism 真折射
  上盘，f90 棱柱马赛克/位姿与参考几乎重合，f170 尾卡 wordmark 对位。
- md5 链：未重编的 promote 首渲 69237912…（= 旧 shim 混合态，avg_p8
  0.12988230）；重编后 promote2 73614057…（最终态）。

### 30.4 Measurement & ledger — kept = **true**（新历史最优）

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r7-promote2.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r7-promote2.mp4 --out out/lbr-wf-r7-promote2-diff
```

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (workflow r0，2D 全胶水) | 4.73897 | 0.09711 | 0.1738 | 0.0782 | 0.0197 | 0.0163 |
| workflow r1–r6（pin 态） | 4.74383 | 0.09713024 | 0.17376 | 0.07822 | 0.01979 | 0.01624 |
| r7 提升首测（旧 shim 混合态，诊断） | 5.59006 | 0.12988230 | 0.24560 | 0.07892 | 0.01851 | 0.01207 |
| **r7 最终（提升 + 色彩管理修正）** | **3.85136** | **0.09057749** | 0.174909 | 0.054470 | 0.008969 | 0.005164 |

worst_p8 0.22379292 @f2（诊断量）。**avg_p8 首次跌破 0.0971**：mae
−18.8% vs r0；S2/S3/S4 减半上下，S1 与 2D 校准假滴打平（0.1749 vs
0.1738，残差集中 f0-9 的 hero 折射放大率/边缘虹彩——见 30.5）。

### 30.5 Next-step suggestions (workflow round 8/12 = threejs-like r11/12)

1. **hero 段（S1 0.1749，worst f0-9 cell[5-8,1-5]）**：与参考逐帧对照
   （out/_r7-promote-frames/cmp2_f0.png）可见两个残差源——① 折射放大率：
   参考透过玻璃的 prism 影像显著更大（RT 采样/refraction vector 或
   getVolumeTransmissionRay 的 thickness·modelScale 路径待核）；② 边缘
   虹彩带（参考 rim 处青/品红 flush 更强，dispersion 路径 + 边缘掠射角）。
   §29.5-3 的 RT mipmap + transmission lod 仍在此之后。
2. **droplet 绿调微差**：f90/f170 对照小滴偏浅绿，attenuation 路径同
   hero，随 1 顺带复核。
3. **prism 段基本到位**（S3 0.01979→0.00897）：仅剩 frost/曝光细节，
   优先级低于 hero。
4. 保持「每轮先查语义正确性、再调参」的归因顺序（30.2-3 本轮教训）。

## 31. Workflow round 8/12 = threejs-like r11/12 (2026-10-07) — 位移法线遮蔽修复：噪声顶点位移只位移位置（**新历史最优 0.06956**）

### 31.1 What was fixed（接通/修正的一段管线语义）

§30.5-1 指名的「hero 折射放大率」残差，本轮定位为**顶点位移法线的
three.js chunk 顺序遮蔽**，一处修复（generic，非 liquid 专用）：

- **根因**（vendored `meshphysical.glsl.js` 顶点 main 逐行核对）：
  `beginnormal_vertex`(:33 `objectNormal = normal`) →
  `defaultnormal_vertex`(:37 `transformedNormal = objectNormal`，经
  `normal_vertex`(:38) 写入 vNormal) **先于** `begin_vertex`(:40) 执行。
  参考 onBeforeCompile 在 begin_vertex 替换里重赋 `objectNormal`
  （js:206）晚于 vNormal 消费——**对片元法线是死代码**。参考的有效语义
  = 位移后的位置 + 未位移法线（折射/高光走光滑球法线，剪影仍波动）。
  此前轮次把 js:189-207 逐行实装（lib.rs vs() 的切向双样本法线重建），
  字面对但语义错——带重建时 hero 折射影像被噪声法线搅成漩涡（r7 的
  「放大率小 3 倍」观感实为漩涡压缩）。
- **修复**（opencat-three/lib.rs vs()）：位移段只算 `d0` 并位移位置，
  `n` 恒为原顶点法线；js:190-205 的 p1/p2/d1/d2/法线重建不进管线
  （注释引用 chunk 行号与证伪记录）。XML 两处 `noiseDisplacement`
  注释（hero/droplets）同步标注有效语义；three_api.js shim 注释同步。
- **伴生 generic 修复**（本轮撞到的契约缺口）：shim
  `MeshPhysicalMaterial.attenuationDistance` 缺省/显式 `Infinity` 原样
  进快照 → serde f32 拒绝（渲染期 fail-loud）。现按 lib.rs:519 既定
  哨兵契约发 `1e30`（WGSL volumeAttenuation `>=1e29` = 不衰减）。新测试
  `three_like_api_physical_attenuation_distance_sentinel`（缺省/显式
  Infinity → 1e30、显式 2 → 2）。liquid 路径显式给 2，渲染逐位不受
  影响（md5 双渲位同验证，见 31.3）。

### 31.2 Pitfalls hit this round (reproducible)

- **ffmpeg 双输入默认映射**：`-i A … -i B -vf select -frames:v 1 out1
  … out2` 两个输出都取**第一个**输入 → 初轮 f2「参考帧」其实是自己的
  帧，目测对比全部无效。必须逐输出显式 `-map 0:v` / `-map 1:v`。
- **CPU 仿真两次符号错**：GLSL `refract(I,N,eta)` 以 `I = −v` 代入后
  `R = −eta·v − (√k − eta·dni)·N`；两个 Python 仿真各错一个符号，一度
  坐实「引擎与公式不符」假象。修正后引擎 vs 公式 mean|Δ|=0.249、
  >64 差异 0.12%（纯条纹 AA 边缘）——**透射采样路径逐像素忠实**，
  含 v 翻转（不翻转版 50% 差异，方向实证）。
- **周期标定对位移盲**：条纹是周期图案，采样点整体平移一个周期不可见
  ——标定场景必须带非对称探针（本轮用上半平面条纹带）才能量测
  位移/镜像类偏差。
- **RT 直接导出**定位法：transmission RT 只缓存 view；临时给
  `ensure_transmission` 保留 texture + COPY_SRC + env 门控回读即可 dump
  （诊断后已全部还原）。f0 的 RT 内容（prism 人脸 + bg 渐变）正确，
  排除 RT 内容嫌疑后才聚焦到法线。

### 31.3 Proof chain (all executed this round)

- Gates：`cargo build --release` clean；`cargo test -p opencat-core`
  688/0；`cargo test -p opencat-engine --lib` **99/0**（含新哨兵测试）；
  `cargo test -p opencat-three --lib` 11/0；`cargo run --release -p
  opencat-three --example cube` 全过（shader/blend/map-wipe/IBL 解析冒烟）。
- 渲染：`out/lbr-wf-r8-final.mp4` md5 =
  3d0a12ffdb82788fec96e99571a818d3（r7 pin 73614057… 的打破是修复语义，
  非回归；shim 哨兵修复后重渲 **md5 位同**——liquid 显式
  attenuationDistance:2，双证零影响）。逐帧对照
  （out/_r8-work/f0_fixed_cmp.png、f3_fixed_cmp.png、f90f170_cmp.png）：
  f0 球内 prism 折射影像（人脸/五边形/位置/尺寸）与参考几乎重合，
  掠射虹彩带随之出现；f90/f170 无回归。

### 31.4 Measurement & ledger — kept = **true**（新历史最优）

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r8-final.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r8-final.mp4 --out out/lbr-wf-r8-diff
```

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (workflow r0，2D 全胶水) | 4.73897 | 0.09711 | 0.1738 | 0.0782 | 0.0197 | 0.0163 |
| workflow r1–r6（pin 态） | 4.74383 | 0.09713024 | 0.17376 | 0.07822 | 0.01979 | 0.01624 |
| r7（提升 + 色彩管理修正） | 3.85136 | 0.09057749 | 0.174909 | 0.054470 | 0.008969 | 0.005164 |
| **r8（位移法线遮蔽修复）** | **2.45140** | **0.06956286** | 0.132873 | 0.044488 | 0.007968 | 0.004504 |

worst_p8 0.16950135 @f3（诊断量；r7 为 0.22379 @f2）。**avg_p8 −23.2%
vs r7、mae −36.4%**；四段全线下降（S1 −24.0%、S2 −18.3%、S3 −11.2%、
S4 −12.8%）。剩余残差仍集中 S1 f0-12（cell[8,7] 右中区），f3 对照
（out/_r8-work/f3_fixed_cmp.png）已到体吸收色调/虹彩分布微差级。

### 31.5 Next-step suggestions (workflow round 9/12 = threejs-like r12/12)

1. **hero 体吸收/虹彩微差**（S1 0.1329，cell[8,7]）：f3 对照我们球体
   下半偏浅（Beer-Lambert 路径长度 = thickness·modelScale 沿折射向量，
   已同构；疑 attenuationColor 线性化后的系数或 hemisphere 强度微差）。
   先复核语义（31.2-2 教训：先语义后调参），再看 §29.5-3 的 RT mipmap +
   transmission lod（lod≈log2(1920)·0.035≈0.38 bicubic，vs 现 lod0 线性
   ——边缘软化差异）。
2. **droplet 透射 RT 采样**：droplets 与 hero 共用同一路径（已随本轮
   受益）；f90 对照小滴绿调已接近，残余随 1 顺带复核。
3. 门禁基线更新：engine 测试数 98→99（新哨兵测试），后续轮次按 99 断言。

## 32. Workflow round 9/12 = threejs-like r12/12 (2026-10-07) — 透射采样链路接通：RT mip 链 + mipped bicubic + RT MSAA/HalfFloat（**新历史最优 0.06811**）

### 32.1 What was connected

§31.5-1 指名「先语义复核、再看 RT mipmap + transmission lod」的推进，两段
管线同一提交（§29.1 已知偏差①②同步解除）：

- **透射采样链路（原偏差①）**（opencat-three/lib.rs，WGSL）：
  `transmission_pars_fragment.glsl.js:45-149` 逐字移植——B 样条权重
  w0/w1/w2/w3（:45-64）、振幅/偏移 g0/g1/h0/h1（:66-84）、`bicubic`
  （:85-111，texel 坐标换算 + 2×2 核四角 textureLod）、`textureBicubic`
  （:113-127，floor/ceil 双级 bicubic 按 fract(lod) mix；GLSL textureSize →
  WGSL textureDimensions，越界护栏 lod 封顶 floor(log2(max(res)))——integer
  lod 时 fract=0，钳到同级数值恒等）、`applyIorToRoughness`（:135-138）、
  `getTransmissionSample`（:145-149；lod = log2(transmissionSamplerSize.x) ×
  roughness × clamp(ior·2−2,0,1)，transmissionSamplerSize = RT 尺寸 =
  globals.resolution.x，WebGLMaterials.js:504 + WebGLRenderer.js:282
  transmissionResolutionScale 默认 1.0）。色散循环逐通道 ior_i 进 lod
  （:195 同构）；transmission_sample(coords) 旧签名删除。
- **RT mip 链**（opencat-three/lib.rs，Rust）：`ensure_transmission`
  mip_count = floor(log2(max(w,h))) + 1；base 视图（level 0——wgpu 附件
  视图必须恰好 1 个 mip）与全 mip sample 视图（binding 9）双视图；
  `generate_transmission_mips` 逐级线性降采样 blit（WebGLTextures
  .updateRenderTargetMipmap → glGenerateMipmap 的对应物，WebGLRenderer.js
  :1942-1943 RT pass 后调用）；fullscreen-triangle mipgen 管线（wgpu NDC
  y=−1 = 帧行 0 = v0 ⇒ uv = ndc·0.5+0.5 直通不翻转——与
  project_to_refraction_coords 的 v 翻转是两套坐标约定，注释分立）。
- **偏差②解除：RT MSAA + HalfFloat**（WebGLRenderer.js:1875-1883 逐项）：
  透射 RT Rgba8Unorm → Rgba16Float（HalfFloatType，:1878）+ 4×MSAA 渲染
  resolve（samples: 4，:1880；updateMultisampleRenderTarget :1942 同序——
  pass_a 附件 = msaa 视图、resolve_target = base 视图）；RT pass 专用
  4× 深度（主 pass 深度 1× 不共用）；`build_render_pipeline` 加
  sample_count/target_format 参数，主管线与 ShaderProgram 各加
  (TRANSMISSION_SAMPLE_COUNT=4, Rgba16Float) RT-pass 变体（wgpu 约束：
  附件采样数 = 管线 multisample.count）；mipgen 管线目标同 Rgba16Float。
- **偏差⑤（本轮新增，已记录）：参考渲染的有效透射 lod = 0**。源码-渲染
  矛盾的完整证据：参考 build（index.html importmap → node_modules/three
  /build/three.module.js，0.181.2，渲染当日 12:10 安装、渲染 12:28）与
  src 逐字一致含 mipped bicubic 路径（hero lod≈log2(1920)×0.0575×0.66
  ≈0.41），但参考渲染透射内容锐度 = lod0 双线性（球内 Laplacian 均值
  ref 10.22 ≈ r8 12.19 ≈ r9 12.21，放大目测参考锐度 = r8）；lod 系数扫描
  （k3diff）单调证明 ×1.0 → S1 +0.0195、×0.5 → +0.0064、×0 最优。机制
  假设（未证实）：swgl/SwiftShader glGenerateMipmap 失效 →
  LINEAR_MIPMAP_LINEAR 不完整纹理按 lod0 回退。对齐地面真值是渲染 mp4
  而非 node_modules 源码 ⇒ WGSL const `TRANSMISSION_LOD_SCALE = 0.0`
  （公式链全保留，注释含证据链与翻回条件）。
- **语义复核（§31.5-1 教训先行）**：hero 材质常量 XML:183-193/346-356 与
  js:163-179 逐字一致；attenuationColor 线性化已在 r7 修正；geometryRoughness
  lib.rs（fs_physical 调用点 dpdx/dpdy(normal) 逐分量 max）与
  lights_physical_fragment.glsl.js:5-9 一致——hero 段语义无新洞，残差非
  语义错。

### 32.2 Pitfalls hit this round (reproducible)

- **「参考源码」≠「参考渲染」**：lod 全链忠实实现反而回归（avg_p8
  0.06956→0.07983）。源码考证（build vs src、package 时间线、公式逐字）
  不能替代对渲染输出的测量——归因顺序升级：先对**渲染**的语义，再源码，
  再调参。偏差⑤从此与 three.js 源码常量同为一级真值来源。
- **MSAA 放大锯齿假设证伪**：先验「无 MSAA 的 RT 锯齿内容被 mip blur
  扩散」被测量否定——×1.0 无 MSAA（S1 +0.0205）→ ×1.0 + MSAA/HalfFloat
  （S1 +0.0195）几乎不变，回归根源是 lod 本身。MSAA/HalfFloat 保留的
  依据是其单独贡献（×0 时 0.06956→0.06811），非该假设。
- **wgpu 附件视图必须恰好 1 个 mip**：mip_level_count>1 时
  `create_view(&Default::default())` 产出全 mip 视图，作 render attachment
  直接验证失败——RT 需 base/sample 双视图（结构体注释记录）。
- **本仓 wgpu API 形状**：PipelineLayoutDescriptor 是 `immediate_size`
  （无 push_constant_ranges）、bind_group_layouts 是 `&[Option<&BGL>]`、
  RenderPipelineDescriptor 用 `multiview_mask`、ShaderSource::Wgsl 要
  `Cow::Borrowed`——照 build_render_pipeline 既有形状抄，不凭记忆。
- **Rust const 不进 WGSL**：TRANSMISSION_LOD_SCALE 首版放 Rust 侧 → naga
  「no definition in scope」运行期失败；WGSL 是独立编译单元，常量须声明
  在 WGSL 内（fail-loud 出现在 render 而非 build）。
- **「r9 更糊」的目测误判**：2× NEAREST 放大对照下 r9 球内看似明显更糊，
  Laplacian 定量 r8/r9 几乎相同（12.19 vs 12.21）——放大对照只作假设
  来源，定量才能进结论。

### 32.3 Proof chain (all executed this round)

- Gates：`cargo build --release` clean；`cargo test -p opencat-core`
  688/0；`cargo test -p opencat-engine --lib` 99/0；`cargo test -p
  opencat-three --lib` **12/0**（11 + 新 `mip_dims` 单测）；`cargo run
  --release -p opencat-three --example cube` 8 项 ok（shader/blend/
  map-wipe/透射/IBL 解析冒烟全过——RT-pass 管线变体在透射冒烟里实跑）。
- 渲染：`out/lbr-wf-r9d-final.mp4` md5 =
  22b6c1e557a91d8a0d4b6fd6ab459b0c（r8 pin 3d0a12ff… 的打破 = lod scale
  0.0 + MSAA/HalfFloat 语义，非回归）。
- 诊断中间态（证据保留）：`out/lbr-wf-r9-final.mp4`（lod×1.0，无 MSAA）
  2a5f243f… avg_p8 0.07983；`out/lbr-wf-r9b-final.mp4`（×1.0 + MSAA/
  HalfFloat）8cc6b202… 0.07930；`out/lbr-wf-r9c-lod05.mp4`（×0.5 + MSAA/
  HalfFloat）0.07269；`out/_r9-work/`（f3 hero 对照/回归热图/锐度测量）。

### 32.4 Measurement & ledger — kept = **true**（新历史最优）

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r9d-final.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r9d-final.mp4 --out out/lbr-wf-r9d-diff
```

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| base (workflow r0，2D 全胶水) | 4.73897 | 0.09711 | 0.1738 | 0.0782 | 0.0197 | 0.0163 |
| workflow r1–r6（pin 态） | 4.74383 | 0.09713024 | 0.17376 | 0.07822 | 0.01979 | 0.01624 |
| r7（提升 + 色彩管理修正） | 3.85136 | 0.09057749 | 0.174909 | 0.054470 | 0.008969 | 0.005164 |
| r8（位移法线遮蔽修复） | 2.45140 | 0.06956286 | 0.132873 | 0.044488 | 0.007968 | 0.004504 |
| **r9（透射采样链路 + RT MSAA/HalfFloat）** | **2.42966** | **0.06810831** | 0.130001 | 0.043390 | 0.007958 | 0.004495 |

worst_p8 0.16505 @f3（r8 为 0.16950 @f3）。**avg_p8 −2.1% vs r8、mae
−0.9%**；135/180 帧改善；四段全线下降（S1 −2.2%、S2 −2.5%、S3 −0.1%、
S4 −0.2%）。残差仍集中 S1 f0-12（hero 段右中区）；透射模糊路线已封
（偏差⑤），该残差与透射采样无关。

### 32.5 Next-step suggestions (workflow round 10/12 = threejs-like r13/12)

1. **hero 段残差的下一嫌疑**（S1 0.1300，f0-12）：透射模糊已排除，按
   §30.5-1/§31.5-1 顺延——体吸收色调/虹彩分布微差（hemisphere 强度/色散
   逐通道相位）；f3 放大对照（out/_r9-work/f3_zoom_ref.png vs
   f3_zoom_cmp.png 左/中）显示参考球内折射影像仍有小幅位置/尺寸偏移
   （折射放大率残差的最后一级，先查 proj/view 矩阵语义再调参）。
2. **偏差⑤机制验证（不急）**：在参考仓用真实 GL/swgl 跑一次 mip1 内容
   readPixels 对照探针可证实 SwiftShader 假设；证实前
   TRANSMISSION_LOD_SCALE 保持 0.0，不要动。
3. 门禁基线更新：opencat-three 测试数 11→**12**（新 mip_dims 单测）；
   engine 维持 99；后续轮次按此断言。

## 33. Workflow round 10/12 (second run) (2026-10-07) — env IBL equirect 垂直翻转修复：flipY 上传语义（**新历史最优 0.02658**）

### 33.1 What was connected

§32.5-1 指名「先查 proj/view 矩阵语义再调参」。proj/view 逐行复核无洞；
残差定位改走 **three.js oracle 成对探针**（本轮新建的基础设施，产出即修复）：

- **Oracle 基础设施**（/tmp/lbr-oracle/，不入仓）：参考仓自带
  node_modules/three 0.181.2 + headless Chrome SwiftShader
  （puppeteer-core，`--use-gl=angle --use-angle=swiftshader
  --allow-file-access-from-files --enable-unsafe-swiftshader`）渲染 1080p
  f0 态探针（probe4.html：js:163-179 材质逐字 + js:181-209 位移逐字 +
  uTime=0/z=6.55/scale 1.08），`readPixels` 出图；opencat 侧同场景 XML 探针
  （examples/_probe-1080-*.xml，**Renderer 构造必须同 1920×1080**）逐配置
  对渲。**noenv 成对差 0.49 ≈ 0** ⇒ 除 IBL 外全链（几何/位移/透射/直接光/
  cc/输出变换）已逐像素对齐，唯一分歧 = env。
- **判决矩阵**（band = cell[8,7]）：修复前 |ref−our| f0 **32.89**、
  noenv **0.49**；env 贡献（full−noenv）我侧 56.06 vs 参考 25.08，逐通道
  比值 R/G/B = **4.22/2.38/1.63**（我方偏暖红、参考偏蓝绿）。
- **修复（generic，pmrem.rs）**：`sample_equirect_linear` 行索引
  `y = sv·h − 0.5` → `y = (1−sv)·h − 0.5`。语义：equirectUv
  （common.glsl.js:82-92）输出的 v=1 = 天顶 = GL 纹理 v=1；three.js 纹理
  默认 **flipY=true**（UNPACK_FLIP_Y_WEBGL 上传翻转）⇒ GL v=1 = 自然图像
  **顶行**；CPU 直读数组行 0 = 顶 ⇒ 行索引必须 (1−v) 反算。调用点
  equirectUv 公式不变，注释同步改写（pmrem.rs:123-131/:387-404）。

### 33.2 Pitfalls hit this round (reproducible)

- **跨尺度对比全作废**：参考 oracle 是 1920×1080，opencat 侧探针 XML 生成
  时 sed 漏改 `new T3.Renderer('threecanvas', 960, 540)` 的内部分辨率参数
  ⇒ 输出贴在画布左上 960×540、其余 cream。第一组成对矩阵（band 42/位移
  效应 −18.5 vs −0.12 等）全部基于错位裁剪，**已作废**。探针两侧必须断言
  同尺寸再比（本轮分析脚本已加 `assert shape==1080`）。
- **flipY 是上传语义不是采样语义**：GL 里 v=0 恒为上传数据首行；flipY
  在上传时翻转数据 ⇒「flipY=true 的纹理」其 GL v=1 = 图像顶行。CPU 复刻
  必须把 equirectUv 的 GL v 换算回自然行序，而不是照抄公式。
- **为什么 960p rsweep（§29 轮）没暴露翻转**：球心带的反射向量近水平，
  asin(dir.y)/π+0.5 在 dir.y=0 时 v=0.5 为翻转不动点 ⇒ 粗扫吻合；球下缘
  反射向下才翻转出错——**探针必须覆盖轮廓带**，中心带不能为 IBL 方向
  语义背书。
- **DFG LUT 语义再确认**（vendored lights_physical_pars_fragment.glsl.js
  :11/:379-396 + DFGLUTData.js）：r181 用预计算 32×32 RG16F LUT（uv =
  (roughness, dotNV)、LinearFilter+ClampToEdge），WebGLRenderer.js:2517-19
  全材质绑定；我方 lib.rs:28/:216/:443-452 的 dfg_lut 模块早已同构
  （Rg16Float 直传 u16 位型）——本轮逐行复核 clearcoatF0=(0.04)/F90=1
  （lights_physical_fragment.glsl.js:56-57）、cc radiance（lights_fragment
  _maps.glsl.js:35）、multiscatter/outgoing（meshphysical 尾段）均一致，
  无需改动。

### 33.3 Proof chain (all executed this round)

- Gates：`cargo build --release` clean；`cargo test -p opencat-core`
  **688**/0；`cargo test -p opencat-engine --lib` **99**/0；
  `cargo test -p opencat-three --lib` **12**/0；`cargo run --release
  -p opencat-three --example cube` ok（IBL 解析解 0.7/env-off 冒烟过）。
- 成对探针（1080p，band=cell[8,7]，修复后）：f0 **3.81**、f0-nocc 3.00、
  f0-spec1 2.56、f0-nodisp 3.01、f0-noenv **0.49**、f0-nodisp-nocc 2.44
  （修复前 f0 32.89）。f31 球中部裁剪对照 |d|=3.92（prism/facet/折射/
  frost/高光点肉眼一致，残差 = 轮廓细窄虹彩带的亚像素弥漫差）。
- 渲染：`out/lbr-wf-r10-final.mp4`；diff 产物 `out/lbr-wf-r10-diff/`
  （summary.json/pixel.csv）。

### 33.4 Measurement & ledger — kept = **true**（新历史最优）

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r10-final.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r10-final.mp4 --out out/lbr-wf-r10-diff
```

| version | mae_mean | avg_p8 | S1 | S2 | S3 | S4 |
|---|---|---|---|---|---|---|
| r8（位移法线遮蔽修复） | 2.45140 | 0.06956286 | 0.132873 | 0.044488 | 0.007968 | 0.004504 |
| r9（透射采样链路 + RT MSAA/HalfFloat） | 2.42966 | 0.06810831 | 0.130001 | 0.043390 | 0.007958 | 0.004495 |
| **r10（equirect flipY 修复）** | **1.42613** | **0.02657791** | 0.0479 | 0.0213 | 0.0053 | 0.0030 |

**avg_p8 −61.0% vs r9、mae −41.3%**。四段全线大降（S1 −63%、S2 −51%、
S3 −33%、S4 −33%）。worst_p8 **0.05568 @f31**（f31-37 均 ≈0.055，
grid_max_cell 落 cell[8,3]-[9,4] 球中部）；grid_max_cell 已从 [8,7]
（球下缘带，flipY 主受损区）移到球中部右缘。ledger：round 10，
kept = true。

### 33.5 Next-step suggestions (workflow round 11/12)

1. **S1 剩余 0.0479 的形态**：f0-12 与 f31-37 均为轮廓细窄虹彩带 + 亚像素
   弥漫差（f31 对照 |d|=3.92 无结构性错位）。下一嫌疑按序：色散逐通道
   相位（dispersion=1 的 halfSpread 微差）、轮廓 AA（MSAA 主 pass？参考
   WebGLRenderer antialias:true 默认开——主 pass 是否 4×MSAA 待核）。
2. **oracle 方法论延续**：/tmp/lbr-oracle/ 的 probe4.html + drive.mjs +
   opencat 侧 _probe-1080-*.xml（本轮清理前）可复现任意单变量对拍；新
   嫌疑先走 noenv 式「成对归零」判定再动引擎。
3. **清理决定**：examples/_probe-*.xml（19 个 960p）与
   examples/_probe-1080-*.xml（6 个）为诊断资产，本轮已删除（方法论入
   §33.1，可按 /tmp/lbr-oracle/gen_xml.py + §33.1 参数重建）；out/_r10
   -work/ 诊断 png 保留（out/ 不入 git）。
4. 门禁基线不变：core 688 / engine 99 / three 12。距门 0.02 还差
   0.0066；S1 每降 0.01 ≈ 全局 −0.0028。


## 34. Workflow round 11/12 (2026-10-07) — 主 pass 4×MSAA（antialias:true 语义）+ oracle 棱柱成对探针（**新历史最优 0.02631**）

### 34.1 What landed

1. **主 pass 4×MSAA 实装**（`crates/opencat-three/src/lib.rs`）：参考
   renderer 构造 `antialias: true`（js/liquid-pop-v2.js:20）→ WebGL2 默认
   帧缓冲带多重采样（ANGLE/SwiftShader 实现 = 4×）。对应物：
   `MAIN_SAMPLE_COUNT=4`（lib.rs:2677）；主管线 + pipeline_blended +
   ShaderProgram 管线 sample_count 1→4（lib.rs:1117/1131/1598/1610）；
   `ensure_depth` 4×（lib.rs:2477/2505）；新增 `ensure_msaa_color`（4×
   Rgba8Unorm RENDER_ATTACHMENT，尺寸变化才重建）；主 pass 附件
   `view=msaa_color_view, resolve_target=Some(base_view)`（lib.rs:2343/
   2489-2525 注释）——draw 落 4× 附件，resolve 写 1× 读回视图，即
   drawing buffer 语义。这是职责顺序④收尾（MSAA 是渲染目标语义，属
   透射 pass 之外的最后一个渲染器构造差异）。
2. **oracle 棱柱成对探针基础设施**（/tmp/lbr-oracle/，不入仓）：
   `probe5.html` + `drive5.mjs`——f31（worst 帧）单帧 oracle：球
   SphereGeometry(1.5,128,128) + prism GLB（字节 base64 注入 +
   `GLTFLoader().parse(ArrayBuffer)`）+ 8 路 facet 帧 PNG（ffmpeg 从两侧
   md5 一致的 mp4 预抽 t=31/30，绕开 headless SwiftShader 的 8×video
   CONTEXT_LOST）+ importmap 直指参考仓 node_modules three。hero/prism
   动画公式内联（js:456-471/:495-508），单变量开关
   `{mat:{...}, nohero, noenv}`。opencat 侧探针 XML 家族
   （`_probe5*.xml`：full/nodisp/nothick/prism-only）**已按 §33.5-3 先例
   删除**，可从主 XML 删 droplets 段（332-382/:429-447）+ 删 hero/prism
   行重建。
3. **get_transmission_sample 定稿**：透射采样核双线性假设**证伪并回滚**
   （PNG 域公平对拍 f5 0.0627→0.0635、f31 0.0637→0.0645，变差）；最终
   状态 `let lod = 0.0; return texture_bicubic(frag_coord, lod);`
   （lib.rs:613）——等价 r10 的 `TRANSMISSION_LOD_SCALE=0.0`，常量已删，
   依据并入注释（参考渲染（SwiftShader）RT generateMipmaps 语义下有效
   lod=0 的源码-渲染矛盾结论，§33）。

### 34.2 Pitfalls hit this round (reproducible)

- **WGSL phony assignment**：`let _ = x;` 非法（naga「Identifier can't
  be `_`」），必须逐个 `_ = x; _ = y;`。
- **file:// 下 GLTFLoader**：fetch API 不支持 file scheme →
  `loadAsync` 不可用；driver 读 GLB 字节 base64 注入页面 +
  `parse(ArrayBuffer)`。且 file:// 无 importmap，模块页必须自带
  `<script type="importmap">` 直指 node_modules/three。
- **headless SwiftShader + 8×`<video>` = CONTEXT_LOST_WEBGL（全黑）**：
  ffmpeg 预抽 PNG + `THREE.TextureLoader` 等价（上传路径同为
  texImage2D），对拍语义不变。
- **oracle 探针动画量必须当帧公式**：hero.scale 沿用 f0 常量
  （establish=0 的 1.08）而 f31 应为 establish=1 → scale(1,1,1)，修复
  前 oracle-vs-ref mean 11.05、修复后 3.789——探针里所有时变量
  （establish/pressure/settle）都要从 js 公式内联求值，不许手算/沿用。
- **PNG 域 vs mp4 域不可跨域判定**：探针 png 对拍 mp4 帧混入编码噪声
  （MSAA 曾被 png-vs-mp4 误判「变差」）；判定同域内做——png-vs-png
  归因、全量 mp4-vs-mp4 k3diff 验收。
- **pixel.csv 解析**：bbox 字段含逗号，`sort -t,`/awk 错列，用 python
  csv/json。
- **未归因项（记录不复原）**：corner 条带放大对照 oracle 粉 vs opencat
  青/绿。常量链全部排除：kCornerColors（js:260-264）、
  videoIdToCornerColor (a,invert) 表（js:272-285，XML cornerRule
  逐值一致）、faceToTexture（js:267-270）、COLOR_0 dominant
  选择器（WGSL lib.rs:383 与 js:273 逐字同构）、COLOR_1 截断
  （514×255/65535 精确=2.0，两侧同构）、GLTFLoader ATTRIBUTES 映射
  （COLOR_1→`attributeName.toLowerCase()`='color_1'，GLTFLoader.js:2309/
  2019）。现象归因于窄条带内 COLOR_0 顶点插值 dominant 翻转 + 亚像素
  移位（prism-only 边框box mean 8.72 的主体是窄线位置差不是色相），
  对 avg_p8 贡献 <0.001 级。

### 34.3 Proof chain (all executed this round)

- Gates（**回滚后代码**重编重测）：`cargo build --release` clean；
  `cargo test -p opencat-core` **688**/0；`cargo test -p
  opencat-engine --lib` **99**/0；`cargo test -p opencat-three --lib`
  **12**/0；`cargo run --release -p opencat-three --example cube` 8 项
  ok（含 transmission/IBL 解析解冒烟）。
- **oracle 判定矩阵**（f31，oracle vs opencat，mean|d| 全图 / 棱柱边框
  box）：full **1.493**/8.72；nodisp 1.286/8.49；nothick 0.781/8.76；
  prism-only（无球）0.769/p8 0.0084（姿态对齐）；oracle AA vs noAA
  相对 opencat 8.64 vs 8.89（**主 pass MSAA 语义排除**——差异非 AA 级）；
  oracle vs ref mp4 线位置 ±1px（oracle 可信基准）。
- **thickness 折射几何同构证明**：两侧各自 full−nothick 差图相关性
  **r=0.9899**（oracle thickness 贡献 mean 15.657 vs opencat 15.304）
  → 透射残差不在 thickness 语义，是虚像位置的亚像素浮点级差。
- **单变量排除矩阵（本轮封闭）**：色散（nodisp 与 full 同级差）；
  thickness（r=0.99）；主 pass AA（8.64 vs 8.89）；透射采样核（双线性
  变差）；GLB 解析（gltf crate，两侧 GLB md5 逐一相同）；
  COLOR_0/COLOR_1 语义（见 34.2）；prism 姿态（prism-only p8 0.0084）。
- 渲染：`out/lbr-wf-r11b-final.mp4`（回滚后 binary）；diff 产物
  `out/lbr-wf-r11b-diff/`。中间态 binary（MSAA + 双线性）的
  `out/lbr-wf-r11-final.mp4` avg_p8 0.02631 与 r11b **逐位一致**
  （lod=0 常量折叠 ⇒ 采样语义等价的直接证明）。

### 34.4 Measurement & ledger — kept = **true**（新历史最优）

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r11b-final.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r11b-final.mp4 --out out/lbr-wf-r11b-diff
```

| version | mae_mean | avg_p8 | S1(0-83) | S2(84-101) | S3(102-161) | S4(162-179) |
|---|---|---|---|---|---|---|
| r10（equirect flipY 修复） | 1.42613 | 0.02657791 | 0.0479 | 0.0213 | 0.0053 | 0.0030 |
| **r11（主 pass 4×MSAA）** | **1.39643** | **0.02631210** | 0.0476 | 0.0210 | 0.0052 | 0.0029 |

**avg_p8 −1.0%、mae −2.1%**；155/180 帧改善、24 变差、1 持平；四段全
线微降。worst_p8 **0.05545 @f35**（r10: 0.05568 @f31，f31-37 带整体
下移）。ledger：round 11，kept = true。

### 34.5 Next-step suggestions (workflow round 12/12)

1. **残差构成已定量**：引擎-引擎（oracle vs opencat f31）mean ~1.49 vs
   引擎-参考（oracle vs ref mp4 同帧）~3.79——**一半以上残差在
   「oracle 场景假设 vs 参考录制真实」之间**（droplets 逐帧状态、视频
   纹理时间轴、录制环境差），引擎纯渲染差只剩 ~1.5。继续压引擎侧
   着色细节的边际收益趋零；**职责顺序⑤ droplets 3D 化**是 S1 剩余
   0.0476 的结构性大头（S1 帧差主要是 droplets 轮廓 + 球内弥漫）。
2. **prism 虚像边框线**（f31 cell[8,3] 球中部）：透过球面看到的 facet
   虚像边框亚像素差，与 corner 条带未归因项同源（34.2 末条）；probe5
   探针家族可复现（nothick 开关最快）。
3. **oracle 方法论延续**：/tmp/lbr-oracle/{probe5.html,drive5.mjs,frames/}
   可复现 f31 单帧任意单变量对拍；opencat 侧探针 XML 按本节 34.1-2 重建；
   新嫌疑先成对归零再动引擎。
4. 门禁基线不变：core 688 / engine 99 / three 12。r11 后距门 0.02 差
   **0.0043**。

## 35. Workflow round 12/12 (2026-10-07) — 视频解码色彩管理接通：BT.709 帧元数据矩阵（**新历史最优 0.02575**）

### 35.1 What was connected

§34.5 的「oracle 场景假设 vs 参考录制真实」拆解落到了一个具体可修的引擎级差：
**native 视频解码的 yuv→rgb 用 swscale 内建默认 BT.601 系数，而参考（浏览器
`<video>` 解码 + ffmpeg CLI auto）按容器标注的 BT.709 矩阵转换**。这是
`crates/opencat-engine/src/media/decode.rs` 的 generic 修复（与 §33 flipY
同类：视频纹理上传语义的正确化），零 liquid 专用代码：

- **归因链**（全部本轮实测）：①三方对拍 f31（oracle raw / opencat mp4 帧 /
  ref mp4 帧）显示 mp4 编码域噪声 ~2.3 mean，引擎归因必须走 raw 域 →
  opencat CLI 本就支持单帧无损 PNG（`opencat render in.xml out.png 31`，
  opencat.rs:79-89 `render_single_frame_png_with_base`）；②720×720 无重采样
  视频探针：opencat 视频采样 vs ffmpeg 709 基准 mean 8.66 / p8 0.3867，且
  vs 帧 25/26 对称 → 非帧错位是系统色偏；③手工 yuv 四组合对拍：opencat 输出
  ≈「601+tv 展开」（mean 2.70），ffmpeg 基准 =「709+tv 展开」（2.78），两
  矩阵互差 mean 8.1 / p8 0.3776，与 oc-vs-ref 完全吻合；④facet-01.mp4 元数据
  `yuv420p/tv/bt709` 全标注（25fps/550 帧），参考 mp4 同为 bt709 全标注，
  opencat 输出 mp4 全 unknown（unknown → ffmpeg auto 按 601 解，与 opencat
  编码默认 601 构成无损环，故 k3diff 比较环两侧各自无损、度量的是真内容差）。
- **修复**（crates/opencat-engine/src/media/decode.rs）：`ColorspaceSpec`
  （decode.rs:452；`from_frame` :461 逐帧 / `from_codec` :473 解码器级流回退）
  ——`color_space()==BT709` → spec，range 按 `color_range()`；`YuvRgbScaler`
  （decode.rs:493）按 libswscale 文档序列构建 scaler：`sws_alloc_context` →
  `av_opt_set_int`（srcw/srch/src_format/dstw/dsth/dst_format/flags）→
  `sws_setColorspaceDetails`（:549；ITU709 表、srcRange 0=mpeg/1=jpeg、
  dstRange 0=RGB full、contrast/saturation 1<<16=16.16 定点 1.0）→
  `sws_init_context`（:560）；scaler 缓存 key 扩为 `(out_w, out_h, spec)`
  （`scaler_for` :415，spec 采集点 `update_scaled_frame` :397-406），spec 不同
  重建。**Unspecified 流完全走原默认路径（601），旧行为零变化**。
- **测试**：`bt709_frame_metadata_selects_bt709_sws_coefficients`（像素级锚：
  709/limited 绿 YUV(173,42,26) → R≈0 而 601 残留 ≈20；709 白 (235,128,128)
  → 255 全范围而非 235；JPEG 白 (255,128,128) → 255；Default spec 钉住
  601 残留语义；spec 相等性）+ `real_bt709_asset_decodes_nonblack`（ignored，
  `LBR_FACET_MP4` 指真资产跑端到端非黑断言）。

### 35.2 Pitfalls hit this round (reproducible)

- **`sws_setColorspaceDetails` 在 `sws_getContext` 之后调用 = 黑输出**：
  `sws_getContext` 即时 init（BT.601 默认系数），FFmpeg 7+ 的系数表预展开后
  事后设置损坏转换（rc=0、输出全 0，连等参重设也黑）。官方序列（swscale.h
  sws_alloc_context 文档）：alloc → AVOptions → **setColorspaceDetails →
  sws_init_context**。本修复按此序列自建 scaler（ffmpeg-next 的
  `ScalingContext` 无 from_raw 且 `Context::run` 走旧 `sws_scale`）。
- **contrast/saturation 是 16.16 定点**：`setColorspaceDetails` 尾三参传 0
  = 对比度 0 倍 = 全黑；中性值是 brightness=0、contrast=saturation=1<<16。
- **srcRange 语义**（swscale.h）：`1=jpeg(full)/0=mpeg(limited)`——与直觉
  相反，tv 范围输入传 0。dstRange 同序（RGB 输出恒 0=full）。
- **bindgen 的 `AVPixelFormat` 无 `.0`**：取 AVOption 数值用
  `transmute::<AVPixelFormat, c_int>`（sys lib.rs 顶部 allow
  unnecessary_transmutes 即此用途）。
- **facet mp4 是 25fps/550 帧**（非 30fps）：合成 t 的视频帧号 = floor(t×25)，
  f31→帧 25、f75→帧 62。opencat 取整帧 floor 语义与浏览器 currentTime
  一致（f75 对 n62 mean 8.87 vs 对 -ss 精确 12.12，整帧更近）——时间轴无错位，
  无需改。
- **跨域对拍陷阱再现**：oracle readPixels(raw) vs mp4 解码帧差 ~3.8 mean，
  其中编码域噪声 ~2.3——引擎归因一律 raw 域成对（§34.2 规则第三次生效）。
- **修完视频内容后残差重新归因**：raw 域 f31 成对 oracle-vs-opencat 中
  sphere-core 8.62→8.15、prism-apex 6.63→6.36（几乎不动）——球内透射虚像差
  是浮点级虚像位置差（§34.3 r=0.99 thickness 同构结论不变），**不可再压**；
  droplet-top 4.36/0.126 恒定且与视频内容无关，差图形态 = 盘面透射亮度系统
  差 + 均匀轮廓环（非位移月牙）——droplet 小球的透射 RT 采样/衰减细节是
  下一轮唯一明确的引擎级嫌疑。

### 35.3 Proof chain (all executed this round)

- Gates：`cargo build --release` clean；`cargo test -p opencat-core` **688**/0；
  `cargo test -p opencat-engine --lib` **100**/0（99 基线 + 新像素锚；另有
  ignored real-asset 探针）；`cargo test -p opencat-three --lib` **12**/0；
  `cargo run --release -p opencat-three --example cube` ok（IBL 解析解 1.0/
  0.7/env-off 冒烟过）。
- 视频探针回归：720×720 全屏 facet-01 探针 f31 修复前 vs ffmpeg 帧基准
  mean 8.658 / p8 0.3795 → 修复后 **mean 0.0 / p8 0.0（逐位一致）**；
  oracle frame-01.png 与 `ffmpeg -ss 31/30` 抽帧逐位一致（mean 0.0，oracle
  基准可信）。
- raw 域成对（f31，oracle vs opencat 探针 PNG）：full mean 2.102→2.052；
  nodrops 对照 droplet-top 4.36→0.96（droplet 差实锤）；prism-only 成对
  mean 1.836（bot-band 0.38——背景引擎差极小）。
- 渲染：`out/lbr-wf-r12-final.mp4`；diff 产物 `out/lbr-wf-r12-diff/`
  （§35.4 命令原样重跑复核，avg_p8 0.025749772269375852 / mae_mean
  1.3923848531450043 与首渲逐位一致；探针 XML 已按 §33.5-3 先例删除，
  配方在本节+§34.1，诊断 png 留 /tmp/lbr-r12/ 不入 git）。

### 35.4 Measurement & ledger — kept = **true**（新历史最优）

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r12-final.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r12-final.mp4 --out out/lbr-wf-r12-diff
```

| version | mae_mean | avg_p8 | S1(0-83) | S2(84-101) | S3(102-161) | S4(162-179) |
|---|---|---|---|---|---|---|
| r11（主 pass 4×MSAA） | 1.39643 | 0.02631210 | 0.0476 | 0.0210 | 0.0052 | 0.0029 |
| **r12（视频解码 BT.709）** | **1.39238** | **0.02574977** | 0.04672 | 0.01999 | 0.00499 | 0.00283 |

**avg_p8 −2.1%、mae −0.3%**；S2 −4.8%、S3 −4.0%（棱柱 facet 占比大的段全
改善），S1 −1.8%。worst_p8 **0.05437 @f35**（r11: 0.05545 @f35，f31-37 带
整体下移 ~0.001）。距门 0.02 还差 **0.0057**。ledger：round 12，kept = true。

### 35.5 Next-step suggestions（后续轮次，按依赖序）

1. **droplet 盘面透射差**（唯一明确的引擎级嫌疑）：oracle-vs-opencat
   droplet-top raw 差 4.36/0.126，形态为盘面亮度系统差——候选：droplet 小球
   的透射 RT 采样视差（thickness 1.5 相对 0.1 半径的折射几何）、RT 内容在
   droplet 屏区的差异、attenuation 路径。probe5 探针家族加 `nodrops` 开关 +
   opencat `_p5_nodrops`（本节配方）可直接复现成对。
2. **编码侧色彩标注**（generic，指标中性）：opencat 输出 mp4 色彩元数据全
   unknown（编码环 601 无损但与参考 bt709 标注不对齐）。encode.rs 的
   rgba→yuv scaler + 编码器 `set_colorspace/set_color_range`（bt709/tv）会
   让输出元数据与参考一致；对 k3diff 环无损中性（两侧各自无损 roundtrip），
   改后需跑一次 vfx-heygen 基线确认中性。
3. **residual 上界已定量**：oracle-vs-opencat f31 全图 raw mean 2.05（其中
   env ~1.0 + 浮点虚像位置差为主），S1 段 mp4 域 0.0467 的构成 = 轮廓细带 +
   droplet 盘面 + 球内虚像亚像素差；引擎着色语义层面无已知未对齐项。
4. 门禁基线：core 688 / engine 100（含新像素锚）/ three 12。

## 36. Workflow final wrap-up (2026-10-07) — 收尾盘点：threejs-like 六段全接通，终态 avg_p8 **0.02575**（**未达 0.02 门，差 0.0057**）

> 本小节为工作流收尾盘点轮（不产代码）。渲染与 k3diff 均为 §35.4 所录
> r12 轮实跑命令；本小节数字是对 r12 产物
> `out/lbr-wf-r12-diff/{summary.json,pixel.csv}` 的**直接读取核实**
> （本轮实跑：python csv/json 解析两个产物文件），非重渲。

### 36.1 threejs-like 路线推进到哪一段：①-⑥ 全部接通，2D 模拟物全部退役

XML 头注释（examples/liquid-brand-refraction.xml:14-21）声明提升态：
`BG_3D_PROMOTED / DROPS_WM_3D_PROMOTED` pin 已撤除，#threecanvas 是唯一
合成层，整条 3D 链同时上盘（§26.3 提升过渡规则）。对照推进顺序：

| 段 | 内容 | 状态与证据 |
|---|---|---|
| ① | 最小 3D 场景进渲染输出（scene→mesh→camera→draw IR） | ✅ opencat-three（wgpu）+ ir/scene3.rs + three_api.js；#threecanvas 唯一合成层（xml:28） |
| ② | hero 球体 SphereGeometry(1.5,128,128)+snoise 位移+Physical 透射 | ✅ xml:206（renderOrder 4）；几何/位移/透射/直接光/cc 全链 oracle noenv 成对差 0.49≈0（§33.1） |
| ③ | prism GLB + 8 路 facet 视频马赛克 | ✅ xml:209-247（GLTFGeometry+MeshMosaicMaterial，逐常量引 js 行号）；GLB 解析 = gltf crate（§34.3 md5 逐一相同） |
| ④ | 折射/透射材质 | ✅ r9 透射链路+RT MSAA/HalfFloat（§32）；get_transmission_sample 定稿 lod=0（§34.1-3，双线性假设证伪回滚） |
| ⑤ | droplets 3D 化 + wordmark 贴图面 | ✅ 已上盘（xml:19；wordmark 面 xml:253-316 引 js:377-410）；**droplet 盘面透射残差实测存在**（§35.2 末条：droplet-top raw 4.36/0.126）——3D 化完成但透射细节未压平，见 36.4-1 |
| ⑥ | ACES×1.08→sRGB 输出 + env IBL | ✅ xml:19-20；色彩管理修正 r8、equirect flipY 修复 r10（§33）、视频解码 BT.709 r12（§35） |

历轮 2D 模拟物（假玻璃滴 lambda、drop.png sprite、prism-crop 静态贴图、
BGFIELD、#wordmark clip-path）已随提升轮退役（xml:15-17），无残留。

### 36.2 最终验证（r12 产物读取核实，本轮实跑）

- `out/lbr-wf-r12-diff/summary.json` agg：**avg_p8 0.025749772269375852**、
  **mae_mean 1.3923848531450043**、180 帧。
- `pixel.csv` 逐帧实测：worst_p8 **0.05437355 @f35**（summary.json 同值，
  grid_max_cell [8,4] 球中部）；分段 S1 **0.04672** / S2 **0.01999** /
  S3 **0.00499** / S4 **0.00283**（与 §35.4 表逐位一致）。
- 产物生成命令 = §35.4 原样（`opencat render … out/lbr-wf-r12-final.mp4`
  + `python3 tools/k3diff.py $REF … --out out/lbr-wf-r12-diff`）。

### 36.3 门判定：**未达标**

验收门 k3diff avg_p8 ≤ 0.02；终态 0.02575，**距门差 0.0057**（−22% 才
达标）。按铁律 7 如实记录，不硬凑。量纲参考：S1 段每降 0.01 ≈ 全局
−0.0028（§33.5-4），故达标大致等价于 **S1 段（hero+droplets，0-83 帧）
再降约 0.02**。

### 36.4 剩余误差来源（全部经探针定量，非猜测）

1. **droplet 盘面透射差**（S1 结构性大头，唯一明确引擎级嫌疑）：raw 域
   oracle-vs-opencat droplet-top 差 4.36 / 0.126 恒定且与视频内容无关
   （nodrops 对照 f31 4.36→0.96 实锤，§35.3）；差图形态 = 盘面透射亮度
   系统差 + 均匀轮廓环（非位移月牙），§35.2 末条。
2. **env 残差 + 透射虚像浮点位置差**：oracle-vs-opencat f31 raw 全图
   mean 2.05（env ~1.0 + 虚像位置亚像素差为主，§35.5-3）；thickness
   语义已同构证明 r=0.9899（§34.3）——浮点级虚像位置差**不可再压**。
3. **corner 条带未归因项**（COLOR_0 顶点插值 dominant 翻转+亚像素移位）：
   对 avg_p8 贡献 <0.001 级（§34.2 末条），记录不复原。
4. mp4 编码域噪声 ~2.3 mean 混在 mp4 域度量中（§35.1-①），两侧各自
   无损 roundtrip，属度量底噪非引擎差。

### 36.5 opencat-three 管线下一步的具体行动（按期望收益排序）

1. **droplet 透射 RT 排查**（对准 36.4-1，S1 主目标）：三候选按序——
   a) 透射 RT 采样视差（droplet thickness 1.5 相对 0.1 半径的折射几何）；
   b) RT 内容在 droplet 屏区的差异（主场景渲染 droplet 前后 RT 快照逐位
   对拍）；c) attenuation 路径。复现配方：/tmp/lbr-oracle/ probe5 家族加
   `nodrops` 开关 + opencat 侧 `_p5_nodrops` 探针 XML（§34.1-2 重建法，
   §35.5-1 配方），raw 域成对归零判定后再动 lib.rs（§34.2 跨域陷阱第三次
   生效：禁 png-vs-mp4 判定）。
2. **编码侧 BT.709 标注**（generic，指标中性，§35.5-2）：encode.rs
   rgba→yuv scaler + 编码器 set_colorspace/set_color_range，使输出 mp4
   元数据与参考一致；改后必须跑一次 vfx-heygen 基线确认中性。
3. oracle 方法论延续：/tmp/lbr-oracle/{probe5.html,drive5.mjs,frames/}
   可复现 f31 任意单变量对拍（§34.5-3）；新嫌疑先成对归零再动引擎。
4. 门禁基线：core **688** / engine **100**（含 BT.709 像素锚）/
   three **12**；engine 变更轮以 `cargo test -p opencat-engine --lib` 100/0
   为准（§35.3）。

---

## 37. Workflow round 13/16 (2026-10-08) — r12 残差归因修正 + 三连负结果轮（**kept = true，行为中性：avg_p8 逐位复现 0.0257498，非新纪录**）

### 37.1 What was connected

本轮**无新接通的管线段**——是一次成对探针归因轮（依赖序六段已在 §32-35
全部接通）。产出三件：① 对 §36.4 的 S1 归因做开关对拍并**推翻**；② 对
真正的 S1 主残差连做三个修复假说，全部被同一探针证伪回滚；③ 把探针结论
固化成 scene3_glb.rs 的注释与语义测试锚（行为零变化）。触碰的仓库面：

- `crates/opencat-core/src/ir/scene3_glb.rs`（行为中性）：COLOR_0/COLOR_1
  归一化保持 gltf crate `into_rgba_f32`（`65535.0_f32.recip()` 乘倒数，
  gltf-1.4.1 src/lib.rs:774）直读；注释记录除法两次实测更差与调试场失效
  机理；测试锚改 514/65535（u16=2×257，恰在 recip trunc 翻转带上）→
  `(colors1[2][0] * 255.0) as u32 == 1`（recip 语义防回归锚）。
- `crates/opencat-three/src/lib.rs`（行为中性）：corner 保持
  `@interpolate(flat)`（:231），注释记录 smooth 尝试的实测数字与未归因
  的开放候选。
- 探针设施（/tmp/lbr-r13/，不入 git）：probe6.html（f31 隔离场景：bg+
  prism+hero+15 droplets 全开关表，js:242-331 prismMaterial verbatim，
  facet 帧用 ffmpeg 预抽 PNG 绕开 headless <video> CONTEXT_LOST）、
  probe7.html（+VIDEOID_DEBUG / RAWC1_DEBUG / smoothvarying 三个 shader
  变体，defines 经 spread 注入）、drive6/7/8.mjs（puppeteer
  chrome-headless-shell + SwiftShader，GLB base64 注入 loader.parse，
  readPixels→rgba）、_p6_*.xml（opencat 侧同构探针，资产绝对路径）。

**归因修正（对 §36.4-1 的推翻）**：f31（t=31/30，S1 段 worst 帧）
oracle/opencat 开关对拍——droplet 单变量族（thick0/attinf/ior1/notrans/
noenv/nolights/noc/spec）合计差异仅 **0.187% 全图**；nodrops 场景
（bg+prism+hero）hot ≈ full（~85098px，4.10% 全图）。S1 主残差实为
**hero 球内透射虚像 core 78996px（3.81%）+ 球缘带 6032px**；§36.4 的
droplet-top 4.36 crop 实为混入的 hero 轮廓带 + 虚像。棱线 corner 色带
±1 错位（见 37.2 残差形态表）经虚像放大是 core 差的主构成之一。

### 37.2 Negative results & pitfalls (all reproducible)

**负结果 1 — GLB COLOR_1「recip→除法」修复（两次证伪，勿再试）**：
GLB 真值 COLOR_1 = videoId×257（u16 精确编码，python 逐字节核对
20032/20032 三角形面内恒定）；gltf crate recip 在消费端
`u32(color_1.r*255.0)` 截断下使 byte∈{1,2,4} 各减 1（f32 逐 byte 模拟表：
recip 0→0, 257→0, **514→1**, 771→3, **1028→3**, 1285→5, 1542→6, 1799→7；
div 全精确）。若参考是除法语义则除法修复应大幅改善——实测相反：
- 首测（r13 内早段，探针含 droplets 污染）：13536→28864；
- 本轮干净复测（_p6_prism.xml 剥离 droplets）：13536→**36697**
  （oc_f31_prism_div.png，全图 mean 0.671→3.292），逐带横截面显示 div
  版带色仍错位。
两个「oracle=div」的直接测定尝试均失效：① f31_viddebug
（`float(vVideoId)/8.0`）首读把背景输出变换色误计为 videoId=7 大块
（背景 0x101014 经 toneMapping+sRGB 后 R≈231，231/255×8≈7.25）——
**误报**；G==0 特征掩码后可见主面 byte∈{7,3,5,6} 不含翻转带 {1,2,4}，
两假说预测相同，调试场无区分力。② f31_rawdebug（vs 传 raw
`color_1.r*255.0`、fs 出 `fract`+`floor/8`）floor 直方图 {2:509,4:7207}
表面支持 div，但与 div 探针两次实测更差矛盾，f32 二次舍入逐 byte 复核
落在噪声内——**SwiftShader unorm 的精确 f32 行为未定案**，工作假设维持
「recip 与实测一致」。落地锚：514→trunc 1。

**负结果 2 — provoking vertex 索引循环（正式排除）**：oracle 侧
flat→smooth float 变体对拍（f31_prism vs f31_prism_smooth）：
opencat vs oracle-flat 差 **0.671** mean/13536 hot，opencat vs
oracle-smooth 差 **6.080**/92571 → opencat 的 flat 语义与参考 GL flat
一致；且 COLOR_1 20032/20032 面内恒定，video_slot 的 provoking 取值
无生效面（r12 rotate_right 零变化的机制解释）。机制排除。

**负结果 3 — corner varying flat→smooth（本轮新证伪）**：js:253
`out vec3 vCornerColor` 无 flat（js:252-254 仅 vIsMainFace/vVideoId 是
flat），GLB 有 720/20032 个三角形 COLOR_0 三顶点不恒定（纯 R
(65535,0,0) dominant=0 与纯 B (0,0,65535) dominant=2 混排）——smooth
应产生跨顶点渐变。实测 recip+smooth：13536→**23783**，且带内出现
**本侧特有**的逐三角形波动（x=960 y502-527：(26,206,21)→(69,162,65)→
(27,205,6)），而 oracle 同带是**纯 KC 色** (0,195,255) 恒定——参考侧
smooth varying 实际表现为逐面恒定，与本侧 flat 形态一致。矛盾未解
（候选：vs 阶段 flat int vVideoId 参与顶点级查表的组合效应、
GLTFLoader 顶点共享语义、SwiftShader varying 处理）。已回滚
`@interpolate(flat)`。

**残差形态表（下一轮的靶子）**——x=960 列逐像素，边界 ±1px 对齐、
同带换色（ci 差 ±1）：

| 带 | oracle | opencat(flat) |
|---|---|---|
| y491-494 | KC1 绿 (27,209,6) | KC2 青 (0,195,255) |
| y502-511 | KC2 青 (0,195,255) | KC1 绿 (21,210,0) |
| y518-528 | KC2 青 (0,195,255) | KC1 绿 (21,210,0) |
| y553-555 | KC1 绿 | KC0 粉 |

CPU 反投影推演（GLB 顶点 → 0.72/YXZ(0.139π+(1-settle)0.22π,
−1.17π−(1−settle)6π)/brandMark(0,0.02,−0.35)/cam z=7 fov35，与 XML
js:499-508 公式核对一致）：(960,503) 属 byte=7 面（COLOR_0 全红
doms=[0,0,0] → 恒 ci=(0+2)%3=2 青 = oracle ✓），opencat 却显示 ci=1
——**opencat 的带-面归属与理论差一个面**；但顶点路径经核对无误
（interleaved 64B stride、location 3/4 = color0/color1 offset 32/48、
原始 indices，lib.rs:2102-2139/:2841-2870）。CPU 光栅化按 4 语义组合
复演（div/recip × smooth/flat）与三个实际渲染对拍 diff 全部 66-99
失配——**自建理论模型不足信，归因必须靠渲染端调试场**。

**坑**：
- oracle 调试场读数必须用 debug 输出的特征通道掩码（如 G==0），
  scene.background 的色经 renderer 输出变换后不是 0x101014 原值（本轮
  误报根源）。
- probe7 的 defines spread `...(cond ? {defines} : {})` 条件只判了一个
  debug 开关，rawdbg 静默失效渲染出正常图——多开关要全覆盖。
- GLTFLoader 对 integer normalized COLOR_**保留 Uint16Array+
  normalized:true**（browser 内 evaluate 实证：ctor=Uint16Array、
  normalized=true、sample=[514,...]）——unorm→f32 转换发生在 GPU
  采样，CPU 无解码；opencat 侧 gltf crate 路径语义对照的是**采样器
  实现**而非 loader。
- 探针 XML 资产相对路径在 /tmp 失效（../assets/ 解析不到），须绝对
  /home/solaren/Projects/opencat/assets/。
- f32 的 trunc 语义判据不可靠：byte=3/7 的 recip 与 div 落在 f32
  舍入的同侧（771→3/3、1799→7/7），只有 byte∈{1,2,4} 有区分力；
  8bit 输出通道也分辨不了 6.9999 vs 7.0000 的 fract（223.11 vs 223.12
  同为 223）——floor 的阶跃才行，且要防插值边缘。

### 37.3 Proof chain (all executed this round)

- 门禁：`cargo build --release` clean（`grep -cE "^error"` = 0）；
  `cargo test -p opencat-core` **688**/0（2 ignored；scene3_glb 4 测试
  全过，含新 recip 锚 `assert_eq!((colors1[2][0] * 255.0) as u32, 1)`）。
- oracle 探针：`node /tmp/lbr-r13/drive6.mjs`（f31_prism/f31_prism_smooth
  全开关注册表）与 `drive7.mjs`（probe7 调试变体）；opencat 侧
  `./target/release/opencat render /tmp/lbr-r13/_p6_prism.xml <out.mp4>`
  + `ffmpeg -vf "select=eq(n\,31)"` 抽帧；python numpy raw 域对拍。
- 关键对拍数字（本轮实测）：full/nodrops hot ~85098/≈full、droplet 族
  合计 0.187%、div 13536→36697、smooth 13536→23783、oracle-flat vs
  oracle-smooth 5.999/92422、opencat vs flat 0.671/13536。
- 渲染：`out/lbr-wf-r13-final.mp4`；diff `out/lbr-wf-r13-diff/`。

### 37.4 Measurement & ledger — kept = **true**（行为中性复现 r12）

```bash
./target/release/opencat render examples/liquid-brand-refraction.xml out/lbr-wf-r13-final.mp4
REF=/home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
python3 tools/k3diff.py "$REF" out/lbr-wf-r13-final.mp4 --out out/lbr-wf-r13-diff
```

| version | mae_mean | avg_p8 | S1(0-83) | S2(84-101) | S3(102-161) | S4(162-179) |
|---|---|---|---|---|---|---|
| r12（视频解码 BT.709，历史最优） | 1.39238 | 0.02574977 | 0.04672 | 0.01999 | 0.00499 | 0.00283 |
| **r13（归因修正+三负结果，行为中性）** | **1.39238** | **0.02574977** | 0.04672 | 0.01999 | 0.00499 | 0.00283 |

**avg_p8 = 0.025749772269375852 与 r12 逐位一致**（mae_mean
1.3923848531450043 同）；worst_p8 **0.05437355324074074 @f35**。无回归
即本轮全部价值的守门证明：三个证伪的修复假说若任何一带病落地都会推高
指标。距门 0.02 仍差 **0.0057**。ledger：round 13，kept = true（历史
最优仍是 r12 的 0.02575，两者逐位相等）。

### 37.5 Next-step suggestions（后续轮次，按依赖序）

1. **opencat 侧棱柱材质调试场（第一刀，直接定位 ±1）**：lib.rs 棱柱
   shader 加临时分支输出 R=dominant/3、G=slot/8、B=is_main（出图后
   撤），与 oracle 同款调试场（probe7.html 模板可复制）在带区对拍
   (dominant, slot) 场——残差形态表（37.2）显示带-面归属差一个面，
   该场一拍即知错在查表输入（dominant/slot）还是光栅化归属。
2. **归因修正后的 S1 策略**：droplet 透射候选（§36.5-1）降级——droplet
   自身差仅 0.187%，主靶是 hero 虚像 core 3.81%，其构成 = 棱线带 ±1
   错位（第 1 条解决后自然收敛）+ 浮点级虚像位置差（§35.2 已证不可再压）。
3. **编码侧 BT.709 标注**（§35.5-2 保持有效，generic、指标中性）。
4. 门禁基线：core 688 / engine 100 / three 12；引擎变更轮以
   `cargo test -p opencat-core` 688/0 为准。
5. oracle 方法论延续：/tmp/lbr-r13/ 全套可复现；新嫌疑先成对归零再动
   引擎（§34.2 跨域陷阱规则；本轮 raw 域对拍第四次生效）。

---

## 38. Workflow round 14/16 (2026-10-08) — 嵌套 select 恒判 2（naga 代码生成级真 bug）修复：avg_p8 **0.0248052**，新历史最优（门 0.02 未达，差 0.0048）

### 38.1 What was connected

无新管线段——本轮是 r13 §37.5-1 第一刀（opencat 侧棱柱材质调试场）的执行
与命中。产出两件：① oracle 侧 probe9 DOM_DEBUG 调试场（/tmp/lbr-r14/
probe9.html = probe7 verbatim + `R=dominant/3 平滑、G=videoId/8 flat、
B=is_main` 输出变体，drive9.mjs 驱动）；② opencat 侧 lib.rs 临时 DBG_FIELD
分支（同编码，flat 打包，三连拍：dom 场 / c0 场 / 合并场，出图后已撤净，
grep DBG_FIELD = 0）。**归因命中：r13 遗留的 corner 色带 ±1 换色机制 =
WGSL 嵌套 select 在 naga→SPIR-V（wgpu 30 同构建）下恒判 2**——引擎级真
bug，修复为 if/else 同语义改写（lib.rs:401-406，js:273 逐分支对应）。

归因链条（每步本轮实测）：
1. probe9 vs DBG_FIELD x=960 纵扫：**面归属两侧一致**（videoId 逐带相同
   5/0/2…，G 通道）——r13「带-面归属差一个面」假说排除；dominant 系统差
   （oracle 0，opencat 2，R 通道）。
2. GLB CPU 反投影（settle=0.94012 逐帧值，ry=−1.5293π）定位 (960,505)
   前沿三角形 tri#12965：COLOR_0 **纯红常量** [65535,0,0]×3（u16 raw）——
   flat 取任一顶点都应 dom=0，渲染却是 2。
3. 合并场（R=c0.r、G=f32(dominant)/3、B=c0.b）同片元 **(255,170,0)**：
   c0 纯红与 dom=2 并存——不存在能产生该组合的 c0（r=1,b=0 时 dom 必须
   0），判定链本身坏。
4. 全图普查（sliver_census.py）：opencat 红/绿/蓝 c0 的 corner 条带全部
   dom=2；oracle 同场 dom 有 0/1/2 分布。
5. 改 if/else → dom 随 c0 正确变化（红→0，与 oracle 一致）。

单层 select（同函数 is_main 行）行为正常——坑精确定位在 **select 实参
嵌套 select** 的代码生成；全仓 grep `select(select` 仅此一处。

### 38.2 Negative results & pitfalls

- **坑（记入必读）：mp4 编码污染按通道编码的调试场**。opencat 侧调试场走
  mp4 时 dom 读数 ±0.1 抖动、1-4px 窄条带色度渗色（所谓「无损 601 编码」
  对色度通道不可信）；`opencat render <xml> out.png <frame>` 单帧 PNG
  直出后读数精确（dom 字节级整数）。以后一切调试场一律 PNG，禁走 mp4。
- **坑**：CPU 反投影姿态必须逐帧算 settle（f31 settle=0.94012 而非终态
  1；ry=−1.5293π 而非 −1.17π）。且 CPU 模型只用于定位嫌疑三角形，判定以
  渲染端调试场为准（r13「自建理论模型不足信」第五次生效——本轮 CPU 曾把
  主面误判为覆盖三角形，终靠调试场纠正）。
- **修复后残余（记录不复原）**：corner 带色族已对齐（同色族、无双换色、
  边界 ±1px），带内仍有 AA/z-fight 级形态差：probe 成对（f31，无 hero/
  drops）full mean 5.24 / hot(>24) 16655，形态 = opencat MSAA4 与 oracle
  SwiftShader 在两片近共面几何（前后壳 t 差 ~0.014 的红/蓝 corner 面三角对）
  上的覆盖合成不同 + 主面视频内容 ±1px 边界。c0 场与普查证实 GLB 同位
  存在红/蓝两族 corner 面，深度决胜敏感。
- naga 嵌套 select 坑的回归锚以 lib.rs:392-400 注释承载（无 GPU 单测
  设施，opencat-three tests 为 CPU-only）；**勿「简化」回嵌套 select**。

### 38.3 Proof chain (all executed this round)

- 门禁：`cargo build --release` clean（error=0）；`cargo test -p
  opencat-core` **688**/0（2 ignored）；`cargo test -p opencat-three`
  **12**/0；`cargo test -p opencat-engine --lib` **100**/0（8 ignored）。
- oracle 探针：`node /tmp/lbr-r14/drive9.mjs`（probe9 DOM_DEBUG，
  f31_domdbg）。opencat 探针：DBG_FIELD 三连拍（dom/c0/合并）各一次
  release 构建 + `_p6_prism.xml` PNG 单帧 f31；修复后成对验证
  `./target/release/opencat render /tmp/lbr-r13/_p6_prism.xml
  /tmp/lbr-r14/oc_fix_prism.mp4` + ffmpeg 抽 f31 vs r13 oracle
  f31_prism.rgba numpy raw 对拍。
- 全量：`./target/release/opencat render examples/liquid-brand-refraction.xml
  out/lbr-wf-r14-final.mp4`；`python3 tools/k3diff.py $REF
  out/lbr-wf-r14-final.mp4 --out out/lbr-wf-r14-diff`。

### 38.4 Measurement & ledger — kept = **true**（新历史最优）

| version | mae_mean | avg_p8 | S1(0-83) | S2(84-101) | S3(102-161) | S4(162-179) |
|---|---|---|---|---|---|---|
| r12/r13（历史最优） | 1.39238 | 0.02574977 | 0.04672 | 0.01999 | 0.00499 | 0.00283 |
| **r14（嵌套 select→if/else）** | **1.34968** | **0.02480515** | 0.04498 | 0.01935 | 0.00484 | 0.00262 |

worst_p8 **0.05333043981481481 @f35**（r13 0.054374 同帧，grid_max_cell
[7,6] 球中部）。四段全部改善；距门 0.02 差 **0.0048**。ledger：round 14，
kept = true，历史最优更新为 r14（avg_p8 0.02481）。

### 38.5 Next-step suggestions（按期望收益排序）

1. **S1 剩余构成**：hero 虚像浮点位置差（§35.2 已证不可再压）+ 条带
   AA/z-fight 微形态（38.2 第三条）。后者下一刀候选 = 近共面双几何的深度
   决胜对拍（oracle 在同位渲染红族、opencat 需核实在同位收敛到同族）——
   但上限估计 <0.001 级，先用量化探针定收益再动引擎。
2. probe 设施已备：/tmp/lbr-r14/{probe9.html,drive9.mjs,cmp_field.py,
   sliver_census.py,glb_truth.py,raycast.py,scan_field.py} + r13 的
   _p6_*.xml 场景族；新嫌疑先 PNG 调试场成对归零再动引擎。
3. 编码侧 BT.709 标注（§35.5-2 保持有效，指标中性，改后需跑 vfx-heygen
   基线确认）。
4. 门禁基线：core 688 / engine 100 / three 12；引擎变更轮以
   `cargo test -p opencat-core` 688/0 为准。

---

## 39. Workflow round 15/16 (2026-10-08) — PMREM 组件级对拍：GGX L 式漏 −V（真 bug）修复→k3diff 变差→回滚 + probe6-vs-参考 +1.8 系统偏置归因修正（**kept = true，行为中性复现 r14 0.0248052**）

### 39.1 What was connected

无新管线段。本轮是 PMREM/env 组件级的成对归因轮，产出三件：
① **PMREM 组件级对拍设施**（/tmp/lbr-r15/：probe_pmrem.html +
drive_pmrem.mjs = three.js r181 `PMREMGenerator.fromEquirectangular` 的
GPU RT 整体读回（HalfFloat bits），opencat 侧临时 example pmrem_dump（已
删）；ggx_verify.py = GLSL `_getGGXShader` 的 python 逐字复刻）；② 发现
并（一度）修复 `crates/opencat-three/src/pmrem.rs` GGX 主循环 **L 式漏减
−V**（L = normalize(2·dot(V,H)·H) 退化为半向量 H 本身，three.js r181
PMREMGenerator.js `L = normalize(2.0 * dot(V, H) * H - V)`）——修复经
python 逐字复刻验证一致到 1e-5；渲染级对 probe6 反而微差（f31 full 成对
差 r14 行为 0.858 → 修复版 0.870，+0.012——与三方零偏置观测自洽：两版
本一致，修复只会偏离），且 **k3diff 全量实测变差**（avg_p8
0.02481→0.02507，回退集中 S1 +0.00051）→ **回滚**，负结果以注释承载（pmrem.rs GGX 循环处，勿再改）；
③ **probe6-vs-参考 +1.8 系统偏置**的归因修正（见 39.2），触碰面仅
pmrem.rs 注释。库行为零变化。

### 39.2 Findings, negative results & pitfalls

**归因修正（对 r13 §37.1 的修正，下一轮主靶）**：f31 三方对拍
（ref-mp4 帧 / probe6 oracle raw / opencat 渲染，python numpy sRGB 域）：
probe6 vs 参考球区 signed **+1.796**、opencat-r14 vs 参考 +1.848、
probe6 vs opencat-r14 **−0.052（零偏置）**。三个结论：
1. probe6（SwiftShader three.js r181.2，同一 node_modules lock 精确锁定）
   的重建场景比参考录制页**球区系统偏亮 1.8**——且不是 mp4 编码噪声
  （oc-r14 vs ref 双方都是 mp4，编码域相消，仍 +1.85）。r13/r14 的
  「opencat vs probe6」成对范式存在系统盲区：历史优化实际把 opencat
   对齐到了 probe6 而非参考录制页；probe6 的 +1.8 重建误差从未被归因。
2. 偏置形态：S1 全段全球区弥漫（f0-83 球区 +1.6~1.9 稳定，f90+ 球
   dock 缩小后偏置降至 +0.36），G 通道主导（+1.26 vs R +0.66 B +0.71）。
   候选（按嫌疑排序）：透射 RT 背景内容差（球盘面大部分像素是透射采样
   背景 RT，直接看背景处却零偏置——RT 内容与直接渲染路径不同源的嫌疑）；
   参考录制 renderer 参数与 probe6 假设差（preserveDrawingBuffer、swgl
   软渲染的 f16/PMREM 行为）；灯/droplet 集合重建残差。
3. PMREM 漏 −V 的修复方向（L 反射核）使球区更亮（vs 参考 +3.05），
   与参考背离——**参考录制环境的 IBL 行为 ≈ 错核版**（Firefox swgl，
   不可复刻）；k3diff 是唯一裁判，回滚。

**负结果 4 — PMREM GPU oracle 读回不可用**：SwiftShader 上
GGX 256-loop 卷积 shader（probe_pmrem.html→fromEquirectangular→
readRenderTargetPixels）512/2048 源均 >30min 不完成（puppeteer
protocolTimeout 双双击穿，1800s 也不够）——SwiftShader 对该 shader 编译/
执行病态。替代：python 逐字复刻（ggx_verify.py，与 CPU 实现对拍 1e-5）
+ 渲染级成对（probe6 XML PNG 直出 vs oracle raw rgba）。
**oracle GPU PMREM 与 CPU 的逐值一致性从未被实测**（r14 之前所有
「CPU PMREM≈three.js」的判断依据是间接的）。

**负结果 5 — 「修复正确 ≠ 对齐参考」**：pmrem.rs 漏 −V 是对照 r181
源码的确定 bug（python 逐字复刻单证），但落地后 avg_p8 0.02481→0.02507，
且对 probe6 域也微差（full 0.858→0.870）——错核版与 probe6（GPU r181）
实测一致（球区零偏置），参考录制环境同样靠近错核版。参考录制的 PMREM/IBL
数值行为偏离 r181 GPU 规范（见上），对齐裁判只能是 k3diff。已回滚；
正确式与三方数据记录在 pmrem.rs 注释。

**坑 — `-p` 范围构建不重编主 bin**：`cargo build --release -p
opencat-three` / `cargo run --example` 只构建该 crate；`target/release/
opencat`（bin）保持旧二进制。本轮曾据此误判「修复未生效」（旧 bin 渲出的
探针 PNG 与全量 mp4 逐位等于 r14）。改引擎后必须裸 `cargo build
--release`（全 workspace）再渲染。

**坑 — 探针场景选择影响归因**：`_p6_prism.xml`（prism-only）的像素 =
背景 ShaderMaterial + 视频马赛克，**不吃 IBL**——用它验证 PMREM 修复
恒为零变化；验证 IBL 改动须用 `_p6_full.xml` / `_p6_nodrops.xml`（含
hero/droplets）。

### 39.3 Proof chain (all executed this round)

- 门禁（回滚后终态）：`cargo build --release` error=0；`cargo test -p
  opencat-core` **688**/0（2 ignored）；`cargo test -p opencat-three`
  **12**/0；`cargo test -p opencat-engine --lib` **100**/0（8 ignored）。
- 组件对拍：`python3 /tmp/lbr-r15/ggx_verify.py`（修复版 dump 9 采样点
  d≤1e-5；修复前窄核级同样 1e-5、宽核级差大——与 2048 全 RT 统计一致：
  lod2+ Δmean 0.0008~0.0026、Δmax 0.19）。
- 渲染级成对（f31，PNG 直出）：`./target/release/opencat render
  /tmp/lbr-r13/_p6_{full,nodrops,prism}.xml out.png 31` vs
  `/tmp/lbr-r13/out/f31_*.rgba`。full：r14 行为（回滚版）0.858 / 修复版
  0.870；nodrops（修复版）0.823；prism 0.286（两版逐位同——prism 场不吃
  IBL，见坑）。注意 §35.3 的「full 2.052」是 **r12** 时代数字——r14 嵌套
  select 修复已把 full 成对差压到 0.86 档，勿再把 0.86 归功于 PMREM 改动
  （本轮曾误读，见 git 历史无、以本条勘误为准）。
- 全量：修复版实测 avg_p8 **0.0250701**（out/lbr-wf-r15-diff 首版，被
  回滚版覆盖）；回滚版 `./target/release/opencat render
  examples/liquid-brand-refraction.xml out/lbr-wf-r15-final.mp4` +
  `python3 tools/k3diff.py $REF out/lbr-wf-r15-final.mp4 --out
  out/lbr-wf-r15-diff` → **0.024805150998799725 与 r14 逐位一致**。
- 三方对拍脚本（f31 ref/oracle/oc）：本轮 python inline（numpy + ffmpeg
  select 抽帧），数字见 39.2-1。

### 39.4 Measurement & ledger — kept = **true**（行为中性复现 r14，负结果随行）

| version | mae_mean | avg_p8 | S1(0-83) | S2(84-101) | S3(102-161) | S4(162-179) |
|---|---|---|---|---|---|---|
| r14（历史最优） | 1.34968 | 0.02480515 | 0.04498 | 0.01935 | 0.00484 | 0.00262 |
| r15（漏 −V 修复，已回滚） | 1.35427 | 0.02507012 | 0.04550 | 0.01948 | 0.00487 | 0.00266 |
| **r15 终态（回滚=逐位 r14）** | 1.34968 | **0.02480515** | 0.04498 | 0.01935 | 0.00484 | 0.00262 |

worst_p8 **0.05333043981481481 @f35**（grid_max_cell [7,6] 球中部；本轮
回滚版全量 k3diff 实测，与 r14 逐位一致故同值）。ledger：round 15，
kept = true（行为中性逐位复现历史最优 0.024805150998799725，无回归即
守门证明——r13 先例；带病落地的「修复」若未回滚会推高指标至 0.02507，
两个坑与三个负结果见 39.2）。历史最优仍为 r14（0.02481）。距门 0.02
差 0.0048。

### 39.5 Next-step suggestions（r16 最后一轮，按期望收益排序）

1. **以参考 mp4 为唯一裁判重开 S1 残差归因**（本轮最大产出）：现有
   「opencat≈probe6」已到头（probe6 自身 +1.8 偏离参考）。第一刀候选 =
   **透射 RT 内容对拍**：probe6/opencat 各自 dump 透射 RT 快照
   （three.js 侧 transmission RT 可 readRenderTargetPixels）逐位比对，
   验证「RT 背景内容与直接渲染背景不同源」假说（39.2-1 候选首位；若是，
   参考侧 RT 的清屏/输出变换语义即靶子）。
2. **env 强度语义排查**：three.js r163+ 有 `scene.environmentIntensity`
   （默认 1）与材质 `envMapIntensity` 两级；确认参考 js/录制版本是否
   走了非默认值，opencat 是否漏乘/多乘任意一级（合法修复必须引自 js
   常量，禁离线配平——铁律 2）。
3. probe6 不再作为 S1 的绝对 oracle；成对对拍一律三方
   （ref-mp4 / probe6 / opencat），probe6 只用于组件隔离。
4. /tmp/lbr-r15/ 设施保留：probe_pmrem.html、drive_pmrem.mjs、
   ggx_verify.py（python 复刻可独立验证任何 PMREM 改动）、
   equirect_{512,2048}.png、oc_pmrem_*.bin（修复前后 dump）。
5. 门禁基线不变：core 688 / engine 100 / three 12。

## 40. Workflow round 16/16 final (2026-10-08) — rim fresnel 辉光超额定性为「env 响应 ×1.2 标量差」：合法修复空间为空，负结果轮收束（**kept = true，逐位复现 r14 0.0248052**）

### 40.1 What was connected

无新管线段。本轮选定误差源 = **hero 球 rim fresnel 辉光超额**（f31 三方对
拍中占帧误差 ~一半，S1 主导项），全程做归因：① 环带径向/p8 剖面 + 水
平扫描线逐像素形态（/tmp/lbr-r16/）；② **修复 r13 消融矩阵的重大缺陷**
——旧 `_p6_d_*` 变体只改了 droplet 材质（如 `_p6_d_nospec.xml:349` 改
的是 droplet 的 specularIntensity，hero :186 仍为 2），此前「clearcoat/
specularIntensity/transmission/thickness/attenuation/ior 无罪」的排清对
hero 全部无效；本轮重做打在 hero 上的消融（`/tmp/lbr-r16/_p6_d2_*.xml`
5 变体 + roughness 2 变体 + envMapIntensity 三点）；③ IBL 全链 chunk 级
对拍（12 项，见 40.2）。库行为零变化，触碰面仅本 handoff 与删除本轮早
期临时探针 `crates/opencat-three/examples/noise_probe.rs`（已删）。

### 40.2 Findings, negative results & pitfalls

**rim 超额的定性（本轮核心产出）**：f31 球心水平扫描线（右缘 x1332-40，
G 通道）：p6 [158,118,36,79] vs oc [170,131,47,102]——超额集中于**剪影
最后 2-4 像素**（dotNV→0 极限掠射区），单侧 +10~+23；盘内仅 +0.5~+1.3。
径向 bin 的「60px 宽环带」是剪影 wiggle 方位差（±29px）把细边缘涂抹成
宽带，非物理宽辉光。d_noenv 消融（scene 级，唯一真打到 hero 的旧变体）
可灭（ring_p8 0.2377→0.0353）→ 超额在 env IBL 加法内。

**响应定律（对下轮最有用的数字）**：环带 signed 差对 envMapIntensity 线
性：I=1.2→+10.22、0.6→−18.26、0→−47.97（斜率 ≈48.5/单位）；**零点
I≈0.99**。渲染验证：oc(I=1.0) 与 p6(I=1.2) 环带逐像素一致——ring_p8
0.2377→**0.0304**（低于 noenv 水平）、full_p8 0.0413→**0.0230**
（f31 −44%）。即 **oc 的 rim env 响应 ≈ 真 three.js ×1.2 的纯标量差**。
⚠ 该标量修复（envMapIntensity=1.0）被铁律 2 否决（js:175 = 1.2），
**未采纳**——只作为下轮定位「真 three.js 在哪一级丢了这 1.2」的靶心。

**排清清单（全部本轮对拍执行）**：① IBL 组装：`get_ibl_radiance/
get_ibl_irradiance`（lib.rs:786-807）逐字对应 vendored
envmap_physical_pars_fragment.glsl.js:4-43（pow4 mix、lod=roughness、π、
envMapIntensity 单乘）；② cubeUV 采样层（lib.rs get_face/get_uv/
bilinear/roughnessToMip/textureCubeUV）逐字对应 cube_uv_reflection_
fragment.glsl.js（含 #25071 内缩与 narrow-row 偏移）；③ **DFG LUT 数据
字节级 0 差异**（2048 u16 程序化 diff，dfg_lut.rs vs
DFGLUTData.js）；④ 过滤模式 LinearFilter/ClampToEdge（DFGLUTData.js:
53-56）与我们 smp_clamp 一致；⑤ DFGApprox/EnvironmentBRDF/
computeMultiscattering（lights_physical_pars_fragment.glsl.js:379-427）
结构一致；⑥ geometryRoughness 公式（lights_physical_fragment.glsl.js:
6-9 vs lib.rs:965-966）一致；⑦ spec_f90=2/specColor=0.0401 推导一致；
⑧ getIBLVolumeRefraction/透射链（transmission_pars_fragment 全文 vs
lib.rs:622-688）一致且内部不采 env、无第二处 envMapIntensity；
⑨ sRGB EOTF 单次解码（pmrem.rs:478-480、:442-444 精确式）；⑩ envMapIntensity
接线单乘（phys_d.x 仅进 radiance/irradiance 各一次，lib.rs:794/:806；
pmrem.rs 无烘焙）；⑪ 位移相位渲染级检验：full−ndisp wiggle 增量图
corr **0.985**、rms 18.2/17.5、均值差 0.13——无相位差；⑫ 法线：r8 死代
码分析 + 负结果在案（lib.rs:360-369），本轮不翻案。

**证伪的故事**：①「p6 边缘 derivative 饱和 roughness 压暗辉光」——固定
roughness 探针 0.126/0.2 边缘像素反而**更亮**（[170,131,47,102]→
[174,152,113,126]：模糊把天空亮部拉进掠射带），roughness 机制方向相反；
②「剪影错位」——环带单侧正差排除 ± 翻转；③ r13 旧消融的「全 Innocent」
结论（见 40.1-②，对 hero 无效）。

**不确定项（下轮首位）**：atlas sharp 级（faceSize 128 基级）在水平线方
向的内容 vs 真 three GPU PMREM——本轮快速 python 对拍被自写脚本的
getFace 分支 bug 污染（face 选择写重、天顶行 uv 落 face 0，比值无意义），
两种方向都无法确认。r15 的 9 点 1e-5 窄核级验证仍是最佳证据但采样稀疏。
**注意棱柱不采 env**（ShaderMaterial=视频马赛克面片）——scene 内采 sharp
env 的只有 hero 球与 droplets，「prism 对齐⇒env 内容对齐」的推理不成立。
r15 设施可直接复用：/tmp/lbr-r15/out/oc_pmrem_{512,2048}[_fix].bin（f16
RT dump）+ ggx_verify.py（含正确的 getFace/bilinear），把验证从 9 点扩
到水平线整环 + 天顶带即可裁决。

**门可达性评估（铁律 7 直说）**：即使 rim 超额完全修复，f31 帧误差
0.0611→~0.030；S1 均值按同比例 ~0.045→~0.025-0.030，全片 avg_p8 落
~0.022-0.025——**仍高于 0.02 门**。剩余不可动质量：盘内 +4~+6（>8 阈值
像素，oc≈p6 而 ref 系统性暗 ~2-3，§39.2 录制侧偏置）与 wiggle/细节散点。
在铁律 2 约束内（禁离线拟合常数），**0.02 门判定为大概率不可达**；r14 的
0.02481 即本路线的合理终态。

**坑——探针 XML 双材质**：`_p6_full.xml` 含 hero+droplet 两份同参数材质，
消融替换 `specularIntensity: 2,` 命中 2 处；须确认改动覆盖 hero（第一处
:186）而非只改 droplet（r13 的 d_* 变体正是只改了 droplet）。

**坑——（1−F）耦合使 spec/cc 消融读数反向**：d2_nospec/d2_nocc 的环带
signed 变得更正（+25.4/+15.1）并非「该条贡献为负」——spec=0 使透射
(1−F)·attenuated 的 F→0（透射 ×7），cc=0 使 (1−clearcoat·Fcc) 调制消失
——环带读数被透射项淹没。消融环带时只用 env 系变体（env 对透射无耦合）。

### 40.3 Proof chain (all executed this round)

- 组件对拍（本轮，命令/产物均为 /tmp/lbr-r16/ 下）：DFG 数据 diff
  （0 差异）；环带消融矩阵 d2_{nospec,nocc,notrans,envhalf,env0} +
  r{0.126,0.2} + d2_env10 共 9 次探针渲染（`./target/release/opencat
  render /tmp/lbr-r16/_p6_*.xml out.png 31`）；径向剖面/扫描线/wiggle
  相关（python numpy inline，数字见 40.2）。
- 门禁（终态）：`cargo build --release` error=0；`cargo test -p
  opencat-core` **688**/0（2 ignored）。opencat-three/-engine 未触碰
  （本轮零库代码改动），基线不重跑。
- 全量：`./target/release/opencat render
  examples/liquid-brand-refraction.xml out/lbr-wf-r16-final.mp4` +
  `python3 tools/k3diff.py $REF out/lbr-wf-r16-final.mp4 --out
  out/lbr-wf-r16-diff` → avg_p8 **0.024805150998799725**，与 r14/r15
  **逐位一致**（行为中性实测证明，非推断）。
- 临时探针清理：crates/opencat-three/examples/noise_probe.rs 已删除
  （本轮早期 snoise 复核用；复核发现旧探针 JSON 读回被 f16 量化污染，
  「max err 0.0157=编码量子」结论不可靠——探针级 snoise 验证须 RGBA32F
  读回重做，渲染级 wiggle corr 0.985 为当前最佳证据）。

### 40.4 Measurement & ledger — kept = **true**（零库改动，逐位复现）

| version | mae_mean | avg_p8 | S1(0-83) | S2(84-101) | S3(102-161) | S4(162-179) |
|---|---|---|---|---|---|---|
| r14 = r15 终态 = **r16 终态** | 1.34968 | **0.02480515** | 0.04498 | 0.01935 | 0.00484 | 0.00262 |

worst_p8 **0.05333043981481481 @f35**（grid [7,6]）。ledger：round 16（最终轮），
kept = true。历史最优仍为 r14 0.02481（16 轮全程：r1-r15 见 §39.4，
r16 中性收束）。距 0.02 门差 0.0048；可达性评估见 40.2。

### 40.5 Next-step suggestions（若路线重启，按期望收益排序）

1. **atlas sharp 级水平线整环验证**（40.2 不确定项首位）：复用
   /tmp/lbr-r15/out/oc_pmrem_512.bin + ggx_verify.py 的正确 getFace/
   bilinear，水平线环（el∈[−15°,+15°] 全方位）+ 天顶带采样，与 python
   GGX 复刻对拍——若 oc 基级在水平线 ×1.2，则修 pmrem.rs 级 0/模糊链
   （源引用 PMREMGenerator.js），这是唯一可能的合法大额修复。
2. **真 GPU PMREM oracle**：r15 负结果 4 的 SwiftShader 30min 超时可尝试
   规避——把 probe 换 headless chrome 真 GPU（--use-gl=angle 不带
   swiftshader，需机器有 GL）或把 PMREM 卷积分块提交（分 6 面 6 lod
   逐个 readRenderTargetPixels）。
3. 探针级 snoise 验证须 RGBA32F RT 重做（40.3-坑）；渲染级证据
   （wiggle corr 0.985）已足够支撑「位移无罪」。
4. 若 1、2 都排除 atlas：×1.2 只能在装配链某处——用「oc(I)=p6(1.2·I)」
   定律做二分：给 oc 加临时 debug uniform 逐项关断（radiance/cc/multi/
   irradiance 四路分别输出），与 p6 的对应量对拍。这是引擎内改探针，
   须走临时 example + 完成即删（r15 先例）。
5. 门禁基线不变：core 688 / engine 100 / three 12。

## 41. Workflow final state (2026-10-08) — 收尾汇报：threejs-like 路线六段全接通，终态 avg_p8 **0.02481**（**0.02 门未达，差 0.0048**；剩余误差归因与重启靶点见 41.3/41.4）

### 41.1 threejs-like 路线推进到哪一段——六段全部接通

用户指令路线（2026-10-06，取代 2D 模拟）的六个依赖序段逐轮接通，无一段缺口：

| 段 | 内容 | 接通轮 |
|---|---|---|
| ① | 最小 3D 场景进渲染输出（scene→mesh→camera→draw IR） | §20 = r1/12 |
| ② | hero 球 SphereGeometry(1.5,128,128)+snoise 顶点位移+基础材质 | §21 = r2/12 |
| ③ | prism GLB 网格 + 8 路 facet 视频纹理（材质/纹理完善 r4） | §22 = r3/12、§24 = r4/12 |
| ④ | 折射/透射材质（ShaderMaterial GLSL 桥 r6 端到端） | §25 = r5/12、§26 = r6/12 |
| ⑤ | droplets 3D 化 + wordmark 贴图面 | §27 = r7/12 |
| ⑥ | ⑥a ACESFilmic×exposure+sRGB OETF 输出变换、⑥b env IBL | §28 = r8/12、§29 = r9/12 |

段后提升轮把 avg_p8 从中性起点压到终态：2D 模拟物全部退役（BG/DROPS/WM
pin 翻转 + #bgcanvas，§30）→ 位移法线遮蔽修复（§31）→ 透射 RT mip 链 +
mipped bicubic + RT MSAA/HalfFloat（§32）→ equirect flipY（§33）→ 主 pass
4×MSAA（§34）→ 视频 BT.709 色彩管理（§35）→ 嵌套 select 恒判 2 的 naga
代码生成级真 bug 修复（§38，0.02575→**0.0248052** 历史最优）；§37/§39/§40
三轮中性/负结果收束（PMREM 漏 −V 修复实测变差已回滚、rim 辉光定性为 env
响应 ×1.2 标量差、合法修复空间为空）。分层纪律全程保持：Rust/JS 引擎侧只
做通用管线，liquid 专用语义全部在 XML `<script>` 经 three_api 声明，与
参考 renderAt() 结构 1:1。

### 41.2 最终验证（本轮 wrap-up 实跑复现，非转抄）

- `cargo build --release` → exit 0、error 0。
- `cargo test -p opencat-core` → **688 passed / 0 failed**（2 ignored）；
  `cargo test -p opencat-three` → **12 passed / 0 failed**；
  `cargo test -p opencat-engine --lib` → **100 passed / 0 failed**（8
  ignored）——与 §40.5 基线一致。
- `python3 tools/k3diff.py
  /home/solaren/Projects/hyperframes-launches/liquid-brand-refraction/renders/liquid-brand-refraction_2026-10-04_swgl.mp4
  out/lbr-wf-r16-final.mp4 --out /tmp/lbr-final-verify` → **avg_p8
  0.024805150998799725、mae_mean 1.3496848610039438**，与
  out/lbr-wf-r16-diff/summary.json 逐位一致（worst_p8
  **0.05333043981481481 @f35**，grid_max_cell [7,6] 球中部；分段
  S1(0-83)=0.04498 / S2(84-101)=0.01935 / S3(102-161)=0.00484 /
  S4(162-179)=0.00262，由 summary.json rows 复算）。

16/16 轮 ledger 全部 kept = true；工作树无遗留临时探针（noise_probe.rs
r16 已删）；本收尾轮零代码改动。

### 41.3 门判定：未达 0.02，剩余误差来源

**0.02481 > 0.02，门未达，差 0.00481。** 且按 §40.2 可达性评估，铁律 2
（禁离线拟合常数）约束下判定为**大概率不可达**：即使 rim 辉光超额完全修
复，全片 avg_p8 也只落 ~0.022-0.025。r14/r16 终态 0.02481 即本路线合理
终态。剩余误差来源（按贡献排序）：

1. **hero 球 rim fresnel 掠射区 env 响应 ×1.2 标量差**（S1 主导，f31 帧
   误差约一半）：oc 的 rim env 响应 = 真 three.js ×1.2 的纯标量差（零点
   I≈0.99；oc(I=1.0) 与 p6(I=1.2) 环带逐像素一致）。数值上 ×1.2 的来源
   就是 js:175 envMapIntensity=1.2 源常量——oc 在装配链某处「丢了」这一
   级，但把 I 配平到 1.0 属离线拟合，被铁律 2 否决；合法修法只有找到丢
   级位置（见 41.4-1/4）。
2. **参考录制侧系统偏置**（不可复刻）：参考录制页（Firefox swgl）比
   probe6（SwiftShader three.js r181.2 同 lock 重建）球区系统性暗 ~1.8
   （G 通道主导）；盘内 oc≈p6 而 ref 暗 2-3 → >8 阈值像素盘内 +4~+6。
   参考录制环境的 IBL/PMREM 数值行为偏离 r181 GPU 规范（r15 负结果 5）。
3. **PMREM atlas sharp 级（faceSize 128 基级）水平线内容未实测**
   （r16 首位不确定项）：r15 的 9 点 1e-5 验证采样稀疏；快速 python 对
   拍被脚本自身 getFace 分支 bug 污染，两方向都无法确认。
4. **wiggle/细节散点残差**：位移相位渲染级 corr 0.985（§40.2-⑪），位移
   本身无罪；残差为细节级散点，worst 帧 f35 即 rim+盘内偏置的合成。

### 41.4 opencat-three 管线下一步的具体行动（若路线重启，按期望收益排序）

1. **atlas sharp 级水平线整环验证**（41.3-3 裁决，唯一可能的合法大额修
   复）：复用 /tmp/lbr-r15/out/oc_pmrem_512.bin + ggx_verify.py（含正确
   getFace/bilinear），水平线环 el∈[−15°,+15°] 全方位 + 天顶带采样，与
   python GGX 复刻对拍；若 oc 基级在水平线 ×1.2，则修 pmrem.rs 级 0/模糊
   链（源引用 PMREMGenerator.js）。
2. **真 GPU PMREM oracle**：r15 负结果 4 的 SwiftShader 30min 超时规避
   ——headless chrome `--use-gl=angle`（不带 swiftshader，需机器有 GL）
   或把 PMREM 卷积分块提交（6 面 ×6 lod 逐个 readRenderTargetPixels）。
3. **×1.2 丢级二分定位**：用「oc(I)=p6(1.2·I)」定律 + 临时 debug uniform
   逐项关断（radiance/clearcoat/multiscattering/irradiance 四路分别输
   出）与 p6 对拍；引擎内改探针须走临时 example + 完成即删（r15 先例）。
4. 探针级 snoise 验证 RGBA32F RT 重做（r16 坑；渲染级 corr 0.985 已足够
   支撑「位移无罪」，优先级最低）。
5. 门禁基线不变：`cargo build --release` error=0；core 688 / engine 100
   / three 12（本轮 41.2 实测通过）。
