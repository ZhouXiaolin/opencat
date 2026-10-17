//! CSS 引擎单元测试：层叠、var/calc/color-mix 求值、核心属性映射。

use super::{CssEngine, EvalContext};
use crate::style::{ColorToken, FlexDirection, LengthPercentageAuto};

fn engine(css: &str) -> CssEngine {
    CssEngine::from_css_text(css, EvalContext::default()).expect("parse css")
}

#[test]
fn utility_class_and_cascade_order() {
    let eng = engine(
        ".a { width: 100px; }\n.b { width: 200px; }\n.c { width: 300px; }\n",
    );
    let style = eng.compute_node_style("c a b", "n1", "div");
    // 同 specificity 按文档顺序，后者胜。
    assert_eq!(style.width, Some(300.0));
}

#[test]
fn specificity_beats_source_order() {
    let eng = engine(
        ".x { width: 10px; }\n#hero { width: 20px; }\n",
    );
    let style = eng.compute_node_style("x", "hero", "div");
    assert_eq!(style.width, Some(20.0));
}

#[test]
fn layer_ordering_utilities_beat_theme() {
    let eng = engine(
        "@layer theme { .card { width: 10px; } }\n@layer utilities { .card { width: 40px; } }\n@layer theme, utilities;\n",
    );
    let style = eng.compute_node_style("card", "n", "div");
    assert_eq!(style.width, Some(40.0));
}

#[test]
fn var_and_calc_from_root() {
    let eng = engine(
        ":root { --spacing: 0.25rem; --brand: #ff0000; }\n.p { padding: calc(var(--spacing) * 4); background-color: var(--brand); }\n",
    );
    let style = eng.compute_node_style("p", "n", "div");
    assert_eq!(style.padding, Some(16.0));
    assert_eq!(style.bg_color, Some(ColorToken::Custom(255, 0, 0, 255)));
}

#[test]
fn color_mix_oklab() {
    let eng = engine(
        ":root { --c: oklch(63.7% 0.237 25.331); }\n.t { background-color: color-mix(in oklab, var(--c) 50%, transparent); }\n",
    );
    let style = eng.compute_node_style("t", "n", "div");
    let Some(ColorToken::Custom(r, _, _, a)) = style.bg_color else {
        panic!("expected custom color, got {:?}", style.bg_color);
    };
    assert!(a < 255, "alpha 应被压低，实际 {a}");
    assert!(r > 100, "红色通道应保持主导，实际 {r}");
}

#[test]
fn flex_layout_properties() {
    let eng = engine(".col { display: flex; flex-direction: column; gap: 12px; }\n");
    let style = eng.compute_node_style("col", "n", "div");
    assert!(style.is_flex);
    assert_eq!(style.flex_direction, Some(FlexDirection::Col));
    assert_eq!(style.gap, Some(12.0));
}

#[test]
fn margin_percent_and_auto() {
    let eng = engine(".m { margin: 10px auto; }\n");
    let style = eng.compute_node_style("m", "n", "div");
    assert_eq!(style.margin_top, Some(LengthPercentageAuto::Length(10.0)));
    assert_eq!(style.margin_left, Some(LengthPercentageAuto::Auto));
}

#[test]
fn nested_supports_picks_oklab_branch() {
    let eng = engine(
        "@layer utilities {\n  .bg-x {\n    background-color: #ff0000;\n    @supports (color: color-mix(in lab, red, red)) {\n      background-color: color-mix(in srgb, #ff0000 50%, transparent);\n    }\n  }\n}\n",
    );
    let style = eng.compute_node_style("bg-x", "n", "div");
    let Some(ColorToken::Custom(_, _, _, a)) = style.bg_color else {
        panic!("expected custom color");
    };
    assert!(a < 255, "应选中 @supports 里的 color-mix 分支");
}

#[test]
fn shadow_blur_sigma_halves() {
    let eng = engine(".s { box-shadow: 0 10px 12px rgba(0,0,0,0.5); }\n");
    let style = eng.compute_node_style("s", "n", "div");
    assert_eq!(style.box_shadow.len(), 1);
    assert!((style.box_shadow[0].blur_sigma - 6.0).abs() < 0.01);
}


#[test]
fn escaped_arbitrary_property_class() {
    let eng = engine(
        ".\\[stroke-dasharray\\:1800px\\] { stroke-dasharray: 1800; stroke-dashoffset: 1800px; }\n",
    );
    let style = eng.compute_node_style(
        "[stroke-dasharray:1800px] [stroke-dashoffset:1800px]",
        "p",
        "path",
    );
    assert_eq!(style.stroke_dasharray, Some(1800.0), "dasharray 应命中");
    assert_eq!(style.stroke_dashoffset, Some(1800.0), "dashoffset 应命中");
}

#[test]
fn inset_shorthand_expands_to_four_insets() {
    // tailwind `inset-0` 产出 `inset: 0`（简写）。漏接时 absolute 容器
    // 无 inset，flex 居中失效（红盒贴左上角的回归）。
    let eng = engine(".box { inset: 0; position: absolute; }\n");
    let style = eng.compute_node_style("box absolute inset-0", "c", "div");
    let zero = |v: Option<LengthPercentageAuto>| matches!(v, Some(LengthPercentageAuto::Length(0.0)));
    assert!(zero(style.inset_top), "inset 简写应落到 inset_top");
    assert!(zero(style.inset_left), "inset 简写应落到 inset_left");
    assert_eq!(style.position, Some(crate::style::Position::Absolute));
}

