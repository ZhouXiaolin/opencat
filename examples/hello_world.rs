use opencat::{
    Composition, EncodingConfig, FrameCtx, Node, component,
    nodes::{AbsoluteFill, AlignItems, FlexBox, FlexDirection, JustifyContent, Text},
    view::IntoNode,
};
use skia_safe::Color;

#[component]
fn hello_world(ctx: &FrameCtx) -> Node {
    let label = format!("The current frame is {}", ctx.frame);

    AbsoluteFill::new()
        .bg(Color::WHITE)
        .child(
            FlexBox::new()
                .direction(FlexDirection::Column)
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center)
                .child(Text::new(label).font_size(84.0).color(Color::BLACK).into_node())
                .into_node(),
        )
        .into_node()
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
