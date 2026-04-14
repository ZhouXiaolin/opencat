//! Browser layout tests that verify Tailwind CSS utilities produce identical layouts
//! in Chrome (via ChromeDriver) and Taffy.
//!
//! # Test Structure
//!
//! - **Auto-generated tests**: `GENERATED_LAYOUT_GROUP_SPECS` creates fixtures from
//!   `testsupport/utilities.test.ts` to test individual utility classes.
//! - **Manual fixtures**: `browser_layout_fixtures()` contains a small set of unique
//!   integration scenarios not covered by auto-generated tests.
//! - **Integration tests**: Complex multi-utility combinations are in
//!   `browser_layout_integration_tests.rs`.
//!
//! # Running Tests
//!
//! Requires ChromeDriver and Chrome to be installed.
//!
//! ```bash
//! cargo test chromedriver_tailwind_layout_matches_taffy
//! cargo test chromedriver_tailwind_extended_flex_layout_matches_taffy
//! ```

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Client;
use serde_json::{Value, json};

use crate::{
    Composition, RenderSession, collect_frame_layout_rects,
    inspect::FrameElementRect,
    jsonl::tailwind::parse_class_name,
    scene::primitives::{div, text},
};

#[test]
fn chromedriver_tailwind_layout_matches_taffy() -> Result<()> {
    let mut fixtures = browser_layout_fixtures();
    fixtures.extend(browser_layout_integration_fixtures());
    run_browser_layout_suite(fixtures)
}

use super::browser_layout_integration_tests::browser_layout_integration_fixtures;

#[test]
fn chromedriver_tailwind_extended_flex_layout_matches_taffy() -> Result<()> {
    run_browser_layout_suite(generated_layout_coverage_fixtures()?)
}

#[test]
fn generated_layout_fixture_templates_cover_utilities_manifest() -> Result<()> {
    let report = generated_layout_coverage_report()?;
    if report.uncovered.is_empty() {
        return Ok(());
    }

    bail!(
        "layout utility coverage gaps in browser fixture generator:\n{}",
        report.uncovered.join("\n")
    )
}

fn run_browser_layout_suite(fixtures: Vec<LayoutFixture>) -> Result<()> {
    let Some(env) = BrowserTestEnv::detect()? else {
        eprintln!(
            "skipping chromedriver Tailwind layout suite: ChromeDriver or Chrome is unavailable"
        );
        return Ok(());
    };

    let runtime = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;

    runtime.block_on(async move {
        let browser = BrowserHarness::new(&env).await?;
        let mut failures = Vec::new();

        for fixture in fixtures {
            let text_ids = fixture.root.collect_text_ids();
            let css = compile_tailwind_css(&fixture)?;
            let html = fixture.render_html_document(&css);
            let html_path = write_fixture_file(&fixture.name, "html", &html)?;
            let browser_rects = browser.measure_layout(&html_path).await?;
            let taffy_rects = measure_taffy_layout(&fixture)?;

            if let Err(error) = assert_layouts_close(
                &fixture.name,
                &browser_rects,
                &taffy_rects,
                &text_ids,
                fixture.tolerance_px,
            ) {
                failures.push(error.to_string());
            }
        }

        browser.shutdown().await?;
        if !failures.is_empty() {
            bail!("{}", failures.join("\n\n"));
        }
        Ok(())
    })
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
    async fn new(env: &BrowserTestEnv) -> Result<Self> {
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

        let session_id =
            create_session(&client, &webdriver_url, env.chrome_bin.as_ref(), 1280, 800).await?;

        Ok(Self {
            client,
            webdriver_url,
            session_id,
            child,
        })
    }

    async fn measure_layout(&self, html_path: &Path) -> Result<BTreeMap<String, BrowserRect>> {
        let canonical = html_path
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", html_path.display()))?;
        let url = format!("file://{}", canonical.to_string_lossy());
        webdriver_post(
            &self.client,
            &self.webdriver_url,
            &self.session_id,
            "url",
            json!({ "url": url }),
        )
        .await?;

        wait_for_document_ready(&self.client, &self.webdriver_url, &self.session_id).await?;

        let result = webdriver_post(
            &self.client,
            &self.webdriver_url,
            &self.session_id,
            "execute/sync",
            json!({
                "script": r#"
                    return Array.from(document.querySelectorAll('[data-oc-id]'))
                      .map((el) => {
                        const rect = el.getBoundingClientRect();
                        return {
                          id: el.dataset.ocId,
                          x: rect.x,
                          y: rect.y,
                          width: rect.width,
                          height: rect.height,
                        };
                      })
                      .filter((rect) => rect.width > 0 && rect.height > 0);
                "#,
                "args": [],
            }),
        )
        .await?;

        let items = result
            .as_array()
            .ok_or_else(|| anyhow!("webdriver script did not return an array"))?;
        let mut rects = BTreeMap::new();
        for item in items {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("browser rect missing id"))?;
            rects.insert(
                id.to_string(),
                BrowserRect {
                    x: parse_f32(item, "x")?,
                    y: parse_f32(item, "y")?,
                    width: parse_f32(item, "width")?,
                    height: parse_f32(item, "height")?,
                },
            );
        }

        Ok(rects)
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
        if let Ok(response) = client.get(format!("{webdriver_url}/status")).send().await {
            if response.status().is_success() {
                return Ok(());
            }
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
            "--disable-gpu",
            "--hide-scrollbars",
            "--force-device-scale-factor=1",
            format!("--window-size={width},{height}")
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
    for _ in 0..50 {
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
        std::net::TcpListener::bind("127.0.0.1:0").context("failed to reserve a local TCP port")?;
    let port = listener
        .local_addr()
        .context("failed to inspect reserved TCP port")?
        .port();
    drop(listener);
    Ok(port)
}

fn parse_f32(value: &Value, key: &str) -> Result<f32> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .map(|v| v as f32)
        .ok_or_else(|| anyhow!("browser rect missing numeric field `{key}`: {value}"))
}

#[derive(Clone, Debug)]
struct BrowserRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn measure_taffy_layout(fixture: &LayoutFixture) -> Result<BTreeMap<String, BrowserRect>> {
    let root = Arc::new(fixture.root.to_node());
    let composition = Composition::new(fixture.name)
        .size(fixture.viewport_width, fixture.viewport_height)
        .fps(30)
        .frames(1)
        .root({
            let root = root.clone();
            move |_| (*root).clone()
        })
        .build()?;

    let mut session = RenderSession::new();
    let rects = collect_frame_layout_rects(&composition, 0, &mut session)?;
    Ok(rect_map_from_frame_rects(rects))
}

fn rect_map_from_frame_rects(rects: Vec<FrameElementRect>) -> BTreeMap<String, BrowserRect> {
    rects
        .into_iter()
        .map(|rect| {
            (
                rect.id,
                BrowserRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                },
            )
        })
        .collect()
}

fn assert_layouts_close(
    fixture_name: &str,
    browser_rects: &BTreeMap<String, BrowserRect>,
    taffy_rects: &BTreeMap<String, BrowserRect>,
    text_ids: &BTreeSet<String>,
    tolerance_px: f32,
) -> Result<()> {
    let browser_ids = browser_rects.keys().cloned().collect::<BTreeSet<_>>();
    let taffy_ids = taffy_rects.keys().cloned().collect::<BTreeSet<_>>();
    if browser_ids != taffy_ids {
        bail!(
            "fixture `{fixture_name}` node id mismatch\nbrowser: {:?}\ntaffy: {:?}",
            browser_ids,
            taffy_ids
        );
    }

    let mut mismatches = Vec::new();
    for id in browser_ids {
        let browser = browser_rects
            .get(&id)
            .expect("browser rect should exist for compared id");
        let taffy = taffy_rects
            .get(&id)
            .expect("taffy rect should exist for compared id");

        for (field, browser_value, taffy_value) in [
            ("x", browser.x, taffy.x),
            ("y", browser.y, taffy.y),
            ("width", browser.width, taffy.width),
            ("height", browser.height, taffy.height),
        ] {
            // Keep text strict so font/layout drift stays visible even when a fixture
            // needs a looser tolerance for non-text geometry.
            let effective_tolerance = if text_ids.contains(&id) {
                1.0
            } else {
                tolerance_px
            };
            let delta = (browser_value - taffy_value).abs();
            if delta > effective_tolerance {
                mismatches.push(format!(
                    "{id}.{field}: browser={browser_value:.2} taffy={taffy_value:.2} Δ={delta:.2} tol={effective_tolerance:.2}"
                ));
            }
        }
    }

    if mismatches.is_empty() {
        return Ok(());
    }

    bail!(
        "fixture `{fixture_name}` layout mismatch (tolerance {:.2}px)\n{}",
        tolerance_px,
        mismatches.join("\n")
    )
}

