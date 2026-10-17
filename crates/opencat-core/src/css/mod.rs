//! 标准 CSS 样式引擎：解析真 CSS（tailwindcss CLI 编译产物、手写样式表），
//! 层叠后落到 [`NodeStyle`]。
//!
//! 类名 → 声明由标准编译器（tailwindcss CLI）负责；本模块只负责浏览器语义
//! 中「声明→计算值」这一段：选择器匹配、层叠、自定义属性求值（var/calc/
//! color-mix），然后把强类型 [`Property`] 映射到 NodeStyle 与 taffy。
//!
//! 求值策略：var() 替换与 calc()/color-mix() 化简全部在 TokenList 层完成，
//! 产出的字面值经 `Property::parse_string` 重新交还 lightningcss 解析成类型，
//! 映射层（`to_node_style`）只面对强类型。无法化简的声明（如 px/% 混合
//! calc）告警后丢弃——与浏览器的错误恢复语义一致。

mod cascade;
mod eval;
mod stylesheet;
mod to_node_style;
mod to_taffy;

pub use stylesheet::CssStylesheet;

use std::collections::HashMap;

use crate::style::NodeStyle;
use lightningcss::properties::Property;

/// 求值上下文：长度单位换算的基准。
#[derive(Debug, Clone, Copy)]
pub struct EvalContext {
    /// 合成画布尺寸，`vw`/`vh`/`vmin`/`vmax` 的基准。
    pub viewport: (f32, f32),
    /// `1rem` 对应的像素值。视频合成没有 DOM 根字号，固定 16px
    /// （与浏览器默认一致，tailwind 的 `--spacing: 0.25rem` 依赖它）。
    pub rem_base: f32,
}

impl Default for EvalContext {
    fn default() -> Self {
        Self {
            viewport: (1920.0, 1080.0),
            rem_base: 16.0,
        }
    }
}

/// 单条声明落到两条腿（paint 的 [`NodeStyle`] 与布局的 taffy [`Style`]）。
/// class 规则与内联 `style` 共用这一段，语义天然一致。
fn apply_declaration(
    style: &mut NodeStyle,
    taffy_style: &mut taffy::Style,
    property: &Property<'static>,
    vars: &mut HashMap<String, lightningcss::properties::custom::TokenList<'static>>,
    ctx: &EvalContext,
) {
    match property {
        Property::Custom(custom) => match &custom.name {
            // 变量键统一去掉 `--` 前缀，与 lookup_var 的取用约定一致。
            lightningcss::properties::custom::CustomPropertyName::Custom(name) => {
                let key = name.0.trim_start_matches("--").to_string();
                vars.insert(key, custom.value.clone());
            }
            // lightningcss 属性表缺项（object-fit 等）也是 Custom：
            // 喂给 paint 腿做关键字识别，布局腿无语义、跳过。
            lightningcss::properties::custom::CustomPropertyName::Unknown(_) => {
                to_node_style::apply_property(style, property, ctx);
            }
        },
        other => {
            let resolved = eval::resolve_to_typed(other, vars, ctx);
            for typed in resolved {
                // 布局腿直通 taffy；paint 腿继续喂 NodeStyle 既有字段。
                // 两条腿消费同一份强类型，值天然一致。
                to_taffy::apply_property(taffy_style, &typed, ctx);
                to_node_style::apply_property(style, &typed, ctx);
            }
        }
    }
}

/// 从一段 CSS 文本构建的样式引擎。
pub struct CssEngine {
    sheet: CssStylesheet,
    ctx: EvalContext,
}

impl CssEngine {
    pub fn from_css_text(css: &str, ctx: EvalContext) -> anyhow::Result<Self> {
        Ok(Self {
            sheet: CssStylesheet::parse(css)?,
            ctx,
        })
    }

    /// 把一个节点的 class 列表层叠成 [`NodeStyle`]。
    ///
    /// `tag` 是节点对应的 HTML 标签名（div/text→span 等），供元素选择器
    /// 匹配；`element_id` 供 `#id` 选择器匹配。两个都可以为空串。
    pub fn compute_node_style(&self, class_name: &str, element_id: &str, tag: &str) -> NodeStyle {
        let matched = cascade::match_rules(&self.sheet, class_name, element_id, tag);
        let mut style = NodeStyle {
            id: element_id.to_string(),
            ..NodeStyle::default()
        };
        let mut taffy_style = to_taffy::initial_style();
        let mut vars: HashMap<String, lightningcss::properties::custom::TokenList<'static>> =
            self.sheet.root_vars.clone();

        for decl in matched {
            apply_declaration(&mut style, &mut taffy_style, &decl.property, &mut vars, &self.ctx);
        }
        style.css_layout = Some(crate::style::CssLayout(taffy_style));
        style
    }

    /// 把内联 `style` 属性叠加到已算出的样式上。
    ///
    /// 内联声明在层叠里高于任何选择器规则，因此它在 class 结果之后应用。
    /// 解析失败的单条声明被丢弃（CSS 的错误恢复语义），不影响其余声明。
    pub fn apply_inline_style(&self, style: &mut NodeStyle, css: &str, _node_id: &str, _tag: &str) {
        // 与样式表同一策略：声明块借用输入文本，泄漏一份换 'static。
        // 内联样式是节点级小文本，量级可忽略。
        let owned: &'static str = Box::leak(css.to_string().into_boxed_str());
        let block = match lightningcss::declaration::DeclarationBlock::parse_string(
            owned,
            lightningcss::stylesheet::ParserOptions::default(),
        ) {
            Ok(block) => block,
            Err(err) => {
                log::warn!("css: 内联 style 解析失败，已忽略：{err:?}");
                return;
            }
        };
        let mut taffy_style = style
            .css_layout
            .take()
            .map(|layout| layout.0)
            .unwrap_or_else(to_taffy::initial_style);
        let mut vars: HashMap<String, lightningcss::properties::custom::TokenList<'static>> =
            self.sheet.root_vars.clone();
        for decl in block.declarations.iter().chain(block.important_declarations.iter()) {
            apply_declaration(style, &mut taffy_style, decl, &mut vars, &self.ctx);
        }
        style.css_layout = Some(crate::style::CssLayout(taffy_style));
    }

    /// `class_name` 里样式表完全没有引用的类。运行时拼接类
    /// （`left-[' + x + 'px]`）在 tailwind 静态扫描下不产出规则，
    /// 调用方用这份名单做诊断。
    pub fn unmatched_classes<'a>(&self, class_name: &'a str) -> Vec<&'a str> {
        class_name
            .split_whitespace()
            .filter(|c| !cascade::class_has_rules(&self.sheet, c))
            .collect()
    }

    /// 供诊断：命中某 class 的规则数（测试与回归对比用）。
    #[cfg(test)]
    pub fn matched_rule_count(&self, class_name: &str) -> usize {
        cascade::match_rules(&self.sheet, class_name, "", "div").len()
    }
}

#[cfg(test)]
mod tests;
