# vfx-heygen-combined — Alignment Handoff (per-launch)

Status: **intake re-run 2026-10-05** — recon only; no alignment edits, no commits.
This supersedes the task's assumption that the provided render is a usable
reference. Branch `feat/hyperframes-launches-alignment`, tip `0429d54`.
Nothing under `examples/` was modified; no commit was made.

> **Blocking finding up front.** The "reference video" the task pointed at
> (`renders/vfx-heygen-combined_2026-10-02_10-26-51.mp4`) is **degenerate**:
> every WebGL / 2D-canvas layer failed to composite under headless Chrome's GPU
> path, exactly the failure documented for `liquid-brand-refraction`. Measured
> state of the original: frames 0–87 pure black (`rgb 3,3,3`), frames 88–191
> the VFX card DOM over a blank/white canvas (no ray-march logo, no refraction
> background), frames 192–403 flat white (`rgb 253,255,255`, no phone/prompt
> content), frames 405–490 flat light-cream, and frames 512–787 the CTA text
> frozen with **no portal reveal** (the interval 512–654 is one byte-identical
> frame, 143 frames long; 655–777 another single frozen frame). A **valid
> reference was regenerated** this intake with software GL and saved beside it —
> see §1.1. Use the regenerated file, not the original.

Reference project: `/home/solaren/Projects/hyperframes-launches/vfx-heygen-combined/`
Original (broken) video: `renders/vfx-heygen-combined_2026-10-02_10-26-51.mp4`
Regenerated (valid) video: `renders/vfx-heygen-combined_2026-10-05_swgl.mp4`

---

## 1. Reference spec (ffprobe, measured)

| Field | Value |
|---|---|
| Resolution | 1920 × 1080 |
| Frame rate | 30/1 fps (CFR) |
| Frames | 788 (`-count_frames`) |
| Duration | 26.2667 s |
| Audio | **none** — `nb_streams=1`, video only (silent) |
| pix_fmt / codec | yuv420p, h264 High |
| Composition id | `vfx-heygen-combined` (root, `data-duration="26.26"`) |

Frames ↔ seconds: `frame = round(sec * 30)` (0-based). 788 frames = 26.267 s.
There is **no audio track**, so no A/V offset to reconcile.

### 1.1 Reference validity — regenerated

`renders/vfx-heygen-combined_2026-10-02_10-26-51.mp4` (8.4 MB) is the broken
render. Evidence (measured this intake):

- frames 0–87 byte-identical (88 × `rgb(3,3,3)`) — the VFX logo sting is black.
- frames 88–191: DOM card only; the ray-march background (`bg-2.mp4` refraction
  field, visible in the asset) never composites — frame 150 shows only "L-in
  /vas" glass type over **white**.
- frames 192–403: flat white — the Three.js iPhone / `bg.mp4` never render.
- frames 512–654: single frozen frame (143 f); 655–777: single frozen frame
  (123 f) — the portal reveal advances `portalRadius` every frame but the GL
  never draws, and the screenshot dedupe then emits identical bytes.

Root cause is the same as `liquid-brand-refraction` (§1.1 there): the headless
renderer loses the GL context on this machine (`WebGL CONTEXT_LOST` /
`THREE.WebGLRenderer: Context Lost`). This composition is *more* GL-dependent —
it drives **four** GL surfaces: the Three.js theater (`#theater`), the VFX
ray-march fragment shader (`#vfx-canvas`), the chromatic RGB `EffectComposer`,
and the portal shader.

**Regenerated valid reference** (done this intake):
```bash
cd /home/solaren/Projects/hyperframes-launches/vfx-heygen-combined
node ~/.npm/_npx/110f701c48e68d66/node_modules/hyperframes/bin/hyperframes.mjs \
  render . -o /tmp/vhc-render/vhc_sw.mp4 --workers 1 --no-browser-gpu
# → 788 frames, 96 MB, 4m49s; log: "software gpu", forceScreenshot
cp /tmp/vhc-render/vhc_sw.mp4 renders/vfx-heygen-combined_2026-10-05_swgl.mp4
```
Result: **195/197 sampled frames distinct (every 4th)** vs. the original's ~34/79
and frozen back-half. Same 1920×1080@30, 788 f, 26.267 s, no audio. **That is
the reference for all k3diff measurement.** One caveat: the SW render logs
`[media_load_failed] image media failed to load before capture` — this is the
missing `captures/heygen-mobile/screen-strip.png` (§2.4), so the *phone screen
texture* is blank in the reference too; everything else (logo, cards, phone
mesh, prompt, all three shaders) is present.

---

## 2. Reference structure map

`index.html` is the root composition and embeds one sub-composition. There are
**two macro-sections**:

- **VFX intro** (0–6.48 s, frames 0–194): a self-composited sub-composition
  `data-composition-src="compositions/vfx-text-cursor.html"` that renders its
  own paused GSAP timeline from t=0.
- **HeyGen sequence** (~5.58–26.27 s, frames ~171–788): the parent timeline.
  The parent body builds its beats at absolute 0–21 s then does
  `tl.shiftChildren(HEYGEN_START=5.58, true, 0)` — so **every parent beat time
  = script time + 5.58 s**. This model was cross-checked against the SW render
  and matches within a frame.

### 2.1 Root composition layers (index.html)

| Layer | id / node | z | window | role |
|---|---|---|---|---|
| VFX intro sub-comp | `#vfx-intro` (1920×1080, overflow allowed) | 12 | 0–6.48 s | `compositions/vfx-text-cursor.html` |
| studio bg video | `#bg-video` (`assets/bg.mp4`) | 0 | 5.58–10.58 s | HeyGen studio background |
| 3D theater | `#theater` (canvas) | 1 | 5.58–~10.1 s | Three.js iPhone render |
| finger overlay | `#finger-overlay` (`assets/pointing-finger-clean.png`) | 2 | ~6.5–8.1 s | pointing/swipe hand |
| prompt scene | `#prompt-scene` | 3 | ~9.66–13.5 s | HeyGen-style composer UI |
| game-change copy | `#game-change-copy` | 5 | ~12.5–13.5 s | "You can make this too" |
| chromatic split | `#chromatic-split` + canvases | 7 | 13.5–16.5 s | WebGL RGB plane split + bloom |
| upgrade CTA | `#upgrade-cta` | 8 | 16.5–21.9 s | "Full catalog / available now" |
| portal logo frame | `#portal-logo-frame` (`assets/hyperframes-dark.svg`) | 9 | 17.0–26.3 s | white end card |
| portal canvas | `#portal-canvas` | 10 | 17.0–26.3 s | portal reveal shader |

### 2.2 Act window table (empirical, SW render)

Frame windows measured from the valid render (luma profile + contact sheets);
the "script t" column is the parent-timeline value (= abs − 5.58) where useful.

