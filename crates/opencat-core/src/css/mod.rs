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
mod property_semantics;
mod stylesheet;
mod to_node_style;
mod to_taffy;

pub use cascade::{match_rules_ctx, SelectorContext, SelectorNode};
pub use eval::VarSource;
pub use property_semantics::{origin_px_to_fractions, parse_css_transform};
pub use to_node_style::{background_from_css, box_shadow_from_css};
pub use stylesheet::CssStylesheet;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::style::NodeStyle;
use lightningcss::properties::Property;
use lightningcss::properties::custom::{CustomPropertyName, TokenList};

/// 元素的计算自定义属性表（键去 `--` 前缀，与 lookup_var 的取用约定一致）。
/// 自定义属性按 CSS 规范沿祖先链继承：子元素以父元素的计算表为起点，
/// 再叠自身的规则与内联声明。
pub type CssVars = HashMap<String, lightningcss::properties::custom::TokenList<'static>>;

/// 求值上下文：长度单位换算的基准。
#[derive(Debug, Clone, Copy)]
pub struct EvalContext {
    /// 合成画布尺寸，`vw`/`vh`/`vmin`/`vmax` 的基准。
    pub viewport: (f32, f32),
    /// `1rem` 对应的像素值。视频合成没有 DOM 根字号，固定 16px
    /// （与浏览器默认一致，tailwind 的 `--spacing: 0.25rem` 依赖它）。
    pub rem_base: f32,
    /// `1em` 对应的像素值：元素自己的计算字号（`font-size` 自身的 em/%
    /// 相对父字号，级联时先按父字号求出它）。无树上下文时等于 rem_base。
    pub em_base: f32,
}

