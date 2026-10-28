//! Headless web render driver: wasm facade + CanvasKit via ChromeDriver.
//!
//! Renders a composition on the web stack and writes one H.264 MP4 for the
//! selected frame range — the web half of the k3diff acceptance standard.
//! Comparison itself belongs to `tools/k3diff.py`; this binary only produces
//! the video.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use opencat_web_oracle::{
    BrowserHarness, BrowserTestEnv, WebAppServer, repo_root, web_envelope_for_oracle,
};

#[derive(Parser)]
#[command(
    name = "opencat-web-render",
    about = "Render a composition to MP4 on the web stack (wasm + CanvasKit, headless Chrome)"
)]
struct Cli {
    /// Composition source (.jsx, repo-relative or absolute) — usually the
    /// compiled `dist/<stem>.bundle.jsx`.
    input: PathBuf,

    /// Output MP4 path.
    #[arg(long)]
    out: PathBuf,

    /// Directory for composition-relative paths (`assets/...`). Defaults to
    /// the input file's directory; set it when the input is a build artifact
    /// in `dist/`.
    #[arg(long)]
    base_dir: Option<PathBuf>,

    /// First frame of the rendered range (inclusive).
    #[arg(long, default_value_t = 0)]
    start: u32,

    /// Last frame of the rendered range (inclusive). Defaults to the last
    /// frame of the composition.
    #[arg(long)]
    end: Option<u32>,

    /// Composition frame ranges to skip, e.g. `213-549,866-905,907-975` —
    /// platform-excluded windows (Chrome WebCodecs cannot decode HEVC source
    /// assets). The kept frames render as contiguous segments and are
    /// concatenated (`ffmpeg -f concat -c copy`) into one MP4 in composition
    /// order; compare it against a reference cut down to the same windows.
    #[arg(long, value_name = "RANGES")]
    exclude: Option<String>,
}

/// Parse `a-b,c-d` into inclusive (start, end) pairs, sorted and merged.
fn parse_ranges(spec: &str) -> Result<Vec<(u32, u32)>> {
    let mut ranges = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (a, b) = part
            .split_once('-')
            .ok_or_else(|| anyhow!("invalid range `{part}` (expected `a-b`)"))?;
        let start: u32 = a.trim().parse().with_context(|| format!("range `{part}`"))?;
        let end: u32 = b.trim().parse().with_context(|| format!("range `{part}`"))?;
        if end < start {
            bail!("range `{part}` is reversed");
        }
        ranges.push((start, end));
    }
    ranges.sort();
    Ok(ranges)
}

/// Complement of `ranges` within `[first, last]`, as contiguous kept segments.
fn kept_segments(first: u32, last: u32, ranges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut segments = Vec::new();
    let mut cursor = first;
    for (start, end) in ranges {
        if *start > cursor {
            segments.push((cursor, start.saturating_sub(1)));
        }
        cursor = cursor.max(end.saturating_add(1));
    }
    if cursor <= last {
        segments.push((cursor, last));
    }
    segments
}

fn composition_meta(source: &str, base_dir: Option<&Path>) -> Result<(u32, u32, u32, u32)> {
    // (width, height, fps, frames) through the engine's own parse — same
    // header the native renderer resolves.
    let parsed = opencat_engine::source_io::parse_with_base_dir(source, base_dir)
        .context("parse composition")?;
    let fps = parsed.fps.max(1) as u32;
    let frames = opencat_core::frame_ctx::duration_secs_to_frames(parsed.duration, fps).max(1);
    Ok((
        parsed.width.max(1) as u32,
        parsed.height.max(1) as u32,
        fps,
        frames,
    ))
}

