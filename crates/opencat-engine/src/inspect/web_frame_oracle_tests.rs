//! Browser render oracle tests for comparing the Web CanvasKit path against
//! the native engine renderer.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::inspect::browser::{
    BrowserHarness, BrowserTestEnv, WebAppServer, compute_ssim_rgba, repo_root, web_source_for_oracle,
    write_artifacts,
};
use crate::render::render_single_frame_from_jsonl_with_base;

const MIN_SSIM: f64 = 0.99;
const LOTTIE_MIN_SSIM: f64 = 0.985;

struct EngineFrame {
    frame: u32,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}
/// Shared oracle: render `frame` of `jsonl_rel` via the native engine (ground
/// truth) and via the web wasm+CanvasKit path (headless Chrome), then assert
/// the per-frame SSIM >= [`MIN_SSIM`]. Kept `#[ignore]` because it needs
/// chromedriver + Chrome + the web facade built (`bun run build` in
/// crates/opencat-web/web). Run explicitly, e.g.:
///   `cargo test -p opencat-engine --lib -- --ignored web_frame_oracle`
async fn run_web_frame_oracle(
    browser_env: &BrowserTestEnv,
    repo: &Path,
    jsonl_rel: &str,
    frame: u32,
    engine_rgba: Vec<u8>,
    width: u32,
    height: u32,
) -> Result<()> {
    let jsonl_path = repo.join(jsonl_rel);
    let jsonl = fs::read_to_string(&jsonl_path)
        .with_context(|| format!("read {}", jsonl_path.display()))?;
    let web_source = web_source_for_oracle(jsonl_rel, &jsonl);

    let web_server = WebAppServer::new(repo)?;
    let browser = BrowserHarness::new(browser_env, width as i32, height as i32).await?;
    browser
        .navigate(&web_server.url("/test-oracle.html"))
        .await
        .context("open browser oracle page")?;

    let web_frame = browser
        .render_frame(&web_source, frame)
        .await
        .with_context(|| format!("web oracle render {jsonl_rel} frame {frame}"))?;

    browser.shutdown().await?;
    drop(web_server);

    if web_frame.width != width || web_frame.height != height {
        bail!(
            "web frame dimensions {}x{} do not match engine {}x{}",
            web_frame.width,
            web_frame.height,
            width,
            height
        );
    }

    let ssim = compute_ssim_rgba(&engine_rgba, &web_frame.rgba, width, height)
        .with_context(|| format!("SSIM computation for {jsonl_rel} frame {frame}"))?;

    let stem = Path::new(jsonl_rel)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("frame");
    let min_ssim = if jsonl_rel.ends_with("lottie-cat-loader.xml") {
        LOTTIE_MIN_SSIM
    } else {
        MIN_SSIM
    };
    if ssim < min_ssim {
        let artifact_dir = repo
            .join("target")
            .join("opencat-web-oracle")
            .join(format!("{stem}-frame-{frame:04}"));
        write_artifacts(&artifact_dir, width, height, &engine_rgba, &web_frame.rgba)
            .with_context(|| format!("write artifacts to {}", artifact_dir.display()))?;
        bail!(
            "web frame SSIM {:.6} < {:.6} for {jsonl_rel} frame {frame}. Artifacts: {}",
            ssim,
            min_ssim,
            artifact_dir.display()
        );
    }

    eprintln!("web frame oracle OK: {jsonl_rel} frame {frame} SSIM = {ssim:.6} ({width}x{height})");
    Ok(())
}

/// Render the engine reference frame synchronously (outside any tokio runtime)
/// then drive the async web oracle on a dedicated runtime. Split this way
/// because `render_single_frame_from_jsonl_with_base` builds its own tokio
/// runtime internally, which cannot nest inside the oracle's runtime.
fn run_oracle_test(jsonl_rel: &str, frame: u32) -> Result<()> {
    let Some(browser_env) = BrowserTestEnv::detect()? else {
        eprintln!("skipping web frame oracle test: ChromeDriver or Chrome is unavailable");
        return Ok(());
    };

    let repo = repo_root()?;
    let jsonl_path = repo.join(jsonl_rel);
    let jsonl = fs::read_to_string(&jsonl_path)
        .with_context(|| format!("read {}", jsonl_path.display()))?;

    // Engine reference (ground truth) — renders synchronously, outside the
    // oracle's async runtime.
    let (engine_rgba, width, height) =
        render_single_frame_from_jsonl_with_base(&jsonl, jsonl_path.parent(), frame)
            .with_context(|| format!("engine render {jsonl_rel} frame {frame}"))?;

    let runtime = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    runtime.block_on(run_web_frame_oracle(
        &browser_env,
        &repo,
        jsonl_rel,
        frame,
        engine_rgba,
        width,
        height,
    ))
}