impl Default for EvalContext {
    fn default() -> Self {
        Self {
            viewport: (1920.0, 1080.0),
            rem_base: 16.0,
            em_base: 16.0,
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
        // 变量键统一去掉 `--` 前缀，与 lookup_var 的取用约定一致。
        Property::Custom(custom) if matches!(custom.name, CustomPropertyName::Custom(_)) => {
            if let CustomPropertyName::Custom(name) = &custom.name {
                let key = name.0.trim_start_matches("--").to_string();
                vars.insert(key, custom.value.clone());
            }
        }
        other => apply_value(style, taffy_style, other, &*vars, ctx),
    }
}

/// 非自定义属性的声明：求值后同时落两条腿。
fn apply_value<V: VarSource + ?Sized>(
    style: &mut NodeStyle,
    taffy_style: &mut taffy::Style,
    property: &Property<'static>,
    vars: &V,
    ctx: &EvalContext,
) {
    match property {
        // lightningcss 属性表缺项（object-fit 等）也是 Custom：
        // 喂给 paint 腿做关键字识别，布局腿无语义、跳过。
        Property::Custom(cp) => to_node_style::apply_custom_property(style, cp, vars, ctx),
        other => {
            for typed in eval::resolve_to_typed(other, vars, ctx) {
                // 布局腿直通 taffy；paint 腿继续喂 NodeStyle 既有字段。
                // 两条腿消费同一份强类型，值天然一致。
                to_taffy::apply_property(taffy_style, &typed, ctx);
                to_node_style::apply_property(style, &typed, ctx);
            }
        }
    }
}

/// 依赖 `var()` 的声明的重放现场。浏览器里自定义属性是活的：脚本按帧
/// `style.setProperty('--x', v)`（gsap 补间 `{"--x": v}` 同样落到这里）后，
/// 引用它的声明随之重算。层叠只跑一次，这里留下重算所需的最小现场，
/// resolve 期链上出现脚本覆盖时按层叠序重放（[`replay_var_deps`]）。
#[derive(Debug)]
pub struct VarDeps {
    /// 元素自身定义的自定义属性（根元素含 `:root` 表）：继承链上的一环。
    pub own: Arc<CssVars>,
    /// `own` 的 CSS 文本（键带 `--`），供脚本读 computed 值；不可序列化
    /// 的值（仍含 var() 等）不入表。
    pub own_text: Vec<(String, String)>,
    /// 与 var() 依赖声明相互作用的全部声明（同属性、简写/长写、变换族），
    /// 层叠序；后写覆盖先写的次序靠整组重放保持。
    replay: Vec<Property<'static>>,
    /// 变换列表与背景层是追加语义，重放前先清空、整族重来。
    resets_transforms: bool,
    resets_backgrounds: bool,
    ctx: EvalContext,
    parent_font_px: Option<f32>,
}

impl VarDeps {
    fn capture(
        decls: &[&Property<'static>],
        root_vars: Option<&CssVars>,
        ctx: &EvalContext,
        parent_font_px: Option<f32>,
    ) -> Option<Self> {
        let mut own = root_vars.cloned().unwrap_or_default();
        for property in decls {
            if let Property::Custom(custom) = property
                && let CustomPropertyName::Custom(name) = &custom.name
            {
                own.insert(name.0.trim_start_matches("--").to_string(), custom.value.clone());
            }
        }
        let dependent: HashSet<String> = decls
            .iter()
            .filter(|p| matches!(p, Property::Unparsed(_)))
            .flat_map(|p| touched_names(p))
            .collect();
        if dependent.is_empty() && own.is_empty() {
            return None;
        }
        let replay: Vec<Property<'static>> = decls
            .iter()
            .filter(|p| {
                !matches!(p, Property::Custom(c) if matches!(c.name, CustomPropertyName::Custom(_)))
            })
            .filter(|p| touched_names(p).iter().any(|name| dependent.contains(name)))
            .map(|p| (*p).clone())
            .collect();
        let touches = |name: &str| replay.iter().any(|p| touched_names(p).iter().any(|n| n == name));
        let own_text = own
            .iter()
            .filter_map(|(key, tokens)| {
                let text = eval::literal_text(tokens, ctx)?;
                Some((format!("--{key}"), text.trim().to_string()))
            })
            .collect();
        Some(Self {
            resets_transforms: touches(TRANSFORM_FAMILY),
            resets_backgrounds: touches("background-image"),
            own: Arc::new(own),
            own_text,
            replay,
            ctx: *ctx,
            parent_font_px,
        })
    }
}

/// 变换族（`transform` 与独立的 translate/rotate/scale）共用一张变换列表。
const TRANSFORM_FAMILY: &str = "<transform-family>";

/// 声明触及的属性名：自身、简写展开的长写、变换族标记。两条声明的名集
/// 相交即相互作用（重放时必须一起按层叠序重来）。
fn touched_names(property: &Property<'static>) -> Vec<String> {
    let id = match property {
        Property::Unparsed(unparsed) => unparsed.property_id.clone(),
        other => other.property_id(),
    };
    let mut names = vec![id.name().to_string()];
    if let Some(longhands) = id.longhands() {
        names.extend(longhands.iter().map(|l| l.name().to_string()));
    }
    if matches!(names[0].as_str(), "transform" | "translate" | "rotate" | "scale") {
        names.push(TRANSFORM_FAMILY.to_string());
    }
    names
}

/// resolve 期的自定义属性继承链：声明了自定义属性的元素挂一环（层叠
/// 结果），脚本按帧写入的覆盖值再挂一环。查找自近而远，同 CSS 继承。
#[derive(Debug)]
pub struct VarLink {
    vars: Arc<CssVars>,
    parent: Option<Arc<VarLink>>,
    /// 本环或祖先环带脚本覆盖：只有这时 var() 依赖声明才需要重放。
    pub scripted: bool,
}

impl VarLink {
    pub fn push(parent: Option<Arc<VarLink>>, vars: Arc<CssVars>, scripted: bool) -> Arc<Self> {
        let scripted = scripted || parent.as_ref().is_some_and(|link| link.scripted);
        Arc::new(Self { vars, parent, scripted })
    }
}

impl VarSource for VarLink {
    fn var(&self, key: &str) -> Option<&TokenList<'static>> {
        let mut link = Some(self);
        while let Some(current) = link {
            if let Some(value) = current.vars.get(key) {
                return Some(value);
            }
            link = current.parent.as_deref();
        }
        None
    }
}

/// 脚本写入的自定义属性值（`setProperty('--cx', '-552.85')`）→ TokenList。
pub fn parse_custom_value(name: &str, value: &str) -> Option<TokenList<'static>> {
    use lightningcss::properties::PropertyId;
    use lightningcss::traits::IntoOwned;
    let id = PropertyId::Custom(CustomPropertyName::Custom(
        lightningcss::values::ident::DashedIdent(name.to_string().into()),
    ));
    match Property::parse_string(id, value, lightningcss::stylesheet::ParserOptions::default()) {
        Ok(Property::Custom(custom)) => Some(custom.value.into_owned()),
        _ => None,
    }
}

