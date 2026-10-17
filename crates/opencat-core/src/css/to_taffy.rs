//! lightningcss [`Property`] → [`taffy::Style`] 直通映射（布局腿）。
//!
//! 与 [`super::to_node_style`](paint 腿) 互补：布局属性在这里 1:1 落到
//! taffy 强类型，不经过 tailwind 时代的中间形状（`width_percent` /
//! `width_full` / `is_flex` / `gap: f32` …）。标准 CSS 的初始值语义
//! （block 显示、auto 尺寸、border-box）由 [`initial_style`] 给出。
//!
//! 求值层已把 var()/calc() 化简成字面值，这里只面对强类型；无法换算的
//! 单位（em 等需要排版上下文）按既有惯例退回 auto/初始值。

use lightningcss::properties::align::{
    AlignContent, AlignItems, AlignSelf, ContentDistribution, GapValue, JustifyContent,
    JustifyItems, JustifySelf, SelfPosition,
};
use lightningcss::properties::align::ContentPosition as ContentPositionKeyword;
use lightningcss::properties::display::{Display, DisplayInside, DisplayKeyword};
use lightningcss::properties::grid::{
    GridAutoFlow as CssGridAutoFlow, GridLine, RepeatCount, TrackBreadth, TrackListItem, TrackSize,
};
use lightningcss::properties::size::{MaxSize, Size};
use lightningcss::properties::Property;
use lightningcss::values::length::{LengthPercentage, LengthPercentageOrAuto, LengthValue};

use super::EvalContext;
use taffy::prelude::{
    AlignContent as TaffyAlignContent, AlignItems as TaffyAlignItems, Dimension, FromFr,
    JustifyContent as TaffyJustifyContent, LengthPercentage as TaffyLP,
    LengthPercentageAuto as TaffyLPA, Style, TaffyAuto, TaffyFitContent, TaffyGridLine,
    TaffyGridSpan, TaffyMaxContent, TaffyMinContent, TaffyZero,
};
use taffy::style::{
    AlignContentKeyword, AlignItemsKeyword, AlignmentSafety, GridTemplateComponent,
    GridTemplateRepetition, MaxTrackSizingFunction, MinTrackSizingFunction, RepetitionCount,
    TrackSizingFunction,
};

/// CSS 初始值（浏览器默认）：`display: block`、border-box、relative、auto 尺寸。
/// taffy 自己的 `Default` 在启用 flexbox feature 时 display 是 Flex，不能直接用。
pub(crate) fn initial_style() -> Style {
    Style {
        display: taffy::prelude::Display::Block,
        ..Style::default()
    }
}

