//! Throwaway debug driver: run an explicit list of page steps one command at
//! a time, printing each result and the browser console between steps — the
//! per-step visibility `web-eval`'s single-blob sweep lacks.
//!
//! Usage: `web-step --input a.html --step open --step draw 0 --step logs ...`
//! Steps: `open` · `ir <f>` · `draw <f>` · `warmup <f>` · `png <f>` ·
//! `mp4 <from> <to>` · `logs`

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use opencat_web_oracle::{
    BrowserHarness, BrowserTestEnv, WebAppServer, repo_root, web_envelope_for_oracle,
};

#[derive(Parser)]
struct Cli {
    input: PathBuf,
    #[arg(long)]
    base_dir: Option<PathBuf>,
    /// Steps run in order, one quoted arg each: `open` · `ir <f>` ·
    /// `draw <f>` · `warmup <f>` · `png <f>` · `mp4 <from> <to>` · `logs`.
    #[arg(long = "step")]
    steps: Vec<String>,
}

fn env_json(envelope: &str) -> String {
    serde_json::to_string(envelope).expect("envelope json")
}

async fn drain(browser: &BrowserHarness) {
    for line in browser.browser_logs().await {
        println!("  console {line}");
    }
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
    let base_dir = cli
        .base_dir
        .clone()
        .or_else(|| input.parent().map(PathBuf::from));
    let envelope = web_envelope_for_oracle(&source, base_dir.as_deref())?;

    let Some(env) = BrowserTestEnv::detect()? else {
        anyhow::bail!("no browser env");
    };
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run(cli, &envelope))
}

async fn run(cli: Cli, envelope: &str) -> Result<()> {
    let repo = repo_root()?;
    let env = BrowserTestEnv::detect()?.expect("browser env checked in main");
    let server = WebAppServer::new(&repo)?;
    let browser = BrowserHarness::new(&env, 1920, 1080).await?;
    browser.navigate(&server.url("/test-oracle.html")).await?;
    drain(&browser).await;

    // Same preamble as web-eval's capture_canvas: grab the harness canvas
    // and create the probe/png canvases BEFORE open_design patches factories.
    browser
        .eval_js(
            "(() => { globalThis.__canvas = document.getElementById('frame'); \
             const pc = document.createElement('canvas'); pc.width = 1920; pc.height = 1080; \
             globalThis.__probeCtx = pc.getContext('2d', { willReadFrequently: true }); \
             globalThis.__pngCanvas = document.createElement('canvas'); \
             return globalThis.__canvas && globalThis.__probeCtx ? 'captured' : 'missing'; })()",
        )
        .await
        .context("capture canvas")?;

    let env = env_json(envelope);
    for step in &cli.steps {
        let js = match step.as_str() {
            "open" => format!("__opencatOracle.openDesign({env})"),
            "logs" => String::new(),
            s if s.starts_with("ir ") => {
                let f = s[3..].trim();
                format!(
                    "(() => {{ const r = __opencatOracle.buildFrameIr({f}); \
                     globalThis.__frame = r; \
                     return 'irLen=' + r.ir.length + ' plan=' + JSON.stringify(r.mediaPlan).slice(0, 120); }})()"
                )
            }
            s if s == "open2" => {
                format!("__opencatOracle.openDesign({env})")
            }
            s if s == "tprobe" => {
                r#"(() => new Promise(async (resolve) => {
                  const shimmed = String(setTimeout).includes('__ocTimers');
                  const flushRaf = typeof globalThis.__ocFlushRaf;
                  const runDue = typeof globalThis.__ocRunDueTimers;
                  const clockMs = typeof globalThis.__ocClockMs === 'function' ? globalThis.__ocClockMs() : null;
                  const rafFn = typeof window.requestAnimationFrame;
                  let rafN = 0;
                  const id = rafFn === 'function' ? window.requestAnimationFrame(function loop() { rafN++; if (rafN < 5) window.requestAnimationFrame(loop); }) : -1;
                  let fired = false;
                  setTimeout(() => { fired = true; }, 0);
                  const t0 = performance.now();
                  await new Promise((r) => setTimeout(r, 60));
                  if (id >= 0) window.cancelAnimationFrame(id);
                  resolve(JSON.stringify({ shimmed, flushRaf, runDue, clockMs, rafFn, rafN, fired, waited: Math.round(performance.now() - t0) }));
                }))()"#.to_string()
            }
            s if s == "kick" => {
                format!("__ocDbg.kickSession({env})")
            }
            s if s == "flags" => {
                "__ocDbg.flags()".to_string()
            }
            s if s == "oclogs" => {
                "(() => JSON.stringify({ logs: (globalThis.__ocLogs || []).slice(-12), unset: (globalThis.__ocUncaughtList || []).slice(-6) }))()".to_string()
            }
            s if s.starts_with("session") => {
                format!("__ocDbg.session({env})")
            }
            s if s.starts_with("sdraw ") => {
                let f = s[6..].trim();
                format!("__ocDbg.drawOnly({f})")
            }
            s if s.starts_with("draw ") => {
                let f = s[5..].trim();
                format!("__opencatOracle.drawFrame({env}, {f}).then(() => 'draw {f} done')")
            }
            s if s.starts_with("warmup ") => {
                let f = s[7..].trim();
                format!(
                    "__opencatOracle.warmupNativeAsync({env}, {f}).then(() => 'warmup {f} done')"
                )
            }
            s if s.starts_with("png ") => {
                let f = s[4..].trim();
                format!(
                    "__opencatOracle.captureCanvasPng({env}, {f}).then((b64) => \
                     'png {f} len=' + b64.length)"
                )
            }
            s if s.starts_with("mp4 ") => {
                let rest: Vec<&str> = s[4..].split_whitespace().collect();
                let (from, to) = (
                    rest.first().copied().unwrap_or("0"),
                    rest.get(1).copied().unwrap_or(rest[0]),
                );
                format!(
                    "__opencatOracle.renderFramesToMp4({env}, {from}, {to}).then((r) => \
                     'mp4 frames=' + r.framesEncoded + ' bytes=' + r.bytes)"
                )
            }
            s if s.starts_with("eval ") => s[5..].to_string(),
            other => anyhow::bail!("unknown step `{other}`"),
        };
        if js.is_empty() {
            println!("[step] drain-only");
        } else {
            println!("[step] {step}");
            match browser.eval_js(&js).await {
                Ok(v) => println!("[ok] {}", &v[..v.len().min(600)]),
                Err(e) => println!("[FAIL] {e}"),
            }
        }
        drain(&browser).await;
    }
    let _ = server;
    Ok(())
}