/// 按新的变量取值重放 var() 依赖声明（脚本改写了链上的自定义属性）。
pub fn replay_var_deps<V: VarSource + ?Sized>(style: &mut NodeStyle, deps: &VarDeps, vars: &V) {
    if deps.replay.is_empty() {
        return;
    }
    let mut taffy_style = style
        .css_layout
        .take()
        .map(|layout| layout.0)
        .unwrap_or_else(to_taffy::initial_style);
    if deps.resets_transforms {
        style.transforms.clear();
    }
    if deps.resets_backgrounds {
        style.background_layers.clear();
    }
    // em 基准与层叠期同序：font-size 先按父字号求，其余按自身字号。
    let mut ctx = deps.ctx;
    let early = |p: &&Property<'static>| deps.parent_font_px.is_some() && is_font_size(p);
    if let Some(parent) = deps.parent_font_px {
        ctx.em_base = parent;
        for property in deps.replay.iter().filter(early) {
            apply_value(style, &mut taffy_style, property, vars, &ctx);
        }
        ctx.em_base = style.text_px.unwrap_or(parent);
    }
    for property in deps.replay.iter().filter(|p| !early(p)) {
        apply_value(style, &mut taffy_style, property, vars, &ctx);
    }
    if deps.resets_backgrounds {
        to_node_style::apply_background_size_to_layers(style);
    }
    style.css_layout = Some(crate::style::CssLayout(taffy_style));
}

fn is_font_size(property: &Property<'static>) -> bool {
    match property {
        Property::FontSize(_) => true,
        Property::Unparsed(unparsed) => {
            unparsed.property_id == lightningcss::properties::PropertyId::FontSize
        }
        _ => false,
    }
}

/// 一组声明按层叠序落地。自定义属性先于一切 `var()` 引用落定（CSS 的
/// 计算序：规则与内联的变量定义都在替换之前生效）。`em` 的基准是元素自己
/// 的计算字号，而 `font-size` 自身的 em/% 相对父字号：有父字号时先单独
/// 应用 font-size，再以求出的字号应用其余声明。
fn apply_declarations(
    style: &mut NodeStyle,
    taffy_style: &mut taffy::Style,
    decls: &[&Property<'static>],
    vars: &mut HashMap<String, lightningcss::properties::custom::TokenList<'static>>,
    base: &EvalContext,
    parent_font_px: Option<f32>,
) {
    let mut ctx = *base;
    if let Some(parent) = parent_font_px {
        ctx.em_base = parent;
    }
    let early = |p: &Property<'static>| {
        matches!(p, Property::Custom(_)) || (parent_font_px.is_some() && is_font_size(p))
    };
    for property in decls.iter().filter(|p| early(p)) {
        apply_declaration(style, taffy_style, property, vars, &ctx);
    }
    if let Some(parent) = parent_font_px {
        ctx.em_base = style.text_px.unwrap_or(parent);
    }
    for property in decls.iter().filter(|p| !early(p)) {
        apply_declaration(style, taffy_style, property, vars, &ctx);
    }
}