| Act | window (f) | window (s) | content | driver |
|---|---|---|---|---|
| V0 | 0–5 | 0.00–0.17 | black (pre-load) | — |
| V1 logo sting | 6–87 | 0.20–2.90 | `bg-2.mp4` refraction rays + HyperFrames wordmark, 112-sample ray-march fragment | `state.logoY` 760→0, `effectMix` 0→1, `progress` 0→1, `logoScale`→0.4 |
| V2 HTML-in canvas | 88–159 | 2.93–5.30 | glass `#next-card` (backdrop-blur), "HTML" types in, "canvas" slides | `addTextCardSequence`, `entryEase=-0.806t²+1.806t` |
| V3 Now natively supported | 160–179 | 5.33–5.97 | availability lockup hold | static (phrase B) |
| V4 phone entrance | 180–250 | 6.00–8.33 | Three.js iPhone over `bg.mp4`, finger enters, screen scrolls | `S.phY` −3.45→0.02, `S.cZ`, `S.scroll` 0→0.9 |
| V5 phone spin+push | 251–297 | 8.37–9.90 | `phRY` PI→5π spin, push-in `cZ`→3.85, `phRZ`→π/2 | S.phRY/phRZ/cZ |
| V6 phone exit→prompt | 298–340 | 9.93–11.33 | theater x→−1220 + blur, prompt-scene in, types `npx hyperframes add iphone-canvas` | `phoneExit@4.04`+5.58, `promptTransition@4.08`+5.58 |
| V7 send click | 341–370 | 11.37–12.33 | composer clears, `.prompt-send` scales ×3, cursor clicks, bounce | `@5.69…6.59`+5.58 |
| V8 you can make this too | 370–400 | 12.33–13.33 | light copy | `#game-change-copy` `@6.96`+5.58 |
| V9 chromatic RGB split | 405–495 | 13.50–16.50 | dark 3-plane RGB split + bloom, camera orbit | `chromaticState` `@CHROMATIC_START=7.92`+5.58 |
| V10 CTA + portal | 497–655 | 16.57–21.83 | "Full catalog / available now" on black + portal reveal growing | `portalState` `@PORTAL_START=11.44`+5.58 |
| V11 end card | 655–788 | 21.83–26.27 | white frame, `hyperframes-dark.svg` + url | portal `portalRadius`→1.8 |

Act boundaries are **hard cuts only at the two sub-composition seams**; within a
macro-section they are GSAP timeline beats. The only true cuts are V1→V2
(x=2.9), V3→V4 (phone enters while VFX fades), and V9↔V10 (RGB→CTA).

### 2.3 VFX sub-composition (`compositions/vfx-text-cursor.html`)

`data-composition-id="vfx-text-cursor"`, 1920×1080, local 0–6.48 s. One GSAP
timeline (`window.__timelines["vfx-text-cursor"]`), paused, `onUpdate: render`.
Layers: `#next-bg` (`assets/bg-2.mp4`, video, plays 3.1–6.48), `#vfx-canvas`
(WebGL ray-march), `.stage-logo-wrap` (inline SVG wordmark, 263×79 viewBox
scaled to 1080 wide, centered), `#next-card` (glass card) with `#phrase-a`
("HTML-in canvas") and `#phrase-b` ("Now / natively / supported.").

Key timeline (local seconds):
- `state.logoY` 760→0, `power4.out`, `0→1.0`
- `state.effectMix` 0→1 `power3.out` @0.72 (0.34 s)
- `state.progress` 0→1 `none` @0.72 (2.28 s) — drives the ray-march cursor
- `state.logoScale`→0.4 `expo.in` @2.9; `#vfx-canvas` opacity→0 blur→16 `expo.in` @2.9
- `#next-bg`/`#next-card` set visible @3.1; card `scale` 1.45→0.88 `entryEase` (0.9 s)
- `addTextCardSequence` @3.15: types "HTML" one glyph / 0.085 s, `html-caret`
  tracking; then `htmlWord` slides to 0 and `supportingWords` (`-in`, `canvas`)
  enter `expo.out` 0.42 s staggered 0.08
- phrase A exits x→−1480 @~5.6; phrase B enters x 1480→0 `expo.out` @~5.96
- final bridge (handoff to HeyGen): `#next-card` y→−1180 blur→24 `power2.in`
  @5.85; `#next-bg` opacity→0 @5.99; card opacity→0 @6.15

Fonts: Inter, **"Big Shoulders Display"** (HTML word, 900 uppercase),
**"DM Serif Display" italic** (natively, 304 px). Colors: card yellow `#f6ff7a`,
white `#ffffff`, glows cyan `rgba(73,242,255,…)` / magenta `rgba(255,79,216,…)`.

### 2.4 Assets / fonts

- `assets/bg.mp4` — 1920×1080, 5.0 s, 30 fps (HeyGen studio bg for the phone).
- `assets/bg-2.mp4` — 1920×1080, 5.0 s, 30 fps (VFX refraction/rays background).
- `assets/pointing-finger-clean.png` — 716×1498 (used; `-extended` 716×3298 unused).
- `assets/hyperframes-dark.svg` — 263×79, black wordmark + cyan→green gradient
  mark (the end-card logo; used via `<img>`). `assets/hf-logo.svg` is a near-twin.
- `models/iphone.glb` — 866 KB, DRACO, GLTFLoader (Three.js theater).
- `captures/heygen-mobile/screen-strip.png` — **MISSING.** `index.html`
  `#preload`s it and `drawHeyGenPage` slices it to texture the phone screen;
  `captures/` contains only `.gitkeep`. The file was never committed. The
  reference's own phone screen is therefore blank (SW log `media_load_failed`).
- Fonts (Google, per `compositions/vfx-text-cursor.html`): **Big Shoulders
  Display** 800/900, **DM Serif Display** italic, Inter, JetBrains Mono.
- No audio asset in this project.

---

## 3. opencat targets (repo-relative) — current state

**No opencat XML exists for this launch.** `examples/` has `k3-promo.xml`,
`liquid-brand-refraction.xml`, `website-to-hyperframes-act{1,3,4}.xml`, etc.,
but nothing matching `vfx-heygen*`. This intake creates no XML.

Proposed target split (3 XMLs — see §5 for the rationale). The reference is
26 s with hard macro-seams and three entirely different render technologies per
macro-section, so a single XML would be a ~2000-line monolith; the repo already
prefers per-act XMLs for multi-clip films (`website-to-hyperframes-act*.xml`).

| Target XML | reference frames | window (s) | macro-section |
|---|---|---|---|
| `examples/vfx-heygen-actA.xml` | 0–194 | 0.000–6.467 | VFX intro (logo sting + HTML card + availability) |
| `examples/vfx-heygen-actB.xml` | 186–400 | 6.200–13.333 | HeyGen phone + prompt + send + "you can make this too" |
| `examples/vfx-heygen-actC.xml` | 400–788 | 13.333–26.267 | chromatic split + CTA + portal reveal + end card |

Overlaps (actA 0–194 / actB 186–400 / actC 400–788) are intentional ~8-frame
handles at the seams so the cuts can be trimmed without losing frames; each XML
should declare `duration` equal to its own window and be diffed against its own
ffmpeg-cut reference segment (§7).

---

## 4. Engine gap list (opencat vs this reference)

The reference is **four WebGL surfaces + a Three.js GLB + a video texture +
DOM**, over a 26 s GSAP timeline. Gaps, most severe first:

- **G1 — 3D geometry + GLB (blocking, V4–V6).** `models/iphone.glb` loaded via
  GLTFLoader, DRACO, Box3-framed, materials swapped to a screen texture; a
  PerspectiveCamera (fov 34) orbits it. opencat has **no 3D pipeline** — no
  mesh/vertex/normal/camera, and no GLB element (`liquid` G6). Fallback: a 2D
  "fake iPhone" (rounded-rect CSS body + screen `<image>`/`<video>`), with the
  spin/push faked as a CSS 3D `rotateY`/`scale`/skew tween. Expect this to be
  the dominant per-frame error in V4–V6.
