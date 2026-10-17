//! 强类型 [`Property`] → [`NodeStyle`] 映射。
//!
//! NodeStyle 是引擎既有消费端（taffy 布局、Skia 绘制、每帧 mutations）
//! 的稳定契约，这里只做「CSS 值 → 既有字段」的搬运；字段形态里 tailwind
//! 特化的部分（width/width_percent 二分、text_px 等）按现状适配，待
//! NodeStyle 归一为标准 CSS 值模型后本模块同步简化。
//!
//! sigma 约定与 parse/tailwind.rs 保持一致：box/text-shadow 的 CSS blur
//! radius 折半，backdrop-filter 的 blur 值直用。

use lightningcss::properties::align::{
    AlignContent, AlignItems as CssAlignItems, AlignSelf as CssAlignSelf, ContentDistribution,
    ContentPosition, GapValue, JustifyContent as CssJustifyContent, SelfPosition,
};
use lightningcss::properties::background::{BackgroundClip, BackgroundSize};
use lightningcss::properties::border::{Border, BorderSideWidth, LineStyle};
use lightningcss::properties::border_radius::BorderRadius as CssBorderRadius;
use lightningcss::properties::box_shadow::BoxShadow as CssBoxShadow;
use lightningcss::properties::display::{Display, DisplayInside};
use lightningcss::properties::effects::{BlendMode as CssBlendMode, Filter, FilterList};
use lightningcss::properties::flex::{FlexDirection as CssFlexDirection, FlexWrap as CssFlexWrap};
use lightningcss::properties::font::{
    AbsoluteFontWeight, FontFamily as CssFontFamily, FontSize, FontStyle as CssFontStyle,
    FontWeight as CssFontWeight, GenericFontFamily, LineHeight as CssLineHeight,
};
use lightningcss::properties::custom::{CustomProperty, CustomPropertyName};
use lightningcss::properties::overflow::{OverflowKeyword, TextOverflow};
use lightningcss::properties::position::Position as CssPosition;
use lightningcss::properties::size::{MaxSize, Size};
use lightningcss::properties::svg::SVGPaint;
use lightningcss::properties::text::{
    Spacing, TextAlign as CssTextAlign, TextTransform as CssTextTransform, TextTransformCase,
    WhiteSpace,
};
use lightningcss::properties::transform::{Transform as CssTransform, TransformList};
use lightningcss::properties::{Property, PropertyId};
use lightningcss::stylesheet::PrinterOptions;
use lightningcss::traits::ToCss;
use lightningcss::values::color::CssColor;
use lightningcss::values::gradient::{Gradient, GradientItem, LineDirection};
use lightningcss::values::image::Image;
use lightningcss::values::length::{Length, LengthPercentage, LengthPercentageOrAuto, LengthValue};
use lightningcss::values::percentage::NumberOrPercentage;
use lightningcss::values::position::{
    HorizontalPositionKeyword, Position as CssPositionValue, PositionComponent,
    VerticalPositionKeyword,
};

use crate::canvas::paint::BlendMode;
use crate::style::{
    BackgroundFill, BorderRadius, BoxShadow as CatBoxShadow, ColorToken, CssFilter, CssFilterKind,
    CssFilterOp, FlexDirection, FlexWrap, GenericFamily, GradientDirection, GradientStop,
    InsetShadow, JustifyContent, LengthPercentageAuto, StopUnit, TextAlign,
    TextShadow as CatTextShadow, TextTransform, Transform as CatTransform, TransformOrigin,
};

use super::eval::{css_color_to_f32, serialize_tokens};
use super::EvalContext;

fn warn_unsupported(pid: PropertyId, why: &str) {
    let name = pid
        .to_css_string(PrinterOptions::default())
        .unwrap_or_else(|_| "<prop>".into());
    log::warn!("css: {name} 暂不支持（{why}），已跳过");
}

fn color_token(color: &CssColor) -> Option<ColorToken> {
    let [r, g, b, a] = css_color_to_f32(color)?;
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(ColorToken::Custom(ch(r), ch(g), ch(b), ch(a)))
}

enum LenPercent {
    Px(f32),
    Pct(f32),
}

/// DimensionPercentage → px/percent。Calc 已在 eval 层化简；残余的
/// px/% 混合形态无载体。em/rem 等 typed 单位在此换算。
fn lp_to_len_percent(lp: &LengthPercentage, ctx: &EvalContext) -> Option<LenPercent> {
    Some(match lp {
        LengthPercentage::Dimension(unit) => match unit {
            LengthValue::Px(v) => LenPercent::Px(*v),
            LengthValue::Rem(v) | LengthValue::Em(v) => LenPercent::Px(v * ctx.rem_base),
            LengthValue::Vw(v) => LenPercent::Px(v * ctx.viewport.0 / 100.0),
            LengthValue::Vh(v) => LenPercent::Px(v * ctx.viewport.1 / 100.0),
            _ => return None,
        },
        LengthPercentage::Percentage(p) => LenPercent::Pct(p.0),
        LengthPercentage::Calc(_) => return None,
    })
}

