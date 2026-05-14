//! `fetch()` 桥接 —— Rust 通过 `#[wasm_bindgen(module)]` extern 调 JS 侧
//! `fetch_bytes_js`，JS 内用原生 `fetch()` + `arrayBuffer()` 完成下载。

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/fetch_bridge.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn fetch_bytes_js(url: &str) -> std::result::Result<js_sys::Uint8Array, JsValue>;
}

pub async fn fetch_bytes(url: &str) -> anyhow::Result<Vec<u8>> {
    let arr = fetch_bytes_js(url).await.map_err(|e| {
        use wasm_bindgen::JsCast;
        let msg = if let Some(err) = e.dyn_ref::<js_sys::Error>() {
            err.message().into()
        } else {
            e.as_string()
                .unwrap_or_else(|| "unknown JS error".to_string())
        };
        anyhow::anyhow!("fetch failed: {msg}")
    })?;
    Ok(arr.to_vec())
}
