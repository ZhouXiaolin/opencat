//! 把 binding 调度从端侧收敛到 core。
//!
//! 端侧（engine 用 rquickjs；web wasm32 用 js_sys + wasm-bindgen）只需通过
//! [`crate::script::js_context::JsContext::install_dispatcher`] 注册唯一的
//! native 入口 `__opencatCallNative(name, ...args)`，dispatcher 内部调用
//! [`dispatch_binding`] 完成具体 binding 派发。
//!
//! 端侧无需展开 `for_each_binding!`，也无需认识任何 binding 名字。

use anyhow::{Result, anyhow};
use serde_json::Value;

use crate::for_each_binding;
use crate::ir::draw_op::{ColorF32, DRRectSpec, DrawOp, Radii4, Rect4};
use crate::ir::draw_types::{ImageRef, PaintId, PathOp};
use crate::script::mutations::TextUnitGranularity;
use crate::script::{
    align_items_from_name, box_shadow_from_name, drop_shadow_from_name, flex_direction_from_name,
    font_edging_from_name, inset_shadow_from_name, justify_content_from_name, line_cap_from_name,
    line_join_from_name, object_fit_from_name, position_from_name, text_align_from_name,
};
use crate::script::animate::state::{parse_easing_from_tag, random_from_seed};
use crate::script::recorder::{MutationRecorder, MutationStore, TextUnitValues};
use crate::script::text_units::{describe_text_units, grapheme_strings};
use crate::style::color_token_from_script_string;
use crate::style::{BorderStyle, FontWeight};
use crate::text::measure_script_text_width;

/// 把 node binding body 的多形态（`()` 与 `anyhow::Result<()>`）归一为 `Result<()>`。
///
/// `bindings.rs` 里 node 类条目两种写法并存：
///  - 单表达式: `$rec . record_opacity($id, v)`（返回 `()`）
///  - 带 `?` 的块: `{ let c = parse_color(&v)?; ...; Ok::<_, anyhow::Error>(()) }`
trait IntoNodeResult {
    fn into_result(self) -> Result<()>;
}

impl IntoNodeResult for () {
    #[inline]
    fn into_result(self) -> Result<()> {
        Ok(())
    }
}

impl IntoNodeResult for Result<()> {
    #[inline]
    fn into_result(self) -> Result<()> {
        self
    }
}