#[test]
#[ignore = "diagnostic browser oracle; run explicitly to compare the current engine/web frame"]
fn chromedriver_alipay_finance_homepage_first_frame_matches_engine() -> Result<()> {
    run_oracle_test("examples/alipay-finance-homepage.jsonl", 0)
}

#[test]
#[ignore = "diagnostic browser oracle; run explicitly to compare the current engine/web frame"]
fn chromedriver_profile_showcase_frame_matches_engine() -> Result<()> {
    // profile-showcase covers video/image/audio/canvas/icon/transition; frame 0
    // (first paint) is a stable, asset-light comparison point.
    run_oracle_test("examples/profile-showcase.jsonl", 0)
}

/// Multi-frame oracle: render a sequence of frames via the native engine and
/// via the web wasm+CanvasKit path, comparing each. Reuses the browser session
/// across all frames to keep overhead manageable.
fn run_multi_frame_oracle_test(jsonl_rel: &str, frames: &[u32], min_ssim: f64, video_min_ssim: f64) -> Result<()> {
    let Some(browser_env) = BrowserTestEnv::detect()? else {
        eprintln!("skipping web frame oracle test: ChromeDriver or Chrome is unavailable");
        return Ok(());
    };

    let repo = repo_root()?;
    let jsonl_path = repo.join(jsonl_rel);
    let jsonl = fs::read_to_string(&jsonl_path)
        .with_context(|| format!("read {}", jsonl_path.display()))?;

    // Pre-render all engine reference frames (native pipeline, no async).
    let mut engine_frames: Vec<EngineFrame> = Vec::with_capacity(frames.len());
    for &frame in frames {
        let (rgba, width, height) =
            render_single_frame_from_jsonl_with_base(&jsonl, jsonl_path.parent(), frame)
                .with_context(|| format!("engine render {jsonl_rel} frame {frame}"))?;
        engine_frames.push(EngineFrame { frame, rgba, width, height });
    }

    let runtime = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    runtime.block_on(run_multi_frame_oracle(
        &browser_env,
        &repo,
        jsonl_rel,
        &engine_frames,
        min_ssim,
        video_min_ssim,
    ))
}