fn lp_to_px(lp: &LengthPercentage, ctx: &EvalContext) -> Option<f32> {
    match lp_to_len_percent(lp, ctx)? {
        LenPercent::Px(px) => Some(px),
        LenPercent::Pct(_) => None,
    }
}

/// [`Length`]（可含 calc()）→ px。eval 层已化简常规 calc；残余形态按 0 处理。
fn len_to_px(len: &Length, ctx: &EvalContext) -> f32 {
    match len {
        Length::Value(v) => length_value_to_px(v, ctx),
        Length::Calc(_) => 0.0,
    }
}

fn lpoa_to_cat(v: &LengthPercentageOrAuto, ctx: &EvalContext) -> Option<LengthPercentageAuto> {
    Some(match v {
        LengthPercentageOrAuto::Auto => LengthPercentageAuto::Auto,
        LengthPercentageOrAuto::LengthPercentage(lp) => match lp_to_len_percent(lp, ctx)? {
            LenPercent::Px(px) => LengthPercentageAuto::Length(px),
            LenPercent::Pct(p) => LengthPercentageAuto::Percent(p),
        },
    })
}

fn number_or_percentage(n: &NumberOrPercentage) -> f32 {
    match n {
        NumberOrPercentage::Number(v) => *v,
        NumberOrPercentage::Percentage(p) => p.0,
    }
}

