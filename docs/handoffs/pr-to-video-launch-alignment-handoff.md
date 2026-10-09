# pr-to-video-launch — Alignment Handoff (per-launch)

Status: **round 1 (alignment) — 2026-10-06** — first alignment edits landed.
Whole-film `avg_p8 0.0506 → 0.0386`, `mae 4.35 → 3.68` (900 f, silent).
Engine fix + XML fixes, all uncommitted (this round does **not** commit; a
separate review agent gates the commit). Branch
`feat/hyperframes-launches-alignment`.

## Round-1 (2026-10-06) — changes, evidence, negatives

Measured baseline this round (the §3.5 intake number 0.0950 was stale; the
current XML measured **avg_p8 0.0506 / mae 4.35**). After the fixes below:
**avg_p8 0.0386 / mae 3.68 / worst_p8 0.1184** (`out/pr-to-video-launch-r1f-diff/`).

| # | change | file | reason / measured effect |
|---|---|---|---|
| A | tween `transformOrigin` now applies for the whole tween, not only the final frame | `crates/opencat-core/src/script/runtime/animation/properties/style_props.js` (`makeOriginProperty.interpolate` → `return to`) | **Engine bug.** GSAP snaps the pivot for the whole tween; the engine held the pre-tween `from` until `progress===1`. The opener zoom-through (`.to('#s-opener',{scale:1.6,…,transformOrigin:'66% 59%'})`) scaled about the box centre for frames 150–153 — the top-2 worst frames. Edge margins now: f150 ref (79,125) vs oc (78,125); f151 (32,105) vs (31,106). Unit test `tween_transform_origin_applies_for_whole_tween_not_just_last_frame` (opencat-engine). |
| B | composer `min-h` corrected for content-box vs border-box | `examples/pr-to-video-launch.xml` (4 bars) | The port's `min-h-[63.36px]`/`min-h-[76.8px]` are border-box in the engine (taffy default) but the reference CSS `min-height:3.3cqw`/`4cqw` are **content-box** (padding adds). Set to the border-box totals `92.24px` (opener/summarize/use-skill) and `115.28px` (CTA). CTA bar: ref y709–822 vs oc was y746–822 (76.8 vs 113 px tall) → now y708–822. |
| C | GitHub mark centred | XML `#cta-gh` | Reference `.gh{top:22cqh; xPercent/yPercent:-50}` centres the mark; the port's `top-[237.6px]` placed its **top** edge there, 42 px low. → `top-[195.36px]`. Verified f850 ref (918,197,1000,278) vs oc (918,196,1001,278). |
| D | Hanken weight faces actually selected | XML: `font-[hk5] font-medium`, `font-[hk6] font-semibold`, `font-[hk7] font-bold` (52 sites) | `font-[face-id]` resolves the **family only**, so every Hanken weight rendered at 400 (weight matching falls back to Regular). CTA headline ink 13 367 → 18 688 (ref 18 868); scene titles already matched. Note: the clean engine fix (carry the face's `usWeightClass`) was **not** viable this round — see negative N3. |
| E | pr-scroll stat carets made visible | XML pr-scroll timeline | The source `typeNum` does `tl.set(car,{opacity:1},t)` at the type start; the port set opacity 0 at the *end* but never 1, so `629 |`,`87,291 |`,`4 |` carets never showed. Added the `set(opacity:1)` and moved the blink to `t+dur`. |
| F | CTA headline x | XML `T.set('cta-headrow',{x:364.5})` | Headline was 6 px right; now bbox ref (368,367,1481,454) vs oc (368,370,1482,455), region mae 35.7 → 11.2. |

**Negatives (do not retry as-is):**
- **N1 — pr-scroll file-row cloning.** The source clones the 17 file rows ×8 so
  the `−3600px` whip never runs dry. Injecting 136 clone rows into the XML as
  siblings broke the div tree and rendered **worse** (f241 mae 5.6 → 7.5) and
  still showed a blank body; reverted. If revisited, needs an engine-level
  clone/`map` primitive or a `background-repeat`-style row list, not sibling dup.
- **N2 — summarize scroll.** Settle position matches (dy≈2 at f340–350) but the
  mid-scroll is off by 26–172 px (f308–f325, the largest remaining cluster).
  The reference measures `dist=inner.offsetHeight-feed.offsetHeight+30`; the
  port hardcodes `-2400`. Next round: measure the port's `sm-inner` content
  height and set `dist` to match (scene-local measurement — opencat has no
  `getBoundingClientRect`, cf. G6).
- **N3 — weight via `font-[id]` in core.** Carrying each face's weight into
  `FontFamilyIndex` works only after fonts load, but `build_parsed_document`
  (the engine open path) runs with `font_index=None` and only has
  `build_family_index()` (no bytes). The XML-layer weight classes (D) are the
  faithful, working fix; a proper core fix needs the parse path to see font
  bytes.
- **N4 — opener zoom f149.** Before the op-zoom tween starts (4.8s) the port's
  window edge is 126 vs ref 113 (13 px; ref window left is ~112 at 1599.36
  wide). Left as-is this round.

## Intake notes (round 2, 2026-10-05) — superseded by the alignment round above

> **CORRECTION vs round-1 handoff.** Round 1 said
> `examples/pr-to-video-launch.xml` "does not exist". That is **stale**: the
> XML now **exists and is fully authored** (631 lines, all 10 scenes + root
> xfades + script timeline). This round measured it: render is instant
> (~14 s), 900 frames, correct 1920×1080@30, silent. **Baseline whole-film
> `avg_p8 = 0.0950`, `mae = 8.95`** — §3.5. Gate is `avg_p8 ≤ 0.02`. The
> port is a strong first pass but not yet gate-passing; the residual is
> dominated by (a) a broken black-title font and (b) per-scene/seam timing +
> mono tracking. Concrete defects and fixes are in §3.5.

> **Recon verdict.** Unlike the 3D/GL titles in this launch family
> (`liquid-brand-refraction`, `vfx-heygen-combined`), this reference is a
> **valid, fully-composited 2D film** — a dark "Claude composer" UI + GitHub
> PR-browser + typographic feature cards, all canvas2D/DOM, no WebGL, no GLB.
> All 900 frames are distinct and carry real content. **No re-render is
> needed.** This content class is the *favourable* end of the family: a 2D
> port can plausibly approach the generic gate (`avg_p8 ≤ 0.02`), with the
> floor set by font rasterisation / text metrics rather than by missing 3D.

Reference project: `/home/solaren/Projects/hyperframes-launches/pr-to-video-launch/`
Reference video: `renders/pr-to-video-launch_2026-10-02_09-19-38.mp4`

---

## 1. Reference spec (ffprobe, measured)

| Field | Value |
|---|---|
| Resolution | 1920 × 1080 |
| Frame rate | 30/1 fps (CFR) |
| Frames | 900 (`nb_frames`; `duration=30.000`, container 30.016 s) |
| Duration | 30.000 s |
| Audio | **yes** — AAC (LC), 48 kHz, stereo, ~30 s |
| Codec | h264 High, yuv420p |
| Composition id | `pr-to-video-launch` (root, `data-duration="30.0"`) |
| Meta | `meta.json`: name `pr-to-video-launch`, duration 30.0, 1920×1080, fps 30 |

Frames ↔ seconds: `frame = round(sec * 30)` (0-based). 900 frames = 30.0 s.

### 1.1 Reference validity — confirmed good

No degeneracy. Distinct frame check (every 15th frame) yields unique hashes
across the whole timeline; sampled frames 0/100/200/300/500/700/880 all differ.
Spot-checked exports (f30 composer typing, f330 summarize scroll, f700 feature
player) all show correct content. The reference is the ordinary
`npx hyperframes render` output — the GPU/WebGL failure that broke the two 3D
titles does **not** apply here (no GL surfaces in this index).

### 1.2 The film in one paragraph

A 30 s product-launch film for the "pr-to-video" skill. **Act 1 (0–16.1 s):
opening** — a Claude composer types an ask with a PR link and morphs
full-bleed → floating window; the PR page whips through its diff with data
callouts; two serif intertitles ("Hard to read", "Still too long"); a
`summarize this PR` composer beat; then `use pr-to-video to make a video for
this pr`. **Act 2 (16.1–25.6 s): features** — four cards, each a Claude-styled
video player showing "scene N / 6" of the output: `SHOW YOUR CODE` (code
editor + diff highlight), `SHOW YOUR FEATURE` (PR diff → storyboard → video
flow), `SHOW YOUR REVIEW` (review threads resolving to merge), `SHOW YOUR
CONTRIBUTOR` (credits roster of humans + Claude/Codex). **CTA
(25.6–30.0 s)** — GitHub mark + "Create your PR video today" headline + a
composer that types `use pr-to-video <url>`. Root BGM (`launch-music.mp3`,
30 s) plays throughout.

The root comment notes a "HyperFrames logo sign-off appends after, ffmpeg" —
that post-append is **not** in the 900-frame measured render; align against the
900-frame file as-is.

---

## 2. Reference structure map

`index.html` is a **single root composition** (`#pr-to-video-launch`) holding
ten absolutely-positioned, z-indexed scene divs, each a
`data-composition-src` into `compositions/*.html`. The root GSAP timeline only
cross-fades scene opacities (`xfade`); every scene's motion lives in
`window.__timelines["<id>"]` inside its own file.

### 2.1 Root scene windows (authoritative)

| # | scene id | composition src | start s | dur s | end s | **frames** | z |
|---|---|---|---|---|---|---|---|
| 1 | `#s-opener` | `compositions/opener.html` | 0.0 | 5.2 | 5.2 | **0–155** | 1 |
| 2 | `#s-prscroll` | `compositions/pr-scroll.html` | 5.2 | 3.2 | 8.4 | **156–251** | 2 |
| 3 | `#s-hard` | `compositions/hard-read.html` | 8.4 | 0.9 | 9.3 | **252–278** | 3 |
| 4 | `#s-summarize` | `compositions/summarize.html` | 9.3 | 3.0 | 12.3 | **279–368** | 4 |
| 5 | `#s-useskill` | `compositions/use-skill.html` | 12.3 | 3.8 | 16.1 | **369–482** | 5 |
| 6 | `#s-fcode` | `compositions/feature-code.html` | 16.1 | 2.4 | 18.5 | **483–554** | 6 |
| 7 | `#s-fflow` | `compositions/feature-flow.html` | 18.5 | 2.5 | 21.0 | **555–629** | 7 |
| 8 | `#s-fdisc` | `compositions/feature-discussion.html` | 21.0 | 2.0 | 23.0 | **630–689** | 8 |
| 9 | `#s-fcredits` | `compositions/feature-credits.html` | 23.0 | 2.6 | 25.6 | **690–767** | 9 |
| 10 | `#s-cta` | `compositions/cta.html` (`cta-outro`) | 25.6 | 4.4 | 30.0 | **768–899** | 10 |

### 2.2 Root timeline — the cross-fades (exact)

`const T = gsap.timeline({paused:true})`. All opacities set 0 at t=0 for
`#s-prscroll.. #s-cta`. Only `#s-opener` starts visible.

```js
// ①→② zoom-through into the PR link (opener is NOT a plain cross-fade)
T.to('#s-opener', {scale:1.6, opacity:0, filter:'blur(8px)', duration:0.4, ease:'power2.in', transformOrigin:'66% 59%'}, 4.8);
T.set('#s-opener',  {opacity:0}, 5.22);
T.set('#s-prscroll',{opacity:1}, 5.2);
T.fromTo('#s-prscroll', {opacity:0}, {opacity:1, duration:0.3, ease:'power2.out'}, 5.15);

// plain cross-fades: (oldSel,newSel,t,d) => old→0 dur d; new 0→1 dur d at t
xfade('#s-prscroll','#s-hard',      8.4,  0.28);
xfade('#s-hard',    '#s-summarize', 9.3,  0.28);
xfade('#s-summarize','#s-useskill', 12.3, 0.28);
xfade('#s-useskill','#s-fcode',     16.1, 0.35);   // opening → Act 2
xfade('#s-fcode',   '#s-fflow',     18.5, 0.3);
xfade('#s-fflow',   '#s-fdisc',     21.0, 0.3);
xfade('#s-fdisc',   '#s-fcredits',  23.0, 0.3);
xfade('#s-fcredits','#s-cta',       25.6, 0.35);   // Act 2 → CTA
```
each `xfade` = `T.to(old,{opacity:0,duration:d,ease:'power1.inOut'},t)` +
`T.fromTo(new,{opacity:0},{opacity:1,duration:d,ease:'power1.inOut'},t)`.

### 2.3 Scene timelines (element / property / duration / position / ease)

**opener** (`opener`, `tl`, 5.2 s). Window full-bleed → shrinks to floating
(1599.36×874.8, r=25.92) between t=2.55–3.10. Composer starts centred
(`left:540,w:840,bottom:370,scale:1.85`) and "morphs" home via a manual `p`
tween (`power2.inOut` 0.55 s at 2.55).
- `inputEl` fromTo x 280→0 op 0→1, **0.55 s power3.out @0.02**; `chipsEl` x
  340→0 **0.55 @0.10**; `metaEl` x 340→0 **0.55 @0.16**.
- `.orb` rotation 360, **1.0 s none, repeat:4 @0.0**.
- caret blink: op 1→0 **0.4 s steps(1), repeat:1 yoyo @0.1**; solid @0.6.
- **typewriter** MSG (`"Someone on our team just opened a PR for a new
  pr-to-video skill — can you review it?"`, 86 chars): obj.n 0→len,
  **1.5 s none @0.6** (slice-based reveal). At **2.1** innerHTML swaps to
  `MSG + " " + URL` where URL=`github.com/heygen-com/hyperframes/pull/1635`
  coloured `#9CC4E6`.
- cursor rises `ret`: fromTo y retY+300→retY op 0→1, **0.7 s power3.out
  @1.3**; tap scale 1→0.84 **0.1 power2.in @2.3**, →1 **0.18 power2.out
  @2.4**; `ret` colour `#6F6E66`→`#D97757` **0.08 power1.in @2.35**, back
  **0.25 @2.5**.
- SEND clears composer @2.5. Morph `p` **0.55 power2.inOut @2.55** drives
  `composer.width 1554→…`, `win.width 1920→1599.36` (`win` tween **0.55
  power2.inOut @2.55**), `.bar` fromTo op 0→1 x −14→0 **0.3 power2.out @2.85**,
  `.chips` op→0 height→0 **0.3 power2.in @2.6**.
- M1 (the ask+link) rises: `m1` op 0→1 **0.3 power2.out @3.2**; `stack` y
  dM1→0 **0.45 power3.out @3.2**.
- cursor travels to PR link **0.6 power3.inOut @3.5**; click scale **0.1
  power2.in @4.2 / 0.18 power2.out @4.3**; `prlink` colour `#D97757`→`#F2A98C`
  **0.1 @4.22** → back **0.3 @4.42**; ripple scale 0→1.1 op→.5 **0.2
  power2.out @4.22**, scale→1.9 op→0 **0.32 @4.42**.
- hold `0.2` @5.0. **All cursor/link x,y are measured live** via
  `getBoundingClientRect` at two window sizes (see §5 gap G6).

**pr-scroll** (`pr-scroll`, 3.2 s). Browser pinned left (53cqw × 84cqh);
right panel headline + 3 stats.
- `br` scale 1.04→1 op 0→1 **0.4 power3.out @0.0**.
- tabs: `.on` toggles @0.25 (Conversation), @0.95 (Commits), @1.15 (Files).
- review rows: 6 × fromTo op 0→1 x 16→0 **0.18 power2.out, stagger 0.12,
  @0.3**.
- @1.1 review op→0 (0.2); @1.15 files op→1 (0.2); `#prs-inner` y 0→−3600
  **1.7 power2.in @1.2**; blur 0→2px **1.1 power2.in @1.5**. (file rows
  cloned ×8 for the whip.)
- right: `head` reveal **0.3 power3.out @0.5**; `s1` @1.2 then typeNum
  `629` **0.45 steps(3) @1.35**; `s2` @1.75 typeNum `87,291` **0.7 steps(6)
  @1.9**; `s3` @2.35 typeNum `4` **0.2 steps(1) @2.5**; carets blink then
  hide. `reveal` = fromTo op 0→1 y 14→0 **0.3 power3.out**.

**hard-read** (`hard-read`, 0.9 s). `#hr-txt` = "Hard to read", Galaxie
Copernicus, 9.5cqw, `#EFEADD` on `#1D1F1F`.
- fromTo op 0→1 **0.28 power3.out @0**; y 14→0 **0.28 power3.out @0**; hold.

**summarize** (`summarize`, 3.0 s). Claude window, feed of markdown lines.
- `.orb` rotation 360 **1.0 none repeat:2 @0**.
- type CMD `"summarize this PR"` **0.8 none @0**; caret off @0.82.
- `composer` y 0→−44 op→0 **0.32 power2.in @0.85**; `feed` op→1 **0.25
  power1.out @0.92**; `inner` y 0→−dist **1.25 power1.in @1.0**
  (`dist = max(0, inner.h − feed.h + 30)` — measured).
- `veil` op→1 **0.3 power2.in @2.2**; `still` ("Still too long") op 0→1 y
  14→0 **0.32 power3.out @2.32**; hold.

**use-skill** (`use-skill`, 3.8 s). Same composer, command rises into feed.
- `.orb` rotation 360 **1.0 none repeat:3 @0**; caret blink **0.5 steps(1)
  repeat:1 yoyo @0.1**.
- type CMD `"use pr-to-video to make a video for this pr"` (42 chars) **1.4
  none @0.4**; send @2.0.
- `cmd` op 0→1 **0.3 power2.out @2.05**; `stack` y dOut→… **0.4 power3.out
  @2.05**; `out` op→1 **0.2 @2.45**; `stack` y→0 **0.4 power3.out @2.5**;
  `think` ("thinking…" + spinning ✳) op 0→1 y 8→0 **0.3 power2.out @2.5**,
  `ast` rotation 360 **1.0 none repeat:1 @2.5**; `l0` ("Reading PR #1635 ·
  629 files · 4 commits") op 0→1 y 10→0 **0.35 power3.out @3.0**; hold.

**feature-code** (`feature-code`, 2.4 s). Inner authored ~4.8 s then
`inner.timeScale(2.0)`. Title typewriter `"SHOW YOUR CODE"` **1.15 none
@0.3**; caret blink steps(1) repeat:9. `.pwin` fromTo op 0→1 scale .62→1 y
10→0 **0.6 power3.out @0.15**; `.codeview` op→1 **0.45 power2.out @1.0**;
`pfill` scaleX .05→.11 + `pknob` left 5%→11% + timecode 0:02→0:07 **3.2
none @1.4**; `#fc-band` top 622px, key lines: non-key lines op→.32 **0.4
power2.out @2.45**, band op→1 **0.4 power2.out @2.5**; hold .6.

**feature-flow** (`feature-flow`, 2.5 s). `inner.timeScale(2.56)` (authored
~6.4 s). Title `"SHOW YOUR FEATURE"` **1.15 none @0.3**. `.pwin` **0.6
power3.out @0.15**; `.cl1` caption op→1 **0.4 @1.7**; scrubber fill .22→.42
+ knob 22%→42% + time 0:07→0:13 **3.6 none @1.9**. Flow: `#u-diff` fromTo op
0→1 x −30→0 scale .95→1 **0.5 power3.out @2.0**; `.a1` **0.35 power2.out
@2.55**; `.fc`×5 op 0→1 y 20→0 scale .85→1 **0.4 stagger .1 back.out(1.6)
@2.7**; `.striplbl` **0.4 power2.out @3.0**; `.a2` **0.35 @3.45**; `#u-video`
fromTo op 0→1 scale .8→1 x 20→0 **0.55 back.out(1.6) @3.6**; `.vring` ripple
op .5→0 scale 1→1.8 **1.0 power2.out repeat:1 @4.1**; hold .3.

**feature-discussion** (`feature-discussion`, 2.0 s). Outer `inner.timeScale
(2.05)` (~4.1 s), conversation sub-timeline `disc.timeScale(2)` (authored 1×).
Title `"SHOW YOUR REVIEW"` **0.9 none @0.2**. `.pwin` **0.5 power3.out @0.1`;
`.cl1` **0.35 @1.2**; fill .66→.80 + knob 66%→80% + time 0:24→0:27 **3.2 none
@0.6**. conversation: `.card` op→1 **0.5 power2.out @0.5**; `.chead` **0.4
@0.9**; rows r1–r4 fromTo op 0→1 scale .96→1 y 14→0 **0.42 back.out(1.5),
stagger 0.4, @1.4**; r5 **0.5 back.out(1.6) @3.4**; resolution pill swaps
(p4→p1): open op→0 **0.2 power2.in**, done fromTo op 0→1 scale .8→1 **0.32
back.out(1.7)**, pairs at @4.2/.28 step; `#fd-merge` op 0→1 scale .85→1 y
10→0 **0.55 back.out(1.6) @5.9**.

**feature-credits** (`feature-credits`, 2.6 s). `inner.timeScale(2.54)`.
Title `"SHOW YOUR CONTRIBUTOR"` **1.0 none @0.3**. `.pwin` **0.6 power3.out
@0.15**; `.cl1` **0.4 @1.7**; fill .86→.98 + knob 86%→98% + time 0:30→0:32
**3.4 none @1.9**. roster: `.glab` op 0→1 y 8→0 **0.4 stagger .12 @1.95**;
`#cr-path` scaleX 0→1 **1.2 power2.inOut @2.0**; per avatar (×6) at
`t=2.05+i*0.16`: av scale .55→1 op→1 **0.45 back.out(1.7)**; ring op .6→0
scale .5→1.35 **0.7 power2.out**; `nm` **0.35 power3.out** (t+.14); `rl`
**0.35** (t+.24); `#cr-sign` **0.5 power3.out @3.9**; hold .3.

**cta-outro** (`cta`, 4.4 s).
- `.orb` rotation 360 **1.0 none repeat:4 @0**; `gh` (GitHub mark) op 0→1
  scale .96→1 **0.6 power2.out @0.0**.
- headline type `"Create your " + "PR video"(#D97757) + " today"` (24 chars)
  **0.95 none @0.3**; `hcaret` blink repeat:1.
- `top` scale 1.18→1 y 206→0 **0.55 power3.inOut @1.5**; caret off **0.2
  power1.out @1.5**; `composer` op 0→1 y 44→0 **0.5 power3.out @1.75**.
- CMD `"use pr-to-video "` **0.55 none @2.2**; URL `https://github.com/
  your-org/your-repo/pull/42` appears @2.85 with bg highlight
  `rgba(217,119,87,.36)`→0 **0.5 power2.out** and scale 1.07→1 **0.3
  power3.out**; caret blink repeat:5 @3.15; `ret` colour→`#D97757` scale→1.18
  **0.16 back.out(2) @3.25**, →1 **0.28 power2.out @3.42**; hold .5.

### 2.4 Assets, fonts, media (all present in the project)

- **video**: `assets/pet-typing.mp4` (opener `.pet` — two sequential trimmed
  instances, mute, `-webkit-mask` feathered; not referenced elsewhere).
  `assets/hyperframes-logo-motion.mp4`/`.webp` exist but are **not referenced**
  by the measured render (used only by the post-hoc ffmpeg sign-off append).
- **images**: `assets/avatars/{waterrr,jrusso1020,miguel,miga,ghas,claude,
  codex}.png`; `assets/{avatar-james,avatar-miguel,avatar-water}.png` (pr-scroll
  avatars).
- **audio**: root BGM `assets/launch-music.mp3` (30 s). SFX (14):
  `whoosh-slide,type,pop,click,whoosh-morph,whoosh,swish,riser,riser-build,
  chime-soft` (+`chime,success,type-short,ux-chime` unused here) under
  `assets/sfx/`.
- **fonts** (woff2, must be converted — see §5 G11): **Hanken Grotesk**
  400/500/600/700, **Galaxie Copernicus** Book (serif intertitles + PR
  headline), **Spline Sans Mono** 400/500, **Archivo Black** (aliased
  `CodeBlackHF` for the `SHOW YOUR …` card titles).
- **inline SVG** (no asset): GitHub mark (cta), lock + chips icons (opener,
  pr-scroll addr), cursor arrow (opener), play triangles, four flow arrows
  (feature-flow). opencat has `<path>`/`<icon>` but **no `<svg>` container** —
  see §5 G1.

### 2.5 Layout unit note

Every scene is authored in **container-query units** (`container-type:size`;
`1cqw = 19.2px`, `1cqh = 10.8px` at 1920×1080). opencat has no cqw/cqh (see
§5 G3) — convert every cqw/cqh to px (`×19.2` / `×10.8`) during the port.

---

## 3. opencat target — current state (measured this round)

| Target | clock | frames | state |
|---|---|---|---|
| `examples/pr-to-video-launch.xml` | 30.0 s | 900 | **EXISTS, fully authored** (untracked) |

One file, 631 lines: a root `div` (`1920×1080`, bg `#F0EFE9`) holding the ten
scenes as `z-[1..10]` absolutely-positioned sibling divs, the §2.2 cross-fade
timeline (`xfade` helper) at root level, and a single root `<script>` with
`var T = ctx.timeline()` driving everything (scene-local motion inlined into
`T` at `O2..O10` offsets — NOT separate `ctx.timeline()`s). Fonts loaded from
`assets/p2v/*.ttf` (8 faces, §5 G11 done). Assets under `assets/p2v/`
(avatars, gh-mark.png, font ttfs). Single-file shape matches the guidance.

### 3.5 Baseline measurement (this round) — the actionable part

Render + whole-film k3diff (commands in §7):
```
avg_p8 = 0.0950   mae = 8.954    (900 f, silent, 34.8 s user)
```

**Worst regions are not random** — they are exactly the seams and two
font/layout blocks:

| region | frames | symptom (ref vs port) | fix |
|---|---|---|---|
| hard→summarize seam | 279–285 | ref fades `s-summarize` in (light) over the dark `hard-read`; port stays fully **dark** at f279 (mean 237 vs 33), fade lags ~2 f | Move `xfade('s-hard','s-summarize', 9.3, .28)` earlier — the incoming tween is ~0.28 s too late. Likely `xfade` writes the `opacity:1` fromTo at `t` but the outgoing `s-hard` covers it; check `s-summarize` initial opacity is really 0 until the fade and the fade actually starts at 9.3 s (ref: hard-read itself is barely visible, covered by the 278→279 hard cut). |
| use-skill→fcode seam | 369–374 | inverse: port shows light `s-summarize`/dark mismatch over the whole `SHOW YOUR CODE` region | Same class — incoming/outgoing opacity curve offset. Ref cross-fades **both** `s-useskill→0` and `s-fcode→1` over the same window; verify both tweens present at `O6=16.1, d=.35`. |
| feature-card titles (all 4) | 483+, 555+, 630+, 690+ | port renders `SHOW YOUR CODE/FEATURE/REVIEW/CONTRIBUTOR` in a **thin light sans**; ref is heavy **Archivo Black** | **`font-[black]` does not resolve to ArchivoBlack.ttf.** The face's real family name is `Archivo Black` (verified via fonttools), but the `<font id="black" family="CodeBlackHF" …>` declares family `CodeBlackHF`, which is not a family that exists in the TTF/fontdb → fallback to default sans. Fix: set `family="Archivo Black"` (or make the id match the resolved family). This is the single largest per-frame error in Act 2. |
| mono blocks (code, composer, pr-scroll numbers) | throughout | port mono text is **tracked wide** with extra inter-glyph gaps; ref is tight | `letter-spacing` not applied / wrong, or monospace advance differs. Ref uses `font-size:33px` with default tracking; port `text-[33px] font-[mono4]` appears to have no `tracking` class. Add the ref's tracking (0 here) or reduce tracking; also verify mono5/mono6 weights (§5 G13). |
| opener→prscroll | 156 | scale morph/zoom-through at 4.8–5.2 | verify `filter:'blur(8px)'` tween + `transformOrigin:'66% 59%'` (§5 G5). |

Per-frame top-12 by `p8` are frames 369–374 and 279–284 — i.e. the two seams
above, followed by the feature seams. **Fix the two seams + the `black` font
family and most of the p8 mass collapses.**

**Do not split into ten XMLs.** The source is one root with ten z-stacked,
opacity-cross-faded scenes — the shape already used in the port (mirrors
`examples/k3-promo.xml` single-file style). Per-scene frame windows (§2.1)
are for **segmented k3diff only**, not for splitting the XML.

---

## 4. Reference playback model (why a single XML is faithful)

- No nested-composition runtime in opencat: flatten the ten
  `data-composition-src` scenes into sibling nodes; reproduce the root `xfade`
  opacities 1:1 (§2.2). Scene z-order is the DOM order (1→10).
- Two scenes use an **inner authored timeline** played at a compressed
  `timeScale` (feature-code 2.0, feature-flow 2.56, feature-discussion 2.05,
  feature-credits 2.54). Mirror this by dividing every inner time by the
  `timeScale` in the opencat timeline (or by using local `data-start` offsets),
  exactly as the vfx-heygen ports flattened nested timelines.
- `#s-opener` is **not** a plain cross-fade: it scales to 1.6, blurs to 8px,
  and fades over 0.4 s (`power2.in`, origin `66% 59%`) while `#s-prscroll`
  fades in under it — port that beat literally.

---

## 5. Engine gap list (opencat vs this reference)

Ordered most-blocking-first. Unlike the 3D titles there is **no 3D/shader
gap**; the gaps are layout/DOM-fidelity items.

- **G1 — inline `<svg>` container unsupported.** opencat parses `div/text/
  canvas/image/lottie/video/icon/path` (markup.rs:650). There is **no `<svg>`**
  element. Every inline SVG (GitHub mark, chips/lock icons, cursor arrow, play
  triangles, flow arrows) must be either (a) re-expressed as `<path>` /
  `<icon>` nodes, or (b) rasterised to PNG assets. Prefer `<path>` where the
  geometry is simple (chevrons, cursor, arrows), `<image>` for the GitHub mark.
- **G2 — CSS `mask-image`/`mask-composite` unsupported.** The opener pet video
  uses a 2-D feathered intersection mask. No mask primitive exists. Options:
  omit the pet (±small error, it is a corner decoration), or approximate with a
  soft radial-gradient overlay. Low priority.
- **G3 — container-query units `cqw`/`cqh` unsupported.** No `container-type`/
  cqw/cqh in the engine. Convert all scene CSS to px (`×19.2`/`×10.8`) — most
  scenes are already authored with a mix of px and cqw.
- **G4 — CSS `background-image: url(...)` (Tailwind `bg-[url(...)]`) unverified.**
  Avatar fills in feature-discussion/credits use `background-size:cover`
  circle backgrounds. opencat's `bg-*` handles gradients/colors (confirmed) but
  `url()` backgrounds are not proven — use an `<image>` node clipped to a
  rounded/circle container instead (the vfx/w2h ports do this).
- **G5 — `filter: blur()` on a tween (opener zoom-through, pr-scroll whip).**
  The engine supports script `filter: 'blur(Npx)'` (vfx actA uses it), so this
  should port; verify the tween accepts the filter string form used here.
- **G6 — live element measurement for cursor targeting.** The opener positions
  the cursor and ripple from `getBoundingClientRect()` at two window sizes.
  opencat script has no rendered-layout measure of arbitrary nodes (the vfx
  port replaced source `getBoundingClientRect` with precomputed offsets). Port
  by pre-computing the ▲/PR-link centres as px constants (the source already
  does this arithmetic once) or add a measure binding if one exists. This is
  the main **opener** fidelity risk.
- **G7 — ease coverage.** The film leans on `back.out(1.5/1.6/1.7/2)`,
  `power1/2/3/4.in/out/inOut`, `expo.out`, `steps(N)`, and
  `repeat/yoyo` blinks. Confirm each is in opencat's ease registry; approximate
  `back.*` with the closest preset if absent (the vfx port substituted
  `power2.out` for a custom cubic ease). `power*` and `none/linear` are the
  must-haves; `back.*` governs the bouncy feature-card entrances.
- **G8 — `textContent`-mutation typewriter vs splitText.** The source mutates
  `textContent` with a sliced string (opener, summarize, use-skill, cta) and
  swaps `innerHTML` to append a coloured URL (opener @2.1). opencat has no
  textContent mutation; use `ctx.splitText(..., {type:'chars'})` per-char
  reveal (vfx actA precedent) and place the URL as a second `<text>` node
  revealed at the swap time. `steps(N)` typing is linear per char in opencat.
- **G9 — audio.** The reference carries AAC BGM + 14 SFX. The k3diff gate is
  frame-only; assume the deliverable is a **silent** MP4 (as with the other
  titles). Port `<audio>` nodes for structural fidelity if the engine renders
  them, but do not let audio block the pixel gate. Confirm whether opencat
  muxes audio at all before investing (open question).
- **G10 — `video` trimming / sequencing is supported** (`data-start`,
  `data-duration`, `data-media-start` per markup.rs) — the pet-typing pair and
  any video backgrounds port directly. No gap.
- **G11 — woff2 fonts. RESOLVED this round.** Converted to `.ttf` under
  `assets/p2v/` (fonttools), 8 faces wired via the `<fonts>` block (Hanken
  400/500/600/700, Galaxie Book, Spline Mono 400/500, Archivo Black). One
  residual bug: the `black` face declares `family="CodeBlackHF"` but the TTF's
  actual family is `Archivo Black` → `font-[black]` falls back to default sans
  (§3.5). Fix the declared family.
- **G12 — `font-display`/`@font-face` CSS** is not a thing in opencat; fonts
  come from the `<fonts>` block only. Mechanical.
- **G13 — mono weight/advance fidelity (NEW, measured).** `SplineSansMono-500.ttf`
  reports `usWeightClass=400` and `family="Spline Sans Mono"` in its name table
  (same as the 400 face), so a `font-[mono5]` request may resolve to the 400
  face (fontdb weight matching) → wrong weight AND, more visibly, the port's
  mono runs are letter-spaced wider than the reference. Set the mono face
  weights explicitly (or set `font-weight` at the text node) and pin
  `letter-spacing:0` for mono blocks unless the source sets it. Affects the
  code editor, all composers, and the pr-scroll numbers.

No 3D, no GLB, no PBR, no refraction, no ACES: this title avoids everything
that capped the two 3D handoffs. The realistic floor is font/text-metrics.

---

## 6. Alignment plan & acceptance gates

Gate (task default): `avg_p8 ≤ 0.02`, `worst_p8 ≤ 0.08`.

**Measured status: `avg_p8 = 0.0950`, `mae = 8.95` (§3.5).** Not yet passing,
but the error is **structured**, not a floor: it concentrates at the seams and
in two font defects. This reference is 2D DOM/canvas with no volumetric
content, so unlike the two 3D titles (structural floors ≈0.10) **this title
has no structural floor** — the gate should be reachable once the seams and
fonts are fixed. Residual after that should be concentrated on large-type
serif intertitles/card titles (glyph raster + tracking + wrap) and typewriter
frames mid-word. If the measured floor lands above 0.02 solely due to font
anti-aliasing, record it against the measured floor (as the other handoffs do)
rather than chasing AA.

Priority order for the winding loop (highest p8-mass first, per §3.5):

1. **Fix the black-title family** (`family="Archivo Black"`) — one-line, kills
   the entire Act-2 title error (frames 483+, 555+, 630+, 690+).
2. **Fix the two big seams** (hard→summarize f279–285, use-skill→fcode
   f369–374). Re-measure both segments after; these are the top-12 worst
   frames.
3. **Mono tracking/weight** (G13) — affects code editor, composers, pr-scroll
   numbers.
4. **Remaining feature seams** (f486, 555/556, 630, 768) — same opacity-curve
   class as (2).
5. Per-scene motion / typewriter / opener cursor choreography (G6) last.


Non-goals: audio muxing (frame gate only); pet-video masking (G2, optional);
the post-hoc HyperFrames logo sign-off (not in the reference file).

---

## 7. How to measure

Reference:
```bash
REF=/home/solaren/Projects/hyperframes-launches/pr-to-video-launch/renders/pr-to-video-launch_2026-10-02_09-19-38.mp4
```

Render opencat (build first: `cargo build --release`):
```bash
cd /home/solaren/Projects/opencat
./target/release/opencat render examples/pr-to-video-launch.xml out/_p2v.mp4
```

Whole film (offset 0), the primary gate:
```bash
python3 tools/k3diff.py "$REF" out/_p2v.mp4 --out out/_p2v-k3diff
# read out/_p2v-k3diff/summary.json -> agg.mae_mean / agg.avg_p8
# out/_p2v-k3diff/pixel.csv -> per-frame mae/p8/bbox
```

Single scene (PNG, fastest iteration) — compare one frame:
```bash
./target/release/opencat render examples/pr-to-video-launch.xml out/_p2v_f0500.png 500
# then diff against the reference frame extracted the same way, or eyeball:
ffmpeg -y -v error -i "$REF" -vf "select='eq(n\,500)'" -vsync 0 /tmp/ref500.png
```

Per-scene cut (example: scene 6 `feature-code`, frames 483–554). No temporal
offset — the reference is 30 fps CFR from frame 0:
```bash
mkdir -p out/_p2v-ref out/_p2v
ffmpeg -y -v error -i "$REF" -vf "select='between(n\,483\,554)',setpts=PTS-STARTPTS" -an \
  -c:v libx264 -preset fast -crf 18 out/_p2v-ref/s06.mp4
ffmpeg -y -v error -i out/_p2v.mp4 -vf "select='between(n\,483\,554)',setpts=PTS-STARTPTS" -an \
  -c:v libx264 -preset fast -crf 18 out/_p2v/s06.mp4
python3 tools/k3diff.py out/_p2v-ref/s06.mp4 out/_p2v/s06.mp4 --out out/_p2v-s06-k3diff
```

Scene frame windows (§2.1), for `between(n,start,end)`:
`s01 0–155 · s02 156–251 · s03 252–278 · s04 279–368 · s05 369–482 ·
s06 483–554 · s07 555–629 · s08 630–689 · s09 690–767 · s10 768–899`.

Artifacts live under `out/` (gitignored). Nothing in `examples/` was modified
and nothing was committed by this intake.

---

## 8. Checklist (updated this round)

- [x] Reference confirmed valid (§1.1) — no re-render needed; re-probed this
      round: 1920×1080@30, 900 f, AAC stereo, 30.016 s container.
- [x] Spec recorded (§1).
- [x] Structure mapped: 10 scenes, windows, timelines, assets, fonts (§2).
- [x] `examples/pr-to-video-launch.xml` **exists and is fully authored**
      (10 sibling scenes + root xfades + script timeline).
- [x] Fonts converted to `.ttf` in `assets/p2v/` (§5 G11).
- [x] **Baseline measured: avg_p8 = 0.0950, mae = 8.95** (§3.5).
- [ ] **Fix `black` font family** → `family="Archivo Black"` (§3.5; biggest fix).
- [ ] **Fix hard→summarize + use-skill→fcode seams** (§3.5; top worst frames).
- [ ] Fix mono tracking/weight (G13).
- [ ] Fix remaining feature seams (f486, 555/556, 630, 768).
- [ ] Re-render + re-measure; iterate to `avg_p8 ≤ 0.02`.
- [ ] Decide audio: port stays silent; gate is frame-only (G9). Confirm whether
      opencat muxes `<audio>` before investing.
- Note: the XML and this handoff are **untracked** (not committed) — no commit
  was made this round.

---

## 9. Quick reference — one-line source map

| scene | src file | window s | frames | key beats |
|---|---|---|---|---|
| opener | opener.html | 0–5.2 | 0–155 | composer type→send, full-bleed→float morph, cursor clicks PR link |
| pr-scroll | pr-scroll.html | 5.2–8.4 | 156–251 | PR page: reviews flash, files whip −3600, stats type 629/87,291/4 |
| hard-read | hard-read.html | 8.4–9.3 | 252–278 | serif "Hard to read" |
| summarize | summarize.html | 9.3–12.3 | 279–368 | `summarize this PR`, summary walls in + scroll, "Still too long" |
| use-skill | use-skill.html | 12.3–16.1 | 369–482 | `use pr-to-video…`, thinking, "Reading PR #1635 · 629 files · 4 commits" |
| feature-code | feature-code.html | 16.1–18.5 | 483–554 | "SHOW YOUR CODE", player, code + truncation-fix highlight |
| feature-flow | feature-flow.html | 18.5–21.0 | 555–629 | "SHOW YOUR FEATURE", diff→storyboard→video flow |
| feature-discussion | feature-discussion.html | 21.0–23.0 | 630–689 | "SHOW YOUR REVIEW", review threads→merge |
| feature-credits | feature-credits.html | 23.0–25.6 | 690–767 | "SHOW YOUR CONTRIBUTOR", roster of 4 humans + Claude/Codex |
| cta | cta.html | 25.6–30.0 | 768–899 | GitHub mark, "Create your **PR video** today", `use pr-to-video <url>` |

---

## 10. Round r1（2026-10-09，对齐工程师-r1）— avg_p8 0.037816 → **0.036802**

产物：`out/_p2v_r3.mp4` + `out/_p2v-r3-k3diff/`（agg avg_p8 **0.036802**, mae **3.491**；
上一最优 r0=0.037816/3.556，产物 `out/hf-pr-to-video-launch-baseline-diff/`）。
逐场景 mass（p8 和，r0→r3）：s01 6.160→**4.913** / s02 5.684→5.852 / s03 0.155→0.180 /
s04 3.892→**3.703** / s05 2.327→2.341 / s06 2.795→2.934 / s07 2.741→2.803 / s08 2.557→2.591 /
s09 2.610→2.668 / s10 5.114→5.138。

### 10.1 引擎通用补全（均过 `cargo build --release` + `cargo test -p opencat-core`，699 passed）

1. **fromTo hold 语义**（`crates/opencat-core/src/script/runtime/animation/facade.js`，
   addTween fromTo 分支）：GSAP 中 toVars 未提及的 fromVar 作为补间起始状态并全程 HOLD；
   此前引擎丢弃它们，导致 `T.fromTo(el,{x:1691.8,opacity:0},{y:563.2,...})` 的 x 锚丢失。
2. **TextDisplayItem 行内盒背景**（`display/list.rs` + `display/build.rs` +
   `render/text.rs` 新 `paint_background_box()` + `render/helpers.rs`）：CSS 行内 text 的
   background（pill/badge 背景色+圆角+blend）原先被丢弃；现在 clip:text 时仍走字形填充，
   否则在字形前绘制背景盒。

### 10.2 XML 修复（examples/pr-to-video-launch/pr-to-video-launch.xml）

- **s04 summarize 滚动 dist**：`-667.9`（错）→ **`-578.352`**。推导：`dist=innerH−feedH+30`
  （summarize.html:79）；margin-collapse 布局、**全部 .al 单行不折行**（ref f315 目视确认）：
  innerH = headl 31.68×1.7 + 6×(26.88×1.7) + 15×(29.76×1.7) + 6×23.04 + 15×11.52 = 1397.952；
  feedH = 1080−2×115.2 = 849.6。此前 -667.9 多算了一行折行（+39px）、-2400 是更早的占位。
- **s02 pr-scroll 文件行行距**：153 行（17 原始 + 8 组克隆）text 加 `leading-[23.04px]`
  （SplineSansMono hhea 1927/−473/0 @ upm 2000 → normal 1.2 × 19.2px；ref 行高实测
  17.664×2+23.04+1=59.368px，端口原默认 1.5 行高 → 65.2px，whip 相位全错）；`py-[17.7px]`
  →`py-[17.664px]`。sm 场景节标题 margin 精化 `mt-[23px] mb-[11.5px]`→`23.04/11.52`。
- s01：光标常量按 opener.html 推导修正（x 1691.8 起点 y 563.2→x 1342.8,y 649.2；ripple
  1365−48/664−48）；morph 改 proxy 组合公式（opener.html:134-138）；`op-pet` 双段视频接入
  （assets/pet-typing.mp4，h264 无 alpha，背景≈窗口底色差 Δ≈1 不可见）。
- 四场景 knob 后按参考公式补时间标签 set（fc/ff/fd/fcred，inner 时间÷timeScale）。

### 10.3 关键测量事实（下一轮直接用）

- **参考渲染时间滞后**：链式逐帧跟踪 summarize 滚动（f315 基线）得 ref 曲线
  y≈405.5·(t−10.357)²（k=405.5），而源码时间（start 10.3s、dist 578.352、dur 1.25、
  power1.in）给 k=370.1。即 **hyperframes 对场景内补间的执行比 data-start 晚 ~1.7 帧**
  （10.357−10.3+半帧量化）。源码不可推导，未硬编码。**下一轮可全局验证**：用 pr-scroll
  whip 结束（f243±）或 feature 卡片节拍再测一次；若全局 +1.7f，把脚本时间线整体平移可
  大幅收 s04 f333-353 与 s02 f222-243。
- **p8 非单调伪影**：内容全错（如旧 dist=-2400 把文本滚出视口→feed 近全空）p8 反而低于
  “对齐但差 10px”（f340：r1 4.08% < r3 6.03%）。逐帧 worse/better 不能全信，以目视+
  total gate 为准。r3 的 s02/s04 “尾段变差”均属此类——r3 行相位实际已对齐（f200/f330/f340
  目视逐行吻合，f330 行差 ≤2px）。
- s02 尾段 f244-251 静态残差 p8≈0.100：ref/port 都在 blur(2px) 态（拉普拉斯 3.19 vs 3.07），
  差异来自 **opencat blur 与 Chrome blur 的核/强度差异**（端口文字更淡更散）+ **▾ 前缀缺失**
  （见 10.4）。不是行相位问题。
- 字体度量（fontTools 实测）：HankenGrotesk-400 normal LH = (1000+303)/1000 = **1.303**；
  SplineSansMono-400 = (1927+473)/2000 = **1.2**。Chrome `line-height:normal` 一律用此推导，
  凡 ref 未写 line-height 的文本（.rev、.gf、composer、编辑器）端口都应补对应 leading。
  注意 .rev 行高由 36.48px 头像主导，文本行高无关 → 已对齐无需改。

### 10.4 负结果（勿重复）

- **↵/▾ 等字形 tofu**：SplineSansMono latin 子集无 ↵(U+21B5)/▾(U+25BE)，opencat 无字体
  回退链。ref 由 Chrome 系统回退渲染。s02 文件行缺 ▾ 前缀（153 处，每处 ~29px 水平偏移
  含 gap 17.28+字形宽）。修法方向：引擎通用字体回退，或引擎缺字时用矢量三角替代——需通用
  方案，勿在 XML 里画假三角。
- **Archivo Black kerning ~4% 窄**（端口比 ref 宽）：未修。
- **steps(1) 半程跳变**：GSAP steps(1) 在补间中点跳变；引擎当前整段保持——未修（s02 caret
  闪烁等影响小）。
- 离线拟合 ref 滚动自由参数 (D=1822,T=2.12) 是**约束不足的假解**（只测到 k=D/T² 曲线族），
  禁止把拟合值写进 XML；一切几何/时序常量必须从源码+布局规则推导。

### 10.5 下一轮候选（按 mass 排序）

1. **全局 ~1.7f 时间滞后验证与修复**（10.3 第一条）——潜在收益覆盖 s02 whip、s04 滚动、
   全部 feature 节拍，量级 ~2-3 mass。
2. **s01 opener（4.913）**：r3 已从 6.160 降（128 帧改善 0 变差），但仍是最大单场景；
   逐帧 bbox 看剩什么（morph 中段？cursor 轨迹细节？pet 位置？）。
3. **s10 cta（5.138）**：本轮未动，先做静态层对齐归因。
4. **s06-s09 feature（各 ~2.6-2.9，本轮 +0.03~0.14 漂移）**：无单帧超 0.005 的变化 →
   弥散性差异；嫌疑：text 背景盒（引擎修复②新画）的 bounds 是否含 padding、pill 缺
   `border:1px solid`（ref 有）、时间标签 set 的相位。逐 pill 放大对比。
5. **opencat blur σ 语义** vs Chrome blur(2px)（s02 尾段 2.8 mass 的主要成分）。

---

## 11. Round 1 归档（2026-10-09，工作流权威测量）— avg_p8 **0.035777**，kept ✅

> §10 是本轮工程师的本地测量与归因过程；本节是工作流管道的权威测量归档。
> 两者数字略有差异（0.036802 vs 0.035777）：来源分别是本地渲染
> `out/_p2v_r3.mp4` + `tools/k3diff.py`，与工作流端测量；以本节为轮次结论。

### 11.1 权威测量

| 指标 | 值 |
|---|---|
| **avg_p8** | **0.035777**（历史最优，此前 0.037816 @r0） |
| mae_mean | 3.369465 |
| worst_p8 | 0.105758（仅记录，不作门；本地观测最差簇为 s02 尾段 f241–251 ≈0.10x，帧号未随权威测量提供） |
| kept | true（采纳为新的历史最优） |
| per-act | actStats 为空——工作流本轮未提供 per-act 拆分；下表 per-scene 来自本地
  `out/hf-pr-to-video-launch-baseline-diff/pixel.csv` vs `out/_p2v-r3-k3diff/pixel.csv`
  的 p8 求和（§10.1），仅供参考 |

per-scene mass（本地测量，p8 和，r0→本轮）：s01 opener 6.160→**4.913** /
s02 pr-scroll 5.684→5.852* / s03 hard 0.155→0.180 / s04 summarize 3.892→**3.703** /
s05 use-skill 2.327→2.341 / s06 fcode 2.795→2.934* / s07 fflow 2.741→2.803* /
s08 fdisc 2.557→2.591* / s09 fcred 2.610→2.668* / s10 cta 5.114→5.138*。
（* 标记的增量经 f340 三方对比证实主要是 **p8 非单调伪影**——旧值把内容滚出视口后
feed 近全空反而得分更高；行相位实际已对齐，f330 逐行差 ≤2px。见 §10.3。）

### 11.2 改动内容与参考出处

| 改动 | 参考出处 |
|---|---|
| s04 滚动 dist `-667.9`→`-578.352`（margin-collapse 布局、全部 .al 单行；innerH=1397.952、feedH=849.6） | summarize.html:79 `dist=inner.offsetHeight-feed.offsetHeight+30`；ref f315 帧目视 |
| s02 153 个文件行（17 原始+8 组克隆）text 加 `leading-[23.04px]`、`py-[17.664px]` | pr-scroll.html `.gf`（`.92cqw` padding + font 1cqw，SplineSansMono normal LH 1.2 → 行高 59.368px）；pr-scroll.html:112 克隆 ×8 |
| sm 节标题 margin `mt-[23px] mb-[11.5px]`→`23.04/11.52`、headl `leading-[1.7]` | summarize.html `.al.h`（mt 1.2cqw / mb .6cqw）、`.al` line-height 1.7 |
| s01 光标/ripple 常量、morph proxy 组合公式、`op-pet` 双段视频 | opener.html:134-138 morph 公式；opener.html:24 pet |
| 4 个 feature 场景 knob 后补时间标签 set（inner 时间 ÷timeScale） | feature-*.html 各自的 Math.floor 计数补间 |
| 引擎：fromTo 中 toVars 未提及的 fromVar HOLD（GSAP 语义） | GSAP fromTo 文档语义；`crates/opencat-core/src/script/runtime/animation/facade.js` |
| 引擎：TextDisplayItem 行内盒背景（pill/badge 背景+圆角+blend，clip:text 仍走字形填充） | summarize/feature html 各 pill/badge 的行内 background | 

### 11.3 负结果 / 坑（本轮实测）

- **hyperframes 场内补间比 data-start 晚 ~1.7 帧**：链式跟踪 ref summarize 滚动得
  y≈405.5·(t−10.357)²，源码时间（10.3s 起）给 k=370.1（同曲线族、起始差 ~1.7f）。
  源码不可推导，未硬编码；下一轮可全局验证后统一平移时间线。
- **p8 非单调**：内容全错（feed 近全空）得分可优于“对齐但差 10px”（f340：4.08% < 6.03%）。
  逐帧 worse/better 不可全信，以目视 + total gate 为准。
- **离线拟合的假解**：自由拟合 ref 滚动得 (D=1822.5, T=2.12)——约束不足的曲线族解
  （测量只定 k=D/T²），禁止把拟合值写进 XML；常量必须从源码+布局规则推导。
- **字体 normal 行高**：HankenGrotesk-400 = (1000+303)/1000 = **1.303**；
  SplineSansMono-400 = (1927+473)/2000 = **1.2**（fontTools 实测 hhea）。ref 未写
  line-height 的文本端口都要按此补 leading；例外：.rev 行高由 36.48px 头像主导。
- **▾/↵ 字形 tofu**：SplineSansMono latin 子集无 U+25BE/U+21B5，opencat 无字体回退链；
  ref 由 Chrome 系统回退渲染。需引擎通用回退，勿在 XML 画假三角。
- **opencat blur σ vs Chrome blur(2px)**：s02 尾段（f244–251 静态 p8≈0.10）双方都在
  blur 态（拉普拉斯 3.19 vs 3.07），端口文字更淡更散——核语义差异未修。
- **Archivo Black kerning ~4% 窄**、**steps(1) 半程跳变语义**：沿承未修。

### 11.4 下一步建议（按潜在收益排序）

1. **全局 ~1.7f 时间滞后验证与修复**（§10.3 第一条）：用 pr-scroll whip 结束（f243±）或
   feature 卡片节拍再测一次；若全局成立，脚本时间线整体平移可收 ~2–3 mass。
2. **s01 opener（4.913）**：逐帧 bbox 归因残差（morph 中段 / cursor 轨迹细节 / pet 位置）。
3. **s10 cta（5.138）**：本轮未动，先做静态层归因。
4. **s06–s09 弥散差异**（各 ~2.6–2.9，无单帧 >0.005）：嫌疑 text 背景盒 bounds 是否含
   padding、pill 缺 `border:1px solid`、时间标签相位；逐 pill 放大对比。
5. **opencat blur σ 语义**（s02 尾段 ~2.8 mass 主成分）。

---

## 12. Round r2（2026-10-09，对齐工程师-r2）— 本地同二进制 0.036802 → **0.035431**

产物：`out/_p2v_r2b.mp4` + `out/_p2v-r2b-k3diff/`（agg avg_p8 **0.035431**, mae **3.3196**；
对照基线 r1 反向版**用本轮同一二进制**重渲：`out/_p2v_r1reb.mp4` + `out/_p2v-r1reb-k3diff/`
= 0.036802/3.4910，与 §10 r1 工程师本地数字一致）。逐场景 mass（r1reb→r2b）：
s01 4.913→**4.345** / s02 5.852→5.852 / s03 0.180→0.180 / s04 3.703→**3.087** /
s05 2.341→**2.291** / s06–s10 全部 0 变化（本轮未触碰）。
⚠️ 对照方法学：工作流权威测量（§11，0.035777）与本地渲染管道有 ~0.001 系统差；本轮
发现工作树里引擎二进制（target/release 01:42）≠ r1 权威测量时的状态，逐帧对比会误判
s01/s02/s06"退步"——**必须用同二进制的 r1 反向 XML 对照**（`/tmp/p2v_r1_xml/` 生成法见
本轮脚本：仅 reverse 本轮 5 组编辑）。

### 12.1 核心负结果（推翻 §10.5/§11.4 第 1 候选）：无全局时间滞后

- **"hyperframes 场内补间晚 ~1.7f"是伪象**。summarize 滚动第二测点（互相关逐帧垂直偏移，
  `out/_p2v_diag` 方法）：f335/340/345/347/350 实测偏移 7/9/11/12/12px，与"纯 dist 差、
  时间精确对齐"的期望 5.8/8.2/11.1/12/12px 逐帧吻合（残差 ≤2px）。r1 的 405.5 vs 370.1
  k 拟合差异实为 dist 差（见下），不是 t0 平移。**禁止再做时间线整体平移。**
- hyperframes 执行语义已从源码确认（`/home/solaren/Workspaces/hyperframes`）：
  `packages/core/src/runtime/init.ts:613-645` `addMissingChildCandidatesToRootTimeline` 把
  `__timelines` 里的场景 timeline `root.add(child, data-start)` 挂进根 timeline，渲染时
  `player.renderSeek(t)`（init.ts:2136）→ `totalTime(quantizeTimeToFrame(t))` 由 GSAP 父子
  传播精确驱动场景局部时间 = t − data-start。`fileServer.ts` `hf.seek` 先 renderSeek 再
  `__HF_VIRTUAL_TIME__.seekToTime`（rAF flush 用虚拟时间戳）。`__HF_EXPORT_RENDER_SEEK_CONFIG`
  的 offsetFraction 无消费者（哨兵）。→ 场景相位精确，opencat 的 data-start 映射是对的。

### 12.2 真因：Chrome offsetHeight 取整投影（本轮修正的推导链）

参考脚本读 `offsetHeight`（CSSOM **整数**），且 BFC 根（absolute 容器）不与最后孩子
margin 塌缩 → 尾 mb 计入高度。float 推导差一位尾 mb + 取整。用 headless Chrome 跑参考
composition 结构实测验证（诊断推导，非像素测量；probe 已清理）：

| 场景 | ref 源码 | float 推导 | Chrome offsetHeight | 本轮修正 |
|---|---|---|---|---|
| s04 summarize dist | summarize.html:80 `inner.offsetHeight-feed.offsetHeight+30` | innerH 1397.952+尾mb 11.52=1409.472；feedH 849.6 | **1409 / 850 → dist 589** | `y:-578.352`→`y:-589` |
| s01 opener dM1 | opener.html:108 `m1.offsetHeight+30.72`（floating 宽 1599.36 下测） | 行1 43.008 + 行2 max(43.008, prlink margin box 11.52+43.008)=54.528 → 97.536 | **98 → dM1 128.72** | `op-stack y:160`→`128.72` |
| s05 use-skill dCmd/dOut | use-skill.html:66 | cmd 25.92×1.4=36.288；out think(1.303×30.72=40.032)+19.2+l0(27.84×1.45=40.368)=99.6 | **36/100 → 62.88/126.88** | `us-stack y:180→189.76`、中段 `y:90→126.88` |

附带的静态层修正：
- **s01 op-feed bottom `286.8`→`316.8`**（opener.html:47 `bottom:16.5cqw`；端口此前写错
  30px，m1 静止位置整体低 30px，f130 互相关 42→30→0 验证）。cursor 点击目标
  `(1342.8,649.2)`→`(1342.8,619.2)`、ripple top `664-48`→`634-48`（prlink 中心 y 随 feed 底
  -30；opener.html:109-110 公式不变）。
- **s01 op-prlink 补 `mt-[11.52px]`**（opener.html:51 `.prlink{display:inline-block;
  margin-top:.6cqw}`）：inline-block margin box 把第二行行盒撑到 54.528（m1 高 86.016→97.536）。
- **s05 think/l0/cmd 行高补 leading**：`us-ast leading-[40.032px]`、`auto192 35.025px`、
  `us-l0 40.368px`、`auto191 36.288px`（ref .think 无 line-height → HK normal 1.303 hhea、
  .ol 1.45、.urow 1.4；引擎默认 1.5，style.rs:1620）——使 us-out 布局高=99.6，与 dOut 的
  Chrome 读数（100+26.88）自洽。

### 12.3 负结果 / 坑（本轮新增）

- **同二进制对照是硬要求**：引擎二进制被并行工程师改动时，拿历史 pixel.csv 逐帧比会
  得出假退步（本轮最初 s01 +0.349 的误判）。对照 = 当前二进制 × reverse-XML。
- Chrome offsetHeight 投影三件套（取整 + BFC 尾 margin + inline-block margin box 撑行盒）
  适用于一切"ref 脚本读 offsetHeight 再 set 位移"的场景；本轮已修全部三处
  （grep offsetHeight 全项目仅 opener:108 / summarize:80 / use-skill:67）。
- 渲染层 float 布局仍与 Chrome 的 paint-time 像素取整有亚像素差（本次未发现需要修的
  证据，行位置残差 ≤2px）。

### 12.4 下一步候选（按 mass 排序，r2b 实测）

1. **s02 pr-scroll（5.852，最大）**：剩余大头是 f234–251 blur 态静态残差（p8≈0.10×18 帧
   ≈1.8 mass）——blur σ 语义 vs Chrome blur(2px)，及 ▾ 前缀 tofu（引擎字体回退缺失）。
   两者均为引擎通用补全（§10.4、§11.3 已记录方案方向）。
2. **s10 cta（5.138）**：未归因，先静态层对齐。
3. **s01 opener（4.345）**：剩余最差簇 f82–85（morph 中段，p8 0.07–0.089），bbox 归因
   （composer morph 轨迹 / pet 位置）。
4. **s04 summarize（3.087）**：滚动段已对齐；剩余 top f309–313（feed 刚显出时的顶部内容
   差 p8≈0.06–0.082，与 dist 无关，疑 headl ✳ tofu 或字体度量）。
5. **s06–s09 弥散**（各 2.6–2.9）：沿承 §11.4 第 4 条。

---

## 13. Round 2 归档（2026-10-09，工作流权威测量）— avg_p8 **0.034439**，kept ✅

> §12 是本轮工程师的本地测量与归因过程；本节是工作流管道的权威测量归档。
> 两者数字略有差异（0.035431 vs 0.034439）：来源分别是本地渲染
> `out/_p2v_r2b.mp4` + `tools/k3diff.py`，与工作流端测量；以本节为轮次结论。

### 13.1 权威测量

| 指标 | 值 |
|---|---|
| **avg_p8** | **0.034439**（历史最优，此前 0.035777 @r1） |
| mae_mean | 3.197274 |
| worst_p8 | 0.105758（仅记录，不作门；本地观测最差簇为 s02 尾段 f234–251 ≈0.10–0.108） |
| kept | true（采纳为新的历史最优） |
| per-act | actStats 为空——工作流本轮未提供 per-act 拆分；下表 per-scene 来自本地
  `out/_p2v-r1reb-k3diff/pixel.csv` vs `out/_p2v-r2b-k3diff/pixel.csv`（同二进制对照）
  的 p8 求和（§12），仅供参考 |

per-scene mass（本地同二进制对照，r1reb→本轮 r2b）：s01 opener 4.913→**4.345** /
s02 pr-scroll 5.852→5.852（未触碰） / s03 hard 0.180→0.180 / s04 summarize 3.703→**3.087** /
s05 use-skill 2.341→**2.291** / s06–s10 全部 0 变化（未触碰，纯净性对照通过）。

### 13.2 改动内容与参考出处

| 改动 | 参考出处 |
|---|---|
| s04 滚动 dist `-578.352`→`-589`：float innerH 1397.952 + 尾 mb 11.52（#sm-inner 为 absolute=BFC 根，尾 margin 不塌缩）=1409.472 → offsetHeight 1409；feedH 849.6 → 850；dist=1409−850+30 | summarize.html:80 `dist=Math.max(0,inner.offsetHeight-feed.offsetHeight+30)`；CSSOM offsetHeight 整数语义；headless Chrome 布局实测验证 |
| s01 op-stack 初始 `y:160`→`128.72`（dM1=98+30.72：m1 行1 43.008 + 行2 max(43.008, prlink margin box 54.528)=97.536→98） | opener.html:108 `dM1=m1.offsetHeight+GAP`（GAP=30.72，floating 宽 1599.36 下测） |
| s01 op-prlink 补 `mt-[11.52px]`（inline-block margin box 撑第二行行盒至 54.528，m1 高 86.016→97.536） | opener.html:51 `.prlink{display:inline-block;margin-top:.6cqw}` |
| s01 op-feed `bottom-[286.8px]`→`bottom-[316.8px]`；cursor 点击目标 `(1342.8,649.2)`→`(1342.8,619.2)`、ripple top `664−48`→`634−48`（prlink 中心随 feed 底 −30） | opener.html:47 `.feed{bottom:16.5cqw}`=316.8；opener.html:109-110 linkX/linkY = prlink center − (22.2,14.8) |
| s05 us-stack 初始 `y:180`→`189.76`（dCmd+dOut=(36+26.88)+(100+26.88)）、中段 `y:90`→`126.88`（dOut） | use-skill.html:66 `dCmd=cmd.offsetHeight+GAP, dOut=out.offsetHeight+GAP`（GAP=26.88）；use-skill.html:73 中段停驻 |
| s05 行高补 leading：us-ast 40.032 / auto192 35.025 / us-l0 40.368 / auto191(cmd) 36.288 px（out 布局高=99.6，与 dOut 的 Chrome 读数 100+26.88 自洽） | use-skill.html:46-47 `.think` 无 line-height→HK normal 1.303、`.ol` 1.45；`.urow` 1.4（use-skill.html:39）；引擎默认 1.5（crates/opencat-core/src/style.rs:1620） |

### 13.3 负结果 / 坑（本轮实测）

- **「hyperframes 场内补间晚 ~1.7f」是伪象，禁止时间线整体平移**：summarize 滚动段逐帧
  互相关（f335/340/345/347/350 → 7/9/11/12/12px）与"纯 dist 差、时间精确对齐"的期望
  （5.8/8.2/11.1/12/12px）逐帧吻合（残差 ≤2px）；r1 的 k=405.5 vs 370.1 拟合差实为
  dist 差被吸收进 t0。hyperframes 执行语义从源码确认精确（`packages/core/src/runtime/
  init.ts:613-645` 场景 timeline 被 `root.add(child, data-start)`，renderSeek 经
  `totalTime(quantizeTimeToFrame(t))` 父子传播；`fileServer.ts` hf.seek 先 renderSeek 再
  `__HF_VIRTUAL_TIME__.seekToTime`；`__HF_EXPORT_RENDER_SEEK_CONFIG` 无消费者）。
- **Chrome offsetHeight 投影三件套**：凡 ref 脚本读 offsetHeight 再 set 位移的场景，float
  推导必须加 (a) CSSOM 整数取整 (b) BFC 根与尾孩子 margin 不塌缩（尾 mb 计入高度）
  (c) inline-block margin box 撑行盒。本轮修完全部三处（全项目 offsetHeight 仅
  opener:108 / summarize:80 / use-skill:67）。
- **同二进制对照是硬要求**：工作树引擎二进制被并行工程师改动时，拿历史 pixel.csv 逐帧
  比会得出假退步（本轮最初对 r1 权威 CSV 比较误判 s01 +0.349；用当前二进制重渲 r1
  反向 XML（`out/_p2v_r1reb.mp4`，0.036802 与 §10 本地数字一致）后确认本轮净改善）。
  对照 = 当前二进制 × reverse-XML，不要跨二进制比逐帧 CSV。
- 沿承未修：opencat blur σ vs Chrome blur(2px)、▾/↵ 字形 tofu（无字体回退链）、
  Archivo Black kerning ~4%、steps(1) 半程跳变。

### 13.4 下一步建议（按潜在收益排序）

1. **s02 pr-scroll（5.852，最大场景）**：剩余大头是 f234–251 blur 态静态残差
   （p8≈0.10×18 帧 ≈1.8 mass）——blur σ 语义 vs Chrome blur(2px) 的引擎通用对齐 +
   ▾ 前缀 tofu（引擎通用字体回退链）；两件均为引擎补全，方案方向见 §10.4/§11.3。
2. **s10 cta（5.138）**：两轮未动，先做静态层归因。
3. **s01 opener（4.345）**：剩余最差簇 f82–85（morph 中段 p8 0.07–0.089），逐帧 bbox
   归因 composer morph 轨迹 / pet 位置。
4. **s04 summarize（3.087）**：滚动段已对齐；剩余 f309–313（feed 显出时顶部内容差，
   与 dist 无关，疑 headl ✳ tofu 或字体度量）。
5. **s06–s09 弥散**（各 2.6–2.9，无单帧 >0.005）：text 背景盒 bounds 是否含 padding、
   pill 缺 `border:1px solid`、时间标签相位（沿承 §11.4 第 4 条）。

---

## 14. Round r3（2026-10-09，对齐工程师-r3）— 本地同管线 0.034439 → **0.034135**

产物：`out/_p2v_r3post.mp4` + `out/_p2v-r3post-k3diff/`（agg avg_p8 **0.034135**, mae **3.1191**）。
对照基线：本轮改动前工作树重渲 `out/_p2v_r3pre.mp4` + `out/_p2v-r3pre-k3diff/`
= **0.034439 / 3.1973，与 r2 权威测量（§13）完全一致**——本轮 pre/post 同管线同流程，
唯一差异 = 本轮改动，无需 reverse-XML。逐场景 mass（pre→post）：
s01 4.102→4.114 / **s02 5.628→5.224（−0.404，目标场景）** / s03 0.137→0.137 /
s04 4.054→4.098 / s05 1.359→1.412 / s06 2.811→2.811 / s07 2.657→2.653 /
s08 2.731→2.748 / s09 2.411→2.410 / s10 5.105→5.114。

### 14.1 改动内容与参考出处

| 改动 | 参考出处 |
|---|---|
| **引擎通用**：`HostInputs::with_font_fallback_dirs`（core lifecycle）+ `extend_db_with_fallback_dirs`（prepare_font_db 两个返回点统一加载，文档/base 字体之后、best-effort） | Chrome 对缺字形经 fontconfig 在系统字体目录解析的语义（fc-match 实证：本机 ▾/↵/→ 均回退 DejaVu） |
| **引擎通用**：`engine_font_fallback_dirs()`（engine fonts.rs，标准 fontconfig 目录 /usr/share/fonts、/usr/local/share/fonts、~/.fonts），接入 pipeline 主路径与 inspect | 同上 |
| **引擎通用**：`BrowserEmojiFirstFallback`（core text/mod.rs，impl cosmic_text::Fallback）：color emoji 置于 common 名单最前（color 字体只覆盖 emoji 码点，非 emoji 集群直接跳过），script/forbidden 转发 `PlatformFallback`；3 处 FontSystem 构造统一走 `shape_font_system` | Chrome emoji 呈现优先 color face 的行为；cosmic-text 0.19 `fallback/unix.rs` 名单（DejaVu Sans 含黑白 U+1F600 且排前） |
| **引擎注释**：helpers.rs `css_filter_image_filter` / dispatch.rs backdrop blur——记录 σ=R 实测结论（行为不变） | 见 14.2 |
| **XML**：s02 文件行 153 处（17 原始 + 136 克隆）补回 `<text class="text-[19.2px] leading-[23.04px] text-[#59636e] font-[mono4]">▾</text>`（首个 flex 子元素，id 按 div 跳号推导 auto056/cl1_056…） | pr-scroll.html:31-47 每行 `<span class="cv">▾</span>`；`.gf .cv{color:#59636e}`；此前端口整段丢弃（§10.4 的 153 处 ~29px 偏移即此） |

验证：`cargo build --release` ✓；`cargo test -p opencat-core` **699 passed** ✓；
`cargo test -p opencat-engine` 101/103——2 个失败为 pre-existing 环境问题（
`three_like_gltf_geometry_*` 依赖不存在的 `assets/lbr/heygen-prism.glb`（threejs 并行
工作未提交的资产）；`chromedriver_tailwind_layout_matches_taffy` 依赖 chromedriver），
与本轮改动无关（本轮触碰面：lifecycle/fonts/text/helpers 注释/dispatch 注释/XML）。
emoji 测试 `color_emoji_glyphs_render_via_generated_image_table` 曾因系统池挂、
BrowserEmojiFirstFallback 修复后通过。

### 14.2 关键实验：CSS `filter: blur(R)` 的 σ=R（r1/r2 的「σ 语义差异」假说否定）

s02 blur 态残差曾归因「opencat blur 与 Chrome blur 核/强度差异」（§10.3/§13.3）。本轮
同帧同区域（f242/245/250，x180-980 y400-960）拉普拉斯方差三方对比：

| 帧 | ref (Chrome) | pre (σ=2) | post (σ=1) |
|---|---|---|---|
| f242 | 12.9 | 16.8（+30%） | 211（+1533%） |
| f245 | 10.1 | 11.5（+14%） | 190（+1790%） |
| f250 | 10.1 | 11.5（+14%） | 190（+1790%） |

结论：**Chrome `filter: blur(2px)` 视觉等效 σ≈2**（MDN「R 即标准差」正确）；引擎现状
（CssFilterOp.value 直接作 sigma，helpers.rs:176）本来就是对的，一度改为 σ=R/2 后目检+
拉普拉斯双重否定，已回滚（仅留注释）。bjango「σ=r/2」与 Chromium issue 40964926 讲的是
**box-shadow 的 blur-radius** 语义，不适用于 filter blur()。
**剩余锐度差（pre 比 ref 锐 14-30%，ref 更糊）**：方向已明（ref 侧更模糊），无法从参考
源码推导（疑 ref 编码/合成管线的额外平滑），属渲染器固有差异，不再追。

### 14.3 负结果 / 坑（本轮新增）

- **全量系统字体池会翻转 emoji 呈现**：DejaVu Sans 自带黑白 U+1F600 且在 cosmic-text
  Unix common 名单中排第 2（在 Noto Color Emoji 前）——池加入系统目录后 😀 从
  ColorImage 变轮廓字形（engine `color_emoji_*` 测试当场抓获）。**勿按字体名拉黑名单
  修**（首个黑名单 NotoSansSymbols2 就猜错——它根本没有 U+1F600）；正解 =
  BrowserEmojiFirstFallback（emoji-first common list，通用、对非 emoji 零影响）。
- **→ (U+2192) 的回退样式漂移（本轮 s04/s05 +0.097 mass 的归因）**：SplineSansMono
  latin 子集无 →。pre 回退落 NotoSansSC（宽箭头）；post 回退落 DejaVu Sans（proportional
  advance）。**ref（Chrome）实际用 DejaVu Sans Mono**（mono advance、窄字形居中、两侧
  余白大——f342 条带三连目检；fc-match 'monospace:charset=2192' = DejaVuSansMono 印证：
  Chrome 的 per-glyph fallback 带 generic family 偏好）。cosmic-text 的 Fallback 名单是
  全局静态的，无法按 run 感知 mono/sans——**正解是通用补全「CSS font-family 的
  generic 尾传播」**（解析 `family,generic` → 文本 style 带 generic 类别 → fallback
  按类别选名单），勿把 DejaVu Sans Mono 全局前移（会伤 sans 文本的回退）。▾ 同理：
  Chrome 用 DejaVu Sans Mono 的 ▾（adv 0.602em=11.56px）而端口现在用 DejaVu Sans 的
  （0.502em=9.64px），每行文件名差 ~1.9px（blur 态影响小）。
- p8 非单调伪影沿承（§10.3）：本轮 s01/s04/s05 的小幅 +0.01~0.05 未见单帧 >0.0033 的
  突变，属弥散级；s02 −0.404 与 f232-251 全簇一致改善（每帧 −0.004~−0.008）是干净信号。

### 14.4 下一步建议（按潜在收益排序）

1. **generic-family 回退传播（引擎通用补全，直接承接 14.3）**：XML/样式层支持
   `font-[mono4,monospace]` 类 generic 尾 → ComputedTextStyle 带 generic 类别 →
   shape_font_system 按 run 类别选 fallback 名单（mono 文本回退 DejaVu Sans Mono、
   sans 回退 DejaVu Sans、serif 回退 Nimbus Roman 等，与 fc-match 的 generic 查询对齐）。
   潜在收益：s04/s05 的 → 段（~0.1 mass）+ s02 ▾ 残差 + s06-s09 弥散成分。
2. **s10 cta（5.114）**：三轮未动，先静态层归因。
3. **s01 opener（4.114）**：f82-85 morph 中段沿承（§13.4）。
4. **s04 summarize（4.098）**：滚动段已对齐；剩余头部内容差（✳ tofu 若存在，本轮字体池
   后可能已变化，需重看 f309-313）。
5. **s02 blur 态残余**（f234-251 每帧 ~0.095）：▾ 已修、σ 语义已证无差；剩余 = 14.3 的
   ▾ advance 1.9px + ref 侧额外平滑（不可推导）+ 文字 AA 固有差。优先级让位 1-4。

---

## 15. Round 3 归档（2026-10-09，工作流权威测量）— avg_p8 **0.034135**，kept ✅

> §14 是本轮工程师的本地测量与归因过程；本节是工作流管道的权威测量归档。
> 本轮本地与权威数字**完全一致**（0.034135）：pre/post 均为同管线渲染
> （`out/_p2v_r3pre.mp4` = 0.034439 与 r2 权威一致，证实同源后同二进制改动前后对照），
> 无跨管道系统差；以本节为轮次结论。

### 15.1 权威测量

| 指标 | 值 |
|---|---|
| **avg_p8** | **0.034135**（历史最优，此前 0.034439 @r2；链路 r0 0.037816 → r1 0.035777 → r2 0.034439 → r3 0.034135） |
| mae_mean | 3.119058 |
| worst_p8 | 0.100753（仅记录，不作门；对应本地 pixel.csv 最差帧 f242 = 0.100753，s02 pr-scroll blur 态簇 f232–251，逐帧已从 ~0.10 降至 0.094–0.101） |
| kept | true（采纳为新的历史最优） |
| per-act | actStats 为空——工作流本轮未提供 per-act 拆分；下表 per-scene 来自本地
  `out/_p2v-r3pre-k3diff/pixel.csv` vs `out/_p2v-r3post-k3diff/pixel.csv`（同管线对照）
  的 p8 求和（§14），仅供参考 |

per-scene mass（本地同管线对照，pre→post）：s01 opener 4.102→4.114 /
**s02 pr-scroll 5.628→5.224（−0.404，本轮目标）** / s03 hard 0.137→0.137 /
s04 summarize 4.054→4.098 / s05 use-skill 1.359→1.412 / s06 fcode 2.811→2.811 /
s07 fflow 2.657→2.653 / s08 fdisc 2.731→2.748 / s09 fcred 2.411→2.410 /
s10 cta 5.105→5.114。s02 f232–251 逐帧一致改善（−0.004~−0.008/帧）；
s04/s05 小幅上升（+0.044/+0.053）已归因为 → (U+2192) 回退字体样式漂移（§14.3）。

### 15.2 改动内容与参考出处

| 改动 | 参考出处 |
|---|---|
| 引擎通用：`HostInputs::with_font_fallback_dirs`（core lifecycle/types.rs + mod.rs）+ `prepare_font_db` 统一 `extend_db_with_fallback_dirs`（文档/base 字体之后加载，best-effort） | Chrome 对缺字形经 fontconfig 在系统字体目录解析的语义（fc-match 实证：本机 ▾/↵/→ 均回退 DejaVu 系） |
| 引擎通用：`engine_font_fallback_dirs()`（crates/opencat-engine/src/fonts.rs）——fontconfig 标准目录（/usr/share/fonts、/usr/local/share/fonts、~/.fonts）；接入 pipeline 主路径与 inspect | 同上 |
| 引擎通用：`BrowserEmojiFirstFallback`（core text/mod.rs，impl cosmic_text::Fallback）——color emoji 置于 common 回退名单最前，script/forbidden 转发 `PlatformFallback`；3 处 FontSystem 构造统一 `shape_font_system` | Chrome emoji 呈现优先 color face；cosmic-text 0.19 `fallback/unix.rs` 名单缺陷（DejaVu Sans 含黑白 U+1F600 且排前） |
| XML：s02 文件行 153 处补回 `<text class="text-[19.2px] leading-[23.04px] text-[#59636e] font-[mono4]">▾</text>`（首个 flex 子元素，id 按 div 跳号推导 auto056/cl1_056…） | pr-scroll.html:31-47 每行 `<span class="cv">▾</span>`；`.gf .cv{color:#59636e}`（§10.4 记录的 153 处 ~29px 水平偏移即此元素的缺失） |
| 引擎注释（行为不变）：helpers.rs `css_filter_image_filter`、dispatch.rs backdrop blur 记录 σ=R 实测结论 | §14.2 实验（见 15.3） |

验证（本轮实际执行）：`cargo build --release` ✓；`cargo test -p opencat-core`
**699 passed, 0 failed** ✓；`cargo test -p opencat-engine` 101/103——2 个失败为
pre-existing 环境问题（`three_like_gltf_geometry_*` 依赖不存在的
`assets/lbr/heygen-prism.glb`，threejs 并行工作未提交资产；
`chromedriver_tailwind_layout_matches_taffy` 依赖 chromedriver），与本轮触碰面无关。

### 15.3 负结果 / 坑（本轮实测）

- **「opencat blur σ 语义 vs Chrome blur(2px)」假说否定，禁止再改**：同帧同区域拉普拉斯
  方差（f242/245/250，x180-980 y400-960）ref 12.9/10.1/10.1 vs σ=2 渲染 16.8/11.5/11.5
  （+30%/+14%/+14%）vs σ=1 渲染 211/190/190（+1533%/+1790%）——**Chrome
  `filter: blur(R)` 实际 σ=R**（MDN 正确）；引擎 `css_filter_image_filter` 现状（value
  直接作 sigma）本来就对。一度改为 σ=R/2 后目检+拉普拉斯双重否定，已回滚仅留注释。
  bjango「σ=r/2」与 Chromium issue 40964926 讲的是 **box-shadow blur-radius** 语义，
  不适用于 filter blur()。剩余锐度差（端口比 ref 锐 14-30%，ref 更糊）疑 ref 编码/合成
  管线的额外平滑，无法从源码推导，不再追。
- **全量系统字体池会翻转 emoji 呈现**：DejaVu Sans 自带黑白 U+1F600 且在 cosmic-text
  Unix common 名单中排第 2（在 Noto Color Emoji 前）。勿按字体名拉黑名单修（首个黑名单
  NotoSansSymbols2 就猜错——它没有 U+1F600）；正解 = BrowserEmojiFirstFallback
  （emoji-first common list，通用、对非 emoji 零影响）。
- **→ (U+2192) 回退样式漂移**（s04/s05 +0.097 mass 的归因）：SplineSansMono latin 子集
  无 →，Chrome per-glyph fallback 带 **generic mono 偏好**（ref 实际用 DejaVu Sans
  Mono，f342 条带三连目检 + fc-match 'monospace:charset=2192' 印证），而 cosmic-text
  的 Fallback 名单全局静态、无法按 run 感知 mono/sans。正解是通用补全「CSS font-family
  的 generic 尾传播」；勿把 DejaVu Sans Mono 全局前移（会伤 sans 回退）。▾ 同理：
  Chrome 用 DejaVu Sans Mono 的 ▾（adv 0.602em=11.56px）而端口现用 DejaVu Sans 的
  （0.502em=9.64px），每行文件名差 ~1.9px（blur 态影响小）。
- 沿承未修：Archivo Black kerning ~4%、steps(1) 半程跳变。

### 15.4 下一步建议（按潜在收益排序）

1. **generic-family 回退传播（引擎通用补全，直接承接 15.3）**：XML/样式层支持
   `font-[mono4,monospace]` 类 generic 尾 → ComputedTextStyle 带 generic 类别 →
   shape fallback 按类别选名单（mono→DejaVu Sans Mono、sans→DejaVu Sans、serif→Nimbus
   Roman，与 fc-match 的 generic 查询对齐）。潜在收益：s04/s05 → 段（~0.1 mass）+
   s02 ▾ advance 残差 + s06–s09 弥散成分。
2. **s10 cta（5.114）**：三轮未动，先静态层归因。
3. **s01 opener（4.114）**：f82–85 morph 中段沿承（§13.4）。
4. **s04 summarize（4.098）**：滚动段已对齐；剩余头部内容差（本轮字体池后 ✳ 等字形
   状态可能已变，重看 f309–313）。
5. **s02 blur 态残余**（f234–251 每帧 ~0.095）：▾ 已修、σ 语义已证无差；剩余 = ▾
   advance 1.9px + ref 侧额外平滑（不可推导）+ 文字 AA 固有差，优先级让位 1–4。

---

## 16. Round r4（2026-10-09，对齐工程师-r4）— 同管线 0.035020 → **0.034175**（s10 mass −0.726）

产物：`out/_p2v_r4.mp4` + `out/_p2v-r4-k3diff/`（本地 k3diff agg avg_p8 **0.034175**, mae **3.1111**）。
**本轮基线发现（先读）**：跨轮 k3diff 存在解码级系统漂移——对 r3 轮留下的
`out/_p2v_r3post.mp4`（≡ `out/hf-pr-to-video-launch-r3.mp4`，md5 同）用当前环境
k3diff 复测 = **0.035020 / 3.2432**（两次完全一致，复现性成立；见
`out/_p2v-r3post-recheck2/`），而 r3 轮同文件测得 0.034135 / 3.1191。差 +0.000885，
**非本轮改动引起**（该文件 r3 轮已渲染，本轮未触碰；且 `tools/k3diff.py` 无修改，
git diff 干净、mtime 2026-09-25）。疑测量环境（ffmpeg 版本/管道）在轮间变化。
**结论：跨轮权威数字不可直接比较，轮内必须同管线 pre/post 对照**——本轮
pre = r3post.mp4 复测 0.035020，post = r4 0.034175，**改善 −0.000845**。

### 16.1 误差源选择与归因

按 §15.4 先归因 s10 cta（同管线重测 mass 5.170，三轮未动）：

- 同帧提取（f810 = 场景内 1.4s，top 仍在初始态）：ref logo 质心 cy=**385**，
  our(r3)=**422**，垂直差 **37px**；水平完全对齐；f890（稳定态 scale=1,y=0）
  差 ≤2px → **只有「scale≠1 且 y≠0」的时段偏移**。
- 理论：CSS/gsap 语义（translate 外层）logo 中心 = 540+206+1.18×(237.6−540)
  = **389.2** ≈ ref 385 ✓；而「先加 y 再缩放」= 540+1.18×(237.6+206−540)
  = **426.3** ≈ our 422 ✓。差 = 0.18×206 = 37.1px，与实测吻合。
- 根因链（全部实读）：XML `T.set('cta-top',{scale:1.18,y:206})`（对象键序 scale
  先）→ `write_style_value`（store.rs:396）逐属性调 `push_transform`
  （mutations.rs，历史上「keep insertion order」）→ transforms = [Scale, TranslateY]
  → `apply_transform`（dispatch.rs:1040）按列表序 push canvas op，后 push 的先
  作用于点 → **y 被 scale 放大**。GSAP 真实行为 = 固定矩阵序
  `translate3d·rotate·scale·skew`，与写入顺序无关，translate 永不被放大。
- 附带影响面：XML 中 `scale+y` 共存动画共 6+ 处（fc/ff/fd/cr-pwin 入场
  `scale:.62,y:10` 类型 + cta-top），y 值 10–18px 时错误序造成 ~4–6px 偏移。

### 16.2 改动内容（引擎通用，XML 零改动）

| 改动 | 参考出处 |
|---|---|
| `push_transform` 写入后按 `matrix_rank`（translate=0, rotate=1, scale=2,
  skew=3）**稳定排序**（mutations.rs）；文档注释同步更新；新增 3 单测
  （`transform_components_normalize_to_gsap_matrix_order` /
  `transform_matrix_order_is_write_order_independent` /
  `transform_last_write_wins_survives_order_normalization`） | GSAP CSSPlugin 固定
  transform 构建序 `translate3d(...) rotate(...) scale(...) skew(...)`；CSS
  transform 函数语法同序。stable sort 保持同组 last-write-wins（OPEN card braces
  语义不变）。snapshot 回放（mutations.rs:692 record_* → push_transform）与 live
  写入同路径，一并覆盖 |

验证（本轮实际执行）：`cargo build --release` ✓；
`cargo test -p opencat-core` **702 passed, 0 failed**（r3 的 699 + 新增 3）✓。

### 16.3 同管线逐场景对照（pre=r3post 复测 → post=r4）

s01 +0.000 / s02 +0.000 / s03 +0.000 / s04 **−0.070** / s05 +0.000 /
s06 −0.001 / s07 −0.014 / s08 −0.004 / s09 +0.057 / **s10 −0.726**；
total **−0.760**。s01/s02/s03/s05 逐帧 bit 级不变（无回归扩散）；s09 最大单帧
+0.0022（f693，弥散级）；s10 初始态簇 f805–813 每帧 **−0.0282**（干净信号）。
f810 logo 质心：r3=422 → r4=**385 = ref**（37px 偏移精确消除）。

⚠️ 不要拿「`out/hf-pr-to-video-launch-r3-diff/pixel.csv`（权威）vs
`out/_p2v-r4-k3diff/pixel.csv`（本地）」做逐场景差——两者解码环境不同，
会得到 s01/s02「+0.25/+0.23 假恶化」（实为跨管道系统差，已用 r3vs r4 双渲染
diff=0 + k3diff 解码复算证伪）。

### 16.4 负结果 / 坑（本轮新增）

- **跨轮 k3diff 数字漂移 ~0.0009**（详见章首）：同 mp4 对 r3 轮 0.034135 vs 本轮
  0.035020（复测两遍一致）。影响所有「kept 比较跨轮权威数字」的仲裁——若工作流
  管道与本地 k3diff 解码不同，应以**同轮同管线 pre/post** 为准。
- 灰度目检陷阱：p8 按 RGB 逐通道 max diff（k3diff.py:96），`convert('L')` 会把
  色度差抹掉——对齐目检必须用 RGB 通道比较。
- XML 路径：handoff §7 的 `examples/pr-to-video-launch.xml` 已过时（commit 1b1dfd5
  按项目分文件夹），现为 `examples/pr-to-video-launch/pr-to-video-launch.xml`。

### 16.5 下一步建议（按潜在收益排序）

1. **s02 pr-scroll（mass 5.452，worst 簇 f232–251 ~0.095/帧）**：▾ advance 1.9px
   残差（§15.3）+ ref 侧额外平滑（不可推导）。generic-family 回退传播（§15.4 #1，
   引擎通用补全）仍是下一个最大可动项，可同时吃 s04/s05 的 → 段。
2. **s01 opener（4.363）**：f82–88 morph 中段沿承（§13.4）。
3. **s04 summarize（3.016）**：字体池后重看 f309–313（✳ 等字形状态）。
4. **s09 fcred（2.724，r4 微升 0.057）**：cr-pwin 入场修复后残差，下轮顺带目检。

---

## 17. Round 4 归档（2026-10-09，工作流权威测量）— avg_p8 **0.033318**，kept ✅（历史最优 0.03332 @r4）

> §16 是本轮工程师的本地测量与归因过程；本节是工作流管道的权威测量归档。
> 与 r3 不同：**本轮本地与权威数字不再一致**（权威 0.033318 vs 本地 0.034175，
> 固定差 0.000857），且 r3post.mp4 复测同样呈 +0.000885 固定差（§16 章首）——
> 两轮偏移一致（0.000857/0.000885）证实是**跨管道解码系统差**（固定偏移）而非
> 随机漂移。两条管线独立结论相同：工作流 0.034135→0.033318（−0.000817）、
> 本地同管线 0.035020→0.034175（−0.000845），**均为干净改善**。以本节为轮次结论。

### 17.1 权威测量

| 指标 | 值 |
|---|---|
| **avg_p8** | **0.033318**（历史最优，此前 0.034135 @r3；链路 r0 0.037816 → r1 0.035777 → r2 0.034439 → r3 0.034135 → r4 0.033318） |
| mae_mean | 2.987838 |
| worst_p8 | 0.100753（仅记录，不作门；对应 s02 pr-scroll blur 态簇最差帧 f242 = 0.100753，与 r3 相同——s10 的 37px 偏移帧已修复，全局最差让回 f232–251 blur 簇） |
| kept | true（采纳为新的历史最优） |
| per-act | actStats 为空——工作流本轮未提供 per-act 拆分；per-scene 见 §16.3（同管线
  `out/_p2v-r3post-recheck2/pixel.csv` vs `out/_p2v-r4-k3diff/pixel.csv` 的 p8 求和，
  唯一差异 = 本轮改动），仅供参考 |

### 17.2 改动内容与参考出处

| 改动 | 参考出处 |
|---|---|
| 引擎通用：`push_transform`（crates/opencat-core/src/script/mutations.rs）写入后按
  `matrix_rank`（translate=0, rotate=1, scale=2, skew=3）**稳定排序**——transform
  组件的渲染矩阵不再依赖脚本属性写入顺序；stable sort 保持同槽 last-write-wins
  （OPEN card braces 语义不变）；snapshot 回放（mutations.rs:692 `record_*` →
  `push_transform`）与 live 写入共用此路径，一并覆盖。新增 3 单测
  （`transform_components_normalize_to_gsap_matrix_order` /
  `transform_matrix_order_is_write_order_independent` /
  `transform_last_write_wins_survives_order_normalization`） | GSAP CSSPlugin 固定
  transform 构建序 `translate3d(...) rotate(...) scale(...) skew(...)`（与写入顺序
  无关，translate 永在 scale 外层不被放大）；CSS transform 函数语法同序。参考帧
  f810（cta.html:80 `gsap.set(top,{scale:1.18, y:206, transformOrigin:'50% 50%'})`，
  场景内 1.4s 仍在初始态）：ref logo 质心 cy=385 = 540+206+1.18×(237.6−540) ✓ |
| XML | 零改动（本轮纯引擎通用修复） |

验证（本轮实际执行）：`cargo build --release` ✓；`cargo test -p opencat-core`
**702 passed, 0 failed**（r3 的 699 + 新增 3）✓。

### 17.3 负结果 / 坑（本轮实测，详 §16.4）

- **跨轮/跨管道 k3diff 数字存在 ~0.0009 固定系统差**：同一 `out/_p2v_r3post.mp4`
  （≡ `out/hf-pr-to-video-launch-r3.mp4`，md5 同）r3 权威测 0.034135、本轮环境复测
  0.035020（两次完全一致，复现性成立）；本轮 r4 权威 0.033318 vs 本地 0.034175，
  差 0.000857 与之吻合 → 偏移稳定。**轮内必须同管线 pre/post 对照；跨轮仲裁须
  同管道数字**。`tools/k3diff.py` 本身无改动（git diff 干净，mtime 2026-09-25），
  疑测量环境（ffmpeg 版本/管道）在轮间变化。
- **跨管道 pixel.csv 逐场景差会产出假回归**：r4 本地 csv vs r3 权威 csv 显示
  s01/s02「+0.25/+0.23 恶化」——已用 r3 vs r4 双渲染逐像素 diff=0 + k3diff 解码
  复算证伪为解码系统差；真实逐场景变化以 §16.3 同管线对照为准（s01/s02/s03/s05
  bit 级零变化）。
- **灰度目检陷阱**：p8 按 RGB 逐通道 max diff（k3diff.py:96），`convert('L')` 会把
  色度差抹掉——对齐目检必须用 RGB 通道比较。
- XML 路径：handoff §7 的 `examples/pr-to-video-launch.xml` 已过时（commit 1b1dfd5
  按项目分文件夹），现为 `examples/pr-to-video-launch/pr-to-video-launch.xml`。
- 沿承未修：s02 blur 态残余（▾ advance 1.9px + ref 侧额外平滑不可推导）、
  Archivo Black kerning ~4%、steps(1) 半程跳变。

### 17.4 下一步建议（按潜在收益排序）

1. **generic-family 回退传播（引擎通用补全，承接 §15.3/§15.4 #1）**：XML/样式层
   支持 `font-[mono4,monospace]` 类 generic 尾 → shape fallback 按类别选名单
   （mono→DejaVu Sans Mono 等）。潜在收益：s04/s05 的 → 段 + s02 ▾ advance 残差
   + s06–s09 弥散成分——r4 后它升为最大已归因可动项。
2. **s02 pr-scroll（同管线 mass 5.452，worst 簇 f232–251 ~0.095/帧）**：▾ advance
   1.9px 残差归 generic-family；ref 侧额外平滑不可推导（§14.2 已证 σ 语义无差）。
3. **s01 opener（4.363）**：f82–88 morph 中段沿承（§13.4）。
4. **s04 summarize（3.016）**：字体池后重看 f309–313（✳ 等字形状态）。
5. **s09 fcred（2.724，r4 微升 +0.057）**：cr-pwin 入场修复后残差，下轮顺带目检。

## 18. Round r5（2026-10-09，对齐工程师-r5）— 同管线 0.034175 → **0.034022**（generic-family 回退 + emoji presentation，零回归）

### 18.1 误差源选择

r4 权威逐帧（§17）重聚合（真实场景边界）：s02 prscroll 5.224 最大。§16/§17 连续三轮第一建议
均为「generic-family 回退传播」+ 本轮 CDP 实证发现 emoji-presentation 错误，故本轮做字体回退
一组（引擎通用层），XML 零改动。

### 18.2 引擎改动（全部通用，无示例 hack）

1. **GenericFamily 传播**（`crates/opencat-core/src/style.rs`）：`GenericFamily`
   {Serif,SansSerif,Monospace} + `parse_css`（sans-serif/system-ui/ui-* 等）；`NodeStyle`
   /`ComputedTextStyle` 增 `font_generic`，`resolve_text_style` 按 CSS 继承。
   `parse/jsonl/tailwind.rs` `font-[a,b,monospace]` 逗号尾解析（显式尾优先）；
   `fonts.rs` `FontRole::generic_family()`：role="mono"/"sans" 种化 generic（显式尾优先）。
2. **Presentation-aware emoji 提升**（`text/mod.rs`）：`EMOJI_PRESENTATION_RANGES`
   （Unicode 15.1 emoji-data，93 段）+ `text_prefers_emoji`（仅默认 emoji 呈现或 VS16 才前置
   Noto Color Emoji）。修复 `✳`(U+2733) 等文本呈现符号被旧 BrowserEmojiFirstFallback 渲成
   彩色位图（Chrome 不会；s04 summarize `.mk` 直接受益）。
3. **ShapingFallback{generic, emoji_first}** 实现 cosmic-text `Fallback`；六条字面名单；
   `shape_font_system` 三签名，调用点 measure_text / rasterize_glyphs / measure_script_text_width。
4. **最终 mono 名单序（本轮关键决策，post2）**：
   `["Liberation Mono","DejaVu Sans","DejaVu Sans Mono","Noto Sans Mono","FreeMono",
   "Liberation Sans","Noto Sans","FreeSans","Noto Sans Symbols","Noto Sans Symbols2"]`。
   模型：web 栈 `'X','Courier New',monospace` 的 'Courier New' 经 fontconfig metric-alias =
   Liberation Mono（有效 monospace，覆盖 → − ●；缺 ▾ ◆ ↵ ✳），缺失字形落到 fontconfig
   默认族 DejaVu Sans，其余 mono 兜底。测试 `missing_glyph_run_falls_back_by_generic_class`
   断言 mono '→'(LibMono 0.600em) < sans '→'(LibSans 1.000em)。

### 18.3 同管线对照（REF 相同、同二进制管线）

| 配置 | avg_p8 | mae | 判定 |
|---|---|---|---|
| pre（r4 状态复刻） | 0.034175 | 3.1111 | 基线 |
| post1（mono=DVSansMono first） | 0.034354 | 3.1560 | **弃**：s02 +0.62/帧（▾ 步进变宽，scene sum −59.2） |
| **post2（LibMono→DVSans→…）** | **0.034022** | **3.0900** | ✅ 采纳，全场景零回归 |

逐场景 pre→post2：s02 0.0568→0.0568（完全恢复）· s04 0.0335→0.0327（✳ 修复 + sans '→'→LibSans）
· s07 0.0372→0.0366（mono '→'→LibMono 比 DVSansMono 更贴 ref，佐证 alias 模型）·
s08 0.0424→0.0421。worst 帧 f236–251（s02 尾段列表区，p8≈0.10，bbox[50,98,1842,993]）。
验证：`cargo build --release` ✅；`cargo test -p opencat-core --lib` 708 passed ✅。

### 18.4 负结果 / 实证（勿重复考古）

- **ref 的 mono 语境 ▾ = DejaVu Sans ▾（大三角 0.502em），非 DejaVu Sans Mono**：f200 实测
  REF bbox x82–88/y241–248 ≈ PRE(DVSans) x82–89/y241–248；POST(DVSansMono) x84–89/y242–247
  小三角右移。本机 Chrome CDP（mono 栈 ▾→DVSansMono）与 ref 渲染机行为不符——ref 机器的
  有效 monospace 应为 Liberation Mono（alias 模型，post2 像素验证成立）。
- **Noto 假设全灭**：NotoSansCJK-Regular.ttc faces 5–9（Mono CJK JP/KR/SC/TC/HK）与
  assets/NotoSansSC-Regular.otf 均无 U+25BE。
- **fflow/fdisc 的 `.cap`（含 mono '→'）是 `display:none`，从未入片**：cap 带 y985–1060 在
  PRE/POST 间零差异。实际 mono '→' 出现处：s07 diffcard「+ storyboard → frames → render」、
  s08 卡片头部等。
- **s07/s08 的大头不是字体**：f609 约 9.3 万 diff px，其中字体相关仅 ~6.5k；f660 mono 头行
  REF x450 vs 我们 x803（~353px 相位差）→ 转场/动画相位或布局才是这两景主体（s07 base 0.0366、
  s08 0.0421）。下轮候选：s07/s08 的时间线相位/入场几何核对。
- s08 头行 '◆' fallback PRE/POST 仅差 ~1px（非主因）。
- steps(1) 半程跳变、Archivo Black kerning ~4%、blur σ=R 等沿用 §16.4 未修清单。

### 18.5 下轮建议

1. s07 fflow / s08 fdisc 相位核对（18.4 第 4 条，场景 base 0.037/0.042，量级远大于字体）。
2. s02 尾段（f236–251 worst≈0.10）：列表滚动/高亮态核对。
3. s06 fcode 0.0407 / s10 cta 0.0337 次级场景。
4. 字体层剩余收益已 <0.0002/轮，非首选。

## 19. Round 5 归档（2026-10-09，工作流权威测量）— avg_p8 **0.033184**，kept ✅（历史最优 0.03318 @r5）

> §18 是本轮工程师的本地测量与归因过程；本节是工作流管道的权威测量归档。
> 本地与权威数字延续 r4 的跨管道固定差模式：权威 0.033184 vs 本地 0.034022，
> 差 0.000838，落在 §17.3 证实的 ~0.0009 跨管道解码系统差内（r3 0.000885 /
> r4 0.000857 / r5 0.000838）。两条管线独立结论一致：工作流 0.033318→0.033184
> （−0.000134）、本地同管线 0.034175→0.034022（−0.000153），**均为干净改善且
> 逐场景零回归**。以本节为轮次结论。

### 19.1 权威测量

| 指标 | 值 |
|---|---|
| **avg_p8** | **0.033184**（历史最优，此前 0.033318 @r4；链路 r0 0.037816 → r1 0.035777 → r2 0.034439 → r3 0.034135 → r4 0.033318 → r5 0.033184） |
| mae_mean | 2.966688 |
| worst_p8 | 0.100753（仅记录，不作门；与 r4 权威 worst 值相同——s02 pr-scroll blur 态簇
  最差帧沿承，本轮 s02 的 ▾ 步进已恢复，残余为 ref 侧额外平滑，§18.4/§17.3） |
| kept | true（采纳为新的历史最优） |
| per-act | actStats 为空——工作流本轮未提供 per-act 拆分；per-scene 见 §18.3（同管线
  `out/_p2v-r5pre-k3diff` vs `out/_p2v-r5b-k3diff`，pre/post 唯一差异 = 本轮改动） |

### 19.2 改动内容与参考出处

| 改动 | 参考出处 |
|---|---|
| 引擎通用：`GenericFamily`（serif/sans-serif/monospace，含 `parse_css` 的
  sans-serif/system-ui/ui-* 别名）全链路传播——`style.rs` `NodeStyle`/`ComputedTextStyle`
  `.font_generic` + `resolve_text_style` CSS 继承；`parse/jsonl/tailwind.rs`
  `font-[hk4,monospace]` 逗号尾解析（显式尾优先）；`fonts.rs`
  `FontRole::generic_family()` role="mono"/"sans" 种化（显式尾优先）。新增 2 单测
  （`parses_font_face_with_generic_tail` / `face_role_seeds_generic_family_and_explicit_tail_wins`） | CSS Font Module §font-family 泛型族语义；参考
  compositions/*.html 各栈尾 `monospace`/`sans-serif`（pr-scroll.html `.gf`
  `'Spline Sans Mono',monospace` 等） |
| 引擎通用：presentation-aware emoji 提升——`text/mod.rs` `EMOJI_PRESENTATION_RANGES`
  （Unicode 15.1 emoji-data.txt，93 段）+ `text_prefers_emoji`：仅默认 emoji 呈现或
  VS16 前置 Noto Color Emoji；`✳`(U+2733)/`▾`/`↵`/`▏` 等文本呈现符号不再渲染成
  彩色位图。单测 `emoji_presentation_detection_matches_unicode_emoji_data` | UAX #51 / Unicode
  emoji-data.txt（Chrome 即此语义：CDP 实证 ref sans '✳' 为文本字形 DejaVu Sans） |
| 引擎通用：`ShapingFallback{generic, emoji_first}` 实现 cosmic-text `Fallback`，
  六条字面名单 + `shape_font_system` 三签名接入 measure_text / rasterize_glyphs /
  measure_script_text_width。**mono 名单序关键决策**：`["Liberation Mono","DejaVu Sans",
  "DejaVu Sans Mono","Noto Sans Mono","FreeMono","Liberation Sans","Noto Sans",
  "FreeSans","Noto Sans Symbols","Noto Sans Symbols2"]`。单测 3 个
  （`common_fallback_prefers_generic_class_faces` /
  `emoji_text_hoists_color_face_but_generic_order_holds` /
  `missing_glyph_run_falls_back_by_generic_class`：mono '→'=LibMono 0.600em <
  sans '→'=LibSans 1.000em） | web 栈 `'X','Courier New',monospace` 的 'Courier New'
  经 fontconfig metric-alias（30-metric-aliases.conf）= Liberation Mono = 有效
  monospace；其缺失字形（▾ ◆ ↵ ✳）落 fontconfig 默认族 DejaVu Sans。像素实证：
  f200 REF ▾ bbox x82–88/y241–248 ≡ DejaVu Sans ▾（DVSansMono-first 版致 s02
  +0.62/帧回归已弃，§18.3 post1）；sans '→'→Liberation Sans 为 CDP 实测 |
| XML | 零改动（本轮纯引擎通用修复） |

验证（本轮实际执行）：`cargo build --release` ✓；`cargo test -p opencat-core --lib`
**708 passed, 0 failed**（r4 的 702 + 新增 6）✓。

### 19.3 负结果 / 坑（本轮实测，详 §18.4）

- **本机 Chrome CDP ≠ ref 渲染机的字体回退**：CDP 实测本机 mono 栈 '▾' → DejaVu
  Sans Mono，但 ref 帧像素是 DejaVu Sans ▾——ref 机器的有效 monospace 为
  Liberation Mono（metric-alias 模型）。**照搬本机 CDP 结果会做出 s02 回归的名单序**
  （post1 0.034354 已弃）；跨机器字体行为以 ref 渲染像素仲裁。
- **Noto 系假设全灭**：NotoSansCJK-Regular.ttc faces 5–9（Mono CJK 全系）与
  assets/NotoSansSC-Regular.otf 均无 U+25BE（▾）。
- **fflow/fdisc 的 `.cap`（含 mono '→'）是 `display:none`，从未入片**：cap 带
  y985–1060 在 PRE/POST 渲染间零差异。实际 mono '→' 在 s07 diffcard 与 s08 卡片头。
- **s07/s08 的误差大头不是字体**：f609 约 9.3 万 diff px 中字体相关仅 ~6.5k；
  f660 mono 头行 REF x450 vs 我们 x803（~353px 相位差）→ 时间线相位/入场几何主导
  （s07 base 0.0366 / s08 0.0421，量级远大于字体残差）。
- 沿承未修：s02 blur 态 ref 侧额外平滑、Archivo Black kerning ~4%、steps(1)
  半程跳变（§16.4/§17.3 清单）。

### 19.4 下一步建议（按潜在收益排序）

1. **s07 fflow / s08 fdisc 时间线相位核对（承接 §18.5 #1）**：~353px 水平相位差
   （f660 头行）+ f609 卡片区整体错位——入场/滚动几何或场景内时钟偏移，场景 base
   0.0366/0.0421，是当前最大已归因可动项。
2. **s02 prscroll 尾段（worst 簇 f236–251 ~0.10/帧）**：▾ 步进已恢复（§18.3），
   残余集中在列表滚动/高亮态与 ref 侧 blur 平滑。
3. **s06 fcode 0.0407 / s10 cta 0.0337**：次级场景顺带目检。
4. **字体回退层收益已尽**（<0.0002/轮）：generic 名单序已按像素实证定案，非下轮首选。

## 20. Round 6（2026-10-09）— s07/s08 结构归因 + ptime 闭包 bug + `whitespace-normal` 引擎补全 — avg_p8 **0.033122**（0.034022→−0.00090），kept ✅（历史最优 0.03312 @r6）

> 本节为 r6 本地同管线测量（`out/_p2v-r6c-k3diff` vs r5 基线 `out/_p2v-r5b-k3diff`，同 ref
> `renders/pr-to-video-launch_2026-10-06_swgl.mp4`）。按 §17.3/§19 跨管道固定差 ~0.00084 推算，
> 权威预期 ≈ 0.0323（将优于 r5 权威 0.033184）。

### 20.1 误差源选择与归因（§18.5 #1 的落地）

§18.4 的「s07/s08 时间线相位」假设经帧级对照（`--dump 609,660`）证伪：**相位没错，是静态
布局结构错误**。两景内部各自实锤：

| 证据 | REF（Chrome） | r5 TEST |
|---|---|---|
| f660 `fd-chead` | `.cl`（◆+#1635 同一 flex 子项）贴左 x450 | 3 个子项被 `justify-between` 拉开，" #1635 · Conversation" 落中 x803 |
| f660 pill | `.pillwrap{width:190px}` 内 `.pill{position:absolute;right:0}`，右缘贴行内缘 | pill 为流内 flex 子项，open pill 右缘距行内缘 ~done pill 宽+gap |
| f609 ptime | `0:11`（随时间轴步进） | `0:14`（=循环终值，var 闭包陷阱） |
| f609 diffcard | flex-shrink 压到 373（布局宽）+ dl 文本换行，卡高 194px 屏 | 宽度收缩一致（x468–659 ✓），但文本不换行，卡高 119px |

### 20.2 改动内容

**XML（静态层，全部有参考行号注释）**
- `fd-chead`：auto376+auto377 包进一个 flex 子项 `auto376w`（ref feature-discussion.html:19）。
- 5 行 `fd-r*`：pill 对包进 `fd-pw*`（`relative w-[190px] h-[40px] flex-none`），pill 改
  `absolute right-0 top-[3.5px]`（(104−33)/2−33/2 相对 pillwrap；首版误用 crow 坐标 35.5 已纠）；
  badge/pill 补 `border-[1px]`+各 border 色+`leading-[1]`；cname/ctext `leading-[1]`；
  av 补 `border-[1px] border-[rgba(20,20,19,0.12)]`（ref 102-129）。
- `ff-diff`/`ff-fc*`/`ff-video` 补参考 box-shadow；`auto333/361` 补 mono normal 行高
  `leading-[1.2]`（字体 metrics：Spline Sans Mono typo=hhea=1.20，USE_TYPO_METRICS 置位）；
  `ff-fc*` 缩略图补 `.fct` 的 ::before/::after 装饰线与 inset-shadow、`.fccap::after` 第二条
  骨架线+`mb-[8px]`；`.fcn` chip 补 `bg-[rgba(250,249,245,0.94)] rounded-[6px] px-[6px]
  leading-[1.55] shadow` + z-[2]（ref feature-flow.html:106-127）。
- `ff-striplbl`：`left-[840px] top-[160px]`（离线补偿值，违 #1 精神）→ `left-0 right-0
  text-center bottom-[-46px]` + `tracking-[0.66px]`（.03em×22px）。
- `ff-a1/ff-a2` 加 `stroke-[3]`（.arrow stroke-width:3；opencat stroke cap/join 默认 round ✓）。
- **脚本层 4 处 ptime 循环 IIFE 捕获**（fc tk/f fk/fd dk/cr rk）：`for(var k…){T.set({set(){
  …k…}})}` 的 set 回调共享循环终值（fd 显示 0:28、ff 0:14、fc 0:06、cr 0:33），全部包
  `(function(k){…})(k)`。

**引擎通用补全（非示例 hack）**
- `whitespace-normal` Tailwind 类：`build.rs` 规则表 + `ExactClassAction::NormalWrap` +
  `NodeStyle.wrap_text` 字段 + `resolve_text_style` 继承传播。语义：把单文本节点恢复 CSS
  默认 `white-space:normal`（引擎默认 wrap_text=false 是标题/标签语义）。
  单测 `whitespace_wrap_classes_toggle_wrap_and_no_wrap`。
  **本轮 XML 最终未使用该类**（见 20.3），保留为能力补全（零渲染影响）。

### 20.3 发现未修（下轮首要工作项）：taffy 0.10.1 flex 收缩 leaf 不重算高度

`whitespace-normal` 在 dl 文本上生效后（中间态 `_p2v-r6b`）：dl1 "gh api --paginate …/files"
正确断行，但 **dl 行高仍按单行（h=51），第二行文字与下一行 dl 重叠**——taffy 对 flex 收缩的
measure-leaf 用 MaxContent basis 的高度，不以分配宽度 re-measure。探针实证（已从源码移除）：
- `measure_text` 本身正常：mono 26px "gh pr view --json files" INF→360.8×39，255→227.9×**78** ✓
- taffy 布局：text 分配宽 255/296/368（且 dl2 未收缩 w=368>容器 321——`measure_node` 对
  `AvailableSpace::MinContent` 返回 max-content（`MinContent => None → INFINITY`），taffy 的
  automatic min-size 因此=max-content，收缩失效），但 text rect.h 恒 39。
- 显式 `w-[255px]`（known_dimensions 必传）仍 h=39 → flex 最终 sizing 后 leaf 无 re-measure pass。
下轮路径：(a) `measure_node` 的 MinContent 分支返回真 min-content（近似：最长词宽），恢复收缩；
(b) 验证升级 taffy 或在 `compute_layout_with_measure` 后对宽高不符的 text leaf 做二次 measure。
**修完前不要在 flex 内使用 whitespace-normal**（重叠失真）。本轮已回滚 3 处 XML 使用
（`_p2v-r6c` 与 `_p2v-r6b` 持平 0.033123/0.033122 且无重叠，取 r6c）。

### 20.4 负结果 / 实证（勿重复考古）

- **§18.4「f660 REF x450 vs x803 ~353px 相位差」不是时间线相位**，是 fd-chead 的 DOM 子项数
  错误（3 vs 2）。时间线换算（/2.56、/4.1 等）经 ptime 步进帧核验全部正确。
- **`leading-none` 从未生效**：tailwind.rs 只解析 `leading-[..]`（无关键字表），XML:252-254/260/
  298/333/395/450/496/539-540 的历史 `leading-none` 实际行高=默认 1.5——是全部历史基线的组成
  部分，**不要「顺手修复」**（r6 中误激活 11 行已回滚；其中 `.ttext{line-height:1}` 参考语义
  确为 1，但牵动全部场景标题基线，须单独一轮验证）。
- opencat flex-shrink 的**宽度**收缩与 Chrome 一致（f609 diffcard x468–659 完全相同）——只有
  高度重算是缺口。
- ff-striplbl 旧 `left-[840px]` 无任何参考出处（离线补偿），已按参考语义重写。

### 20.5 测量表（本轮实际执行）

汇总（`python3 tools/k3diff.py renders/pr-to-video-launch_2026-10-06_swgl.mp4
out/pr-to-video-launch.mp4 --out out/_p2v-r6c-k3diff`，`summary.json`）：

| 指标 | r5b 基线 | r6 本轮 | Δ |
|---|---|---|---|
| avg_p8 | 0.034022 | **0.033122** | −0.000900 |
| mae_mean | 3.09 | **2.9588** | −0.13 |
| worst_p8 | （f242 沿承） | **0.10075**（f242，s02 prscroll 尾段） | 记录项，不作门 |

per-scene（工作流 actStats 为空；下表由本轮实跑聚合两份 pixel.csv 重算：
`out/_p2v-r5b-k3diff/pixel.csv` vs `out/_p2v-r6c-k3diff/pixel.csv`，场景帧区间沿用 §16）：

| 场景 | 帧区间 | r5b p8 | r6 p8 | Δ |
|---|---|---|---|---|
| opener | 0–155 | 0.027971 | 0.026369 | −0.001601 |
| prscroll | 156–251 | 0.056795 | 0.054418 | −0.002378 |
| hard | 252–278 | 0.006680 | 0.005754 | −0.000926 |
| summarize | 279–368 | 0.032705 | 0.032049 | −0.000656 |
| useskill | 369–482 | 0.020257 | 0.020573 | +0.000316 |
| fcode | 483–554 | 0.040723 | 0.038810 | −0.001913 |
| fflow (s07) | 555–629 | 0.036634 | 0.036802 | +0.000168 |
| fdisc (s08) | 630–689 | 0.042050 | 0.040582 | −0.001468 |
| fcredits | 690–767 | 0.034922 | 0.034172 | −0.000750 |
| cta | 768–899 | 0.033666 | 0.033574 | −0.000092 |
| **TOTAL** | 0–899 | **0.034022** | **0.033122** | **−0.000900** |

8 场景下降、2 场景噪声级上升（useskill +0.0003 / fflow +0.0002，均 <0.0005 门限）。
fflow 的换行收益未兑现——受 20.3 taffy leaf 高度不重算阻塞，结构性对齐抵消后持平。

构建/测试验证：
- `cargo build --release`：0 error ✓
- `cargo test -p opencat-core --lib`：**709 passed, 0 failed**（r5 的 708 + whitespace 单测）✓

### 20.6 下一步建议

1. **taffy leaf re-measure**（20.3，引擎通用项，本轮最高预期收益）：修 `measure_node`
   MinContent 分支 + flex 收缩后 text leaf 二次 measure，兑现 fflow diffcard 换行（~42px 卡高差）。
2. s02 prscroll 尾段 f236–251（worst f242 p8 0.1008 所在，连续多轮沿承）。
3. `.ttext` `line-height:1` 语义单轮验证（现 XML 的 leading-none 全部为无解析路径的 no-op，
   参考语义确为 1，但牵动全场景标题基线，须单独一轮）。

## 21. Round 7（2026-10-09，对齐工程师-r7；r7b 评审勘误后终版）— taffy MinContent min-content 修复 + fflow dl 换行兑现 — 本地同管线 0.033122 → **0.032800**（−0.000322），9 场景 bit 级零回归

### 21.1 误差源选择

§20.6 #1「taffy leaf re-measure」（§20.3 实锤的引擎缺口）直接落地：`measure_node` 对
`AvailableSpace::MinContent` 返回 INFINITY → flex item 的 automatic minimum size =
max-content → flex-shrink 失效、收缩后 text leaf 高度不重算（fflow diffcard 卡高 119 vs
REF 194 的根因）。

### 21.2 引擎改动（通用，无示例 hack）

`crates/opencat-core/src/layout/mod.rs`：
- `measure_node` 的 `AvailableSpace::MinContent` 分支改为返回 `text_min_content_width()`
  （新 helper：按空白分词逐词 measure 取最大，作为 wrap 预算交给 cosmic-text；最终
  min-content 由 cosmic-text 的 Unicode 断行裁决——hyphen 后即断点，与 Chrome 的
  min-content contribution 语义一致，单测实证 "--paginate" → "paginate"）。
- taffy 消费链（源码核验）：flexbox.rs:806-820 用 `Size::MIN_CONTENT` measure 求
  automatic min size → 修复后收缩钳制恢复真 min-content；final sizing（flexbox.rs:1913
  `calculate_flex_item` 传 `item.target_size`）→ leaf.rs:97-115 把 known width 转
  Definite available_space → PerformLayout 高度按分配宽度重算。单点修复同时解决收缩与高度。

新单测 2 个：`min_content_measurement_reports_longest_word`（min/max-content 分离 + 最长词
对照）、`shrinking_flex_row_remeasures_wrapped_text_leaf_height`（溢出 flex 行收缩后 wrap
文本两行高，复刻 diffcard 形态）。

### 21.3 XML 改动

- auto336/339/342（fflow diffcard dl 文本）加 `whitespace-normal`——恢复 r6 因 §20.3
  缺陷回滚的 3 处（r6 已备好引擎类，本轮修复后兑现）。
- ~~auto334 补 `mt-[18px]`~~ **（r7b 勘误：已删除）**。初版误读 feature-flow.html:99
  `.dl:first-of-type{margin-top:18px}` 为生效规则；`:first-of-type` 按标签类型计数，
  diffcard 第一个 div 是 `.dch`（feature-flow.html:19），三个 `.dl` 均非首个 div →
  该选择器不匹配任何元素，参考渲染 dl1 margin-top 实为 0（headless Chrome 实测
  matches=false + computed 0px，评审实证）。删除后复测 r7b：fflow 0.032303 → 0.032944
  （+0.000642——18px 此前是方向碰巧的补偿），TOTAL 0.032747 → 0.032800，仍优于 r6
  0.033122。教训：**「源码存在规则」≠「规则生效」，伪类选择器须按参考 DOM 推导匹配面**。
  对照：auto340 的 `mb-[22px]`（`.dl:last-of-type`）有效——diffcard 最后一个 div 恰为
  第三个 `.dl`，匹配成立，保留。

### 21.4 测量（本轮实际执行）

`target/release/opencat render examples/pr-to-video-launch/pr-to-video-launch.xml
out/pr-to-video-launch.mp4` + `python3 tools/k3diff.py
/home/solaren/Projects/hyperframes-launches/pr-to-video-launch/renders/pr-to-video-launch_2026-10-06_swgl.mp4
out/pr-to-video-launch.mp4 --out out/_p2v-r7-k3diff`（注意：参考 mp4 的 repo 相对路径
renders/… 已不存在，须用绝对路径；k3diff --dump 只对完整 mp4 test 生效）：

| 指标 | r6 基线（_p2v-r6c） | r7 含 mt18（_p2v-r7） | **r7b 终版（_p2v-r7b）** | Δ(r6→r7b) |
|---|---|---|---|---|
| avg_p8 | 0.033122 | 0.032747 | **0.032800** | −0.000322 |
| worst_p8 | 0.1008（f242） | 0.1008 | 0.1008（f242 沿承） | 记录项 |

per-scene（同管线聚合 pixel.csv，帧区间沿用 §20.5）：**fflow 0.036802 → 0.032944
（−0.003858，−10.5%）**，其余 9 场景 Δ = +0.000000（bit 级零变化）。
按 §19 跨管道固定差 ~0.00084 推算权威 ≈ 0.0320（将优于 r6 权威 0.033122）。

验证（r7b 复跑）：`cargo build --release` ✓；`cargo test -p opencat-core --lib`
**711 passed, 0 failed**（r6 的 709 + 本轮 2）✓。

### 21.5 f609 dump 归因（out/_p2v-r7-f609dump/，下轮入口）

dl 换行部分兑现（r7 初版 dump，diffcard-combo.png）：dl2/dl3 均两行，但**字符级断点与
REF 不一致**——REF dl1 `gh pr view --json`/`files` 两行 vs TEST dl1 单行（TEST 收缩
451 flow 下内容 ~430px 单行放得下）；REF dl2 第二行首为 `…/files` vs TEST 第一行末为
`…/`。f609 p8 0.0510→0.0446（r7 含 mt18 时）。剩余残差两大主线：

1. **REF f609 多元素处于动画中间态**（像素实测，非猜测）：REF fc2 黑缩略图宽 68 画布 vs
   TEST 55（终值 108 flow × 0.5124 = 55.3）→ REF fc2 处 scale ≈ 1.17–1.2（back.out(1.6)
   峰值仅 1.09，时间映射未闭合）；REF vcard 含 vring 脉冲中环（宽 176 画布 vs TEST 143）。
   → 下轮先核 **s07 的 inner timeline 时间映射**（composition duration 2.5s vs inner
   事件至 6.1s，须查 timeScale/loop；f609 的 vring 态表明 t > 4.1s）。
2. **diffcard flex 收缩量差异**：REF 375 flow（画布 192/0.5124，dl1 两行、断点
   `--json` 后）vs TEST 451 flow（dl1 单行）。TEST 的收缩分配 = 标准加权（diffcard 113 +
   vcard 85 ≈ 194 溢出按 560:420）。Chrome 侧收缩合计 ~325 > 几何溢出 194/207，单一
   flexbox 模型无法闭合——疑似与 #1 的同帧动画态（.flow scale 或子项 transform）耦合，
   先解 #1 再回看。

### 21.6 下一步建议

1. s07 时间线映射（21.5 #1）：读参考 index/compositions 的时间重映射（timeScale、
   loop、场景切换偏移），用 vring/pfill/fc stagger 多锚点联立解 f609 的真实 t。
2. s02 prscroll 尾段 f235–251（worst f242 0.1008 沿承）。
3. `.ttext` line-height:1 语义单轮验证（§20.4 沿承）。

## 22. Round 7 归档（2026-10-09，工作流权威测量）— avg_p8 **0.032800**，kept ✅（历史最优 0.03280 @r7）

> §21 是本轮工程师的本地测量与归因过程（含 r7b 评审勘误）；本节是工作流管道的权威测量
> 归档。权威数字逐位取自 `out/_p2v-r7b-k3diff/summary.json`（r7b 勘误后终版渲染，
> summary.json avg_p8/mae_mean 与工作流下发值逐位一致，本轮实跑核对）：工作流
> 0.033122 → 0.032800（−0.000322）、本地同管线 0.033122 → 0.032800（同一产物），
> 干净改善且逐场景零回归。以本节为轮次结论。

### 22.1 权威测量

| 指标 | 值 |
|---|---|
| **avg_p8** | **0.032800**（历史最优，此前 0.033122 @r6；链路 r0 0.037816 → r1 0.035777 → r2 0.034439 → r3 0.034135 → r4 0.033318 → r5 0.033184 → r6 0.033122 → r7 0.032800） |
| mae_mean | 2.895897 |
| worst_p8 | 0.100753（仅记录，不作门；f242，s02 pr-scroll blur 态簇最差帧沿承 §18.4/§20.5） |
| kept | true（采纳为新的历史最优） |
| per-act | actStats 为空——工作流本轮未提供 per-act 拆分；per-scene 见 §21.4 表（同管线
  `out/_p2v-r6c-k3diff` vs `out/_p2v-r7b-k3diff`，逐帧聚合实跑复现：fflow
  0.036802→0.032944（−0.003858，−10.5%），其余 9 场景 bit 级零变化） |

### 22.2 改动内容与参考出处

| 改动 | 参考出处 |
|---|---|
| 引擎通用：`measure_node` 的 `AvailableSpace::MinContent` 分支由返回 INFINITY 改为
  `text_min_content_width()`（按空白分词逐词 measure 取最大作 wrap 预算；最终
  min-content 由 cosmic-text Unicode 断行裁决，hyphen 后即断点，与 Chrome 一致）。
  单点闭合 flex 收缩与高度重算：taffy flexbox.rs:806-820 以 `Size::MIN_CONTENT`
  measure 求 automatic minimum size（css-flexbox #min-size-auto），flexbox.rs:1913
  final sizing 传 `item.target_size` → leaf.rs:97-115 转 Definite available_space。
  新增 2 单测：`min_content_measurement_reports_longest_word`（min/max-content 分离 +
  cosmic-text 断词对照）、`shrinking_flex_row_remeasures_wrapped_text_leaf_height`
  （溢出 flex 行收缩后 wrap 文本两行高） | CSS Sizing §min-content contribution +
  css-flexbox §4.5 automatic minimum size；根因即 §20.3 实锤（fflow diffcard 卡高
  119 vs REF 194、dl2 未收缩 w=368>容器 321） |
| XML：auto336/339/342（fflow diffcard dl 文本）加 `whitespace-normal`——恢复 r6 因
  §20.3 缺陷回滚的 3 处（`whitespace-normal` 引擎类 r6 已备好，本轮缺陷修复后兑现） | ref feature-flow.html:22-24 `.dl` 文本在收缩卡片内按 CSS 默认
  `white-space:normal` 换行 |
| ~~XML：auto334 补 `mt-[18px]`~~ **（r7 初版误加，r7b 勘误删除）**：`.dl:first-of-type`
  按标签类型计数，diffcard 首个 div 是 `.dch`（feature-flow.html:19）→ 选择器无匹配，
  参考 dl1 margin-top 实为 0（headless Chrome 实测 matches=false + computed 0px）。
  删除后复测 fflow +0.000642——18px 是方向碰巧的补偿。对照：auto340 `mb-[22px]`
  （`.dl:last-of-type`）有效保留（diffcard 末个 div 恰为第三个 `.dl`） | ref feature-flow.html:99 `.dl:first-of-type{margin-top:18px}` 为
  死规则（详见 §21.3 勘误与教训） |
| 验证（本轮实际执行）：`cargo build --release` ✓；`cargo test -p opencat-core --lib`
  **711 passed, 0 failed**（r6 的 709 + 新增 2）✓ | — |

### 22.3 负结果 / 坑（本轮实测，详 §21.5）

- **`:first-of-type` 伪类按标签计数不按类**：diffcard 内三个 `.dl` 永不匹配
  `.dl:first-of-type`。教训：**「源码存在规则」≠「规则生效」**——伪类选择器须按参考
  DOM 推导匹配面；被删的 mt-18px 曾让数字变好（0.032747 vs 0.032800），属方向碰巧的
  补偿，宁劣勿错。
- **REF f609 多元素处于动画中间态**（像素实测）：REF fc2 黑缩略图 68 画布 vs TEST 55
  （终值 108 flow × 0.5124）→ REF fc2 scale ≈ 1.17–1.2，超出 back.out(1.6) 峰值 1.09，
  时间映射未闭合；vcard vring 脉冲中环（176 vs 143 画布）表明 t > 4.1s。
- **diffcard flex 收缩量差异**：REF 375 flow vs TEST 451 flow；TEST 分配 = 标准加权
  （113+85 ≈ 溢出 194 按 560:420），Chrome 侧收缩合计 ~325 > 几何溢出 194/207，单一
  flexbox 模型无法闭合——疑与同帧动画态耦合。
- **dl 断点未全对齐**：dl2/dl3 两行但字符级断点两边不一致（REF `…/files` 第二行行首 vs
  TEST `…/` 第一行行尾）；dl1 REF 两行 vs TEST 单行（随收缩量差异解决）。
- 工具坑：参考 mp4 的 repo 相对路径 `renders/…` 已失效，k3diff 须用绝对路径
  `/home/solaren/Projects/hyperframes-launches/pr-to-video-launch/renders/…`；
  `--dump` 只对完整 mp4 test 生效（单 PNG 输入不落 dump 帧）。
- 沿承未修：s02 prscroll blur 态 ref 侧额外平滑、Archivo Black kerning ~4%、steps(1)
  半程跳变、`.ttext` line-height:1 语义（§16.4/§20.4/§20.6 清单）。

### 22.4 下一步建议（按潜在收益排序）

1. **s07 fflow 时间线映射**（§21.5 #1）：composition duration 2.5s vs inner 事件至
   6.1s，须查参考 index/compositions 的时间重映射（timeScale/loop/场景切换偏移），用
   vring/pfill/fc stagger 多锚点联立解 f609 的真实 t；随后回看 diffcard 收缩量差异。
2. s02 prscroll 尾段 f235–251（worst f242 p8 0.1008 沿承，连续多轮）。
3. `.ttext` line-height:1 语义单轮验证（现 XML 的 leading-none 全部为无解析路径的
   no-op，牵动全场景标题基线，须单独一轮）。

## 23. Round 8（2026-10-09，对齐工程师-r8）— s07「时间线映射」假设证伪 → 真因静态布局四缺陷（content-box / ptime min-width / knob top / knob left 单位）+ 引擎 min-w 补全 — 本地同管线 0.032800 → **0.032468**（−0.000332），6 场景 bit 级零回归

### 23.1 误差源选择与归因（§22.4 #1 的落地与证伪）

按 §22.4 首选「s07 fflow 时间线映射」（§21.5 的 fc2 scale 1.17–1.2 超出 back.out(1.6) 峰值、
vring 176 vs 143）开工。帧级重测（`/tmp/p2v_r8_frames/`，REF f609 vs r7 TEST f609）：

- **pfill 锚点证伪时间映射假设**：理论 scaleX(4.608 inner) = 0.3704；r7 TEST 实测
  (785−428)/967 = 0.3692 ✓ 映射正确；REF 0.360 是 knob 白圆盖住填充末端的测量伪差。
  ptime 文本两侧同为 0:11 ✓。
- **真因 1 — ptime 缺 min-width**：ref `.ptime{min-width:9cqw}`（feature-flow.html:87 等 4
  场景同构）未端口 → flex-none 的 ptime 收到文本宽 ~128px → ptrack（flex-1）被拉长
  44.8px（实测 track REF [426,1349]=923 vs TEST [428,1395]=967）。
- **真因 2 — pknob top-[540px]**：ref `.pknob{top:50%}` 相对 6.53px 高的 ptrack = 3.265px；
  端口把 50% 错当屏幕半高 540px → knob 落到 pwin（overflow:hidden）外全片不可见。
- **真因 3 — pknob left 无单位数字**：ref 为 `left:'22%'/'42%'`（百分比，GSAP 无单位→px
  语义不同）；XML `left: 22` 被当 22px。引擎 `inset_left: Option<f32>` 仅 px
  （mutations.rs:40），无百分比 → track 宽由布局规则推导 923.6px（pwin 1228.8 − pctrl px
  2×28.8 − pplay 28.8 − gap 2×23 − ptime min-width 172.8），`% × 923.6` 落 px 表达式。
- **真因 4 — fc/vcard content-box**（本轮最大单项）：ref 为原生 CSS content-box，
  `.fc{width:134px;padding:12px;border:1px}` 的 134 是 content 宽 → 卡总占宽 160 flow、
  `.fct{width:100%}` = 134 flow。端口按 Tailwind border-box 把 134 当总宽 → fct 只剩
  108 flow。像素闭合：REF fc2 黑缩略图 68 = 134×0.5124 ✓ vs TEST 55 = 108×0.5124 ✓；
  且 flow 行基准总宽 content-box 下溢出 327 flow ≈ §21.5「Chrome 收缩合计 ~325」之谜、
  border-box 下溢出 197 ≈ TEST 收缩 194 ✓——**§21.5 #2 的 diffcard 收缩差异同根**
  （vcard 同理 420×260 content-box + 1.5px border → 总 423×263）。

### 23.2 改动内容

**XML（examples/pr-to-video-launch/pr-to-video-launch.xml，全部有参考行号注释）**
- 4 场景 ptime（fc/ff/fd/cr）补 `min-w-[172.8px]`（ref 各 `.ptime{min-width:9cqw}`）。
- 4 处 pknob `top-[540px]` → `top-[3.265px]`（50% × 6.53px ptrack）。
- 8 处 pknob left（set 5/22/66/86 + to 11/42/80/98）→ `(N / 100) * 923.6` 表达式，
  注释引用 feature-*.html 行号与 track 推导。
- 5 张 `#ff-fc1..5` `w-[134px]` → `w-[160px]`（134 content + 2×12 pad + 2×1 border）；
  `ff-video` `w-[420px] h-[260px]` → `w-[423px] h-[263px]`。

**引擎通用补全（非示例 hack）**
- `min-w-[Npx]` / `min-w-full` Tailwind 类（此前引擎无任何 min-width 支持）：
  `style.rs` NodeStyle.min_width + `tailwind.rs` 解析（镜像 min-h 块）+
  `resolve/style.rs` ComputedLayoutStyle.min_width + `resolve.rs` 流转 +
  `fingerprint.rs` hash + `layout/mod.rs` taffy `min_size.width`。
  **坑**：min_size 初版放在 `taffy_style_for_element` 的 Div 内联分支 → text leaf
  （走 `base_style` 共享臂）不生效；已移入 `base_style` 全 kind 共享。
- 新单测 2 个：`min_w_bracket_and_full_classes_set_min_width`（tailwind.rs，bracket/full/
  未知值 no-op）、`min_width_class_expands_flex_child_past_content_size`（layout/mod.rs，
  min-width 撑开 flex-none 文本叶 172.8px + flex-1 轨道收缩到余量，复刻 ptime/ptrack 形态）。

### 23.3 测量（本轮实际执行）

`target/release/opencat render …` + `python3 tools/k3diff.py
/home/solaren/Projects/hyperframes-launches/pr-to-video-launch/renders/pr-to-video-launch_2026-10-06_swgl.mp4
out/pr-to-video-launch.mp4 --out out/_p2v-r8c-k3diff`（迭代中间态 _p2v-r8-k3diff
0.032742 / _p2v-r8b-k3diff 0.032522，终态 r8c）：

| 指标 | r7 基线 | r8 终版 | Δ |
|---|---|---|---|
| avg_p8 | 0.032800 | **0.032468** | −0.000332 |
| mae_mean | 2.895897 | **2.844922** | −0.051 |
| worst_p8 | 0.100753（f242 沿承） | 0.100753（f242 沿承） | 记录项 |

per-scene（同管线 `out/hf-pr-to-video-launch-r7-diff/pixel.csv` vs
`out/_p2v-r8c-k3diff/pixel.csv`，帧区间沿用 §20.5）：**fflow 0.032944 → 0.030061
（−0.002883，−8.8%）**、**fcredits 0.034172 → 0.033254（−0.000919）**、fcode −0.000120、
fdisc −0.000033；opener/prscroll/hard/summarize/useskill/cta 六场景 bit 级零变化（与 r7
逐帧完全相同的 625/900 帧覆盖全部六场景）。

f609 三锚点复核（r8c）：fc2 黑缩略图宽 REF 68 = TEST 68 ✓；knob REF 760-777 = TEST
760-778 ✓；ptime 文本位置一致（1417 vs 1420）。

构建/测试验证：`cargo build --release` ✓；`cargo test -p opencat-core --lib`
**713 passed, 0 failed**（r7 的 711 + 本轮 2）✓。

### 23.4 负结果 / 坑（本轮实测）

- **§22.4 #1「s07 时间线映射未闭合」假设证伪**：pfill/ptime 双锚点证明 /2.56 映射本就
  正确；「fc2 scale 1.17–1.2 超出 back.out 峰值」实为 fc2 黑缩略图 content-box 宽度差
  （134 vs 108 flow，比值 1.2407 ≈「scale 1.23」的假象）；「vring 176 vs 143」同族。
  **教训：锚点元素本身的静态尺寸错误会伪装成相位/缓动错误，先排除布局再谈时间。**
- **`top-[540px]` 全局替换误伤**：XML 共 7 处该类，其中 3 处（auto139/hr-txt/sm-still）
  是「top:50% 屏幕半高」的**正确**转换（垂直居中锚点），已回滚；只有 4 处 pknob 的
  50% 是相对 ptrack 的。同类字符串必须逐处核对语义再替换。
- **CSS 注释误入 class 属性**：fc1/vcard 首次替换把 `/* … */` 写进 class → 会污染
  Tailwind 类解析，已清除并改用 XML 注释（标签外）。
- **min_size 只加 Div 分支则 text leaf 不生效**：`taffy_style_for_element` 按 kind 分臂，
  Text/Bitmap/Lottie 走 `base_style`；共享属性必须放 base_style。
- XML 576 行附近脚本裸 `<`（`i < N`）使 minidom 报 not well-formed——历史既有写法，
  opencat 解析器容忍，非本轮引入（r7 同构渲染成功），勿「顺手修复」。
- 沿承未修：s02 prscroll 尾段（worst f242 0.1008 连续多轮）、Archivo Black kerning ~4%、
  steps(1) 半程跳变、`.ttext` line-height:1 语义（§20.4/§22.3 清单）。

### 23.5 下一步建议（按潜在收益排序）

1. **s07 flow 行收缩分配残差**（本轮 f609 复核：fc2 起点 REF 858 vs TEST 873，差 15px）：
   content-box 修复后 flow 行基准已对（溢出 327），但 diffcard/fc/vcard 的 shrink 分配
   仍有差——vcard 现在 423 flow（总宽变化）会改变分配权重，需重解 §21.5 #2 的剩余项。
2. **s02 prscroll 尾段 f235–251**（worst f242 0.1008 沿承）。
3. **fdisc（0.0405，现最大非 blur 场景）**：f660 knob/ptime 已对齐，剩余在对话卡区，
   值得逐块归因。
4. `.ttext` line-height:1 语义单轮验证（沿承）。

## 24. Round 8 归档（2026-10-09，工作流权威测量）— avg_p8 **0.032468**，kept ✅（历史最优 0.03247 @r8）

> §23 是本轮工程师的本地测量与归因过程（含时间线映射假设的证伪）；本节是工作流管道的
> 权威测量归档。权威数字逐位取自工作流下发值，并与本轮实跑
> `out/_p2v-r8c-k3diff/summary.json` 核对逐位一致（avg_p8 0.03246836580504115、
> mae_mean 2.844922153635117、worst f242 0.10075327932098765）——**本轮权威 = 本地
> 同管线，无跨管道固定差**（延续 r7，§17.3 的 ~0.00084 固定差自 r7 起不再出现）。
> 以本节为轮次结论。

### 24.1 权威测量

| 指标 | 值 |
|---|---|
| **avg_p8** | **0.032468**（历史最优，此前 0.032800 @r7；链路 r0 0.037816 → r1 0.035777 → r2 0.034439 → r3 0.034135 → r4 0.033318 → r5 0.033184 → r6 0.033122 → r7 0.032800 → r8 0.032468） |
| mae_mean | 2.844922（r7 2.895897 → −0.051） |
| worst_p8 | 0.100753（仅记录，不作门；f242，s02 pr-scroll blur 态簇最差帧，连续第 5 轮沿承） |
| kept | true（采纳为新的历史最优） |
| per-act | actStats 为空——工作流本轮未提供 per-act 拆分；per-scene 见 §23.3 表（同管线
  `out/hf-pr-to-video-launch-r7-diff/pixel.csv` vs `out/_p2v-r8c-k3diff/pixel.csv`，逐帧
  聚合实跑复现：fflow 0.032944→0.030061（−0.002883，−8.8%）、fcredits 0.034172→0.033254
  （−0.000919）、fcode −0.000120、fdisc −0.000033；opener/prscroll/hard/summarize/
  useskill/cta 六场景 bit 级零变化，覆盖 625/900 个逐帧相同帧） |

### 24.2 改动内容与参考出处

| 改动 | 参考出处 |
|---|---|
| XML：4 场景 ptime（fc/ff/fd/cr）补 `min-w-[172.8px]`——flex-none 文本列此前收到内容宽
  ~128px，ptrack（flex-1）被拉长 44.8px（track 实测 REF [426,1349]=923 vs TEST
  [428,1395]=967），进度条填充端与 knob 比例失配 | ref feature-code.html:75 /
  feature-flow.html:87 / feature-discussion.html:92 / feature-credits.html:73
  `.ptime{min-width:9cqw;flex:0 0 auto}`（9cqw×19.2=172.8px） |
| XML：4 处 pknob `top-[540px]` → `top-[3.265px]`——50% 相对 6.53px 高的 ptrack
  （=3.265px），端口误当屏幕半高 540px → knob 落到 pwin（overflow:hidden）外全片不可见
  （f609 REF knob 760-777 vs r7 TEST 缺失） | ref feature-flow.html:86 `.pknob{top:50%}`
  等 4 场景同构；ptrack 高 `.34cqw`=6.53px（feature-flow.html:84） |
| XML：8 处 pknob left（set 5/22/66/86、to 11/42/80/98）→ `(N / 100) * 923.6` px 表达式——
  ref 为 `left:'N%'`，引擎 `inset_left: Option<f32>` 仅 px（mutations.rs:40）无百分比；
  track=923.6px 由布局规则推导（pwin 1228.8 − pctrl px 2×28.8 − pplay 28.8 − gap 2×23 −
  ptime min-width 172.8）。修复后 f609 knob REF 760-777 = TEST 760-778、f660 knob
  1086-1103 两侧一致 | ref feature-code.html:104/123、feature-flow.html:151/172、
  feature-discussion.html:152/171、feature-credits.html:108/131 |
| XML：5 张 `#ff-fc1..5` `w-[134px]` → `w-[160px]`、`ff-video` `w-[420px] h-[260px]` →
  `w-[423px] h-[263px]`——ref 原生 CSS content-box，`.fc{width:134px;padding:12px;
  border:1px}` 的 134 是 content 宽（卡总占宽 160 flow、`.fct{width:100%}`=134 flow），
  端口按 Tailwind border-box 落 134 → 缩略图只剩 108 flow。像素闭合：fc2 黑缩略图
  REF 68 = 134×0.5124 vs TEST 55 = 108×0.5124（修复后 68=68）；flow 行基准总宽
  content-box 溢出 327 flow ≈ §21.5「Chrome 收缩 ~325」、border-box 溢出 197 ≈ TEST
  收缩 194——§21.5 #2 与「fc2 scale 1.23」假象同根 | ref feature-flow.html:106-108
  `.strip/.fc`、:129 `.vcard{width:420px;height:260px;border:1.5px solid #CC785C}` |
| 引擎通用：`min-w-[Npx]` / `min-w-full` Tailwind 类（此前无任何 min-width 支持）——
  `style.rs` NodeStyle.min_width + `tailwind.rs` 解析（镜像 min-h 块）+
  `resolve/style.rs` ComputedLayoutStyle.min_width + `resolve.rs:1144` 流转 +
  `fingerprint.rs` hash + `layout/mod.rs` taffy `min_size.width`（置于 `base_style`
  全 kind 共享；初版误放 Div 内联分支致 text leaf 不生效）。新增 2 单测：
  `min_w_bracket_and_full_classes_set_min_width`、
  `min_width_class_expands_flex_child_past_content_size`（min-width 撑开 flex-none 文本
  叶 172.8px + flex-1 轨道收缩到余量，复刻 ptime/ptrack 形态） | CSS min-width / Tailwind
  min-w-* 工具类；本轮 4 场景 `.ptime` 为首个消费点（CSS 惯用 min-width 保 flex 列宽） |
| 验证（本轮实际执行）：`cargo build --release` ✓；`cargo test -p opencat-core --lib`
  **713 passed, 0 failed**（r7 的 711 + 新增 2）✓；f609 三锚点复核 fc2 68=68、knob
  760-777≈760-778、ptime 文本位置一致 | — |

### 24.3 负结果 / 坑（本轮实测，详 §23.4）

- **§22.4 #1「s07 时间线映射未闭合」假设证伪**：pfill（0.3692 vs 理论 0.3704）/ptime
  （两侧 0:11）双锚点证明 /2.56 映射本就正确；「fc2 scale 1.17–1.2 超出 back.out(1.6)
  峰值 1.09」是 content-box 缩略图宽度差的假象（134/108 = 1.2407 ≈「scale 1.23」）。
  **教训：锚点元素本身的静态尺寸错误会伪装成相位/缓动错误——先排除布局再谈时间。**
- **同类字符串全局替换须逐处核对语义**：`top-[540px]` 共 7 处，4 处 pknob 是错转换、
  3 处（auto139/hr-txt/sm-still）是「top:50% 屏幕半高」的正确转换，替换后已回滚这 3 处。
- **CSS 注释不能写进 class 属性**（会污染 Tailwind 类解析）；参考出处注释一律用 XML
  注释（标签外）。
- **taffy 共享样式属性必须放 `base_style`**：`taffy_style_for_element` 按 ElementKind
  分臂，Text/Bitmap/Lottie 走 `base_style(element)`，只改 Div 内联分支则 text leaf 不生效
  （本轮单测当场拦截：ptime 宽 91 而非 172.8）。
- minidom 对 XML 576 行附近脚本裸 `<` 报 not well-formed——opencat 解析器容忍的历史
  既有写法，非本轮引入，勿「顺手修复」。
- 沿承未修：s02 prscroll blur 态（worst f242 0.1008 连续 5 轮）、Archivo Black kerning
  ~4%、steps(1) 半程跳变、`.ttext` line-height:1 语义（§20.4/§22.3 清单）。

### 24.4 下一步建议（按潜在收益排序）

1. **s07 flow 行收缩分配残差**（本轮 f609 复核：fc2 起点 REF 858 vs TEST 873，差 15px）：
   content-box 修复后 flow 行基准已对（溢出 327），但 diffcard/fc/vcard 的 shrink 分配
   仍有差——vcard 改 423 flow 后分配权重已变，需按新基准重解 §21.5 #2 剩余项。
2. **s02 prscroll 尾段 f235–251**（worst f242 0.1008 沿承，连续 5 轮）。
3. **fdisc（0.0405，现为最大非 blur 场景）**：f660 knob/ptime 已对齐，剩余在对话卡区，
   值得逐块归因。
4. `.ttext` line-height:1 语义单轮验证（§20.4/§22.3 沿承）。

## 25. Round 9（2026-10-09，对齐工程师-r9）— cta-url opacity bug + URL 高亮 flash 端口 + 引擎 Steps 语义修正为 GSAP 半程跳 — 本地同管线 0.032468 → 0.032487（+0.0000189，编码噪声级；结构修复被噪声淹没）— 另证伪 §24.4 #4

### 25.1 误差源选择与归因

按场景质量（r8 pixel.csv 实聚合）：prscroll 5.22（17.9%）> cta 4.43（15.2%）> opener 4.11
（14.1%）> 其余各 8–10%。逐帧 >0.05 簇分布：prscroll 61 帧（f191–251，滚动 whip 段）、
fdisc 16（f674–689 静止）、cta 14（f821–827 动画 + f854–860 静止）、opener 18、summarize 7。

- **prscroll whip 段（f191–251）证伪「滚动相位差」**：f195/210/225/240 垂直互相关实测
  test 相对 ref 位移仅 −1～−4px（/tmp/p2v_r9_frames/ 互相关脚本），曲线对齐；弥散差主源
  仍是 §15.3 判定的 ref 编码平滑（不可从源码推导，不再追）。
- **cta f857 目检发现结构性差异**：ref 已显示 `use pr-to-video
  https://github.com/your-org/your-repo/pull/42`（带橙色高亮残留），test 只有
  `use pr-to-video`。→ cta-url opacity bug（§25.2）。
- **§24.4 #4「leading-none 无解析路径」证伪（重要负结果）**：`leading-none/tight/snug/
  normal/relaxed/loose` 自始由 `build.rs:1035-1040` 生成 `ExactClassAction::LineHeight`
  规则（EXACT_CLASS_RULES 经 `include!(OUT_DIR/tailwind_jsonl_rules.rs)` 注入
  tailwind.rs）——历代（§20.4 起）只 grep 了 tailwind.rs 未查 build.rs。本轮实验链：
  删 hr-t/auto183 的 leading-none → hard/summarize 恶化 +0.92 mass（f265 文本行从
  ref 451/engine 453 位移到 498 = 1.5 倍行距 half-leading 45px）；恢复后 bit 级回 r8。
  **XML 现有 13 处 leading-none 全部正确**（hr-t/auto183 虽然参考无 line-height，但引擎
  继承默认 1.5 ≠ Chrome normal≈1.0，leading-none 恰是正确逼近）。本轮引擎 named
  leading 补全已实装又回滚（与 build.rs 规则重复，宁缺勿冗）。

### 25.2 改动内容

**XML（examples/pr-to-video-launch/pr-to-video-launch.xml，cta 段脚本）**
- 删除 `T.set('cta-url',{opacity:0},0)`：该行设 0 后无任何恢复路径（T.set 第二参数 vars
  只对 id 目标生效，843 行函数目标被忽略），URL 文本实际已插入但全程不可见——caret 被
  不可见文本推到右侧（f857 实测 test caret x 与 ref 差 33px ≈ 已占位）。初始文本为空已
  保证 t<2.85 零宽，该行纯属多余且有害。修复后 f855–860 p8 −0.005/帧（0.0556→0.0504）。
- 补 URL 高亮 flash：`T.fromTo('cta-url',{backgroundColor:'rgba(217,119,87,0.36)'},
  {backgroundColor:'rgba(217,119,87,0)',duration:.5,ease:'power2.out'},O10+2.85)`
  （ref cta.html:94；引擎 backgroundColor 动画属性已注册 color.js:27）。
- ref cta.html:93 `tl.add(()=>{url.textContent=URL},2.85)` 的既有端口（XML 843 行
  `T.set({set:...},{opacity:1},O10+2.85)` 惯用法，与 748/760/780/806 行 ptime 同构）保留。

**引擎通用（crates/opencat-core/src/parse/easing.rs）**
- `Easing::Steps(n)` 语义修正：旧实现段尾跳 `floor(t·n)/n`（t≥1 特判 1），与 GSAP 不符。
  依据 gsap@3.12.5 min 源码反编译（k3-promo/assets/gsap.min.js：`SteppedEase.config：
  ratio = (floor((N+1)·clamp(p,0,.99999999)) + 0)/N`）改为 **`floor((N+1)·t)/N`**——
  steps(1) 即 floor(2p)，**在补间中点跳变**。ref 逐帧验证（pr-scroll/cta/opener caret）：
  段 0 前半 ON 后半 OFF、yoyo 段反向同理，全部吻合；旧语义段尾跳与 ref 全程错半段。
  f32 精度坑：JS double 下 0.99999999≠1，f32 舍入为 1.0，故 t≥1 需显式特判返回 1.0，
  不得依赖 clamp 上限。
- 单测 `steps_easing_quantizes` 更新为 GSAP 语义（steps(4) 首跳 1/5=0.2；steps(1) 半程
  跳断言 apply(0.49)=0 / apply(0.5)=1）。另加临时探针测试后已删除。
- 影响面：XML 全部 10 处 steps(1)（opener/prscroll×3/useskill/summarize 隐含/fcode/
  fdisc/fcredits/cta×2 caret 闪烁）相位整体对齐 GSAP。XML caret 参数本身与参考同构
  （opener.html:125=.4/1、use-skill.html:74=.5/1+79 行 set 2.0 已端口 723 行、
  feature-code.html:114=cardTitle .4/9、discussion:166=.35/5、credits 同族）——无错构。

### 25.3 测量（本轮实际执行）

`cargo build --release` ✓；`cargo test -p opencat-core --lib` **713 passed, 0 failed** ✓。
渲染 + `python3 tools/k3diff.py <ref> out/pr-to-video-launch.mp4 --out out/_p2v-r9c-k3diff`：

| 指标 | r8 | r9c | Δ |
|---|---|---|---|
| avg_p8 | 0.032468 | 0.032487 | +0.0000189（噪声级） |
| mae_mean | 2.844922 | 2.841832 | −0.0031 |
| worst_p8 | 0.100753（f242） | **0.100377（f242）** | 记录项（f242 在 prs-cr2 caret
  闪烁段，steps 半程跳修复使其微降 −0.00038） |

迭代中间态：_p2v-r9-k3diff（含错误的 leading 删除）0.033489 弃用；_p2v-r9-expA-k3diff
（仅回滚验证）= r8 bit 级；_p2v-r9b-k3diff（仅 URL 修复）0.0324709。

**编码噪声警示（方法论级负结果）**：steps/URL 修复改变 618 帧的像素输入 → x264 帧间
预测把差异扩散到全片 → per-scene p8 出现 ±0.04 mass 级不可归约抖动。实测：useskill
+0.0448 mass 恶化中，r8 与 r9c 渲染帧直接 diff 仅 35px/帧（≈0.0019 mass，4%）——其余
96% 是编码噪声；opener −0.0090/fcode −0.0071 的改善同样混有噪声。真实收益的结构证据：
cta mass 4.4317→4.4290（URL 段 f855–860 −0.005/帧，caret 相位段 f870–899 文字渲染微差
+0.001/帧，相抵后净 −0.0027）；opener/fcode 的 op-caret/fc-tcaret 相位帧逐帧目检对齐。
**教训：改动画时序类修复后，单场景 p8 对比不可直接归因；应做渲染帧直接 diff
（pre/post mp4 同帧像素差）分离「真实渲染差」与「编码噪声」。**

### 25.4 负结果 / 坑（本轮实测）

- **§24.4 #4 证伪**：named leading 类一直在 build.rs:1035-1040；「leading-none no-op」
  是历代误诊。Tailwind named line-height/颜色/blend 等命名类**都可能在 build.rs 生成表
  里**，查解析支持必须同时 grep tailwind.rs 与 build.rs（OUT_DIR 注入规则）。
- **XML 删类的「顺手修正」风险**：hr-t/auto183 的 leading-none 与参考 CSS 字面不对应，
  但它是引擎继承默认（1.5）→ Chrome normal（≈1.0）差异的正确补偿；按「参考无此属性」
  删除反而恶化 +0.92 mass。参考字面 1:1 之外还要过「引擎默认值 ≠ 浏览器默认值」校验。
- **T.set 函数目标的第二参数 vars 不生效**（只对 id 目标应用）——842 行 opacity:0 设出
  后无人能恢复，URL 全程隐身。同构惯用法（748 行等）没踩坑是因为它们不需要恢复。
- **f32 不能照抄 JS double 的 clamp(p,0,.99999999)**（舍入到 1.0），steps 修复首版单测
  当场拦截（apply(1.0)=1.25）。
- r9 第一版同时动了 named-leading 补全 + 2 处 XML 删类 + 未验证 → 0.033489；靠
  「恢复 → bit 级对照 → 单变量实验」逐步定位，最终保留的只有验证过方向的两项。

### 25.5 下一步建议（按潜在收益排序）

1. **fdisc 静止尾段 f674–689**（0.0405 现最大非 blur 场景，16 帧恒定 0.056，bbox 固定
   (346,68,1574,975)）：对话卡区逐块归因（f452 同法：r8/r9 渲染帧直接 diff 已排除本轮
   变化，残差为原生）。
2. **cta f870–899 的 URL 文字渲染微差**（+0.001/帧×30 帧）：URL 可见后引擎与 Chrome 的
   字形/字距差浮出，量级 2000px>8 阈值/帧；若 mono4 的字距语义可再逼近（沿承 §20.4
   Archivo Black kerning 同族），收益 30 帧。
3. **prscroll f191–251 whip 段**：滚动相位已证伪（dy≤4px），主残差为 ref 编码平滑；
   唯一可试项是 §20.4 沿承的「ref 侧额外平滑」用轻微 blur 逼近——但 §15.3 已判定不追。
4. **方法论**：后续任何动画时序类修复，请用「pre/post 渲染帧直接 diff」报告真实渲染差，
   并以聚合 10 场景净变化（而非单场景）判方向。

## 26. Round 9 归档（2026-10-09，工作流权威测量）— avg_p8 **0.032487**，kept ✅（历史最优保持 0.03247 @r8）

> §25 是本轮工程师的本地测量与归因过程（含 leading-none 证伪实验链、steps 语义考古与
> 编码噪声分离实验）；本节是工作流管道的权威测量归档。权威数字逐位取自工作流下发值，
> 并与本轮实跑 `out/_p2v-r9c-k3diff/summary.json` 核对逐位一致（avg_p8
> 0.032487242262517146、mae_mean 2.8418316215134887），工作流 diff 产物
> `out/hf-pr-to-video-launch-r9-diff/pixel.csv` 与本地 900 帧逐位相同（sum
> 29.238518036 两侧相等）——**本轮权威 = 本地同管线，无跨管道固定差**（延续 r7/r8）。
> 以本节为轮次结论。

### 26.1 权威测量

| 指标 | 值 |
|---|---|
| **avg_p8** | **0.032487**（r8 0.032468 → +0.0000189，编码噪声级；结构修复被噪声淹没。链路 r0 0.037816 → r1 0.035777 → r2 0.034439 → r3 0.034135 → r4 0.033318 → r5 0.033184 → r6 0.033122 → r7 0.032800 → r8 0.032468 → r9 0.032487；**历史最优保持 0.03247 @r8**） |
| mae_mean | 2.841832（r8 2.844922 → −0.0031） |
| worst_p8 | 0.100377（仅记录，不作门；f242，s02 pr-scroll blur 态簇最差帧沿承；**相对 r8 的 0.100753 微降 −0.00038**——f242 在 prs-cr2 caret 闪烁段，steps 半程跳修复的真实效果之一） |
| kept | **true**（工作流判定：两项结构修复方向正确，采纳为本轮工作基线；avg_p8 未破最优故历史最优保持 r8） |
| per-act | actStats 为空——工作流本轮未提供 per-act 拆分；per-scene 见 §25.3 表（本地
  `out/hf-pr-to-video-launch-r8-diff/pixel.csv` vs `out/_p2v-r9c-k3diff/pixel.csv`）。
  **解读必须带 §25.3 编码噪声警示**：618 帧编码输入变化使 per-scene ±0.04 mass 抖动
  （useskill +0.0448 中渲染真实差仅 4%），真实收益以「pre/post 渲染帧直接 diff」为准：
  cta f855–860 −0.005/帧（URL 可见）、opener/fcode/fcredits caret 相位帧、worst f242
  −0.00038 均为真实渲染差 |

### 26.2 改动内容与参考出处

| 改动 | 参考出处 |
|---|---|
| XML：删除 `T.set('cta-url',{opacity:0},0)`——设 0 后无恢复路径（T.set 第二参数 vars
  只对 id 目标生效，函数目标被忽略），参考在 t=2.85 一次性插入的 URL 文本实际已占宽
  （f857 实测 test caret x 与 ref 差 33px ≈ 已占位）但全程不可见；初始空文本已保证
  零宽，该行多余且有害。修复后 f855–860 p8 0.0556→0.0504（−0.005/帧） | ref cta.html:93
  `tl.add(()=>{url.textContent=URL},2.85)`；XML 既有端口 843 行（与 ptime 惯用法同构） |
| XML：补 URL 高亮 flash `T.fromTo('cta-url',{backgroundColor:'rgba(217,119,87,0.36)'},
  {backgroundColor:'rgba(217,119,87,0)',duration:.5,ease:'power2.out'},O10+2.85)` | ref
  cta.html:94；引擎 backgroundColor 动画属性已注册（script/runtime/animation/properties/
  color.js:27） |
| 引擎通用：`Easing::Steps(n)` 段尾跳 `floor(t·N)/N` → GSAP SteppedEase
  `floor((N+1)·t)/N`（steps(1) 在补间中点跳变）——依据 gsap@3.12.5 min 源码反编译
  （`ratio = floor((N+1)·clamp(p,0,.99999999))/N`）；ref caret 逐帧闪烁时序验证吻合
  （段 0 前半 ON 后半 OFF、yoyo 反向段同理）。f32 精度坑：JS double 的
  0.99999999 在 f32 舍入为 1.0，t≥1 显式特判返回 1.0。单测 `steps_easing_quantizes`
  更新为 GSAP 语义 + steps(1) 半程跳断言。影响面：XML 全部 10 处 steps(1) caret 相位
  对齐 GSAP；XML caret 参数与参考同构无错构（opener.html:125=.4/1、use-skill.html:74
  =.5/1 + :79 set 2.0 已端口、feature-code.html:114=.4/9、discussion:166=.35/5） | gsap
  3.12.5 Ease.js SteppedEase；k3-promo/assets/gsap.min.js 反编译 + ref 逐帧实测
  （/tmp/p2v_r9_seq/ caret ON/OFF 序列） |
| 验证（本轮实际执行）：`cargo build --release` ✓；`cargo test -p opencat-core --lib`
  **713 passed, 0 failed** ✓；f857 目检 URL 可见 + 淡橙高亮残留与 ref 一致（
  /tmp/p2v_r9_frames/cmp_r9b_f857_url.png）；f897 放大字符级对比两侧文字一致（
  cmp_f897_zoom.png） | — |
| 证伪回滚（未保留改动，结论保留）：§24.4 #4「leading-none 无解析路径」错误——named
  leading 类自始在 `build.rs:1035-1040`（EXACT_CLASS_RULES 经 OUT_DIR 注入）；实验链
  证明 XML 13 处 leading-none 全部正确，本轮一度实装的 named leading 补全与删类改动
  已全部回滚（与 build.rs 规则重复） | build.rs:1035-1040（`leading-none`→
  `ExactClassAction::LineHeight(1.0)` 等六项） |

### 26.3 负结果 / 坑（本轮实测，详 §25.4）

- **§24.4 #4 证伪**：查 Tailwind 命名类解析支持必须同时 grep `tailwind.rs` 与
  `build.rs`（OUT_DIR 生成规则）——只查前者会误诊「无解析路径」。
- **参考字面 1:1 之外还要过「引擎默认值 ≠ 浏览器默认值」校验**：hr-t/auto183 的
  leading-none 与参考 CSS 字面不对应，但它是引擎继承默认 1.5 → Chrome normal ≈1.0
  差异的正确补偿；按字面删除反而恶化 +0.92 mass（f265 大标题位移 45px = half-leading）。
- **T.set 函数目标的第二参数 vars 不生效**：842 行 opacity:0 设出后无人能恢复——同构
  惯用法（748 行等 ptime）没踩坑是因为它们不需要恢复状态。
- **编码噪声淹没结构性修复（方法论级）**：改变数百帧像素输入的修复，其 per-scene p8
  对比含 ±0.04 mass 级 x264 帧间预测扩散噪声（useskill 实测 96% 为噪声）；后续动画
  时序类修复请用 pre/post 渲染帧直接 diff 报告真实渲染差，并以 10 场景聚合净变化判方向。
- 沿承未修：s02 prscroll blur 态（worst f242 连续 6 轮，本轮因 caret 相位微降）、
  Archivo Black kerning ~4%、cta f870–899 URL 文字渲染微差（+0.001/帧，§25.5 #2）。

### 26.4 下一步建议（按潜在收益排序）

1. **fdisc 静止尾段 f674–689**（0.0405，现最大非 blur 场景，16 帧恒定 0.056，bbox 固定
   (346,68,1574,975)）：对话卡区逐块归因（残差与本轮变化无关，已用渲染帧 diff 排除）。
2. **cta f870–899 的 URL 文字渲染微差**（+0.001/帧×30 帧）：mono4 字距/字形与 Chrome
   的差在 URL 可见后浮出；与沿承的 Archivo Black kerning 同族。
3. **opener f63–85 高误差簇**（18 帧 >0.05，mass 4.11 占 14.1%）：多轮未专门归因。
4. **prscroll f191–251**：滚动相位已证伪（dy≤4px），主残差 ref 编码平滑不追（§15.3）。

## 27. Round 10（2026-10-09，对齐工程师-r10）— §26.4 #1 fdisc 静止尾段归因落地：fd-merge 徽章三处结构修复（盒宽写死/weight600 丢失/入场 from 缺失）— 同管公平对比全片 −0.0000178、fdisc −0.000268（16 帧唯一差异帧区）；另证「归档 mp4 与权威测量不同源」（±0.0008 轮次漂移非噪声）

### 27.1 误差源选择与归因

按 r9 pixel.csv 场景质量排序（prscroll 17.9% 已证伪不追），选 **§26.4 #1 fdisc 静止尾段
f674–689**（fdisc 0.040579，现最大非 blur 场景；16 帧恒定 ~0.056，bbox 固定
(346,69,1574,975)，残差与 r9 改动无关已用渲染帧 diff 排除）。

f678 ref/test 并排 + 放大目检（/tmp/p2v_r10/）锁定差异元素 = 底部 `✓ all conversations
resolved · ready to merge` 徽章（ref feature-discussion.html:50,130-132）：

1. **盒宽写死导致内容溢出背景**：ref `.merge` 是 `position:absolute` + `display:inline-flex`
   （absolute 下被 Chrome blockify 为 flex，宽 = max-content 收缩盒 ~872 vstage / 532 屏，
   mp4 实测 BG [694,1225]）。XML 端口写死 `left-[630px] w-[660px] justify-center`（内容区
   598）——mono5 30px 的真实 advance ≈18px（0.6em，Spline Sans Mono 与引擎 cosmic-text
   一致），✓+29 字符+14 字符+2×gap11 ≈ 815 vstage → justify-center 把内容**居中溢出**，
   ✓ 与 `ready to merge` 画出背景外（test BG [712,1215]、TEXT [713,1209] 贴边溢出）。
   引擎侧证据：临时探针单测（已删）证明 width 660 生效（660×79 @630,880，items 658 >
   内容区 598 溢出居中，auto406 x=1.0）；单帧剖面 y=790 bg w=403 = 660×0.61 ✓。
   **「引擎 text 测量比渲染窄 8%」假设证伪**：`measure_text` 与 `rasterize_glyphs` 同源
   （text/mod.rs:439/484 同一 cosmic-text Buffer 管线），shrink-to-fit 会精确包住文字。
2. **`<b>` 的 weight 600 语义丢失**：ref `.merge b{font-weight:600}`（132 行），assets 只有
   SplineSansMono 400/500（fonts/ 目录实证）→ Chrome 选 500 face + 合成粗体（放大图
   `ready to merge` 笔画明显加粗）。XML auto408 无字重类 → 500 细体。修复 = `font-semibold`
   （引擎合成粗体路径已存在，text/mod.rs bold-amount 测试族），放大图确认生效。
3. **入场动画 from 语义缺失**：ref `gsap.from('#fd-merge',{opacity:0,scale:.85,y:10})` +
   `gsap.to(...,{opacity:1,scale:1,y:0,duration:.55,ease:'back.out(1.6)'},5.9)`；XML 只有
   `T.to(...,{opacity:1,scale:1,y:0,...})`——无 from 时 scale/y 起点即终点（不生效），
   入场帧 f673–678（正处静止段头部）无 scale .85→1 / y 10→0。修复 = `T.fromTo`。
4. 微调：auto407 尾随空格删除（ref flex 匿名 item 折叠首尾空白，`·` 后只剩 gap 11）。

### 27.2 改动内容与参考出处（全部 XML，零引擎改动）

| 改动 | 参考出处 |
|---|---|
| fd-merge 外包通栏 wrapper `fd-merge-c`（`absolute left-0 top-[880px] w-[1920px] flex
  justify-center`），fd-merge 去掉 `left-[630px] w-[660px] justify-center`，保留 flex
  items-center gap-[11px] + 全部视觉类 → 盒随内容收缩（等价 ref absolute inline-flex
  shrink-to-fit + left:50% translateX(-50%) 居中） | feature-discussion.html:130
  `.merge{position:absolute;left:50%;transform:translateX(-50%);display:inline-flex;...}` |
| auto408 加 `font-semibold`（600） | feature-discussion.html:132 `.merge b{font-weight:600}` |
| 796 行 `T.to('fd-merge',{opacity:1,...})` → `T.fromTo('fd-merge',{opacity:0,scale:.85,y:10},
  {opacity:1,scale:1,y:0,...})` | feature-discussion.html 脚本 `gsap.from(...)` + `gsap.to(...,5.9)` |
| auto407 文本尾空格删除 | ref html:50 匿名文本 item 首尾空白折叠 |

修复后单帧 f678 实测（eng PNG）：BG [691,1229] w=539（ref 532，修复前 504）、TEXT
[713,1209] w=497 全部入盒、`ready to merge` 合成粗体、✓ 在内——放大图
badge-zoom-eng2.png 与 ref 目检一致。

### 27.3 测量（本轮实际执行）与方法论级发现

`cargo build --release` ✓；`cargo test -p opencat-core --lib` **713 passed, 0 failed** ✓。
渲染 + `python3 tools/k3diff.py <ref> out/pr-to-video-launch.mp4 --out out/_p2v-r10-k3diff`：
表观 avg_p8 **0.0333086**（vs r9 权威 0.032487 = +0.000821，表观恶化）。

**公平性实验链（本轮最重要产出）**：

1. 渲染帧直接 diff（r9 归档 mp4 vs 本轮 mp4）：fdisc 徽章区 f675–689 ~9600 px/帧外，
   f660/f695/f700 及 opener/prscroll/summarize/fcode/fflow/fcredits/cta 抽样帧 max
   delta = 0——**改动只落在 16 帧徽章区，无任何副作用扩散**。
2. 但 per-scene 表现 opener +0.0016 / prscroll +0.0024 / fcode +0.0019「恶化」→ 与像素
   零变化矛盾 → 深挖。
3. k3diff 重复性：同 mp4 两次运行 0/900 帧差（测量确定）。
4. 渲染确定性：同 XML 两次渲染 0/900 帧差（`out/_p2v-r10-k3diff` vs `_p2v-r10c`，avg_p8
   逐位相同 0.033308634）——**渲染完全确定，「x264 编码噪声」不来自同 XML 重渲染**。
5. **归档 r9 mp4 同管重测（`out/_p2v-r9re-k3diff`）= 0.0333265 ≠ r9 权威 0.032487**
   （差 0.00084）。f83 逐位核对：归档 mp4 与本轮 mp4 解码 bit 级相同（max delta 0），两者
   对 ref 的 p8 均为 0.088679591，而 r9-diff CSV 记 0.089421296 → **r9 轮跑权威测量的
   test mp4 与后来归档的 out/hf-pr-to-video-launch-r9.mp4 不是同一渲染产物**（归档流程
   gap：k3diff 之后 out/pr-to-video-launch.mp4 被再次渲染覆盖过）。
6. 同管公平对比（r9re vs r10）：**差异帧恰好只有 f674–689 的 16 帧**，fdisc
   0.040359→0.040091（−0.000268），全片 0.0333265→0.0333086（**−0.0000178**），其余
   9 场景 bit 级 0——本轮修复是真实、纯净的改善（方向正确，量级小于跨 mp4 版本差）。

**结论（方法论级，覆盖 §25.3「编码噪声」解释）**：渲染与测量双双确定性；跨轮 avg_p8
对比的 ±0.0008 级漂移来自**各轮 k3diff 所测 mp4 非同源**（XML 改动重渲染不可避免，
但「基线 mp4 与权威数字对应的渲染产物」未被归档固定）。**任何单轮 <±0.001 的全片
p8 变化不可作为改动效果的判据**；判据应改为：(a) 改动帧区的渲染帧直接 diff +
(b) 基线 mp4 与改动 mp4 的**同管配对 diff**（本轮方法）。历史链路数字（r0→r9）仍按
各轮权威记录保留，但解读须带此警示。

### 27.4 负结果 / 坑（本轮实测）

- **headless chrome 探针字体陷阱**：把 feature-discussion.html 复制到 /tmp 加 probe 脚本
  测 `.merge` bbox/advance，得 b 项 w=154（11px/字符）——**全部不可信**：`../fonts/`
  相对路径在 /tmp 失效 → fallback 系统 mono（advance 与 Spline Sans Mono 不同）。参考
  测量必须在参考项目目录内做（或 data-url 内联字体）。真实 Spline Sans Mono 30px
  advance ≈18px（0.6em），与引擎 cosmic-text 渲染一致（43 字符内容 814.8 vstage 反推）。
- **宽松色彩 mask 混入文字像素**：`(g>r+4)&(g>b+4)&(g>180)` 抓徽章背景时把抗锯齿文字
  边也计入 → 徽章宽 498 假象（=背景 403+文字溢出段），据此推出「引擎 text 测量偏小
  8%」的错误假设。几何测量须用单行剖面 + 严格背景阈值（g>195 且逐行核对）。
- **探针单测定位法有效**：`classed_div/classed_text + resolve_ui_tree +
  compute_layout_with_font_db_fn` 复刻 XML 子树，一击证明 width 660 生效、内容 658 溢出
  内容区 598（探针文件已删，模式可复用）。
- **T.to 无法表达 gsap.from**（无 from 状态时 scale/y 补间是 no-op）——ref 的
  `gsap.from` 必须端口为 `T.fromTo`，沿承 §25.2「T.set 函数目标 vars 不生效」同族陷阱：
  动画方向 API 的语义差要逐条核对。
- 沿承未修：s02 prscroll blur 态（worst f242 0.1027 沿承，本轮 16 帧外零变化）、
  Archivo Black kerning ~4%、cta f870–899 URL 文字渲染微差、badge 顶部残余 +4 屏
  （0.37% 帧高，疑似 ref 编码边缘扩散，未追）。

### 27.5 下一步建议（按潜在收益排序）

1. **先修归档流程**：每轮 k3diff 用的 test mp4 必须当即复制为 `out/hf-pr-to-video-launch-rN.mp4`
   （本轮已确保 r10 = out/pr-to-video-launch.mp4 即所测产物）；跨轮对比一律用
   「基线 mp4 同管重测」而非历史权威数字（本轮 27.3 方法）。
2. **fdisc 剩余残差**：徽章修复后 fdisc 0.040091，仍大于 fcredits/fflow；静止段 bbox
   仍 (346,69,1574,975) 级别 → 下一步逐块归因对话卡区（crow 行距/头像/pill 文本）。
3. **cta f870–899 URL 文字渲染微差**（§26.4 #2 沿承，+0.001/帧×30 帧）。
4. **opener f63–85 高误差簇**（§26.4 #3 沿承，18 帧 >0.05）。
5. **prscroll f191–251**：不追（§15.3）。

## 28. Round 10 归档（2026-10-09，工作流权威测量）— avg_p8 **0.032470**，kept ✅（历史最优保持 0.03247 @r8）— 跨端固定管差 0.000839 逐位复现，同管相对改善跨端一致

> §27 是本轮工程师的本地测量与归因过程（含 fd-merge 三处结构修复、渲染/测量确定性
> 实验链、「归档 mp4 与 r9 权威不同源」的发现）；本节是工作流管道的权威测量归档。
> 权威数字逐位取自工作流下发值：avg_p8 0.032469677533436214、mae_mean
> 2.841352868512803、worst f242 0.10037663966049383、kept true、actStats []。
> **与本地测量的关系（本轮新确立，修正 §27.3 表述）**：工作流权威与本地同片测量存在
> **固定管差 0.000839**——r10 端差 0.0333086−0.0324697=0.0008390，r9 端差（§27.3 归档
> mp4 重测 0.0333265−r9 权威 0.0324872）=0.0008392，两轮管差稳定到 2.8e-7 →
> 工作流端与本地端渲染/编码产物系统性不同（呼应 §17.3 曾出现的 ~0.00085 固定差：
> r7–r9 期间消失，r10 起重现），**不是**「归档流程 gap」孤例，而是跨端管线差。
> 跨端管差对改动效果无影响：工作流端 r9→r10 = −0.00001756，本地同管 r9re→r10 =
> −0.00001784——同管相对改善逐位一致（差 3e-7）。以本节为轮次结论。

### 28.1 权威测量

| 指标 | 值 |
|---|---|
| **avg_p8** | **0.032470**（r9 0.032487 → −0.0000176，与本地同管配对 −0.0000178 一致，fdisc 徽章修复的真实效果。链路 r0 0.037816 → r1 0.035777 → r2 0.034439 → r3 0.034135 → r4 0.033318 → r5 0.033184 → r6 0.033122 → r7 0.032800 → r8 0.032468 → r9 0.032487 → r10 0.032470；**历史最优保持 0.03247 @r8**——r10 比 r8 高 1.3e-6，噪声级） |
| mae_mean | 2.841353（r9 2.841832 → −0.000479，微降） |
| worst_p8 | 0.100377（仅记录，不作门；f242，s02 prscroll blur 态簇最差帧沿承。**与 r9 权威逐位相同**——f242 在本轮 16 帧改动区之外，工作流端渲染跨轮稳定；本地重渲染 f242=0.102685 与工作流端不同，恰为跨端管差的又一佐证） |
| kept | **true**（工作流判定：fd-merge 结构修复方向正确，采纳为本轮工作基线；avg_p8 未破最优故历史最优保持 r8） |
| per-act | actStats 为空——工作流本轮未提供 per-act 拆分；per-scene 沿用 §27.3 同管配对表（本地
  `out/_p2v-r9re-k3diff/pixel.csv` vs `out/_p2v-r10-k3diff/pixel.csv`）：**差异帧恰好只有
  f674–689 的 16 帧**，fdisc 0.040359→0.040091（−0.000268），其余 9 场景 bit 级 0 |

### 28.2 改动内容与参考出处

| 改动 | 参考出处 |
|---|---|
| XML：fd-merge 外包通栏 wrapper `fd-merge-c`（`absolute left-0 top-[880px] w-[1920px]
  flex justify-center`），fd-merge 去掉 `left-[630px] w-[660px] justify-center` 保留
  flex items-center gap-[11px] 与全部视觉类——盒随内容收缩（等价 ref absolute
  inline-flex shrink-to-fit + left:50% translateX(-50%) 居中）；此前 w-660 写死盒宽而
  mono5 30px advance≈18px 使内容 ~815 vstage 溢出内容区 598，✓ 与 ready to merge
  画出背景外。修复后 f678 徽章 BG 539 屏（ref 532，修复前 504）、文字全部入盒 | ref
  feature-discussion.html:130 `.merge{position:absolute;left:50%;transform:translateX(-50%);
  display:inline-flex;...}` |
| XML：auto408 加 `font-semibold`（600）——ref assets 无 600 face，Chrome 选 500 +
  合成粗体，放大图确认引擎合成粗体路径生效 | feature-discussion.html:132
  `.merge b{font-weight:600}`；fonts/ 目录仅 SplineSansMono 400/500 |
| XML：796 行 `T.to('fd-merge',{opacity:1,scale:1,y:0,...})` → `T.fromTo('fd-merge',
  {opacity:0,scale:.85,y:10},{opacity:1,scale:1,y:0,...})`——无 from 时 scale/y 补间
  是 no-op，入场帧 f673–678 的 scale .85→1 / y 10→0 此前缺失 | feature-discussion.html
  脚本 `gsap.from('#fd-merge',{opacity:0,scale:.85,y:10})` + `gsap.to(...,5.9)` |
| XML：auto407 文本尾空格删除 | ref html:50 匿名 flex item 首尾空白折叠 |
| 验证（本轮实际执行）：`cargo build --release` ✓；`cargo test -p opencat-core --lib`
  **713 passed, 0 failed** ✓（零引擎改动）；同 XML 两次渲染 0/900 帧差（渲染确定）、
  同 mp4 两次 k3diff 0 帧差（测量确定） | — |

### 28.3 负结果 / 坑（本轮实测，详 §27.4）

- **跨端固定管差 0.000839（本轮最重要发现，修正 §27.3「归档流程 gap」表述）**：r9/r10
  两轮「本地−工作流」端差稳定到 2.8e-7，且 worst 侧佐证（工作流端 f242 跨轮逐位相同、
  本地重渲染 f242 不同）指向工作流端与本地端的渲染/编码产物系统差。**任何单轮
  <±0.001 的全片 p8 变化不可作为改动效果判据**；判据改为：(a) 基线 mp4 与改动 mp4 的
  同管配对 diff（本轮 −0.0000178）+ (b) 改动帧区渲染帧直接 diff（本轮 16 帧徽章区）。
  历史链路数字仍按各轮权威保留，解读须带此警示。
- **headless chrome 探针字体陷阱**：/tmp 下的 wrapper 页面 `../fonts/` 相对路径失效 →
  fallback 系统 mono 测出 advance 11px 假象；参考测量必须在参考项目目录内做。真实
  Spline Sans Mono 30px advance ≈18px（0.6em），与引擎 cosmic-text 一致。
- **宽松色彩 mask 混入文字像素**：徽章宽 498 假象（背景 403+溢出文字段），据此推出
  「引擎 text 测量偏小 8%」的错误假设（已证伪：measure_text 与 rasterize_glyphs 同源，
  text/mod.rs:439/484）。几何测量须单行剖面 + 严格背景阈值。
- **T.to 无法表达 gsap.from**（无 from 状态时 scale/y 补间 no-op）——沿承 §25.4
  「T.set 函数目标 vars 不生效」同族：动画方向 API 语义差逐条核对。
- 探针单测定位法有效（classed_div/classed_text + resolve_ui_tree +
  compute_layout_with_font_db_fn 复刻子树，一击证明 width 660 生效、内容 658 溢出
  内容区 598；探针文件已删，模式可复用）。
- 沿承未修：s02 prscroll blur 态（worst f242 连续 7 轮）、Archivo Black kerning ~4%、
  cta f870–899 URL 文字渲染微差、badge 顶部残余 +4 屏（0.37% 帧高，未追）。

### 28.4 下一步建议（按潜在收益排序）

1. **fdisc 剩余残差**：徽章修复后 fdisc 同管 0.040091，仍大于 fcredits/fflow；静止段
   bbox 仍 (346,69,1574,975) 级别 → 对话卡区逐块归因（crow 行距/头像/pill 文本）。
2. **cta f870–899 URL 文字渲染微差**（+0.001/帧×30 帧，§26.4 #2 沿承；mono4 字距/字形
   与 Chrome 的差，与 Archivo Black kerning 同族）。
3. **opener f63–85 高误差簇**（18 帧 >0.05，§26.4 #3 沿承，多轮未专门归因）。
4. **跨端管差定因（方法论）**：0.000839 固定差的工作流端 vs 本地端渲染命令/编码参数
   diff（r7–r9 曾消失，r10 重现）——若可对齐则跨轮数字恢复直接可比。
5. **prscroll f191–251**：不追（§15.3）。
