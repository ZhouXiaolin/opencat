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
    let base_dir = input.parent().map(PathBuf::from);
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
            "(() => { globalThis.__canvas = document.getElementById('frame'); return globalThis.__canvas ? 'captured' : 'missing'; })()".to_string(),
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
