//! 样式解析入口（HTML/JSONL 输入的树装配已统一由 `parse::dom` 承接）。
//!
//! 2026-11-17 M1 ⑩ 清扫：JSONL 组装链（`JsonLine` / `lines_to_dom` /
//! `composition_from_lines` 及配套装配函数）与无树上下文旧入口
//! `resolve_style` 全仓零消费，随"原始 hyperframes HTML 是唯一入口"
//! 收敛删除；本文件只保留 `resolve_style_ctx`。

use crate::style::NodeStyle;

/// 带树上下文的样式解析（docs/html-align/DESIGN.md）：祖先/前兄链供组合器
/// 选择器匹配。同时返回元素的计算自定义属性表（子元素继承的起点）。
pub(crate) fn resolve_style_ctx(
    css: Option<&crate::css::CssEngine>,
    class_name: &str,
    inline_style: Option<&str>,
    node_id: &str,
    tag: &str,
    sel_ctx: &crate::css::SelectorContext<'_>,
) -> (NodeStyle, crate::css::CssVars) {
    match css {
        Some(engine) => {
            engine.compute_node_style_ctx(class_name, node_id, tag, sel_ctx, inline_style)
        }
        None => {
            let mut style = NodeStyle {
                id: node_id.to_string(),
                ..NodeStyle::default()
            };
            let mut vars = sel_ctx.inherited_vars.cloned().unwrap_or_default();
            if let Some(text) = inline_style {
                match crate::css::CssEngine::from_css_text("", crate::css::EvalContext::default()) {
                    Ok(engine) => engine.apply_inline_style(
                        &mut style,
                        text,
                        sel_ctx.parent_font_px,
                        &mut vars,
                    ),
                    Err(err) => log::warn!("css: 内联 style 引擎不可用：{err}"),
                }
            }
            (style, vars)
        }
    }
}
