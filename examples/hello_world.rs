use opencat::{
    Composition, EncodingConfig, FrameCtx, Node, component,
    nodes::{AbsoluteFill, Text},
};

#[component]
fn style_inheritance_demo(_ctx: &FrameCtx) -> Node {
    let current_frame = _ctx.frame;
    AbsoluteFill::new()
        .flex_col()
        .justify_evenly()
        .items_center()
        .bg_white()
        .text_black()
        .text_px(100.0)
        .child(Text::new(&format!("Frame: {}", current_frame)))
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
