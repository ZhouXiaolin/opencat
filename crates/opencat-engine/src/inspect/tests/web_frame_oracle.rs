//! Browser render oracle tests for comparing the Web CanvasKit path against
//! the native engine renderer.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::inspect::browser::{
    BrowserHarness, BrowserTestEnv, PixelDiff, WebAppServer, compute_pixel_diff_rgba, repo_root,
    web_source_for_oracle, write_artifacts,
};
use crate::render::render_single_frame_from_jsonl_with_base;

/// Alignment gate in **k3diff terms (hard pixel metrics), not SSIM**. A frame
/// passes when its mean absolute error and `p8` (fraction of pixels changed by
/// more than 8) are both within the gate. See DEVELOPMENT.md
/// "Engine / Web pixel alignment (k3diff)".
#[derive(Clone, Copy, Debug)]
struct PixelGate {
    max_mae: f64,
    max_p8: f64,
}

/// Pipeline / still frames: engine and web must agree closely. `p8` is
/// dominated by glyph antialiasing — Skia and CanvasKit coverage-differ on
/// text edges by ~1% of pixels with no positional shift (verified: best-fit
/// shift is (0,0) on the failing text frames), so the gate allows that while
/// `mae` still pins the overall error near zero.
const STRICT_GATE: PixelGate = PixelGate {
    max_mae: 1.0,
    max_p8: 0.02,
};
/// Frames with active video: ffmpeg vs WebCodecs YUV→RGB differs inherently.
const VIDEO_GATE: PixelGate = PixelGate {
    max_mae: 2.0,
    max_p8: 0.03,
};
/// Lottie (Skottie vs CanvasKit animation sampling).
const LOTTIE_GATE: PixelGate = PixelGate {
    max_mae: 2.0,
    max_p8: 0.02,
};

/// `p8` is index 2 of the k3diff threshold ladder `[2, 4, 8, 16, 32, 64, 128]`.
const P8_INDEX: usize = 2;

fn gate_passes(diff: &PixelDiff, gate: PixelGate) -> bool {
    diff.mae <= gate.max_mae && diff.frac_above[P8_INDEX] <= gate.max_p8
}

struct EngineFrame {
    frame: u32,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}
/// Shared oracle: render `frame` of `jsonl_rel` via the native engine (ground
/// truth) and via the web wasm+CanvasKit path (headless Chrome), then assert
/// the frame clears its k3diff pixel gate. Kept `#[ignore]` because it needs
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

    let diff = compute_pixel_diff_rgba(&engine_rgba, &web_frame.rgba, width, height)
        .with_context(|| format!("pixel diff for {jsonl_rel} frame {frame}"))?;

    let stem = Path::new(jsonl_rel)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("frame");
    let gate = if jsonl_rel.ends_with("lottie-cat-loader.xml") {
        LOTTIE_GATE
    } else {
        STRICT_GATE
    };
    if !gate_passes(&diff, gate) {
        let artifact_dir = repo
            .join("target")
            .join("opencat-web-oracle")
            .join(format!("{stem}-frame-{frame:04}"));
        write_artifacts(&artifact_dir, width, height, &engine_rgba, &web_frame.rgba)
            .with_context(|| format!("write artifacts to {}", artifact_dir.display()))?;
        bail!(
            "web frame mae {:.4} / p8 {:.5} exceeds gate (mae<={:.4}, p8<={:.5}) for {jsonl_rel} frame {frame}. Artifacts: {}",
            diff.mae,
            diff.frac_above[P8_INDEX],
            gate.max_mae,
            gate.max_p8,
            artifact_dir.display()
        );
    }

    eprintln!(
        "web frame oracle OK: {jsonl_rel} frame {frame} mae={:.4} p8={:.5} ({width}x{height})",
        diff.mae,
        diff.frac_above[P8_INDEX],
    );
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
fn run_multi_frame_oracle_test(jsonl_rel: &str, frames: &[u32], gate: PixelGate) -> Result<()> {
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
        gate,
    ))
}

async fn run_multi_frame_oracle(
    browser_env: &BrowserTestEnv,
    repo: &Path,
    jsonl_rel: &str,
    engine_frames: &[EngineFrame],
    gate: PixelGate,
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

    // Hard pixel gate (k3diff metrics). The `gate` is pre-chosen by the
    // caller: a video-heavy composition gets the looser VIDEO_GATE because the
    // engine (ffmpeg) and browser (WebCodecs) video decoders produce slightly
    // different YUV→RGB results — this is inherent, not a pipeline regression.
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

        let diff = compute_pixel_diff_rgba(&ef.rgba, &web_frame.rgba, ef.width, ef.height)
            .with_context(|| format!("pixel diff for {jsonl_rel} frame {}", ef.frame))?;
        let p8 = diff.frac_above[P8_INDEX];
        let passed = gate_passes(&diff, gate);

        if !passed {
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
                "WEB FRAME FAIL: {jsonl_rel} frame {} mae={:.4} p8={:.5} exceeds gate (mae<={:.4}, p8<={:.5}). Artifacts: {}",
                ef.frame,
                diff.mae,
                p8,
                gate.max_mae,
                gate.max_p8,
                artifact_dir.display(),
            );
        } else {
            eprintln!(
                "web frame oracle OK: {jsonl_rel} frame {} mae={:.4} p8={:.5} ({}x{})",
                ef.frame, diff.mae, p8, ef.width, ef.height,
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
    let frames: Vec<u32> = (0..414).step_by(10).collect();
    eprintln!(
        "profile-showcase multi-frame oracle: testing {} frames (0–413, step 10) — gate mae<={:.4} p8<={:.5}",
        frames.len(),
        VIDEO_GATE.max_mae,
        VIDEO_GATE.max_p8,
    );
    run_multi_frame_oracle_test("examples/profile-showcase.jsonl", &frames, VIDEO_GATE)
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

