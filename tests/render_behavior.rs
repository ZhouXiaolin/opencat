use opencat::{
    Composition, FrameCtx, Node, component,
    nodes::{AlignItems, Div, JustifyContent, Text},
    render::render_frame_rgb,
    transitions::{TransitionSeries, linear, slide},
};

#[component]
fn text_scene(ctx: &FrameCtx) -> Node {
    Div::new()
        .bg_white()
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .child(
            Text::new(format!("Frame {}", ctx.frame))
                .text_px(96.0)
                .text_black(),
        )
        .into()
}

#[component]
fn flex_scene(_ctx: &FrameCtx) -> Node {
    Div::new()
        .bg_white()
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .child(Text::new("A").text_px(72.0).text_black())
        .into()
}

#[component]
fn title(_ctx: &FrameCtx, title: String) -> Node {
    Text::new(title).text_px(96.0).text_black().into()
}

#[component]
fn prop_component_scene(_ctx: &FrameCtx) -> Node {
    Div::new()
        .bg_white()
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .child(title(String::from("Hello Props")))
        .into()
}

#[component]
fn opacity_scene(_ctx: &FrameCtx) -> Node {
    Div::new()
        .bg_white()
        .child(
            Div::new()
                .absolute()
                .left(270.0)
                .top(130.0)
                .w(100.0)
                .h(100.0)
                .bg_red()
                .opacity(0.5),
        )
        .into()
}

#[component]
fn nested_absolute_offset_scene(_ctx: &FrameCtx) -> Node {
    Div::new()
        .bg_white()
        .child(
            Div::new()
                .absolute()
                .left(320.0)
                .top(0.0)
                .w(320.0)
                .h(360.0)
                .bg_red()
                .child(Div::new().bg_blue()),
        )
        .into()
}

fn color_scene(label: &str, is_pink: bool) -> Node {
    let base = Div::new()
        .justify_center()
        .items_center()
        .child(Text::new(label).text_px(120.0).text_white());

    if is_pink {
        base.bg_pink().into()
    } else {
        base.bg_blue().into()
    }
}

#[component]
fn transition_series_scene(_ctx: &FrameCtx) -> Node {
    let scene_a = color_scene("A", false);
    let scene_b = color_scene("B", true);

    TransitionSeries::new()
        .sequence(40, scene_a)
        .transition(slide().timing(linear().duration(30)))
        .sequence(60, scene_b)
        .into()
}

fn pixel_at(rgb: &[u8], width: usize, x: usize, y: usize) -> [u8; 3] {
    let idx = (y * width + x) * 3;
    [rgb[idx], rgb[idx + 1], rgb[idx + 2]]
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

    assert!(
        has_non_white,
        "expected text rendering to produce non-white pixels"
    );
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

#[test]
fn prop_component_scene_should_render_passed_text() -> anyhow::Result<()> {
    let composition = Composition::new("prop_component_scene")
        .size(640, 360)
        .fps(30)
        .frames(1)
        .root(prop_component_scene)
        .build()?;

    let rgb = render_frame_rgb(&composition, 0)?;
    let has_non_white = rgb.chunks_exact(3).any(|px| px != [255, 255, 255]);

    assert!(
        has_non_white,
        "expected prop-driven component rendering to produce non-white pixels",
    );
    Ok(())
}

#[test]
fn opacity_scene_should_blend_with_background() -> anyhow::Result<()> {
    let composition = Composition::new("opacity_scene")
        .size(640, 360)
        .fps(30)
        .frames(1)
        .root(opacity_scene)
        .build()?;

    let rgb = render_frame_rgb(&composition, 0)?;
    let idx = (180usize * 640usize + 320usize) * 3;
    let pixel = &rgb[idx..idx + 3];

    assert_eq!(
        pixel[0], 255,
        "expected red channel to stay fully saturated"
    );
    assert!(
        (120..=135).contains(&pixel[1]),
        "expected green channel to be blended near half opacity, got {}",
        pixel[1]
    );
    assert!(
        (120..=135).contains(&pixel[2]),
        "expected blue channel to be blended near half opacity, got {}",
        pixel[2]
    );
    Ok(())
}

#[test]
fn nested_children_should_respect_absolute_parent_offset() -> anyhow::Result<()> {
    let composition = Composition::new("nested_absolute_offset_scene")
        .size(640, 360)
        .fps(30)
        .frames(1)
        .root(nested_absolute_offset_scene)
        .build()?;

    let rgb = render_frame_rgb(&composition, 0)?;

    let left_idx = (180usize * 640usize + 160usize) * 3;
    let left_pixel = &rgb[left_idx..left_idx + 3];
    assert_eq!(
        left_pixel,
        [255, 255, 255],
        "expected left half to remain white when absolute parent is offset",
    );

    let right_idx = (180usize * 640usize + 480usize) * 3;
    let right_pixel = &rgb[right_idx..right_idx + 3];
    assert!(
        right_pixel[2] > right_pixel[0] && right_pixel[2] > right_pixel[1],
        "expected nested child to be drawn within the offset parent bounds, got {:?}",
        right_pixel,
    );

    Ok(())
}

#[test]
fn transition_series_should_render_first_transition_and_last_segments() -> anyhow::Result<()> {
    let composition = Composition::new("transition_series_scene")
        .size(640, 360)
        .fps(30)
        .frames(130)
        .root(transition_series_scene)
        .build()?;

    let frame_0 = render_frame_rgb(&composition, 0)?;
    assert_eq!(
        pixel_at(&frame_0, 640, 80, 180),
        [59, 130, 246],
        "expected the first segment to show scene A",
    );

    let transition_frame = render_frame_rgb(&composition, 55)?;
    assert_eq!(
        pixel_at(&transition_frame, 640, 160, 180),
        [236, 72, 153],
        "expected the left half to show scene B during the transition",
    );
    assert_eq!(
        pixel_at(&transition_frame, 640, 480, 180),
        [59, 130, 246],
        "expected the right half to show scene A during the transition",
    );

    let final_frame = render_frame_rgb(&composition, 100)?;
    assert_eq!(
        pixel_at(&final_frame, 640, 80, 180),
        [236, 72, 153],
        "expected the last segment to show scene B",
    );

    Ok(())
}

#[test]
fn composition_should_infer_frames_from_transition_series_root() -> anyhow::Result<()> {
    let composition = Composition::new("transition_series_scene")
        .size(640, 360)
        .fps(30)
        .root(transition_series_scene)
        .build()?;

    assert_eq!(
        composition.frames, 130,
        "expected composition to infer total frames from the transition series root",
    );

    Ok(())
}