/// 内联 `style` 文本解析成声明块。与样式表同一策略：声明块借用输入文本，
/// 泄漏一份换 'static（节点级小文本，量级可忽略）。解析失败整段忽略。
fn parse_inline_block(
    css: &str,
) -> Option<lightningcss::declaration::DeclarationBlock<'static>> {
    let owned: &'static str = Box::leak(css.to_string().into_boxed_str());
    match lightningcss::declaration::DeclarationBlock::parse_string(
        owned,
        lightningcss::stylesheet::ParserOptions::default(),
    ) {
        Ok(block) => Some(block),
        Err(err) => {
            log::warn!("css: 内联 style 解析失败，已忽略：{err:?}");
            None
        }
    }
}

/// 从一段 CSS 文本构建的样式引擎。
pub struct CssEngine {
    sheet: Arc<CssStylesheet>,
    ctx: EvalContext,
}

impl CssEngine {
    /// 底层样式表（svg 内部元素绘制注入等跨元素匹配用）。Arc 供解析期把
    /// 注入现场挂到节点上、resolve 期复用。
    pub fn sheet_arc(&self) -> Arc<CssStylesheet> {
        Arc::clone(&self.sheet)
    }
}

/// UA 兜底样式表：HTML 语义标签的浏览器默认字形（字号/字重/斜体）。
/// 版式零漂移优先——不引入浏览器默认 margin（与既有 p 约定一致），
/// 间距一律由作者 class/inline 提供。层叠时整表压进 layer 0，
/// 作者规则（unlayered 或任何 @layer）都压过它。
const UA_CSS: &str = concat!(
    "h1 { font-size: 32px; font-weight: 700; }\n",
    "h2 { font-size: 24px; font-weight: 700; }\n",
    "h3 { font-size: 18.72px; font-weight: 700; }\n",
    "h4 { font-size: 16px; font-weight: 700; }\n",
    "h5 { font-size: 13.28px; font-weight: 700; }\n",
    "h6 { font-size: 10.72px; font-weight: 700; }\n",
    "b, strong { font-weight: 700; }\n",
    "i, em { font-style: italic; }\n",
);

fn ua_sheet() -> &'static CssStylesheet {
    use std::sync::OnceLock;
    static SHEET: OnceLock<CssStylesheet> = OnceLock::new();
    SHEET.get_or_init(|| CssStylesheet::parse(UA_CSS).expect("内置 UA 样式表解析失败"))
}

impl CssEngine {
    pub fn from_css_text(css: &str, ctx: EvalContext) -> anyhow::Result<Self> {
        Ok(Self {
            sheet: Arc::new(CssStylesheet::parse(css)?),
            ctx,
        })
    }

    /// 把一个节点的 class 列表层叠成 [`NodeStyle`]。
    ///
    /// `tag` 是节点对应的 HTML 标签名（div/text→span 等），供元素选择器
    /// 匹配；`element_id` 供 `#id` 选择器匹配。两个都可以为空串。
    pub fn compute_node_style(&self, class_name: &str, element_id: &str, tag: &str) -> NodeStyle {
        self.compute_node_style_ctx(
            class_name,
            element_id,
            tag,
            &cascade::SelectorContext::default(),
            None,
        )
        .0
    }

