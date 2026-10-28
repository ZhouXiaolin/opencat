//! Throwaway debug driver: step through the harness page calls one by one
//! and keep full JS stacks. `cargo run --bin web-eval -- <input.jsx> [frame]`

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use opencat_web_oracle::{
    repo_root, web_envelope_for_oracle, BrowserHarness, BrowserTestEnv, WebAppServer,
};

#[derive(Parser)]
struct Cli {
    input: PathBuf,
    #[arg(long, default_value_t = 0)]
    frame: u32,
    /// Composition base dir for `assets/...` (defaults to the input's parent;
    /// set it to the example root when the input is a dist/ bundle).
    #[arg(long)]
    base_dir: Option<PathBuf>,
    /// Render frames from this frame up to --frame in one session first.
    #[arg(long)]
    from: Option<u32>,
    /// Sweep variant: release the reusable CanvasKit surface after every
    /// non-final frame — isolates GL-context texture-pool state as the
    /// carrier of the stale-video-content bug.
    #[arg(long)]
    fresh_surface: bool,
    /// Sweep variant: capture + close a `new VideoFrame(canvas)` after every
    /// drawFrame, mimicking the production encode loop's per-frame canvas
    /// readback.
    #[arg(long)]
    mimic_encode: bool,
    /// Run the page's full renderFramesToMp4(from..=frame) instead of the
    /// sweep — the encode path (VideoEncoder + mux) is the last untested
    /// production difference. MP4 lands at %TEMP%/opencat-oracle-video.mp4.
    #[arg(long)]
    mp4_from: Option<u32>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo = repo_root()?;
    let input = if cli.input.is_absolute() {
        cli.input.clone()
    } else {
        repo.join(&cli.input)
    };
    let source = std::fs::read_to_string(&input)?;
    let base_dir = cli.base_dir.clone().or_else(|| input.parent().map(PathBuf::from));
    let envelope = web_envelope_for_oracle(&source, base_dir.as_deref())?;

    let Some(env) = BrowserTestEnv::detect()? else {
        anyhow::bail!("no browser env");
    };
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run(cli, envelope))
}

