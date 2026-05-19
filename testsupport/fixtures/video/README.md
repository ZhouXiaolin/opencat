# Video Fixtures

Synthetic test clips for the Worker video decode pipeline. Each is 2s,
480x270 @ 30fps, ≤1MB. Regenerate with `scripts/regen-video-fixtures.sh`
if encoders or testing requirements change.

| File | Container | Video | Audio | Use |
|---|---|---|---|---|
| `sample.mp4` | MP4 | H.264 yuv420p | AAC 64kbps | Baseline; also exercises Open-GOP fallback when keyframes are non-IDR |
| `sample.webm` | WebM | VP9 yuv420p | none | Verifies non-MP4 demux path |
| `sample.mkv` | Matroska | H.264 yuv420p | none | Verifies non-MP4 container with H.264 codec |

Sources are `testsrc2` from ffmpeg's `lavfi` — deterministic, no
external assets.