- **G2 — ray-march logo fragment shader (V1).** The VFX logo is a 112-iteration
  screen-space ray-march (`#define SAMPLES 112.0`) into a signed-distance logo
  mask, with spectrum/grain/spectral sweep. opencat's authoring surface is the
  **Effect Lambda DSL** (`CK.Effect.fromLambda`, whitelist + SKSL/CPU lowering);
  it is per-pixel 2D and supports `uv`, `image.eval(uv)`, math, `mix`,
  `smoothstep`, `byte`, float/float2..4 uniforms — but **no `for` loops / fixed
  sample counts** were demonstrated in the `liquid` port. Options: (a) unroll a
  reduced-sample raymarch inside one lambda if the DSL allows a bounded loop;
  (b) approximate the logo bloom entirely with layered radial-gradient glows +
  `blur` over the wordmark `<image>`. **Do not bake a PNG filmstrip** (methodology
  violation, rejected in `liquid` §4 G9).
- **G3 — chromatic RGB split + `UnrealBloomPass` (V9).** Three RGB planes with
  additive blending, camera orbit, ACES-free tone mapping, and a bloom pass.
  opencat has **no post-processing composer / bloom**. Approximate with an
  effect-lambda: 3 offset channel samples of `#game-change-copy`'s still frame,
  additive composite, plus a separate blurred copy for glow. Colour/tone will
  diverge (no ACES) — accept.
- **G4 — portal reveal shader (V10).** `portalRadius`/`edgeWidth`/`chromatic`
  drive a `smoothstep` ring with `hash`/`noise` edge jitter, chromatic edge
  offset, and spark glints, over a `tCanvasTexture` of the CTA. This is 2D
  per-pixel and **portable to an effect lambda** (noise, smoothstep, pow all
  supported). Low risk once the DSL noise helpers are available.
- **G5 — video-textured phone screen with scroll (V4).** The phone screen is a
  `CanvasTexture` of a sliced capture strip, scrolled by `S.scroll`. In opencat
  the screen can be a plain `<video>` (`assets/bg.mp4` is the *background*, not
  the screen) or an `<image>`; the missing capture strip (§2.4) means the screen
  content must be **re-authored** (draw the HeyGen page as `text`/`div` inside
  the phone screen, as the fallback `drawHeyGenPage` in `index.html` already
  does) or a new strip captured.
- **G6 — `backdrop-filter` glass card (V2).** `#next-card` uses
  `backdrop-filter: blur(14px) saturate(1.12)`. Verify opencat's backdrop-blur
  support (`liquid` §4 lists it as supported); else approximate with a
  semi-transparent gradient + a blurred copy of the background behind it.
- **G7 — `filter: blur()` on DOM nodes mid-tween.** Widely used here (theater
  exit, prompt exit, node entry). opencat supports `blur` as a style prop
  (`script/recorder/store.rs`), so this should port as `ctx.to(node,{filter:
  'blur(Npx)'})` — confirm against an act.
- **G8 — missing asset (§2.4).** `captures/heygen-mobile/screen-strip.png` does
  not exist. Nothing to port for the phone screen until it is regenerated.
- **G9 — fonts.** Big Shoulders Display and DM Serif Display are not in
  `assets/` (only Inter + JetBrainsMono). Either vendor the two fonts into
  `assets/` + `<font>` declarations, or substitute and accept glyph error in V1/V2.

**Elements/assets opencat *does* support** (from existing examples): `video`
(`path`/`url`, `data-start`/`data-duration`/`data-media-start`, `object-cover`),
`image` (png/svg), `div`/`text`, `canvas`, `lottie`, `ctx.timeline()`,
`ctx.splitText`, `ctx.createSurface`/`runEffect`/`scanPass`, the Effect Lambda
DSL, CSS `filter: blur()`, backdrop-blur, box-shadow, clip-path, and Tailwind
utility classes.

---

## 5. Alignment plan & acceptance gates

Gate (task default): `avg_p8 ≤ 0.02`, `worst_p8 ≤ 0.08`. **Honest forecast: the
0.02 gate is not reachable for this title.** The reference is four WebGL shaders
plus a physically-lit GLB phone over a 26 s timeline; even the repo's *aligned*
shader-free acts sit at p8 ≈ 0.03–0.04 (`website-to-hyperframes` §4), and the
recognised floor for any shader/3D reference is well above 0.02. Recommendation:
render first, measure, then **recalibrate the title gate to the measured floor**
(exactly as `liquid` §5 and w2h §4 did) rather than chase 0.02. A realistic
per-act target is `avg_p8 ≤ 0.10–0.18` with a stable (non-frozen) back half.

Priority order (highest value first):

1. **Static composition of Act B first** (phone + prompt), because the original
   reference's *middle is entirely flat white* — the original gives us no
   signal there, so the regenerated SW reference is the only target, and Act B
   has no GL except the phone mesh. Nail the 2D fake-iPhone, the prompt UI
   (Tailwind), the typing command, and the send-click. This alone moves the
   whole-film p8 a lot.
2. **Timing parity across all acts** — mirror the +5.58 s shift and the
   §2.2 boundaries with `ctx.timeline()` labels. Phase timing is free accuracy.
3. **Act A** — V2 HTML card + V3 availability are pure DOM/Tailwind and cheap;
   V1 logo is the hard part (G2) — do the DOM card first, the sting last.
4. **Act C shaders** — CTA text (cheap), RGB split (G3 approximation), portal
   reveal (G4, portable). End card (V11) is a static white frame + SVG `<image>`
   + mono text — should be near-exact and is a good early win.
5. **Phone screen content** — regenerate `screen-strip.png` or re-author the page
   in the phone screen (§2.4/G5); the reference phone screen is blank, so this
   is low-ROI for matching the reference but needed for the piece to read.

Non-goals: no bit-exact glass/ray-march; no GLB; no ACES tone mapping; no bloom
composer (approximations only).

Tailwind-first is the repo rule (static styling in `class=`, motion in the
script layer) — see `liquid` §6 for the enforced conventions.

---

## 6. How to measure

Reference (**use the regenerated valid file, not the original**):
```bash
REF=/home/solaren/Projects/hyperframes-launches/vfx-heygen-combined/renders/vfx-heygen-combined_2026-10-05_swgl.mp4
```

Build + render an act (build first: `cargo build --release`):
```bash
cd /home/solaren/Projects/opencat
./target/release/opencat render examples/vfx-heygen-actB.xml out/_vhc-actB.mp4
```

Whole film (only meaningful once all acts render into one timeline; otherwise
measure per act):
```bash
python3 tools/k3diff.py "$REF" out/_vhc.mp4 --out out/_vhc-k3diff
# read out/_vhc-k3diff/summary.json -> agg.mae_mean / agg.avg_p8 ; pixel.csv -> per-frame
```

Per-act cut — cut **both** the reference and the opencat render to the same
frame window, then diff. Frame windows (§2.2/§3), 30 fps, no temporal offset:
`V1 6–87`, `V2 88–159`, `V3 160–179`, `V4 180–250`, `V5 251–297`,
`V6 298–340`, `V7 341–370`, `V8 370–400`, `V9 405–495`, `V10 497–655`,
`V11 655–787`. Example for the macro-split act windows:
```bash
# act B = frames 186..400
ffmpeg -y -v error -i "$REF" -vf "select='between(n\,186\,400)',setpts=PTS-STARTPTS" -an \
  -c:v libx264 -preset fast -crf 18 out/_vhc-ref/actB.mp4
ffmpeg -y -v error -i out/_vhc-actB.mp4 -vf "select='between(n\,0\,214)',setpts=PTS-STARTPTS" -an \
  -c:v libx264 -preset fast -crf 18 out/_vhc/actB.mp4
python3 tools/k3diff.py out/_vhc-ref/actB.mp4 out/_vhc/actB.mp4 --out out/_vhc-actB-k3diff
```

