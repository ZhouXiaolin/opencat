use std::f32::consts::PI;

use opencat::{
    Composition, EncodingConfig, FrameCtx, Node, light_leak,
    nodes::{div, image, text, video},
    transitions::{linear, slide, transition_series},
};

const VIDEO_PATH: &str = "/Users/solaren/Resources/mp4/2.mp4";
const IMAGE_PATH: &str = "/Users/solaren/Resources/png/3.png";

fn pulse(frame: u32, fps: u32, speed: f32) -> f32 {
    (((frame as f32 / fps as f32) * speed * PI * 2.0).sin() + 1.0) * 0.5
}

fn swing(frame: u32, fps: u32, speed: f32) -> f32 {
    ((frame as f32 / fps as f32) * speed * PI * 2.0).sin()
}

fn hero_copy(frame: u32) -> &'static str {
    match (frame / 18) % 4 {
        0 => "Video + image baseline",
        1 => "Text changes every beat",
        2 => "Transform stack stays ordered",
        _ => "Transitions connect live scenes",
    }
}

fn scene_one(ctx: &FrameCtx) -> Node {
    let orbit = swing(ctx.frame, ctx.fps, 0.25);
    let badge_scale = 0.92 + pulse(ctx.frame, ctx.fps, 0.75) * 0.16;
    let title_y = swing(ctx.frame, ctx.fps, 0.5) * 10.0;

    div()
        .flex_row()
        .items_center()
        .justify_between()
        .w_full()
        .h_full()
        .bg_slate_900()
        .px(56.0)
        .py(48.0)
        .child(
            div()
                .flex_col()
                .justify_center()
                .w(500.0)
                .gap(18.0)
                .child(
                    text("OpenCat Evaluation Demo")
                        .text_px(64.0)
                        .font_bold()
                        .text_white()
                        .translate(0.0, title_y),
                )
                .child(
                    text(hero_copy(ctx.frame))
                        .text_px(26.0)
                        .text_white()
                        .opacity(0.92),
                )
                .child(
                    text(
                        "Scene 1: video playback, image overlay, live text and chained transforms.",
                    )
                    .text_px(18.0)
                    .text_slate_400()
                    .leading(1.4),
                )
                .child(
                    div()
                        .flex_row()
                        .items_center()
                        .gap(12.0)
                        .pt(10.0)
                        .child(
                            div()
                                .w(14.0)
                                .h(14.0)
                                .rounded_full()
                                .bg_teal_400()
                                .scale(0.9 + pulse(ctx.frame, ctx.fps, 1.2) * 0.35),
                        )
                        .child(text("text").text_px(18.0).text_teal_400())
                        .child(text("image").text_px(18.0).text_yellow())
                        .child(text("video").text_px(18.0).text_pink())
                        .child(text("transform").text_px(18.0).text_white()),
                ),
        )
        .child(
            div()
                .relative()
                .w(640.0)
                .h(580.0)
                .child(
                    video(VIDEO_PATH)
                        .w(640.0)
                        .h(580.0)
                        .cover()
                        .rounded_2xl()
                        .translate_x(orbit * 16.0)
                        .scale(1.0 + pulse(ctx.frame, ctx.fps, 0.35) * 0.04),
                )
                .child(
                    image(IMAGE_PATH)
                        .absolute()
                        .right(20.0)
                        .top(20.0)
                        .w(180.0)
                        .h(180.0)
                        .cover()
                        .rounded_xl()
                        .border()
                        .border_slate_200()
                        .translate_x(orbit * -24.0)
                        .rotate_deg(orbit * 8.0)
                        .scale(badge_scale),
                )
                .child(
                    div()
                        .absolute()
                        .left(24.0)
                        .bottom(24.0)
                        .px(18.0)
                        .py(14.0)
                        .rounded_xl()
                        .bg_black()
                        .opacity(0.85)
                        .child(
                            text("Video is decoded live while overlays keep animating.")
                                .text_px(18.0)
                                .text_white(),
                        ),
                ),
        )
        .into()
}

