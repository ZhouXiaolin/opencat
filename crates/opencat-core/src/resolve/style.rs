use crate::canvas::paint::BlendMode;
use crate::style::{
    AlignItems, BackgroundFill, BoxShadow, ClipPath, ComputedTextStyle, CssFilter, CssLayout,
    DropShadow, FlexDirection, FlexWrap, GridAutoFlow, GridAutoRows, GridPlacement, InsetShadow,
    JustifyContent, LengthPercentageAuto, ObjectFit, Position, TextShadow, Transform,
    TransformOrigin,
};

/// 绝对定位盒的宽度是否确定。CSS §10.3.7：`left` 与 `right` 都非 auto 且
/// `width` 为 auto 时，宽度由包含块 − left − right 解出（taffy 已按此定
/// 尺寸），不是 shrink-to-fit。
fn absolute_definite_width(layout: &ComputedLayoutStyle) -> bool {
    let definite = |v: Option<crate::style::LengthPercentageAuto>| {
        matches!(
            v,
            Some(crate::style::LengthPercentageAuto::Length(_))
                | Some(crate::style::LengthPercentageAuto::Percent(_))
        )
    };
    definite(layout.inset_left) && definite(layout.inset_right)
}

/// 文本是否允许换行。布局测量与显示树构建共用同一条判定。
///
/// `truncate`/`white-space: nowrap` 强制单行；绝对定位且宽度不确定的元素
/// 按 shrink-to-fit 由内容决定宽度，不换行；其余情况由继承来的
/// `wrap_text` 或确定宽度决定。
pub fn text_element_allows_wrap(element: &crate::resolve::tree::ElementNode) -> bool {
    if element.style.layout.truncate {
        return false;
    }

    if element.style.text.no_wrap {
        return false;
    }

    let has_definite_width = element.style.layout.width.is_some()
        || element.style.layout.width_percent.is_some()
        || (element.style.layout.position == Position::Absolute
            && absolute_definite_width(&element.style.layout));

    if element.style.layout.position == Position::Absolute && !has_definite_width {
        return false;
    }

    element.style.text.wrap_text || has_definite_width
}

#[derive(Clone, Debug)]
pub struct ComputedStyle {
    pub layout: ComputedLayoutStyle,
    pub visual: ComputedVisualStyle,
    pub text: ComputedTextStyle,
    pub id: String,
    /// CSS 布局腿直通产物（`css::to_taffy`），布局消费端的第一优先输入；
    /// 旧类名路径为 `None`。
    pub css_layout: Option<CssLayout>,
    /// CSS `container-type`：本元素建立尺寸查询容器。
    pub container_type: Option<crate::style::ContainerType>,
    /// 最近祖先尺寸查询容器的内容盒尺寸；`cqw`/`cqh` 的求值基准。
    pub query_container: Option<(f32, f32)>,
}

/// Only properties in this struct are allowed to flow from parent to child.
/// Everything else remains local to the node that declared it, or applies to the
/// rendered subtree at paint time.
#[derive(Clone, Debug, Default)]
pub struct InheritedStyle {
    pub text: ComputedTextStyle,
    /// 父元素的计算 `visibility: hidden`。
    pub hidden: bool,
    /// 自定义属性继承链（脚本按帧改写变量时 var() 依赖声明据此重算）。
    pub vars: Option<std::sync::Arc<crate::css::VarLink>>,
    /// 最近尺寸查询容器（含祖先链）的内容盒尺寸。
    pub query_container: Option<(f32, f32)>,
}

impl InheritedStyle {
    pub fn for_child(style: &ComputedStyle) -> Self {
        let mut text = style.text.clone();
        let container_has_definite_width = style.layout.width.is_some()
            || style.layout.width_percent.is_some()
            || (style.layout.position == Position::Absolute
                && absolute_definite_width(&style.layout));
        // `position: absolute` with `width: auto` resolves via CSS shrink-to-fit —
        // its width comes from its own content, not from the containing block.
        // Inline-width constraints from ancestors must stop at such a boundary,
        // otherwise `wrap_text` leaks past it and breaks shrink-to-fit for text.
        // The left+right inset case (§10.3.7) has a definite width and keeps it.
        let breaks_width_inheritance =
            style.layout.position == Position::Absolute && !container_has_definite_width;
        let parent_has_inline_constraint = if breaks_width_inheritance {
            false
        } else {
            container_has_definite_width || style.text.wrap_text
        };
        let parent_stacks_text_vertically = !style.layout.is_flex
            || ((style.layout.flex_direction == FlexDirection::Col
                || style.layout.flex_direction == FlexDirection::ColReverse)
                && style.layout.align_items == AlignItems::Stretch);
        text.wrap_text = parent_has_inline_constraint && parent_stacks_text_vertically;
        Self {
            text,
            hidden: style.visual.hidden,
            vars: None,
            query_container: style.query_container,
        }
    }