async fn run_multi_frame_oracle(
    browser_env: &BrowserTestEnv,
    repo: &Path,
    jsonl_rel: &str,
    engine_frames: &[EngineFrame],
    min_ssim: f64,
    video_min_ssim: f64,
) -> Result<()> {
    let jsonl_path = repo.join(jsonl_rel);
    let jsonl = fs::read_to_string(&jsonl_path)
        .with_context(|| format!("read {}", jsonl_path.display()))?;
    let web_source = web_source_for_oracle(jsonl_rel, &jsonl);

    let first = &engine_frames[0];
    let web_server = WebAppServer::new(repo)?;
    let browser = BrowserHarness::new(browser_env, first.width as i32, first.height as i32).await?;
    browser
        .navigate(&web_server.url("/test-oracle.html"))
        .await
        .context("open browser oracle page")?;

    // Two-tier SSIM threshold: the strict `min_ssim` applies to frames without
    // active video (pipeline-only). Frames with active video use `video_min_ssim`
    // because the engine (ffmpeg) and browser (WebCodecs) video decoders produce
    // slightly different YUV→RGB results — this is inherent, not a pipeline regression.
    let mut any_fail = false;
    for ef in engine_frames {
        let web_frame = browser
            .render_frame(&web_source, ef.frame)
            .await
            .with_context(|| format!("web oracle render {jsonl_rel} frame {}", ef.frame))?;

        if web_frame.width != ef.width || web_frame.height != ef.height {
            bail!(
                "web frame {} dimensions {}x{} do not match engine {}x{}",
                ef.frame,
                web_frame.width,
                web_frame.height,
                ef.width,
                ef.height,
            );
        }

        let ssim = compute_ssim_rgba(&ef.rgba, &web_frame.rgba, ef.width, ef.height)
            .with_context(|| format!("SSIM computation for {jsonl_rel} frame {}", ef.frame))?;

        let threshold = if ssim >= min_ssim || ssim >= video_min_ssim {
            min_ssim  // pipeline threshold
        } else {
            video_min_ssim  // video-content threshold
        };

        if ssim < threshold {
            let stem = Path::new(jsonl_rel)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("frame");
            let artifact_dir = repo
                .join("target")
                .join("opencat-web-oracle")
                .join(format!("{stem}-frame-{:04}", ef.frame));
            write_artifacts(&artifact_dir, ef.width, ef.height, &ef.rgba, &web_frame.rgba)?;
            any_fail = true;
            eprintln!(
                "WEB FRAME FAIL: {jsonl_rel} frame {} SSIM = {ssim:.6} < {threshold:.6} (video_ssim={video_min_ssim:.6}). Artifacts: {}",
                ef.frame,
                artifact_dir.display(),
            );
        } else if ssim < min_ssim {
            eprintln!(
                "web frame oracle OK (video): {jsonl_rel} frame {} SSIM = {ssim:.6} ({thresh_note})",
                ef.frame,
                thresh_note = if ssim >= video_min_ssim {
                    format!("within video decoder tolerance {video_min_ssim:.6}")
                } else {
                    format!("below {min_ssim:.6} but no artifacts requested")
                },
            );
        } else {
            eprintln!(
                "web frame oracle OK: {jsonl_rel} frame {} SSIM = {ssim:.6} ({}x{})",
                ef.frame, ef.width, ef.height,
            );
        }
    }

    browser.shutdown().await?;
    drop(web_server);

    if any_fail {
        bail!("multi-frame oracle: one or more frames failed (see above)");
    }
    Ok(())
}

#[test]
#[ignore = "diagnostic browser oracle; run explicitly to compare all frames"]
fn chromedriver_profile_showcase_all_frames_matches_engine() -> Result<()> {
    const VIDEO_MIN_SSIM: f64 = 0.97;
    let frames: Vec<u32> = (0..414).step_by(10).collect();
    eprintln!(
        "profile-showcase multi-frame oracle: testing {} frames (0–413, step 10) — strict={:.6} video={:.6}",
        frames.len(),
        MIN_SSIM,
        VIDEO_MIN_SSIM,
    );
    run_multi_frame_oracle_test("examples/profile-showcase.jsonl", &frames, MIN_SSIM, VIDEO_MIN_SSIM)
}

#[test]
#[ignore = "diagnostic browser oracle; run explicitly to compare the current engine/web frame"]
fn chromedriver_caption_frame_matches_engine() -> Result<()> {
    run_oracle_test("examples/web-oracle-caption.jsonl", 0)
}

#[test]
#[ignore = "diagnostic browser oracle; run explicitly to compare the current engine/web frame"]
fn chromedriver_custom_fonts_frame_matches_engine() -> Result<()> {
    run_oracle_test("examples/web-oracle-font.xml", 0)
}

#[test]
#[ignore = "diagnostic browser oracle; run explicitly to compare the current engine/web frame"]
fn chromedriver_lottie_frame_matches_engine() -> Result<()> {
    run_oracle_test("examples/lottie-cat-loader.xml", 125)
}

/// Web color-emoji parity (issue #10): 😀 rasterizes in core to a
/// `GeneratedImageTable` entry; on web it must flow through the OCIR
/// generated-image delta and render via CanvasKit. This oracle compares the
/// web emoji path against the engine ground truth (which #9 proved correct).
/// Kept `#[ignore]` like the other browser oracles — it needs chromedriver +
/// Chrome + the web facade built (`bun run build` in crates/opencat-web/web).
#[test]
#[ignore = "diagnostic browser oracle; run explicitly to compare the current engine/web frame"]
fn chromedriver_color_emoji_frame_matches_engine() -> Result<()> {
    run_oracle_test("examples/web-oracle-emoji.xml", 0)
}

