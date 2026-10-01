# Regression Evidence — Engine / Web Cross-Platform Gate

> Current gate: **k3diff hard-pixel metrics**, not SSIM. The earlier SSIM runs
> (#25 / #48) were superseded when the oracle moved to `compute_pixel_diff_rgba`
> (`mae` / `maxd` / changed-pixel fractions), matching [`tools/k3diff.py`](../tools/k3diff.py).
> Methodology, commands and prerequisites: [`DEVELOPMENT.md`](../DEVELOPMENT.md)
> (Engine / Web pixel alignment).

## Gates

Compile-time `PixelGate { max_mae, max_p8 }` in
`crates/opencat-engine/src/inspect/tests/web_frame_oracle.rs`:

| Band | `mae` | `p8` |
|------|------:|-----:|
| Strict (still frames) | ≤ 1.0 | ≤ 0.02 |
| Video-active | ≤ 2.0 | ≤ 0.03 |
| Lottie | ≤ 2.0 | ≤ 0.02 |

`p8` is dominated by glyph antialiasing — Skia and CanvasKit differ in coverage
on text edges by ~1% of pixels, with no positional shift.

The native metric is verified byte-for-byte against `tools/k3diff.py` on the
same PNG pair, so engine and CLI reports never diverge.

## Baseline: web render vs the reference video

The engine-vs-web oracle only catches divergence **between** the two backends;
a bug in the shared parser/render core passes it. Measuring both against the
original render (hyperframes-launch `k3-promo.mp4`, via
`opencat-web-compare --reference`) closes that gap:

| Baseline | `mae` (approx.) |
|----------|----------------:|
| Native engine vs reference | 1.64 |
| WASM/web vs reference | 1.67 |

The web path sits within ~0.03 `mae` of the native floor — the wasm transport
itself adds no meaningful error. Command and details: `DEVELOPMENT.md`.

## Evidence

- Per-frame oracle artifacts (pass and fail) are written under
  `target/opencat-web-oracle/<stem>-frame-NNNN/{engine,web,diff}.png`.
- Whole-video native-vs-reference summaries are written by
  `scripts/compare-k3diff.sh` to `out/compare-k3diff-<stem>/`.
- CI ([`.github/workflows/regression.yml`](../.github/workflows/regression.yml))
  runs the Rust suite, clippy, and the web build/typecheck/test jobs; the
  ChromeDriver oracles are `#[ignore]` and run locally (they need Chrome,
  ChromeDriver, ffmpeg, and the built web facade).
