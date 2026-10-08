# website-to-hyperframes — Alignment Handoff (per-launch)

Status: **round 7 done** — CTA `$` margin-right + act4 wordmark 1px; whole-film
mae 2.229 → 2.200, avg_p8 0.01994 → 0.01984 (see §16). This branch
`feat/hyperframes-launches-alignment` is the work branch for this title.

> Round 1 note: the intake baseline (§4) was measured **before** two engine
> fixes that are now baked into the R1 measurement (box/text-shadow σ = blur/2;
> GSAP timeline no longer rescaled to fit scene duration — see §10). The R1
> baseline used below is the **pre-session render** (act1 mae 5.883 / p8 0.0449,
> act3 2.598 / 0.0357, act4 3.211 / 0.0366; whole-film 3.658 / 0.0385).

Reference project: `/home/solaren/Projects/hyperframes-launches/website-to-hyperframes/`
Reference video: `renders/website-to-hyperframes_2026-10-02_09-18-10.mp4`

---

## 1. Reference spec (ffprobe, measured)

| Field | Value |
|---|---|
| Container / codec | MP4, h264 High, yuv420p |
| Resolution | 1920 × 1080 |
| Frame rate | 30/1 fps |
| Frames | 1254 (`-count_frames`) |
| Duration | 41.800 s (container 41.813 s) |
| Audio | yes — AAC stereo, 48 kHz, 41.813 s |
| Color tags | range=tv, space/transfer/primaries=bt709 |

Frames ↔ seconds: `frame = round(sec * 30)` (0-based). 1254 frames = 41.8 s.

## 2. Reference structure map

Root `index.html` (`data-composition-id="master"`, 1920×1080, duration 41.8):

| Act | composition | window (s) | window (frames) | notes |
|---|---|---|---|---|
| Act 1 | `compositions/act-1-cold-open.html` | 0.0 – 12.0 | 0 – 359 | Cold open |
| (Act 2) | — | — | — | cut; Act 2 does not exist in this cut |
| Act 3 | 11 inline `<video>`/cap nodes at body level | 12.0 – 33.8 | 360 – 1013 | Capability reel |
| Act 4 | `compositions/act-4-end-card.html` | 33.8 – 41.8 | 1014 – 1253 | End card |

Act 3 is **not** a sub-composition element — the 11 clips + captions are
declared directly under `<body>` with `data-start`/`data-duration` and
`data-track-index` 10–31; Act 3 captions are `data-track-index` 21–31 (top-left
`.a3-cap`) and 100–110 (bottom `.cap`).

Master-level audio/caption tracks (absolute times, body-level): music 0–41.8
(vol 0.11), 5 typing SFX, 2 click SFX, 1 bass drop; VO Act-1 ×5,
Act-3 ×11, Act-4 ×3; bottom `.cap` captions Act-1 ×4, Act-3 ×11, Act-4 ×2.

### Act 1 (`act-1-cold-open.html`), local time 0–12.0
Single `gsap.timeline({paused:true})` (`window.__timelines["act-1-cold-open"]`).
Phase summary (originals):
- P1 CTA: carets blink 0.20/0.40; `cta-cmd` clipPath `100%→0` `steps(36)` @0.45 (0.90s); caret blinks 1.38/1.53.
- Send CTA @1.65 (`y:-50,scale:1.02,opacity:0`, 0.28, power2.out); `morph` squash 0.98→1.0.
- `mc-main` cursor fromTo @0.80 (x220→40), settle @1.25, click @1.65.
- P2 Morph @1.75: box `w1720×h180 → w1400×h780` (0.45, power3.inOut); `term-content` opacity @1.98.
- P3 typing: p1 steps(38) @2.15 (1.05s); t-cursor fromTo @3.30 (x400,y-80,rot-8→x-55,y-28,rot-4, 0.55, power3.out); p2 steps(21) @3.40; t-codex @4.80 (x-400,y90,rot10→50,32,5); p3 steps(25) @4.90; t-gemini @6.40 (x60,y-220,rot-7→-28,48,-3); p4 steps(18) @6.50; caret single-blink pairs.
- P4 charge @8.00: `x:"+=3"` yoyo repeat 5, 0.045.
- P5 scatter @8.15: four terms to ±480/±270, scale 0.52, 0.20 power3.out; settle 0.55/0.52 elastic @8.35/8.49.
- P6 phrase: terms opacity 0.12 @8.36; pw1 "Now" @8.45, pw2 "any site becomes" @9.00, pw3 "video." @9.85 (blur 14→0, power2.out).
- P7 YEAHH @10.55: `.yeahh-wrap` scale 0→1.18 rot-8→-3 back.out(2.5), settle 1.0; 4 `.star` ✦ pop; fade @10.95.
- P8 send @11.05–11.90: 2 click cursors, ripples, prompts fly up, 4 sent bubbles back.out, cursors/bubbles fade.
- P9 flash @11.92: white 0→1 (0.05) →0 (0.18).

### Act 3 — 11 hard-cut clips (relative to 12.0, i.e. local = abs − 12.0)

| # | video (s) | local start | dur | brand | light cap |
|---|---|---|---|---|---|
| 01 | a3-01-stripe | 0.0 | 1.500 | stripe.com | |
| 02 | a3-02-tailwindcss | 1.5 | 1.900 | tailwindcss.com | |
| 03 | a3-03-framer | 3.4 | 2.000 | framer.com | |
| 04 | a3-04-spotify | 5.4 | 1.800 | spotify.com | |
| 05 | a3-05-heygen | 7.2 | 2.000 | heygen.com | ✓ |
| 06 | a3-06-vercel | 9.2 | 2.000 | vercel.com | |
| 07 | a3-07-vibecode | 11.2 | 2.000 | vibecode.app | |
| 08 | a3-08-linear | 13.2 | 2.000 | linear.app | |
| 09 | a3-09-dribbble | 15.2 | 3.000 | dribbble.com | ✓ |
| 10 | a3-10-tailwindcss-logo | 18.2 | 1.600 | tailwindcss.com | |
| 11 | a3-11-github | 19.8 | 2.000 | github.com | |

No cross-fade — hard cuts. `.a3-cap` top-left card (mono num + Inter-400
brand, border-left, backdrop blur 10). `.cap` bottom captions run CSS
`cap-pop` keyframes.

### Act 4 (`act-4-end-card.html`), local 0–8.0
Single timeline. Aurora opacity 0→0.85 @0.0 (1.2s) + drift x+40,y-20 (7.5s);
spotlight @0.2; 12 particles stagger 0.06 @0.3 + per-particle breathe;
wordmark "Hyper" blur 20→0 @0.9, "Frames" gradient `background-clip:text` @1.20
(both scale 1.01 breath @1.8); promise lines @2.4 / @3.3; install pill @4.8 /
cmd @5.0; breath dot @6.0; bottom captions "Give your agent a URL" (local 2.0 —
note: reference master VO is at 2.0 rel) and "Go make something" (6.3).

### Assets / fonts
- Fonts: `Inter-Variable.woff2`, `JetBrainsMono-Variable.woff2`, `BungeeShade-Regular.woff2` (Act 1 YEAHH only).
- Logos: `claude.svg`, `codex.svg`, `codex-large.svg`, `cursor.png`, `gemini.png`.
- Act 3: 11 mp4 clips. Audio: music, 3 SFX, 23 VO mp3s.
- opencat mirrors these at `assets/Inter-*.ttf`, `assets/JetBrainsMono-*.ttf`,
  `assets/BungeeShade-Regular.ttf`, `assets/w2h-logos/*`, `assets/w2h-a3/*.mp4`.

## 3. opencat targets (repo-relative)

| Act | XML | status |
|---|---|---|
| Act 1 | `examples/website-to-hyperframes-act1.xml` | **untracked / WIP, NOT aligned** (mae 13.46) |
| Act 3 | `examples/website-to-hyperframes-act3.xml` | tracked, aligned (mae 2.60) |
| Act 4 | `examples/website-to-hyperframes-act4.xml` | tracked, aligned (mae 3.22) |

No master/whole-film XML exists; the film is expressed as three per-act XMLs
(each with its own local clock: duration 12.0 / 21.8 / 8.0). Act 1 currently
renders 360 frames, Act 3 654, Act 4 240 → concat = 1254 = full reference.
There is no `examples/website-to-hyperframes.xml`; do not create one unless a
whole-film measurement requires it.

## 4. Measured alignment (this intake; nothing modified)

Method: `ffmpeg select=between(n,start,end)` to cut the reference act, then
`tools/k3diff.py <ref-seg> <render>`. **Use the exact act windows in §2 with
offset 0** — do not use the ±4-frame idea (see §6).

| Act | frames | ref window | mae_mean | avg_p8 | worst_p8 | worst_mae | frames p8>0.08 |
|---|---|---|---|---|---|---|---|
| Act 1 | 360 | 0–359 | **13.463** | **0.1867** | 1.000 | 126.5 | 353/360 |
| Act 3 | 654 | 360–1013 | 2.601 | 0.0357 | 0.846 | 26.6 | 47/654 |
| Act 4 | 240 | 1014–1253 | 3.219 | 0.0370 | 0.456 | 17.2 | 1/240 |

act3/act4 reproduce their committed numbers (2.63 / 3.13) at **offset 0**.
Act 1 is far outside; a global offset hypothesis was tested and rejected (§6).

### Acceptance gate reconciliation
Task gate: `avg_p8 ≤ 0.02`, `worst_p8 ≤ 0.08`. **No act meets it strictly** —
even the two "aligned" acts sit at avg_p8 ≈ 0.036. Two consequences:
1. The gate as written is not the empirical working floor of this pipeline; it
   likely needs recalibration (e.g. avg_p8 ≤ 0.05, worst_p8 ≤ 0.15), or...
2. ...there is a systematic reference-vs-render offset (H1 below) that sets a
   ~2–5 mae / ~0.03 p8 floor for *all* acts. Neutralize it first, then re-judge.

## 5. Engine gap checklist (Act 1 focus)

Verified present in the engine: `backdrop-filter` (blur), `box-shadow`,
`text-shadow`, `rotate`/GSAP `rotation`, `filter: blur()`, `background-clip:
text`, `ctx.timeline()`.

Gaps / risks to close before act1 can reach act3/act4 parity:

- **G1 — global background/color offset (H1).** Reference page bg reads
  (239,239,232) while opencat renders (245,243,238) = exactly `#f5f3ee`; the CTA
  box reads (23,24,27) vs (26,27,31). A uniform ≈ (−6,−4,−6) shift on every
  frame. Suspect the reference's yuv420p range/transfer tagging vs opencat's
  untagged output. Dominates act1's mae. **Verify first** with the command in
  §7 before chasing content.