fn compile_tailwind_css(fixture: &LayoutFixture) -> Result<String> {
    let candidates = fixture.root.collect_candidates();
    let payload = json!({ "candidates": candidates });
    let payload_path = write_fixture_file(&fixture.name, "json", &payload.to_string())?;
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .ok_or_else(|| anyhow!("failed to find repository root from CARGO_MANIFEST_DIR"))?;
    let script_path = manifest_dir.join("testsupport/compile_tailwind_css.mjs");

    let output = Command::new("node")
        .arg(script_path)
        .arg(payload_path)
        .current_dir(repo_root)
        .output()
        .context("failed to execute Tailwind CSS helper")?;

    if !output.status.success() {
        bail!(
            "Tailwind CSS helper failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout).context("Tailwind CSS helper returned non-utf8 CSS")
}

fn write_fixture_file(name: &str, extension: &str, content: &str) -> Result<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("browser-layout-tests");
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_nanos();
    let path = dir.join(format!("{name}-{nonce}.{extension}"));
    fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

#[derive(Clone)]
pub(crate) struct LayoutFixture {
    pub(crate) name: &'static str,
    pub(crate) viewport_width: i32,
    pub(crate) viewport_height: i32,
    pub(crate) tolerance_px: f32,
    pub(crate) root: FixtureNode,
}

impl LayoutFixture {
    fn render_html_document(&self, css: &str) -> String {
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><style>{}</style><style>html,body{{margin:0;padding:0;width:{}px;height:{}px;overflow:hidden;}}body{{width:{}px;height:{}px;}}</style></head><body>{}</body></html>",
            css,
            self.viewport_width,
            self.viewport_height,
            self.viewport_width,
            self.viewport_height,
            self.root.render_html()
        )
    }
}

#[derive(Clone)]
pub(crate) enum FixtureNodeKind {
    Div,
    Text(&'static str),
}

#[derive(Clone)]
pub(crate) struct FixtureNode {
    pub(crate) id: &'static str,
    pub(crate) class_name: &'static str,
    pub(crate) kind: FixtureNodeKind,
    pub(crate) children: Vec<FixtureNode>,
}

impl FixtureNode {
    pub(crate) fn div(id: &'static str, class_name: &'static str, children: Vec<FixtureNode>) -> Self {
        Self {
            id,
            class_name,
            kind: FixtureNodeKind::Div,
            children,
        }
    }

    pub(crate) fn text(id: &'static str, class_name: &'static str, content: &'static str) -> Self {
        Self {
            id,
            class_name,
            kind: FixtureNodeKind::Text(content),
            children: Vec::new(),
        }
    }

    fn collect_candidates(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_candidates_into(&mut out);
        out
    }

    fn collect_text_ids(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        self.collect_text_ids_into(&mut out);
        out
    }

    fn collect_candidates_into(&self, out: &mut Vec<String>) {
        for class in self.class_name.split_whitespace() {
            out.push(class.to_string());
        }
        for child in &self.children {
            child.collect_candidates_into(out);
        }
    }

    fn collect_text_ids_into(&self, out: &mut BTreeSet<String>) {
        if matches!(self.kind, FixtureNodeKind::Text(_)) {
            out.insert(self.id.to_string());
        }
        for child in &self.children {
            child.collect_text_ids_into(out);
        }
    }

    fn render_html(&self) -> String {
        let attrs = format!(
            "id=\"{}\" data-oc-id=\"{}\" class=\"{}\"",
            self.id, self.id, self.class_name
        );
        match &self.kind {
            FixtureNodeKind::Div => {
                let children = self
                    .children
                    .iter()
                    .map(FixtureNode::render_html)
                    .collect::<String>();
                format!("<div {attrs}>{children}</div>")
            }
            FixtureNodeKind::Text(content) => {
                format!("<div {attrs}>{}</div>", escape_html(content))
            }
        }
    }

    fn to_node(&self) -> crate::Node {
        match &self.kind {
            FixtureNodeKind::Div => {
                let mut node = div();
                node.style = parse_class_name(self.class_name);
                node.style.id = self.id.to_string();
                node.children = self.children.iter().map(FixtureNode::to_node).collect();
                node.into()
            }
            FixtureNodeKind::Text(content) => {
                let mut node = text(*content);
                node.style = parse_class_name(self.class_name);
                node.style.id = self.id.to_string();
                node.into()
            }
        }
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

struct GeneratedCoverageReport {
    fixtures: Vec<LayoutFixture>,
    uncovered: Vec<String>,
}

#[derive(Clone, Copy)]
struct LayoutGroupSpec {
    test_name: &'static str,
    normalize: fn(&str) -> Option<String>,
    build_fixture: fn(&str) -> Option<LayoutFixture>,
}

const GENERATED_LAYOUT_GROUP_SPECS: &[LayoutGroupSpec] = &[
    LayoutGroupSpec {
        test_name: "position",
        normalize: normalize_position_candidate,
        build_fixture: build_position_fixture,
    },
    LayoutGroupSpec {
        test_name: "inset",
        normalize: normalize_inset_candidate,
        build_fixture: build_inset_fixture,
    },
    LayoutGroupSpec {
        test_name: "inset-x",
        normalize: normalize_inset_candidate,
        build_fixture: build_inset_fixture,
    },
    LayoutGroupSpec {
        test_name: "inset-y",
        normalize: normalize_inset_candidate,
        build_fixture: build_inset_fixture,
    },
    LayoutGroupSpec {
        test_name: "inset-s",
        normalize: normalize_inset_candidate,
        build_fixture: build_inset_fixture,
    },
    LayoutGroupSpec {
        test_name: "inset-e",
        normalize: normalize_inset_candidate,
        build_fixture: build_inset_fixture,
    },
    LayoutGroupSpec {
        test_name: "inset-bs",
        normalize: normalize_inset_candidate,
        build_fixture: build_inset_fixture,
    },
    LayoutGroupSpec {
        test_name: "inset-be",
        normalize: normalize_inset_candidate,
        build_fixture: build_inset_fixture,
    },
    LayoutGroupSpec {
        test_name: "top",
        normalize: normalize_inset_candidate,
        build_fixture: build_inset_fixture,
    },
    LayoutGroupSpec {
        test_name: "right",
        normalize: normalize_inset_candidate,
        build_fixture: build_inset_fixture,
    },
    LayoutGroupSpec {
        test_name: "bottom",
        normalize: normalize_inset_candidate,
        build_fixture: build_inset_fixture,
    },
    LayoutGroupSpec {
        test_name: "left",
        normalize: normalize_inset_candidate,
        build_fixture: build_inset_fixture,
    },
    LayoutGroupSpec {
        test_name: "width",
        normalize: normalize_width_candidate,
        build_fixture: build_width_fixture,
    },
    LayoutGroupSpec {
        test_name: "height",
        normalize: normalize_height_candidate,
        build_fixture: build_height_fixture,
    },
    LayoutGroupSpec {
        test_name: "flex",
        normalize: normalize_flex_candidate,
        build_fixture: build_flex_sizing_fixture,
    },
    LayoutGroupSpec {
        test_name: "flex-shrink",
        normalize: normalize_flex_shrink_candidate,
        build_fixture: build_flex_sizing_fixture,
    },
    LayoutGroupSpec {
        test_name: "flex-grow",
        normalize: normalize_flex_grow_candidate,
        build_fixture: build_flex_sizing_fixture,
    },
    LayoutGroupSpec {
        test_name: "flex-basis",
        normalize: normalize_flex_basis_candidate,
        build_fixture: build_flex_sizing_fixture,
    },
    LayoutGroupSpec {
        test_name: "flex-direction",
        normalize: normalize_flex_direction_candidate,
        build_fixture: build_flex_direction_fixture,
    },
    LayoutGroupSpec {
        test_name: "flex-wrap",
        normalize: normalize_flex_wrap_candidate,
        build_fixture: build_flex_wrap_fixture,
    },
    LayoutGroupSpec {
        test_name: "justify",
        normalize: normalize_justify_candidate,
        build_fixture: build_justify_fixture,
    },
    LayoutGroupSpec {
        test_name: "align-content",
        normalize: normalize_align_content_candidate,
        build_fixture: build_align_content_fixture,
    },
    LayoutGroupSpec {
        test_name: "place-content",
        normalize: normalize_place_content_candidate,
        build_fixture: build_place_content_fixture,
    },
    LayoutGroupSpec {
        test_name: "items",
        normalize: normalize_items_candidate,
        build_fixture: build_items_fixture,
    },
    LayoutGroupSpec {
        test_name: "place-items",
        normalize: normalize_place_items_candidate,
        build_fixture: build_place_items_fixture,
    },
    LayoutGroupSpec {
        test_name: "gap",
        normalize: normalize_gap_candidate,
        build_fixture: build_gap_fixture,
    },
    LayoutGroupSpec {
        test_name: "p",
        normalize: normalize_padding_candidate,
        build_fixture: build_padding_fixture,
    },
    LayoutGroupSpec {
        test_name: "px",
        normalize: normalize_padding_candidate,
        build_fixture: build_padding_fixture,
    },
    LayoutGroupSpec {
        test_name: "py",
        normalize: normalize_padding_candidate,
        build_fixture: build_padding_fixture,
    },
    LayoutGroupSpec {
        test_name: "pt",
        normalize: normalize_padding_candidate,
        build_fixture: build_padding_fixture,
    },
    LayoutGroupSpec {
        test_name: "pr",
        normalize: normalize_padding_candidate,
        build_fixture: build_padding_fixture,
    },
    LayoutGroupSpec {
        test_name: "pb",
        normalize: normalize_padding_candidate,
        build_fixture: build_padding_fixture,
    },
    LayoutGroupSpec {
        test_name: "pl",
        normalize: normalize_padding_candidate,
        build_fixture: build_padding_fixture,
    },
    LayoutGroupSpec {
        test_name: "margin",
        normalize: normalize_margin_candidate,
        build_fixture: build_margin_fixture,
    },
    LayoutGroupSpec {
        test_name: "mx",
        normalize: normalize_margin_candidate,
        build_fixture: build_margin_fixture,
    },
    LayoutGroupSpec {
        test_name: "my",
        normalize: normalize_margin_candidate,
        build_fixture: build_margin_fixture,
    },
    LayoutGroupSpec {
        test_name: "mt",
        normalize: normalize_margin_candidate,
        build_fixture: build_margin_fixture,
    },
    LayoutGroupSpec {
        test_name: "ms",
        normalize: normalize_margin_candidate,
        build_fixture: build_margin_fixture,
    },
    LayoutGroupSpec {
        test_name: "me",
        normalize: normalize_margin_candidate,
        build_fixture: build_margin_fixture,
    },
    LayoutGroupSpec {
        test_name: "mbs",
        normalize: normalize_margin_candidate,
        build_fixture: build_margin_fixture,
    },
    LayoutGroupSpec {
        test_name: "mbe",
        normalize: normalize_margin_candidate,
        build_fixture: build_margin_fixture,
    },
    LayoutGroupSpec {
        test_name: "mr",
        normalize: normalize_margin_candidate,
        build_fixture: build_margin_fixture,
    },
    LayoutGroupSpec {
        test_name: "mb",
        normalize: normalize_margin_candidate,
        build_fixture: build_margin_fixture,
    },
    LayoutGroupSpec {
        test_name: "ml",
        normalize: normalize_margin_candidate,
        build_fixture: build_margin_fixture,
    },
    LayoutGroupSpec {
        test_name: "self",
        normalize: normalize_self_candidate,
        build_fixture: build_self_fixture,
    },
    LayoutGroupSpec {
        test_name: "gap-x",
        normalize: normalize_gap_candidate,
        build_fixture: build_gap_x_fixture,
    },
    LayoutGroupSpec {
        test_name: "gap-y",
        normalize: normalize_gap_candidate,
        build_fixture: build_gap_y_fixture,
    },
    LayoutGroupSpec {
        test_name: "min-width",
        normalize: normalize_min_width_candidate,
        build_fixture: build_min_width_fixture,
    },
    LayoutGroupSpec {
        test_name: "max-width",
        normalize: normalize_max_width_candidate,
        build_fixture: build_max_width_fixture,
    },
    LayoutGroupSpec {
        test_name: "min-height",
        normalize: normalize_min_height_candidate,
        build_fixture: build_min_height_fixture,
    },
    LayoutGroupSpec {
        test_name: "max-height",
        normalize: normalize_max_height_candidate,
        build_fixture: build_max_height_fixture,
    },
    LayoutGroupSpec {
        test_name: "order",
        normalize: normalize_order_candidate,
        build_fixture: build_order_fixture,
    },
    LayoutGroupSpec {
        test_name: "translate-x",
        normalize: normalize_translate_x_candidate,
        build_fixture: build_translate_x_fixture,
    },
    LayoutGroupSpec {
        test_name: "translate-y",
        normalize: normalize_translate_y_candidate,
        build_fixture: build_translate_y_fixture,
    },
    LayoutGroupSpec {
        test_name: "visibility",
        normalize: normalize_visibility_candidate,
        build_fixture: build_visibility_fixture,
    },
    LayoutGroupSpec {
        test_name: "box-sizing",
        normalize: normalize_box_sizing_candidate,
        build_fixture: build_box_sizing_fixture,
    },
    LayoutGroupSpec {
        test_name: "aspect-ratio",
        normalize: normalize_aspect_ratio_candidate,
        build_fixture: build_aspect_ratio_fixture,
    },
    LayoutGroupSpec {
        test_name: "place-self",
        normalize: normalize_place_self_candidate,
        build_fixture: build_place_self_fixture,
    },
    LayoutGroupSpec {
        test_name: "justify-items",
        normalize: normalize_justify_items_candidate,
        build_fixture: build_justify_items_fixture,
    },
    LayoutGroupSpec {
        test_name: "justify-self",
        normalize: normalize_justify_self_candidate,
        build_fixture: build_justify_self_fixture,
    },
    // ── Grid ────────────────────────────────────────────────────────
    LayoutGroupSpec {
        test_name: "grid-cols",
        normalize: normalize_grid_cols_candidate,
        build_fixture: build_grid_cols_fixture,
    },
    LayoutGroupSpec {
        test_name: "grid-rows",
        normalize: normalize_grid_rows_candidate,
        build_fixture: build_grid_rows_fixture,
    },
    LayoutGroupSpec {
        test_name: "grid-flow",
        normalize: normalize_grid_flow_candidate,
        build_fixture: build_grid_flow_fixture,
    },
    LayoutGroupSpec {
        test_name: "auto-cols",
        normalize: normalize_auto_sizing_candidate,
        build_fixture: build_auto_cols_fixture,
    },
    LayoutGroupSpec {
        test_name: "auto-rows",
        normalize: normalize_auto_sizing_candidate,
        build_fixture: build_auto_rows_fixture,
    },
    LayoutGroupSpec {
        test_name: "col-start",
        normalize: normalize_grid_placement_candidate,
        build_fixture: build_col_start_fixture,
    },
    LayoutGroupSpec {
        test_name: "col-end",
        normalize: normalize_grid_placement_candidate,
        build_fixture: build_col_end_fixture,
    },
    LayoutGroupSpec {
        test_name: "row-start",
        normalize: normalize_grid_placement_candidate,
        build_fixture: build_row_start_fixture,
    },
    LayoutGroupSpec {
        test_name: "row-end",
        normalize: normalize_grid_placement_candidate,
        build_fixture: build_row_end_fixture,
    },
];

fn generated_layout_coverage_fixtures() -> Result<Vec<LayoutFixture>> {
    Ok(generated_layout_coverage_report()?.fixtures)
}

fn generated_layout_coverage_report() -> Result<GeneratedCoverageReport> {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("testsupport/utilities.test.ts"),
    )
    .context("failed to read testsupport/utilities.test.ts")?;

    let mut fixtures = Vec::new();
    let mut uncovered = Vec::new();

    for spec in GENERATED_LAYOUT_GROUP_SPECS {
        extend_group_fixtures(
            &mut fixtures,
            &mut uncovered,
            spec.test_name,
            extract_layout_test_candidates(&source, spec.test_name)?,
            spec.normalize,
            spec.build_fixture,
        );
    }

    Ok(GeneratedCoverageReport {
        fixtures,
        uncovered,
    })
}

fn extend_group_fixtures(
    fixtures: &mut Vec<LayoutFixture>,
    uncovered: &mut Vec<String>,
    group_name: &str,
    candidates: Vec<String>,
    normalize: fn(&str) -> Option<String>,
    build_fixture: fn(&str) -> Option<LayoutFixture>,
) {
    let normalized = normalize_layout_candidates(candidates, normalize);
    for class_name in normalized {
        match build_fixture(&class_name) {
            Some(fixture) => fixtures.push(fixture),
            None => uncovered.push(format!("{group_name}: {class_name}")),
        }
    }
}

fn normalize_layout_candidates(
    candidates: Vec<String>,
    normalize: fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut deduped = BTreeSet::new();
    for candidate in candidates {
        if let Some(candidate) = normalize(&candidate) {
            deduped.insert(candidate);
        }
    }
    deduped.into_iter().collect()
}

fn extract_layout_test_candidates(source: &str, test_name: &str) -> Result<Vec<String>> {
    let marker = format!("test('{test_name}', async () => {{");
    let start = source
        .find(&marker)
        .ok_or_else(|| anyhow!("failed to find `{test_name}` in utilities.test.ts"))?;
    let after_test = &source[start..];
    let body = extract_test_body(after_test)
        .ok_or_else(|| anyhow!("failed to isolate body for `{test_name}`"))?;
    let array = extract_first_array_literal(body)
        .ok_or_else(|| anyhow!("failed to find first candidate array for `{test_name}`"))?;
    Ok(parse_js_string_literals(array))
}

fn parse_js_string_literals(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\'' && ch != '"' {
            continue;
        }
        let quote = ch;
        let mut value = String::new();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(escaped) = chars.next() {
                    value.push(escaped);
                }
                continue;
            }
            if ch == quote {
                break;
            }
            value.push(ch);
        }
        out.push(value);
    }
    out
}

