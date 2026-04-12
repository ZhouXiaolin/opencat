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
    let Some(env) = BrowserTestEnv::detect()? else {
        eprintln!(
            "skipping chromedriver Tailwind layout suite: ChromeDriver or Chrome is unavailable"
        );
        return Ok(());
    };

    let fixtures = browser_layout_fixtures();
    let runtime = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;

    runtime.block_on(async move {
        let browser = BrowserHarness::new(&env).await?;
        let mut failures = Vec::new();

        for fixture in fixtures {
            let css = compile_tailwind_css(&fixture)?;
            let html = fixture.render_html_document(&css);
            let html_path = write_fixture_file(&fixture.name, "html", &html)?;
            let browser_rects = browser.measure_layout(&html_path).await?;
            let taffy_rects = measure_taffy_layout(&fixture)?;

            if let Err(error) = assert_layouts_close(
                &fixture.name,
                &browser_rects,
                &taffy_rects,
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
                "chromedriver",
                "/opt/homebrew/bin/chromedriver",
                "/usr/local/bin/chromedriver",
            ],
        );
        let chrome_bin = find_executable(
            "CHROME_BIN",
            &[
                "google-chrome",
                "chromium",
                "chromium-browser",
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
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

        let session_id = create_session(
            &client,
            &webdriver_url,
            env.chrome_bin.as_ref(),
            1280,
            800,
        )
        .await?;

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
        .post(format!(
            "{webdriver_url}/session/{session_id}/{endpoint}"
        ))
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
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .context("failed to reserve a local TCP port")?;
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
            let delta = (browser_value - taffy_value).abs();
            if delta > tolerance_px {
                mismatches.push(format!(
                    "{id}.{field}: browser={browser_value:.2} taffy={taffy_value:.2} Δ={delta:.2}"
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
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create {}", dir.display()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_nanos();
    let path = dir.join(format!("{name}-{nonce}.{extension}"));
    fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

#[derive(Clone)]
struct LayoutFixture {
    name: &'static str,
    viewport_width: i32,
    viewport_height: i32,
    tolerance_px: f32,
    root: FixtureNode,
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
enum FixtureNodeKind {
    Div,
    Text(&'static str),
}

#[derive(Clone)]
struct FixtureNode {
    id: &'static str,
    class_name: &'static str,
    kind: FixtureNodeKind,
    children: Vec<FixtureNode>,
}

impl FixtureNode {
    fn div(id: &'static str, class_name: &'static str, children: Vec<FixtureNode>) -> Self {
        Self {
            id,
            class_name,
            kind: FixtureNodeKind::Div,
            children,
        }
    }

    fn text(id: &'static str, class_name: &'static str, content: &'static str) -> Self {
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

    fn collect_candidates_into(&self, out: &mut Vec<String>) {
        for class in self.class_name.split_whitespace() {
            out.push(class.to_string());
        }
        for child in &self.children {
            child.collect_candidates_into(out);
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

fn browser_layout_fixtures() -> Vec<LayoutFixture> {
    vec![
        LayoutFixture {
            name: "block-flow-stacks-siblings",
            viewport_width: 320,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![
                    FixtureNode::div(
                        "header",
                        "pt-[20px] pb-[20px]",
                        vec![FixtureNode::text("header-text", "text-[24px]", "Header")],
                    ),
                    FixtureNode::div(
                        "content",
                        "pt-[10px] pb-[10px]",
                        vec![FixtureNode::text("content-text", "text-[18px]", "Content")],
                    ),
                ],
            ),
        },
        LayoutFixture {
            name: "flex-row-justify-between",
            viewport_width: 390,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-between items-center w-full h-full px-[24px] py-[16px]",
                vec![
                    FixtureNode::div("left", "w-[56px] h-[56px]", vec![]),
                    FixtureNode::div("center", "w-[72px] h-[40px]", vec![]),
                    FixtureNode::div("right", "w-[56px] h-[56px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "flex-col-gap-padding",
            viewport_width: 280,
            viewport_height: 200,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-full h-full p-[20px]",
                vec![FixtureNode::div(
                    "card",
                    "flex flex-col gap-[12px] w-[180px] h-[120px] px-[16px] py-[12px]",
                    vec![
                        FixtureNode::div("title", "w-[90px] h-[20px]", vec![]),
                        FixtureNode::div("body", "w-[140px] h-[32px]", vec![]),
                        FixtureNode::div("footer", "w-[60px] h-[16px] mt-[4px]", vec![]),
                    ],
                )],
            ),
        },
        LayoutFixture {
            name: "text-wraps-within-parent-card-width",
            viewport_width: 220,
            viewport_height: 180,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![FixtureNode::div(
                    "card",
                    "w-[160px] px-[8px] py-[8px]",
                    vec![FixtureNode::text(
                        "body",
                        "text-[16px]",
                        "从微小的原子到浩瀚的宇宙，科学无处不在。保持好奇心，勇敢提问。",
                    )],
                )],
            ),
        },
        LayoutFixture {
            name: "absolute-inset-layout",
            viewport_width: 320,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "relative w-full h-full",
                vec![
                    FixtureNode::div(
                        "badge",
                        "absolute left-[12px] top-[10px] w-[80px] h-[24px]",
                        vec![],
                    ),
                    FixtureNode::div(
                        "panel",
                        "absolute right-[18px] bottom-[16px] w-[120px] h-[64px]",
                        vec![],
                    ),
                ],
            ),
        },
        LayoutFixture {
            name: "auto-sized-flex-column-labels",
            viewport_width: 390,
            viewport_height: 160,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-between w-full px-[20px] py-[16px]",
                vec![
                    FixtureNode::div(
                        "cat-pizza",
                        "flex flex-col items-center gap-[8px]",
                        vec![
                            FixtureNode::div("cat-pizza-icon", "w-[56px] h-[56px]", vec![]),
                            FixtureNode::text(
                                "cat-pizza-text",
                                "text-[12px] font-medium",
                                "Pizza",
                            ),
                        ],
                    ),
                    FixtureNode::div(
                        "cat-burger",
                        "flex flex-col items-center gap-[8px]",
                        vec![
                            FixtureNode::div("cat-burger-icon", "w-[56px] h-[56px]", vec![]),
                            FixtureNode::text(
                                "cat-burger-text",
                                "text-[12px] font-medium",
                                "Burger",
                            ),
                        ],
                    ),
                    FixtureNode::div(
                        "cat-sushi",
                        "flex flex-col items-center gap-[8px]",
                        vec![
                            FixtureNode::div("cat-sushi-icon", "w-[56px] h-[56px]", vec![]),
                            FixtureNode::text(
                                "cat-sushi-text",
                                "text-[12px] font-medium",
                                "Sushi",
                            ),
                        ],
                    ),
                    FixtureNode::div(
                        "cat-salad",
                        "flex flex-col items-center gap-[8px]",
                        vec![
                            FixtureNode::div("cat-salad-icon", "w-[56px] h-[56px]", vec![]),
                            FixtureNode::text(
                                "cat-salad-text",
                                "text-[12px] font-medium",
                                "Salad",
                            ),
                        ],
                    ),
                ],
            ),
        },
        LayoutFixture {
            name: "auto-sized-flex-column-prefers-single-line",
            viewport_width: 390,
            viewport_height: 160,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![FixtureNode::div(
                    "promo-banner",
                    "flex flex-row items-center w-[350px] px-[20px] py-[16px]",
                    vec![FixtureNode::div(
                        "promo-text",
                        "flex flex-col gap-[4px]",
                        vec![
                            FixtureNode::text(
                                "promo-title",
                                "text-[18px] font-bold",
                                "50% OFF",
                            ),
                            FixtureNode::text(
                                "promo-desc",
                                "text-[13px]",
                                "First order discount",
                            ),
                        ],
                    )],
                )],
            ),
        },
        LayoutFixture {
            name: "fixed-width-flex-column-text-wraps",
            viewport_width: 390,
            viewport_height: 160,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![FixtureNode::div(
                    "promo-text",
                    "flex flex-col w-[80px] gap-[4px]",
                    vec![
                        FixtureNode::text("promo-title", "text-[18px] font-bold", "50% OFF"),
                        FixtureNode::text(
                            "promo-desc",
                            "text-[13px]",
                            "First order discount",
                        ),
                    ],
                )],
            ),
        },
        LayoutFixture {
            name: "stretched-flex-column-card-wraps-text",
            viewport_width: 520,
            viewport_height: 220,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![FixtureNode::div(
                    "card",
                    "flex flex-col w-[440px] border-2 border-blue-200",
                    vec![FixtureNode::div(
                        "card-body",
                        "flex flex-col gap-[16px] p-[20px]",
                        vec![FixtureNode::text(
                            "card-text",
                            "text-[15px] text-slate-600 leading-relaxed",
                            "从微小的原子到浩瀚的宇宙，科学无处不在。保持好奇心，勇敢提问，每一次实验都是新的发现！",
                        )],
                    )],
                )],
            ),
        },
        LayoutFixture {
            name: "nested-full-width-shell",
            viewport_width: 420,
            viewport_height: 220,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-[20px]",
                vec![FixtureNode::div(
                    "shell",
                    "flex flex-col w-full h-full px-[12px] py-[10px]",
                    vec![
                        FixtureNode::div("header", "w-full h-[32px]", vec![]),
                        FixtureNode::div(
                            "content",
                            "flex flex-row justify-between w-full mt-[12px]",
                            vec![
                                FixtureNode::div("content-left", "w-[120px] h-[96px]", vec![]),
                                FixtureNode::div("content-right", "w-[180px] h-[96px]", vec![]),
                            ],
                        ),
                    ],
                )],
            ),
        },
        LayoutFixture {
            name: "fixed-width-flex-row-text-stays-single-line",
            viewport_width: 390,
            viewport_height: 120,
            tolerance_px: 6.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full",
                vec![FixtureNode::div(
                    "status-bar",
                    "flex flex-row justify-between items-center w-full h-[44px] px-[24px]",
                    vec![FixtureNode::text(
                        "status-time",
                        "text-[15px] font-semibold",
                        "9:41",
                    )],
                )],
            ),
        },
        LayoutFixture {
            name: "flex-column-children-stretch",
            viewport_width: 320,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-[320px] h-[180px]",
                vec![FixtureNode::div("header", "h-[40px]", vec![])],
            ),
        },
        LayoutFixture {
            name: "justify-around-three-cards",
            viewport_width: 360,
            viewport_height: 120,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-around items-center w-full h-full",
                vec![
                    FixtureNode::div("card-a", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-b", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-c", "w-[48px] h-[48px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "justify-start-three-cards",
            viewport_width: 360,
            viewport_height: 120,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-start items-center w-full h-full gap-[12px] px-[16px]",
                vec![
                    FixtureNode::div("card-a", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-b", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-c", "w-[48px] h-[48px]", vec![]),
                ],
            ),
        },
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
            name: "justify-end-three-cards",
            viewport_width: 360,
            viewport_height: 120,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-end items-center w-full h-full gap-[12px] px-[16px]",
                vec![
                    FixtureNode::div("card-a", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-b", "w-[48px] h-[48px]", vec![]),
                    FixtureNode::div("card-c", "w-[48px] h-[48px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "justify-evenly-four-pills",
            viewport_width: 420,
            viewport_height: 96,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row justify-evenly items-center w-full h-full",
                vec![
                    FixtureNode::div("pill-a", "w-[56px] h-[24px]", vec![]),
                    FixtureNode::div("pill-b", "w-[56px] h-[24px]", vec![]),
                    FixtureNode::div("pill-c", "w-[56px] h-[24px]", vec![]),
                    FixtureNode::div("pill-d", "w-[56px] h-[24px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "items-start-row",
            viewport_width: 320,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-start w-full h-full gap-[12px] px-[16px] py-[12px]",
                vec![
                    FixtureNode::div("box-a", "w-[40px] h-[24px]", vec![]),
                    FixtureNode::div("box-b", "w-[40px] h-[56px]", vec![]),
                    FixtureNode::div("box-c", "w-[40px] h-[36px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "items-center-row",
            viewport_width: 320,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-center w-full h-full gap-[12px] px-[16px] py-[12px]",
                vec![
                    FixtureNode::div("box-a", "w-[40px] h-[24px]", vec![]),
                    FixtureNode::div("box-b", "w-[40px] h-[56px]", vec![]),
                    FixtureNode::div("box-c", "w-[40px] h-[36px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "items-end-row",
            viewport_width: 320,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-end w-full h-full gap-[12px] px-[16px] py-[12px]",
                vec![
                    FixtureNode::div("box-a", "w-[40px] h-[24px]", vec![]),
                    FixtureNode::div("box-b", "w-[40px] h-[56px]", vec![]),
                    FixtureNode::div("box-c", "w-[40px] h-[36px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "items-stretch-row",
            viewport_width: 320,
            viewport_height: 140,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-row items-stretch w-full h-full gap-[12px] px-[16px] py-[12px]",
                vec![
                    FixtureNode::div("box-a", "w-[40px]", vec![]),
                    FixtureNode::div("box-b", "w-[40px]", vec![]),
                    FixtureNode::div("box-c", "w-[40px]", vec![]),
                ],
            ),
        },
        LayoutFixture {
            name: "row-gap-with-margins",
            viewport_width: 360,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "flex flex-col w-full h-full px-[16px] py-[12px]",
                vec![
                    FixtureNode::div("banner", "w-full h-[40px] mb-[8px]", vec![]),
                    FixtureNode::div(
                        "actions",
                        "flex flex-row gap-[12px] mt-[4px]",
                        vec![
                            FixtureNode::div("action-a", "w-[80px] h-[32px]", vec![]),
                            FixtureNode::div("action-b", "w-[80px] h-[32px]", vec![]),
                            FixtureNode::div("action-c", "w-[80px] h-[32px]", vec![]),
                        ],
                    ),
                ],
            ),
        },
        LayoutFixture {
            name: "spacing-scale-padding-card",
            viewport_width: 320,
            viewport_height: 180,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-4",
                vec![FixtureNode::div(
                    "card",
                    "w-32 h-20 px-4 py-2",
                    vec![
                        FixtureNode::div("card-title", "w-16 h-4 mb-2", vec![]),
                        FixtureNode::div("card-body", "w-20 h-8", vec![]),
                    ],
                )],
            ),
        },
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
        LayoutFixture {
            name: "tracking-wide-text-width",
            viewport_width: 420,
            viewport_height: 120,
            tolerance_px: 8.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-[20px]",
                vec![FixtureNode::text(
                    "headline",
                    "text-[18px] tracking-[2px] uppercase",
                    "OpenCat Layout",
                )],
            ),
        },
        LayoutFixture {
            name: "leading-relaxed-multiline-text",
            viewport_width: 280,
            viewport_height: 200,
            tolerance_px: 8.0,
            root: FixtureNode::div(
                "root",
                "w-full h-full p-[20px]",
                vec![FixtureNode::div(
                    "copy-wrap",
                    "w-[180px]",
                    vec![FixtureNode::text(
                        "copy",
                        "text-[14px] leading-relaxed",
                        "Tailwind layout parity should stay stable across browser and Taffy.",
                    )],
                )],
            ),
        },
        LayoutFixture {
            name: "absolute-corners-badges",
            viewport_width: 300,
            viewport_height: 160,
            tolerance_px: 1.0,
            root: FixtureNode::div(
                "root",
                "relative w-full h-full",
                vec![
                    FixtureNode::div("top-left", "absolute left-[8px] top-[8px] w-[36px] h-[20px]", vec![]),
                    FixtureNode::div("top-right", "absolute right-[8px] top-[8px] w-[36px] h-[20px]", vec![]),
                    FixtureNode::div("bottom-left", "absolute left-[8px] bottom-[8px] w-[36px] h-[20px]", vec![]),
                    FixtureNode::div("bottom-right", "absolute right-[8px] bottom-[8px] w-[36px] h-[20px]", vec![]),
                ],
            ),
        },
    ]
}
