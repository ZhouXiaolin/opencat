//! CSS 文本 → 展平规则库。
//!
//! lightningcss 解析后递归下钻 `@layer`/`@supports`（条件求值通过才保留），
//! 把所有样式规则摊平成 [`FlatRule`] 并保留层叠所需的三个键：layer 序号、
//! specificity（selectors crate 直接给出）、文档顺序。`@media` 对视频合成
//! 没有语义，跳过。`:root` 的自定义属性单独抽出作为全局变量表。

use std::collections::HashMap;

use lightningcss::properties::custom::{Token, TokenList, TokenOrValue};
use lightningcss::properties::Property;
use lightningcss::rules::{supports::SupportsCondition, CssRule};
use lightningcss::selector::{Combinator, Component, Selector, SelectorList};
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::traits::{ParseWithOptions, ToCss};
use lightningcss::values::syntax::ParsedComponent;

/// 层叠顺序里 unlayered 规则的层号：CSS 规范里未分层规则排在所有
/// `@layer` 之上（后应用、获胜）。
pub const UNLAYERED: usize = usize::MAX;

pub struct CssStylesheet {
    pub rules: Vec<FlatRule>,
    /// `:root` 与 `@property initial-value` 提供的自定义属性
    /// （文档顺序，后者覆盖前者）。
    pub root_vars: HashMap<String, TokenList<'static>>,
    /// 顶层 `@layer` 名 → 层号（登记顺序即优先级顺序）。
    top_layers: HashMap<String, usize>,
}

/// 一条展平后的样式规则。
pub struct FlatRule {
    /// 文档顺序（同层同 specificity 时后者胜）。
    pub order: usize,
    /// `@layer` 序号；[`UNLAYERED`] 表示未分层。
    pub layer: usize,
    pub selectors: Vec<Selector<'static>>,
    pub declarations: Vec<Property<'static>>,
    pub important: Vec<Property<'static>>,
}

impl CssStylesheet {
    pub fn parse(css: &str) -> anyhow::Result<Self> {
        // lightningcss alpha.72 的 StyleSheet 借用输入（'i 贯穿规则、声明、
        // token）。alpha 阶段没有 into_owned()，这里泄漏输入文本换取
        // 'static：样式表与引擎同生命周期，文本体量是 KB 级，可接受。
        let owned: &'static str = Box::leak(css.to_string().into_boxed_str());
        let sheet = StyleSheet::parse(owned, ParserOptions::default())
            .map_err(|e| anyhow::anyhow!("CSS 解析失败: {e:?}"))?;
        let mut out = Self {
            rules: Vec::new(),
            root_vars: HashMap::new(),
            top_layers: HashMap::new(),
        };
        out.walk(&sheet.rules.0, UNLAYERED, None);
        Ok(out)
    }

