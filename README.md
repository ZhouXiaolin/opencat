# OpenCat

OpenCat is a Rust toolkit for building and rendering timeline-based visual compositions.
It combines a declarative scene model, a JSONL-based interchange format, a layout engine,
and a rendering pipeline for generating animated output.

The project is organized as:

- `opencat` library: the core APIs for parsing, scene construction, layout, rendering, audio, and runtime support
- `opencat-player`: a desktop player for previewing JSONL compositions
- `parse_json`: a small CLI for parsing a JSONL composition and rendering it to `out/parsed.mp4`
- `examples/`: standalone demos and experiments

## Main Concepts

- Scene graph and timeline composition
- JSONL input format for describing compositions
- Layout and text measurement
- Frame rendering and audio rendering
- Script-driven animation and transitions

## Project Layout

```text
src/lib.rs                 Core library
src/bin/opencat-player.rs  Desktop player
src/bin/parse_json.rs      CLI renderer
examples/                  Example programs
```

## Quick Start

Run the desktop player:

```bash
cargo run --bin opencat-player -- path/to/input.jsonl
```

Render a JSONL file from the CLI:

```bash
cargo run --bin parse_json -- path/to/input.jsonl
```

Run an example:

```bash
cargo run --example hello_world
```

## Notes

- The player currently targets macOS and Windows.
- Some build targets require local graphics and FFmpeg dependencies.
- The JSONL format reference and related design notes live in `opencat.md`.
