//! engine 端：rquickjs binding 注册逻辑 + 错误映射 helper。
//!
//! 本文件保存了"端侧专属"的两件事：
//!  - native callback 注册（`install_node_style_bindings` + `install_to_rquickjs!` 宏）
//!  - rquickjs 异常 → anyhow 的映射（`map_js_result`、`IntoAnyhow`）
//!
//! `RqJsContext` 与 `impl JsContext` 在下一任务追加到本文件。

use std::sync::{Arc, Mutex};

use rquickjs::{Context, Error as JsError, Exception, FromJs, Function, Object, Persistent, Runtime};

use opencat_core::for_each_binding;
use opencat_core::scene::script::mutations::{CanvasCommand, TextUnitGranularity};
use opencat_core::scene::script::object_fit_from_name;
use opencat_core::scene::script::{
    align_items_from_name, box_shadow_from_name, drop_shadow_from_name, flex_direction_from_name,
    font_edging_from_name, inset_shadow_from_name, justify_content_from_name, line_cap_from_name,
    line_join_from_name, point_mode_from_name, position_from_name, text_align_from_name,
};
use opencat_core::script::animate::state::{parse_easing_from_tag, random_from_seed};
use opencat_core::script::js_context::JsContext;
use opencat_core::script::recorder::{MutationRecorder, MutationStore, TextUnitValues};
use opencat_core::script::text_units::{describe_text_units, grapheme_strings};
use opencat_core::style::color_token_from_script_string;
use opencat_core::style::{BorderStyle, FontWeight};
use opencat_core::text::measure_script_text_width;

pub(crate) const NODE_STYLE_RUNTIME: &str = opencat_core::script::runtime::NODE_STYLE_RUNTIME;
pub(crate) const CANVASKIT_RUNTIME: &str = opencat_core::script::runtime::CANVAS_API_RUNTIME;
pub(crate) const ANIMATE_RUNTIME: &str = opencat_core::script::runtime::ANIMATION_RUNTIME;

// ── Error mapping helpers ────────────────────────────────────────────

pub(crate) fn map_js_result<T>(
    result: Result<T, JsError>,
    ctx: &rquickjs::Ctx<'_>,
    error_context: &str,
) -> anyhow::Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(JsError::Exception) => {
            let caught = ctx.catch();
            if let Ok(exception) = Exception::from_js(ctx, caught.clone()) {
                let message = exception
                    .message()
                    .unwrap_or_else(|| "uncaught JavaScript exception".to_string());
                let stack = exception.stack().unwrap_or_default();
                if stack.is_empty() {
                    anyhow::bail!("{error_context}: {message}");
                }
                anyhow::bail!("{error_context}: {message}\n{stack}");
            }
            anyhow::bail!("{error_context}: uncaught JavaScript exception");
        }
        Err(err) => Err(err.into()),
    }
}

pub(crate) trait IntoAnyhow {
    fn into_anyhow(self) -> anyhow::Result<()>;
}

impl IntoAnyhow for () {
    fn into_anyhow(self) -> anyhow::Result<()> {
        Ok(())
    }
}

impl IntoAnyhow for anyhow::Result<()> {
    fn into_anyhow(self) -> anyhow::Result<()> {
        self
    }
}

// ── rquickjs binding installation ────────────────────────────────────

