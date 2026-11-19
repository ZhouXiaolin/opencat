//! Headless Chrome + static server harness for driving wasm renders.
//!
//! ChromeDriver opens the in-crate harness page (`web/test-oracle.html`),
//! which loads the opencat.js facade (wasm + CanvasKit) and renders frames of
//! a host-extracted HTML/JS dist envelope. Two transports are exposed:
//! - **hex** (sampled oracle): one frame per call, CanvasKit `readPixels` → raw
//!   RGBA hex. Exact, but ships 16.6 MB of hex per 1920×1080 frame and grows the
//!   page JS heap without bound → the tab OOMs after a few hundred frames.
//! - **video** (whole-range sweep): render a frame range in-page, encode with
//!   WebCodecs, mux with MP4Box.js, and return one H.264 MP4; the harness
//!   decodes it with ffmpeg. Only the compressed bitstream (a few MB total,
//!   bounded by the muxer) crosses the boundary, so a full 494-frame sweep
//!   needs no chunking. Codec loss is negligible on real content (measured:
//!   H.264 CRF 12 → mae 0.006–0.17, p8 ≈ 0 vs the ~1.64 baseline), so the
//!   k3diff pixel gate stays meaningful.
//!
//! Whole-video WebAV `exportMp4` is still intentionally not used: the facade
//! leaves `@webav/av-cliper` external and it pulls in OPFS + a DOM-bound
//! muxer worker; the oracle uses WebCodecs + MP4Box.js directly instead.
//!
//! 模块布局：`static_server`（TCP 静态服务 + 页面回传带外通道）、
//! `webdriver`（chromedriver/msedgedriver 进程与会话原语）、`harness`
//! （环境探测、页面驱动与响应解析）。

mod harness;
mod mem_gate;
mod static_server;
mod webdriver;

pub use harness::{BrowserHarness, BrowserTestEnv, FramesMp4, WebFrame};
pub use static_server::WebAppServer;

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

/// The web bridge has no extraction realm, so the oracle page receives the
/// host-extracted HTML envelope (`{extract, driver}`); wasm-side `from_extract`
/// rebuilds the composition from it. `base_dir` resolves relative scripts,
/// styles, modules and assets during extraction.
pub fn web_envelope_for_oracle(source: &str, base_dir: Option<&Path>) -> Result<String> {
    web_envelope_with_extract(source, base_dir).map(|(envelope, _)| envelope)
}

/// [`web_envelope_for_oracle`] plus the extract payload from the same
/// extraction: `extract.meta` carries the composition header (width/height/
/// fps/duration) that wasm-side `from_extract` reads, so callers needing the
/// metadata take it from here instead of re-running the HTML/V8 extraction.
pub fn web_envelope_with_extract(source: &str, base_dir: Option<&Path>) -> Result<(String, Value)> {
    let trimmed = source.trim_start();
    if !trimmed.starts_with('<') {
        return Err(anyhow!(
            "web oracle accepts HTML/dist documents only; compile JSX/Remotion sources to HTML/JS first"
        ));
    }
    let (extract, driver) =
        opencat_engine::script::html::extract_html_with_base_dir(source, base_dir)?;
    // Repo-relative baseDir (`examples/<example>`): the static server maps
    // `/assets/<baseDir>/...` back under the repo root, so an absolute
    // host path would leak into the URL. Compositions outside the repo
    // (hyperframes-launches is a sibling of it) resolve against the repo's
    // parent instead — the static server mirrors that fallback.
    let base_dir = base_dir.map(|p| {
        let root = repo_root().ok();
        root.as_deref()
            .and_then(|r| p.strip_prefix(r).ok().map(|rel| rel.to_path_buf()))
            .or_else(|| {
                let parent = root.as_deref().and_then(Path::parent);
                parent.and_then(|parent| p.strip_prefix(parent).ok().map(|rel| rel.to_path_buf()))
            })
            .unwrap_or_else(|| p.to_path_buf())
    });
    let envelope = serde_json::to_string(&json!({
        "extract": extract,
        "driver": driver,
        "html": true,
        // Composition-relative resource paths (`assets/...`) resolve against
        // this prefix; the oracle asset reader turns them into `/assets/`
        // URLs that the static server maps back under `repo/<baseDir>/...`.
        "baseDir": base_dir.map(|p| p.to_string_lossy().into_owned()),
    }))?;
    Ok((envelope, extract))
}

pub fn repo_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("failed to derive repo root from CARGO_MANIFEST_DIR"))
}
