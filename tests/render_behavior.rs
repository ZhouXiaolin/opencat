use opencat::{
    Composition, FrameCtx, Node, NodeKind, component,
    media::MediaContext,
    nodes::{AlignItems, JustifyContent, div, text},
    render::render_frame_rgb,
    transitions::{linear, slide, transition_series},
};

fn render_frame(composition: &Composition, frame_index: u32) -> anyhow::Result<Vec<u8>> {
    let mut media_ctx = MediaContext::new();
    render_frame_rgb(composition, frame_index, &mut media_ctx)
}

#[component]
fn text_scene(ctx: &FrameCtx) -> Node {
    div()
        .bg_white()
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .child(
            text(format!("Frame {}", ctx.frame))
                .text_px(96.0)
                .text_black(),
        )
        .into()
}

#[component]
fn flex_scene(_ctx: &FrameCtx) -> Node {
    div()
        .bg_white()
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .child(text("A").text_px(72.0).text_black())
        .into()
}

#[component]
fn title(_ctx: &FrameCtx, title: String) -> Node {
    text(title).text_px(96.0).text_black().into()
}

#[component]
fn prop_component_scene(_ctx: &FrameCtx) -> Node {
    div()
        .bg_white()
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .child(title(String::from("Hello Props")))
        .into()
}

#[component]
fn opacity_scene(_ctx: &FrameCtx) -> Node {
    div()
        .bg_white()
        .child(
            div()
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
    div()
        .bg_white()
        .child(
            div()
                .absolute()
                .left(320.0)
                .top(0.0)
                .w(320.0)
                .h(360.0)
                .bg_red()
                .child(div().bg_blue()),
        )
        .into()
}

fn color_scene(label: &str, is_pink: bool) -> Node {
    let base = div()
        .justify_center()
        .items_center()
        .child(text(label).text_px(120.0).text_white());

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

    transition_series()
        .sequence(40, scene_a)
        .transition(slide().timing(linear().duration(30)))
        .sequence(60, scene_b)
        .into()
}

#[component]
fn nested_child_component(_ctx: &FrameCtx) -> Node {
    text("Nested").text_px(72.0).text_black().into()
}

#[component]
fn parent_component_calls_child_without_ctx(_ctx: &FrameCtx) -> Node {
    div()
        .bg_white()
        .justify_center()
        .items_center()
        .child(nested_child_component())
        .into()
}

#[component]
fn transform_order_scene(_ctx: &FrameCtx) -> Node {
    div()
        .bg_white()
        .child(
            div()
                .absolute()
                .left(120.0)
                .top(130.0)
                .w(40.0)
                .h(40.0)
                .bg_blue()
                .translate_x(80.0)
                .scale(2.0),
        )
        .child(
            div()
                .absolute()
                .left(120.0)
                .top(210.0)
                .w(40.0)
                .h(40.0)
                .bg_pink()
                .scale(2.0)
                .translate_x(80.0),
        )
        .into()
}

fn pixel_at(rgb: &[u8], width: usize, x: usize, y: usize) -> [u8; 3] {
    let idx = (y * width + x) * 3;
    [rgb[idx], rgb[idx + 1], rgb[idx + 2]]
}

fn color_distance(a: [u8; 3], b: [u8; 3]) -> u32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    (dr * dr + dg * dg + db * db) as u32
}

