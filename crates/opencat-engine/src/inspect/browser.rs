//! Headless Chrome + static server harness for web-vs-engine frame oracles.
//!
//! Shared by `scripts/compare-mp4.sh` via the `opencat-web-compare` binary
//! (sampled hex or whole-range video).
//!
//! The browser path always goes through `web/test-oracle.html`. Two transports
//! are exposed:
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

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    thread::JoinHandle,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Client;
use serde_json::{Value, json};

/// The web bridge has no JS realm, so the oracle page receives the
/// host-extracted JSX envelope (`{extract, driver}`); wasm-side `from_extract`
/// rebuilds the composition from it. `base_dir` resolves relative `import`s
/// during extraction.
pub fn web_envelope_for_oracle(source: &str, base_dir: Option<&Path>) -> Result<String> {
    let (extract, driver) = crate::jsx::extract_with_base_dir(source, base_dir)?;
    // Repo-relative baseDir (`examples/<example>`): the static server maps
    // `/assets/<baseDir>/...` back under the repo root, so an absolute
    // host path would leak into the URL.
    let base_dir = base_dir.map(|p| {
        repo_root()
            .ok()
            .and_then(|root| p.strip_prefix(&root).ok().map(|rel| rel.to_path_buf()))
            .unwrap_or_else(|| p.to_path_buf())
    });
    Ok(serde_json::to_string(&json!({
        "extract": extract,
        "driver": driver,
        // Composition-relative resource paths (`assets/...`) resolve against
        // this prefix; the oracle asset reader turns them into `/assets/`
        // URLs that the static server maps back under `repo/<baseDir>/...`.
        "baseDir": base_dir.map(|p| p.to_string_lossy().into_owned()),
    }))?)
}


pub fn repo_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("failed to derive repo root from CARGO_MANIFEST_DIR"))
}