    /// 带树上下文的层叠入口（docs/html-align/DESIGN.md）：祖先链/前兄链
    /// 供组合器匹配，`tag_aliases` 让 HTML 根节点同时命中 html/body 规则。
    /// 内联 `style` 排在全部规则之后（层叠里高于任何选择器）；它的变量
    /// 定义与规则的一起先于 `var()` 替换生效（`.feel-look{background:
    /// var(--feel-bg)}` 读脚本写进内联的 `--feel-bg`）。同时返回元素的计算
    /// 自定义属性表（子元素继承的起点）。
    pub fn compute_node_style_ctx(
        &self,
        class_name: &str,
        element_id: &str,
        tag: &str,
        ctx: &cascade::SelectorContext<'_>,
        inline_style: Option<&str>,
    ) -> (NodeStyle, CssVars) {
        let matched =
            cascade::match_rules_ctx(&self.sheet, class_name, element_id, tag, ctx);
        // UA 兜底层压进 layer 0：作者规则（unlayered = usize::MAX 或任何
        // 显式 @layer）在层叠键上天然压过它，同 specificity 时 `text-4xl`
        // 这类作者字号照常覆盖 h2 的 UA 字号。去重是「后者胜」且不重排，
        // 故 UA 声明必须排在全部作者声明之前（曾追加在后，UA 字号反压作者）。
        let mut ua = cascade::match_rules_ctx(ua_sheet(), class_name, element_id, tag, ctx);
        for decl in &mut ua {
            decl.layer = 0;
        }
        ua.extend(matched);
        let matched = cascade::sort_and_dedup(ua);
        let mut style = NodeStyle {
            id: element_id.to_string(),
            ..NodeStyle::default()
        };
        let mut taffy_style = to_taffy::initial_style();
        let mut vars = ctx
            .inherited_vars
            .cloned()
            .unwrap_or_else(|| self.sheet.root_vars.clone());

        let inline = inline_style.and_then(parse_inline_block);
        let decls: Vec<&Property<'static>> = matched
            .iter()
            .map(|decl| &decl.property)
            .chain(inline.iter().flat_map(|block| {
                block.declarations.iter().chain(block.important_declarations.iter())
            }))
            .collect();
        apply_declarations(
            &mut style,
            &mut taffy_style,
            &decls,
            &mut vars,
            &self.ctx,
            ctx.parent_font_px,
        );
        to_node_style::apply_background_size_to_layers(&mut style);
        style.css_layout = Some(crate::style::CssLayout(taffy_style));
        // 变量继承链只在树形层叠里成立：根节点带根变量表，其余节点只带
        // 自身定义；无树上下文（JSX 通道）不留重放现场。
        if ctx.tree_root || ctx.inherited_vars.is_some() {
            style.var_deps = VarDeps::capture(
                &decls,
                ctx.tree_root.then_some(&self.sheet.root_vars),
                &self.ctx,
                ctx.parent_font_px,
            )
            .map(Arc::new);
        }
        (style, vars)
    }

    /// 把内联 `style` 属性叠加到已算出的样式上（无样式表的文档入口）。
    ///
    /// 内联声明在层叠里高于任何选择器规则，因此它在 class 结果之后应用。
    /// 解析失败的单条声明被丢弃（CSS 的错误恢复语义），不影响其余声明。
    /// `parent_font_px` 同 [`cascade::SelectorContext::parent_font_px`]；
    /// `vars` 是元素按规则层叠后的计算自定义属性表，内联变量定义叠在其上。
    pub fn apply_inline_style(
        &self,
        style: &mut NodeStyle,
        css: &str,
        parent_font_px: Option<f32>,
        vars: &mut CssVars,
    ) {
        let Some(block) = parse_inline_block(css) else {
            return;
        };
        let mut taffy_style = style
            .css_layout
            .take()
            .map(|layout| layout.0)
            .unwrap_or_else(to_taffy::initial_style);
        let decls: Vec<&Property<'static>> = block
            .declarations
            .iter()
            .chain(block.important_declarations.iter())
            .collect();
        apply_declarations(style, &mut taffy_style, &decls, vars, &self.ctx, parent_font_px);
        to_node_style::apply_background_size_to_layers(style);
        style.css_layout = Some(crate::style::CssLayout(taffy_style));
    }
}