/// 主入口：把一条类型化声明写进 NodeStyle。刻意不做 `use Property::*`——
/// 变体名与 style.rs 的类型名大量同名（FlexDirection/Display/…），
/// 全限定避免遮蔽。
pub fn apply_property(
    style: &mut crate::style::NodeStyle,
    prop: &Property<'static>,
    ctx: &EvalContext,
) {
    match prop {
        // ---- 布局 ----
        Property::Display(d) => match d {
            Display::Pair(pair) => {
                style.is_flex = matches!(pair.inside, DisplayInside::Flex(_));
                style.is_grid = matches!(pair.inside, DisplayInside::Grid);
            }
            _ => {
                style.is_flex = false;
                style.is_grid = false;
            }
        },
        Property::FlexDirection(v, _) => {
            style.flex_direction = Some(match v {
                CssFlexDirection::Row => FlexDirection::Row,
                CssFlexDirection::Column => FlexDirection::Col,
                CssFlexDirection::RowReverse => FlexDirection::RowReverse,
                CssFlexDirection::ColumnReverse => FlexDirection::ColReverse,
            });
        }
        Property::FlexWrap(v, _) => {
            style.flex_wrap = Some(match v {
                CssFlexWrap::NoWrap => FlexWrap::NoWrap,
                CssFlexWrap::Wrap => FlexWrap::Wrap,
                CssFlexWrap::WrapReverse => FlexWrap::WrapReverse,
            });
        }
        Property::FlexGrow(v, _) => style.flex_grow = Some(*v),
        Property::FlexShrink(v, _) => style.flex_shrink = Some(*v),
        Property::FlexBasis(v, _) => style.flex_basis = lpoa_to_cat(v, ctx),
        Property::JustifyContent(v, _) => style.justify_content = justify_to_cat(v),
        Property::AlignContent(v, _) => style.align_content = align_content_to_cat(v),
        Property::AlignItems(v, _) => style.align_items = align_items_to_cat(v),
        Property::AlignSelf(v, _) => style.align_self = align_self_to_cat(v),
        Property::Gap(g) => {
            if let Some(px) = gap_to_px(&g.row, ctx) {
                style.gap = Some(px);
                style.gap_y = Some(px);
            }
            if let Some(px) = gap_to_px(&g.column, ctx) {
                style.gap_x = Some(px);
            }
        }
        Property::RowGap(v) => style.gap_y = gap_to_px(v, ctx),
        Property::ColumnGap(v) => style.gap_x = gap_to_px(v, ctx),
        Property::Width(v) => {
            if let Size::LengthPercentage(lp) = v {
                match lp_to_len_percent(lp, ctx) {
                    Some(LenPercent::Px(px)) => style.width = Some(px),
                    Some(LenPercent::Pct(p)) => style.width_percent = Some(p * 100.0),
                    None => warn_unsupported(PropertyId::Width, "calc"),
                }
            }
        }
        Property::Height(v) => {
            if let Size::LengthPercentage(lp) = v {
                match lp_to_len_percent(lp, ctx) {
                    Some(LenPercent::Px(px)) => style.height = Some(px),
                    Some(LenPercent::Pct(p)) => style.height_percent = Some(p * 100.0),
                    None => warn_unsupported(PropertyId::Height, "calc"),
                }
            }
        }
        Property::MinWidth(v) => style.min_width = size_to_cat(v, ctx),
        Property::MinHeight(v) => style.min_height = size_to_cat(v, ctx),
        Property::MaxWidth(v) => {
            if let MaxSize::LengthPercentage(lp) = v {
                style.max_width = lp_to_px(lp, ctx);
            }
        }
        Property::Padding(v) => {
            padding_longhand(&mut style.padding, &v.top, ctx);
            padding_longhand(&mut style.padding_top, &v.top, ctx);
            padding_longhand(&mut style.padding_right, &v.right, ctx);
            padding_longhand(&mut style.padding_bottom, &v.bottom, ctx);
            padding_longhand(&mut style.padding_left, &v.left, ctx);
        }
        Property::PaddingTop(v) => padding_longhand(&mut style.padding_top, v, ctx),
        Property::PaddingRight(v) => padding_longhand(&mut style.padding_right, v, ctx),
        Property::PaddingBottom(v) => padding_longhand(&mut style.padding_bottom, v, ctx),
        Property::PaddingLeft(v) => padding_longhand(&mut style.padding_left, v, ctx),
        // tailwind v4 产物用逻辑属性承载 px-/py- 类（padding-inline/block）。
        Property::PaddingBlock(v) => {
            padding_longhand(&mut style.padding_top, &v.block_start, ctx);
            padding_longhand(&mut style.padding_bottom, &v.block_end, ctx);
        }
        Property::PaddingInline(v) => {
            padding_longhand(&mut style.padding_left, &v.inline_start, ctx);
            padding_longhand(&mut style.padding_right, &v.inline_end, ctx);
        }
        Property::PaddingInlineStart(v) => padding_longhand(&mut style.padding_left, v, ctx),
        Property::Margin(v) => {
            style.margin_top = lpoa_to_cat(&v.top, ctx);
            style.margin_right = lpoa_to_cat(&v.right, ctx);
            style.margin_bottom = lpoa_to_cat(&v.bottom, ctx);
            style.margin_left = lpoa_to_cat(&v.left, ctx);
        }
        Property::MarginTop(v) => style.margin_top = lpoa_to_cat(v, ctx),
        Property::MarginRight(v) => style.margin_right = lpoa_to_cat(v, ctx),
        Property::MarginBottom(v) => style.margin_bottom = lpoa_to_cat(v, ctx),
        Property::MarginLeft(v) => style.margin_left = lpoa_to_cat(v, ctx),
        Property::MarginInlineEnd(v) => style.margin_right = lpoa_to_cat(v, ctx),
        Property::Position(v) => {
            style.position = Some(match v {
                CssPosition::Absolute | CssPosition::Fixed => crate::style::Position::Absolute,
                CssPosition::Static | CssPosition::Relative => crate::style::Position::Relative,
                CssPosition::Sticky(_) => return,
            });
        }
        Property::Top(v) => style.inset_top = lpoa_to_cat(v, ctx),
        Property::Right(v) => style.inset_right = lpoa_to_cat(v, ctx),
        Property::Bottom(v) => style.inset_bottom = lpoa_to_cat(v, ctx),
        Property::Left(v) => style.inset_left = lpoa_to_cat(v, ctx),
        Property::Inset(v) => {
            // `inset: 0` 这类简写（tailwind 的 inset-0/ inset-x-* 都会产生）。
            style.inset_top = lpoa_to_cat(&v.top, ctx);
            style.inset_right = lpoa_to_cat(&v.right, ctx);
            style.inset_bottom = lpoa_to_cat(&v.bottom, ctx);
            style.inset_left = lpoa_to_cat(&v.left, ctx);
        }
        Property::ZIndex(v) => {
            if let lightningcss::properties::position::ZIndex::Integer(i) = v {
                style.z_index = Some(*i);
            }
        }
        Property::Order(v, _) => style.order = Some(*v),
        Property::AspectRatio(v) => {
            if let Some(r) = &v.ratio {
                if r.1 != 0.0 {
                    style.aspect_ratio = Some(r.0 / r.1);
                }
            }
        }
        Property::Overflow(v) => {
            style.overflow_hidden = matches!(
                v.x,
                OverflowKeyword::Hidden | OverflowKeyword::Clip
            );
        }
        Property::BackgroundColor(c) => style.bg_color = color_token(c),
        Property::BackgroundImage(images) => {
            for image in images {
                if let Some(fill) = image_to_fill(image, ctx) {
                    style.background_layers.push(fill);
                }
            }
        }
        Property::BackgroundClip(v, _) => {
            if v.iter().any(|c| matches!(c, BackgroundClip::Text)) {
                style.background_clip = crate::style::BackgroundClip::Text;
            }
        }
        Property::BackgroundSize(sizes) => {
            if let Some(BackgroundSize::Explicit { width, height }) = sizes.first() {
                let side = |v: &LengthPercentageOrAuto| match v {
                    LengthPercentageOrAuto::LengthPercentage(lp) => lp_to_px(lp, ctx),
                    LengthPercentageOrAuto::Auto => None,
                };
                if let (Some(w), Some(h)) = (side(width), side(height)) {
                    style.bg_size = Some([w, h]);
                }
            }
        }
        Property::BorderColor(v) => style.border_color = color_token(&v.top),
        Property::BorderWidth(v) => border_side(&mut style.border_width, &v.top, ctx),
        Property::BorderTopWidth(v) => border_side(&mut style.border_top_width, v, ctx),
        Property::BorderRightWidth(v) => border_side(&mut style.border_right_width, v, ctx),
        Property::BorderBottomWidth(v) => border_side(&mut style.border_bottom_width, v, ctx),
        Property::BorderLeftWidth(v) => border_side(&mut style.border_left_width, v, ctx),
        Property::BorderStyle(v) => style.border_style = line_style_to_cat(&v.top),
        Property::Border(v) => apply_border(style, v, ctx),
        Property::BorderRadius(v, _) => apply_border_radius(style, v, ctx),
        Property::BoxShadow(shadows, _) => {
            for shadow in shadows {
                apply_box_shadow(style, shadow, ctx);
            }
        }
        Property::Opacity(v) => style.opacity = Some(v.0),
        Property::MixBlendMode(v) => style.blend_mode = blend_to_cat(*v),
        Property::Filter(list, _) => style.css_filter = filter_list_to_css_filter(list, ctx),
        Property::BackdropFilter(list, _) => {
            if let FilterList::Filters(filters) = list {
                for filter in filters {
                    if let Filter::Blur(radius) = filter {
                        // 与 tailwind.rs 的 BackdropBlurSigma 约定一致：px 直用。
                        style.backdrop_blur_sigma = Some(len_to_px(radius, ctx));
                    }
                }
            }
        }
        Property::Transform(list, _) => apply_transform(style, list, ctx),
        Property::TransformOrigin(pos, _) => style.transform_origin = position_to_origin(pos, ctx),

        // ---- 文本 ----
        Property::Color(c) => style.text_color = color_token(c),
        Property::FontSize(size) => {
            if let FontSize::Length(lp) = size {
                match lp_to_len_percent(lp, ctx) {
                    Some(LenPercent::Px(px)) => style.text_px = Some(px),
                    // 字号百分比相对父字号；静态期用 rem 基准近似。
                    Some(LenPercent::Pct(p)) => style.text_px = Some(ctx.rem_base * p),
                    None => warn_unsupported(PropertyId::FontSize, "calc"),
                }
            }
        }
        Property::FontWeight(w) => {
            style.font_weight = Some(match w {
                CssFontWeight::Absolute(n) => crate::style::FontWeight(match n {
                    AbsoluteFontWeight::Weight(v) => *v as u16,
                    AbsoluteFontWeight::Normal => 400,
                    AbsoluteFontWeight::Bold => 700,
                }),
                CssFontWeight::Bolder | CssFontWeight::Lighter => {
                    warn_unsupported(PropertyId::FontWeight, "相对字重");
                    return;
                }
            });
        }
        Property::FontStyle(v) => {
            style.font_style = Some(match v {
                CssFontStyle::Italic | CssFontStyle::Oblique(_) => crate::style::FontStyle::Italic,
                CssFontStyle::Normal => crate::style::FontStyle::Normal,
            });
        }
        Property::FontFamily(list) => {
            for family in list {
                match family {
                    CssFontFamily::FamilyName(name) => {
                        if style.font_family.is_none() {
                            // 字段私有，走 ToCss 序列化还原名字面。
                            if let Ok(s) = name.to_css_string(PrinterOptions::default()) {
                                let cleaned = s.trim_matches(|c| c == '"' || c == '\'');
                                // 名字与 manifest face id 同形时（font-[id] 的标准
                                // 形态），同时落 face 通道；apply_font_refs_to_styles
                                // 会把可解析的 id 换成真实 family，解析不了的原样
                                // 保留为字面 family。
                                style.font_face_id = Some(cleaned.to_string());
                                style.font_family = Some(cleaned.to_string());
                            }
                        }
                    }
                    CssFontFamily::Generic(generic) => {
                        if let Some(g) = generic_to_cat(generic) {
                            style.font_generic = Some(g);
                        }
                    }
                }
                if style.font_family.is_some() && style.font_generic.is_some() {
                    break;
                }
            }
        }
        Property::LineHeight(v) => match v {
            CssLineHeight::Number(n) => style.line_height = Some(*n),
            CssLineHeight::Length(lp) => style.line_height_px = lp_to_px(lp, ctx),
            CssLineHeight::Normal => {}
        },
        Property::LetterSpacing(v) => {
            if let Spacing::Length(len) = v {
                // em 相对本元素字号（tracking-[-0.02em] 在 108px 字号下是
                // -2.16px）；走 letter_spacing_em 由 resolve_text_style 按
                // text_px 折算，rem/其余单位按根基准换 px。
                match len {
                    Length::Value(LengthValue::Em(n)) => {
                        style.letter_spacing_em = Some(*n);
                    }
                    other => {
                        style.letter_spacing = Some(len_to_px(other, ctx));
                    }
                }
            }
        }
        Property::TextAlign(v) => {
            style.text_align = Some(match v {
                CssTextAlign::Left | CssTextAlign::Start => TextAlign::Left,
                CssTextAlign::Right | CssTextAlign::End => TextAlign::Right,
                CssTextAlign::Center => TextAlign::Center,
                CssTextAlign::Justify
                | CssTextAlign::MatchParent
                | CssTextAlign::JustifyAll => return,
            });
        }
        Property::TextDecorationLine(_, _) => {
            log::warn!("css: text-decoration-line 暂不支持（underline 无渲染载体），已跳过");
        }
        Property::TextShadow(shadows) => {
            style.text_shadows = shadows
                .iter()
                .map(|s| CatTextShadow {
                    offset_x: len_to_px(&s.x_offset, ctx),
                    offset_y: len_to_px(&s.y_offset, ctx),
                    blur_sigma: len_to_px(&s.blur, ctx) / 2.0,
                    color: color_token(&s.color).unwrap_or(ColorToken::Custom(0, 0, 0, 255)),
                })
                .collect();
        }
        Property::WhiteSpace(v) => {
            style.no_wrap = matches!(v, WhiteSpace::NoWrap);
            style.wrap_text = matches!(v, WhiteSpace::Normal | WhiteSpace::PreWrap);
        }
        Property::TextOverflow(v, _) => {
            if matches!(v, TextOverflow::Ellipsis) {
                style.truncate = true;
            }
        }
        Property::TextTransform(v) => {
            style.text_transform = Some(match v.case {
                TextTransformCase::Uppercase => TextTransform::Uppercase,
                TextTransformCase::None => TextTransform::None,
                TextTransformCase::Lowercase | TextTransformCase::Capitalize => {
                    warn_unsupported(PropertyId::TextTransform, "lowercase/capitalize");
                    return;
                }
            });
        }

        // ---- SVG ----
        Property::Fill(paint) => {
            if let SVGPaint::Color(c) = paint {
                style.fill_color = color_token(c);
            }
        }
        Property::Stroke(paint) => {
            if let SVGPaint::Color(c) = paint {
                style.stroke_color = color_token(c);
            }
        }
        Property::StrokeWidth(v) => style.stroke_width = lp_to_px(v, ctx),
        Property::StrokeDasharray(v) => {
            if let lightningcss::properties::svg::StrokeDasharray::Values(pattern) = v {
                // 引擎载体是单值 f32：取首段（等距虚线场景）。
                if let Some(first) = pattern.first() {
                    style.stroke_dasharray = lp_to_px(first, ctx);
                }
            }
        }
        Property::StrokeDashoffset(v) => style.stroke_dashoffset = lp_to_px(v, ctx),

        // lightningcss alpha.72 属性表缺项（object-fit 等）解析为 Custom，
        // 这里逐个识别引擎支持的；未知项照常告警。
        Property::Custom(cp) => apply_custom_property(style, cp),

        other => warn_unsupported(other.property_id(), "第一阶段未接入"),
    }
}