/// 单条布局属性 → taffy。返回 `false` 表示不是布局属性（调用方继续走
/// paint 腿）。
pub(crate) fn apply_property(style: &mut Style, p: &Property, ctx: &EvalContext) -> bool {
    match p {
        Property::Display(d) => {
            style.display = map_display(d);
        }
        Property::Position(v) => {
            use lightningcss::properties::position::Position as CssPosition;
            style.position = match v {
                CssPosition::Absolute | CssPosition::Fixed => taffy::prelude::Position::Absolute,
                CssPosition::Static | CssPosition::Relative => taffy::prelude::Position::Relative,
                CssPosition::Sticky(_) => return true,
            };
        }
        Property::Top(v) => style.inset.top = lpa(v, ctx).unwrap_or_else(TaffyLPA::auto),
        Property::Right(v) => style.inset.right = lpa(v, ctx).unwrap_or_else(TaffyLPA::auto),
        Property::Bottom(v) => style.inset.bottom = lpa(v, ctx).unwrap_or_else(TaffyLPA::auto),
        Property::Left(v) => style.inset.left = lpa(v, ctx).unwrap_or_else(TaffyLPA::auto),
        // border 宽度参与布局（border-box 下挤压内容区）。CSS 路径下
        // `css_layout` 是布局的第一优先输入，漏接会让边框不占位。
        Property::BorderWidth(v) => {
            style.border = taffy::geometry::Rect {
                left: side_px(&v.left, ctx),
                right: side_px(&v.right, ctx),
                top: side_px(&v.top, ctx),
                bottom: side_px(&v.bottom, ctx),
            };
        }
        Property::BorderTopWidth(v) => style.border.top = side_px(v, ctx),
        Property::BorderRightWidth(v) => style.border.right = side_px(v, ctx),
        Property::BorderBottomWidth(v) => style.border.bottom = side_px(v, ctx),
        Property::BorderLeftWidth(v) => style.border.left = side_px(v, ctx),
        Property::Border(v) => {
            let w = side_px(&v.width, ctx);
            style.border = taffy::geometry::Rect {
                left: w,
                right: w,
                top: w,
                bottom: w,
            };
        }
        Property::Inset(v) => {
            // `inset: 0` 简写（tailwind inset-0 / inset-x-* 的产物）。
            style.inset.top = lpa(&v.top, ctx).unwrap_or_else(TaffyLPA::auto);
            style.inset.right = lpa(&v.right, ctx).unwrap_or_else(TaffyLPA::auto);
            style.inset.bottom = lpa(&v.bottom, ctx).unwrap_or_else(TaffyLPA::auto);
            style.inset.left = lpa(&v.left, ctx).unwrap_or_else(TaffyLPA::auto);
        }
        Property::Width(v) => {
            if let Size::LengthPercentage(inner) = v {
                style.size.width = dim_lp(inner, ctx);
            }
        }
        Property::Height(v) => {
            if let Size::LengthPercentage(inner) = v {
                style.size.height = dim_lp(inner, ctx);
            }
        }
        Property::MinWidth(v) => style.min_size.width = size_lpa(v, ctx),
        Property::MinHeight(v) => style.min_size.height = size_lpa(v, ctx),
        Property::MaxWidth(v) => style.max_size.width = max_size_lpa(v, ctx),
        Property::MaxHeight(v) => style.max_size.height = max_size_lpa(v, ctx),
        Property::Padding(v) => {
            style.padding = rect_lp(&v.top, &v.right, &v.bottom, &v.left, ctx);
        }
        Property::PaddingTop(v) => style.padding.top = lpa_to_lp(v, ctx),
        Property::PaddingRight(v) => style.padding.right = lpa_to_lp(v, ctx),
        Property::PaddingBottom(v) => style.padding.bottom = lpa_to_lp(v, ctx),
        Property::PaddingLeft(v) => style.padding.left = lpa_to_lp(v, ctx),
        // tailwind v4 产物用逻辑属性承载 px-/py- 类（padding-inline/block）。
        Property::PaddingBlock(v) => {
            style.padding.top = lpa_to_lp(&v.block_start, ctx);
            style.padding.bottom = lpa_to_lp(&v.block_end, ctx);
        }
        Property::PaddingInline(v) => {
            style.padding.left = lpa_to_lp(&v.inline_start, ctx);
            style.padding.right = lpa_to_lp(&v.inline_end, ctx);
        }
        Property::PaddingInlineStart(v) => style.padding.left = lpa_to_lp(v, ctx),
        Property::Margin(v) => {
            style.margin = rect_lpa(&v.top, &v.right, &v.bottom, &v.left, ctx);
        }
        Property::MarginTop(v) => style.margin.top = lpa(v, ctx).unwrap_or(TaffyLPA::ZERO),
        Property::MarginRight(v) => style.margin.right = lpa(v, ctx).unwrap_or(TaffyLPA::ZERO),
        Property::MarginBottom(v) => {
            style.margin.bottom = lpa(v, ctx).unwrap_or(TaffyLPA::ZERO)
        }
        Property::MarginLeft(v) => style.margin.left = lpa(v, ctx).unwrap_or(TaffyLPA::ZERO),
        Property::MarginInlineEnd(v) => {
            style.margin.right = lpa(v, ctx).unwrap_or(TaffyLPA::ZERO)
        }
        Property::FlexDirection(v, _) => {
            use lightningcss::properties::flex::FlexDirection as Css;
            style.flex_direction = match v {
                Css::Row => taffy::prelude::FlexDirection::Row,
                Css::RowReverse => taffy::prelude::FlexDirection::RowReverse,
                Css::Column => taffy::prelude::FlexDirection::Column,
                Css::ColumnReverse => taffy::prelude::FlexDirection::ColumnReverse,
            };
        }
        Property::FlexWrap(v, _) => {
            use lightningcss::properties::flex::FlexWrap as Css;
            style.flex_wrap = match v {
                Css::NoWrap => taffy::prelude::FlexWrap::NoWrap,
                Css::Wrap => taffy::prelude::FlexWrap::Wrap,
                Css::WrapReverse => taffy::prelude::FlexWrap::WrapReverse,
            };
        }
        Property::FlexBasis(v, _) => style.flex_basis = dim(v, ctx),
        Property::FlexGrow(v, _) => style.flex_grow = *v,
        Property::FlexShrink(v, _) => style.flex_shrink = *v,
        // `flex` 简写（tailwind `flex-1` / `flex-none` 的产物）。长写属性
        // 各自有分支，简写必须展开，否则整条声明被丢弃、flex 子项不伸展。
        Property::Flex(flex, _) => {
            style.flex_grow = flex.grow;
            style.flex_shrink = flex.shrink;
            style.flex_basis = dim(&flex.basis, ctx);
        }
        Property::JustifyContent(v, _) => {
            style.justify_content = map_justify_content(v);
        }
        Property::AlignContent(v, _) => {
            style.align_content = map_align_content(v);
        }
        Property::AlignItems(v, _) => {
            style.align_items = map_align_items(v);
        }
        Property::AlignSelf(v, _) => {
            style.align_self = map_align_self(v);
        }
        Property::JustifyItems(v) => {
            style.justify_items = map_justify_items(v);
        }
        Property::JustifySelf(v) => {
            style.justify_self = map_justify_self(v);
        }
        Property::Gap(gap) => {
            // taffy 的 gap.width 是水平（column）gap，gap.height 是垂直（row）gap。
            style.gap.width = gap_value(&gap.column, ctx);
            style.gap.height = gap_value(&gap.row, ctx);
        }
        Property::RowGap(v) => style.gap.height = gap_value(v, ctx),
        Property::ColumnGap(v) => style.gap.width = gap_value(v, ctx),
        Property::AspectRatio(v) => {
            style.aspect_ratio = v.ratio.as_ref().map(|r| r.0 / r.1);
        }
        Property::BoxSizing(v, _) => {
            use lightningcss::properties::size::BoxSizing as Css;
            style.box_sizing = match v {
                Css::BorderBox => taffy::style::BoxSizing::BorderBox,
                Css::ContentBox => taffy::style::BoxSizing::ContentBox,
            };
        }
        Property::Overflow(v) => {
            style.overflow.x = map_overflow(&v.x);
            style.overflow.y = map_overflow(&v.y);
        }
        Property::GridTemplateColumns(v) => match v {
            lightningcss::properties::grid::TrackSizing::TrackList(list) => {
                style.grid_template_columns = map_tracks(&list.items, ctx);
            }
            lightningcss::properties::grid::TrackSizing::None => {
                style.grid_template_columns = Vec::new();
            }
        },
        Property::GridTemplateRows(v) => match v {
            lightningcss::properties::grid::TrackSizing::TrackList(list) => {
                style.grid_template_rows = map_tracks(&list.items, ctx);
            }
            lightningcss::properties::grid::TrackSizing::None => {
                style.grid_template_rows = Vec::new();
            }
        },
        Property::GridAutoFlow(v) => {
            let dense = v.contains(CssGridAutoFlow::Dense);
            style.grid_auto_flow = if v.contains(CssGridAutoFlow::Column) {
                if dense {
                    taffy::style::GridAutoFlow::ColumnDense
                } else {
                    taffy::style::GridAutoFlow::Column
                }
            } else if dense {
                taffy::style::GridAutoFlow::RowDense
            } else {
                taffy::style::GridAutoFlow::Row
            };
        }
        Property::GridAutoRows(v) => {
            style.grid_auto_rows = v.0.iter().map(|t| map_track(t, ctx)).collect();
        }
        Property::GridAutoColumns(v) => {
            style.grid_auto_columns = v.0.iter().map(|t| map_track(t, ctx)).collect();
        }
        Property::GridColumnStart(v) => style.grid_column.start = map_grid_line(v),
        Property::GridColumnEnd(v) => style.grid_column.end = map_grid_line(v),
        Property::GridRowStart(v) => style.grid_row.start = map_grid_line(v),
        Property::GridRowEnd(v) => style.grid_row.end = map_grid_line(v),
        _ => return false,
    }
    true
}

