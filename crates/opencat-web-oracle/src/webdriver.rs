//! WebDriver 会话原语：chromedriver/msedgedriver 的进程引导、会话建立与
//! POST 封装。`BrowserHarness`（`harness.rs`）消费这些原语；本模块不持有
//! 会话语义，只做 HTTP/进程层的薄封装。

use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Client;
use serde_json::{Value, json};

pub(crate) fn find_executable(env_var: &str, fallbacks: &[&str]) -> Option<PathBuf> {
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

pub(crate) async fn wait_for_webdriver_ready(client: &Client, webdriver_url: &str) -> Result<()> {
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

pub(crate) async fn create_session(
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
            // WebGPU 原生面（shaders 语料 web 腿）：headless 下 navigator.gpu
            // 需要显式开启；ref 腿（shoot.mjs）同旗标，两侧同 adapter。
            // 注意不挂 --use-gl/--use-angle：ANGLE/SwiftShader 组会改变 Dawn
            // 的 adapter 选取，shaders 对拍两侧必须同 WebGPU 后端。
            "--enable-unsafe-webgpu",
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
                    // browserName 不设：chromedriver 隐含 chrome、msedgedriver
                    // 隐含 msedge；写死任一名都会被另一家拒（msedgedriver 报
                    // No matching capabilities，chromedriver 报 unrecognized
                    // Chrome version: Edg/*）。binary capability 已显式指向
                    // 浏览器可执行文件。
                    "goog:loggingPrefs": { "browser": "ALL" },
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

pub(crate) async fn wait_for_document_ready(
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

pub(crate) async fn webdriver_post(
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

pub(crate) fn reserve_port() -> Result<u16> {
    let listener =
        TcpListener::bind("127.0.0.1:0").context("failed to reserve a local TCP port")?;
    let port = listener
        .local_addr()
        .context("failed to inspect reserved TCP port")?
        .port();
    drop(listener);
    Ok(port)
}
