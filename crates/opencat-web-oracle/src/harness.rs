//! 浏览器 oracle：环境探测（`BrowserTestEnv`）、页面驱动（`BrowserHarness`）
//! 与响应解析（`WebFrame` / `FramesMp4`）。渲染本体在 harness 页
//! （`web/test-oracle.html`）里，本模块只经 WebDriver 调用页面暴露的
//! `window.__opencatOracle` 合同（TS 侧单一 owner：
//! `opencat-web/web/src/oracle-contract.ts`）。

use std::{fs, path::PathBuf, process::Child};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Client;
use serde_json::{Value, json};

use crate::static_server::oracle_video_path;
use crate::webdriver::{
    create_session, find_executable, reserve_port, wait_for_document_ready,
    wait_for_webdriver_ready, webdriver_post,
};

pub struct BrowserTestEnv {
    webdriver_url: Option<String>,
    chromedriver_bin: Option<PathBuf>,
    chrome_bin: Option<PathBuf>,
}

impl BrowserTestEnv {
    pub fn detect() -> Result<Option<Self>> {
        if let Ok(webdriver_url) = std::env::var("CHROMEDRIVER_URL") {
            return Ok(Some(Self {
                webdriver_url: Some(webdriver_url),
                chromedriver_bin: None,
                chrome_bin: None,
            }));
        }

        let chromedriver_bin = find_executable(
            "CHROMEDRIVER_BIN",
            &[
                "/opt/homebrew/bin/chromedriver",
                "/usr/local/bin/chromedriver",
                "/usr/bin/chromium-driver",
                "chromedriver",
            ],
        );
        let chrome_bin = find_executable(
            "CHROME_BIN",
            &[
                "/usr/bin/google-chrome",
                "/usr/bin/google-chrome-stable",
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
                "/snap/bin/chromium",
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
                "google-chrome",
                "chromium",
                "chromium-browser",
            ],
        )
        .or_else(|| find_executable("GOOGLE_CHROME_BIN", &[]));

        if chromedriver_bin.is_none() || chrome_bin.is_none() {
            return Ok(None);
        }

        Ok(Some(Self {
            webdriver_url: None,
            chromedriver_bin,
            chrome_bin,
        }))
    }
}

pub struct BrowserHarness {
    client: Client,
    webdriver_url: String,
    session_id: String,
    child: Option<Child>,
}

impl BrowserHarness {
    pub async fn new(env: &BrowserTestEnv, width: i32, height: i32) -> Result<Self> {
        let client = Client::builder()
            .build()
            .context("failed to build webdriver HTTP client")?;

        let (webdriver_url, child) = if let Some(url) = &env.webdriver_url {
            (url.clone(), None)
        } else {
            let port = reserve_port()?;
            let webdriver_url = format!("http://127.0.0.1:{port}");
            let mut command = std::process::Command::new(
                env.chromedriver_bin
                    .as_ref()
                    .expect("local chromedriver path should exist"),
            );
            command
                .arg(format!("--port={port}"))
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            let child = command.spawn().context("failed to spawn chromedriver")?;
            wait_for_webdriver_ready(&client, &webdriver_url).await?;
            (webdriver_url, Some(child))
        };

        let session_id = create_session(
            &client,
            &webdriver_url,
            env.chrome_bin.as_ref(),
            width,
            height,
        )
        .await?;
        webdriver_post(
            &client,
            &webdriver_url,
            &session_id,
            "timeouts",
            json!({
                "implicit": 0,
                "pageLoad": 120000,
                // In-page whole-movie MP4 encodes run in ONE async script;
                // heavy content (SkSL shader segments) can take seconds per
                // frame, far past the 120s W3C default.
                "script": 3_600_000,
            }),
        )
        .await?;

        Ok(Self {
            client,
            webdriver_url,
            session_id,
            child,
        })
    }

    pub async fn navigate(&self, url: &str) -> Result<()> {
        webdriver_post(
            &self.client,
            &self.webdriver_url,
            &self.session_id,
            "url",
            json!({ "url": url }),
        )
        .await?;
        wait_for_document_ready(&self.client, &self.webdriver_url, &self.session_id).await
    }