Web oracle (optional — most of this composition is WebGL, so the browser oracle
is only a cross-check for the DOM-only acts A/B):
```bash
./target/release/opencat-web-compare examples/vfx-heygen-actB.xml \
  --out-dir out/_vhc-web --interval-secs 0.5 --reference "$REF"
```

Artifacts live under `out/` (gitignored). Nothing in `examples/` was modified
and nothing was committed by this intake.

---

## 7. Checklist for the next (alignment) run

- [x] Regenerate a valid reference with `--no-browser-gpu` → §1.1. **Use
      `renders/vfx-heygen-combined_2026-10-05_swgl.mp4` as `REF`.**
- [x] No `examples/vfx-heygen*.xml` exists (§3); this intake created none.
- [ ] Decide single-XML vs the proposed 3-act split (§3); recommended: split.
- [ ] Vendor Big Shoulders Display + DM Serif Display into `assets/` or pick
      substitutions (G9).
- [ ] Regenerate `captures/heygen-mobile/screen-strip.png` or re-author the
      phone-screen page (G5/G8); the reference phone screen is blank.
- [ ] Build **Act B** static first (fake iPhone + prompt UI + typing + send) and
      measure; then timing parity; then Act A; then Act C shaders.
- [ ] Recalibrate the gate (§5) to the measured floor before iterating; 0.02 is
      not the right target for a four-shader + GLB title.
- [ ] Keep Tailwind-first for static styling; motion stays in script; GLSL stays
      in Effect Lambdas, never baked filmstrips (G2).

---

## 8. Round 1 results (2026-10-05) — XMLs created, whole-film baseline measured

Round 1 built the three acts from scratch (no XML existed) and took the first
whole-film measurement. **Branch `feat/hyperframes-launches-alignment`, no
commit** (per task step 8). Files added (working tree):

- `examples/vfx-heygen-actA.xml` (dur 6.4667 s, ref frames 0-194)
- `examples/vfx-heygen-actB.xml` (dur 7.2 s, ref frames 186-399)
- `examples/vfx-heygen-actC.xml` (dur 12.9667 s, ref frames 400-787)
- `assets/vhc/` — bg.mp4, bg-2.mp4, finger.png, hyperframes-dark.png,
  sting-logo.png, cursor.png, BigShouldersDisplay-900.ttf,
  DMSerifDisplay-Italic.ttf (all reused from the reference project / cairosvg
  rasterisation of its brand SVGs; nothing retyped).

### 8.1 Measured baseline (k3diff, per act vs ffmpeg-cut ref segment)

`REF = renders/vfx-heygen-combined_2026-10-05_swgl.mp4`; each act cut to its
own window and diffed against its own opencat render (`tools/k3diff.py`).

| act | frames | mae | avg_p8 | avg_p32 |
|---|---|---|---|---|
| A (VFX intro) | 195 | 57.18 | **0.7325** | 0.5494 |
| B (phone+prompt) | 214 | 30.54 | **0.5850** | 0.3650 |
| C (chromatic+CTA+portal) | 388 | 19.14 | **0.1939** | 0.1315 |
| **whole film (frame-weighted)** | 797 | | **0.4307** | |

Gate `avg_p8 ≤ 0.02` is **not met** (as forecast in §5). The whole-film number
is dominated by Act A's V1 sting (frames 30-115 p8 ≈ 0.87-0.96), which is one
full-frame ray-march field — see 8.3.

### 8.2 What was ported 1:1 (and matches)

- **Act C is the big win (p8 0.194).** The CTA ("Full catalog / available now",
  frames 497-560) and the white end card (logo + mono url, frames 655-787) are
  crisp DOM and land at p8 0.006-0.06. The portal reveal was fitted to the
  reference's measured white-fraction curve (piecewise radius table in the XML),
  giving p8 ≈ 0.05-0.5 across the reveal instead of the initial 0.98.
- **Act B's DOM phase** (prompt UI, typing `npx hyperframes add iphone-canvas`,
  send-button scale/click, "You can make this too") matches at p8 0.006-0.13
  (frames 380-400 of the ref). The residual is entirely the 2D phone
  approximation (frames 186-330).
- Timing parity: every beat uses local = source script time + OFFSET
  (actA offset 0; actB offset −0.62 = HEYGEN_START 5.58 − 6.20; actC offset
  −7.753) so the `shiftChildren(5.58)` structure is preserved.

### 8.3 Engine gaps that blocked better numbers (negative results)

- **V1 ray-march (Act A, dominant error).** The reference is a 112-sample
  screen-space ray-march into a signed-distance logo mask that fills the whole
  frame with a bright spectrum bloom. opencat's effect-lambda DSL has **no
  loop** (`parse.rs` has no `for`/`while`), and the subtree-child sampling
  primitive (`canvas.getSubTree().makeShader` + `image.eval`) returned a flat
  field in this build (verified in isolation: sampling the rasterised wordmark
  via a subtree child produced no ink; the image renders fine as a plain
  `<image>`). So V1 is approximated as (a) an analytic anamorphic streak on a
  canvas + (b) the rasterised wordmark `<image>` on top. The wordmark is placed
  correctly and the streak colour/geometry is close at f60-90, but a full-frame
  bright ray field is simply not reproducible this way → p8 ~0.9 in V1. **The
  only path to <0.2 here is an unrolled/bounded-loop lambda in the engine.**
- **3D phone (Act B, frames 186-330).** No GLB/3D pipeline; the iPhone is a
  rounded-rect + re-authored HeyGen screen, and the spin is a 2D `scaleX` fake.
  The reference's phone is a physically-lit GLB over `bg.mp4`; p8 0.9 for the
  entrance, dropping to ~0.13 once the prompt scene takes over. Expected.
- **Chromatic RGB split (Act C V9)** is three offset colour `<text>` copies on
  black (no `<canvas>`/`<svg>` and only `inset()` clip-path are supported); it
  reads structurally but the additive bloom/ACES tone is missing (p8 0.3-0.4
  across frames 0-90).
- Markup constraints found this round: the parser rejects `<span>`, `<svg>`,
  inline `style=`, boolean video attrs (`muted playsinline`), and requires an
  `id` on every `<div>`/`<text>`. Cursor/send-arrow artwork is rasterised PNG or
  a text glyph. `boxShadow` and JS-function eases are unsupported in script
  tweens (`ease:'none'` must be `'linear'`; `entryEase` approximated by
  `power2.out`).
- `backdrop-blur-[14px]` is reported unsupported on a node (so the glass card
  has no blur); the class-loading is otherwise fine.

### 8.4 Next-round suggestions (highest ROI first)

1. **Act A V1**: add a bounded-loop (or unrolled) primitive to the effect-lambda
   DSL so the 112-sample ray-march can run, with the wordmark as a subtree
   child. This is the single biggest lever (Act A is 0.73 of the 0.43).
2. **Act B phone**: accept the 2D fake or invest in a minimal 3D/GLB element;
   the DOM half is already good. Improve the phone screen content contrast to
   match the ref's white-card look (out of scope for the byte floor).
