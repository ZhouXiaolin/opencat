//! Optional host-provided VFS reader for web builds.
//!
//! The standalone web target has no filesystem. A host app can register a JS
//! function that maps logical paths (for example `/workspace/assets/a.png`) to
//! bytes, allowing web `path` resources to participate in the normal preload
//! pipeline.

use std::cell::RefCell;

use anyhow::Result;
use js_sys::{Array, ArrayBuffer, Function, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

thread_local! {
    static ASSET_READER: RefCell<Option<Function>> = const { RefCell::new(None) };
}

pub fn set_reader(reader: Function) {
    ASSET_READER.with(|slot| *slot.borrow_mut() = Some(reader));
}

pub fn clear_reader() {
    ASSET_READER.with(|slot| *slot.borrow_mut() = None);
}

pub async fn read_path(path: &str) -> Result<Vec<u8>> {
    let reader = ASSET_READER
        .with(|slot| slot.borrow().clone())
        .ok_or_else(|| anyhow::anyhow!("asset reader is not configured"))?;

    let value = reader
        .call1(&JsValue::NULL, &JsValue::from_str(path))
        .map_err(js_err)?;
    let value = JsFuture::from(js_sys::Promise::resolve(&value))
        .await
        .map_err(js_err)?;

    js_value_to_bytes(value)
}

fn js_value_to_bytes(value: JsValue) -> Result<Vec<u8>> {
    if value.is_instance_of::<Uint8Array>() {
        return Ok(Uint8Array::new(&value).to_vec());
    }
    if value.is_instance_of::<ArrayBuffer>() {
        return Ok(Uint8Array::new(&value).to_vec());
    }
    if Array::is_array(&value) {
        return Ok(Uint8Array::from(value).to_vec());
    }

    Err(anyhow::anyhow!(
        "asset reader returned unsupported value; expected Uint8Array, ArrayBuffer, or byte array"
    ))
}

fn js_err(value: JsValue) -> anyhow::Error {
    if let Some(err) = value.dyn_ref::<js_sys::Error>() {
        anyhow::anyhow!("{}", err.message())
    } else {
        anyhow::anyhow!(
            "{}",
            value
                .as_string()
                .unwrap_or_else(|| "unknown JS error".to_string())
        )
    }
}