fn map_display(d: &Display) -> taffy::prelude::Display {
    match d {
        Display::Keyword(DisplayKeyword::None) => taffy::prelude::Display::None,
        Display::Pair(pair) => match pair.inside {
            DisplayInside::Flex(_) => taffy::prelude::Display::Flex,
            DisplayInside::Grid => taffy::prelude::Display::Grid,
            _ => taffy::prelude::Display::Block,
        },
        _ => taffy::prelude::Display::Block,
    }
}

fn map_overflow(k: &lightningcss::properties::overflow::OverflowKeyword) -> taffy::style::Overflow {
    use lightningcss::properties::overflow::OverflowKeyword;
    match k {
        OverflowKeyword::Hidden | OverflowKeyword::Clip => taffy::style::Overflow::Hidden,
        OverflowKeyword::Scroll => taffy::style::Overflow::Scroll,
        _ => taffy::style::Overflow::Visible,
    }
}

fn map_justify_content(v: &JustifyContent) -> Option<TaffyJustifyContent> {
    match v {
        JustifyContent::Normal => None,
        JustifyContent::ContentDistribution(d) => Some(match d {
            ContentDistribution::SpaceBetween => TaffyJustifyContent::SPACE_BETWEEN,
            ContentDistribution::SpaceAround => TaffyJustifyContent::SPACE_AROUND,
            ContentDistribution::SpaceEvenly => TaffyJustifyContent::SPACE_EVENLY,
            ContentDistribution::Stretch => TaffyJustifyContent::STRETCH,
        }),
        JustifyContent::ContentPosition { value, .. } => Some(TaffyJustifyContent {
            keyword: content_keyword(value),
            safety: AlignmentSafety::Unsafe,
        }),
        // `left`/`right` 依赖物理方向，taffy 无对应。
        _ => None,
    }
}