pub struct WebAppServer {
    base_url: String,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl WebAppServer {
    pub fn new(repo: &Path) -> Result<Self> {
        let listener =
            TcpListener::bind("127.0.0.1:0").context("failed to bind web oracle server")?;
        let port = listener
            .local_addr()
            .context("failed to inspect web oracle server address")?
            .port();
        let base_url = format!("http://127.0.0.1:{port}");
        let routes = StaticRoutes::new(repo);
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = Arc::clone(&shutdown);
        let join = thread::spawn(move || {
            for stream in listener.incoming() {
                if server_shutdown.load(Ordering::Relaxed) {
                    break;
                }

                match stream {
                    Ok(stream) => {
                        let _ = handle_static_request(stream, &routes);
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            base_url,
            shutdown,
            join: Some(join),
        })
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

impl Drop for WebAppServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.base_url.strip_prefix("http://").unwrap_or_default());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

struct StaticRoutes {
    repo: PathBuf,
}

impl StaticRoutes {
    fn new(repo: &Path) -> Self {
        Self {
            repo: repo.to_path_buf(),
        }
    }

    fn resolve(&self, request_path: &str) -> Option<PathBuf> {
        // The oracle harness page.
        if request_path == "/" || request_path == "/test-oracle.html" {
            return Some(self.repo.join("web/test-oracle.html"));
        }

        // web-demuxer.wasm is resolved by the video-decode worker as
        // `new URL('../web-demuxer.wasm', self.location.href)` → `/wasm/web-demuxer.wasm`.
        // Must be checked BEFORE the generic `/wasm/` dist map — otherwise the
        // request falls through to `dist/web-demuxer.wasm` (missing) and every
        // video prepare silently fails (profile-showcase scene 2/3 blank).
        if request_path == "/wasm/web-demuxer.wasm" {
            return Some(
                self.repo
                    .join("web/node_modules/web-demuxer/dist/wasm-files/web-demuxer.wasm"),
            );
        }

        // Anything under /wasm/ is served from the built opencat.js facade
        // (crates/opencat-web/web/dist). This covers the facade bundle, its
        // hashed chunks, the wasm-pack glue, the bg wasm, and the worker —
        // without enumerating each file (hashed names change per build).
        if let Some(rest) = request_path.strip_prefix("/wasm/") {
            if rest.is_empty() || rest.contains("..") {
                return None;
            }
            return Some(self.repo.join("crates/opencat-web/web/dist").join(rest));
        }

        // MP4Box.js muxer for the video transport: `web/test-oracle.html` loads
        // it as a classic script (`<script src="/mp4box.js">` → global
        // `MP4Box`). Served straight from web/node_modules, same as CanvasKit.
        if request_path == "/mp4box.js" {
            return Some(
                self.repo
                    .join("web/node_modules/@webav/mp4box.js/dist/mp4box.all.js"),
            );
        }

        // CanvasKit (the wasm + its JS loader) comes from the dev app's
        // node_modules (installed via `bun install` in web/).
        if let Some(rest) = request_path.strip_prefix("/canvaskit/") {
            if rest.is_empty() || rest.contains("..") {
                return None;
            }
            return Some(
                self.repo
                    .join("web/node_modules/canvaskit-wasm/bin/full")
                    .join(rest),
            );
        }

        // Default fonts (NotoSansSC, NotoColorEmoji) live in the repo `assets/`
        // dir — the dev app serves it at `/fonts`. The oracle mirrors that.
        if let Some(rest) = request_path.strip_prefix("/fonts/") {
            if rest.is_empty() || rest.contains("..") {
                return None;
            }
            return Some(self.repo.join("assets").join(rest));
        }

        // Lottie bundle deps and other relative asset URLs resolve against the
        // repo `assets/` dir (the dev app serves these under `/assets`). The
        // oracle page prefixes composition-relative paths with the envelope
        // `baseDir`, so `rest` may also be repo-relative
        // (`examples/<example>/assets/...`).
        if let Some(rest) = request_path.strip_prefix("/assets/") {
            let rest = percent_decode(rest);
            if rest.is_empty() || rest.contains("..") {
                return None;
            }
            for candidate in [
                self.repo.join("assets").join(&rest),
                self.repo.join("examples").join(&rest),
                self.repo.join(&rest),
            ] {
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            // Nothing matched; hand back the repo assets pool so the caller
            // serves a clean 404 instead of dropping the connection.
            return Some(self.repo.join("assets").join(rest));
        }

        None
    }
}

fn handle_static_request(mut stream: TcpStream, routes: &StaticRoutes) -> Result<()> {
    let mut buffer = [0_u8; 8192];
    let len = stream.read(&mut buffer)?;
    if len == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..len]);
    let Some(first_line) = request.lines().next() else {
        return Ok(());
    };
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let raw_path = parts.next().unwrap_or_default();
    if method != "GET" && method != "HEAD" {
        write_response(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain",
            b"method not allowed",
        )?;
        return Ok(());
    }

    let request_path = raw_path.split('?').next().unwrap_or(raw_path);
    if let Some(upstream_path) = request_path.strip_prefix("/assets-proxy/") {
        return proxy_asset_request(&mut stream, method, upstream_path);
    }

    let Some(path) = routes.resolve(request_path) else {
        write_response(&mut stream, "404 Not Found", "text/plain", b"not found")?;
        return Ok(());
    };

    // A resolved-but-unreadable path must answer 404: closing the socket
    // without a response makes the browser fetch reject with a bare
    // "Failed to fetch", which hides which asset was missing.
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(_) => {
            write_response(&mut stream, "404 Not Found", "text/plain", b"not found")?;
            return Ok(());
        }
    };
    let content_type = content_type_for(&path);
    if method == "HEAD" {
        write_headers(&mut stream, "200 OK", content_type, bytes.len())?;
    } else {
        write_headers(&mut stream, "200 OK", content_type, bytes.len())?;
        stream.write_all(&bytes)?;
    }
    Ok(())
}

fn proxy_asset_request(stream: &mut TcpStream, method: &str, upstream_path: &str) -> Result<()> {
    if upstream_path.is_empty() || upstream_path.contains("..") {
        write_response(
            stream,
            "400 Bad Request",
            "text/plain",
            b"invalid asset path",
        )?;
        return Ok(());
    }

    let mut upstream = TcpStream::connect("127.0.0.1:8080")
        .context("connect to profile-showcase asset server at 127.0.0.1:8080")?;
    write!(
        upstream,
        "{method} /{upstream_path} HTTP/1.1\r\nHost: 127.0.0.1:8080\r\nConnection: close\r\n\r\n"
    )?;
    std::io::copy(&mut upstream, stream)?;
    Ok(())
}

/// Percent-decode a request path (`Frame%201.mp4` → `Frame 1.mp4`). The
/// browser percent-encodes spaces and other non-ASCII bytes before sending;
/// asset filenames do contain spaces.
fn percent_decode(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or_default(),
                16,
            ) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    write_headers(stream, status, content_type, body.len())?;
    stream.write_all(body)?;
    Ok(())
}

fn write_headers(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    content_len: usize,
) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {content_len}\r\n\
         Cross-Origin-Opener-Policy: same-origin\r\n\
         Cross-Origin-Embedder-Policy: require-corp\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\
         \r\n"
    )?;
    Ok(())
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "wasm" => "application/wasm",
        "css" => "text/css; charset=utf-8",
        "otf" => "font/otf",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

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

fn find_executable(env_var: &str, fallbacks: &[&str]) -> Option<PathBuf> {
    if let Some(value) = std::env::var_os(env_var) {
        let path = PathBuf::from(value);
        if path.exists() {
            return Some(path);
        }
    }

    for candidate in fallbacks {
        let path = PathBuf::from(candidate);
        if path.is_absolute() && path.exists() {
            return Some(path);
        }

        if Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return Some(path);
        }
    }