async fn run(cli: Cli, envelope: String) -> Result<()> {
    let repo = repo_root()?;
    let env = BrowserTestEnv::detect()?.expect("browser env checked in main");
    let server = WebAppServer::new(&repo)?;
    let browser = BrowserHarness::new(&env, 1920, 1080).await?;
    browser.navigate(&server.url("/test-oracle.html")).await?;

    let steps: Vec<(&str, String)> = vec![
        (
            "capture_canvas",
            // Grab a 2D canvas BEFORE open_design: the wasm realm patches
            // main-thread element factories afterwards and its stubs have no
            // drawImage/getImageData, so probes can't create one late.
            "(() => { globalThis.__canvas = document.getElementById('frame'); const pc = document.createElement('canvas'); pc.width = 1920; pc.height = 1080; globalThis.__probeCtx = pc.getContext('2d', { willReadFrequently: true }); globalThis.__pngCanvas = document.createElement('canvas'); return globalThis.__canvas && globalThis.__probeCtx ? 'captured' : 'missing'; })()".to_string(),
        ),
        (
            "open_design",
            format!("__opencatOracle.openDesign({})", serde_json::to_string(&envelope)?),
        ),
        (
            "build_frame_ir",
            format!(
                "(() => {{ const r = __opencatOracle.buildFrameIr({}); globalThis.__frame = r; return {{ irLen: r.ir.length, plan: JSON.stringify(r.mediaPlan).slice(0, 300) }}; }})()",
                cli.frame
            ),
        ),
    ];
    for (name, js) in &steps {
        match browser.eval_js(js).await {
            Ok(v) => println!("[ok] {name}: {}", &v[..v.len().min(500)]),
            Err(e) => println!("[FAIL] {name}:\n{e}"),
        }
    }

    // Dump the frame op stream at the requested frame — the IR-level answer
    // to "which layer vanished on web".
    let dump_js = format!("__opencatOracle.dumpFrameOps({}, 400)", cli.frame);
    match browser.eval_js(&dump_js).await {
        Ok(v) => println!("[ok] dumpFrameOps:\n{}", v.replace("\\n", "\n").trim_matches('"')),
        Err(e) => println!("[FAIL] dumpFrameOps:\n{e}"),
    }

    // Print the composition meta the page will parse — NaN width/height/fps
    // reaching VideoEncoderConfig is the leading suspect for the u64 error.
    let meta_js = format!(
        "(() => {{ const e = JSON.parse({}); return JSON.stringify(e.extract && e.extract.meta); }})()",
        serde_json::to_string(&envelope)?
    );
    match browser.eval_js(&meta_js).await {
        Ok(v) => println!("[ok] meta: {v}"),
        Err(e) => println!("[FAIL] meta:\n{e}"),
    }

    // Full drawFrame (videos injected); the page POSTs the PNG to /oracle-blob
    // (temp file) — multi-MB payloads don't fit WebDriver JSON. --from N
    // renders N..=frame in one session and captures WITHOUT redrawing: sweep
    // state (stale video caches / canvas carry-over) only shows sequentially.
    let post_png_js = "const b64 = await __opencatOracle.captureCanvasPng(); const bin = atob(b64); const bytes = new Uint8Array(bin.length); for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i); const res = await fetch('/oracle-blob', { method: 'POST', body: bytes }); var posted = 'posted ' + res.status;";
    match cli.from {
        Some(from) => {
            // Sweep, capturing every frame's PNG to its own blob sink; on the
            // last frame answer what the decoder returned (probe).
            let env_str = serde_json::to_string(&envelope)?;
            let fresh_surface_line = if cli.fresh_surface {
                "if (f < frame) __opencatOracle.releaseReusableSurface(globalThis.__canvas); "
            } else {
                ""
            };
            let mimic_line = if cli.mimic_encode {
                "const vf = new VideoFrame(globalThis.__canvas, { timestamp: f * 33333, duration: 33333 }); vf.close(); "
            } else {
                ""
            };
            let js = format!(
                "(async () => {{ const frame = {frame}; const post = async (f) => {{ const b64 = await __opencatOracle.captureCanvasPng(); const bin = atob(b64); const bytes = new Uint8Array(bin.length); for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i); const res = await fetch('/oracle-blob?f=' + f, {{ method: 'POST', body: bytes }}); return res.status; }}; \
                 let lastPost = null; \
                 for (let f = {from}; f <= frame; f++) {{ \
                   await __opencatOracle.drawFrame({env}, f); \
                   {fresh_surface_line}\
                   {mimic_line}\
                   lastPost = await post(f); \
                 }} \
                 const probe = await __opencatOracle.probeVideoTimes({env}, {frame}); \
                 return 'lastPost=' + lastPost + ' probe=' + probe; }})()",
                from = from,
                frame = cli.frame,
                env = env_str,
            );
            match browser.eval_js(&js).await {
                Ok(v) => println!("[ok] sweep-probe: {v}"),
                Err(e) => println!("[FAIL] sweep-probe:\n{e}"),
            }
        }
        None => {
            let mut mp4_ran = false;
            if let Some(mp4_from) = cli.mp4_from {
                let js = format!(
                    "(async () => {{ const r = await __opencatOracle.renderFramesToMp4({}, {mp4_from}, {}); \
                     return JSON.stringify({{ frames: r.framesEncoded, bytes: r.bytes, first: r.firstFrame }}); }})()",
                    serde_json::to_string(&envelope)?,
                    cli.frame
                );
                match browser.eval_js(&js).await {
                    Ok(v) => println!("[ok] renderFramesToMp4: {v}"),
                    Err(e) => println!("[FAIL] renderFramesToMp4:\n{e}"),
                }
                mp4_ran = true;
            }
            if !mp4_ran {
                let js = format!(
                    "(async () => {{ await __opencatOracle.drawFrame({}, {}); {post_png_js} return posted; }})()",
                    serde_json::to_string(&envelope)?,
                    cli.frame
                );
                match browser.eval_js(&js).await {
                    Ok(v) => println!("[ok] drawFramePng: {v}"),
                    Err(e) => println!("[FAIL] drawFramePng:\n{e}"),
                }
            }
        }
    }

    // Probe WebCodecs AFTER the design is open: if the wasm realm's shims
    // clobber page globals, the native encoder config check breaks here.
    let probe_js = r#"
        (async () => {
          const config = { codec: 'avc1.640028', width: 1920, height: 1080, bitrate: 8000000, framerate: 30, latencyMode: 'quality' };
          const support = await VideoEncoder.isConfigSupported(config);
          return 'supported=' + support.supported + ' ctor=' + String(VideoEncoder).slice(0, 80);
        })()
    "#;
    match browser.eval_js(probe_js).await {
        Ok(v) => println!("[ok] probe_encoder: {v}"),
        Err(e) => println!("[FAIL] probe_encoder:\n{e}"),
    }

    // Then the render half, instrumented: render into the harness canvas and
    // report pixel stats + a PNG data URL length so a silent black frame is
    // visible without shipping megabytes through WebDriver.
    let render_js = r#"
        (async () => {
          const CK = globalThis.__canvasKit;
          const canvas = globalThis.__canvas;
          const r = globalThis.__frame;
          if (!r) return 'no __frame — build step failed';
          const surface = __opencatOracle.getReusableSurface(CK, canvas, undefined, { alphaType: CK.AlphaType.Premul, preserveDrawingBuffer: 1 });
          if (!surface) return 'surface creation failed';
          __opencatOracle.renderEncodedDrawFrame(r.ir, surface.getCanvas(), CK, { surface });
          surface.flush();
          const img = surface.makeImageSnapshot();
          const px = img.readPixels(0, 0, { width: canvas.width, height: canvas.height, colorType: CK.ColorType.RGBA_8888, alphaType: CK.AlphaType.Premul, colorSpace: CK.ColorSpace.SRGB });
          img.delete();
          if (!px) return 'readPixels null';
          let sum = 0, nonBlack = 0;
          for (let i = 0; i < px.length; i += 4) {
            const lum = (px[i] + px[i+1] + px[i+2]) / 3;
            sum += lum;
            if (lum > 8) nonBlack++;
          }
          const stats = { meanLuma: (sum / (px.length / 4)).toFixed(2), nonBlackPct: (100 * nonBlack / (px.length / 4)).toFixed(2) };
          return JSON.stringify(stats);
        })()
    "#;
    match browser.eval_js(render_js).await {
        Ok(v) => println!("[ok] renderEncodedDrawFrame: {v}"),
        Err(e) => println!("[FAIL] renderEncodedDrawFrame:\n{e}"),
    }

    let _ = browser.shutdown().await;
    drop(server);
    Ok(())
}