3. **Act C chromatic**: replace the 3-text approximation with an additive-blend
   effect-lambda over the "You can make this too" still (3 offset samples +
   bloom), now that the portal lambda pattern is proven.
4. Recalibrate the title gate to the measured floor (§5) before iterating;
   0.02 is unreachable for this four-shader + GLB title.
5. Vendor font check: `BigShouldersDisplay-900.ttf` / `DMSerifDisplay-Italic.ttf`
   are in `assets/vhc/`; confirm they load (Act A card text used them; the
   availability lockup read correctly in the strips).

---

## 9. Round 2 results (2026-10-05) — style-review violations fixed + engine loop

Round 2 = **fix the 5 style-review violations from r1 first**, then improve. All
five were resolved without measurement-fitting; every palette/timing value now
comes from the source constants (`index.html`, `compositions/vfx-text-cursor.html`).

### 9.1 What changed

- **actA V1 colours [no-hardcoded-measurement].** Deleted the dead
  `LEFT_COLOR`/`RIGHT_COLOR` (= measured rgb(6,11,22)/rgb(77,187,250)). The
  anamorphic streak palette is now the source `sweepColor =
  vec3(0.14,0.65,0.85) + spectrum(cursorX*1.8)*0.28` with `spectrum(x)` inlined
  from vfx-text-cursor.html:489-542; `cursorX` follows the source `progress`
  0→1 over 0.72..3.0. Net actA: p8 0.7325→0.7295 (neutral — V1 is dominated by
  the missing ray-march, see §9.3).
- **actC portalRadius [no-hardcoded-measurement].** Removed the measured
  `PORT_T` piecewise table + the `0.9` disc-coverage calibration. `portalRadius`
  is now source-derived: `PORTAL_START+0.05` → 0.72 power1.inOut 7.59 s, then
  →1.8 power3.in 1.45 s from `19.4−RGB_CUT_DELTA` (local 11.147). The
  camera/uv mapping is written explicitly (source portal camera fov 50, z 2.8)
  in the lambda rather than fitted.
- **actC portal layer structure [structural-1to1].** Renamed the end card back
  to the source `#portal-logo-frame` (z9). Added the source `#portal-canvas`
  (z10) child `chrome-front` = the CTA `tFront` texture (`Full catalog /
  available now` on black); the portal lambda now samples it as a shader child
  and composites `frontColor*mask + ring*glow` exactly as the source fragment
  (mask = smoothstep(edge±0.008, dist)); inside the disc stays transparent so
  the white logo frame shows through.