fn apply_custom_property(style: &mut crate::style::NodeStyle, cp: &CustomProperty<'static>) {
    let CustomPropertyName::Unknown(name) = &cp.name else {
        return;
    };
    match name.as_ref() {
        "object-fit" => {
            if let Some(text) = serialize_tokens(&cp.value) {
                style.object_fit = match text.as_str() {
                    "contain" => Some(crate::style::ObjectFit::Contain),
                    "cover" => Some(crate::style::ObjectFit::Cover),
                    "fill" => Some(crate::style::ObjectFit::Fill),
                    other => {
                        log::warn!("css: object-fit 值 {other} 无法识别，已跳过");
                        None
                    }
                };
            }
        }
        other => {
            log::warn!("css: {other} 暂不支持（lightningcss 属性表缺项），已跳过");
        }
    }
}

/// `border: W S C` 简写 → 三通道字段。
fn apply_border(style: &mut crate::style::NodeStyle, v: &Border, ctx: &EvalContext) {
    border_side(&mut style.border_width, &v.width, ctx);
    style.border_style = line_style_to_cat(&v.style);
    style.border_color = color_token(&v.color);
}

fn padding_longhand(slot: &mut Option<f32>, v: &LengthPercentageOrAuto, ctx: &EvalContext) {
    if let LengthPercentageOrAuto::LengthPercentage(lp) = v {
        match lp_to_len_percent(lp, ctx) {
            Some(LenPercent::Px(px)) => *slot = Some(px),
            Some(LenPercent::Pct(_)) => log::warn!("css: 百分比 padding 暂无载体，已跳过"),
            None => {}
        }
    }
}

