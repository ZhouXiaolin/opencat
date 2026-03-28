use opencat::{
    Composition, EncodingConfig, FrameCtx, Node, component,
    nodes::{div, image, video},
};

#[component]
fn video_demo(_ctx: &FrameCtx) -> Node {
    div()
        .flex_col()
        .justify_center()
        .items_center()
        .bg_black()
        .child(video("/Users/solaren/Resources/mp4/2.mp4").rounded_full())
        .child(image("/Users/solaren/Resources/png/3.png"))
        .into()
}

fn main() -> anyhow::Result<()> {
    let composition = Composition::new("video_playback")
        .size(1280, 720)
        .fps(30)
        .frames(90)
        .root(|_ctx| video_demo())
        .build()?;

    let encode_config = EncodingConfig::default();
    std::fs::create_dir_all("out")?;
    composition.render_to_mp4("out/video_playback.mp4", &encode_config)?;
    println!("Rendered out/video_playback.mp4");

    Ok(())
}