    /// Evaluate one async JS snippet in the page (result of the last
    /// expression is JSON-serialized back). Debug escape hatch — keeps the
    /// full `err.stack` that the typed render methods flatten away.
    pub async fn eval_js(&self, script: &str) -> Result<String> {
        let result = webdriver_post(
            &self.client,
            &self.webdriver_url,
            &self.session_id,
            "execute/async",
            json!({
                "script": format!(
                    r#"
                    const userScript = arguments[0];
                    const done = arguments[arguments.length - 1];
                    (async () => {{
                        // eslint-disable-next-line no-eval
                        return eval(userScript);
                    }})()
                      .then((v) => done({{ ok: true, value: v === undefined ? null : JSON.stringify(v) }}))
                      .catch((err) => done({{ ok: false, error: String(err && (err.stack || err.message) || err) }}));
                    "#
                ),
                "args": [script],
            }),
        )
        .await?;

        if result.get("ok").and_then(Value::as_bool) != Some(true) {
            bail!(
                "{}",
                result
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("eval_js returned an unknown error")
            );
        }
        Ok(result
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string())
    }

    pub async fn render_frame(&self, envelope: &str, frame: u32) -> Result<WebFrame> {
        let result = webdriver_post(
            &self.client,
            &self.webdriver_url,
            &self.session_id,
            "execute/async",
            json!({
                "script": r#"
                    const envelope = arguments[0];
                    const frame = arguments[1];
                    const done = arguments[arguments.length - 1];
                    if (!window.__opencatOracle || typeof window.__opencatOracle.renderFrame !== 'function') {
                      done({ ok: false, error: 'window.__opencatOracle.renderFrame is not available' });
                      return;
                    }
                    window.__opencatOracle.renderFrame(envelope, frame)
                      .then((result) => done({ ok: true, result }))
                      .catch((err) => done({ ok: false, error: String(err && (err.stack || err.message) || err) }));
                "#,
                "args": [envelope, frame],
            }),
        )
        .await?;

        if result.get("ok").and_then(Value::as_bool) != Some(true) {
            bail!(
                "{}",
                result
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("browser oracle returned an unknown error")
            );
        }

        parse_web_frame(
            result
                .get("result")
                .ok_or_else(|| anyhow!("browser oracle response missing result"))?,
        )
    }