- **actC portal reveal timing [structural-1to1, violation #5].** `ENERGY_END`
  was 6.817 s local (≈5.3 s early). Rebuilt from the source: energy 0→1
  (0.28 power3.out) at `PORTAL_START` 3.687, energy→0 (0.55 power2.out) from
  `20.35−RGB_CUT_DELTA`→local **12.097**, canvas hidden at
  `20.85−RGB_CUT_DELTA`→local **12.597**. Removed the bogus
  `PROMPT_PAN_EXTRA/HEYGEN_START` terms.
- **actC chromatic [structural-1to1].** The 4 flat opaque `<text>` copies are
  gone. `#chromatic-canvas` now samples a `chrom-still` shader child (source
  `#chromatic-cap`: black `#08060e` + warm/blue/green radial gradients +
  136px Inter 800 "You can make this too") and outputs the source per-channel
  lum: `R=cr.r*(cr.r*0.6+max(cr.g,cr.b)*0.4)`, `G`, `B` likewise — a single
  additive-ish plane each, glow-accumulated. Plane world offsets and camera
  (`fov 40, z 5`, `camX→4.7/camZ→2.55` on `RGB_ROTATE_START`) are the source
  constants, projected via a pinhole solve, replacing the fitted `split*420px`.
  RGBA is composited (source composites RGB additive over the black clear but
  then puts alpha=1 — noted below).

### 9.2 Measured (actC, ref frames 400-787)

Baseline r2 → after: mae **19.14 → 12.57**, avg_p8 **0.194 → 0.188**, p32
0.131→0.113. Target frames: chromatic ~f30-90 flat-to-better (f90 0.10→0.006);
CTA hold f100 0.013; portal reveal f120-340 within ~0.01 of baseline. actA
0.7325→0.7295; actB unchanged (not touched this round).

### 9.3 Engine: bounded `for` loop added (the §8.4 top-ROI gap)

`crates/opencat-core/src/script/effects_lambda/{program,parse,lower_sksl,interp}.rs`
now accept `for (let i = a; i <|<= b; i++ | i += 1) { … }` with `return`
propagation, a hard 1e6-iteration guard, and compound `+= -= *= /=` on mutable
local slots. Dispatches to SKSL (`for (float oc_l{n} = …)…`) or the CPU
interpreter. 3 unit tests added (`cargo test -p opencat-core`: 669 pass).

**But V1 still cannot use it at runtime.** The source ray-march is 112 samples
per pixel (2.07M px × 195 frames ≈ 4.5e11 eval ops). A pixel-loop lambda would
force the CPU backend (the source `hash()` needs `h01`, which is CPU-only) and
is minutes-per-frame. So V1 stays the analytic streak + wordmark `<image>`;
actA's ~0.73 is a measured floor for the current engine. The loop is in place
for cheaper per-pixel kernels (e.g. a short-unroll chromatic/portal variant).

### 9.4 Negative results / notes

- The source DOM `.stage-logo` wordmark is never made visible (opacity stays 0);
  the wordmark in the reference is painted *inside* the WebGL canvas. The
  opencat `<image>` wordmark is therefore an approximation, not a DOM port.
- `.html-word` is `width: calc(1.78em+82px)` (602px @292px) in the source.
  Applying that width in opencat wrapped "HTML" (the Big Shoulders metrics on
  our side are wider than the browser's), so it was reverted; the V2 "HTML-in"
  gap remains a known layout delta.
- Chromatic RGB is composited with alpha=1 (the source planes are
  AdditiveBlending + UnrealBloom over a 0x000000 clear; source then leaves the
  offscreen buffer's alpha at 1). We emulate the additive look by accumulating
  the three lum-weighted channels in one plane; a true additive draw
  (non-SrcOver blend) is still unsupported (`drawColor` rejects non-SrcOver).

### 9.5 Next-round suggestions

1. **V1 ray-march**: the loop exists; the blocker is cost + the source `hash()`.
   Either add an SKSL-expressible hash (fract/sin) to the stdlib so the march
   can run on the GPU, or unroll a bounded 8-16 sample march in SKSL. This is
   the single largest lever (actA 0.73 of the whole film).
2. **V2 layout**: reproduce the 602px `.html-word` width with a corrected
   Big Shoulders measurement, or scale-fit the word to the source's 1.78em.
3. **Additive RGB planes**: add a non-SrcOver blend path to `drawColor`/
   `drawRect` so the three chromatic planes can be truly additive (removes the
   approximate single-plane accumulation).
4. Gate: 0.02 remains unreachable while the four shaders + GLB phone are
   approximations; treat ~0.19 (actC) / ~0.58 (actB) as the current floor.

---

## 10. Round 3 results (2026-10-05) — bg.mp4 media-clock offset (actB −0.11 p8)

Round 3 = diagnose the two boundary clusters flagged by the r2 diff (actB
frames 0–120 at p8≈0.98, actC frames 0–7 flat black) and fix the highest-ROI
one with a source-derived value.

### 10.1 What changed — actB `bg-video` media clock (source-derived, +0.62 s)

The actB head was a **background phase error**, not a missing layer. The
offset is derived from source constants, not a measurement fit:

- source markup (index.html:433): `<video id="bg-video" data-start="5.58" …>`
  with **no** `data-media-start` ⇒ the video's own media clock starts at 0 at
  absolute time 5.58.
- `HEYGEN_START = 5.58` (index.html:1249), and the shifted timeline puts the
  bg-video beat at absolute 5.58.
- the actB window opens at absolute **6.20** (handoff §3 / act-window table).

Therefore `media(local) = local + (6.20 − 5.58) = local + 0.62`. The end time
is frame-rate-independent and needs no measurement table.

Corroborating frame evidence (background corner crop, matched against
`assets/bg.mp4` frames — records that the constant is right, not the source of
the value). Here "local" is the actB-local frame index; our render with
`data-media-start=0` would show bg frame `local`, while the constant predicts
`local + 30·0.62 = local + 18.6`:

| local frame | ref frame | matched bg.mp4 frame | predicted `local+18.6` |
|---|---|---|---|
| 10 | 196 | 28 | 28.6 |
| 20 | 206 | 38 | 38.6 |
| 34 | 220 | 52 | 52.6 |
| 54 | 240 | 72 | 72.6 |
| 64 | 250 | 82 | 82.6 |
| 74 | 260 | 92 | 92.6 |
| 84 | 270 | 102 | 102.6 |

The matched swirl frames track the 0.62 s constant to within one frame; the
residual is sub-frame render-clock jitter, not a rate error.

Fix: `examples/vfx-heygen-actB.xml` `<video id="bg-video" … data-media-start="0.62">`
(the only change this round; the XML comment records the constant derivation).

### 10.2 Measured (actB, ref frames 186–400 = `out/_vhc-ref/actB.mp4`)

| metric | r2 baseline | r3 (bg offset) | delta |
|---|---|---|---|
| mae | 30.266 | **17.372** | −12.89 |
| avg_p8 | 0.5797 | **0.4000** | −0.180 |
| avg_p32 | 0.3616 | 0.1485 | −0.213 |

No frame regressed (checked all 215: 0 frames worse by >0.01 p8). Single-frame
spot check at f30 (ref 216): full mae 73.0 → **31.2**, background-only crop
mae 28.1 → **3.8**. Worst remaining actB frames are f0–4 (p8≈0.99) — the
actA→actB seam, where our composer still renders the VFX "HTML-in canvas"
card at 100% while the ref has already dropped it to the phone scene; that is
a **cross-act seam** artefact (actB begins 2 s after the source V4 phone
entrance, so the DOM card is dropped abruptly) and is left for a later round.

### 10.3 Whole-film effect

| act | n | mae | avg_p8 | avg_p32 |
|---|---|---|---|---|
| A | 195 | 57.376 | 0.7295 | 0.5408 |
| B | 215 | **17.372** | **0.4000** | 0.1485 |
| C | 387 | 12.566 | 0.1884 | 0.1129 |
| **film** | **797** | **24.826** | **0.3778** | 0.2272 |

Round 2 film: mae 28.304 / avg_p8 0.4263 → round 3: mae **24.826** (−3.48),
avg_p8 **0.3778** (−0.0485). Gate `avg_p8 ≤ 0.02` still not met (actA's
ray-march floor dominates).

### 10.4 Negative results / open hypotheses

- **actC head (seg local 0–4 = ref 400–404) is not a real bug.** The ref
  actC segment legitimately starts with 5 cream "You can make this too" frames
  (mean 242,242,225) because the cut at ref 400 lands on the propagated V8
  card; the dark chromatic scene begins at seg local 5 (ref 405). Our actC
  starts black at local 0. A cheap fix (extend the V8 card by 5 frames) exists
  but requires touching actB's duration, so it is deferred; it would only
  recover ~5/387 of actC.
- **Phone size** (actB): our 2D proxy is ~9 % smaller than the ref GLB
  (body 886 px vs 976 px tall at the settled pose). The projected height
  `3.2·FOCAL/cZ` gives 889 px; the ref reads 974 px, consistent with the GLB's
  real bounding height ≠ 3.2 (only its Y-extent is normalised). Not a clean
  scalar and camera-dependent while zooming, so not chased.
- **actA bg-2 phase is already correct** (matched bg-2 frame == `(t−3.1)·30`
  for t 4.0/5.0). actA's error is the missing 112-sample ray-march (V1) plus
  the V2–V4 card layout, as in §8/§9 — the bg-offset lever does not apply.
- **actA→actB seam** (actB f0–4, actC f0–4): the composer's cross-act handoff
  drops the VFX card ~5 frames too late relative to the ref. Fixing it means
  aligning the actA tail/actB head composition split (the source has no hard
  cut there), not a per-act value.

### 10.5 Next-round suggestions

1. **actA V1 ray-march** remains the single largest lever (actA 0.73 × 0.245
   of the film). Add an SKSL-expressible hash to the stdlib (fract/sin) so the
   march runs on GPU, or unroll an 8–16 sample SKSL march.
2. **Cross-act seam**: make actB start at the source V4 entrance (drop the
   stale VFX card at the top of actB) — recovers actB f0–4 and actC f0–4.
3. **actC portal/end** (p8≈0.17 at f260–386): the portal ring/disc detail is
   still an approximation; compare per-frame at f300+ before touching.
4. Gate: 0.02 unreachable while the four shaders + GLB phone are
   approximations; current floor after r3 is ≈ 0.38 film (actA 0.73 dominates).

---

## 11. Round 4 results (2026-10-05) — style fix + chromatic perspective projection

Round 4 = (a) resolve the r3 style-review violation first, then (b) fix the
highest-ROI region. The lever was **actC's chromatic RGB scene**, which the r3
render had black for ~80 frames (ref frames 405–490).

### 11.1 Style fix (mandatory, from the r3 review) — actB bg media clock

The r3 value `data-media-start="0.6"` was a measured approximation. It is now
**source-derived**: source `index.html:433` has
`<video id="bg-video" data-start="5.58" …>` with **no** `data-media-start`, and
`HEYGEN_START = 5.58` (`index.html:1249`); the actB window opens at absolute
**6.20**, so `media(local) = local + (6.20 − 5.58) = local + 0.62`. XML line 32
is now `data-media-start="0.62"` and its comment + §10.1 record the constant
derivation (the offline matched-frame table is kept only as corroboration).
Metric effect: none (0.02 s < 1 frame) — actB stays mae 17.372 / p8 0.4000,
identical to r3. The violation is cleared.

### 11.2 What changed — actC chromatic projection (the lever)

The r3 chromatic guest shader used a hand-rolled "pinhole" projection that
ignored the camera's `lookAt` rotation (`czN = camZ − worldZ`, pan by `camX`).
At the source camera poses (e.g. frame 74: `camX 3.2, camZ 2.67, lookAt
(−0.12,0.05,0)`) that maps screen-centre rays onto the still's dark **left
edge**, so the whole composite rendered ~black (actC seg 6–87 out-mean ≈ 0.0
vs ref ≈ 14–17; a direct probe confirmed the child `chrom-still` subtree itself
captures correctly — the fault was purely the projection).

Fixed 1:1 with the source (`PerspectiveCamera(40, W/H)`, three
`PlaneGeometry(3.0*aspect, 3.0)` meshes at `depthX∓xSplit / depthZ±zSplit`, Y
rotations `red/green/blueRotY`, `index.html:1009/1071-1088`): an exact
ray/plane solve. For pixel-local uv the view ray is
`D = F + sx·(tanH·aspect·R) + sy·(tanH·U)` (sx,sy ∈ [−1,1], F/R/U the camera
basis); plane hit `t = −dot(C−Pc, n)/dot(D, n)` with `n = (sinθ,0,cosθ)`; plane
uv `= (0.5 + lx/planeW, 0.5 − ly/planeH)`, `lx = e.x·cosθ − e.z·sinθ`, `ly = e.y`.
All uniforms (camera C/F/R/U + per-plane centre and cos/sin) are passed as
flat floats; the three additive channel planes are still accumulated in one
draw (single-plane approximation of the source's three AdditiveBlending draws).

Verification: the ref wordmark is **horizontally mirrored** at these poses, so
the basis sign matters — `R = +cross(F,up)` was tried and measured worse
(actC mae 9.388 vs 9.125), so `R = (−Fz, 0, Fx)` (the other cross order) is
kept. Single-frame f74 bbox now matches the ref band (x 1–1353 vs ref 0–1393).

> **[r5 correction — see §12.1]** The basis is no longer hand-written with the
> sign chosen by mae. It is now derived by the engine helper
> `geom.rs::camera_basis`, which implements THREE `Matrix4.lookAt`
> (`right = normalize(cross(forward, up_world))`); that derivation returns
> exactly `(−Fz, 0, Fx)`, so the pixels are unchanged (actC re-render is
> byte-identical) and the "chose the smaller-mae sign" rationale is retired.
> The "mirror" noted above is `CanvasTexture.flipY` (a Y-axis flip), not the
> right-axis sign.

### 11.3 actC head-copy off-by-one

The V8 tail card gate was `st > 0.167`, which holds the card through seg frame 5
(one frame too long; ref seg 0–4 are cream, seg 5 is dark). Retimed to
`st >= 5/30` (= src `CHROMATIC_START` 7.92 → actC local 0.16667), matching the
frame set 0–4. Frame 5: p8 0.999 → 0.272.

### 11.4 Measured (actC, ref frames 400–787 = `out/_vhc-ref/actC.mp4`)

| metric | r3 | r4 | delta |
|---|---|---|---|
| mae | 12.566 | **9.125** | −3.44 |
| avg_p8 | 0.1884 | **0.1698** | −0.019 |
| avg_p32 | 0.1129 | **0.0976** | −0.015 |

Whole film (797 frames, weighted per act):

| act | n | mae | avg_p8 | avg_p32 |
|---|---|---|---|---|
| A | 195 | 49.769 | 0.7255 | 0.5365 |
| B | 215 | 17.372 | 0.4000 | 0.1485 |
| C | 387 | 9.125 | 0.1698 | 0.0976 |
| **film** | **797** | **21.294** | **0.3679** | **0.2187** |

r3 film: mae 24.826 / avg_p8 0.3778 → r4: mae **21.294** (−3.53), avg_p8
**0.3679** (−0.0099). Gate `avg_p8 ≤ 0.02` still not met (actA 0.7255 dominates).

### 11.5 Negative results / next

- **No engine change needed** this round; both fixes are pure XML (the shader
  stays a single-plane approximation because `drawColor` still rejects
  non-SrcOver add).
- **actC chromatic residual** (seg 30–86, p8 ~0.30) is now a *glow/bloom* gap:
  the source runs `UnrealBloomPass`; our composite is unbloomed, so the ref is
  brighter and larger. A cheap lever is an emissive radial glow pass (the portal
  guest already builds one) applied to the chromatic canvas.
- **actB f102–110** (p8 ~0.88) is the source prologue→product scroll of the
  phone screen (our GLB-proxy screen is a static re-author) — the known V4/V5
  gap, not a timing bug.
- **actA 0.73** unchanged: the V1 ray-march (G2) remains the film's dominant
  error; else port the phone screen scroll (V4/V5) which is actB's dominant term.

---

## 12. Round 5 results (2026-10-05) — style violations resolved (source-derived)

Round 5 = clear the three r4 style-review violations. All three are fixed by
making the code match the reference **source semantics deterministically**;
none uses a measurement fit. The improvement edits the task text schedules for
this round (actB `data-media-start="0.62"`, the actC exact ray/plane projection,
the actC gate `st >= 5/30`) were **already present in the working tree** at
round start — they are the r4 changes whose commit the review blocked — so they
are unchanged and actB re-measures identically to r4 (mae 17.372 / p8 0.4000).

### 12.1 actC camera basis — derivation, not an mae-chosen sign

- Deleted the hand-written basis whose right vector was
  `var Rx = -Fz, Ry = 0.0, Rz = Fx;  /* F x (0,1,0) */` and whose sign §11.2
  admitted was selected because `+cross` measured worse. The script now calls
  `__util_camera_basis([camX, camY, camZ], [lookX, lookY, 0.0])`.
- New pure binding over `crates/opencat-core/src/geom.rs::camera_basis`
  (unit-tested): Three.js `PerspectiveCamera.lookAt` / `Matrix4.lookAt` exactly
  — `forward = normalize(target − eye)`,
  `right = normalize(cross(forward, up_world=(0,1,0)))`, `up = cross(right,
  forward)` (degenerate `eye==target` → the Three.js default −z view basis).
  `cargo test -p opencat-core` → **672 pass** (3 new basis tests).
- **The derivation returns exactly `right = (−Fz, 0, Fx)` = `F × up`** — the
  same vector the code already used — so this is a rationale removal, not a
  pixel change. Verified: actC re-rendered is **byte-identical**
  (mae 9.12460 / p8 0.169826 in both the pre- and post-fix render). The
  "mirror" in the source is `CanvasTexture.flipY` on the `Y` axis, not the
  right-axis sign.

### 12.2 actA ray-march mask — sample the coverage channel (alpha)

- `const mask = colormask.eval([...]).r` → `.a`, and the in-loop
  `acc += colormask.eval([...]).r / 112.0` → `.a / 112.0`. The source
  `drawLogoMasks` (vfx-text-cursor.html:597-620) fills every `.stage-logo path`
  with `maskCtx.fillStyle="#fff"`, so the mask canvas is RGB=(1,1,1) on ink and
  the coverage lives entirely in the ALPHA channel (`readMask().r` is 1.0 on
  ink, 0 off). Our composite `logo-field.png` carries coverage in `.a`; its
  `.r` is the *colour* channel, non-constant over the gradient shards
  (measured: 23.5% of ink has R<200), so the old `.r` under-read coverage.
- Measured: **mae 49.769 → 50.931, p8 0.7255 → 0.7260** (actA frames
  0-194). Net mae rises ~1.16 because the `.r` under-read happened to be
  slightly closer to the reference's darker ray field; the change is kept
  because it is source-correct (the mask canvas IS all-white/alpha) and the
  `.r` read was a channel-semantics error. It is not chased for pixels.

### 12.3 Whole-film measurement (r5, after the fixes)

| act | n | mae | avg_p8 | avg_p32 |
|---|---|---|---|---|
| A | 195 | 50.931 | 0.7260 | 0.5381 |
| B | 215 | 17.372 | 0.4000 | 0.1485 |
| C | 387 | 9.125 | 0.1698 | 0.0976 |
| **film** | **797** | **21.578** | **0.3680** | **0.2191** |

r4 film: mae 21.294 / avg_p8 0.3679 / p32 0.2187 → r5: mae **21.578**, p8
**0.3680**. The +0.28 mae is entirely actA's source-correct mask change; the
other two acts are unchanged (actB unchanged by construction, actC
byte-identical). Gate `avg_p8 ≤ 0.02` not met (actA's V1 ray-march floor
dominates, as in every prior round).

### 12.4 Negative results / next

- **actA ray field is 2-3× too bright** across V1 (frames 20-199 p8≈0.9).
  Frame-45 block means show out ≈ (200,250,255) vs ref ≈ (60,180,210) — a
  saturation/brightness gap in `acc` (mask coverage) and the sweep/logRay
  terms, not a placement bug (wordmark lands correctly). The source uses a
  **bounded 112-sample march per pixel**, which the engine's `for` lambda
  supports but only on the CPU backend at minutes/frame; a real fix needs an
  SKSL-expressible hash (fract/sin) so a bounded march can run on the GPU, or
  an unrolled SKSL march — then reduce `acc`/sweep gains to source values.
  Still the film's dominant error.
- **actC chromatic residual** (seg 5-140 p8 0.21-0.35) is unchanged and is the
  expected UnrealBloomPass/additive gap (documented §11.5); a single-plane
  non-additive composite cannot reproduce three additive planes + bloom.
- **actA→actB / actB→actC seam** (actB f0-4, actA f186-194 p8≈0.99) is the
  cross-act composition-split artefact (§10.4), not fixed this round.
- Additive non-SrcOver draw to `drawColor` remains the single engine lever that
  would unlock a true three-plane chromatic composite (§9.5).

---

## 13. Round 6 results (2026-10-05) — r5 fixes verified in-tree; V1/V2 probed

Round 6 = confirm the three r5 style-review violations are actually gone from
the **whole actA** (violation #3), then spend the round on the regional lever.
No regression was introduced and no new violation is left behind.

### 13.1 The three r5 violations — re-checked against the tree, all clear

- **#1 actC camera basis.** `examples/vfx-heygen-actC.xml` calls
  `__util_camera_basis(...)` (line 280); `grep` confirms the old
  `var Rx = -Fz, Ry = 0.0, Rz = Fx; /* F x (0,1,0) */` is gone. The helper
  lives in `crates/opencat-core/src/geom.rs::camera_basis` (strict
  `PerspectiveCamera.lookAt` semantics, 3 unit tests) and is wired through
  `bindings.rs` (`util_camera_basis`). No measurement-picked sign remains; the
  flip is `CanvasTexture.flipY`, handled by the `0.5 - ly/ph` sample in the
  lambda. `cargo test -p opencat-core` → **672 pass / 0 fail**.
- **#2 actA ray-march mask.** actA.xml:103 and :126 both use `.a`
  (`colormask.eval(...).a`), not `.r`; the comment records the source
  `drawLogoMasks` white-mask/alpha-coverage semantics.
- **#3 actA coverage.** actA is now actually **rendered and measured this
  round**, so it is no longer an "unrendered/untested rewrite riding along in a
  commit". The r6 render is byte-identical to r5 (`git diff` of actA.xml vs the
  round-start working tree is empty after probing — see §13.3), i.e. the r5
  fixes are frozen and measured, not merely present.

### 13.2 Measured (r6, per act; actA ref frames 0–194 = `out/_vhc-ref/actA.mp4`)

| act | n | mae | avg_p8 | avg_p32 |
|---|---|---|---|---|
| A | 195 | 50.931 | 0.7260 | 0.5381 |
| B | 215 | 17.372 | 0.4000 | 0.1485 |
| C | 387 | 9.125 | 0.1698 | 0.0976 |
| **film** | **797** | **21.578** | **0.3680** | **0.2191** |

Identical to the r5 numbers to 3 dp (actA re-rendered and re-diffed this round;
actB/actC re-rendered and re-diffed too). Gate `avg_p8 ≤ 0.02` not met — actA's
V1 ray-march floor still dominates.

### 13.3 What was tried this round (negative results, all reverted)

Per the round rule, every experiment that did not improve pixels was reverted;
the working tree at round end equals the r5 tree for `examples/` (verified with
`git diff`).

- **V2 "HTML" word box (`width:calc(1.78em+82px)`).** Added the source's
  `.html-word` fixed width. Measured **identical** (f115/130/145/160/175
  band-mae unchanged to 3 dp) — the box does not re-center at `scale:1` in
  opencat, so it is a no-op. Reverted.
- **V1 `hash2` "all-components +1" fix.** The source
  `hash2(vec3 p) = vec2(hash(p), hash(p + 1.0))` adds 1.0 to **all** of p; our
  `h2` only adds +1 to `ry`,`i`. Implemented the literal port; measured
  **worse** (f45 mae 65.1 → 69.1, f30 112 → 114.3). Reverted (kept the existing
  `h2`), recorded here as a negative result.

### 13.4 Root-cause finding — the V1 (and V2) mismatch is a *font*, not the shader

Both the V1 wordmark and the V2 "HTML" glyphs are visibly the **wrong typeface**
in our render, not a shader/brightness error:

- **V1 wordmark** is drawn from the rasterised `#logo-field` (source
  `.stage-logo` inline SVG, 263×79 viewBox, scaled to 1080 wide). It is
  **byte-identical to the source SVG**, so the V1 wordmark placement is correct;
  V1's residual is the procedural ray field.
- **V2 "HTML"** uses "Big Shoulders Display" 900. The source loads it as a web
  font (no `@font-face` in the repo; `grep -rln "Big Shoulders"` finds only the
  CSS binding). The bundled `assets/vhc/BigShouldersDisplay-900.ttf` resolves as
  **"Big Shoulders Display Thin Black"** (`BigShouldersDisplayThin-Black`, name
  IDs; no `fvar` table) and renders 650×(214 @ f130) for "HTML", whereas the
  reference reads ~500×~500 — i.e. the ref glyphs are **2.3× taller / much more
  condensed** than our cut. No correct Big Shoulders file exists anywhere in
  `/home/solaren/Projects/hyperframes-launches`, so this cannot be fixed from
  assets: it needs the exact Google-Fonts *Big Shoulders* static cut the source
  browser fetched. (The "DM Serif Display" italic for "natively" is likely a
  similar mismatch.)

### 13.5 Next-round suggestions

1. **Obtain the exact fonts.** Download the Google-Fonts *Big Shoulders*
   static 900 cut (and verify *DM Serif Display* italic) and drop them in
   `assets/vhc/`, then re-measure V2/V3 and the V1 wordmark. This is the
   highest-ROI lever for actA's V2 window (band p8 ~0.6) and is orthogonal to
   the shader.
2. **GPU-bounded ray march (actA V1, still the film's dominant error).** The
   `for`-lambda runs on the CPU at ~10 s/frame for V1; a real fix needs the
   fract/sin hash lowered to SKSL with a bounded/unrolled loop so the march runs
   on the GPU, after which the `acc`/sweep gains can be tuned toward the source
   values. The procedural field is also speckle-dominated, so per-frame
   fluctuation sets a high floor regardless.
3. **Additive `drawColor`** (unchanged from §9.5/§11.5) remains the lever for a
   true three-plane chromatic composite + bloom in actC.
4. Do **not** re-attempt the two no-op/negative edits in §13.3.