fn gap_to_px(v: &GapValue, ctx: &EvalContext) -> Option<f32> {
    match v {
        GapValue::LengthPercentage(lp) => lp_to_px(lp, ctx),
        GapValue::Normal => None,
    }
}

fn size_to_cat(v: &Size, ctx: &EvalContext) -> Option<LengthPercentageAuto> {
    match v {
        Size::LengthPercentage(lp) => match lp_to_len_percent(lp, ctx) {
            Some(LenPercent::Px(px)) => Some(LengthPercentageAuto::Length(px)),
            Some(LenPercent::Pct(p)) => Some(LengthPercentageAuto::Percent(p)),
            None => None,
        },
        Size::Auto => Some(LengthPercentageAuto::Auto),
        _ => None,
    }
}

fn border_side(slot: &mut Option<f32>, v: &BorderSideWidth, ctx: &EvalContext) {
    if let BorderSideWidth::Length(l) = v {
        *slot = Some(len_to_px(l, ctx));
    }
}

fn line_style_to_cat(v: &LineStyle) -> Option<crate::style::BorderStyle> {
    Some(match v {
        LineStyle::Solid => crate::style::BorderStyle::Solid,
        LineStyle::Dashed => crate::style::BorderStyle::Dashed,
        LineStyle::Dotted => crate::style::BorderStyle::Dotted,
        _ => return None,
    })
}

