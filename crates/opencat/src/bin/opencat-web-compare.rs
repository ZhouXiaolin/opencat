//! Sampled web-vs-engine frame diff via the inspect ChromeDriver harness.
//!
//! Alignment methodology: **hard pixel metrics (k3diff), not SSIM**. SSIM
//! collapses a frame to one structural scalar and hides localized misalignment;
//! every sample here is judged by `mae` / `maxd` / per-threshold changed-pixel
//! fractions — the same ladder as `tools/k3diff.py`. Output (`pixel.csv` +
//! `summary.json`) reuses k3diff's schema so the native and web gates are
//! directly comparable.
//!
//! Why sampling (not whole MP4)?
//! - Web ground truth in this repo is raw RGBA from `web/test-oracle.html`
//!   (CanvasKit `readPixels`), the same path as `web_frame_oracle_tests`.
//! - Facade `exportMp4` depends on external `@webav/av-cliper` and re-encodes;
//!   that path is not the inspect oracle contract and is unreliable headless.
//!
//! Usage:
//!   opencat-web-compare examples/profile-showcase.jsonl \
//!     --out-dir out/compare-mp4-profile-showcase \
//!     --interval-secs 0.5
//!
//! Env: CHROME_BIN / CHROMEDRIVER_BIN / CHROMEDRIVER_URL (same as oracle tests).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Parser;
use opencat_engine::inspect::browser::{
    BrowserHarness, BrowserTestEnv, PIXEL_DIFF_THRESHOLDS, PixelDiff, WebAppServer,
    compute_pixel_diff_rgba, decode_reference_frames_rgba, repo_root, web_source_for_oracle,
    write_artifacts,
};
use opencat_engine::render::render_single_frame_from_jsonl_with_base;

#[derive(Parser, Debug)]
#[command(
    name = "opencat-web-compare",
    about = "Sample web frames via inspect ChromeDriver and SSIM against engine"
)]
struct Cli {
    /// Markup / JSONL example (repo-relative or absolute).
    input: PathBuf,

    /// Report directory (engine/web PNGs + summary).
    #[arg(long)]
    out_dir: PathBuf,

    /// Sample interval in seconds (default 0.5 → roughly every half second).
    #[arg(long, default_value_t = 0.5)]
    interval_secs: f64,

    /// Optional hard cap on number of sample frames.
    #[arg(long)]
    max_samples: Option<usize>,

    /// Only sample frames within this range (inclusive). Long sweeps can be
    /// chunked because a single headless Chrome tab may crash after a few
    /// hundred frames (see issue with tab crashes on complex scenes).
    #[arg(long, default_value_t = 0)]
    start_frame: u32,

    /// End of the frame range (inclusive). Defaults to the last frame.
    #[arg(long)]
    end_frame: Option<u32>,

    /// Per-frame MAE ceiling (0..255). A sample passes when its mean absolute
    /// error is at or below this. k3diff-equivalent gate — no SSIM.
    #[arg(long, default_value_t = 1.0)]
    max_mae: f64,

    /// Per-frame max-channel-delta ceiling (0..255). 0 disables the check.
    #[arg(long, default_value_t = 0)]
    max_maxd: u32,

    /// Fraction of pixels above this max-channel delta must stay at or below
    /// `--max-frac`. Defaults to 8 (k3diff's `p8`).
    #[arg(long, default_value_t = 8)]
    frac_threshold: u32,

    /// Allowed fraction of pixels above `--frac-threshold` (k3diff `p8`). The
    /// default mirrors the oracle `STRICT_GATE`; glyph antialiasing alone can
    /// reach ~1% on text-dense frames.
    #[arg(long, default_value_t = 0.02)]
    max_frac: f64,

    /// Compare the web render directly against this reference video (e.g.
    /// hyperframes-launch `k3-promo.mp4`) instead of the native engine. This
    /// surfaces wasm-specific problems the engine-vs-web oracle cannot, since
    /// both sides there would share an engine bug.
    #[arg(long)]
    reference: Option<PathBuf>,

    /// Always write web/diff PNGs for every sample (not only failures).
    #[arg(long, default_value_t = false)]
    save_all: bool,
}

#[derive(Debug)]
struct SampleResult {
    frame: u32,
    diff: PixelDiff,
    passed: bool,
}