fn map_align_content(v: &AlignContent) -> Option<TaffyAlignContent> {
    match v {
        AlignContent::Normal => None,
        AlignContent::ContentDistribution(d) => Some(match d {
            ContentDistribution::SpaceBetween => TaffyAlignContent::SPACE_BETWEEN,
            ContentDistribution::SpaceAround => TaffyAlignContent::SPACE_AROUND,
            ContentDistribution::SpaceEvenly => TaffyAlignContent::SPACE_EVENLY,
            ContentDistribution::Stretch => TaffyAlignContent::STRETCH,
        }),
        AlignContent::ContentPosition { value, .. } => Some(TaffyAlignContent {
            keyword: content_keyword(value),
            safety: AlignmentSafety::Unsafe,
        }),
        _ => None,
    }
}

/// `align-items: normal` 在 flex 容器表现为 stretch（CSS Box Alignment §5）。
fn map_align_items(v: &AlignItems) -> Option<TaffyAlignItems> {
    let keyword = match v {
        AlignItems::Normal | AlignItems::Stretch => AlignItemsKeyword::Stretch,
        AlignItems::BaselinePosition(_) => AlignItemsKeyword::Baseline,
        AlignItems::SelfPosition { value, .. } => return self_keyword(value).map(|k| TaffyAlignItems { keyword: k, safety: AlignmentSafety::Unsafe }),
    };
    Some(TaffyAlignItems { keyword, safety: AlignmentSafety::Unsafe })
}