fn color_bounds(rgb: &[u8], width: usize, height: usize, color: [u8; 3]) -> Option<[usize; 4]> {
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    let mut found = false;

    for y in 0..height {
        for x in 0..width {
            if pixel_at(rgb, width, x, y) == color {
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    found.then_some([min_x, min_y, max_x, max_y])
}

#[test]
fn node_kind_should_expose_concrete_variant() {
    let node = text("Hello enum");

    let node = Node::from(node);
    match node.kind() {
        NodeKind::Text(text) => assert_eq!(text.content(), "Hello enum"),
        _ => panic!("expected text node"),
    }
}

#[test]
fn text_scene_should_draw_non_white_pixels() -> anyhow::Result<()> {
    let composition = Composition::new("text_scene")
        .size(640, 360)
        .fps(30)
        .frames(1)
        .root(|_ctx| text_scene())
        .build()?;

    let rgb = render_frame(&composition, 0)?;
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
        .root(|_ctx| flex_scene())
        .build()?;

    let rgb = render_frame(&composition, 0)?;
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
        .root(|_ctx| prop_component_scene())
        .build()?;

    let rgb = render_frame(&composition, 0)?;
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
        .root(|_ctx| opacity_scene())
        .build()?;

    let rgb = render_frame(&composition, 0)?;
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
        .root(|_ctx| nested_absolute_offset_scene())
        .build()?;

    let rgb = render_frame(&composition, 0)?;

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
        .root(|_ctx| transition_series_scene())
        .build()?;

    let frame_0 = render_frame(&composition, 0)?;
    assert_eq!(
        pixel_at(&frame_0, 640, 80, 180),
        [59, 130, 246],
        "expected the first segment to show scene A",
    );

    let transition_frame = render_frame(&composition, 55)?;
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

    let final_frame = render_frame(&composition, 100)?;
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
        .root(|_ctx| transition_series_scene())
        .build()?;

    assert_eq!(
        composition.frames, 130,
        "expected composition to infer total frames from the transition series root",
    );

    Ok(())
}

#[test]
fn slide_transition_should_still_show_expected_halves_midway() -> anyhow::Result<()> {
    // Build the same transition series used by the existing integration test but
    // focus exclusively on the midway frame to guard against regressions when the
    // slide presentation is refactored from a layout trick to picture-based rendering.
    let composition = Composition::new("slide_midway_test")
        .size(640, 360)
        .fps(30)
        .frames(130)
        .root(|_ctx| transition_series_scene())
        .build()?;

    // Timeline: sequence(A, 40) | slide(30) | sequence(B, 60)
    // Frame 55 is 15 frames into the 30-frame slide transition (progress = 0.5).
    let rgb = render_frame(&composition, 55)?;

    // At 50 % progress the incoming scene B (pink) should occupy the left half.
    let left = pixel_at(&rgb, 640, 160, 180);
    assert_eq!(
        left,
        [236, 72, 153],
        "expected left half to show scene B (pink) at midway of slide transition",
    );

    // The outgoing scene A (blue) should still be visible on the right half.
    let right = pixel_at(&rgb, 640, 480, 180);
    assert_eq!(
        right,
        [59, 130, 246],
        "expected right half to show scene A (blue) at midway of slide transition",
    );

    Ok(())
}

#[test]
fn light_leak_transition_should_render_non_uniform_transition_pixels() -> anyhow::Result<()> {
    use opencat::transitions::light_leak;

    let scene_a = color_scene("A", false);
    let scene_b = color_scene("B", true);

    let composition = Composition::new("light_leak_test")
        .size(640, 360)
        .fps(30)
        .frames(100)
        .root(move |_ctx| {
            transition_series()
                .sequence(20, scene_a.clone())
                .transition(light_leak().timing(linear().duration(30)))
                .sequence(50, scene_b.clone())
                .into()
        })
        .build()?;

    // Frame 10 is still scene A. Frame 35 is 15 frames into the 30-frame
    // light leak transition (progress = 0.5).
    let rgb_scene_a = render_frame(&composition, 10)?;
    let rgb = render_frame(&composition, 35)?;

    assert_ne!(
        rgb, rgb_scene_a,
        "expected the midway light leak frame to differ from the static scene A frame"
    );

    // A light leak produces non-uniform, organic pixel blending rather than a single
    // flat colour. Assert that at least three distinct colours appear in the frame.
    let mut unique_colors = std::collections::HashSet::new();
    for px in rgb.chunks_exact(3) {
        unique_colors.insert([px[0], px[1], px[2]]);
        if unique_colors.len() >= 3 {
            break;
        }
    }

    assert!(
        unique_colors.len() >= 3,
        "expected light leak transition to produce at least 3 distinct pixel colours at midway, \
         but found only {:?}",
        unique_colors,
    );

    Ok(())
}

#[test]
fn light_leak_transition_should_be_closer_to_target_scene_near_the_end() -> anyhow::Result<()> {
    use opencat::transitions::light_leak;

    let scene_a = color_scene("A", false);
    let scene_b = color_scene("B", true);

    let composition = Composition::new("light_leak_direction_test")
        .size(640, 360)
        .fps(30)
        .frames(100)
        .root(move |_ctx| {
            transition_series()
                .sequence(20, scene_b.clone())
                .transition(light_leak().timing(linear().duration(30)))
                .sequence(50, scene_a.clone())
                .into()
        })
        .build()?;

    let rgb = render_frame(&composition, 49)?;
    let px = pixel_at(&rgb, 640, 40, 40);

    let distance_to_target = color_distance(px, [59, 130, 246]);
    let distance_to_source = color_distance(px, [236, 72, 153]);

    assert!(
        distance_to_target < distance_to_source,
        "expected a late light leak frame to be closer to the target scene than the source scene, but pixel {:?} was not",
        px,
    );

    Ok(())
}

#[test]
fn component_can_call_child_component_without_passing_ctx() -> anyhow::Result<()> {
    let composition = Composition::new("parent_component_calls_child_without_ctx")
        .size(640, 360)
        .fps(30)
        .frames(1)
        .root(|_ctx| parent_component_calls_child_without_ctx())
        .build()?;

    let rgb = render_frame(&composition, 0)?;
    let has_non_white = rgb.chunks_exact(3).any(|px| px != [255, 255, 255]);

    assert!(
        has_non_white,
        "expected nested child component to render when called without ctx",
    );

    Ok(())
}

#[test]
fn transition_frames_should_compile_to_transition_display_commands() -> anyhow::Result<()> {
    use opencat::display::build::build_display_list;
    use opencat::display::list::DisplayCommand;
    use opencat::element::resolve::resolve_ui_tree;
    use opencat::layout::compute_layout;
    use opencat::transitions::{TransitionKind, TransitionNode};

    let from = color_scene("A", false);
    let to = color_scene("B", true);
    let transition = Node::from(TransitionNode::new(from, to, 0.5, TransitionKind::Slide));

    let mut media_ctx = MediaContext::new();
    let frame_ctx = FrameCtx {
        frame: 0,
        fps: 30,
        width: 640,
        height: 360,
        frames: 1,
    };

    let element = resolve_ui_tree(&transition, &frame_ctx, &mut media_ctx);
    let layout = compute_layout(&element, &frame_ctx)?;
    let display_list = build_display_list(&layout, &frame_ctx)?;

    let has_transition = display_list
        .commands
        .iter()
        .any(|cmd| matches!(cmd, DisplayCommand::Transition { .. }));
    assert!(
        has_transition,
        "expected display list to contain a Transition command"
    );

    Ok(())
}

#[test]
fn transform_order_should_change_the_final_position() -> anyhow::Result<()> {
    let composition = Composition::new("transform_order_scene")
        .size(640, 360)
        .fps(30)
        .frames(1)
        .root(|_ctx| transform_order_scene())
        .build()?;

    let rgb = render_frame(&composition, 0)?;
    let blue_bounds = color_bounds(&rgb, 640, 360, [59, 130, 246])
        .expect("expected to find blue pixels for the translated-then-scaled square");
    let pink_bounds = color_bounds(&rgb, 640, 360, [236, 72, 153])
        .expect("expected to find pink pixels for the scaled-then-translated square");

    assert!(
        blue_bounds[1] < pink_bounds[1],
        "expected the blue square to stay above the pink square",
    );
    assert!(
        blue_bounds[0] > pink_bounds[0],
        "expected translate().scale() to place the blue square further right than scale().translate(), got blue={blue_bounds:?}, pink={pink_bounds:?}",
    );
    assert!(
        blue_bounds[2] > pink_bounds[2],
        "expected transform order to change the final right edge, got blue={blue_bounds:?}, pink={pink_bounds:?}",
    );

    Ok(())
}