#[test]
fn unmatched_classes_reports_runtime_composed_classes() {
    let eng = engine(".left-0 { left: 0px }\n.absolute { position: absolute }\n");
    let missing = eng.unmatched_classes("absolute left-[210px] left-0 w-[1267px]");
    assert_eq!(missing, vec!["left-[210px]", "w-[1267px]"]);
}

#[test]
fn inline_style_overrides_class_rules() {
    // 内联 style 在层叠里高于选择器规则：同一属性内联胜，其余类规则保留。
    let eng = engine(".x{width:100px;height:50px;color:#ff0000}");
    let mut style = eng.compute_node_style("x", "n1", "div");
    eng.apply_inline_style(&mut style, "width: 200px; opacity: 0.5", "n1", "div");
    assert_eq!(style.width, Some(200.0), "内联 width 应覆盖类规则");
    assert_eq!(style.height, Some(50.0), "未内联的属性保留类规则");
    assert_eq!(style.opacity, Some(0.5));
}

#[test]
fn inline_style_supports_shorthand_and_custom_properties() {
    let eng = engine(".x{width:10px}");
    let mut style = eng.compute_node_style("x", "n1", "div");
    eng.apply_inline_style(
        &mut style,
        "padding: 4px 8px; --accent: #00ff00; background-color: var(--accent)",
        "n1",
        "div",
    );
    assert_eq!(style.padding_top, Some(4.0));
    assert_eq!(style.padding_left, Some(8.0));
    assert_eq!(style.bg_color, Some(ColorToken::Custom(0, 255, 0, 255)));
}

#[test]
fn inline_style_drops_invalid_declarations_only() {
    let eng = engine("");
    let mut style = eng.compute_node_style("", "n1", "div");
    eng.apply_inline_style(&mut style, "width: nonsense; height: 30px", "n1", "div");
    assert_eq!(style.width, None, "非法声明被丢弃");
    assert_eq!(style.height, Some(30.0), "其余声明仍生效");
}

#[test]
fn inline_style_calc_normalizes_angle_units() {
    let eng = engine("");
    let mut style = eng.compute_node_style("", "n1", "div");
    eng.apply_inline_style(&mut style, "transform: rotate(calc(0.25turn + 45deg))", "n1", "div");
    assert!(
        matches!(
            style.transforms.as_slice(),
            [crate::style::Transform::RotateDeg { value }] if (value - 135.0).abs() < 1e-3
        ),
        "calc 应把 turn/deg 归一到 135deg，实际 {:?}",
        style.transforms
    );
}

#[test]
fn object_fit_maps_through_custom_property_fallback() {
    // lightningcss alpha.72 属性表没有 object-fit，声明解析为 Custom；
    // 引擎需把它喂到 paint 腿并识别关键字。
    let eng = engine(".x{object-fit:cover}");
    let style = eng.compute_node_style("x", "n1", "image");
    assert_eq!(style.object_fit, Some(crate::style::ObjectFit::Cover));

    let eng = engine(".x{object-fit:contain}");
    let style = eng.compute_node_style("x", "n1", "image");
    assert_eq!(style.object_fit, Some(crate::style::ObjectFit::Contain));
}




#[test]
fn cascade_dedupes_shadow_var_chain_and_drops_placeholders() {
    // tailwind v4 的 shadow-/inset-shadow- 类各自声明同一条 var 链：
    // box-shadow: var(--tw-inset-shadow), <未用占位 var>, var(--tw-shadow)。
    // 层叠需按属性去重（只留胜者），alpha 0 的 `0 0 #0000` 占位不入列表。
    let eng = engine(
        "@property --tw-inset-shadow{syntax:\"*\";inherits:false;initial-value:0 0 #0000}\n\
         @property --tw-shadow{syntax:\"*\";inherits:false;initial-value:0 0 #0000}\n\
         .a{--tw-inset-shadow:inset 0 1px 0 #ffffff38;box-shadow:var(--tw-inset-shadow), var(--tw-shadow)}\n\
         .b{--tw-shadow:0 30px 90px #0000006b;box-shadow:var(--tw-inset-shadow), var(--tw-shadow)}\n",
    );
    let style = eng.compute_node_style("a b", "frame", "div");
    assert_eq!(style.box_shadow.len(), 1);
    assert_eq!(style.box_shadow[0].offset_y, 30.0);
    assert_eq!(style.box_shadow[0].blur_sigma, 45.0);
    assert_eq!(style.inset_shadow.len(), 1);
    assert_eq!(style.inset_shadow[0].offset_y, 1.0);
}

#[test]
fn letter_spacing_em_tracks_element_font_size() {
    // em 相对本元素字号（tracking-[-0.02em] 在 108px 下是 -2.16px），
    // 走 letter_spacing_em 由 resolve_text_style 折算；px 直存。
    let eng = engine(".x{letter-spacing:-0.02em}");
    let style = eng.compute_node_style("x", "n1", "text");
    assert_eq!(style.letter_spacing_em, Some(-0.02));
    assert_eq!(style.letter_spacing, None);

    let eng = engine(".x{letter-spacing:-2.5px}");
    let style = eng.compute_node_style("x", "n1", "text");
    assert_eq!(style.letter_spacing, Some(-2.5));
    assert_eq!(style.letter_spacing_em, None);
}

#[test]
fn filter_blur_sigma_is_used_directly() {
    // CSS filter: blur(x) 的 x 就是标准差，不折半（k3 bloom 层约定）。
    let eng = engine(".x{filter:blur(12px)}");
    let style = eng.compute_node_style("x", "n1", "div");
    let blur = style
        .css_filter
        .ops
        .iter()
        .find(|op| op.kind == crate::style::CssFilterKind::Blur)
        .expect("blur op");
    assert_eq!(blur.value, 12.0);
}