- **G2 — text/shape raster parity.** Even aligned acts carry ~0.03 p8 from
  glyph AA (Skia vs the reference's raster). Act 1's JetBrains-Mono prompts and
  Inter panels show edge deltas inside boxes.
- **G3 — animated-layer timing.** Panel entry / scatter / YEAHH show sub-pixel
  positional lag (Rec.709 conversion aside, from the ±4 test the true frame
  error is <1 frame). Biggest mae spikes are at transitions (frames 30–59
  22.5, 240–269 32.7, 300–329 27.3, 330–359 20.6).
- **G4 — YEAHH decorations dropped.** Reference `.star` ✦ ×4 (pink, back.out
  pop) are absent from the XML (`yeahh`/`yeahh-t` only). Also the reference
  `.yeahh` has hard 3px/6px pink+dark text-shadows and `rotate(-3deg)`.
- **G5 — morph-box scaling.** opencat animates an inner `#morph-shape` width/
  height while `#morph` is pinned; confirm the reference's outer-box scale
  (0.98/1.0 squash) is reproduced.
- **G6 — caption pop timing.** cap-pop is approximated by two keyframe
  segments (0.182 s + 0.078 s) per act — ~1-frame imprecision vs CSS's 0.26 s.
- **G7 — steps() typing.** Reference `steps(N)` advances one glyph per
  `round(i*N/total)`; opencat advances `N` even steps. May produce a 1-char lag
  on long strings (36/38/21/25/18 steps).
- **G8 — cursor / logo fidelity.** `mc-main`, `mc-tl`, `mc-bl` are rectangle
  placeholders (no SVG path raster); logos are PNGs (`w2h-logos/*.png`) vs the
  reference SVG. Minor but contributes to p8 at cursor frames.

## 6. Ruled out

- **Global time shift:** cross-correlation over ±15 frames gives a minimum at
  ≈ −4 frames but the *static mid-section* (frames 90–210) peaks at +2; the
  −4/+4 minimum is driven by the Act-1 flash/transition spikes, not a real
  offset. Re-cutting the reference for act 1 at +3..+8 all gave **worse** mae
  (11.5 best at +4 vs 13.5 at 0) and, critically, broke act3/act4 (which are
  correct at **offset 0**). Conclusion: use offset 0 for all acts.
- Act 1's mid-section already matches structurally (panels 2×2, phrase order,
  colors); the residual is the color offset (H1) + AA + sub-pixel timing.

## 7. How to measure

Reference windows (offset 0):
`act1 0–359`, `act3 360–1013`, `act4 1014–1253`.

Per-act (example act 1):
```bash
cd /home/solaren/Projects/opencat
REF=/home/solaren/Projects/hyperframes-launches/website-to-hyperframes/renders/website-to-hyperframes_2026-10-02_09-18-10.mp4
./target/release/opencat render examples/website-to-hyperframes-act1.xml out/_w2h-act1.mp4
ffmpeg -y -v error -i "$REF" -vf "select='between(n\,0\,359)',setpts=PTS-STARTPTS" -an \
  -c:v libx264 -preset fast -crf 18 out/_w2h-ref/act1.mp4
python3 tools/k3diff.py out/_w2h-ref/act1.mp4 out/_w2h-act1.mp4 --out out/_w2h-act1-k3diff
# read out/_w2h-act1-k3diff/summary.json -> agg.mae_mean / agg.avg_p8 ; pixel.csv -> per-frame
```
Act 3 window `between(n\,360\,1013)`; Act 4 window `between(n\,1014\,1253)`.

Whole film (concat the three act renders in order, then compare to the full ref):
```bash
printf "file '%s'\n" out/_w2h-act1.mp4 out/_w2h-act3.mp4 out/_w2h-act4.mp4 > /tmp/w2h.txt
ffmpeg -y -v error -f concat -safe 0 -i /tmp/w2h.txt -c copy out/_w2h-full.mp4
python3 tools/k3diff.py "$REF" out/_w2h-full.mp4 --out out/_w2h-full-k3diff
```

Web oracle vs reference (per act):
```bash
./target/release/opencat-web-compare examples/website-to-hyperframes-act1.xml \
  --out-dir out/_w2h-act1-web --interval-secs 1 --reference out/_w2h-ref/act1.mp4
```
(Skip the engine-vs-web oracle for video-heavy act 3 unless chasing a wasm-only
regression; hard-pixel reference diff is the faithful signal.)

H1 isolation (color offset): render a static opencat frame and compare its
background against the reference with an explicit range hint, e.g.
`ffmpeg -i "$REF" -vf "select=eq(n\,10),scale=in_range=tv:out_range=pc" ...`,
or compare against a known `#f5f3ee` patch. If the opencat bg is already
`(245,243,238)` = `#f5f3ee`, the reference's (239,239,232) is a reference-side
encode/decode artifact and should be normalized in the measurement, not "fixed"
in the XML.

## 8. Alignment plan & priority

1. **Act 1 diagnosis first (this loop).** Neutralize / quantify H1 (G1); if the
   floor drops, re-measure before touching markup.
2. Fix content gaps in order of impact: G3 (transition timing) → G5 (morph
   scale) → G4 (YEAHH stars/shadow/rotation) → G7 (steps typing) → G6 (cap-pop)
   → G8 (cursors/logos).
3. Do not touch act3/act4 unless a shared-core change regresses them (re-run
   their offset-0 diffs after any engine/style change).
4. Gate: adopt a measured floor (see §4); target act1 within ~1.5× act3's
   mae/p8 once H1 is removed.

## 9. Non-goals for this loop
No `examples/` XML edits, no commits. Outputs live under `out/` (gitignored).

## 10. Round 1 results

Method (exactly §7, offset 0). All numbers are whole-film-concat k3diff unless
noted.

| scope | mae | avg_p8 | worst_p8 | p8>0.08 frames |
|---|---|---|---|---|
| baseline (pre-session) whole film | 3.658 | 0.0385 | 1.000 | 411/1254 |
| **R1 whole film** | **3.311** | **0.0356** | **0.845** | **59/1254** |
| baseline act1 | 5.883 | 0.0449 | 1.000 | 58/360 |
| **R1 act1** | **4.650** | **0.0353** | **0.533** | **11/360** |

### What changed (files)
- `examples/website-to-hyperframes-act1.xml` (new file, was untracked WIP):
  - **G4 phrase/yeahh centering (the big one).** Reference `.phrase-wrap` and
    `.yeahh-wrap` are `left:50%; top:50%; translate(-50%,-50%)`. opencat pinned
    `#phrase` at `top-[380px]` and `#yeahh` at `top-[400px]`. Measured: phrase
    glyphs ~91px too low (ref f300 dark rows 314–430 vs opencat 405–523). Fixed
    to the centered equivalents — `#phrase top-[288px]` (wrap height 3×168 = 504
    → (1080−504)/2), `#yeahh top-[420px]` (240px cap). After: 313–431 ≈ ref.
    This erased the entire 271–323 cluster (52 frames) and dropped act1
    p8>0.08 from 58 → 10.
  - **G4 YEAHH text-shadow.** Added
    `[text-shadow:3px_3px_0_#ff006e,6px_6px_0_rgba(0,0,0,0.4)]` to `#yeahh-t`
    (was missing entirely; ref f325 has ~11.7k pink pixels, opencat 0). Now
    ~4.7k pink — better, partial (font-metric offset between the glyph edge and
    its offset shadow). Net act1 p8 0.0352→0.0353 = wash; kept (faithful).
  - `assets/w2h-logos/*.png` and `assets/BungeeShade-Regular.ttf` newly
    committed: act1 references them and they were untracked (mirrors the tracked
    `assets/w2h-a3/*` convention for act3).

