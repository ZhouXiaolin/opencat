//! 将 JSONL composition 跑到 MP4 并强制启用 profile 采集。
//!
//! 用法：`cargo run --release --bin profile_jsonl -- <jsonl_path> [frames_limit] [out_mp4]`
//!
//! - `frames_limit`：覆盖 composition 的原始帧数上限，默认 120 帧（便于对比采样）。
//! - `out_mp4`：输出文件名，默认 `out/profile-<stem>.mp4`。
//!
//! 会设置 `OPENCAT_PROFILE=1` / `OPENCAT_PROFILE_FORMAT=both`，profile 文本与 JSON
//! 都打到 stderr。便于重定向到对比文件（baseline vs head）。

use std::{path::PathBuf, time::Instant};

use opencat::{Composition, EncodingConfig, ScriptDriver, parse_file};

fn main() -> anyhow::Result<()> {
    let jsonl_path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("jsonl_path required as first argument"))?;
    let frames_limit: u32 = std::env::args()
        .nth(2)
        .map(|value| value.parse::<u32>())
        .transpose()?
        .unwrap_or(120);
    let out_path = std::env::args().nth(3).map(PathBuf::from).unwrap_or_else(|| {
        let stem = std::path::Path::new(&jsonl_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("composition");
        PathBuf::from(format!("out/profile-{stem}.mp4"))
    });

    // 强制开启 profile 聚合。render_mp4 内部会构造 ProfileConfig::from_env() 并
    // 在结束时 print_profile_summary 到 stderr。
    // SAFETY: single-threaded, before any rendering starts.
    unsafe {
        std::env::set_var("OPENCAT_PROFILE", "1");
        std::env::set_var("OPENCAT_PROFILE_FORMAT", "both");
    }

    let parsed = parse_file(&jsonl_path)?;
    let original_frames = parsed.frames as u32;
    let frames = original_frames.min(frames_limit);

    eprintln!(
        "profile_jsonl: {jsonl_path} — {}x{} @ {}fps, using {frames}/{original_frames} frames",
        parsed.width, parsed.height, parsed.fps
    );

    let root = if let Some(script) = parsed.script.as_deref() {
        if script.trim().is_empty() {
            parsed.root
        } else {
            let driver = ScriptDriver::from_source(script)?;
            parsed.root.script_driver(driver)
        }
    } else {
        parsed.root
    };

    let composition = Composition::new("profile")
        .size(parsed.width, parsed.height)
        .fps(parsed.fps as u32)
        .frames(frames)
        .audio_sources(parsed.audio_sources.clone())
        .root(move |_ctx| root.clone())
        .build()?;

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let encode_config = EncodingConfig::mp4();

    let started = Instant::now();
    composition.render(&out_path, &encode_config)?;
    let elapsed = started.elapsed();

    eprintln!(
        "profile_jsonl: rendered {} in {:.2}s ({:.1} fps effective)",
        out_path.display(),
        elapsed.as_secs_f64(),
        frames as f64 / elapsed.as_secs_f64()
    );

    Ok(())
}
