use opencat::{
    Composition, FrameCtx, Node, component,
    nodes::{AlignItems, AbsoluteFill, JustifyContent, Text},
    render::render_frame_rgb,
};

#[component]
fn text_scene(ctx: &FrameCtx) -> Node {
    AbsoluteFill::new()
        .bg_white()
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .child(
            Text::new(format!("Frame {}", ctx.frame))
                .font_size(96.0)
                .text_black(),
        )
        .into()
}

#[component]
fn flex_scene(_ctx: &FrameCtx) -> Node {
    AbsoluteFill::new()
        .bg_white()
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .child(Text::new("A").text_px(72.0).text_black())
        .into()
}

#[test]
fn text_scene_should_draw_non_white_pixels() -> anyhow::Result<()> {
    let composition = Composition::new("text_scene")
        .size(640, 360)
        .fps(30)
        .frames(1)
        .root(text_scene)
        .build()?;

    let rgb = render_frame_rgb(&composition, 0)?;
    let has_non_white = rgb.chunks_exact(3).any(|px| px != [255, 255, 255]);

    assert!(has_non_white, "expected text rendering to produce non-white pixels");
    Ok(())
}

#[test]
fn flex_scene_should_draw_near_center() -> anyhow::Result<()> {
    let composition = Composition::new("flex_scene")
        .size(640, 360)
        .fps(30)
        .frames(1)
        .root(flex_scene)
        .build()?;

    let rgb = render_frame_rgb(&composition, 0)?;
    let (w, h) = (640usize, 360usize);
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    let mut found_dark = false;

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 3;
            let px = &rgb[idx..idx + 3];
            if px[0] < 250 || px[1] < 250 || px[2] < 250 {
                found_dark = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    assert!(found_dark, "expected flex scene to draw non-white pixels");
    let content_cx = (min_x + max_x) as f32 / 2.0;
    let content_cy = (min_y + max_y) as f32 / 2.0;
    let frame_cx = w as f32 / 2.0;
    let frame_cy = h as f32 / 2.0;

    assert!(
        (content_cx - frame_cx).abs() < w as f32 * 0.35,
        "expected flex content x-center to be near frame center",
    );
    assert!(
        (content_cy - frame_cy).abs() < h as f32 * 0.35,
        "expected flex content y-center to be near frame center",
    );
    Ok(())
}