pub(crate) fn install_node_style_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &Arc<Mutex<MutationStore>>,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    macro_rules! install_to_rquickjs {
        // ── Node commands ──
        (node $rec:ident $id:ident $name:ident ($first_param:ident : &str $(, $param:ident : $param_ty:ty)*) $($body:tt)*) => {{
            let s = store.clone();
            globals.set(
                concat!("__", stringify!($name)),
                Function::new(ctx.clone(), move |$first_param: String $(, $param: $param_ty)*| -> Result<(), rquickjs::Error> {
                    let mut guard = s.lock().map_err(|e| rquickjs::Error::new_from_js_message("script", "lock", &e.to_string()))?;
                    let $rec = &mut *guard as &mut dyn MutationRecorder;
                    let $id: &str = &$first_param;
                    (|| -> anyhow::Result<()> {
                        { $($body)* }.into_anyhow()
                    })().map_err(|e| rquickjs::Error::new_from_js_message("script", "script", &e.to_string()))
                })?,
            )?;
        }};

        // ── Store commands ──
        (cmd $store:ident $name:ident ($($param:ident : $param_ty:ty),*) -> $ret:ty $body:block) => {{
            let s = store.clone();
            globals.set(
                concat!("__", stringify!($name)),
                Function::new(ctx.clone(), move |$($param: $param_ty),*| -> Result<$ret, rquickjs::Error> {
                    let mut guard = s.lock().map_err(|e| rquickjs::Error::new_from_js_message("script", "lock", &e.to_string()))?;
                    let $store = &mut *guard;
                    (|| -> anyhow::Result<$ret> { $body })()
                        .map_err(|e| rquickjs::Error::new_from_js_message("script", "script", &e.to_string()))
                })?,
            )?;
        }};

        // ── Store queries ──
        (qry $store:ident $name:ident ($($param:ident : $param_ty:ty),*) -> $ret:ty $body:block) => {{
            let s = store.clone();
            globals.set(
                concat!("__", stringify!($name)),
                Function::new(ctx.clone(), move |$($param: $param_ty),*| -> Result<$ret, rquickjs::Error> {
                    let guard = s.lock().map_err(|e| rquickjs::Error::new_from_js_message("script", "lock", &e.to_string()))?;
                    let $store = &*guard;
                    (|| -> anyhow::Result<$ret> { $body })()
                        .map_err(|e| rquickjs::Error::new_from_js_message("script", "script", &e.to_string()))
                })?,
            )?;
        }};

        // ── Pure functions ──
        (pure $name:ident ($($param:ident : $param_ty:ty),*) -> $ret:ty $body:block) => {{
            globals.set(
                concat!("__", stringify!($name)),
                Function::new(ctx.clone(), move |$($param: $param_ty),*| -> Result<$ret, rquickjs::Error> {
                    (|| -> anyhow::Result<$ret> { $body })()
                        .map_err(|e| rquickjs::Error::new_from_js_message("script", "script", &e.to_string()))
                })?,
            )?;
        }};
    }

    for_each_binding!(rec id store install_to_rquickjs);

    Ok(())
}

// ── RqJsContext ──────────────────────────────────────────────────────

pub struct RqJsContext {
    context: Context,
    store: Arc<Mutex<MutationStore>>,
    ctx_obj: Persistent<Object<'static>>,
    _runtime: Runtime,
}

impl JsContext for RqJsContext {
    fn new() -> anyhow::Result<Self> {
        let runtime = Runtime::new()?;
        let context = Context::full(&runtime)?;
        let store = Arc::new(Mutex::new(MutationStore::default()));

        let ctx_obj = context.with(|ctx| -> anyhow::Result<_> {
            let obj = Object::new(ctx.clone())?;
            ctx.globals().set("ctx", obj.clone())?;
            Ok(Persistent::save(&ctx, obj))
        })?;

        Ok(Self {
            context,
            store,
            ctx_obj,
            _runtime: runtime,
        })
    }

    fn eval(&self, code: &str) -> anyhow::Result<()> {
        self.context.with(|ctx| {
            map_js_result(ctx.eval::<(), _>(code), &ctx, "script eval")
        })
    }

    fn set_ctx_field_i64(&self, name: &str, v: i64) -> anyhow::Result<()> {
        self.context.with(|ctx| -> anyhow::Result<()> {
            let obj = self.ctx_obj.clone().restore(&ctx)?;
            obj.set(name, v)?;
            Ok(())
        })
    }

    fn set_ctx_field_str(&self, name: &str, v: &str) -> anyhow::Result<()> {
        self.context.with(|ctx| -> anyhow::Result<()> {
            let obj = self.ctx_obj.clone().restore(&ctx)?;
            obj.set(name, v)?;
            Ok(())
        })
    }

    fn call_global_fn(&self, name: &str) -> anyhow::Result<()> {
        self.context.with(|ctx| -> anyhow::Result<()> {
            let f: Function = ctx.globals().get(name)?;
            map_js_result(f.call::<(), ()>(()), &ctx, name)
        })
    }

    fn install_all_bindings(&self) -> anyhow::Result<()> {
        self.context.with(|ctx| -> anyhow::Result<()> {
            install_node_style_bindings(&ctx, &self.store)
        })
    }

    fn with_store_mut<R>(&self, f: impl FnOnce(&mut MutationStore) -> R) -> R {
        let mut guard = self.store.lock().expect("script store lock poisoned");
        f(&mut guard)
    }
}