### Confirmed NOT the problem (negative results, don't re-chase)
- **G1 color offset is a reference-side artifact.** Reference bg = (239,239,232)
  / p99 white ≈ (242,242,235); opencat bg = `#f5f3ee` = (245,243,236). The
  offset (−3..−6) also decodes from the *reference encode* (the mtv 709 transfer
  sends #f5f3ee↔#efefe8); `scale=in_range=tv:out_range=pc` does not change it.
  It is uniform and below the p8 threshold, so it inflates **mae** (every frame
  ~+0.4) but not p8. **Do not warp the XML to chase it** — normalize in the
  measurement if a future gate needs mae parity.
- **Scatter transient 245–248 is a narrow, unreproducible window.** By f249–250
  ref and opencat agree exactly (panel bbox identical). A constant frame shift
  does not help (min mae at shift 0). f246–248 mae 56–93 is ~4 frames of noise
  that only moves act1 p8 by ~0.002. Leave it.
- **Act3 top spike f9 (p8 0.85) is an inside-video hard cut.** The reference
  video content jumps a full frame ahead (stripes → gradient) around the cut,
  likely an encode B-frame artifact around a splice; ≈26 mae on 1 frame. Not a
  layout bug. act3 f12–14/f77–84 are the same class (cut transient / 1-frame
  inside-video lag).
- **Act4 f0 (p8 0.45) is a window-boundary artifact.** Ref frame 1014 still
  shows the tail of the Act3 GitHub clip (reference has no cross-fade at
  33.8s); opencat's act3 ends at 33.8 and act4 starts. 1 frame, unavoidable
  given per-act windows. Do not chase.

### Engine
- Pre-existing uncommitted fixes, now committed after verification:
  `parse/jsonl/tailwind.rs` + `parse/markup.rs` box/text-shadow σ = blur/2
  (canonical Skia/CSS); `script/runtime/animation/facade.js` GSAP timelines are
  no longer rescaled to fit `sceneDuration` (absolute times preserved; a late
  tween is clipped, not stretched).
- Fixed the new regression test
  `timeline_tween_times_are_not_rescaled_to_fit_scene_duration`: its expected
  value was wrong (frame 45 = t=1.5s on a 1.0–3.0s ramp = x 75, asserted 150)
  and its 12.1s overrun was too small to expose the 12/12.1 rescale; now uses a
  24.0s overrun (scale 0.5) against x=75, so a regression is unmistakable.
- `cargo test -p opencat-core` → 652 passed; `cargo test -p opencat-engine` →
  90 passed (all green after the test fix).

### Next round
1. **G3 transition timing.** Per-frame lag dominates the remaining act1 bad
   frames (271–323 is gone; now 317–322 YEAHH tail + the 245–248 scatter
   window). Re-derive the scatter/charge sub-frame alignment; consider the
   reference's `x:"+=3" yoyo repeat 5 @8.00` interacting with scatter @8.15.
2. **G7 steps() typing.** opencat advances N even steps vs the reference's
   `round(i*N/total)`; likely a 1-char lag on the 36/38-char prompts. Visible as
   the glyph band in the frozen diff, small p8 impact.
3. **G5/G4 YEAHH shadow parity.** Match the shadow offset to the glyph edge
   (font-metric mismatch currently leaves ~6k pink px unhit).
4. **Act3 is now the p8 floor** (0.0357, dominated by inside-video cut frames);
   if the gate must go lower, the lever is a 1-frame inside-video seek, not
   markup. Reconsider the gate ceiling (§4) — 0.02 is below the reference
   encode's own noise.
5. Do not touch act3/act4 markup; re-run their offset-0 diffs after any shared
   engine/style change.

## 11. Round 2 results

Method unchanged (§7, offset 0, whole-film = concat of the 3 act renders).

| scope | mae | avg_p8 | worst_p8 | p8>0.08 frames |
|---|---|---|---|---|
| R1 whole film | 3.311 | 0.0356 | 0.845 | 59/1254 |
| **R2 whole film** | **2.769** | **0.0316** | **0.842** | **57/1254** |
| R1 act1 | 4.650 | 0.0353 | 0.533 | 11/360 |
| **R2 act1** | **4.462** | **0.0346** | 0.533 | 11/360 |
| R1 act3 | 2.598 | 0.0357 | 0.846 | 47/654 |
| **R2 act3** | **1.818** | **0.0309** | 0.842 | 45/654 |
| R1 act4 | 3.211 | 0.0366 | 0.456 | 1/240 |
| **R2 act4** | **2.788** | **0.0300** | 0.447 | 1/240 |

Both fixes are exact-reference ports (numbers copied from the reference source),
not fitted.

### What changed (files, all `examples/`)
- **`website-to-hyperframes-act4.xml` — caption never hid (the round's biggest
  single win).** `popCaption(id, at)` popped the pill in and *held it forever*;
  it never scheduled the fade-out. The reference master declares each act4
  caption as a timed clip — `data-start="35.80" data-duration="2.83"` (cap1) and
  `data-start="40.10" data-duration="1.21"` (cap2); act4 starts at abs 33.8s, so
  local 2.00/6.30 with the *data-duration* window. Measured: opencat's cap1
  lingered frames 145–189 (past its 4.83s end) and cap2 226–239, where the
  reference shows no caption; the whole bottom band was wrong there. Added a
  3rd arg (`dur`) + `T.set(id,{opacity:0}, at+dur)`; call sites now
  `popCaption('cap1', 2.0, 2.83)` / `popCaption('cap2', 6.3, 1.21)`. act1 and
  act3 already had this hide (act1 `at+dur`, act3 `T.set(c,{opacity:0},at+dur)`),
  so only act4 was broken. act4 avg_p8 0.0366 → 0.0326 from this alone.
- **`website-to-hyperframes-act3.xml` — bottom `.cap` used the clip duration.**
  The 11 `m01..m11` master captions were shown `at..at+dur` (the *video* window).
  The reference master gives each its own shorter window (`data-start`
  12.20/13.70/15.60/…, `data-duration` 1.25/1.35/1.02/…). Added an `MCAP` table
  (local start/dur = master − 12.0) and drove `m01..m11` from it, leaving the
  top-left `.a3-cap` (c01..c11) on the full clip. Clears the caption overrun in
  clips 2/3/9/11. avg_p8 0.0357 → 0.0326.
- **`website-to-hyperframes-act1.xml` / `act3.xml` / `act4.xml` — bottom caption
  4px too high.** Reference `.cap` is `bottom:88px`; the XMLs pinned the wrap at
  `bottom-[101px]` (act4 `bottom-[99px]`). Cross-correlation gave op ≈ 4–5px
  high on every act (cap-text rows ref 920–979 vs op 917–974 at act4 f100).
  Set `bottom-[95px]` (act1/act3) and act4 `bottom-[99px]→bottom-[95px]`. After:
  residual sub-pixel shift ±0.5px on act1/act3/act4. act1 4.650→4.462,
  act3 2.630→1.818, act4 2.966→2.788.

### Confirmed NOT the problem / rejected (negative results, don't re-chase)
- **act4 wordmark is NOT offset.** A raw grid made the "Hyper"/"Frames" band
  (p8≈0.44) look shifted, but cross-correlation of the white glyphs puts the
  best shift at dx=0, dy=1 (mae barely moves) and the white x/y extents are
  identical (332–907 / 346–543 both). The band p8 is simply the gradient
  ("Frames") legibility delta — reference's gradient decodes with mid-pixels
  below threshold in places where opencat's is 1–2 levels over; 200px glyph
  edges flip the >8 classifier. `top-[320px]→top-[321px]` was tested and made
  act4 **worse** (0.02995→0.03148) — reverted, kept 320px. Not a positional bug.
- **act4 install pill / command text ~5px high** (ref command rows 830–857 vs op
  825–853) but the region's contribution is tiny (install band p8 ≈ 0.0014); left
  as-is this round to avoid another unverified shift.
- **`backdrop-blur-[8px]` / `backdrop-blur-[12px]` on act4's install-inner and
  cap pills are silently ignored** (parse warning; no bracket rule matches the
  arbitrary `backdrop-blur-[Npx]` form — cf. `blur-[12px]` which does match).
  Never-rendered backdrop filter, so it is not a regression; the pill's own
  translucent fill + border dominate. Engine gap if a future gate needs it.
- **act3 f9/f13/f14/f29 etc. remain** — inside-video cut transients (the
  reference decoder jumps a frame at a video splice), as recorded in R1 §10. Not
  markup; worst_p8 is stuck at ~0.84 for this reason.

### Engine
No `crates/` changes this round (no `cargo` rebuild needed; §10's engine state
still applies). Both wins are XML-only exact ports.

### Next round
1. **act4 wordmark region is the largest remaining stable block** (p8≈0.44 at
   frames 145–239, ≈ half of act4's residual). It is a gradient-edge / decode
   legibility floor, not a shift — only worth chasing if a per-channel metric
   (e.g. p4) shows opencat is robustly *wrong* there; do not shift the geometry.
2. **act4 install pill** sits ~5px high vs the reference (rows 825 vs 830);
   cheap to test a `top-[810px]→top-[815px]` port if pursuing sub-0.030.
3. **G7 steps() typing** (act1) still open — opencat advances N even steps vs the
   reference `round(i*N/total)`; likely a 1-char lag on the 36/38-char prompts.
4. The task gate (avg_p8 ≤ 0.02) is still far below the pipeline floor; the
   remaining act3 mass is inside-video cut frames plus the video codec itself.

## 12. Round 3 results

Method unchanged (§7, offset 0, whole-film = concat of the 3 act renders).

| scope | mae | avg_p8 | worst_p8 | p8>0.08 frames |
|---|---|---|---|---|
| R2 whole film | 2.769 | 0.0316 | 0.842 | 57/1254 |
| **R3 whole film** | **2.540** | **0.0272** | **0.761** | **46/1254** |
| R2 act1 | 4.462 | 0.0346 | 0.533 | 11/360 |
| **R3 act1** | **4.462** | **0.0346** | 0.533 | 11/360 |
| R2 act3 | 1.818 | 0.0309 | 0.842 | 45/654 |
| **R3 act3** | **1.405** | **0.0225** | **0.762** | **34/654** |
| R2 act4 | 2.788 | 0.0300 | 0.447 | 1/240 |
| **R3 act4** | **2.714** | **0.0296** | 0.456 | 1/240 |

(The R3 baseline was re-measured this session at 2.607 / 0.03058, marginally
better than R2's committed 2.769 / 0.0316 — reference-side drift; the deltas
below are against this session's own pre-change render.)

### What changed (files) — engine video frame-selection semantics

- **`crates/opencat-engine/src/media/decode.rs` — the video decoder selected
  the wrong source frame (systematic, whole-act-3 win).** `receive_until`
  selected the **first** source frame with `pts >= target`, which *leads* the
  reference by up to one source frame; a browser `<video>` (the reference
  renderer) shows the **last** frame with `pts <= target` (floor). Rewrote
  selection to floor semantics: hold the current frame until the *next* source
  frame's pts exceeds the target, storing that frame in a new `ahead_source`
  slot so the following request is already decoded. Split packet/frame reading
  into `decode_next_frame`; `decode_forward` now runs a floor-advance loop and
  **drains the frame-threaded decoder after packet exhaustion** (a stale
  `eof` short-circuit previously discarded frames still buffered in the
  decoder after a seek — the codex regression class).
- **`crates/opencat-engine/tests/video_decode_monotonic.rs` — expectation
  updated** from "first pts >= target" to browser floor semantics
  (`expected_frame` now finds the last frame with pts <= target), and the
  freeze check now compares each displayed frame against a per-source-frame
  byte anchor instead of a neighbour (`assert_ne`), which false-failed on
  byte-identical static content (adjacent fig source frames are identical).
- **`Cargo.toml`/`Cargo.lock`**: none (no dep changes).

Measured evidence (single-frame mae vs the reference act-3 segment): local
f12 11.87→3.24, f13 8.62→3.20, f14 9.33→3.22, f24 6.69→3.30, f29
10.50→3.24 — i.e. the video content now lands on the correct source frame;
the residual ~3.2 is the reference h264 encode/codec floor (a static frame
compared to the same source clip frame is ~2–5). act3's clip-1 cluster
(f0–29) fell from mass 5.06 (avg 0.169) to 2.28 (avg 0.076).

### Remaining act3 outliers (NOT systematic — don't re-chase)
- **f10 (p8 0.76)** is a clip-boundary tie: media-local 0.333s is exactly
  between source frames at 0.292 and 0.333; the render path resolves the
  boundary to the frame *before* the reference does. A CPU-only query first
  observed the *opposite* rounding than the render — the frame-threaded
  decoder's delivery order at the boundary makes the tie non-deterministic
  between runs. It is one frame per boundary (f10, and part of f30/f31 etc.).
  Do not chase without a deterministic tie rule.
- The residual act3 windows (30–59, 60–89, 210–269) are inside-video cut
  transients and the codec floor, not layout.

### Next round
1. **Clip-boundary tie rule.** Media-local `frame/30` hits source-frame
   boundaries exactly when both rates divide; a deterministic half-open rule
   (`pts < target + half_interval`) would make f10-class frames pick the
   reference's frame. Test against act3 f10/f31 before/after act-by-act.
2. **act1 G7 steps() typing** still open (§10): opencat advances N even steps
   vs `round(i*N/total)`.
3. **act1 YEAHH (f317–322).** Bungee Shade renders yellow-filled in opencat
   (the grey "inline" strokes the reference has are the font's inline relief
   reading as grey on the opencat raster); the residual there is the font-
   metric/shadow-offset mismatch noted in §10 G4, plus sub-frame timing — not
   a colour bug.
4. **act4 wordmark gradient floor** (p8≈0.04 blocks) and **install pill ~5px
   high** remain the act4 levers (§11 next-round 1–2).
5. The task gate (avg_p8 ≤ 0.02) is still below the pipeline floor; act1 is
   now the p8 ceiling (0.0346), act3 the mae floor is codec + cut transients.

## 13. Round 4 results

Method unchanged (§7, offset 0, whole-film = concat of the 3 act renders).

| scope | mae | avg_p8 | worst_p8 | p8>0.08 frames |
|---|---|---|---|---|
| R3 whole film | 2.540 | 0.0272 | 0.761 | 46/1254 |
| **R4 whole film** | **2.269** | **0.0226** | **0.679** | **16/1254** |
| R3 act1 | 4.462 | 0.0346 | 0.533 | 11/360 |
| **R4 act1** | **4.044** | **0.0312** | **0.533** | **10/360** |
| R3 act3 | 1.405 | 0.0225 | 0.762 | 34/654 |
| **R4 act3** | **1.128** | **0.0153** | **0.679** | **5/654** |
| R3 act4 | 2.714 | 0.0296 | 0.456 | 1/240 |
| **R4 act4** | **2.714** | **0.0296** | **0.456** | **1/240** |

(The R4 baseline was re-rendered this session at 2.533 / 0.02733, matching
R3's committed 2.540 / 0.0272 within reference-side drift.)

### What changed (files)

- **`crates/opencat-engine/src/media/decode.rs` — microsecond-unit boundary
  comparison (the big win; act3 0.0225 → 0.0153, worst 0.762 → 0.679).** Core
  hands the host a **microsecond-quantized** target: clip-local media time
  `frame/30` is passed through `secs_to_micros` (`round`), so the nominal source
  boundary `8/24 s = 0.333333333…` arrives as `0.333333`. The decoder's floor
  test compared the successor's exact rational pts against this rounded target
  in raw f64 (`0.333333333 <= 0.333333 + 1e-9` is false), so it kept the
  *previous* source frame — a systematic one-frame lag on the ~12 clip-boundary
  frames. `decode_forward` now compares both sides on the same whole-microsecond
  grid (`floor_advance`: `round(ahead_pts*1e6) <= round(target*1e6)`). f10/f25
  are the clean cases (mae 24.5→1.8, 6.0→1.6).
  - Verified this is **floor + microsecond rounding**, not nearest-frame: a
    nearest-frame variant was tested and rejected (it fixed f11/f31/f81 but
    broke f12/f22/f27/f32/f77/f82 and was net-worse: 12 frames worse vs 8
    better). The reference itself shows a non-deterministic boundary pick (f11
    matches source frame 8+1 while f12 matches the floor 9), so no single
    offset rule can match every boundary; the principled unit fix is kept.
  - Pure helper `floor_advance(target_micros, ahead_pts_secs)` + unit tests
    (`cargo test -p opencat-engine --lib media::decode`: 3 passed).
- **`examples/website-to-hyperframes-act1.xml` — phrase letter-spacing moved
  from the 160px word elements to the wrapping div (act1 4.462 → 4.044,
  0.0346 → 0.0312).** The reference declares `letter-spacing:-0.025em` on
  `.phrase-wrap` (whose font-size is the base ~16px) and the 160px `.pw-N`
  font-size is set only on the words. CSS resolves `em` at the element that
  *declares* the value, so the effective spacing is ≈ −0.4px, not −4px. Because
  the words are `display:inline-block`, browser letter-spacing does **not**
  apply inside them; opencat inherited the tracking onto each 160px `<text>`
  and applied −4px per gap, rendering "any site becomes" 54px (4%) narrower
  than the reference. Moving `tracking-[-0.025em]` onto `#phrase` reproduces
  the reference width exactly (blue-word extent 1365px vs ref 1366px; was
  1312). The 9 other `tracking-[-0.025em]` uses (60px `.mt` morph prompts,
  whose font-size also lives on the same element as the tracking) are left
  untouched and remain correct.

### Confirmed NOT the problem / rejected (negative results, don't re-chase)

- **Nearest-source-frame selection.** Tested and reverted (see above). The
  documented browser `<video>` contract is floor; the reference capture's
  boundary frames are inconsistent, so nearest-frame is not a better model.
- **act3 interior mass is now the codec floor.** After the fix the top act3
  windows are all ≈0.5–1.0 mass per 30 frames of uniform glyph/AA noise; the
  only residual outliers are f11/f31/f81 (a3-01-stripe source-fraction 0.8,
  where the reference picks the next frame — the non-deterministic boundary
  class, not a rule we can encode).
- **act4 unchanged** (no shared-engine or markup change affected it); its
  residual remains the wordmark gradient decode floor (§11) and f0 window
  boundary (§10).

### Next round
1. **act1 scatter f245–248 remains the single worst cluster** (mass 2.09, worst
   f248 p8 0.53). The opencat panels are *correctly positioned and rotated*
   (top-left panel bbox/tilt match within 1–3px at f246–248) but the **drawn
   size differs on transient frames**: at f246 ref panel width 543px vs op
   817px, even though the transform `scale: 0.52` matches the reference exactly
   and they converge again at f249. Hypothesis: opencat applies the scatter
   transform to the panel's own box (a 1400×780 panel scales toward its own
   center) while the reference composes `translate` on the outer `.term` and
   `scale` on an inner layer, so the transient in-between scales differently.
2. **act4 install pill `top-[810px]→top-[815px]`** (R2 §11 port) — untested;
   the region is tiny (band p8 ≈ 0.0014) but it is the last cheap act4 port.
3. Do not re-chase act3 boundaries (see negative result) or the act4 wordmark
   gradient (decode floor).

## 14. Round 5 results

Method unchanged (§7, offset 0; whole-film = concat of the 3 act renders,
compared against the full reference).

| scope | mae | avg_p8 | worst_p8 | p8>0.08 frames |
|---|---|---|---|---|
| R4 whole film | 2.269 | 0.0226 | 0.679 | 16/1254 |
| **R5 whole film** | **2.252** | **0.02216** | **0.680** | **15/1254** |
| R4 act1 | 4.044 | 0.0312 | 0.533 | 10/360 |
| **R5 act1** | **3.969** | **0.0306** | 0.533 | 10/360 |
| R4 act3 | 1.128 | 0.0153 | 0.679 | 5/654 |
| **R5 act3** | **1.128** | **0.0153** | 0.680 | 5/654 |
| R4 act4 | 2.714 | 0.0296 | 0.456 | 1/240 |
| **R5 act4** | **2.714** | **0.0296** | 0.456 | 1/240 |

This session's own baseline re-render (the working tree already carried the
codex-port below) was 2.252 / 0.02216; the committed-HEAD act1 re-rendered at
4.044 / 0.0312, so the deltas below are measured both ways.

### What changed (files)

- **`examples/website-to-hyperframes-act1.xml` — Codex panel text-fidelity
  (act1 4.044 → 3.969, 0.03117 → 0.03061).** Three 1:1 ports of the reference
  `.term-codex` CSS:
  1. `.co-plus` ("✦ Get Plus"): the XML had this as a single `<text>` with
     `px-[12px] py-[4px] bg-[#7a6aff] rounded-full` at 14px. Padding is inert on
     a text node, and the reference `.co-plus` is **11px**, not 14px. Wrapped it
     in a `<div>` pill (`px-[12px] py-[4px] bg-[#7a6aff] rounded-full`) with an
     inner 11px `<text>` — matches `.co-plus { padding:4px 12px; font-size:11px }`.
  2. `.co-center`: reference is `position:absolute; left:50%; top:180px;
     transform:translateX(-50%)` (the 180px is measured from the panel top, i.e.
     header included). The XML pinned the centre block at `left-[634px]
     top-[126px]`; set to `left-[600px] top-[180px]` (panel 1400 wide → centre
     600 = (1400−200)/2; top 180).
  3. `co-brand` logo: added `mx-auto` (reference `.co-brand { margin:0 auto 14px }`).

### Ruled out this round (negative results, don't re-chase)

- **act1 scatter f245–248 (mass 1.50, worst f248 p8 0.53) is a reference-capture
  transient, not an opencat bug.** Frame-by-frame ref↔op matching shows ref f246
  best-matches op **f245** (44 vs 57), f247↔op245 (59 vs 78), f248↔op245 (70 vs
  93): the reference is one *frame* behind the whole window, then snaps to
  f249 == op249 exactly. A web-oracle run (real Chrome GSAP) over f244–250 is
  *even further* from the reference (mae 95–100), so it is not a JS-easing bug —
  the browser reproduces opencat's timeline. The reference's top-left panel
  **shrinks around screen-centre (x_final 117+…, i.e. scale happens about 960,540)
  and only then jumps to the corner**, whereas opencat translates+scales together
  (x_final 121 → scale about the 820,540 centre) — a *scale-then-translate*
  ordering. GSAP (`x`/`y`/`scale` on one element) emits a single
  `translate·rotate·scale` matrix; reproducing the reference's transient would
  need a `translate`-only wrapper + inner `scale` layer, which is not what the
  reference DOM does, and a duration change (0.20→0.60) confirmed opencat *does*
  advance over 6 frames (f245 TLleft 246, f246 188, f247 143), so the duration is
  not truncated. Verdict: exotic capture-side in-between; not reproducible. A
  global ±1-frame shift makes the whole film worse (act1 0.0306→0.0629/0.0675),
  so no shift should be applied.
- **YEAHH stars (`.star` ✦ ×4) are declared in the XML but had no animation at
  all** (§10 G4 left them static `opacity-0`). Porting the reference's
  `fromTo(stars,{opacity:0,scale:0},{opacity:1,scale:1,dur:0.2,stagger:0.04,
  back.out(1.8)},10.55)` + `to(stars,{opacity:0,dur:0.14},10.95)` (and trying
  both sibling- and child-of-`#yeahh` placement, with and without the wrap's
  rotate(−3°)/scale) is **net-negative**: the reference's star centres after the
  wrap scale are ≈(364,405)/(1519,365)/(570,697)/(1355,681), and the `✦` glyph
  pivot + the wrap scale put opencat's at (354,436)/(1511,334)/(576,718) — a
  15–30px offset that adds MORE diff pixels than showing nothing (act1
  0.03061→0.03070; f318–330 each +0.001–0.002). Reverted. The stars are a
  ~0.02 p8 region; not worth a fitted-transform chase.
- **YEAHH pop transient f318–320 (mass 0.44).** The reference grows the letters
  markedly inside the 10.55–10.73 pop-in (f318–321 yellow spans 1383→1423px
  wide) while opencat's are near-static (1317→1391). Same class as the scatter:
  a capture-side scaling in-between, not a markup/layout error. The static
  frames f324–332 differ only by the font-metric/grey-inline relief floor (§10
  G4), ~0.077 p8, also not a bug.
- **act1 background offset is sub-p8.** opencat `#f5f3ee` vs the reference's
  decoded bg is a uniform −2.82/−1.82/−1.82 (R,G,B) shift on *every* frame
  (measured identically at f150/195/205), giving |d|>8 on only 2–3% of pixels
  and |d|>16 on ~1–2% — below the p8 classifier except on glyph edges. It
  inflates mae by ~0.6–0.7 (raw 2.77 vs 2.05 if the shift is removed) but is a
  reference-encode artifact (§10). Do not warp the XML.
- **act1 typing f51–57 / act4 wordmark** are Skia-vs-reference AA/raster floors:
  the act4 `.w-frames` gradient is `linear-gradient(95deg,#533afd 0%,#00c3ff
  45%,#3ecf8e 100%)` in both reference and XML (exact match), so the ~0.04 p8
  block is decode legibility on 200px gradient glyphs, not a parameter error.
- **act3 worst_p8 (f31 0.68, f11 0.49, f81 0.42)** are the inside-video splice
  transients of §12/§13; unchanged and not markup. act4 f0 (0.45) is the
  §10 window boundary.

### Engine
No `crates/` changes this round (no rebuild needed; §12/§13 engine state still
applies). All changes are XML-only exact ports; no `cargo test` impact.

### Next round
1. The pipeline floor is now ~0.022 whole-film (mae 2.25). The single biggest
   remaining block is act1's scatter transient (mass 1.50 across 4 frames) which
   is a reference-capture artifact — the only lever left there is a
   `translate`-wrapper/`scale`-inner transform split, which the reference DOM
   does not use; treat as closed unless a per-frame web-capture comparison
   becomes available.
2. act1's broad pre-scatter floor (mass 5.2 across 245 frames, avg 0.021) and
   act4's wordmark band (mass ~6.4) are both AA/raster/decode floors, not
   layout. The task gate (avg_p8 ≤ 0.02) is below this pipeline's empirical
   floor (§4); recommend recalibrating the gate rather than chasing it.
3. Do not re-chase the scatter transient, the YEAHH stars/pop, or the global bg
   offset (all measured net-negative or sub-threshold this round).

## 15. Round 6 results

Method unchanged (§7, offset 0; whole-film = concat of the 3 act renders,
compared against the full reference).

| scope | mae | avg_p8 | worst_p8 | p8>0.08 frames |
|---|---|---|---|---|
| R5 whole film | 2.252 | 0.02216 | 0.680 | 15/1254 |
| **R6 whole film** | **2.229** | **0.01994** | **0.680** | **16/1254** |
| R5 act1 | 3.969 | 0.0306 | 0.533 | 10/360 |
| **R6 act1** | **3.979** | **0.0305** | 0.533 | 10/360 |
| R5 act3 | 1.128 | 0.0153 | 0.680 | 5/654 |
| **R6 act3** | **1.119** | **0.0148** | 0.680 | 5/654 |
| R5 act4 | 2.714 | 0.0296 | 0.456 | 1/240 |
| **R6 act4** | **2.631** | **0.0181** | 0.446 | 1/240 |

This round's baseline re-render (clean HEAD) was 2.252 / 0.02216, matching R5's
committed number exactly.

### What changed (files) — engine: linear-gradient direction was aspect-ratio dependent

- **`crates/opencat-core/src/render/helpers.rs` — linear gradients were evaluated
  in anisotropic normalized space, so the perceived angle skewed with the box
  aspect ratio (the single biggest remaining block: act4 wordmark, mass ≈ 6.4
  over 240 frames).** `build_linear_gradient` emitted unit-box endpoints
  (`to_local`: `px/w`, `py/h`) plus an anisotropic `local_matrix = scale(w, h)`.
  A Skia linear shader with a local matrix `M` and unit endpoints `a→b`
  evaluates the field along `(b-a)ᵀ M⁻¹`, so in device space the direction
  becomes `(dx/w², dy/h²)` instead of `(dx, dy)`. Measured: `linear-gradient(95deg)`
  rendered at **54°** in a 400×100 box, **19°** in 400×200, **5°** in 400×400
  (CSS must be 5° regardless). The act4 `.w-frames` gradient is a 200px glyph
  run ~840px wide × ~200px tall; its 95deg gradient rendered at ~53.5° (2D plane
  fit of the green channel), turning the word into a diagonal red→green wash
  instead of the reference's near-horizontal purple→cyan→green sweep.
  Fix: emit the endpoints directly in **device space** (offset by the box
  origin) with `local_matrix: None`; the raw box-space endpoints already carry
  the correct CSS angle, and no matrix is needed to keep them size-independent.
  Extracted `GradientBox::to_device`; removed the now-unused `matrix`/`to_local`.
  - Replaced three unit-box linear tests with device-space expectations
    (`linear_horizontal_maps_unit_square_onto_rect`,
    `linear_horizontal_transparent_stop_keeps_alpha_fade`,
    `repeating_linear_scanline_uses_px_period`).
  - `cargo test -p opencat-core` → 652 passed, 0 failed; `cargo build --release
    --bin opencat` green. Radial gradients (which legitimately need the
    anisotropic matrix to make ellipses) are untouched.

Measured evidence (single-frame p8 vs the reference): act4 f1180 wordmark-region
p8 0.3307 → 0.0918, f1210 0.3291 → 0.0972; whole-frame f1180 0.0333 → 0.0188,
f1210 0.0386 → 0.0244. Aspect-ratio probe after the fix: 400×100 / 400×200 /
400×400 all render 95deg at **5.0°**. k3-promo (the other aligned title, which
uses `linear-gradient(90/100/115/132deg)` patterns) re-measured at mae 1.2496 /
avg_p8 0.01453 — no regression (its bars are wide/short, but the 90deg families
were already correct and stay correct).

### Confirmed NOT the problem / rejected (negative results, don't re-chase)

- **Master `.cap` text-shadow** (`0 2px 14px rgba(0,0,0,0.55), 0 0 2px
  rgba(0,0,0,0.35)`) is absent from all three XMLs' caption texts. Ported it to
  act1 `mt1..mt4` and A/B'd f120/150/200: **net-negative** (p8 0.0260→0.0271 at
  f150, mae +0.03). opencat's caption glyphs already raster slightly *heavier*
  than the reference (f150 caption white px 31073 vs 28768), so adding a shadow
  darkens the wrong side of the edge. The caption band remains an AA floor.
- **Caption font-weight 800→700** (the reference uses the Inter *variable* face,
  weight 800; `Inter-800.ttf` static is marginally heavier). Render A/B showed
  the caption pixels are identical (the weight class isn't re-rasterized
  differently for the band) — no gain; reverted, not landed.
- **act3 f391/f371/f441 worst frames remain inside-video splice transients**
  (whole-frame content lags at a source cut; f391 p8 0.68, err spread over the
  full frame, top-left caption only 0.39 of it). Documented class (§12/§13); not
  markup, dominates `worst_p8` and cannot be closed without a deterministic
  inside-video frame rule.
- **act1 scatter f245–248, YEAHH f318–321, and the broad act1 caption/panel AA
  floor** are unchanged (reference-capture transients + Skia raster, §14).

### Next round
1. Whole-film avg_p8 is now **0.0199**, i.e. it clears the 0.02 task gate for
   the first time; `worst_p8` (0.68) still fails the 0.08 gate and is pinned by
   the act3 inside-video splice transient (f391), which is not markup. If the
   gate must be met, the lever is a deterministic inside-video frame-selection
   rule, not layout.
2. Remaining blocks are all floors: act1 caption/panel AA (~0.020 pre-scatter),
   act1 scatter transient (capture artifact), act4 wordmark residual (now
   gradient-correct; the rest is decode legibility), act3 codec+splice.
3. Do not re-chase the caption text-shadow, the scatter transient, the YEAHH
   stars/pop, or the global bg offset — all measured net-negative or
   sub-threshold.

## 16. Round 7 results

Method unchanged (§7, offset 0; whole-film = concat of the 3 act renders,
compared against the full reference).

| scope | mae | avg_p8 | worst_p8 | p8>0.08 frames |
|---|---|---|---|---|
| R6 whole film | 2.229 | 0.01994 | 0.680 | 16/1254 |
| **R7 whole film** | **2.200** | **0.01984** | **0.680** | **16/1254** |
| R6 act1 | 3.979 | 0.0305 | 0.533 | 10/360 |
| **R7 act1** | **3.905** | **0.0303** | 0.533 | 10/360 |
| R6 act3 | 1.119 | 0.0148 | 0.680 | 5/654 |
| **R7 act3** | 1.119 | 0.0148 | 0.680 | 5/654 |
| R6 act4 | 2.631 | 0.0181 | 0.446 | 1/240 |
| **R7 act4** | **2.536** | **0.0176** | 0.446 | 1/240 |

This session's baseline re-render (clean HEAD) matched R6: act1 3.969 / 0.03061,
act3 1.128 / 0.01531, act4 2.593 / 0.01787 → whole 0.0202. (Reference-side drift
is ±0.001 in p8; the deltas below are against this session's own pre-change
render.)

### What changed (files) — all `examples/`, exact-reference ports

- **`website-to-hyperframes-act1.xml` — CTA `$ ` prompt had no `margin-right`
  (act1 3.969 → 3.905, 0.03061 → 0.03030; whole-film mae 2.229 → 2.200).** The
  reference `.cta-prompt { color:#c86d3f; margin-right:4px }` follows the
  `$&nbsp;` span; the XML fused it into a single `<text>$ </text>` with no
  margin, so the typed `cta-cmd` started **4px too far left** from f0.28
  (typing start): ref `cta-cmd` white extent x0 217 vs op 216 at f28 → at f30
  ref x1 810 vs op 803 (a whole ~7px). Fix: added `mr-[4px]` to `x001`.
  Measured single-frame: CTA-region (x180–900, y510–585) p8 0.223 → 0.171 at
  f30, 0.258 → 0.206 at f40/f45; whole-frame f30 0.0246 → 0.0232. Every CTA
  frame f28–53 improved (184/360 act1 frames better, 63 worse, 113 unchanged).
- **`website-to-hyperframes-act4.xml` — wordmark block 1px high (act4 2.593 →
  2.536, 0.01787 → 0.01758).** `#wm-wrap top-[320px] → top-[319px]`. The 320px
  position (R2 §11 reverted a 321px test as worse) sits 1px *below* the
  reference; a 3-value probe (318/319/320) at f166/178/196 plus 5 more frames
  (f60/100/130/150/220) shows **319 is the unique minimum** — 8/8 frames better
  by ≈0.06 mae (f192 wholemae 3.559→3.494), 320 second, 318 worst (318 means
  the block is 2px too high). The reference `.wordmark-wrap top:320px` is
  measured from a 1920-wide flow box whose first line box has no leading offset;
  opencat's 200px/line-height:1 text node lands 1px lower, so 319 reproduces the
  ref glyph rows. Promise lines (`top-[586px]`/`top-[664px]`) tested +1/−1 — 0
  is already optimal (better by 0.008–0.009); the earlier "promise lines sit a
  hair low" note was a crop illusion. Install pill already matches within 2px
  (ref rows 830–857 vs op 832–858; the R2 §11 "5px high" reading was f100 of an
  older render — re-measured at f200/220 it is ≤2px and the band is ~0.001 p8).

### Changed the graded act division but not a real defect (negative, don't chase)

- **act1 typed `cta-cmd` char-count lead (f32).** The reference `steps(36)`
  easing advances one glyph per `round(i/36 * 36)` — i.e. `floor` — so when the
  clip fraction lands exactly on a char boundary the reference is one char
  *ahead* of opencat's `steps` output (f32 ref white extends to x888 vs op 857).
  This is the global CSS `steps()` rounding (a browser/engine convention), not a
  parameter error, and it is a single transient through the 0.9s ramp. Not
  chased; the 4px margin port above removed the *systematic* offset that
  dominated every CTA frame.

### Confirmed NOT the problem / rejected (negative results, don't re-chase)

- **`backdrop-blur-[12px]` on the caption pills is unparsed** (no bracket rule
  matches the arbitrary form; only `backdrop-blur-md`=12.0 exists). Porting the
  captions to `backdrop-blur-md` to enable the reference's real
  `backdrop-filter: blur(12px)` is **strongly net-negative**: it drops the pill's
  own rendered shadow (Skia `SaveLayer` semantics) and over-darkens the pill;
  act1 f150 wholemae 3.945 → 4.498, caption-region mae 8.6 → 25.0. Leave the
  pill without the backdrop filter (its translucent fill + border dominate and
  currently match the reference's pill fill within ~3 levels).
- **act1 "Let's build" weight** (ref `.co-title { font-weight:500 }`): the XML's
  `font-medium font-[inter400]` uses the static Inter-400 face — switching to
  the variable `font-[inter]` (500) produced **byte-identical** output (the
  weight class isn't re-rasterized differently), so no change was landed. The
  region's p8 (~0.06–0.17) is the panel's faint secondary-text AA floor.
- **act1 panel "new thread"/header band and prompt-line AA** are Skia-vs-
  reference raster floors (both sides render the same extents/colors; every
  diffuse blob is a glyph edge). No structural difference.
- **act4 wordmark gradient is correct** — re-profiled f192: the 95deg
  purple→cyan→green columns match the reference within 1–3 levels at every
  sample x (R6's angle fix holds). The residual is anti-aliased glyph edges on a
  gradient, not a parameter error.
- **act3 worst_p8 (f391 0.68, f371 0.49, f441 0.41)** remain the inside-video
  splice transients (§12/§13); act1 f245–248 scatter and f40 f1014 window
  boundary are unchanged capture-side artifacts (§14). `worst_p8` is pinned by
  these and cannot be closed by markup.

### Engine
No `crates/` changes this round (no rebuild needed beyond the up-to-date
`--bin opencat`). All changes are XML-only; `cargo test -p opencat-core` → 652
passed, 0 failed.

### Next round
1. Whole-film is at mae **2.200 / avg_p8 0.01984** — both below the R6 numbers
   and avg_p8 already under the 0.02 task gate; `worst_p8` (0.68) is still
   pinned by the act3 inside-video splice transient, which is not markup. If the
   gate must be met, the only lever is a deterministic inside-video boundary
   frame rule (the reference itself is non-deterministic there, §12).
2. Remaining blocks are all floors: act1 diffuse panel/caption AA (~0.030), the
   act1 scatter transient, act4 wordmark glyph-edge decode (~0.017), act3
   codec+splice. No single cheap port remains in act1/act4; further gains need
   per-glyph raster parity (Skia vs the reference capture), not layout.
3. Do not re-chase: caption backdrop-blur (net-negative), "Let's build" weight
   (no-op), the promise-line shift (already optimal), the scatter/YEAHH
   transients, the bg offset, or the act3 boundaries.


## 17. Round 8 results

Method unchanged (§7, offset 0; whole-film = concat of the 3 act renders,
compared against the full reference).

| scope | mae | avg_p8 | worst_p8 | p8>0.08 frames |
|---|---|---|---|---|
| R7 whole film | 2.200 | 0.01984 | 0.680 | 16/1254 |
| **R8 whole film** | **2.178** | **0.01969** | **0.680** | **14/1254** |
| R7 act1 | 3.905 | 0.03030 | 0.533 | 10/360 |
| **R8 act1** | **3.839** | **0.02979** | 0.533 | 8/360 |
| R7 act3 | 1.119 | 0.01480 | 0.680 | 5/654 |
| **R8 act3** | 1.119 | 0.01480 | 0.680 | 5/654 |
| R7 act4 | 2.536 | 0.01758 | 0.446 | 1/240 |
| **R8 act4** | 2.574 | 0.01786 | 0.446 | 1/240 |

This session's baseline re-render (clean HEAD) matched R7: whole 2.1998 /
0.019841. (act4 shows ±0.04 mae reference-side drift; act4 was untouched this
round — its R7 number is from the previous session.)

### What changed (files) — one XML-only exact-reference port

- **`website-to-hyperframes-act1.xml` — YEAHH had no static inner rotation
  (act1 mass_p8 10.91 → 10.72, mae 3.915 → 3.839; whole-film mae 2.1998 →
  2.1781, avg_p8 0.019841 → 0.019693).** The reference `.yeahh { font-family:
  "Bungee Shade"; font-size:240px; transform: rotate(-3deg);
  text-shadow:3px 3px 0 #ff006e,6px 6px 0 rgba(0,0,0,0.4) }` carries a
  **static CSS rotate(-3deg) on the text element**. opencat ported both the
  text-shadow and the GSAP wrap tween (`fromTo(yeahh, {rotation:-8},
  {rotation:-3})`, §10 G4 / §14), but dropped the inner static rotate — the
  XML's `<text id="yeahh-t">` had only `font-[bungee] text-[240px]
  leading-none text-[#ffd60a] [text-shadow:...]` and **no rotation class**
  (the engine has no `rotate-[Ndeg]` utility; only script transforms). Net
  effect: the reference's -3° (inner) composes with the wrap's -3° (at rest,
  tween 10.55–10.83 lands at -3°) for **-6°**, while opencat rendered only
  -3°. Top-edge tilt fit: ref **-5.6°**, opencat **-3.0°** (f322/325/330).
  Fix: `T.set('yeahh-t', { rotation: -3 }, 0);` before Phase 7, so the text
  child rotates -3° inside the wrap's own -3°.
  - Measured: tilt after fix **-5.6°** (f325) / -5.7° (f330), matching ref;
    f322–330 each mae **6.51→4.05** (Δ ≈ −2.45), the whole YEAHH window
    f313–333 total mae **143.5 → 116.0**; 332/360 act1 frames improved, 27
    worse (≤0.03, e.g. f252 +0.03), 1 unchanged. Whole-film p8>0.08 frames
    **16 → 14**.

### Confirmed NOT the problem / rejected (negative results, don't re-chase)

- **YEAHH pink shadow coverage.** After the rotation fix, f325 ref pink =
  15679 vs opencat 7938 (~half), and a best-fit shadow vector finds the pink
  at the same offset (dx=4,dy=3) but with ~half the overlap (ref 11658 vs op
  5774). The Bungee Shade inline "shade" region carries the pink 3px shadow
  where opencat's does not — a glyph-raster/shadow-placement floor, not a
  parameter error (offset already matches). Left as-is.
- **YEAHH `.star` ✦ ×4** remain dropped (net-negative to add, §14). The ref
  "far" pink (y 390–450 left, y 680–720 bottom) is *unexplained* by the
  post-rotation diff — the glyphs explain the bands but ~3–4k stray pink px
  sit where opencat shows none. Likely the `.star` decorations at the wrap
  corners (opacity-0 in opencat, on in ref at these frames), but every
  placement attempt was net-negative (§14), so not landed.
- **Phrase italic "video."** (`.pw-3 { font-style: italic }`). opencat's
  `italic` class is a silent no-op (no Inter italic face). Tested a synthetic
  sk**e**w via `T.set('pw3', { skewX: -N }, 0)` — recovers the slant but is
  net-negative (f305 video-band p8 0.1114→0.1210, whole mae 4.93→5.06);
  reverted. The residual is the glyph shape (a true italic face differs from
  a skewed roman), so no transform port helps.
- **act1/act4 static plateaus are Skia/AA + reference-bg-offset floors.** The
  phrase plateau f297–312 (mass 0.44) and the caption/panel bands (f60–239,
  mass ~0.023/frame) have structurally-identical layout (glyph extents match
  to ±2px vs the reference) — the diff rides the uniform ~−3 bg offset and
  glyph edges, not a layout bug. Confirmed this round: phrase word extents
  match exactly (dark "Now" 791–1135 both, blue "any site becomes" 277–1642
  both); act4 caption band text rows/pill rows within 0–2px.
- **act3 worst frames (f391 0.68, f371 0.49, f441 0.41) are source→capture
  content lags, not a rule we can encode.** Matched every clip-1 output frame
  (local 0–45) to its 36 source frames: opencat's floor-picks are *correct*
  (they beat or match the reference's own mae-to-source at every frame, e.g.
  L31 ref matches src25 (mae 4.72) while opencat shows src24 (mae 4.62) — the
  reference is itself off-by-one at f391). The reference capture's video layer
  lags its own source by ~1 frame at 80% boundaries; floor semantics (§12–13)
  is the principled rule and is already in place. worst_p8 (0.68) is pinned
  here and cannot be closed by markup.
- **Color tags are never emitted.** `_rgb24 → libx264` writes no
  color_range/space/transfer/primaries (all "unknown") while the reference is
  `range=tv, bt709`. This is the H1 background offset's likely source (§10) and
  inflates mae by ~0.6 on every frame, but is not a p8 contributor for static
  content (the f305 bg diff is |Δ|≈7–9, only 1–2 levels). An engine output
  change (tag or range-convert all clips) would be needed; out of scope for a
  markup port, and the measured gain is mae-only.

### Engine
No `crates/` changes this round (no rebuild needed beyond `--bin opencat`); all
changes are XML-only. `cargo build --release --bin opencat` green.

### Next round
1. Whole-film is at mae **2.178 / avg_p8 0.01969** (both under R7); `worst_p8`
   (0.680) is still pinned by the act3 source→capture video lag, which is not
   markup. avg_p8 clears the 0.02 gate; worst_p8 cannot meet 0.08 without a
   change to the *reference capture's* video timing.
2. The remaining act1 mass is now the diffuse panel/caption AA floor (~0.023/
   frame over f60–239) + the phrase plateau + the YEAHH pink-shadow floor; the
   YEAHH geometry lever (inner rotation) is now spent. act4 is the wordmark
   glyph-edge decode + caption band; act3 is codec + source lag.
3. Do not re-chase: the `.star` decorations, the italic synthetic skew, the
   caption backdrop-blur, the scatter transient, the bg offset, or the act3
   boundaries — all measured net-negative or floors.


## 18. Round 9 results

Method unchanged (§7, offset 0; whole-film = concat of the 3 act renders,
compared against the full reference).

| scope | mae | avg_p8 | worst_p8 | p8>0.08 frames |
|---|---|---|---|---|
| R8 whole film | 2.178 | 0.01969 | 0.680 | 14/1254 |
| **R9 whole film** | **2.142** | **0.01954** | **0.680** | **14/1254** |
| R8 act1 | 3.839 | 0.02979 | 0.533 | 8/360 |
| R9 act1 | 3.839 | 0.02979 | 0.533 | 8/360 |
| R8 act3 | 1.128 | 0.01480 | 0.680 | 5/654 |
| **R9 act3** | **1.050** | **0.01450** | 0.680 | 5/654 |
| R8 act4 | 2.574 | 0.01786 | 0.446 | 1/240 |
| R9 act4 | 2.574 | 0.01786 | 0.446 | 1/240 |

This session's baseline re-render (clean HEAD) reproduced R8 exactly:
whole 2.17809 / 0.019693. (act1 and act4 are untouched this round; their R8
numbers are from the previous session's higher-quality-encode render.)

### What changed (files) — one XML-only exact-reference port

- **`website-to-hyperframes-act3.xml` — the top-left `.a3-cap` card was 2px
  low (act3 mae 1.128 → 1.050, avg_p8 0.01480 → 0.01450; whole-film mae
  2.17809 → 2.14206, avg_p8 0.019693 → 0.019536).** All 11 cards carried
  `left-[56px] top-[56px]`; the reference declares `.a3-cap { left:56px;
  top:56px }` but is `position:absolute` inside the master's body-level clip
  layer, and its rendered border-left lands at **y58** while opencat's lands
  at y59 (measured on the 1px `border-left` bright column at x56, frames
  100/200/300: ref y[58,153], op y[59,152]). A 4-value bracket on the caption
  card region (y40–210, x56–430) at f30/100/200 gives top-[54px] as the unique
  minimum: f100 card mae 7.48 → 3.55, f200 5.15 → 2.48, f30 5.77 → 3.30; 53 is
  too high, 55 close, 56 (unchanged) worst. Changed all 11 to `top-[54px]`.
  The bottom `.cap` captions (m01..m11) and the clip content are untouched.

### Confirmed NOT the problem / rejected (negative results, don't re-chase)

- **Color tags (`_rgb24 → libx264`)** remain `unknown` vs the reference's
  `range=tv, bt709`. A metadata-only retag (`-c copy -color_range tv
  -colorspace bt709 -color_trc bt709 -color_primaries bt709`) on the opencat
  acts changes the decode from BT.601 to BT.709: act1 mae 3.830 → 3.738 but
  **p8 0.0298 → 0.0339**; act3 mae 1.128 → 1.321, p8 0.0148 → 0.0268; act4
  mae 2.536 → 2.709, p8 0.0176 → 0.0383. Net-negative on the gate metric
  everywhere — the reference's own BT.709 matrix is not what a decoder uses
  for opencat's untagged stream, so "matching the tag" does not reproduce the
  reference shift; an untagged stream decoded as BT.601 happens to be closer.
  Do not tag or warp the XML for this (mae-only, negative direction).
- **act1/act4 diffuse p8 mass is the bg + panel-flat decode offset + glyph AA,
  not layout.** Flat-region probes: light bg ref (241.1,241.1,234.1) vs op
  (244,243,236), Δ≈(+2.9,+1.9,+1.9); dark panel ref (23,24,27) vs op (24,25,30),
  Δ≈(+1,+1,+3). Both uniform and sub-p8 on their own; the p8 mass is entirely
  glyph edges over these offsets. Not a parameter error.
- **act4 install pill ~0.15 p8 is text-glyph AA, not a shift.** Command text
  bbox ref x[649,1270] y[830,857] vs op x[649,1270] y[830,858] — within 1px;
  pill interior fill diff (0.5,−0.2,−0.4) top / (0.2,−0.7,−0.9) bottom;
  glyph count ref 3481 vs op 3462. No positional or fill port available.
- **act4 bottom caption (`cap1`/`cap2`) ~0.12 p8 is glyph AA.** Text bbox ref
  x[655,1266] y[921,979] vs op x[656,1267] y[921,977] — x within 1px, baseline
  identical; the residual is the pill backdrop + glyph raster. Do not shift.
- **The 4 YEAHH `.star` ✦ blobs ARE confirmed present in the reference** at
  f325 as 4 clusters of ~400–490 pink px each at (364,406)/(1519,365)/
  (569,700)/(1355,682) — matching the R5 §14 star centres exactly. R5 tested
  every placement and found net-negative (the `✦` glyph pivot + wrap scale
  lands them 15–30px off), so they stay dropped; they are not a placement we
  can hit with a class port.
- **The YEAHH pop transient (f318–321, mass 0.65) is NOT a `transform` CSS
  rule opencat dropped.** The XML ports it exactly: `<div id="yeahh">` is a
  1920-wide full-width text-center block, and GSAP's default `transform-origin`
  is `50% 50%`, so scaling `#yeahh` scales about (960,540) — the same centre
  the reference's `.yeahh-wrap { left:50%; top:50%; translate(-50%,-50%) }`
  scales about. A per-frame yellow-span comparison (f318–334) shows ref and op
  agree to ≤2px once the pop ends (f324+: ref width 1305 vs op 1304) and differ
  only during the pop transient itself (f319 ref 1495 vs op 1392) — a
  capture-side in-between, the same class as the §14 scatter. Not markup.
- **act3 worst_p8 (f391 0.68, f371 0.49, f441 0.41) remain the clip-1
  source→capture boundary.** Local frame↔source-frame matching of clip 1
  (a3-01-stripe, 24fps, 36 frames) shows ref is one *source* frame ahead of op
  at every interior boundary (ref L4..9 → src 2,3,3,3,3,3 vs op 3,4,4,5,6,7;
  ref L31 → src25 vs op src24), i.e. the reference capture shows the *next*
  source frame for several frames after each 0.8·k boundary while opencat's
  documented browser floor semantics hold the current one. Not encodable
  without contradicting the floor contract that fixes every other act, and the
  reference itself is inconsistent (the handoff already records ref showing
  src25 at L31 vs src24 at L30). Pinned; not markup.

### Engine
No `crates/` changes this round (no rebuild needed beyond the up-to-date
`--bin opencat`). All changes are XML-only; `cargo test -p opencat-core`
unaffected.

### Next round
1. Whole-film is at mae **2.142 / avg_p8 0.01954** — both below R8 and avg_p8
   well under the 0.02 gate. `worst_p8` (0.680) cannot be met: it is pinned by
   the act3 clip-1 source-boundary lag, which is a property of the *reference
   capture*, not the markup (ref itself is one source frame ahead at the
   boundaries). `gatePass` therefore stays false on `worst_p8` alone.
2. Remaining stable blocks are all floors: act3 top-left caption glyph AA
   (the card geometry is now fixed), act4 wordmark gradient glyph edges
   (~0.048 whole-act), act4 install/caption glyph AA (~0.12–0.15 in-band),
   act1 panel/caption AA (~0.030), and the bg +flat-offset.
3. Do not re-chase: the color tags (net-negative), the act3 source boundaries
   (reference-side), the YEAHH stars/pop, the scatter transient, the italic
   synthetic skew, or the caption backdrop-blur.

## 19. Round 10 results

Method unchanged (§7, offset 0; whole-film = concat of the 3 act renders,
compared against the full reference).

| scope | mae | avg_p8 | worst_p8 | p8>0.08 frames |
|---|---|---|---|---|
| R9 whole film | 2.142 | 0.019536 | 0.680 | 14/1254 |
| **R10 whole film** | **2.123** | **0.019494** | **0.680** | **14/1254** |
| R9 act1 | 3.830 | 0.029789 | 0.533 | 8/360 |
| R10 act1 | 3.830 | 0.029789 | 0.533 | 8/360 |
| R9 act3 | 1.059 | 0.015051 | 0.680 | 5/654 |
| **R10 act3** | **1.018** | **0.014528** | 0.680 | 5/654 |
| R9 act4 | 2.536 | 0.017580 | 0.446 | 1/240 |
| R10 act4 | 2.536 | 0.017580 | 0.446 | 1/240 |

This session's baseline re-render (clean HEAD) reproduced R9 exactly: whole
2.14206 / 0.019536, act3 1.05875 / 0.015051. (act1 and act4 are untouched this
round.)

### What changed (files) — one XML-only exact-reference port

- **`website-to-hyperframes-act3.xml` — the top-left `.a3-cap` card was
  animated in, but the reference never animates it (act3 mae 1.05875 → 1.01786,
  avg_p8 0.015051 → 0.014528; whole-film mae 2.14206 → 2.12335, avg_p8
  0.019536 → 0.019494).** In the reference `index.html` the `.a3-cap` rule
  (§2, lines 31–48) has **no `animation` property** and no GSAP timeline targets
  it, so each card is statically visible from its clip's first displayed frame.
  Only the bottom `.cap` runs `cap-pop` (`.cap { animation: cap-pop 0.26s … }`).
  opencat's loop popped *both* (`c` and `m`) with the cap-pop keyframes, so the
  card was absent at the start of every one of the 11 clips. Measured at clip 1
  (local f0–8): reference card is fully rendered at f0 (white border-left/text
  max 255) while opencat's was still invisible (max 7); card-region (x50–440,
  y40–210) mae 9.46 → 3.08, p8 0.053 → 0.028 at f0, and now constant across
  f0–f8 instead of ramping. Replaced the card's two pop tweens with
  `T.set(c, { opacity: 1 }, at)`.

### Confirmed NOT the problem / rejected (negative results, don't re-chase)

- **act1 scatter timing is already optimal.** A ±1-frame (0.0333 s) shift of the
  phase-5 scatter start (8.15) is **net-worse** on the f244–249 window: sum p8
  base 1.710, −1f 2.181, +1f 1.731. The baseline already matches the reference
  best at f244/f249. Confirms R5 §14 (the transient is a capture-side
  scale-then-translate in-between, not an opencat timing error) — do not shift.
- **act1 caption pill geometry is exact.** The pill is `bottom-[95px]` (R2 §11);
  a re-bracket at f75/105/150/180/200 shows 95 is the unique minimum for the
  *text* rows (ref text top 921/920 vs op 922/920 at 89/92/95 → 95 matches),
  and the text x-extent/centre match to ±0 px. The caption-band p8 (~0.10–0.34
  in-band) is entirely glyph AA / the pill-fill decode offset, not a shift.
- **act3 `.a3-cap` backdrop-filter: blur(10px) is net-negative** (matches R7).
  Porting it to `backdrop-blur-md` (12.0) over-darkens and drops the card's own
  shadow: card mae 3.2 → 14.7 (f30), 3.5 → 20.1 (f100), p8 0.03 → 0.34.
- **act3 `.a3-cap` text-shadow `0 1px 2px rgba(0,0,0,0.35)` is a no-op** on the
  dark cards (the probe parsed the class but the card region is byte-identical:
  mae 3.24 both) — the card fill + border-left dominate. Not landed.
- **act1 `.grain` layer.** The reference has `.grain` (radial-gradient dot
  pattern, opacity 0.5) as well as `.canvas`; the XML ports only `.canvas`. The
  grain is a 1px dot pattern at 0.015 alpha and the caption/panel bands already
  match structurally (glyph extents within 1–2px), so the residual p8 is the
  ~−3 bg offset + glyph AA, not a missing layer. Left alone (no measured gain).
- **act4 wordmark sub-pixel is optimal.** Hyper glyph rows match to ±1px at
  f90/120/192/225 (rowshift 0), cols within 1px. The residual band p8 (0.04–0.10)
  is gradient-glyph-edge decode, as recorded in R7/R8.
- **act4 f0 is the window boundary** (ref act4 f0 still decodes the act-3
  GitHub tail: ref act4 f0 vs ref act3 f653 mae 0.76, i.e. the reference has no
  cut there; opencat's act3→act4 concat does). Unavoidable given per-act XMLs.
- **act3 worst frames (f391 0.68, f371 0.49) remain the clip-1 source→capture
  boundary** (R8/R9 negative result); not markup.

### Engine
No `crates/` changes this round (no rebuild needed beyond the up-to-date
`--bin opencat`). All changes are XML-only; `cargo test -p opencat-core`
unaffected.

### Next round
1. Whole-film is at mae **2.123 / avg_p8 0.019494** — both below R9 and avg_p8
   well under the 0.02 gate. `worst_p8` (0.680) cannot be met: pinned by the
   act3 clip-1 source-boundary lag (reference-capture property, R9 §18).
2. Remaining stable blocks are all floors: act1 caption/panel diffs (~0.03),
   act3 card glyph AA + codec, act4 wordmark gradient glyph edges (~0.018).
   No cheap markup port remains — the act3 card fix was the last un-ported
   structural detail found.
3. Do not re-chase: the scatter timing (optimal), the caption pill position
   (optimal), the `.a3-cap` backdrop-blur/text-shadow (net-negative/no-op), the
   act4 wordmark geometry (optimal), or the act3 source boundaries.

## 20. Round 11 results

Method unchanged (§7, offset 0; whole-film = concat of the 3 act renders,
compared against the full reference).

| scope | mae | avg_p8 | worst_p8 | p8>0.08 frames |
|---|---|---|---|---|
| R10 whole film | 2.12335 | 0.019494 | 0.680 | 14/1254 |
| **R11 whole film** | **2.11821** | **0.019418** | 0.680 | 14/1254 |
| R10 act1 | 3.8298 | 0.029789 | 0.533 | 8/360 |
| **R11 act1** | **3.8121** | **0.029524** | 0.533 | 8/360 |
| R10 act3 | 1.0179 | 0.014528 | 0.680 | 5/654 |
| R11 act3 | 1.0179 | 0.014528 | 0.680 | 5/654 |
| R10 act4 | 2.5363 | 0.017580 | 0.446 | 1/240 |
| R11 act4 | 2.5363 | 0.017580 | 0.446 | 1/240 |

This session's baseline re-render (clean HEAD) reproduced R10 exactly: whole
2.12335 / 0.019494. (act3 and act4 are untouched this round.)

### What changed (files) — one XML-only exact-reference port

- **`website-to-hyperframes-act1.xml` — the Cursor panel's input card was
  ~33px too short, lifting its footer row (Agent / Opus 4.7 pills) by ~33px
  (act1 mae 3.8298 → 3.8121, avg_p8 0.029789 → 0.029524; whole-film mae
  2.12335 → 2.11821, avg_p8 0.019494 → 0.019418).** The reference
  `.cu-input-card { border:1.5px; padding:18px 18px 14px; min-height:180px }`
  uses the default **content-box** box model, so its rendered height is
  180 + 36 (pad) + 3 (border) = 219px; the border box spans y226→436 (measured
  on the 1px card border column x1250: peaks y226 + y436). opencat's port used
  `h-[180px]`, and its Tailwind box model is **border-box** — the whole card was
  only 180px (measured border box y228→403), so `.cu-footer-row`'s
  `bottom:14px` landed 33px high: `New Chat` header column-scan put opencat's
  footer text at y432 vs reference y464, and `Agent`/`Opus 4.7` pills measured
  ref rows y464–476 / opencat y432–441. Changed `h-[180px]` → `h-[214px]`
  (180+36+3=219, minus opencat's own 4px; 214 reproduces the ref bottom border
  at y436 and the footer text at y462–476). Footer-band region (y405–490,
  x300–800) p8 0.096 → 0.052, mae 8.78 → 7.13 at f120.

### Confirmed NOT the problem / rejected (negative results, don't re-chase)

- **act1 master bottom captions lack the reference's `text-shadow`.** The ref
  `.cap` has `text-shadow: 0 2px 14px rgba(0,0,0,0.55), 0 0 2px rgba(0,0,0,0.35)`
  (§2 index.html) and the XML `.mt1..mt4` have none. Porting it verbatim via
  `[text-shadow:0_2px_14px_rgba(0,0,0,0.55),0_0_2px_rgba(0,0,0,0.35)]` over-
  darkens: caption band (y895–1015, x700–1200) p8 0.554 → 0.589, mae 15.66 →
  17.43 at f120 (opencat applies true gaussian blur; ref's is already thick,
  and opencat's glyphs are dimmer, so the extra dark halo doubles the error).
  Reverted. The caption band residual is glyph-AA + the ~−3 bg decode offset —
  same floor class as R10, not a missing shadow.
- **The act1 caption text is already at the right x** (opencat x779–1143 vs ref
  x778–1143 at f120, ±1px). The band p8 (~0.42 in-band) is the pill-fill +
  glyph raster, verified identical across f80/120/180. No shift.
- **act3/act4 residuals are decode-floor, not shifts.** Integer cross-
  correlation of every act3/act4 frame tried returns best shift exactly (0,0):
  act3 f180/300/400/500 mae 0.93/0.69/0.41/0.41 unmoved; act4 f100/150/200/220
  mae 2.51/2.45/2.65/2.63 unmoved. The residual is glyph AA + the ~−3 bg
  offset + codec. The 11 act3 clips are byte-identical (`md5sum`) to the
  reference `compositions/act-3-clips/*`, so act3's worst frames (f31 0.68,
  f11 0.49 — clip-1 source→capture boundary) are reference-side, matching R8–R10.
- **act1 scatter transient f245–248 remains a reference-capture in-between.**
  Re-measured the corner-panel occupancy f244–250: opencat holds the pre-
  scatter layout through the 0.20s power3.out tween (TL 0.37→0.70 monotonic)
  while the reference **converges** (TL 0.38→0.22→0.0) into f246–248 then
  explodes to the scattered layout at f249. At f249–250 both are pixel-identical
  (dark bbox both x[117,1802] y[67,1012]). The reference is a scale-then-
  translate capture that skips the outward tween; not reproducible by any XML
  timing (R5/R10 confirmed). Pinned.
- **gemini panel rows sit ~12px low (f240: prompt ref y588–594 vs op y600–606;
  column x1500 rules ref y277/524 vs op y306).** Investigated but no clean
  unique port found this round (the ge-input-line `padding:80px 0` +
  `.ge-top-hint padding:18px 0 14px` content-box chain is a candidate next
  round, but a blind height bump had no isolated minimum in the 2 probes spent,
  so left untouched rather than risk a net-negative).

### Engine
No `crates/` changes this round (no rebuild needed beyond the up-to-date
`--bin opencat`). All changes are XML-only; `cargo test -p opencat-core` green
(only ignored doctests).

### Next round
1. Whole-film is at mae **2.118 / avg_p8 0.019418** — both below R10 and avg_p8
   under the 0.02 gate. `worst_p8` (0.680) still cannot be met: pinned by the
   act3 clip-1 source-boundary lag (reference-capture property).
2. Best remaining candidate: the **gemini panel's ~12px-low content** (the
   `.ge-top-hint` / `.ge-input-line` content-box chain, §above) — this is the
   only structural mismatch left in act1's panel phase after the cursor-card fix.
   Also check the **codex `.co-input` / `.cc-input`** cards for the same
   content-box-vs-`h-[]` issue if a height is ever set (currently they use
   padding only, so they look correct).
3. Do not re-chase: the caption text-shadow (net-negative), the caption pill
   position/geometry (optimal), the scatter transient (reference-side), the
   act3/act4 integer shifts (all zero), or the act3 source boundaries.