fn map_align_self(v: &AlignSelf) -> Option<TaffyAlignItems> {
    match v {
        AlignSelf::Auto => None,
        AlignSelf::Normal | AlignSelf::Stretch => Some(TaffyAlignItems::STRETCH),
        AlignSelf::BaselinePosition(_) => Some(TaffyAlignItems::BASELINE),
        AlignSelf::SelfPosition { value, .. } => self_keyword(value).map(|k| TaffyAlignItems { keyword: k, safety: AlignmentSafety::Unsafe }),
    }
}

fn map_justify_items(v: &JustifyItems) -> Option<TaffyAlignItems> {
    match v {
        JustifyItems::Normal | JustifyItems::Stretch => Some(TaffyAlignItems::STRETCH),
        JustifyItems::BaselinePosition(_) => Some(TaffyAlignItems::BASELINE),
        JustifyItems::SelfPosition { value, .. } => self_keyword(value).map(|k| TaffyAlignItems { keyword: k, safety: AlignmentSafety::Unsafe }),
        _ => None,
    }
}

fn map_justify_self(v: &JustifySelf) -> Option<TaffyAlignItems> {
    match v {
        JustifySelf::Auto => None,
        JustifySelf::Normal | JustifySelf::Stretch => Some(TaffyAlignItems::STRETCH),
        JustifySelf::BaselinePosition(_) => Some(TaffyAlignItems::BASELINE),
        JustifySelf::SelfPosition { value, .. } => self_keyword(value).map(|k| TaffyAlignItems { keyword: k, safety: AlignmentSafety::Unsafe }),
        _ => None,
    }
}

fn content_keyword(v: &ContentPositionKeyword) -> AlignContentKeyword {
    match v {
        ContentPositionKeyword::Start => AlignContentKeyword::Start,
        ContentPositionKeyword::Center => AlignContentKeyword::Center,
        ContentPositionKeyword::End => AlignContentKeyword::End,
        ContentPositionKeyword::FlexStart => AlignContentKeyword::FlexStart,
        ContentPositionKeyword::FlexEnd => AlignContentKeyword::FlexEnd,
    }
}

fn self_keyword(v: &SelfPosition) -> Option<AlignItemsKeyword> {
    Some(match v {
        SelfPosition::Start => AlignItemsKeyword::Start,
        SelfPosition::Center => AlignItemsKeyword::Center,
        SelfPosition::End => AlignItemsKeyword::End,
        SelfPosition::SelfStart => AlignItemsKeyword::SelfStart,
        SelfPosition::SelfEnd => AlignItemsKeyword::SelfEnd,
        SelfPosition::FlexStart => AlignItemsKeyword::FlexStart,
        SelfPosition::FlexEnd => AlignItemsKeyword::FlexEnd,
    })
}

fn map_tracks(items: &[TrackListItem], ctx: &EvalContext) -> Vec<GridTemplateComponent<String>> {
    items
        .iter()
        .filter_map(|item| match item {
            TrackListItem::TrackSize(t) => Some(GridTemplateComponent::Single(map_track(t, ctx))),
            TrackListItem::TrackRepeat(r) => {
                let tracks: Vec<TrackSizingFunction> =
                    r.track_sizes.iter().map(|t| map_track(t, ctx)).collect();
                let count = match r.count {
                    RepeatCount::Number(n) => RepetitionCount::Count(u16::try_from(n).ok()?),
                    RepeatCount::AutoFill => RepetitionCount::AutoFill,
                    RepeatCount::AutoFit => RepetitionCount::AutoFit,
                };
                // 命名线按 taffy 约定留空：空集 = 每条线全部未命名。
                Some(GridTemplateComponent::Repeat(GridTemplateRepetition {
                    count,
                    tracks,
                    line_names: Vec::new(),
                }))
            }
        })
        .collect()
}

fn map_track(t: &TrackSize, ctx: &EvalContext) -> TrackSizingFunction {
    match t {
        TrackSize::TrackBreadth(b) => TrackSizingFunction {
            min: track_min(b, ctx),
            max: track_max(b, ctx),
        },
        TrackSize::MinMax { min, max } => TrackSizingFunction {
            min: track_min(min, ctx),
            max: track_max(max, ctx),
        },
        TrackSize::FitContent(inner) => TrackSizingFunction {
            min: MinTrackSizingFunction::AUTO,
            max: match lp(inner, ctx) {
                Some(v) => MaxTrackSizingFunction::fit_content(v),
                None => MaxTrackSizingFunction::AUTO,
            },
        },
    }
}

