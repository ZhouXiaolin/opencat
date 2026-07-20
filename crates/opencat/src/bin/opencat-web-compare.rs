//! Sampled web-vs-engine frame SSIM via the inspect ChromeDriver harness.
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
    BrowserHarness, BrowserTestEnv, WebAppServer, compute_ssim_rgba, repo_root,
    web_source_for_oracle, write_artifacts,
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

    /// Minimum per-frame SSIM to pass (default 0.99; video-heavy frames may need lower).
    #[arg(long, default_value_t = 0.99)]
    min_ssim: f64,

    /// Softer threshold used when the strict one fails (video decoder tolerance).
    #[arg(long, default_value_t = 0.97)]
    video_min_ssim: f64,

    /// Always write engine/web/diff PNGs for every sample (not only failures).
    #[arg(long, default_value_t = false)]
    save_all: bool,
}

#[derive(Debug)]
struct SampleResult {
    frame: u32,
    ssim: f64,
    passed: bool,
    threshold: f64,
}

fn composition_meta(source: &str) -> Result<(u32, u32, u32, u32)> {
    // Returns (width, height, fps, frames). Prefer lightweight header parse.
    let trimmed = source.trim_start();
    if trimmed.starts_with('<') {
        let doc = roxmltree::Document::parse(source).context("parse xml composition")?;
        let root = doc.root_element();
        let width = root
            .attribute("width")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1920u32);
        let height = root
            .attribute("height")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1080u32);
        let fps = root
            .attribute("fps")
            .and_then(|s| s.parse().ok())
            .unwrap_or(30u32)
            .max(1);
        let duration: f64 = root
            .attribute("duration")
            .and_then(|s| s.parse().ok())
            .unwrap_or(3.0);
        let frames = (duration * f64::from(fps)).ceil().max(1.0) as u32;
        return Ok((width, height, fps, frames));
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

        let ssim = compute_ssim_rgba(engine_rgba, &web.rgba, width, height)
            .with_context(|| format!("ssim frame {frame}"))?;

        let threshold = if ssim >= cli.min_ssim {
            cli.min_ssim
        } else {
            cli.video_min_ssim
        };
        let passed = ssim >= threshold;
        if !passed {
            any_fail = true;
        }

        let frame_dir = cli.out_dir.join(format!("frame-{frame:04}"));
        if !passed || cli.save_all {
            write_artifacts(&frame_dir, width, height, engine_rgba, &web.rgba)
                .with_context(|| format!("write artifacts {}", frame_dir.display()))?;
        }

        let tag = if passed {
            if ssim >= cli.min_ssim {
                "OK"
            } else {
                "OK(video)"
            }
        } else {
            "FAIL"
        };
        eprintln!(
            "  [{tag}] frame {frame:>4}  SSIM={ssim:.6}  threshold={threshold:.6}"
        );
        results.push(SampleResult {
            frame: *frame,
            ssim,
            passed,
            threshold,
        });
    }

    browser.shutdown().await?;
    drop(web_server);

    let min = results
        .iter()
        .map(|r| r.ssim)
        .fold(f64::INFINITY, f64::min);
    let max = results
        .iter()
        .map(|r| r.ssim)
        .fold(f64::NEG_INFINITY, f64::max);
    let avg = results.iter().map(|r| r.ssim).sum::<f64>() / results.len().max(1) as f64;
    let failed = results.iter().filter(|r| !r.passed).count();

    let summary = format!(
        "opencat-web-compare (inspect ChromeDriver, raw RGBA)\n\
         input:          {input_rel}\n\
         composition:    {width}x{height} @{fps}fps total_frames={total_frames}\n\
         sample:         every {interval:.2}s → {n} frames {frames:?}\n\
         thresholds:     min_ssim={min_ssim:.6} video_min_ssim={video_min_ssim:.6}\n\
         SSIM min/avg/max: {min:.6} / {avg:.6} / {max:.6}\n\
         failed:         {failed}/{n}\n\
         out_dir:        {out}\n\
         note:           uses web/test-oracle.html (same as web_frame_oracle_tests);\n\
                         not WebAV exportMp4. Prefer this for visual parity.\n",
        interval = cli.interval_secs,
        n = results.len(),
        frames = frames,
        min_ssim = cli.min_ssim,
        video_min_ssim = cli.video_min_ssim,
        out = cli.out_dir.display(),
    );
    let summary_path = cli.out_dir.join("summary.txt");
    fs::write(&summary_path, &summary).context("write summary")?;
    let mut stats = String::from("frame,ssim,threshold,passed\n");
    for r in &results {
        stats.push_str(&format!(
            "{},{:.6},{:.6},{}\n",
            r.frame, r.ssim, r.threshold, r.passed
        ));
    }
    fs::write(cli.out_dir.join("ssim_samples.csv"), stats).context("write csv")?;

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
    let frames = sample_frames(fps, total_frames, cli.interval_secs, cli.max_samples);

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

    // Engine references first — outside any tokio runtime (render_single_frame
    // builds its own runtime and cannot nest).
    let mut engine_frames = Vec::with_capacity(frames.len());
    for &frame in &frames {
        let (rgba, w, h) =
            render_single_frame_from_jsonl_with_base(&source, input.parent(), frame)
                .with_context(|| format!("engine render frame {frame}"))?;
        if w != width || h != height {
            bail!("engine frame {frame} size {w}x{h} != composition {width}x{height}");
        }
        engine_frames.push((frame, rgba));
    }

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