/// CSS 分布关键字 → cat 枚举（justify/align 共用 NodeStyle::JustifyContent 载体）。
fn content_distribution_to_cat(d: &ContentDistribution) -> Option<JustifyContent> {
    Some(match d {
        ContentDistribution::SpaceBetween => JustifyContent::Between,
        ContentDistribution::SpaceAround => JustifyContent::Around,
        ContentDistribution::SpaceEvenly => JustifyContent::Evenly,
        ContentDistribution::Stretch => JustifyContent::Stretch,
    })
}

fn content_position_to_cat(v: &ContentPosition) -> Option<JustifyContent> {
    Some(match v {
        ContentPosition::Start | ContentPosition::FlexStart => JustifyContent::Start,
        ContentPosition::Center => JustifyContent::Center,
        ContentPosition::End | ContentPosition::FlexEnd => JustifyContent::End,
        _ => return None,
    })
}

fn justify_to_cat(v: &CssJustifyContent) -> Option<JustifyContent> {
    match v {
        CssJustifyContent::ContentDistribution(d) => content_distribution_to_cat(d),
        CssJustifyContent::ContentPosition { value, .. } => content_position_to_cat(value),
        _ => None,
    }
}

fn align_content_to_cat(v: &AlignContent) -> Option<JustifyContent> {
    match v {
        AlignContent::ContentDistribution(d) => content_distribution_to_cat(d),
        AlignContent::ContentPosition { value, .. } => content_position_to_cat(value),
        _ => None,
    }
}

fn self_position_to_cat(v: &SelfPosition) -> Option<crate::style::AlignItems> {
    Some(match v {
        SelfPosition::Center => crate::style::AlignItems::Center,
        SelfPosition::Start | SelfPosition::FlexStart => crate::style::AlignItems::Start,
        SelfPosition::End | SelfPosition::FlexEnd => crate::style::AlignItems::End,
        _ => return None,
    })
}

fn align_items_to_cat(v: &CssAlignItems) -> Option<crate::style::AlignItems> {
    match v {
        CssAlignItems::Stretch => Some(crate::style::AlignItems::Stretch),
        CssAlignItems::BaselinePosition(_) => Some(crate::style::AlignItems::Baseline),
        CssAlignItems::SelfPosition { value, .. } => self_position_to_cat(value),
        _ => None,
    }
}

fn align_self_to_cat(v: &CssAlignSelf) -> Option<crate::style::AlignItems> {
    match v {
        CssAlignSelf::Stretch => Some(crate::style::AlignItems::Stretch),
        CssAlignSelf::BaselinePosition(_) => Some(crate::style::AlignItems::Baseline),
        CssAlignSelf::SelfPosition { value, .. } => self_position_to_cat(value),
        _ => None, // Auto/Normal 交给父容器
    }
}

fn apply_border_radius(style: &mut crate::style::NodeStyle, v: &CssBorderRadius, ctx: &EvalContext) {
    // 字段是 Size2D<LengthPercentage>（x, y 圆角）；NodeStyle 只有 px 形态，
    // 椭圆圆角取 x。百分比圆角丢弃。
    let corner = |lp: &LengthPercentage| lp_to_px(lp, ctx);
    if let (Some(tl), Some(tr), Some(br), Some(bl)) = (
        corner(&v.top_left.0),
        corner(&v.top_right.0),
        corner(&v.bottom_right.0),
        corner(&v.bottom_left.0),
    ) {
        style.border_radius = Some(BorderRadius {
            top_left: tl,
            top_right: tr,
            bottom_right: br,
            bottom_left: bl,
        });
    }
}

fn apply_box_shadow(style: &mut crate::style::NodeStyle, s: &CssBoxShadow, ctx: &EvalContext) {
    let color = color_token(&s.color).unwrap_or(ColorToken::Custom(0, 0, 0, 102));
    // tailwind v4 的 box-shadow var 链给未用分量填 `0 0 #0000` 占位；
    // alpha 0 无绘制语义，跳过以对齐 tailwind.rs 匹配器的输出。
    if matches!(color, ColorToken::Custom(_, _, _, 0)) {
        return;
    }
    let (x, y, blur, spread) = (
        len_to_px(&s.x_offset, ctx),
        len_to_px(&s.y_offset, ctx),
        len_to_px(&s.blur, ctx) / 2.0,
        len_to_px(&s.spread, ctx),
    );
    if s.inset {
        style.inset_shadow.push(InsetShadow {
            offset_x: x,
            offset_y: y,
            blur_sigma: blur,
            spread,
            color,
        });
    } else {
        style.box_shadow.push(CatBoxShadow {
            offset_x: x,
            offset_y: y,
            blur_sigma: blur,
            spread,
            color,
        });
    }
}