fn track_min(b: &TrackBreadth, ctx: &EvalContext) -> MinTrackSizingFunction {
    match b {
        TrackBreadth::Length(inner) => match lp(inner, ctx) {
            Some(v) => MinTrackSizingFunction::from(v),
            None => MinTrackSizingFunction::AUTO,
        },
        TrackBreadth::MinContent => MinTrackSizingFunction::MIN_CONTENT,
        TrackBreadth::MaxContent => MinTrackSizingFunction::MAX_CONTENT,
        TrackBreadth::Auto | TrackBreadth::Flex(_) => MinTrackSizingFunction::AUTO,
    }
}

fn track_max(b: &TrackBreadth, ctx: &EvalContext) -> MaxTrackSizingFunction {
    match b {
        TrackBreadth::Flex(f) => MaxTrackSizingFunction::from_fr(*f as f32),
        TrackBreadth::Length(inner) => match lp(inner, ctx) {
            Some(v) => MaxTrackSizingFunction::from(v),
            None => MaxTrackSizingFunction::AUTO,
        },
        TrackBreadth::MinContent => MaxTrackSizingFunction::MIN_CONTENT,
        TrackBreadth::MaxContent => MaxTrackSizingFunction::MAX_CONTENT,
        TrackBreadth::Auto => MaxTrackSizingFunction::AUTO,
    }
}

fn map_grid_line(line: &GridLine) -> taffy::style::GridPlacement {
    match line {
        GridLine::Auto => taffy::style::GridPlacement::Auto,
        GridLine::Line { index, .. } => {
            taffy::style::GridPlacement::from_line_index(*index as i16)
        }
        GridLine::Span { index, .. } => {
            taffy::style::GridPlacement::from_span(u16::try_from((*index).max(0)).unwrap_or(1))
        }
        GridLine::Area { .. } => taffy::style::GridPlacement::Auto,
    }
}

// ---- 数值换算 ----

fn length_px(v: &LengthValue, ctx: &EvalContext) -> Option<f32> {
    match v {
        LengthValue::Px(px) => Some(*px),
        LengthValue::Rem(r) => Some(r * ctx.rem_base),
        // vw/vh：视频画布即视口。
        LengthValue::Vw(v) => Some(v * ctx.viewport.0 / 100.0),
        LengthValue::Vh(v) => Some(v * ctx.viewport.1 / 100.0),
        LengthValue::Vmin(v) => Some(v * ctx.viewport.0.min(ctx.viewport.1) / 100.0),
        LengthValue::Vmax(v) => Some(v * ctx.viewport.0.max(ctx.viewport.1) / 100.0),
        _ => None,
    }
}

fn lp(v: &LengthPercentage, ctx: &EvalContext) -> Option<TaffyLP> {
    match v {
        LengthPercentage::Dimension(unit) => length_px(unit, ctx).map(TaffyLP::length),
        LengthPercentage::Percentage(p) => Some(TaffyLP::percent(p.0)),
        LengthPercentage::Calc(_) => None,
    }
}

fn lpa(v: &LengthPercentageOrAuto, ctx: &EvalContext) -> Option<TaffyLPA> {
    match v {
        LengthPercentageOrAuto::Auto => Some(TaffyLPA::auto()),
        LengthPercentageOrAuto::LengthPercentage(inner) => {
            lp(inner, ctx).map(TaffyLPA::from)
        }
    }
}


