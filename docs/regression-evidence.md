# Regression Evidence — Engine / Web Cross-Platform Gate

> Gate: **k3diff hard-pixel metrics**, not SSIM — `mae` / `maxd` / changed-pixel
> fractions from `compute_pixel_diff_rgba`, matching
> [`tools/k3diff.py`](../tools/k3diff.py). The earlier SSIM runs (#25 / #48)
> and the in-tree `web_frame_oracle` tests (removed in 4a6e903) are superseded
> by the `opencat-web-compare` ChromeDriver harness.

## Channels

- **Web vs native engine** (executor alignment — the primary gate):
  `scripts/compare-mp4.sh <example>`.
  - hex transport (default): per-frame CanvasKit `readPixels`, exact pixels;
    a long sweep OOMs the tab after a few hundred frames — chunk the range.
  - `WEB_VIDEO=1`: the range renders in-page and crosses as one H.264 MP4
    (WebCodecs + MP4Box.js). Codec loss is negligible on ordinary content but
    smears high-frequency noise textures — read those frames with tolerance.
- **Web vs a reference video**: add `REFERENCE=<mp4>`. Meaningful only
  relative to the **native floor** — first measure native-vs-reference
  (`scripts/compare-k3diff.sh`); a web-vs-reference failure that the native
  engine also has is a content gap, not an executor bug.

Defaults: `mae ≤ 1.0`, `p8 ≤ 0.02` (`MAX_MAE` / `MAX_FRAC`).

`p8` is dominated by glyph antialiasing — Skia and CanvasKit differ in
coverage on text edges by ~1% of pixels, with no positional shift.

## Known platform limits

- **HEVC**: the Chrome used by the harness has no HEVC WebCodecs decoder
  (`hev1`/`hvc1` unsupported; H.264 is fine), so HEVC source assets fail the
  web fail-fast media validation by design. Exclude those windows from the
  web-video gate; the native engine decodes them fine.
- **3D**: `Scene3` executes natively on `opencat-three` (wgpu) and on the web
  through real three.js r181 (`three-facade.ts`) — same IR, tolerance-level
  agreement, same k3diff gate.

## Evidence

- Per-frame artifacts (engine/web/diff PNGs; fails always, all frames with
  `SAVE_ALL=1`) are written under `out/compare-mp4-<stem>/frame-NNNN/`, or
  `OUT_DIR` if set.
- Whole-video native-vs-reference summaries are written by
  `scripts/compare-k3diff.sh` to `out/compare-k3diff-<stem>/`.
- CI (`.github/workflows/regression.yml`) runs the Rust suite, clippy, and the
  web build/typecheck jobs. The browser gate needs Chrome, ChromeDriver, and
  ffmpeg — run it locally via `scripts/compare-mp4.sh`.

### hyperframes-launch web gate (Scene3 era, 2026-10)

Native baseline: `out/hfl-gpu-crf0-r4.mp4` (native GPU render, crf0).
Transport notes: the hex channel is exact pixels but a tab dies after a few
hundred frames — probe single frames; the web-video channel encodes in-page
H.264 at 8 Mbps, so its mae is dominated by encode noise (static-gradient
floor ≈0.86, noise-texture content far above) and only makes sense against
the native floor, not raw gates. Mid-range `START_FRAME` works because
`decode_reference_frames_rgba` offsets by the MP4's `firstFrame`.
In-page render+encode cost varies by content: shader/SkSL segments exceed the
WebDriver script timeout even at 35 frames — probe those with hex singles
instead (executor-identical, mae 0.0002 on frame 645).

- Executor alignment (hex, exact pixels, web vs native render): static frame 0
  mae 0.05; flex-css frame 575 mae 0.24; flex-shader frames
  620/630/640/645/650/654 mae 0.0002-0.16; flex-threejs section 46/46, mae
  mean 0.074. Composite video+backdrop frames
  carry a decoder/blur-spill residual: frame 80 mae 2.83, frame 100 mae 2.51,
  frame 180 mae 5.77 (hot bands localized under cards, same order as the
  native-vs-reference content floor).
- Whole-movie web-video vs native (encode-noise floor): seg 550-654 mae 0.60 /
  0.18; 655-760 mae 0.97 (fails only at the 3D-section entry frames 655-660);
  761-865 mae 0.91; 976-1180 mae 0.98; 1181-1385 mae 0.64; 1386-1493 mae 0.21.
  Gate misses sit in the 1.1-2.5 mae band, i.e. transport artifacts, with hex
  probes covering the same regions exactly.
- Platform-excluded windows (HEVC assets `beat4-clip.mp4`, `pill.mp4`,
  `Frame 1.mp4`, `Frame 3.mp4`): frames ≈213-549, 866-905, 907-975.
- Pre-existing native gap (not web): the native render misses the text-intro
  headline (frame 0 dark-frac 0 vs reference 0.179), present before the
  Scene3 refactor (`out/hfl-cpu-ref.mp4` corroborates).