fn extract_test_body(input: &str) -> Option<&str> {
    let body_start = input.find('{')? + 1;
    let mut depth = 1_i32;
    let mut single = false;
    let mut double = false;
    let mut template = false;
    let mut escape = false;

    for (index, ch) in input
        .char_indices()
        .skip_while(|(index, _)| *index < body_start)
    {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' if single || double || template => escape = true,
            '\'' if !double && !template => single = !single,
            '"' if !single && !template => double = !double,
            '`' if !single && !double => template = !template,
            '{' if !single && !double && !template => depth += 1,
            '}' if !single && !double && !template => {
                depth -= 1;
                if depth == 0 {
                    return Some(&input[body_start..index]);
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_first_array_literal(input: &str) -> Option<&str> {
    let mut single = false;
    let mut double = false;
    let mut template = false;
    let mut escape = false;
    let mut depth = 0_i32;
    let mut start = None;

    for (index, ch) in input.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' if single || double || template => escape = true,
            '\'' if !double && !template => single = !single,
            '"' if !single && !template => double = !double,
            '`' if !single && !double => template = !template,
            '[' if !single && !double && !template => {
                if depth == 0 {
                    start = Some(index + 1);
                }
                depth += 1;
            }
            ']' if !single && !double && !template => {
                depth -= 1;
                if depth == 0 {
                    return start.map(|start| &input[start..index]);
                }
            }
            _ => {}
        }
    }
    None
}

fn normalize_safe_alias(class_name: &str) -> String {
    class_name
        .strip_suffix("-safe")
        .unwrap_or(class_name)
        .to_string()
}

fn is_numeric_spacing_or_bracket(class_name: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| {
        class_name
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.ends_with(']') || suffix.parse::<f32>().is_ok())
    })
}