/// 边框宽度 → taffy 长度；非长度值（thin/medium/thick）按 CSS 计算值取中。
fn side_px(v: &lightningcss::properties::border::BorderSideWidth, ctx: &EvalContext) -> taffy::style::LengthPercentage {
    use lightningcss::properties::border::BorderSideWidth;
    match v {
        BorderSideWidth::Length(l) => taffy::style::LengthPercentage::length(match l {
            lightningcss::values::length::Length::Value(v) => length_px(v, ctx).unwrap_or(0.0),
            _ => 0.0,
        }),
        BorderSideWidth::Thin => taffy::style::LengthPercentage::length(1.0),
        BorderSideWidth::Medium => taffy::style::LengthPercentage::length(3.0),
        BorderSideWidth::Thick => taffy::style::LengthPercentage::length(5.0),
    }
}

fn dim(v: &LengthPercentageOrAuto, ctx: &EvalContext) -> Dimension {
    match v {
        LengthPercentageOrAuto::Auto => Dimension::auto(),
        LengthPercentageOrAuto::LengthPercentage(inner) => dim_lp(inner, ctx),
    }
}

fn dim_lp(v: &LengthPercentage, ctx: &EvalContext) -> Dimension {
    match v {
        LengthPercentage::Dimension(unit) => {
            length_px(unit, ctx).map(Dimension::length).unwrap_or_else(Dimension::auto)
        }
        LengthPercentage::Percentage(p) => Dimension::percent(p.0),
        LengthPercentage::Calc(_) => Dimension::auto(),
    }
}

/// min-width/min-height：taffy 的类型是 LengthPercentageAuto，min/max-content
/// 等关键字表达不了，退回 auto（taffy 对 auto min 本身就有 content 语义）。
fn size_lpa(v: &Size, ctx: &EvalContext) -> TaffyLPA {
    match v {
        Size::LengthPercentage(inner) => {
            lp(inner, ctx).map(TaffyLPA::from).unwrap_or_else(TaffyLPA::auto)
        }
        _ => TaffyLPA::auto(),
    }
}

/// max-width/max-height：`none` → auto。
fn max_size_lpa(v: &MaxSize, ctx: &EvalContext) -> TaffyLPA {
    match v {
        MaxSize::LengthPercentage(inner) => {
            lp(inner, ctx).map(TaffyLPA::from).unwrap_or_else(TaffyLPA::auto)
        }
        _ => TaffyLPA::auto(),
    }
}

fn gap_value(v: &GapValue, ctx: &EvalContext) -> TaffyLP {
    match v {
        GapValue::LengthPercentage(inner) => lp(inner, ctx).unwrap_or(TaffyLP::ZERO),
        GapValue::Normal => TaffyLP::ZERO,
    }
}

/// padding 落到 taffy 时 auto 无意义，按浏览器行为记 0。
fn lpa_to_lp(v: &LengthPercentageOrAuto, ctx: &EvalContext) -> TaffyLP {
    match v {
        LengthPercentageOrAuto::LengthPercentage(inner) => {
            lp(inner, ctx).unwrap_or(TaffyLP::ZERO)
        }
        _ => TaffyLP::ZERO,
    }
}

fn rect_lp(
    top: &LengthPercentageOrAuto,
    right: &LengthPercentageOrAuto,
    bottom: &LengthPercentageOrAuto,
    left: &LengthPercentageOrAuto,
    ctx: &EvalContext,
) -> taffy::geometry::Rect<TaffyLP> {
    taffy::geometry::Rect {
        top: lpa_to_lp(top, ctx),
        right: lpa_to_lp(right, ctx),
        bottom: lpa_to_lp(bottom, ctx),
        left: lpa_to_lp(left, ctx),
    }
}

fn rect_lpa(
    top: &LengthPercentageOrAuto,
    right: &LengthPercentageOrAuto,
    bottom: &LengthPercentageOrAuto,
    left: &LengthPercentageOrAuto,
    ctx: &EvalContext,
) -> taffy::geometry::Rect<TaffyLPA> {
    taffy::geometry::Rect {
        top: lpa(top, ctx).unwrap_or(TaffyLPA::ZERO),
        right: lpa(right, ctx).unwrap_or(TaffyLPA::ZERO),
        bottom: lpa(bottom, ctx).unwrap_or(TaffyLPA::ZERO),
        left: lpa(left, ctx).unwrap_or(TaffyLPA::ZERO),
    }
}