fn composition_meta(source: &str) -> Result<(u32, u32, u32, u32)> {
    // Returns (width, height, fps, frames). Prefer lightweight header parse.
    let trimmed = source.trim_start();
    if trimmed.starts_with('<') {
        // Parse through the engine's own markup path (strips the `<script>`
        // island, expands `<template>`s) rather than raw roxmltree — the
        // inline script body is not XML and would otherwise fail to parse.
        let parts = opencat_core::parse::parse_parts_with_base_dir(source, None)
            .context("parse xml composition")?;
        let fps = parts.fps.max(1) as u32;
        // Match the engine's frame count exactly (rational rounding, not ceil).
        let frames = opencat_core::duration_secs_to_frames(parts.duration, fps).max(1);
        return Ok((parts.width.max(1) as u32, parts.height.max(1) as u32, fps, frames));
    }

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(line).context("parse jsonl composition header")?;
        if value.get("type").and_then(|v| v.as_str()) == Some("composition") {
            let width = value
                .get("width")
                .and_then(|v| v.as_u64())
                .unwrap_or(1920) as u32;
            let height = value
                .get("height")
                .and_then(|v| v.as_u64())
                .unwrap_or(1080) as u32;
            let fps = value
                .get("fps")
                .and_then(|v| v.as_u64())
                .unwrap_or(30)
                .max(1) as u32;
            let frames = if let Some(f) = value.get("frames").and_then(|v| v.as_u64()) {
                f.max(1) as u32
            } else {
                let duration = value
                    .get("duration")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(3.0);
                (duration * f64::from(fps)).ceil().max(1.0) as u32
            };
            return Ok((width, height, fps, frames));
        }
    }
    bail!("could not find composition header in input");
}

fn sample_frames(fps: u32, total_frames: u32, interval_secs: f64, max_samples: Option<usize>) -> Vec<u32> {
    let step = ((interval_secs * f64::from(fps.max(1))).round() as u32).max(1);
    let mut frames = Vec::new();
    let mut f = 0u32;
    while f < total_frames {
        frames.push(f);
        if let Some(max) = max_samples {
            if frames.len() >= max {
                break;
            }
        }
        f = f.saturating_add(step);
    }
    if frames.is_empty() {
        frames.push(0);
    }
    // Always include last frame when it is not already covered.
    let last = total_frames.saturating_sub(1);
    if frames.last().copied() != Some(last) {
        if max_samples.is_none_or(|m| frames.len() < m) {
            frames.push(last);
        }
    }
    frames
}