    /// 子元素继承的自定义属性链（本元素 resolve 期挂好的那一环）。
    pub fn with_vars(mut self, vars: Option<std::sync::Arc<crate::css::VarLink>>) -> Self {
        self.vars = vars;
        self
    }
}

crate::fp_struct! {
    #[derive(Clone, Debug)]
    pub struct ComputedLayoutStyle {
        pub position: Position => [Layout],
        /// 显式定位（CSS `relative`/`absolute`/`fixed`）：是绝对定位后代的包含块。
        /// `position` 把 static 与 relative 都记成 Relative，这一位补上二者之差。
        pub positioned: bool => [Layout],
        pub inset_left: Option<LengthPercentageAuto> => [Layout],
        pub inset_top: Option<LengthPercentageAuto> => [Layout],
        pub inset_right: Option<LengthPercentageAuto> => [Layout],
        pub inset_bottom: Option<LengthPercentageAuto> => [Layout],
        pub width: Option<f32> => [Layout] opt_bits,
        pub width_percent: Option<f32> => [Layout] opt_bits,
        pub height_percent: Option<f32> => [Layout] opt_bits,
        pub height: Option<f32> => [Layout] opt_bits,
        pub max_width: Option<f32> => [Layout] opt_bits,
        pub padding_top: f32 => [Layout] bits,
        pub padding_right: f32 => [Layout] bits,
        pub padding_bottom: f32 => [Layout] bits,
        pub padding_left: f32 => [Layout] bits,
        pub margin_top: LengthPercentageAuto => [Layout],
        pub margin_right: LengthPercentageAuto => [Layout],
        pub margin_bottom: LengthPercentageAuto => [Layout],
        pub margin_left: LengthPercentageAuto => [Layout],
        pub min_height: Option<LengthPercentageAuto> => [Layout],
        pub min_width: Option<LengthPercentageAuto> => [Layout],
        pub is_flex: bool => [Layout],
        pub is_grid: bool => [Layout],
        /// 行内格式化（docs/html-align/DESIGN.md）：HTML 前端里子节点全为
        /// span 的 div 是一行行内内容；布局端在 taffy 之后按 advance 串接
        /// 重排（taffy 无行内流）。浏览器语义的真实现，非近似节点。
        pub inline_flow: bool => [Layout],
        pub inline_level: bool => [Layout],
        /// 原子行内盒（inline-block/inline-flex/…）：行内流里整体排行。
        pub inline_atomic: bool => [Layout],
        /// 行内流里原子盒的 `vertical-align`。
        pub vertical_align: crate::style::VerticalAlign => [Layout],
        pub grid_template_columns: Option<u16> => [Layout],
        pub grid_template_rows: Option<u16> => [Layout],
        pub grid_auto_flow: Option<GridAutoFlow> => [Layout],
        pub grid_auto_rows: Option<GridAutoRows> => [Layout],
        pub col_start: Option<GridPlacement> => [Layout],
        pub col_end: Option<GridPlacement> => [Layout],
        pub row_start: Option<GridPlacement> => [Layout],
        pub row_end: Option<GridPlacement> => [Layout],
        pub auto_size: bool => [Layout],
        pub flex_direction: FlexDirection => [Layout],
        pub justify_content: JustifyContent => [Layout],
        pub align_items: AlignItems => [Layout],
        pub flex_wrap: FlexWrap => [Layout],
        pub align_content: Option<JustifyContent> => [Layout],
        pub align_self: Option<AlignItems> => [Layout],
        pub justify_items: Option<AlignItems> => [Layout],
        pub justify_self: Option<AlignItems> => [Layout],
        pub gap: f32 => [Layout] bits,
        pub gap_x: Option<f32> => [Layout] opt_bits,
        pub gap_y: Option<f32> => [Layout] opt_bits,
        pub order: i32 => [Layout],
        pub aspect_ratio: Option<f32> => [Layout] opt_bits,
        pub flex_basis: Option<LengthPercentageAuto> => [Layout],
        pub flex_grow: f32 => [Layout] bits,
        pub flex_shrink: Option<f32> => [Layout] opt_bits,
        pub z_index: i32 => [Layout],
        pub truncate: bool => [Layout],
    }
}

