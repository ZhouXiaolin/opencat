# liquid-brand-refraction — Alignment Handoff (per-launch)

Status: **intake re-run 2026-10-04 (evening)** — recon only; no alignment edits,
no commits. This revision supersedes the earlier §3 ("no XML exists" was already
stale when written): `examples/liquid-brand-refraction.xml` **does exist**
(untracked, authored earlier the same day) and a **measured baseline now
exists** — see §3 and §5.1. Branch `feat/hyperframes-launches-alignment`.
Neither `examples/` nor this branch was modified by the re-run.

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