fn is_fraction_class(class_name: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| {
        class_name.strip_prefix(prefix).is_some_and(|suffix| {
            let Some((left, right)) = suffix.split_once('/') else {
                return false;
            };
            left.parse::<f32>().is_ok() && right.parse::<f32>().is_ok()
        })
    })
}

fn normalize_position_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "absolute" | "relative" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_inset_candidate(class_name: &str) -> Option<String> {
    if class_name.contains("shadow") || class_name.contains("shadowned") {
        return None;
    }
    Some(class_name.to_string())
}

fn normalize_flex_direction_candidate(class_name: &str) -> Option<String> {
    Some(class_name.to_string())
}

fn normalize_flex_wrap_candidate(class_name: &str) -> Option<String> {
    Some(class_name.to_string())
}

fn normalize_justify_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "justify-normal" => None,
        _ => Some(normalize_safe_alias(class_name)),
    }
}

fn normalize_align_content_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "content-normal" | "content-baseline" => None,
        _ => Some(normalize_safe_alias(class_name)),
    }
}

fn normalize_place_content_candidate(class_name: &str) -> Option<String> {
    if class_name == "place-content-baseline" {
        None
    } else {
        Some(normalize_safe_alias(class_name))
    }
}

fn normalize_items_candidate(class_name: &str) -> Option<String> {
    if class_name.contains("baseline") {
        None
    } else {
        Some(normalize_safe_alias(class_name))
    }
}

fn normalize_place_items_candidate(class_name: &str) -> Option<String> {
    if class_name.contains("baseline") {
        None
    } else {
        Some(normalize_safe_alias(class_name))
    }
}

fn normalize_self_candidate(class_name: &str) -> Option<String> {
    if class_name.contains("baseline") {
        None
    } else {
        Some(normalize_safe_alias(class_name))
    }
}

fn normalize_gap_candidate(class_name: &str) -> Option<String> {
    if is_numeric_spacing_or_bracket(class_name, &["gap-", "gap-x-", "gap-y-"]) {
        Some(class_name.to_string())
    } else {
        None
    }
}

fn normalize_min_width_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "min-w-0" | "min-w-full" | "min-w-[123px]" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_max_width_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "max-w-0" | "max-w-full" | "max-w-[123px]" | "max-w-xs" | "max-w-sm" | "max-w-md"
        | "max-w-lg" | "max-w-xl" | "max-w-2xl" | "max-w-3xl" | "max-w-4xl" | "max-w-5xl"
        | "max-w-6xl" | "max-w-7xl" | "max-w-none" | "max-w-screen-sm" | "max-w-screen-md"
        | "max-w-screen-lg" | "max-w-screen-xl" | "max-w-screen-2xl" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_min_height_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "min-h-0" | "min-h-full" | "min-h-screen" | "min-h-[123px]" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_max_height_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "max-h-0" | "max-h-full" | "max-h-screen" | "max-h-[123px]" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_order_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "order-1" | "order-2" | "order-3" | "order-first" | "order-last" | "order-[123]" => {
            Some(class_name.to_string())
        }
        _ => None,
    }
}