fn blend_to_cat(v: CssBlendMode) -> Option<BlendMode> {
    use CssBlendMode as B;
    Some(match v {
        B::Multiply => BlendMode::Multiply,
        B::Screen => BlendMode::Screen,
        B::Overlay => BlendMode::Overlay,
        B::Darken => BlendMode::Darken,
        B::Lighten => BlendMode::Lighten,
        B::ColorDodge => BlendMode::ColorDodge,
        B::ColorBurn => BlendMode::ColorBurn,
        B::HardLight => BlendMode::HardLight,
        B::SoftLight => BlendMode::SoftLight,
        B::Difference => BlendMode::Difference,
        B::Exclusion => BlendMode::Exclusion,
        B::Hue => BlendMode::Hue,
        B::Saturation => BlendMode::Saturation,
        B::Color => BlendMode::Color,
        B::Luminosity => BlendMode::Luminosity,
        _ => return None,
    })
}

fn filter_list_to_css_filter(list: &FilterList<'static>, ctx: &EvalContext) -> CssFilter {
    let mut ops = Vec::new();
    if let FilterList::Filters(filters) = list {
        for filter in filters {
            let op = match filter {
                // CSS filter: blur(x) 的 x 就是标准差，直用；折半的是
                // box/text-shadow 的 blur radius（σ≈radius/2）。
                Filter::Blur(radius) => {
                    CssFilterOp::new(CssFilterKind::Blur, len_to_px(radius, ctx))
                }
                Filter::Brightness(n) => CssFilterOp::new(CssFilterKind::Brightness, number_or_percentage(n)),
                Filter::Contrast(n) => CssFilterOp::new(CssFilterKind::Contrast, number_or_percentage(n)),
                Filter::Saturate(n) => CssFilterOp::new(CssFilterKind::Saturate, number_or_percentage(n)),
                Filter::Grayscale(n) => CssFilterOp::new(CssFilterKind::Grayscale, number_or_percentage(n)),
                Filter::Invert(n) => CssFilterOp::new(CssFilterKind::Invert, number_or_percentage(n)),
                Filter::Sepia(n) => CssFilterOp::new(CssFilterKind::Sepia, number_or_percentage(n)),
                Filter::HueRotate(angle) => {
                    CssFilterOp::new(CssFilterKind::HueRotate, angle.to_degrees())
                }
                Filter::DropShadow(_) | Filter::Opacity(_) | Filter::Url(_) => continue,
            };
            ops.push(op);
        }
    }
    CssFilter { ops }
}

fn apply_transform(style: &mut crate::style::NodeStyle, list: &TransformList, ctx: &EvalContext) {
    for t in &list.0 {
        match t {
            CssTransform::Translate(x, y) => {
                if let (Some(tx), Some(ty)) = (lp_to_px(x, ctx), lp_to_px(y, ctx)) {
                    style.transforms.push(CatTransform::Translate { x: tx, y: ty });
                } else {
                    log::warn!("css: translate 百分比暂无载体，已跳过");
                }
            }
            CssTransform::TranslateX(x) => {
                if let Some(tx) = lp_to_px(x, ctx) {
                    style.transforms.push(CatTransform::TranslateX { value: tx });
                }
            }
            CssTransform::TranslateY(y) => {
                if let Some(ty) = lp_to_px(y, ctx) {
                    style.transforms.push(CatTransform::TranslateY { value: ty });
                }
            }
            CssTransform::Scale(x, y) => {
                style.transforms.push(CatTransform::Scale {
                    value: number_or_percentage(x),
                });
                let sy = number_or_percentage(y);
                if sy != number_or_percentage(x) {
                    style.transforms.push(CatTransform::ScaleY { value: sy });
                }
            }
            CssTransform::ScaleX(x) => style
                .transforms
                .push(CatTransform::ScaleX { value: number_or_percentage(x) }),
            CssTransform::ScaleY(y) => style
                .transforms
                .push(CatTransform::ScaleY { value: number_or_percentage(y) }),
            CssTransform::Rotate(angle) => style
                .transforms
                .push(CatTransform::RotateDeg { value: angle.to_degrees() }),
            _ => {}
        }
    }
}

fn position_to_origin(pos: &CssPositionValue, ctx: &EvalContext) -> Option<TransformOrigin> {
    // transform-origin 归一为盒子分数：百分比直取，关键字映射端点，
    // `side offset` 沿 CSS 语义（偏移自该侧起算）换算；px 偏移需要盒子
    // 尺寸才能归一，静态期丢弃。
    fn frac(lp: &LengthPercentage, ctx: &EvalContext) -> Option<f32> {
        match lp_to_len_percent(lp, ctx) {
            Some(LenPercent::Pct(p)) => Some(p),
            _ => None,
        }
    }
    fn comp<S: Copy>(
        c: &PositionComponent<S>,
        side: impl Fn(&S) -> Option<f32>,
        ctx: &EvalContext,
    ) -> Option<f32> {
        match c {
            PositionComponent::Center => Some(0.5),
            PositionComponent::Length(lp) => frac(lp, ctx),
            PositionComponent::Side { side: s, offset } => {
                let base = side(s)?;
                Some(match offset {
                    None => base,
                    Some(o) => base + (0.5 - base) * 2.0 * frac(o, ctx)?,
                })
            }
        }
    }
    let h = |s: &HorizontalPositionKeyword| match s {
        HorizontalPositionKeyword::Left => Some(0.0),
        HorizontalPositionKeyword::Right => Some(1.0),
    };
    let v = |s: &VerticalPositionKeyword| match s {
        VerticalPositionKeyword::Top => Some(0.0),
        VerticalPositionKeyword::Bottom => Some(1.0),
    };
    Some(TransformOrigin {
        x: comp(&pos.x, h, ctx)?,
        y: comp(&pos.y, v, ctx)?,
    })
}

