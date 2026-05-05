//! ScriptDriver 单元测试 — 由 host quickjs 实现驱动。
//! 从 `core/scene/script/mod.rs` 搬迁，避免 core 内出现 `host-default` feature gate。

#![cfg(test)]

use opencat_core::scene::script::*;
use opencat_core::style::{ColorToken, TextAlign, Transform};

#[test]
fn script_driver_records_text_alignment_and_line_height() {
    let driver = ScriptDriver::from_source(
        r#"
        const title = ctx.getNode("title");
        title.textAlign("center").lineHeight(1.8);
    "#,
    )
    .expect("script should compile");

    let mutations = super::run_driver(&driver, 0, 1, 0, 1, None).expect("script should run");
    let title = mutations.get("title").expect("title mutation should exist");

    assert_eq!(title.text_align, Some(TextAlign::Center));
    assert_eq!(title.line_height, Some(1.8));
}

#[test]
fn script_driver_exposes_global_and_scene_frame_fields() {
    let driver = ScriptDriver::from_source(
        r#"
        ctx.getNode("box")
            .translateX(ctx.frame + ctx.totalFrames)
            .translateY(ctx.currentFrame + ctx.sceneFrames);
    "#,
    )
    .expect("script should compile");

    let mutations = super::run_driver(&driver, 12, 240, 3, 30, Some("box"))
        .expect("script should run");
    let node = mutations.get("box").expect("box mutation should exist");

    assert_eq!(
        node.transforms,
        vec![Transform::TranslateX { value: 252.0 }, Transform::TranslateY { value: 33.0 }]
    );
}

#[test]
fn script_driver_preserves_transform_call_order() {
    let driver = ScriptDriver::from_source(
        r#"
        ctx.getNode("box")
            .translateX(40)
            .rotate(15)
            .scale(1.2);
    "#,
    )
    .expect("script should compile");

    let mutations = super::run_driver(&driver, 0, 1, 0, 1, None).expect("script should run");
    let node = mutations.get("box").expect("box mutation should exist");

    assert_eq!(
        node.transforms,
        vec![
            Transform::TranslateX { value: 40.0 },
            Transform::RotateDeg { value: 15.0 },
            Transform::Scale { value: 1.2 },
        ]
    );
}

#[test]
fn script_driver_records_lucide_fill_and_stroke() {
    let driver = ScriptDriver::from_source(
        r#"
        ctx.getNode("icon")
            .strokeColor("blue")
            .strokeWidth(3)
            .fillColor("sky200");
    "#,
    )
    .expect("script should compile");

    let mutations = super::run_driver(&driver, 0, 1, 0, 1, None).expect("script should run");
    let icon = mutations.get("icon").expect("icon mutation should exist");

    assert_eq!(icon.stroke_color, Some(ColorToken::Blue));
    assert_eq!(icon.stroke_width, Some(3.0));
    assert_eq!(icon.fill_color, Some(ColorToken::Sky200));
    assert_eq!(icon.border_color, None);
    assert_eq!(icon.border_width, None);
    assert_eq!(icon.bg_color, None);
}

#[test]
fn script_driver_records_standard_canvaskit_rect_and_image_commands() {
    let driver = ScriptDriver::from_source(
        r##"
        const CK = ctx.CanvasKit;
        const canvas = ctx.getCanvas();
        const fill = new CK.Paint();
        fill.setStyle(CK.PaintStyle.Fill);
        fill.setColor(CK.Color(255, 0, 0, 1));

        const image = ctx.getImage("hero");
        canvas
            .drawRect(CK.XYWHRect(0, 0, 40, 20), fill)
            .drawImageRect(
                image,
                CK.XYWHRect(0, 0, 1, 1),
                CK.XYWHRect(10, 10, 80, 60),
            );
    "##,
    )
    .expect("script should compile");

    let mutations = super::run_driver(&driver, 0, 1, 0, 1, Some("card"))
        .expect("script should run");
    let canvas = mutations
        .get_canvas("card")
        .expect("canvas mutation should exist");

    assert_eq!(
        canvas.commands[0],
        CanvasCommand::SetAntiAlias { enabled: true }
    );
}
