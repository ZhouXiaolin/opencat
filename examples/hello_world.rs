use opencat::{
    Composition, EncodingConfig, FrameCtx, Node, component,
    nodes::{Div, Text},
};

#[component]
fn title(_ctx: &FrameCtx, title: String, opacity: f32) -> Node {
    Text::new(title).opacity(opacity).text_px(72.0).into()
}

#[component]
fn style_inheritance_demo(_ctx: &FrameCtx) -> Node {
    let current_frame = _ctx.frame;
    let opacity = (current_frame as f32 / 30.0).min(1.0);
    Div::new()
        .flex_col()
        .justify_center()
        .items_center()
        .bg_white()
        .text_black()
        .text_px(100.0)
        .child(
            Div::new()
                .absolute()
                .left(100.0)
                .top(100.0)
                .w(100.0)
                .h(100.0)
                .rounded_full()
                .bg_green(),
        )
        .child(title(format!("Frame: {}", current_frame), opacity))
        .child(Text::new("B").text_px(48.0))
        .child(Text::new("C").text_red())
        .into()
}

fn main() -> anyhow::Result<()> {
    let composition = Composition::new("hello_world")
        .size(1280, 720)
        .fps(30)
        .frames(90)
        .root(style_inheritance_demo)
        .build()?;

    let encode_config = EncodingConfig::default();
    std::fs::create_dir_all("out")?;
    composition.render_to_mp4("out/hello_world.mp4", &encode_config)?;
    println!("Rendered out/hello_world.mp4");

    Ok(())
}