    None
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
            let mut command = Command::new(
                env.chromedriver_bin
                    .as_ref()
                    .expect("local chromedriver path should exist"),
            );
            command
                .arg(format!("--port={port}"))
                .stdout(Stdio::null())
                .stderr(Stdio::null());
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
    /// produced in-page by WebCodecs + MP4Box.js (see the module doc).
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
        parse_frames_mp4(parsed)
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

async fn wait_for_webdriver_ready(client: &Client, webdriver_url: &str) -> Result<()> {
    for _ in 0..50 {
        if let Ok(response) = client.get(format!("{webdriver_url}/status")).send().await
            && response.status().is_success()
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("chromedriver did not become ready");
}

async fn create_session(
    client: &Client,
    webdriver_url: &str,
    chrome_bin: Option<&PathBuf>,
    width: i32,
    height: i32,
) -> Result<String> {
    let mut chrome_options = json!({
        "args": [
            "--headless=new",
            "--hide-scrollbars",
            "--force-device-scale-factor=1",
            "--disable-dev-shm-usage",
            "--ignore-gpu-blocklist",
            "--enable-unsafe-swiftshader",
            "--use-gl=angle",
            "--use-angle=swiftshader",
            format!("--window-size={},{}", width.max(1), height.max(1)),
        ]
    });

    if let Some(binary) = chrome_bin {
        chrome_options["binary"] = Value::String(binary.to_string_lossy().to_string());
    }

    let response = client
        .post(format!("{webdriver_url}/session"))
        .json(&json!({
            "capabilities": {
                "alwaysMatch": {
                    "browserName": "chrome",
                    "goog:chromeOptions": chrome_options
                }
            }
        }))
        .send()
        .await
        .context("failed to create webdriver session")?;
    let body: Value = response
        .json()
        .await
        .context("failed to decode webdriver session response")?;

    body.get("sessionId")
        .and_then(Value::as_str)
        .or_else(|| {
            body.get("value")
                .and_then(|value| value.get("sessionId"))
                .and_then(Value::as_str)
        })
        .map(|id| id.to_string())
        .ok_or_else(|| anyhow!("webdriver session response missing session id: {body}"))
}

async fn wait_for_document_ready(
    client: &Client,
    webdriver_url: &str,
    session_id: &str,
) -> Result<()> {
    for _ in 0..100 {
        let result = webdriver_post(
            client,
            webdriver_url,
            session_id,
            "execute/sync",
            json!({
                "script": "return document.readyState;",
                "args": [],
            }),
        )
        .await?;
        if result.as_str() == Some("complete") {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }

    bail!("document did not reach readyState=complete");
}

async fn webdriver_post(
    client: &Client,
    webdriver_url: &str,
    session_id: &str,
    endpoint: &str,
    payload: Value,
) -> Result<Value> {
    let response = client
        .post(format!("{webdriver_url}/session/{session_id}/{endpoint}"))
        .json(&payload)
        .send()
        .await
        .with_context(|| format!("webdriver POST {endpoint} failed"))?;
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .with_context(|| format!("webdriver POST {endpoint} returned invalid JSON"))?;
    if !status.is_success() {
        bail!("webdriver POST {endpoint} failed with {status}: {body}");
    }
    Ok(body.get("value").cloned().unwrap_or(Value::Null))
}

fn reserve_port() -> Result<u16> {
    let listener =
        TcpListener::bind("127.0.0.1:0").context("failed to reserve a local TCP port")?;
    let port = listener
        .local_addr()
        .context("failed to inspect reserved TCP port")?
        .port();
    drop(listener);
    Ok(port)
}

pub struct WebFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

struct EngineFrame {
    frame: u32,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
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
/// timescale. Decode with [`decode_mp4_bytes_frames_rgba`].
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
    let b64 = value
        .get("videoBase64")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("browser video oracle result missing videoBase64 string"))?;
    use base64::Engine as _;
    let mp4 = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .context("decode browser video oracle base64 payload")?;
    if mp4.is_empty() {
        bail!("browser video oracle returned an empty MP4");
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

/// Decode an in-memory MP4 (from [`FramesMp4`]) to RGBA via ffmpeg.
///
/// The bytes are written to a temp file because ffmpeg pipes do not seek the
/// `moov` atom reliably; the oracle MP4 is at most a few MB.
pub fn decode_mp4_bytes_frames_rgba(
    mp4: &[u8],
    width: u32,
    height: u32,
    frames: &[u32],
    first_frame: u32,
) -> Result<std::collections::HashMap<u32, Vec<u8>>> {
    let mut tmp = tempfile::Builder::new()
        .prefix("opencat-oracle-")
        .suffix(".mp4")
        .tempfile()
        .context("create temp file for oracle MP4")?;
    tmp.write_all(mp4).context("write oracle MP4 to temp file")?;
    tmp.as_file_mut()
        .sync_all()
        .context("flush oracle MP4 temp file")?;
    decode_reference_frames_rgba(tmp.path(), width, height, frames, first_frame)
}

#[allow(dead_code)] // retained for per-pixel diagnostics; the oracle uses SSIM
struct DiffStats {
    mismatched_pixels: usize,
    mismatched_pixel_ratio: f64,
    max_channel_delta: u8,
    mean_abs_channel_delta: f64,
}

#[allow(dead_code)] // retained for per-pixel diagnostics; the oracle uses SSIM
fn compare_rgba(expected: &[u8], actual: &[u8], channel_tolerance: u8) -> Result<DiffStats> {
    if expected.len() != actual.len() {
        bail!(
            "rgba length mismatch: expected {} bytes, got {} bytes",
            expected.len(),
            actual.len()
        );
    }
    if expected.len() % 4 != 0 {
        bail!("rgba length is not divisible by 4: {}", expected.len());
    }

    let mut mismatched_pixels = 0usize;
    let mut max_channel_delta = 0u8;
    let mut abs_delta_sum = 0u64;

    for (expected_px, actual_px) in expected.chunks_exact(4).zip(actual.chunks_exact(4)) {
        let mut pixel_mismatched = false;
        for (&e, &a) in expected_px.iter().zip(actual_px) {
            let delta = e.abs_diff(a);
            max_channel_delta = max_channel_delta.max(delta);
            abs_delta_sum += u64::from(delta);
            if delta > channel_tolerance {
                pixel_mismatched = true;
            }
        }
        if pixel_mismatched {
            mismatched_pixels += 1;
        }
    }

    let pixel_count = expected.len() / 4;
    Ok(DiffStats {
        mismatched_pixels,
        mismatched_pixel_ratio: mismatched_pixels as f64 / pixel_count.max(1) as f64,
        max_channel_delta,
        mean_abs_channel_delta: abs_delta_sum as f64 / expected.len().max(1) as f64,
    })
}

/// Hard pixel-level difference between two RGBA buffers — the native
/// equivalent of `tools/k3diff.py` (per-channel max delta, not SSIM).
///
/// SSIM collapses to a single "structural similarity" scalar and hides
/// localized misalignment; the alignment methodology uses this instead so
/// every frame is judged by `mae` / `maxd` / per-threshold changed-pixel
/// fractions, exactly like the reference gate.
#[derive(Debug, Clone, Copy)]
pub struct PixelDiff {
    /// Mean absolute error across R,G,B (0..255).
    pub mae: f64,
    /// Max per-pixel channel delta in the frame.
    pub maxd: u32,
    /// Fraction of pixels whose max-channel delta is above
    /// `[2, 4, 8, 16, 32, 64, 128]` (k3diff `p2`..`p128`).
    pub frac_above: [f64; 7],
    /// Bounding box (x0,y0,x1,y1) of pixels above the 16 delta threshold.
    pub bbox: Option<(u32, u32, u32, u32)>,
}

/// k3diff threshold ladder (excludes the `p0` = "changed at all" bucket).
pub const PIXEL_DIFF_THRESHOLDS: [u32; 7] = [2, 4, 8, 16, 32, 64, 128];

/// Compare two RGBA buffers with hard pixel metrics (k3diff semantics).
pub fn compute_pixel_diff_rgba(
    a: &[u8],
    b: &[u8],
    width: u32,
    height: u32,
) -> Result<PixelDiff> {
    let pixels = width as usize * height as usize;
    let expected = pixels * 4;
    if a.len() != expected || b.len() != expected {
        bail!(
            "pixel diff expects {expected} bytes per buffer, got a={} b={}",
            a.len(),
            b.len()
        );
    }

    let mut sum: u64 = 0;
    let mut maxd: u32 = 0;
    let mut counts = [0u64; PIXEL_DIFF_THRESHOLDS.len()];
    let mut bbox: Option<(u32, u32, u32, u32)> = None;

    for i in 0..pixels {
        let base = i * 4;
        let mut dm: u32 = 0;
        // k3diff max-channel delta is over R,G,B only (alpha excluded).
        for c in 0..3 {
            let d = a[base + c].abs_diff(b[base + c]) as u32;
            sum += u64::from(d);
            dm = dm.max(d);
        }
        maxd = maxd.max(dm);
        for (k, t) in PIXEL_DIFF_THRESHOLDS.iter().enumerate() {
            if dm > *t {
                counts[k] += 1;
            }
        }
        // bbox uses the k3diff default bbox threshold (> 16).
        if dm > 16 {
            let x = (i % width as usize) as u32;
            let y = (i / width as usize) as u32;
            bbox = Some(match bbox {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
    }

    let mae = sum as f64 / (pixels as f64 * 3.0);
    let frac_above = counts.map(|c| c as f64 / pixels as f64);
    Ok(PixelDiff {
        mae,
        maxd,
        frac_above,
        bbox,
    })
}

/// Decode selected frames from a reference video (e.g. the hyperframes-launch
/// `k3-promo.mp4`) as RGBA at `width`x`height`, streaming the file once.
///
/// This lets the web (wasm + CanvasKit) path be pixel-aligned **directly
/// against the original reference render**, not merely against the native
/// engine — which surfaces wasm-specific problems the engine-vs-web oracle
/// cannot (both sides would share an engine bug). Frames past the end of the
/// video are omitted from the result. ffmpeg is required on PATH.
pub fn decode_reference_frames_rgba(
    path: &Path,
    width: u32,
    height: u32,
    frames: &[u32],
    first_frame: u32,
) -> Result<std::collections::HashMap<u32, Vec<u8>>> {
    use std::collections::{BTreeSet, HashMap};

    let mut wanted: BTreeSet<u32> = frames.iter().copied().collect();
    let Some(max_frame) = wanted.iter().next_back().copied() else {
        return Ok(HashMap::new());
    };

    let mut child = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args([
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &format!("{width}x{height}"),
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("spawn ffmpeg to decode {}", path.display()))?;
    let mut stdout = child
        .stdout
        .take()
        .context("ffmpeg stdout should be piped")?;

    let frame_len = width as usize * height as usize * 4;
    let mut buf = vec![0u8; frame_len];
    let mut out = HashMap::new();
    // Stream order ≠ composition frame number: a web-video MP4 rendered from
    // a mid-range START_FRAME stores its first frame at stream index 0 but is
    // tagged `first_frame`, so offsets every decoded frame into absolute
    // composition frame numbers before matching `wanted`.
    let mut idx: u32 = 0;
    loop {
        if stdout.read_exact(&mut buf).is_err() {
            break;
        }
        let frame_no = first_frame.saturating_add(idx);
        if wanted.remove(&frame_no) {
            out.insert(frame_no, buf.clone());
        }
        if frame_no >= max_frame {
            break;
        }
        idx += 1;
    }
    drop(stdout);
    let _ = child.wait();
    Ok(out)
}


pub fn write_artifacts(
    dir: &Path,
    width: u32,
    height: u32,
    expected: &[u8],
    actual: &[u8],
) -> Result<()> {
    fs::create_dir_all(dir)?;
    write_png(&dir.join("engine.png"), width, height, expected)?;
    write_png(&dir.join("web.png"), width, height, actual)?;

    let mut diff = Vec::with_capacity(expected.len());
    for (&e, &a) in expected.iter().zip(actual) {
        diff.push(e.abs_diff(a).saturating_mul(8));
    }
    write_png(&dir.join("diff.png"), width, height, &diff)
}

pub fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let image = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| anyhow!("failed to build png image buffer for {}", path.display()))?;
    image
        .save(path)
        .with_context(|| format!("save {}", path.display()))
}