/// 按 name 派发 native binding。
///
/// `args` 长度允许小于声明 arity —— 缺位会被解码为 `Value::Null`，让 serde 报"参数类型错误"。
/// 未知 name 返回 `Err`。
pub fn dispatch_binding(store: &mut MutationStore, name: &str, args: &[Value]) -> Result<Value> {
    macro_rules! handle {
        // ── Node ──
        (node $rec:ident $id:ident $bn:ident
            ($id_param:ident : &str $(, $p:ident : $t:ty)*) $($body:tt)*) => {{
            if name == stringify!($bn) {
                let $rec = &mut *store as &mut dyn MutationRecorder;
                let $id_param: &str = args.get(0)
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("{}: missing arg 0 (id: &str)", stringify!($bn)))?;
                let mut __idx: usize = 1;
                $(
                    let $p: $t = serde_json::from_value(
                        args.get(__idx).cloned().unwrap_or(Value::Null),
                    )
                        .map_err(|e| anyhow!("{}: arg {} decode: {}", stringify!($bn), __idx, e))?;
                    __idx += 1;
                )*
                let _ = __idx;
                let __r: Result<()> = (|| -> Result<()> {
                    { $($body)* }.into_result()
                })();
                __r.map_err(|e| anyhow!("{}: {}", stringify!($bn), e))?;
                return Ok(Value::Null);
            }
        }};

        // ── Cmd ──
        (cmd $st:ident $bn:ident ($($p:ident : $t:ty),*) -> $ret:ty $body:block) => {{
            if name == stringify!($bn) {
                let mut __idx: usize = 0;
                $(
                    let $p: $t = serde_json::from_value(
                        args.get(__idx).cloned().unwrap_or(Value::Null),
                    )
                        .map_err(|e| anyhow!("{}: arg {} decode: {}", stringify!($bn), __idx, e))?;
                    __idx += 1;
                )*
                let _ = __idx;
                let $st = &mut *store;
                let __result: $ret = (|| -> Result<$ret> { $body })()
                    .map_err(|e| anyhow!("{}: {}", stringify!($bn), e))?;
                return serde_json::to_value(__result)
                    .map_err(|e| anyhow!("{}: encode return: {}", stringify!($bn), e));
            }
        }};

        // ── Qry ──
        (qry $st:ident $bn:ident ($($p:ident : $t:ty),*) -> $ret:ty $body:block) => {{
            if name == stringify!($bn) {
                let mut __idx: usize = 0;
                $(
                    let $p: $t = serde_json::from_value(
                        args.get(__idx).cloned().unwrap_or(Value::Null),
                    )
                        .map_err(|e| anyhow!("{}: arg {} decode: {}", stringify!($bn), __idx, e))?;
                    __idx += 1;
                )*
                let _ = __idx;
                let $st = &*store;
                let __result: $ret = (|| -> Result<$ret> { $body })()
                    .map_err(|e| anyhow!("{}: {}", stringify!($bn), e))?;
                return serde_json::to_value(__result)
                    .map_err(|e| anyhow!("{}: encode return: {}", stringify!($bn), e));
            }
        }};

        // ── Pure ──
        (pure $bn:ident ($($p:ident : $t:ty),*) -> $ret:ty $body:block) => {{
            if name == stringify!($bn) {
                let mut __idx: usize = 0;
                $(
                    let $p: $t = serde_json::from_value(
                        args.get(__idx).cloned().unwrap_or(Value::Null),
                    )
                        .map_err(|e| anyhow!("{}: arg {} decode: {}", stringify!($bn), __idx, e))?;
                    __idx += 1;
                )*
                let _ = __idx;
                let __result: $ret = (|| -> Result<$ret> { $body })()
                    .map_err(|e| anyhow!("{}: {}", stringify!($bn), e))?;
                return serde_json::to_value(__result)
                    .map_err(|e| anyhow!("{}: encode return: {}", stringify!($bn), e));
            }
        }};
    }

    for_each_binding!(rec id store handle);

    Err(anyhow!("unknown binding: {name}"))
}

/// 依次回调每条 binding 的名字。供 engine 在装载阶段生成 JS shim。
pub fn for_each_binding_name(mut f: impl FnMut(&str)) {
    macro_rules! emit {
        (node $rec:ident $id:ident $bn:ident $($rest:tt)*) => {
            f(stringify!($bn));
        };
        (cmd  $st:ident $bn:ident $($rest:tt)*) => {
            f(stringify!($bn));
        };
        (qry  $st:ident $bn:ident $($rest:tt)*) => {
            f(stringify!($bn));
        };
        (pure $bn:ident $($rest:tt)*) => {
            f(stringify!($bn));
        };
    }
    for_each_binding!(rec id store emit);
}

/// 生成 JS 端 shim 字符串：把每个 binding 名包成
/// `globalThis.__<name> = (...a) => __opencatCallNative('<name>', a);`。
///
/// runtime/*.js 仍按原方式直接调用 `__record_opacity(...)`、`__canvas_*(...)`，
/// 通过本 shim 路由到 dispatcher。
pub fn binding_shim_js() -> String {
    let mut buf = String::with_capacity(16 * 1024);
    for_each_binding_name(|n| {
        buf.push_str("globalThis.__");
        buf.push_str(n);
        buf.push_str("=function(){return __opencatCallNative('");
        buf.push_str(n);
        buf.push_str("',Array.from(arguments));};\n");
    });
    buf
}