fn generic_to_cat(generic: &GenericFontFamily) -> Option<GenericFamily> {
    Some(match generic {
        GenericFontFamily::SansSerif => GenericFamily::SansSerif,
        GenericFontFamily::Serif => GenericFamily::Serif,
        GenericFontFamily::Monospace => GenericFamily::Monospace,
        _ => return None,
    })
}

fn image_to_fill(image: &Image, ctx: &EvalContext) -> Option<BackgroundFill> {
    match image {
        Image::Gradient(g) => gradient_to_fill(g),
        _ => None,
    }
}

fn gradient_to_fill(g: &Gradient) -> Option<BackgroundFill> {
    match g {
        Gradient::Linear(lg) => {
            let (angle, direction) = match &lg.direction {
                LineDirection::Angle(a) => (Some(a.to_degrees()), None),
                LineDirection::Horizontal(h) => (
                    None,
                    Some(match h {
                        HorizontalPositionKeyword::Right => GradientDirection::ToRight,
                        _ => GradientDirection::ToLeft,
                    }),
                ),
                LineDirection::Vertical(v) => (
                    None,
                    Some(match v {
                        VerticalPositionKeyword::Bottom => GradientDirection::ToBottom,
                        _ => GradientDirection::ToTop,
                    }),
                ),
                LineDirection::Corner { horizontal, vertical } => (
                    None,
                    Some(match (horizontal, vertical) {
                        (HorizontalPositionKeyword::Right, VerticalPositionKeyword::Bottom) => {
                            GradientDirection::ToBottomRight
                        }
                        (HorizontalPositionKeyword::Right, _) => GradientDirection::ToRight,
                        (_, VerticalPositionKeyword::Bottom) => GradientDirection::ToBottom,
                        _ => GradientDirection::ToTop,
                    }),
                ),
            };
            Some(BackgroundFill::ArbitraryGradient {
                gradient: crate::style::ArbitraryGradient::LinearGradient {
                    angle_deg: angle,
                    direction,
                    stops: gradient_items_to_stops(&lg.items)?,
                    size: None,
                    repeat: false,
                },
            })
        }
        Gradient::Radial(rg) => Some(BackgroundFill::ArbitraryGradient {
            gradient: crate::style::ArbitraryGradient::RadialGradient {
                center: [0.5, 0.5],
                stops: gradient_items_to_stops(&rg.items)?,
                size: None,
                repeat: false,
                shape: crate::style::RadialShape::default(),
                radii: None,
                extent: crate::style::RadialExtent::default(),
            },
        }),
        _ => None,
    }
}

fn gradient_items_to_stops(items: &[GradientItem<LengthPercentage>]) -> Option<Vec<GradientStop>> {
    let mut stops = Vec::new();
    let total = items.len().max(2) as f32;
    for (i, item) in items.iter().enumerate() {
        let GradientItem::ColorStop(stop) = item else {
            continue;
        };
        let color = css_color_to_f32(&stop.color)?;
        let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let (pos, unit) = match &stop.position {
            Some(LengthPercentage::Percentage(p)) => (p.0, StopUnit::Fraction),
            Some(LengthPercentage::Dimension(d)) => {
                (length_value_to_px(d, &EvalContext::default()), StopUnit::Px)
            }
            None => (i as f32 / (total - 1.0), StopUnit::Fraction),
            _ => continue,
        };
        stops.push(GradientStop {
            pos,
            unit,
            color: ColorToken::Custom(ch(color[0]), ch(color[1]), ch(color[2]), ch(color[3])),
        });
    }
    Some(stops)
}

fn length_value_to_px(v: &LengthValue, ctx: &EvalContext) -> f32 {
    match v {
        LengthValue::Px(x) => *x,
        LengthValue::Rem(x) | LengthValue::Em(x) => x * ctx.rem_base,
        LengthValue::Vw(x) => x * ctx.viewport.0 / 100.0,
        LengthValue::Vh(x) => x * ctx.viewport.1 / 100.0,
        LengthValue::Vmin(x) => x * ctx.viewport.0.min(ctx.viewport.1) / 100.0,
        LengthValue::Vmax(x) => x * ctx.viewport.0.max(ctx.viewport.1) / 100.0,
        LengthValue::Pt(x) => x * 96.0 / 72.0,
        LengthValue::In(x) => x * 96.0,
        LengthValue::Cm(x) => x * 96.0 / 2.54,
        LengthValue::Mm(x) => x * 96.0 / 25.4,
        LengthValue::Pc(x) => x * 16.0,
        _ => 0.0,
    }
}