fn run(cli: &Cli) -> Result<()> {
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
    let base_dir = cli
        .base_dir
        .clone()
        .or_else(|| input.parent().map(Path::to_path_buf));
    let base_dir = base_dir.as_deref();

    let envelope = web_envelope_for_oracle(&source, base_dir)
        .with_context(|| format!("extract JSX for the web oracle ({})", input.display()))?;
    let (width, height, fps, total_frames) = composition_meta(&source, base_dir)?;
    let start = cli.start.min(total_frames.saturating_sub(1));
    let end = cli.end.unwrap_or(total_frames.saturating_sub(1)).min(total_frames.saturating_sub(1));
    if end < start {
        bail!("frame range {start}..={end} is empty");
    }
    let exclude = match &cli.exclude {
        Some(spec) => parse_ranges(spec)?,
        None => Vec::new(),
    };
    let segments = kept_segments(start, end, &exclude);
    if segments.is_empty() {
        bail!("frame range {start}..={end} is fully excluded");
    }
    let kept_frames: u32 = segments.iter().map(|(a, b)| b - a + 1).sum();
    let excluded_frames = end - start + 1 - kept_frames;

    let Some(browser_env) = BrowserTestEnv::detect()? else {
        bail!(
            "ChromeDriver/Chrome unavailable. Set CHROMEDRIVER_BIN + CHROME_BIN, or CHROMEDRIVER_URL."
        );
    };

    eprintln!(
        "opencat-web-render: {}  {width}x{height} @{fps}fps frames {start}..={end} \
         ({} segment(s), {excluded_frames} excluded) -> {}",
        input.display(),
        segments.len(),
        cli.out.display()
    );

    let runtime = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    let rendered: Vec<(u32, u32, opencat_web_oracle::FramesMp4)> = runtime.block_on(async {
        let web_server = WebAppServer::new(&repo)?;
        let browser = BrowserHarness::new(&browser_env, width as i32, height as i32).await?;
        browser
            .navigate(&web_server.url("/test-oracle.html"))
            .await
            .context("navigate test-oracle.html")?;
        let mut rendered = Vec::with_capacity(segments.len());
        let mut result: Result<()> = Ok(());
        for (seg_start, seg_end) in &segments {
            match browser.render_frames_to_mp4(&envelope, *seg_start, *seg_end).await {
                Ok(mp4) => rendered.push((*seg_start, *seg_end, mp4)),
                Err(err) => {
                    result = Err(err).with_context(|| format!("web render frames {seg_start}..={seg_end}"));
                    break;
                }
            }
        }
        for line in browser.browser_logs().await {
            eprintln!("browser-console {line}");
        }
        let _ = browser.shutdown().await;
        drop(web_server);
        result?;
        Ok::<Vec<(u32, u32, opencat_web_oracle::FramesMp4)>, anyhow::Error>(rendered)
    })?;

    for (seg_start, seg_end, mp4) in &rendered {
        if mp4.width != width || mp4.height != height {
            bail!(
                "web video size {}x{} != composition {width}x{height}",
                mp4.width,
                mp4.height
            );
        }
        let expected = seg_end - seg_start + 1;
        if mp4.frames_encoded != expected {
            bail!(
                "web video encoded {} frames, expected {expected} ({seg_start}..={seg_end})",
                mp4.frames_encoded
            );
        }
    }

    if let Some(parent) = cli.out.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    // The in-page WebCodecs H.264 encoder emits bt709 full-range but writes
    // no VUI, so every decoder assumes tv-range and the comparison shifts by
    // a uniform color offset. Inject the true VUI on the way out (stream
    // copy, no re-encode). Without ffmpeg on PATH, fall back to the raw
    // untagged bitstream with a loud warning.
    const COLOR_BSF: &[&str] = &[
        "-bsf:v",
        "h264_metadata=colour_primaries=1:transfer_characteristics=1:matrix_coefficients=1:video_full_range_flag=1",
    ];

    if rendered.len() == 1 {
        let (_, _, mp4) = &rendered[0];
        let seg_path = cli
            .out
            .with_extension("raw.mp4");
        fs::write(&seg_path, &mp4.mp4).with_context(|| format!("write {}", seg_path.display()))?;
        let status = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-i"])
            .arg(&seg_path)
            .args(["-c", "copy"])
            .args(COLOR_BSF)
            .arg(&cli.out)
            .status();
        let _ = fs::remove_file(&seg_path);
        match status {
            Ok(status) if status.success() => {}
            status => {
                eprintln!(
                    "warning: ffmpeg VUI tag failed ({status:?}); wrote untagged bitstream"
                );
                fs::write(&cli.out, &mp4.mp4)
                    .with_context(|| format!("write {}", cli.out.display()))?;
            }
        }
        println!(
            "Done: {} ({} frames, {} bytes)",
            cli.out.display(),
            mp4.frames_encoded,
            mp4.mp4.len()
        );
        return Ok(());
    }

    // Multiple kept segments: write each to a scratch file and concatenate
    // with the ffmpeg concat demuxer (stream copy — every segment comes from
    // the same in-page encoder configuration).
    let concat_dir = cli
        .out
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let mut list = String::new();
    let mut total_frames = 0u32;
    for (idx, (seg_start, seg_end, mp4)) in rendered.iter().enumerate() {
        let seg_path = concat_dir.join(format!(
            "{}.seg{idx:02}.mp4",
            cli.out.file_stem().unwrap_or_default().to_string_lossy()
        ));
        fs::write(&seg_path, &mp4.mp4)
            .with_context(|| format!("write segment {}", seg_path.display()))?;
        eprintln!(
            "  segment {idx}: frames {seg_start}..={seg_end} -> {} ({} bytes)",
            seg_path.display(),
            mp4.mp4.len()
        );
        list.push_str(&format!("file '{}'\n", seg_path.display()));
        total_frames += mp4.frames_encoded;
    }
    let list_path = concat_dir.join(format!(
        "{}.concat.txt",
        cli.out.file_stem().unwrap_or_default().to_string_lossy()
    ));
    fs::write(&list_path, &list).with_context(|| format!("write {}", list_path.display()))?;
    // Each in-page segment stamps its VideoFrames with COMPOSITION-frame
    // timestamps (segment k starts at frame k's time, not zero), so a stream
    // copy would merge into a non-monotonic PTS mess. Decode-and-re-encode
    // with a clean per-frame timeline instead; crf 0 keeps the bitstream
    // faithful and the explicit color tags replace the single-segment bsf.
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list_path)
        .args(["-vf", "setpts=N/FRAME_RATE/TB", "-r"])
        .arg(fps.to_string())
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "0",
            "-pix_fmt",
            "yuv420p",
            "-color_range",
            "pc",
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
        ])
        .arg(&cli.out)
        .status()
        .context("spawn ffmpeg to concatenate web render segments")?;
    let _ = fs::remove_file(&list_path);
    for (idx, _) in rendered.iter().enumerate() {
        let seg_path = concat_dir.join(format!(
            "{}.seg{idx:02}.mp4",
            cli.out.file_stem().unwrap_or_default().to_string_lossy()
        ));
        let _ = fs::remove_file(&seg_path);
    }
    if !status.success() {
        bail!("ffmpeg segment concatenation failed (exit {:?})", status.code());
    }
    println!(
        "Done: {} ({} frames in {} segments)",
        cli.out.display(),
        total_frames,
        rendered.len()
    );
    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(2)
        }
    }
}
