//! Render every frame of a composition as `frame_%04d.png` into a directory.
//!
//! The k3diff harness path for machines whose linked ffmpeg lacks a software
//! H.264 encoder: render PNG sequences, mux them with an external ffmpeg
//! (`-crf 0`), then run `tools/k3diff.py` on the two videos.

use std::path::Path;

use opencat_engine::render::{EncodingConfig, render_from_jsonl_with_base};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("usage: opencat-frames <input> <out-dir>");
    let out_dir = args.next().expect("usage: opencat-frames <input> <out-dir>");
    let source = std::fs::read_to_string(&input)?;
    let base_dir = Path::new(&input).parent();
    std::fs::create_dir_all(&out_dir)?;
    opencat_core::profile::run_from_env(|| {
        render_from_jsonl_with_base(&source, base_dir, &out_dir, &EncodingConfig::png())
    })
}
