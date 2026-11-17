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

/// Standard base64 decode (with padding), avoiding a dependency in a
/// throwaway driver. `A`–`Z`·`a`–`z`·`0`–`9`·`+`·`/`, `=` tail.
fn decode_b64(text: &str) -> Result<Vec<u8>> {
    fn val(b: u8) -> Result<u32> {
        match b {
            b'A'..=b'Z' => Ok((b - b'A') as u32),
            b'a'..=b'z' => Ok((b - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((b - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => anyhow::bail!("invalid base64 byte {b:#x}"),
        }
    }
    let clean: Vec<u8> = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks_exact(4) {
        let n = (val(chunk[0])? << 18) | (val(chunk[1])? << 12) | (val(chunk[2])? << 6) | val(chunk[3])?;
        out.push((n >> 16) as u8);
        out.push((n >> 8) as u8);
        out.push(n as u8);
    }
    match clean.len() % 4 {
        0 => {}
        2 => {
            let c = &clean[clean.len() - 2..];
            let n = (val(c[0])? << 18) | (val(c[1])? << 12);
            out.push((n >> 16) as u8);
        }
        3 => {
            let c = &clean[clean.len() - 3..];
            let n = (val(c[0])? << 18) | (val(c[1])? << 12) | (val(c[2])? << 6);
            out.push((n >> 16) as u8);
            out.push((n >> 8) as u8);
        }
        r => anyhow::bail!("base64 length mod 4 = {r}"),
    }
    Ok(out)
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
    // Page query passthrough (?traceStream=...) for in-page debug probes.
    let page = match std::env::var("OC_PAGE_QUERY") {
        Ok(q) if !q.is_empty() => format!("/test-oracle.html?{q}"),
        _ => "/test-oracle.html".to_string(),
    };
    browser.navigate(&server.url(&page)).await?;
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
            s if s.starts_with("dump ") => {
                let f = s[5..].trim();
                format!("__opencatOracle.dumpFrameOps({f}, 400)")
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
                // `png <f> [out.png]` — with a path, the b64 payload is
                // decoded and written to disk instead of printed.
                let rest = s[4..].trim();
                let (f, out) = match rest.split_once(char::is_whitespace) {
                    Some((f, out)) => (f.trim(), out.trim()),
                    None => (rest, ""),
                };
                if out.is_empty() {
                    format!(
                        "__opencatOracle.captureCanvasPng({env}, {f}).then((b64) => \
                         'png {f} len=' + b64.length)"
                    )
                } else {
                    format!(
                        "__opencatOracle.captureCanvasPng({env}, {f}).then((payload) => {{ \
                         window.__ocPngDump = payload; \
                         return 'png {f} staged len=' + payload.length; }})"
                    )
                }
            }
            s if s.starts_with("save ") => {
                // `save <out.png>` — fetch the staged b64 (from `png <f> staged`)
                // and write it to disk. Needs special-casing here because the
                // multi-MB payload must not cross the truncated println path.
                let out = s[5..].trim().to_string();
                let b64 = match browser
                    .eval_js("window.__ocPngDump || ''")
                    .await
                    .context("fetch staged png")?
                {
                    v if v.is_empty() => anyhow::bail!("no staged png (run `png <f> staged` first)"),
                    v => v,
                };
                let bytes = decode_b64(b64.trim().trim_matches('"'))?;
                std::fs::write(&out, &bytes).with_context(|| format!("write {out}"))?;
                println!("[step] save {out}");
                println!("[ok] {} bytes", bytes.len());
                drain(&browser).await;
                continue;
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
    // 显式收尾：delete session 让 chromedriver 关浏览器树。只靠 Drop 会留
    // 下孤儿 chrome（Drop 只杀 chromedriver 子进程，浏览器无人管）——残
    // 留浏览器慢性吃 GPU/共享内存映射，正是本轮 OOM 的来源。
    let _ = browser.shutdown().await;
    let _ = server;
    Ok(())
}