fn rel_input(path: &Path, repo: &Path) -> String {
    path.strip_prefix(repo)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

async fn run_web_compare(
    cli: &Cli,
    repo: &Path,
    input_rel: &str,
    web_source: &str,
    width: u32,
    height: u32,
    fps: u32,
    total_frames: u32,
    frames: &[u32],
    engine_frames: &[(u32, Vec<u8>)],
    browser_env: &BrowserTestEnv,
) -> Result<ExitCode> {
    let web_server = WebAppServer::new(repo)?;
    let browser = BrowserHarness::new(browser_env, width as i32, height as i32).await?;
    browser
        .navigate(&web_server.url("/test-oracle.html"))
        .await
        .context("navigate test-oracle.html")?;

    let frac_idx = PIXEL_DIFF_THRESHOLDS
        .iter()
        .position(|t| *t == cli.frac_threshold)
        .ok_or_else(|| anyhow::anyhow!(
            "--frac-threshold must be one of {:?}",
            PIXEL_DIFF_THRESHOLDS
        ))?;

    let mut results = Vec::with_capacity(frames.len());
    let mut any_fail = false;

    for (frame, engine_rgba) in engine_frames {
        let web = browser
            .render_frame(web_source, *frame)
            .await
            .with_context(|| format!("web render frame {frame}"))?;
        if web.width != width || web.height != height {
            bail!(
                "web frame {frame} size {}x{} != composition {width}x{height}",
                web.width,
                web.height
            );
        }

        let diff = compute_pixel_diff_rgba(engine_rgba, &web.rgba, width, height)
            .with_context(|| format!("pixel diff frame {frame}"))?;

        let frac = diff.frac_above[frac_idx];
        let passed = diff.mae <= cli.max_mae
            && (cli.max_maxd == 0 || diff.maxd <= cli.max_maxd)
            && frac <= cli.max_frac;
        if !passed {
            any_fail = true;
        }

        let frame_dir = cli.out_dir.join(format!("frame-{frame:04}"));
        if !passed || cli.save_all {
            write_artifacts(&frame_dir, width, height, engine_rgba, &web.rgba)
                .with_context(|| format!("write artifacts {}", frame_dir.display()))?;
        }

        let tag = if passed { "OK" } else { "FAIL" };
        eprintln!(
            "  [{tag}] frame {frame:>4}  mae={:.4}  maxd={}  p{}={:.5}",
            diff.mae, diff.maxd, cli.frac_threshold, frac
        );
        results.push(SampleResult {
            frame: *frame,
            diff,
            passed,
        });
    }

    browser.shutdown().await?;
    drop(web_server);

    let n = results.len().max(1);
    let mae_mean = results.iter().map(|r| r.diff.mae).sum::<f64>() / n as f64;
    let mae_max = results
        .iter()
        .map(|r| r.diff.mae)
        .fold(f64::NEG_INFINITY, f64::max);
    let maxd_max = results.iter().map(|r| r.diff.maxd).max().unwrap_or(0);
    let failed = results.iter().filter(|r| !r.passed).count();

    let reference_label = match &cli.reference {
        Some(p) => format!("video {}", p.display()),
        None => "native engine (Skia)".to_string(),
    };
    let summary = format!(
        "opencat-web-compare (inspect ChromeDriver, raw RGBA — k3diff metrics, no SSIM)\n\
         input:          {input_rel}\n\
         reference:      {reference_label}\n\
         composition:    {width}x{height} @{fps}fps total_frames={total_frames}\n\
         sample:         every {interval:.2}s → {n} frames {frames:?}\n\
         gate:           mae<={max_mae}  maxd<={max_maxd_desc}  p{frac_threshold}<={max_frac}\n\
         mae mean/max:   {mae_mean:.6} / {mae_max:.6}\n\
         maxd max:       {maxd_max}\n\
         failed:         {failed}/{n}\n\
         out_dir:        {out}\n\
         note:           uses web/test-oracle.html (same as web_frame_oracle_tests);\n\
                         not WebAV exportMp4. Prefer this for visual parity.\n",
        interval = cli.interval_secs,
        frames = frames,
        max_mae = cli.max_mae,
        max_maxd_desc = if cli.max_maxd == 0 {
            "off".to_string()
        } else {
            cli.max_maxd.to_string()
        },
        frac_threshold = cli.frac_threshold,
        max_frac = cli.max_frac,
        out = cli.out_dir.display(),
    );
    let summary_path = cli.out_dir.join("summary.txt");
    fs::write(&summary_path, &summary).context("write summary")?;

    // k3diff-compatible rows + aggregate for direct comparison with
    // tools/k3diff.py output.
    let mut rows = Vec::with_capacity(results.len());
    for r in &results {
        let d = &r.diff;
        rows.push(serde_json::json!({
            "frame": r.frame,
            "mae": d.mae,
            "maxd": d.maxd,
            "p2": d.frac_above[0],
            "p4": d.frac_above[1],
            "p8": d.frac_above[2],
            "p16": d.frac_above[3],
            "p32": d.frac_above[4],
            "p64": d.frac_above[5],
            "p128": d.frac_above[6],
            "bbox": d.bbox.map(|(x0, y0, x1, y1)| serde_json::json!([x0, y0, x1, y1])),
            "passed": r.passed,
        }));
    }
    let data = serde_json::json!({
        "agg": {
            "frames": n,
            "mae_mean": mae_mean,
            "mae_max": mae_max,
            "maxd_max": maxd_max,
            "avg_p2": results.iter().map(|r| r.diff.frac_above[0]).sum::<f64>() / n as f64,
            "avg_p4": results.iter().map(|r| r.diff.frac_above[1]).sum::<f64>() / n as f64,
            "avg_p8": results.iter().map(|r| r.diff.frac_above[2]).sum::<f64>() / n as f64,
            "avg_p16": results.iter().map(|r| r.diff.frac_above[3]).sum::<f64>() / n as f64,
            "avg_p32": results.iter().map(|r| r.diff.frac_above[4]).sum::<f64>() / n as f64,
            "avg_p64": results.iter().map(|r| r.diff.frac_above[5]).sum::<f64>() / n as f64,
            "avg_p128": results.iter().map(|r| r.diff.frac_above[6]).sum::<f64>() / n as f64,
        },
        "gate": {
            "max_mae": cli.max_mae,
            "max_maxd": cli.max_maxd,
            "frac_threshold": cli.frac_threshold,
            "max_frac": cli.max_frac,
            "failed": failed,
        },
        "rows": rows,
    });
    fs::write(
        cli.out_dir.join("summary.json"),
        serde_json::to_vec_pretty(&data)?,
    )
    .context("write summary.json")?;

    // Per-frame CSV in k3diff's column order.
    let mut csv = String::from("frame,mae,maxd,p2,p4,p8,p16,p32,p64,p128,bbox\n");
    for r in &results {
        let d = &r.diff;
        let bbox = d
            .bbox
            .map(|(x0, y0, x1, y1)| format!("\"[{x0}, {y0}, {x1}, {y1}]\" "))
            .unwrap_or_default();
        csv.push_str(&format!(
            "{},{:.4},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{}\n",
            r.frame,
            d.mae,
            d.maxd,
            d.frac_above[0],
            d.frac_above[1],
            d.frac_above[2],
            d.frac_above[3],
            d.frac_above[4],
            d.frac_above[5],
            d.frac_above[6],
            bbox.trim_end(),
        ));
    }
    fs::write(cli.out_dir.join("pixel.csv"), csv).context("write pixel.csv")?;

    eprint!("{summary}");
    if any_fail {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}


fn main() -> ExitCode {
    let cli = Cli::parse();
    match run_compare_sync(&cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(2)
        }
    }
}

fn run_compare_sync(cli: &Cli) -> Result<ExitCode> {
    let repo = repo_root()?;
    let input = if cli.input.is_absolute() {
        cli.input.clone()
    } else {
        repo.join(&cli.input)
    };
    if !input.is_file() {
        bail!("input not found: {}", input.display());
    }

    let source = fs::read_to_string(&input).with_context(|| format!("read {}", input.display()))?;
    let input_rel = rel_input(&input, &repo);
    let web_source = web_source_for_oracle(&input_rel, &source);
    let (width, height, fps, total_frames) = composition_meta(&source)?;
    let mut frames = sample_frames(fps, total_frames, cli.interval_secs, cli.max_samples);
    let end_frame = cli.end_frame.unwrap_or(total_frames.saturating_sub(1));
    if cli.start_frame > 0 || cli.end_frame.is_some() {
        frames.retain(|&f| f >= cli.start_frame && f <= end_frame);
        if frames.is_empty() {
            bail!(
                "frame range {}..={} selects no samples",
                cli.start_frame,
                end_frame
            );
        }
    }

    eprintln!(
        "opencat-web-compare: {}  {}x{} @{}fps frames={} samples={} (every {:.2}s ≈ step {})",
        input_rel,
        width,
        height,
        fps,
        total_frames,
        frames.len(),
        cli.interval_secs,
        ((cli.interval_secs * f64::from(fps)).round() as u32).max(1),
    );
    eprintln!("sample frames: {frames:?}");

    let Some(browser_env) = BrowserTestEnv::detect()? else {
        bail!(
            "ChromeDriver/Chrome unavailable. Set CHROMEDRIVER_BIN + CHROME_BIN, or CHROMEDRIVER_URL."
        );
    };

    fs::create_dir_all(&cli.out_dir)
        .with_context(|| format!("create {}", cli.out_dir.display()))?;

    // Reference frames first — outside any tokio runtime (render_single_frame
    // builds its own runtime and cannot nest). Default is the native engine;
    // `--reference <video>` compares the web render directly against the
    // original reference render instead.
    let engine_frames: Vec<(u32, Vec<u8>)> = if let Some(reference) = &cli.reference {
        if !reference.is_file() {
            bail!("reference video not found: {}", reference.display());
        }
        let decoded = decode_reference_frames_rgba(reference, width, height, &frames)
            .with_context(|| format!("decode reference {}", reference.display()))?;
        let mut out = Vec::with_capacity(frames.len());
        for &frame in &frames {
            match decoded.get(&frame) {
                Some(rgba) => out.push((frame, rgba.clone())),
                None => eprintln!(
                    "  (reference has no frame {frame}; skipping — video is shorter than the composition)"
                ),
            }
        }
        if out.is_empty() {
            bail!("reference video yielded none of the sampled frames");
        }
        out
    } else {
        let mut out = Vec::with_capacity(frames.len());
        for &frame in &frames {
            let (rgba, w, h) =
                render_single_frame_from_jsonl_with_base(&source, input.parent(), frame)
                    .with_context(|| format!("engine render frame {frame}"))?;
            if w != width || h != height {
                bail!("engine frame {frame} size {w}x{h} != composition {width}x{height}");
            }
            out.push((frame, rgba));
        }
        out
    };

    let runtime = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    runtime.block_on(run_web_compare(
        cli,
        &repo,
        &input_rel,
        &web_source,
        width,
        height,
        fps,
        total_frames,
        &frames,
        &engine_frames,
        &browser_env,
    ))
}
