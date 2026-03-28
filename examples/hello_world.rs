use opencat::{
    Composition, EncodingConfig, FrameCtx, Node, component,
    nodes::{div, text},
};

#[component]
fn hello_world_demo(_ctx: &FrameCtx) -> Node {
    let current_frame = _ctx.frame as f32;
    let opacity = (current_frame / 20.0).min(1.0);
    let rotation = ((current_frame - 20.0) * 1.8).clamp(0.0, 24.0);
    let blue_progress = (current_frame / 50.0).min(1.0);
    let blue_translate = 180.0 * blue_progress;
    let blue_scale = 0.8 + blue_progress * 0.7;
    let pink_translate = 140.0 + (current_frame / 45.0).min(1.0) * 40.0;
    let pink_scale = 1.0 + (current_frame / 35.0).min(1.0) * 0.35;
    let label_offset = ((current_frame - 10.0) / 25.0).clamp(0.0, 1.0) * 36.0;
    div()
        .flex_col()
        .justify_center()
        .items_center()
        .gap(28.0)
        .bg_white()
        .text_black()
        .text_px(72.0)
        .child(
            div()
                .absolute()
                .left(160.0)
                .top(120.0)
                .w(120.0)
                .h(120.0)
                .rounded_xl()
                .bg_blue()
                .translate_x(blue_translate)
                .scale(blue_scale),
        )
        .child(
            div()
                .absolute()
                .left(160.0)
                .top(290.0)
                .w(120.0)
                .h(120.0)
                .rounded_xl()
                .bg_pink()
                .scale(pink_scale)
                .translate_x(pink_translate),
        )
        .child(
            text("Ordered transforms")
                .text_px(72.0)
                .text_black()
                .opacity(opacity)
                .rotate_deg(rotation)
                .scale(1.0 + opacity * 0.05),
        )
        .child(
            text("Blue: translate_x().scale()")
                .text_px(34.0)
                .translate_x(-label_offset)
                .opacity((current_frame / 24.0).min(1.0)),
        )
        .child(
            text("Pink: scale().translate_x()")
                .text_px(34.0)
                .text_pink()
                .translate_x(label_offset)
                .opacity((current_frame / 28.0).min(1.0)),
        )
        .into()
}

fn main() -> anyhow::Result<()> {
    let composition = Composition::new("hello_world")
        .size(1280, 720)
        .fps(30)
        .frames(90)
        .root(|_ctx| hello_world_demo())
        .build()?;

    let encode_config = EncodingConfig::default();
    std::fs::create_dir_all("out")?;
    composition.render_to_mp4("out/hello_world.mp4", &encode_config)?;
    println!("Rendered out/hello_world.mp4");

    Ok(())
}