fn normalize_translate_x_candidate(class_name: &str) -> Option<String> {
    if is_numeric_spacing_or_bracket(class_name, &["translate-x-", "-translate-x-"]) {
        Some(class_name.to_string())
    } else {
        None
    }
}

fn normalize_translate_y_candidate(class_name: &str) -> Option<String> {
    if is_numeric_spacing_or_bracket(class_name, &["translate-y-", "-translate-y-"]) {
        Some(class_name.to_string())
    } else {
        None
    }
}

fn normalize_visibility_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "visible" | "invisible" | "collapse" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_box_sizing_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "border-box" | "content-box" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_aspect_ratio_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "aspect-auto" | "aspect-square" | "aspect-video" | "aspect-[123/456]" => {
            Some(class_name.to_string())
        }
        _ => None,
    }
}

fn normalize_place_self_candidate(class_name: &str) -> Option<String> {
    if class_name.contains("baseline") {
        None
    } else {
        Some(normalize_safe_alias(class_name))
    }
}

fn normalize_justify_items_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "justify-items-start" | "justify-items-end" | "justify-items-center"
        | "justify-items-stretch" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_justify_self_candidate(class_name: &str) -> Option<String> {
    if class_name.contains("baseline") {
        None
    } else {
        Some(normalize_safe_alias(class_name))
    }
}

// ── Grid normalize functions ─────────────────────────────────────────────

fn normalize_grid_cols_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "grid-cols-12" | "grid-cols-99" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_grid_rows_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "grid-rows-12" | "grid-rows-99" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_grid_flow_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "grid-flow-row" | "grid-flow-col" | "grid-flow-dense" | "grid-flow-row-dense"
        | "grid-flow-col-dense" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_auto_sizing_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "auto-cols-auto" | "auto-cols-min" | "auto-cols-max" | "auto-cols-fr"
        | "auto-rows-auto" | "auto-rows-min" | "auto-rows-max" | "auto-rows-fr" => {
            Some(class_name.to_string())
        }
        _ => None,
    }
}

fn normalize_grid_placement_candidate(class_name: &str) -> Option<String> {
    if class_name.contains('/') || class_name.contains("unknown") || class_name.contains("custom") {
        return None;
    }
    // col-start-4, col-start-99, -col-start-4, col-start-auto, col-start-[123]
    if class_name.starts_with("col-start-") || class_name.starts_with("-col-start-") {
        return Some(class_name.to_string());
    }
    if class_name.starts_with("col-end-") || class_name.starts_with("-col-end-") {
        return Some(class_name.to_string());
    }
    if class_name.starts_with("row-start-") || class_name.starts_with("-row-start-") {
        return Some(class_name.to_string());
    }
    if class_name.starts_with("row-end-") || class_name.starts_with("-row-end-") {
        return Some(class_name.to_string());
    }
    None
}

fn normalize_width_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "w-full" => Some(class_name.to_string()),
        _ if is_numeric_spacing_or_bracket(class_name, &["w-"]) => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_height_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "h-full" => Some(class_name.to_string()),
        _ if is_numeric_spacing_or_bracket(class_name, &["h-"]) => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_flex_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "flex-1" | "flex-99" | "flex-auto" | "flex-initial" | "flex-none" | "flex-[123]" => {
            Some(class_name.to_string())
        }
        "flex-1/2" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_flex_shrink_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "shrink" | "shrink-0" | "shrink-[123]" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_flex_grow_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "grow" | "grow-0" | "grow-[123]" => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_flex_basis_candidate(class_name: &str) -> Option<String> {
    match class_name {
        "basis-auto" | "basis-full" | "basis-[123px]" => Some(class_name.to_string()),
        _ if is_fraction_class(class_name, &["basis-"]) => Some(class_name.to_string()),
        _ => None,
    }
}

fn normalize_padding_candidate(class_name: &str) -> Option<String> {
    if class_name.contains("big") {
        return None;
    }
    if is_numeric_spacing_or_bracket(
        class_name,
        &["p-", "px-", "py-", "pt-", "pr-", "pb-", "pl-"],
    ) {
        Some(class_name.to_string())
    } else {
        None
    }
}

fn normalize_margin_candidate(class_name: &str) -> Option<String> {
    // Allow margin-auto variants (mx-auto, ml-auto, etc.)
    if matches!(
        class_name,
        "mx-auto" | "my-auto" | "ml-auto" | "mr-auto" | "mt-auto" | "mb-auto" | "m-auto"
    ) {
        return Some(class_name.to_string());
    }
    if class_name.contains("big") || class_name.contains("var(") {
        return None;
    }
    if is_numeric_spacing_or_bracket(
        class_name,
        &[
            "m-", "-m-", "mx-", "-mx-", "my-", "-my-", "mt-", "-mt-", "mr-", "-mr-", "mb-", "-mb-",
            "ml-", "-ml-", "ms-", "-ms-", "me-", "-me-", "mbs-", "-mbs-", "mbe-", "-mbe-",
        ],
    ) {
        Some(class_name.to_string())
    } else {
        None
    }
}

fn leak_str(value: impl Into<String>) -> &'static str {
    Box::leak(value.into().into_boxed_str())
}

fn generated_fixture_name(group: &str, class_name: &str) -> &'static str {
    let sanitized = class_name
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch,
            _ => '-',
        })
        .collect::<String>();
    leak_str(format!("generated-{group}-{sanitized}"))
}

fn build_position_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("position", class_name),
        viewport_width: 320,
        viewport_height: 180,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "relative w-full h-full",
            vec![
                FixtureNode::div("before", "w-[120px] h-[32px]", vec![]),
                FixtureNode::div(
                    "target",
                    leak_str(format!(
                        "{class_name} left-[24px] top-[12px] w-[80px] h-[28px]"
                    )),
                    vec![],
                ),
                FixtureNode::div("after", "w-[96px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_inset_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("inset", class_name),
        viewport_width: 320,
        viewport_height: 220,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "relative w-[220px] h-[180px]",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("absolute {class_name} w-[40px] h-[32px]")),
                vec![],
            )],
        ),
    })
}

fn build_flex_direction_fixture(class_name: &str) -> Option<LayoutFixture> {
    let (viewport_width, viewport_height, root_class) = if class_name.contains("col") {
        (
            240,
            220,
            format!("flex {class_name} items-start w-[180px] h-[200px] p-[12px] gap-[10px]"),
        )
    } else {
        (
            320,
            140,
            format!("flex {class_name} items-start w-full h-full p-[16px] gap-[8px]"),
        )
    };

    Some(LayoutFixture {
        name: generated_fixture_name("flex-direction", class_name),
        viewport_width,
        viewport_height,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(root_class),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[28px]", vec![]),
                FixtureNode::div("item-b", "w-[56px] h-[32px]", vec![]),
                FixtureNode::div("item-c", "w-[48px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_width_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("width", class_name),
        viewport_width: 320,
        viewport_height: 180,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "w-full h-full",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("{class_name} h-[24px]")),
                vec![],
            )],
        ),
    })
}

fn build_height_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("height", class_name),
        viewport_width: 320,
        viewport_height: 220,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "w-[220px] h-[180px]",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("{class_name} w-[48px]")),
                vec![],
            )],
        ),
    })
}

fn build_flex_sizing_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("flex-sizing", class_name),
        viewport_width: 360,
        viewport_height: 120,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "flex w-[280px] h-[64px] gap-[8px]",
            vec![
                FixtureNode::div("fixed-a", "w-[40px] h-[24px] shrink-0", vec![]),
                FixtureNode::div("target", leak_str(format!("{class_name} h-[24px]")), vec![]),
                FixtureNode::div("fixed-b", "w-[56px] h-[24px] shrink-0", vec![]),
            ],
        ),
    })
}

