//! Oracle harness 的静态服务器：一处 TCP 监听供页面与资产两条通道。
//!
//! 路由三族：
//! - harness 页与依赖（`/`、`/wasm/`、`/mp4box.js`、`/canvaskit/`、`/fonts/`）
//! - 资产（`/assets/`，按 envelope `baseDir` 映射回 repo）
//! - 页面回传的带外大块（`POST /oracle-video`、`POST /oracle-blob`）——
//!   多 MB 位流不过 WebDriver JSON 边界，走 loopback 落临时文件。

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    thread::JoinHandle,
};

use anyhow::{Context, Result, anyhow};

/// The oracle harness page ships with this crate, so the static server serves
/// it independently of where the wasm facade or the node_modules deps live.
fn harness_page() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("web")
        .join("test-oracle.html")
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
            return Some(harness_page());
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

    // Split headers from any body bytes already read. Header text is decoded
    // as UTF-8; body bytes (the oracle MP4 POST) must stay raw.
    let header_end = buffer[..len]
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map_or(len, |p| p + 4);
    let request = String::from_utf8_lossy(&buffer[..header_end]);
    let Some(first_line) = request.lines().next() else {
        return Ok(());
    };
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let raw_path = parts.next().unwrap_or_default();
    if method == "POST" && raw_path.split('?').next() == Some("/oracle-video") {
        return handle_oracle_video_post(&mut stream, &request, &buffer[header_end..len]);
    }
    // Debug sink for the eval_js channel (`drawFramePng`): a single binary
    // blob (e.g. a PNG of one frame) too big for the WebDriver JSON boundary.
    // The response is the temp file path so the driver knows what to read.
    if method == "POST" && raw_path.split('?').next() == Some("/oracle-blob") {
        // Optional ?f=N suffix names the sink per frame, so a sweep can POST
        // every frame's PNG without each overwriting the last.
        let file = raw_path
            .split_once('?')
            .and_then(|(_, q)| q.strip_prefix("f=").map(|n| format!("opencat-oracle-blob-{n}.bin")))
            .unwrap_or_else(|| "opencat-oracle-blob.bin".to_string());
        let path = std::env::temp_dir().join(file);
        return handle_oracle_video_post_at(&mut stream, &request, &buffer[header_end..len], &path);
    }
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

/// Sink for the oracle page's `POST /oracle-video`: the in-page-encoded MP4
/// bitstream. Written to a fixed temp path that `harness::parse_frames_mp4`
/// reads back, so the multi-MB payload never crosses the WebDriver JSON
/// boundary.
pub(crate) fn oracle_video_path() -> PathBuf {
    std::env::temp_dir().join("opencat-oracle-video.mp4")
}

fn handle_oracle_video_post(
    stream: &mut TcpStream,
    head: &str,
    body_prefix: &[u8],
) -> Result<()> {
    handle_oracle_video_post_at(stream, head, body_prefix, &oracle_video_path())
}

fn handle_oracle_video_post_at(
    stream: &mut TcpStream,
    head: &str,
    body_prefix: &[u8],
    path: &std::path::Path,
) -> Result<()> {
    // fetch keeps the connection alive after the body, so read exactly
    // Content-Length bytes and then answer — reading to EOF would block.
    let content_length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .ok_or_else(|| anyhow!("oracle blob POST missing content-length"))?;
    let mut mp4 = body_prefix.to_vec();
    let mut chunk = [0_u8; 65_536];
    while mp4.len() < content_length {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        mp4.extend_from_slice(&chunk[..n]);
    }
    if mp4.len() < content_length {
        write_response(stream, "400 Bad Request", "text/plain", b"truncated body")?;
        return Ok(());
    }
    fs::write(path, &mp4)?;
    write_response(stream, "200 OK", "text/plain", b"ok")?;
    Ok(())
}

/// Legacy debug shim for sources that hard-code `http://127.0.0.1:8080/...`
/// asset URLs (profile-showcase era): the wasm fetch layer rewrites them to
/// `/assets-proxy/...` (`opencat-web/src/resource/fetch.rs` PROXY_MAP) and the
/// dev app proxies the same prefix in `web/vite.config.ts`. Retire all three
/// legs together once no example source references the bare 8080 origin.
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