    /// Render `[start_frame, end_frame]` (inclusive) and return one H.264 MP4
    /// produced in-page by WebCodecs + MP4Box.js (see the crate doc).
    ///
    /// Unlike [`Self::render_frame`], only the compressed bitstream crosses the
    /// WebDriver boundary, so this scales to a full sweep without chunking.
    pub async fn render_frames_to_mp4(
        &self,
        envelope: &str,
        start_frame: u32,
        end_frame: u32,
    ) -> Result<FramesMp4> {
        // Diagnostic channel: with OPENCAT_ORACLE_RENDER_ONLY set, every frame
        // is drawn but nothing is encoded or muxed — splits render-side memory
        // growth from the encode path when a continuous sweep aborts.
        let render_only = std::env::var("OPENCAT_ORACLE_RENDER_ONLY").is_ok();
        let result = webdriver_post(
            &self.client,
            &self.webdriver_url,
            &self.session_id,
            "execute/async",
            json!({
                "script": r#"
                    const envelope = arguments[0];
                    const start = arguments[1];
                    const end = arguments[2];
                    const renderOnly = arguments[3] === true;
                    const done = arguments[arguments.length - 1];
                    if (!window.__opencatOracle || typeof window.__opencatOracle.renderFramesToMp4 !== 'function') {
                      done({ ok: false, error: 'window.__opencatOracle.renderFramesToMp4 is not available' });
                      return;
                    }
                    window.__opencatOracle.renderFramesToMp4(envelope, start, end, renderOnly)
                      .then((result) => done({ ok: true, result }))
                      .catch((err) => done({ ok: false, error: String(err && (err.stack || err.message) || err) }));
                "#,
                "args": [envelope, start_frame, end_frame, render_only],
            }),
        )
        .await?;

        if result.get("ok").and_then(Value::as_bool) != Some(true) {
            bail!(
                "{}",
                result
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("browser oracle returned an unknown error")
            );
        }

        let parsed = result
            .get("result")
            .ok_or_else(|| anyhow!("browser oracle video response missing result"))?;
        if render_only {
            let frames_encoded = parse_u32(parsed, "framesEncoded").unwrap_or(0);
            eprintln!(
                "  render-only sweep finished: {frames_encoded} frames; probe={}",
                parsed
                    .get("probe")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "[]".into())
            );
            return Ok(FramesMp4 {
                width: 0,
                height: 0,
                fps: 30,
                first_frame: start_frame,
                frames_encoded,
                mp4: Vec::new(),
            });
        }
        if let Some(ft) = parsed.get("frameTimings") {
            eprintln!("  frameTimings={ft}");
        }
        parse_frames_mp4(parsed)
    }

    /// Drain the browser console log (chromedriver `browser` log type).
    /// Session capabilities enable `goog:loggingPrefs` so WebGL/shader
    /// diagnostics reach this — call after a failed or suspicious render.
    pub async fn browser_logs(&self) -> Vec<String> {
        let response = self
            .client
            .post(format!(
                "{}/session/{}/se/log",
                self.webdriver_url, self.session_id
            ))
            .json(&serde_json::json!({ "type": "browser" }))
            .send()
            .await;
        let Ok(response) = response else {
            return Vec::new();
        };
        let Ok(body) = response.json::<Value>().await else {
            return Vec::new();
        };
        body.get("value")
            .and_then(Value::as_array)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|e| {
                        let level = e.get("level").and_then(Value::as_str)?;
                        let msg = e.get("message").and_then(Value::as_str)?;
                        Some(format!("[{level}] {msg}"))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub async fn shutdown(mut self) -> Result<()> {
        let _ = self
            .client
            .delete(format!(
                "{}/session/{}",
                self.webdriver_url, self.session_id
            ))
            .send()
            .await;

        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        Ok(())
    }
}

impl Drop for BrowserHarness {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub struct WebFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

fn parse_web_frame(value: &Value) -> Result<WebFrame> {
    let width = parse_u32(value, "width")?;
    let height = parse_u32(value, "height")?;
    let expected_len = width as usize * height as usize * 4;
    let rgba_hex = value
        .get("rgbaHex")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("browser oracle result missing rgbaHex string"))?;
    let rgba = decode_hex_rgba(rgba_hex, expected_len)?;

    Ok(WebFrame {
        width,
        height,
        rgba,
    })
}

fn decode_hex_rgba(hex: &str, expected_len: usize) -> Result<Vec<u8>> {
    if hex.len() != expected_len * 2 {
        bail!(
            "browser oracle rgbaHex length {} does not match expected byte length {}",
            hex.len(),
            expected_len
        );
    }

    let mut rgba = Vec::with_capacity(expected_len);
    for pair in hex.as_bytes().chunks_exact(2) {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        rgba.push((high << 4) | low);
    }
    Ok(rgba)
}

fn hex_nibble(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => bail!(
            "invalid hex digit in browser oracle rgbaHex: {}",
            byte as char
        ),
    }
}

fn parse_u32(value: &Value, key: &str) -> Result<u32> {
    let number = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("browser oracle result missing numeric field `{key}`: {value}"))?;
    u32::try_from(number).with_context(|| format!("field `{key}` is out of range: {number}"))
}

/// One in-page-encoded H.264 MP4 covering a contiguous frame range, returned by
/// [`BrowserHarness::render_frames_to_mp4`]. `first_frame` is the composition
/// index of sample 0 (0 unless the range was chunked); `fps` is the track
/// timescale. Feed `mp4` to a video comparator (e.g. `tools/k3diff.py`).
pub struct FramesMp4 {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub first_frame: u32,
    pub frames_encoded: u32,
    pub mp4: Vec<u8>,
}

fn parse_frames_mp4(value: &Value) -> Result<FramesMp4> {
    let width = parse_u32(value, "width")?;
    let height = parse_u32(value, "height")?;
    let fps = parse_u32(value, "fps")?;
    let first_frame = parse_u32(value, "firstFrame")?;
    let frames_encoded = parse_u32(value, "framesEncoded")?;
    // The bitstream arrives via the page's `POST /oracle-video` (handled by
    // the static server); the WebDriver result carries metadata only. `bytes`
    // is the page-side MP4 length — a mismatch means the POST was truncated
    // and the file must not be trusted.
    let mp4 = fs::read(oracle_video_path())
        .context("read oracle video posted by the page (POST /oracle-video)")?;
    if mp4.is_empty() {
        bail!("browser video oracle posted an empty MP4");
    }
    let expected = parse_u32(value, "bytes")?;
    if mp4.len() as u64 != u64::from(expected) {
        bail!(
            "oracle video size mismatch: posted {} bytes, page reported {expected}",
            mp4.len()
        );
    }
    Ok(FramesMp4 {
        width,
        height,
        fps,
        first_frame,
        frames_encoded,
        mp4,
    })
}