fn build_flex_wrap_fixture(class_name: &str) -> Option<LayoutFixture> {
    let viewport_height = if class_name == "flex-nowrap" {
        120
    } else {
        160
    };
    let root_class = if class_name == "flex-nowrap" {
        format!("flex {class_name} items-start w-[120px] h-[80px] p-[8px] gap-[8px]")
    } else {
        format!("flex {class_name} items-start w-[152px] h-[140px] p-[8px] gap-[8px]")
    };

    Some(LayoutFixture {
        name: generated_fixture_name("flex-wrap", class_name),
        viewport_width: 220,
        viewport_height,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(root_class),
            vec![
                FixtureNode::div("item-a", "w-[56px] h-[24px] shrink-0", vec![]),
                FixtureNode::div("item-b", "w-[56px] h-[24px] shrink-0", vec![]),
                FixtureNode::div("item-c", "w-[56px] h-[24px] shrink-0", vec![]),
                FixtureNode::div("item-d", "w-[56px] h-[24px] shrink-0", vec![]),
            ],
        ),
    })
}

fn build_justify_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("justify", class_name),
        viewport_width: 320,
        viewport_height: 140,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!(
                "flex {class_name} items-start w-[240px] h-[80px] p-[8px] gap-[8px]"
            )),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_align_content_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("align-content", class_name),
        viewport_width: 220,
        viewport_height: 220,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!(
                "flex flex-wrap {class_name} items-start w-[152px] h-[180px] p-[8px] gap-[8px]"
            )),
            vec![
                FixtureNode::div("item-a", "w-[56px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[56px] h-[24px]", vec![]),
                FixtureNode::div("item-c", "w-[56px] h-[24px]", vec![]),
                FixtureNode::div("item-d", "w-[56px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_place_content_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("place-content", class_name),
        viewport_width: 220,
        viewport_height: 220,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!(
                "flex flex-wrap {class_name} items-start w-[152px] h-[180px] p-[8px] gap-[8px]"
            )),
            vec![
                FixtureNode::div("item-a", "w-[120px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[120px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_items_fixture(class_name: &str) -> Option<LayoutFixture> {
    let children = if class_name.ends_with("stretch") {
        vec![
            FixtureNode::div("item-a", "w-[40px]", vec![]),
            FixtureNode::div("item-b", "w-[40px]", vec![]),
            FixtureNode::div("item-c", "w-[40px]", vec![]),
        ]
    } else {
        vec![
            FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
            FixtureNode::div("item-b", "w-[40px] h-[32px]", vec![]),
            FixtureNode::div("item-c", "w-[40px] h-[20px]", vec![]),
        ]
    };

    Some(LayoutFixture {
        name: generated_fixture_name("items", class_name),
        viewport_width: 260,
        viewport_height: 160,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!(
                "flex {class_name} w-[220px] h-[120px] p-[8px] gap-[8px]"
            )),
            children,
        ),
    })
}

fn build_place_items_fixture(class_name: &str) -> Option<LayoutFixture> {
    let (viewport_width, viewport_height, root_class, children) = if class_name.ends_with("end") {
        (
            260,
            200,
            format!("flex flex-col {class_name} w-[220px] h-[160px] p-[8px] gap-[8px]"),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[64px] h-[24px]", vec![]),
                FixtureNode::div("item-c", "w-[52px] h-[24px]", vec![]),
            ],
        )
    } else if class_name.ends_with("stretch") {
        (
            260,
            160,
            format!("flex {class_name} w-[220px] h-[120px] p-[8px] gap-[8px]"),
            vec![
                FixtureNode::div("item-a", "w-[40px]", vec![]),
                FixtureNode::div("item-b", "w-[40px]", vec![]),
                FixtureNode::div("item-c", "w-[40px]", vec![]),
            ],
        )
    } else {
        (
            260,
            160,
            format!("flex {class_name} w-[220px] h-[120px] p-[8px] gap-[8px]"),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[40px] h-[32px]", vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[20px]", vec![]),
            ],
        )
    };

    Some(LayoutFixture {
        name: generated_fixture_name("place-items", class_name),
        viewport_width,
        viewport_height,
        tolerance_px: 1.0,
        root: FixtureNode::div("root", leak_str(root_class), children),
    })
}

fn build_gap_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("gap", class_name),
        viewport_width: 320,
        viewport_height: 120,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!("flex w-[240px] h-[48px] {class_name}")),
            vec![
                FixtureNode::div("item-a", "w-[24px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[24px] h-[24px]", vec![]),
                FixtureNode::div("item-c", "w-[24px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_padding_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("padding", class_name),
        viewport_width: 320,
        viewport_height: 220,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "w-full h-full",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("w-[160px] h-[120px] {class_name}")),
                vec![FixtureNode::div("inner", "w-[24px] h-[24px]", vec![])],
            )],
        ),
    })
}

fn build_margin_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("margin", class_name),
        viewport_width: 960,
        viewport_height: 320,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "flex flex-col w-full h-full p-[16px] gap-[24px]",
            vec![
                FixtureNode::div(
                    "row-case",
                    "flex items-start w-[900px] h-[72px] gap-[8px]",
                    vec![
                        FixtureNode::div("row-before", "w-[48px] h-[24px] shrink-0", vec![]),
                        FixtureNode::div(
                            "row-target",
                            leak_str(format!("{class_name} w-[40px] h-[24px] shrink-0")),
                            vec![],
                        ),
                        FixtureNode::div("row-after", "w-[64px] h-[24px] shrink-0", vec![]),
                    ],
                ),
                FixtureNode::div(
                    "col-case",
                    "flex flex-col items-start w-[240px] gap-[8px]",
                    vec![
                        FixtureNode::div("col-before", "w-[56px] h-[24px]", vec![]),
                        FixtureNode::div(
                            "col-target",
                            leak_str(format!("{class_name} w-[40px] h-[24px]")),
                            vec![],
                        ),
                        FixtureNode::div("col-after", "w-[72px] h-[24px]", vec![]),
                    ],
                ),
            ],
        ),
    })
}

fn build_self_fixture(class_name: &str) -> Option<LayoutFixture> {
    let (parent_class, target_class) = match class_name {
        "self-auto" => (
            "flex items-end w-[220px] h-[120px] p-[8px] gap-[8px]",
            "self-auto w-[40px] h-[24px]",
        ),
        "self-start" => (
            "flex items-end w-[220px] h-[120px] p-[8px] gap-[8px]",
            "self-start w-[40px] h-[24px]",
        ),
        "self-center" => (
            "flex items-end w-[220px] h-[120px] p-[8px] gap-[8px]",
            "self-center w-[40px] h-[24px]",
        ),
        "self-end" => (
            "flex items-start w-[220px] h-[120px] p-[8px] gap-[8px]",
            "self-end w-[40px] h-[24px]",
        ),
        "self-stretch" => (
            "flex items-start w-[220px] h-[120px] p-[8px] gap-[8px]",
            "self-stretch w-[40px]",
        ),
        _ => return None,
    };

    Some(LayoutFixture {
        name: generated_fixture_name("self", class_name),
        viewport_width: 260,
        viewport_height: 160,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            parent_class,
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", leak_str(target_class), vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_gap_x_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("gap-x", class_name),
        viewport_width: 320,
        viewport_height: 120,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!("flex flex-row w-[280px] h-[64px] {class_name}")),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_gap_y_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("gap-y", class_name),
        viewport_width: 220,
        viewport_height: 220,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!("flex flex-col w-[180px] h-[200px] {class_name}")),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_min_width_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("min-width", class_name),
        viewport_width: 320,
        viewport_height: 180,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "w-full h-full",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("{class_name} h-[32px]")),
                vec![FixtureNode::text("inner", "text-[14px]", "min width")],
            )],
        ),
    })
}