    /// `parent` 是外层样式规则的选择器（CSS 嵌套上下文）：内层 `&` 选择器
    /// 与 `NestedDeclarations` 都落到它头上。
    fn walk(
        &mut self,
        rules: &[CssRule<'static>],
        layer: usize,
        parent: Option<&[Selector<'static>]>,
    ) {
        for rule in rules {
            match rule {
                CssRule::Style(style) => {
                    let selectors = match parent {
                        None => supported_selectors(&style.selectors),
                        Some(parent) => nested_selectors(&style.selectors, parent),
                    };
                    if !selectors.is_empty() {
                        self.rules.push(FlatRule {
                            order: self.rules.len(),
                            layer,
                            selectors: selectors.clone(),
                            declarations: style.declarations.declarations.clone(),
                            important: style.declarations.important_declarations.clone(),
                        });
                    }
                    self.walk(&style.rules.0, layer, Some(&selectors));
                }
                CssRule::NestedDeclarations(block) => {
                    let Some(parent) = parent else {
                        continue;
                    };
                    self.rules.push(FlatRule {
                        order: self.rules.len(),
                        layer,
                        selectors: parent.to_vec(),
                        declarations: block.declarations.declarations.clone(),
                        important: block.declarations.important_declarations.clone(),
                    });
                }
                CssRule::LayerBlock(block) => {
                    // 嵌套层名按点路径登记（"a" 内的 "b" → "a.b"）；同层
                    // 复用同一层号，保证层内源顺序参与层叠。
                    let child_layer = match &block.name {
                        Some(name) => self.layer_index(&name.0.join("."), layer),
                        None => layer,
                    };
                    self.walk(&block.rules.0, child_layer, parent);
                }
                CssRule::LayerStatement(stmt) => {
                    for name in &stmt.names {
                        self.layer_index(&name.0.join("."), layer);
                    }
                }

                CssRule::Supports(supports) => {
                    if supports_condition_holds(&supports.condition) {
                        self.walk(&supports.rules.0, layer, parent);
                    }
                }
                CssRule::Media(_) => {
                    // 视频合成没有媒体语义；debug 级日志帮助发现意外用法。
                    log::debug!("css: @media 规则在视频合成中无语义，已跳过");
                }
                CssRule::Property(prop) => {
                    // @property 的 initial-value 并入根变量表：tailwind 用
                    // 它给 --tw-shadow 等提供默认值。prop.name 已是 DashedIdent。
                    if let Some(initial) = &prop.initial_value {
                        let tokens = parsed_component_to_tokens(initial);
                        let key = prop.name.0.trim_start_matches("--").to_string();
                        self.root_vars.insert(key, tokens);
                    }
                }
                // @keyframes/@font-face 等在脚本动画与字体清单模型之外，
                // 第一阶段不支持。
                _ => {}
            }
        }
    }

    /// 登记层名并返回其层号。顶层声明（父层为 UNLAYERED）按登记顺序分配
    /// 新号；嵌套层沿用父层号（暂不区分嵌套层的独立顺序）。
    fn layer_index(&mut self, path: &str, parent: usize) -> usize {
        if parent == UNLAYERED {
            let next = self.top_layers.len();
            *self.top_layers.entry(path.to_string()).or_insert(next)
        } else {
            parent
        }
    }
}

/// 求值器能力表：我们支持 color-mix 与 LAB 系颜色，因此
/// `@supports (color: color-mix(in oklab, ...))` 应当选中——这正是
/// tailwind v4 输出渐进增强分支的方式。其余浏览器能力探测一律视为
/// 不成立（如 `-webkit-hyphens`、`contain-intrinsic-size`）。
fn supports_condition_holds(condition: &SupportsCondition) -> bool {
    match condition {
        SupportsCondition::Not(inner) => !supports_condition_holds(inner),
        SupportsCondition::And(list) => list.iter().all(supports_condition_holds),
        SupportsCondition::Or(list) => list.iter().any(supports_condition_holds),
        SupportsCondition::Declaration { value, .. } => {
            let v: &str = value.as_ref();
            v.contains("color-mix") || v.contains("oklab") || v.contains("oklch") || v.contains("lab(")
        }
        SupportsCondition::Selector(_) | SupportsCondition::Unknown(_) => false,
    }
}

/// CSS 嵌套内层选择器：只支持纯 `&`（隐式嵌套声明块）；带后代组合器
/// 的嵌套选择器第一阶段不支持。
fn nested_selectors(
    list: &SelectorList<'static>,
    parent: &[Selector<'static>],
) -> Vec<Selector<'static>> {
    let only_nesting = list.0.iter().all(|s| {
        s.iter_raw_match_order()
            .all(|c| matches!(c, Component::Nesting))
    });
    if only_nesting && !list.0.is_empty() {
        parent.to_vec()
    } else {
        Vec::new()
    }
}

/// 过滤掉带组合器/伪元素等暂不支持结构的 selector 分支。
/// 返回空表示整条规则没有任何可匹配分支。
fn supported_selectors(list: &SelectorList<'static>) -> Vec<Selector<'static>> {
    list.0
        .iter()
        .filter(|s| selector_is_supported(s))
        .cloned()
        .collect()
}

fn selector_is_supported(selector: &Selector<'static>) -> bool {
    let mut saw_combinator = false;
    for comp in selector.iter_raw_match_order() {
        match comp {
            Component::Combinator(c) => {
                // 伪元素派生的内部组合器不算真组合器。
                if !matches!(
                    c,
                    Combinator::PseudoElement | Combinator::SlotAssignment | Combinator::Part
                ) {
                    saw_combinator = true;
                }
            }
            // ::before 等伪元素在合成管线里无载体，分支作废。
            Component::PseudoElement(..) => return false,
            // hover/active 等伪类在静态合成里无意义，分支作废；:root 由
            // parcel_selectors 的 Component::Root 表达，走默认分支放行。
            Component::NonTSPseudoClass(_) => return false,
            Component::Slotted(..) | Component::Part(..) | Component::Host(..) => return false,
            _ => {}
        }
    }
    !saw_combinator
}

fn parsed_component_to_tokens(comp: &ParsedComponent<'static>) -> TokenList<'static> {
    fn push(tokens: &mut Vec<TokenOrValue<'static>>, comp: &ParsedComponent<'static>) {
        match comp {
            ParsedComponent::TokenList(tl) => tokens.extend(tl.0.iter().cloned()),
            ParsedComponent::Length(l) => {
                if let lightningcss::values::length::Length::Value(unit) = l {
                    tokens.push(TokenOrValue::Length(unit.clone()));
                }
            }
            ParsedComponent::Percentage(p) => {
                tokens.push(TokenOrValue::Token(Token::Percentage {
                    has_sign: false,
                    unit_value: p.0,
                    int_value: None,
                }))
            }
            ParsedComponent::Number(n) => {
                tokens.push(TokenOrValue::Token(Token::Number {
                    has_sign: *n < 0.0,
                    value: *n,
                    int_value: None,
                }))
            }
            ParsedComponent::Color(c) => tokens.push(TokenOrValue::Color(c.clone())),
            other => {
                // 复杂 initial 值（Image/Transform 等）先序列化回字符串再按
                // token 流取出，仅在非常规 @property 中出现。TokenList 借用
                // 输入，泄漏这段小文本换 'static（与样式表同策略）。
                if let Ok(s) = other.to_css_string(PrinterOptions::default()) {
                    let leaked: &'static str = Box::leak(s.into_boxed_str());
                    if let Ok(tl) =
                        TokenList::parse_string_with_options(leaked, ParserOptions::default())
                    {
                        tokens.extend(tl.0.iter().cloned());
                    }
                }
            }
        }
    }
    let mut tokens = Vec::new();
    push(&mut tokens, comp);
    TokenList(tokens)
}
