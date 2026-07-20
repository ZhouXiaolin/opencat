//! Browser render oracle tests for comparing the Web CanvasKit path against
//! the native engine renderer.

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    thread::JoinHandle,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Client;
use serde_json::{Value, json};

use crate::render::render_single_frame_from_jsonl_with_base;

/// Minimum SSIM for a web frame to pass against the engine reference. Matches
/// the engine-vs-engine target of `compare-ssim.sh` (1.000000) with a small
/// allowance for sub-pixel CanvasKit-vs-Skia rasterization differences.
const MIN_SSIM: f64 = 0.99;
const LOTTIE_MIN_SSIM: f64 = 0.985;
static SSIM_TEMP_ID: AtomicU64 = AtomicU64::new(0);

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

fn web_source_for_oracle(path: &str, source: &str) -> String {
    match path {
        "examples/web-oracle-font.xml" => source.replace("path=\"../assets/", "url=\"/fonts/"),
        _ => source.to_string(),
    }
}

fn repo_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("failed to derive repo root from CARGO_MANIFEST_DIR"))
}

struct WebAppServer {
    base_url: String,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl WebAppServer {
    fn new(repo: &Path) -> Result<Self> {
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

    fn url(&self, path: &str) -> String {
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

        // CanvasKit (the wasm + its JS loader) and web-demuxer wasm come from
        // the dev app's node_modules (installed via `bun install` in web/).
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
        if request_path == "/wasm/web-demuxer.wasm" {
            return Some(
                self.repo
                    .join("web/node_modules/web-demuxer/dist/wasm-files/web-demuxer.wasm"),
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
        // repo `assets/` dir (the dev app serves these under `/assets`).
        if let Some(rest) = request_path.strip_prefix("/assets/") {
            if rest.is_empty() || rest.contains("..") {
                return None;
            }
            let asset = self.repo.join("assets").join(rest);
            return Some(if asset.exists() {
                asset
            } else {
                self.repo.join("examples").join(rest)
            });
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

    let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
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

struct BrowserTestEnv {
    webdriver_url: Option<String>,
    chromedriver_bin: Option<PathBuf>,
    chrome_bin: Option<PathBuf>,
}

impl BrowserTestEnv {
    fn detect() -> Result<Option<Self>> {
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

struct BrowserHarness {
    client: Client,
    webdriver_url: String,
    session_id: String,
    child: Option<Child>,
}

impl BrowserHarness {
    async fn new(env: &BrowserTestEnv, width: i32, height: i32) -> Result<Self> {
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
                "script": 120000,
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

    async fn navigate(&self, url: &str) -> Result<()> {
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

    async fn render_frame(&self, jsonl: &str, frame: u32) -> Result<WebFrame> {
        let result = webdriver_post(
            &self.client,
            &self.webdriver_url,
            &self.session_id,
            "execute/async",
            json!({
                "script": r#"
                    const jsonl = arguments[0];
                    const frame = arguments[1];
                    const done = arguments[arguments.length - 1];
                    if (!window.__opencatOracle || typeof window.__opencatOracle.renderFrame !== 'function') {
                      done({ ok: false, error: 'window.__opencatOracle.renderFrame is not available' });
                      return;
                    }
                    window.__opencatOracle.renderFrame(jsonl, frame)
                      .then((result) => done({ ok: true, result }))
                      .catch((err) => done({ ok: false, error: String(err && (err.stack || err.message) || err) }));
                "#,
                "args": [jsonl, frame],
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

    async fn shutdown(mut self) -> Result<()> {
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

struct WebFrame {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
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

/// Compute the SSIM between two RGBA buffers using ffmpeg, matching the metric
/// used by `scripts/compare-ssim.sh` (engine-vs-engine). Each buffer is written
/// to a temporary PNG and compared via `ffmpeg -filter_complex ssim`; the
/// returned value is the "All" SSIM (1.0 = identical).
///
/// This is the per-frame web-vs-engine regression metric for the host-owned
/// web pipeline (#8). ffmpeg is required on PATH.
fn compute_ssim_rgba(a: &[u8], b: &[u8], width: u32, height: u32) -> Result<f64> {
    let temp_id = SSIM_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!(
        "opencat-web-oracle-ssim-{}-{temp_id}",
        std::process::id()
    ));
    fs::create_dir_all(&tmp)?;
    let a_png = tmp.join("engine.png");
    let b_png = tmp.join("web.png");
    write_png(&a_png, width, height, a)?;
    write_png(&b_png, width, height, b)?;

    let output = Command::new("ffmpeg")
        .args([
            "-i",
            &a_png.to_string_lossy(),
            "-i",
            &b_png.to_string_lossy(),
            "-filter_complex",
            "ssim",
            "-f",
            "null",
            "-",
        ])
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .output()
        .context("run ffmpeg ssim")?;

    let _ = fs::remove_dir_all(&tmp);

    let stderr = String::from_utf8_lossy(&output.stderr);
    // ffmpeg prints `SSIM avg: ... (...): All:1.000000 (...)` — capture the
    // `All:<value>` token (same field compare-ssim.sh greps).
    let line = stderr
        .lines()
        .rev()
        .find(|l| l.contains("SSIM") && l.contains("All:"))
        .ok_or_else(|| anyhow!("ffmpeg produced no SSIM line:\n{stderr}"))?;
    let after = line
        .split("All:")
        .nth(1)
        .ok_or_else(|| anyhow!("malformed SSIM line: {line}"))?;
    let value = after
        .split(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("could not parse SSIM value from: {line}"))?;
    value
        .parse::<f64>()
        .with_context(|| format!("parse SSIM value `{value}`"))
}

fn write_artifacts(
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

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let image = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| anyhow!("failed to build png image buffer for {}", path.display()))?;
    image
        .save(path)
        .with_context(|| format!("save {}", path.display()))
}