fn build_max_width_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("max-width", class_name),
        viewport_width: 420,
        viewport_height: 180,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "w-full h-full",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("{class_name} h-[32px]")),
                vec![FixtureNode::text("inner", "text-[14px]", "max width content")],
            )],
        ),
    })
}

fn build_min_height_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("min-height", class_name),
        viewport_width: 320,
        viewport_height: 220,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "w-full h-full",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("{class_name} w-[120px]")),
                vec![FixtureNode::text("inner", "text-[14px]", "min height")],
            )],
        ),
    })
}

fn build_max_height_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("max-height", class_name),
        viewport_width: 320,
        viewport_height: 320,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "w-full h-full",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("{class_name} w-[120px]")),
                vec![FixtureNode::text("inner", "text-[14px]", "max height content that may overflow")],
            )],
        ),
    })
}

fn build_order_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("order", class_name),
        viewport_width: 320,
        viewport_height: 120,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "flex w-[280px] h-[64px] gap-[8px]",
            vec![
                FixtureNode::div("item-a", "order-3 w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", leak_str(format!("{class_name} w-[40px] h-[24px]")), vec![]),
                FixtureNode::div("item-c", "order-2 w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_translate_x_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("translate-x", class_name),
        viewport_width: 320,
        viewport_height: 180,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "relative w-full h-full",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("{class_name} w-[60px] h-[32px]")),
                vec![],
            )],
        ),
    })
}

fn build_translate_y_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("translate-y", class_name),
        viewport_width: 320,
        viewport_height: 180,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "relative w-full h-full",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("{class_name} w-[60px] h-[32px]")),
                vec![],
            )],
        ),
    })
}

fn build_visibility_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("visibility", class_name),
        viewport_width: 320,
        viewport_height: 180,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "relative w-full h-full",
            vec![
                FixtureNode::div("before", "w-[80px] h-[32px]", vec![]),
                FixtureNode::div(
                    "target",
                    leak_str(format!("{class_name} w-[80px] h-[32px]")),
                    vec![],
                ),
                FixtureNode::div("after", "w-[80px] h-[32px]", vec![]),
            ],
        ),
    })
}

fn build_box_sizing_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("box-sizing", class_name),
        viewport_width: 320,
        viewport_height: 180,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "w-full h-full",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("{class_name} w-[120px] h-[64px] p-[12px] border-4 border-black")),
                vec![],
            )],
        ),
    })
}

fn build_aspect_ratio_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("aspect-ratio", class_name),
        viewport_width: 320,
        viewport_height: 220,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "w-full h-full",
            vec![FixtureNode::div(
                "target",
                leak_str(format!("{class_name} w-[120px]")),
                vec![],
            )],
        ),
    })
}

fn build_place_self_fixture(class_name: &str) -> Option<LayoutFixture> {
    let (parent_class, target_class) = match class_name {
        "place-self-auto" => (
            "grid grid-cols-1 w-[220px] h-[160px] p-[8px] gap-[8px]",
            "place-self-auto w-[60px] h-[32px]",
        ),
        "place-self-start" => (
            "grid grid-cols-1 w-[220px] h-[160px] p-[8px] gap-[8px]",
            "place-self-start w-[60px] h-[32px]",
        ),
        "place-self-center" => (
            "grid grid-cols-1 w-[220px] h-[160px] p-[8px] gap-[8px]",
            "place-self-center w-[60px] h-[32px]",
        ),
        "place-self-end" => (
            "grid grid-cols-1 w-[220px] h-[160px] p-[8px] gap-[8px]",
            "place-self-end w-[60px] h-[32px]",
        ),
        "place-self-stretch" => (
            "grid grid-cols-1 w-[220px] h-[160px] p-[8px] gap-[8px]",
            "place-self-stretch h-[32px]",
        ),
        _ => return None,
    };

    Some(LayoutFixture {
        name: generated_fixture_name("place-self", class_name),
        viewport_width: 260,
        viewport_height: 200,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            parent_class,
            vec![FixtureNode::div(
                "target",
                leak_str(target_class),
                vec![],
            )],
        ),
    })
}

fn build_justify_items_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("justify-items", class_name),
        viewport_width: 260,
        viewport_height: 160,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!(
                "grid grid-cols-3 {class_name} w-[220px] h-[120px] p-[8px] gap-[8px]"
            )),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[40px] h-[32px]", vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[20px]", vec![]),
            ],
        ),
    })
}

fn build_justify_self_fixture(class_name: &str) -> Option<LayoutFixture> {
    let (parent_class, target_class) = match class_name {
        "justify-self-auto" => (
            "grid grid-cols-3 w-[220px] h-[120px] p-[8px] gap-[8px]",
            "justify-self-auto w-[60px] h-[24px]",
        ),
        "justify-self-start" => (
            "grid grid-cols-3 w-[220px] h-[120px] p-[8px] gap-[8px]",
            "justify-self-start w-[60px] h-[24px]",
        ),
        "justify-self-center" => (
            "grid grid-cols-3 w-[220px] h-[120px] p-[8px] gap-[8px]",
            "justify-self-center w-[60px] h-[24px]",
        ),
        "justify-self-end" => (
            "grid grid-cols-3 w-[220px] h-[120px] p-[8px] gap-[8px]",
            "justify-self-end w-[60px] h-[24px]",
        ),
        "justify-self-stretch" => (
            "grid grid-cols-3 w-[220px] h-[120px] p-[8px] gap-[8px]",
            "justify-self-stretch h-[24px]",
        ),
        _ => return None,
    };

    Some(LayoutFixture {
        name: generated_fixture_name("justify-self", class_name),
        viewport_width: 260,
        viewport_height: 160,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            parent_class,
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", leak_str(target_class), vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

// ── Grid build_fixture functions ──────────────────────────────────────────

fn build_grid_cols_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("grid-cols", class_name),
        viewport_width: 320,
        viewport_height: 160,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!("grid {class_name} w-[280px] h-[120px] gap-[8px] p-[8px]")),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[40px] h-[32px]", vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[20px]", vec![]),
                FixtureNode::div("item-d", "w-[40px] h-[28px]", vec![]),
                FixtureNode::div("item-e", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-f", "w-[40px] h-[20px]", vec![]),
            ],
        ),
    })
}

fn build_grid_rows_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("grid-rows", class_name),
        viewport_width: 320,
        viewport_height: 220,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!(
                "grid grid-cols-2 {class_name} w-[280px] h-[180px] gap-[8px] p-[8px]"
            )),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[40px] h-[32px]", vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[20px]", vec![]),
                FixtureNode::div("item-d", "w-[40px] h-[28px]", vec![]),
                FixtureNode::div("item-e", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-f", "w-[40px] h-[20px]", vec![]),
            ],
        ),
    })
}