crate::fp_struct! {
    #[derive(Clone, Debug)]
    pub struct ComputedVisualStyle {
        pub opacity: f32 => [Apply] bits,
        /// 计算 `visibility: hidden`：不画自己（盒子仍占布局），子元素可以
        /// 用 `visible` 重新显示。
        pub hidden: bool => [Paint],
        // blend_mode / background_blend_mode 是绘制输入（进 OCIR 与合成），
        // 3b 补进 Paint 维。
        pub blend_mode: BlendMode => [Paint],
        pub background_blend_mode: BlendMode => [Paint],
        pub background: Vec<BackgroundFill> => [Paint],
        // background_clip 只影响渐变文本填充，display 层 TextFp 经
        // text_fill 覆盖（历史行为；3b 复核元素级是否补）。
        pub background_clip: crate::style::BackgroundClip => [],
        pub fill: Option<BackgroundFill> => [Paint],
        pub border_radius: crate::style::BorderRadius => [Paint],
        pub border_width: Option<f32> => [Paint] opt_bits,
        pub border_top_width: Option<f32> => [Paint] opt_bits,
        pub border_right_width: Option<f32> => [Paint] opt_bits,
        pub border_bottom_width: Option<f32> => [Paint] opt_bits,
        pub border_left_width: Option<f32> => [Paint] opt_bits,
        pub border_color: Option<crate::style::ColorToken> => [Paint],
        pub border_top_color: Option<crate::style::ColorToken> => [Paint],
        pub border_right_color: Option<crate::style::ColorToken> => [Paint],
        pub border_bottom_color: Option<crate::style::ColorToken> => [Paint],
        pub border_left_color: Option<crate::style::ColorToken> => [Paint],
        pub stroke_color: Option<crate::style::ColorToken> => [Paint],
        pub stroke_width: Option<f32> => [Paint] opt_bits,
        pub stroke_dasharray: Option<crate::style::StrokeDash> => [Paint],
        pub stroke_dashoffset: Option<f32> => [Paint] opt_bits,
        pub border_style: Option<crate::style::BorderStyle> => [Paint],
        pub css_filter: CssFilter => [Apply],
        pub backdrop_blur_sigma: Option<f32> => [Apply] opt_bits,
        /// CSS `backdrop-filter` 的非 blur 算子（blur 走 `backdrop_blur_sigma`）。
        pub backdrop_filter: CssFilter => [Apply],
        /// CSS `mask-image`：本元素及其子树合成后按它裁剪 alpha。
        pub mask_image: Option<crate::style::MaskImage> => [Apply],
        pub object_fit: ObjectFit => [Paint],
        pub clip_contents: bool => [Paint],
        /// `clip_contents` 只作用于横向（`overflow-x:clip; overflow-y:visible`）。
        pub clip_x_only: bool => [Paint],
        // overflow_visible 经 display 层 Svg 项的 clip_overflow 体现（历史
        // 行为；3b 复核元素级是否补）。
        pub overflow_visible: bool => [],
        pub clip_path: Option<ClipPath> => [Paint],
        pub transforms: Vec<Transform> => [Apply],
        /// Static `transform-origin` anchor (fractions of the box). Defaults to centre.
        pub transform_origin: TransformOrigin => [Apply],
        /// CSS `perspective` distance; `None` = children stay orthographic.
        pub perspective: Option<f32> => [Apply] opt_bits,
        /// CSS `transform-style: preserve-3d` — children share this node's 3-D space.
        pub transform_style_preserve_3d: bool => [Apply],
        /// CSS `backface-visibility: hidden` — skip this node when it faces away.
        pub backface_visibility_hidden: bool => [Apply],
        pub box_shadow: Vec<BoxShadow> => [Paint],
        pub inset_shadow: Vec<InsetShadow> => [Paint],
        pub drop_shadow: Vec<DropShadow> => [Paint],
        // text_shadows 进 display 层 TextFp（历史行为）。
        pub text_shadows: Vec<TextShadow> => [],
        pub svg_path: Option<String> => [Paint],
    }
}
