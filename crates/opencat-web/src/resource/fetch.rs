#![cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

const PROXY_MAP: &[(&str, &str)] = &[("http://127.0.0.1:8080/", "/assets-proxy/")];

fn resolve_url(raw: &str) -> String {
    for (prefix, replacement) in PROXY_MAP {
        if raw.starts_with(prefix) {
            return replacement.to_string() + &raw[prefix.len()..];
        }
    }
    raw.to_string()
}

fn js_err(val: wasm_bindgen::JsValue) -> anyhow::Error {
    if let Some(err) = val.dyn_ref::<js_sys::Error>() {
        anyhow::anyhow!("fetch failed: {}", err.message())
    } else {
        let msg = val.as_string().unwrap_or_else(|| "unknown error".to_string());
        anyhow::anyhow!("fetch failed: {msg}")
    }
}

pub async fn fetch_bytes(url: &str) -> anyhow::Result<Vec<u8>> {
    let url = resolve_url(url);

    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);

    let request =
        Request::new_with_str_and_init(&url, &opts).map_err(js_err)?;

    let window = web_sys::window().ok_or_else(|| anyhow::anyhow!("no window"))?;
    let resp_value =
        JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(js_err)?;

    let resp: Response = resp_value
        .dyn_into()
        .map_err(|_| anyhow::anyhow!("fetch failed: expected Response"))?;

    let status = resp.status();
    if !(200..300).contains(&status) {
        return Err(anyhow::anyhow!("fetch failed: HTTP {status}"));
    }

    let buffer = JsFuture::from(resp.array_buffer().map_err(js_err)?)
        .await
        .map_err(js_err)?;

    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}