fn build_grid_flow_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("grid-flow", class_name),
        viewport_width: 320,
        viewport_height: 180,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!(
                "grid grid-cols-3 {class_name} w-[280px] h-[140px] gap-[8px] p-[8px]"
            )),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[40px] h-[32px]", vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[20px]", vec![]),
                FixtureNode::div("item-d", "w-[40px] h-[28px]", vec![]),
                FixtureNode::div("item-e", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_auto_cols_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("auto-cols", class_name),
        viewport_width: 320,
        viewport_height: 160,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!("grid {class_name} w-[280px] h-[120px] gap-[8px] p-[8px]")),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[40px] h-[32px]", vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[20px]", vec![]),
                FixtureNode::div("item-d", "w-[40px] h-[28px]", vec![]),
                FixtureNode::div("item-e", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_auto_rows_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("auto-rows", class_name),
        viewport_width: 320,
        viewport_height: 180,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            leak_str(format!(
                "grid grid-cols-3 {class_name} w-[280px] h-[140px] gap-[8px] p-[8px]"
            )),
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div("item-b", "w-[40px] h-[32px]", vec![]),
                FixtureNode::div("item-c", "w-[40px] h-[20px]", vec![]),
                FixtureNode::div("item-d", "w-[40px] h-[28px]", vec![]),
                FixtureNode::div("item-e", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_col_start_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("col-start", class_name),
        viewport_width: 320,
        viewport_height: 140,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "grid grid-cols-3 w-[280px] h-[100px] gap-[8px] p-[8px]",
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div(
                    "target",
                    leak_str(format!("{class_name} w-[40px] h-[24px]")),
                    vec![],
                ),
                FixtureNode::div("item-c", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_col_end_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("col-end", class_name),
        viewport_width: 320,
        viewport_height: 140,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "grid grid-cols-3 w-[280px] h-[100px] gap-[8px] p-[8px]",
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div(
                    "target",
                    leak_str(format!("{class_name} w-[40px] h-[24px]")),
                    vec![],
                ),
                FixtureNode::div("item-c", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_row_start_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("row-start", class_name),
        viewport_width: 320,
        viewport_height: 200,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "grid grid-cols-3 grid-rows-3 w-[280px] h-[160px] gap-[8px] p-[8px]",
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div(
                    "target",
                    leak_str(format!("{class_name} w-[40px] h-[24px]")),
                    vec![],
                ),
                FixtureNode::div("item-c", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn build_row_end_fixture(class_name: &str) -> Option<LayoutFixture> {
    Some(LayoutFixture {
        name: generated_fixture_name("row-end", class_name),
        viewport_width: 320,
        viewport_height: 200,
        tolerance_px: 1.0,
        root: FixtureNode::div(
            "root",
            "grid grid-cols-3 grid-rows-3 w-[280px] h-[160px] gap-[8px] p-[8px]",
            vec![
                FixtureNode::div("item-a", "w-[40px] h-[24px]", vec![]),
                FixtureNode::div(
                    "target",
                    leak_str(format!("{class_name} w-[40px] h-[24px]")),
                    vec![],
                ),
                FixtureNode::div("item-c", "w-[40px] h-[24px]", vec![]),
            ],
        ),
    })
}

fn browser_layout_fixtures() -> Vec<LayoutFixture> {
    vec![
        LayoutFixture {
            name: "justify-center-three-cards",
            viewport_width: 360,
            viewport_height: 120,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-center items-center w-full h-full gap-[12px]",
                vec![
                    FixtureNode::div("card-a", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-b", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-c", "w-[48px] h-[48px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "items-center-column-stack",
            viewport_width: 240,
            viewport_height: 220,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col items-center w-[220px] h-[200px] gap-3 p-4",
                vec![
                    FixtureNode::div("item-a", "w-12 h-6", vec![]),
                    FixtureNode::div("item-b", "w-20 h-8", vec![]),
                    FixtureNode::div("item-c", "w-16 h-10", vec![]),
                ],
            ),
        },
        // ── shrink arbitrary constrained row ───────────────────────────
    LayoutFixture {
        name: "shrink-arbitrary-constrained-row",
            viewport_width: 300,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-center w-full h-full gap-2 px-4",
                vec![
                    FixtureNode::div("keep", "w-24 h-8 shrink-0", vec![]),
                    FixtureNode::div("shrink-two", "w-24 h-8 shrink-[2]", vec![]),
                    FixtureNode::div("shrink-one", "w-24 h-8 shrink", vec![]),
                    FixtureNode::div("shrink-three", "w-24 h-8 shrink-[3]", vec![]),
                ],
            ),
        },
        // ── absolute inset zero overlay ────────────────────────────────
    LayoutFixture {
        name: "absolute-inset-zero-overlay",
            viewport_width: 280,
            viewport_height: 160,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "relative w-full h-full p-4",
                vec![
                    FixtureNode::div("card", "relative w-32 h-20", vec![]),
                    FixtureNode::div("overlay", "absolute inset-0", vec![]),
                ],
            ),
        },
        // ── inset scale overlay bands ──────────────────────────────────
    LayoutFixture {
        name: "inset-scale-overlay-bands",
            viewport_width: 320,
            viewport_height: 200,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "relative w-full h-full",
                vec![
                    FixtureNode::div("header-band", "absolute inset-x-4 top-4 h-6", vec![]),
                    FixtureNode::div("footer-band", "absolute inset-x-px bottom-4 h-4", vec![]),
                    FixtureNode::div("left-band", "absolute left-6 inset-y-8 w-8", vec![]),
                ],
            ),
        },
        // ── text size stack ────────────────────────────────────────────
    LayoutFixture {
        name: "text-size-stack",
            viewport_width: 320,
            viewport_height: 220,
            tolerance_px: 8.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-full h-full gap-[10px] p-[20px]",
                vec![
                    FixtureNode::text("txt-xs", "text-xs", "Scale"),
                    FixtureNode::text("txt-sm", "text-sm", "Scale"),
                    FixtureNode::text("txt-lg", "text-lg", "Scale"),
                    FixtureNode::text("txt-xl", "text-xl", "Scale"),
                ],
            ),
        },
        // ── fixed width multisize copy ──────────────────────────────────
    LayoutFixture {
        name: "fixed-width-multisize-copy",
            viewport_width: 320,
            viewport_height: 240,
            tolerance_px: 8.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-[20px]",
                vec![FixtureNode::div(
                    "copy-card",
                    "flex flex-col gap-[8px] w-[180px]",
                    vec![
                        FixtureNode::div(
                            "copy-title-wrap",
                            "",
                            vec![FixtureNode::text(
                                "copy-title",
                                "text-[20px] leading-[24px]",
                                "Layout parity",
                            )],
                        ),
                        FixtureNode::text(
                            "copy-body",
                            "text-sm leading-relaxed tracking-[0.5px]",
                            "Compare browser layout and Taffy layout across multiple text styles.",
                        ),
                    ],
                )],
            ),
        },
        // ── items start column ─────────────────────────────────────────
    LayoutFixture {
        name: "items-start-column",
            viewport_width: 240,
            viewport_height: 220,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col items-start w-[220px] h-[200px] gap-3 p-4",
                vec![
                    FixtureNode::div("item-a", "w-12 h-6", vec![]),
                    FixtureNode::div("item-b", "w-20 h-8", vec![]),
                    FixtureNode::div("item-c", "w-16 h-10", vec![]),
                ],
            ),
        },
        // ── equal grow three columns ────────────────────────────────────
        LayoutFixture {
            name: "equal-grow-three-columns",
            viewport_width: 360,
            viewport_height: 120,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row w-full h-full gap-[8px] px-[12px]",
                vec![
                    FixtureNode::div("col-a", "grow h-full", vec![]),
                    FixtureNode::div("col-b", "grow h-full", vec![]),
                    FixtureNode::div("col-c", "grow h-full", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "flex-col-stretch-implicit-width",
            viewport_width: 320,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-[240px] h-[160px] p-[12px] gap-[8px]",
                vec![
                    FixtureNode::div("header", "w-full h-[32px]", vec![]),
                    FixtureNode::div("body", "w-full h-[48px]", vec![]),
                    FixtureNode::div("footer", "w-full h-[28px]", vec![]),
                ],
            ),
        },
        // ── sidebar + main layout ──────────────────────────────────────
        LayoutFixture {
            name: "chinese-text-wrap-narrow",
            viewport_width: 240,
            viewport_height: 200,
            tolerance_px: 8.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-[12px]",
                vec![FixtureNode::div(
                    "text-card",
                    "w-[180px]",
                    vec![
                        FixtureNode::text(
                            "headline",
                            "text-[15px] font-bold mb-[6px]",
                            "中文标题换行测试",
                        ),
                        FixtureNode::text(
                            "body",
                            "text-[12px] leading-relaxed",
                            "从微小的原子到浩瀚的宇宙，科学无处不在。保持好奇心，勇敢提问，每一次实验都是新的发现。",
                        ),
                    ],
                )],
            ),
        },
        // ── flex row with text auto-sizing and fixed box ────────────────
    ]
}
