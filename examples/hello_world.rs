use opencat::{
    Composition, EncodingConfig, FrameCtx, Node, component,
    nodes::{AbsoluteFill, AlignItems, JustifyContent, Text},
};
use skia_safe::Color;

#[component]
fn hello_world(ctx: &FrameCtx) -> Node {
    AbsoluteFill::new()
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .background_color(Color::WHITE)
        .child(
            Text::new(format!("The current frame is {}", ctx.frame))
                .font_size(100.0)
                .color(Color::BLACK),
        )
        .into()
}

fn main() -> anyhow::Result<()> {
    let composition = Composition::new("hello_world")
        .size(1280, 720)
        .fps(30)
        .frames(150)
        .root(hello_world)
        .build()?;

    let encode_config = EncodingConfig::default();
    std::fs::create_dir_all("out")?;
    composition.render_to_mp4("out/hello_world.mp4", &encode_config)?;
    println!("Rendered out/hello_world.mp4");

    Ok(())
}