fn scene_two(ctx: &FrameCtx) -> Node {
    let drift = swing(ctx.frame, ctx.fps, 0.18);
    let card_rotate = swing(ctx.frame + 12, ctx.fps, 0.22) * 6.0;
    let headline = match (ctx.frame / 20) % 3 {
        0 => "Image-first layout",
        1 => "Pinned video preview",
        _ => "Transform-heavy overlay",
    };

    div()
        .relative()
        .w_full()
        .h_full()
        .bg_slate_50()
        .child(
            image(IMAGE_PATH)
                .absolute()
                .left(0.0)
                .top(0.0)
                .w(1280.0)
                .h(720.0)
                .cover()
                .opacity(0.24)
                .scale(1.05 + pulse(ctx.frame, ctx.fps, 0.15) * 0.05),
        )
        .child(
            div()
                .absolute()
                .left(80.0)
                .top(72.0)
                .w(620.0)
                .p(28.0)
                .rounded_2xl()
                .bg_white()
                .shadow_xl()
                .border()
                .border_slate_200()
                .translate_x(drift * 22.0)
                .rotate_deg(card_rotate)
                .scale(0.98 + pulse(ctx.frame, ctx.fps, 0.4) * 0.04)
                .child(text(headline).text_px(52.0).font_bold().text_slate_900())
                .child(
                    text("Scene 2 adds a floating card, rotating image plane, and a smaller live video inset.")
                        .text_px(22.0)
                        .text_slate_600()
                        .pt(14.0)
                        .leading(1.45),
                )
                .child(
                    div()
                        .pt(22.0)
                        .child(
                            text("Every frame rebuilds the declarative tree, but the runtime now reuses much more work underneath.")
                                .text_px(18.0)
                                .text_slate_500()
                                .leading(1.5),
                        ),
                ),
        )
        .child(
            video(VIDEO_PATH)
                .absolute()
                .right(86.0)
                .top(96.0)
                .w(380.0)
                .h(520.0)
                .cover()
                .rounded_2xl()
                .border()
                .border_slate_300()
                .translate_y(drift * -18.0)
                .rotate_deg(drift * -4.0),
        )
        .child(
            text("text changes + transform + image + video")
                .absolute()
                .right(100.0)
                .bottom(78.0)
                .text_px(28.0)
                .font_semibold()
                .text_slate_800()
                .translate_x(drift * -26.0)
                .scale(0.96 + pulse(ctx.frame, ctx.fps, 0.9) * 0.08),
        )
        .into()
}

fn scene_three(ctx: &FrameCtx) -> Node {
    let beam = pulse(ctx.frame, ctx.fps, 0.6);
    let row_shift = swing(ctx.frame, ctx.fps, 0.3) * 14.0;
    let summary = match (ctx.frame / 24) % 3 {
        0 => "Composite stress check",
        1 => "Transition + media + text",
        _ => "Final integrated scene",
    };

    div()
        .w_full()
        .h_full()
        .bg_black()
        .px(52.0)
        .py(48.0)
        .child(
            div()
                .flex_row()
                .justify_between()
                .items_center()
                .w_full()
                .child(
                    div()
                        .flex_col()
                        .gap(10.0)
                        .child(text("Scene 3").text_px(18.0).tracking_wider().text_teal_400())
                        .child(text(summary).text_px(58.0).font_bold().text_white())
                        .child(
                            text("Use this scene to inspect text quality, video composition, image reuse and transform stability after the transitions.")
                                .text_px(20.0)
                                .text_slate_400()
                                .leading(1.45),
                        ),
                )
                .child(
                    div()
                        .w(180.0)
                        .h(180.0)
                        .rounded_full()
                        .bg_teal_500()
                        .opacity(0.25 + beam * 0.45)
                        .scale(0.85 + beam * 0.35)
                        .rotate_deg(beam * 28.0)
                        .translate_x(row_shift),
                ),
        )
        .child(
            div()
                .flex_row()
                .justify_between()
                .items_center()
                .w_full()
                .pt(32.0)
                .gap(24.0)
                .child(
                    image(IMAGE_PATH)
                        .w(360.0)
                        .h(360.0)
                        .cover()
                        .rounded_2xl()
                        .border()
                        .border_slate_700()
                        .rotate_deg(row_shift * 0.2)
                        .scale(0.98 + beam * 0.04),
                )
                .child(
                    video(VIDEO_PATH)
                        .w(500.0)
                        .h(360.0)
                        .cover()
                        .rounded_2xl()
                        .translate_x(row_shift * -0.6),
                )
                .child(
                    div()
                        .flex_col()
                        .w(280.0)
                        .gap(18.0)
                        .child(metric_card("Transitions", "slide + light leak"))
                        .child(metric_card("Dynamic text", hero_copy(ctx.frame)))
                        .child(metric_card("Transforms", "translate -> rotate -> scale")),
                ),
        )
        .into()
}

fn metric_card(label: &str, value: &str) -> Node {
    div()
        .flex_col()
        .gap(8.0)
        .p(18.0)
        .rounded_xl()
        .bg_slate_900()
        .border()
        .border_slate_700()
        .child(text(label).text_px(15.0).text_slate_400())
        .child(text(value).text_px(22.0).font_semibold().text_white())
        .into()
}

fn evaluation_demo(ctx: &FrameCtx) -> Node {
    transition_series()
        .sequence(90, scene_one(ctx))
        .transition(slide().timing(linear().duration(24)))
        .sequence(90, scene_two(ctx))
        .transition(
            light_leak()
                .seed(0.25)
                .retract_seed(1.1)
                .hue_shift(24.0)
                .timing(linear().duration(72)),
        )
        .sequence(90, scene_three(ctx))
        .into()
}

fn main() -> anyhow::Result<()> {
    let composition = Composition::new("video_playback")
        .size(1280, 720)
        .fps(30)
        .root(evaluation_demo)
        .build()?;

    let encode_config = EncodingConfig::mp4();
    std::fs::create_dir_all("out")?;
    composition.render("out/video_playback.mp4", &encode_config)?;
    println!("Rendered out/video_playback.mp4");

    Ok(())
}
