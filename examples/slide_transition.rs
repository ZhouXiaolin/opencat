use opencat::{
    Composition, EncodingConfig, FrameCtx, Node, component, light_leak,
    nodes::{div, text},
    transitions::{linear, slide, transition_series},
};

fn scene_panel(label: &str, is_pink: bool) -> Node {
    let mut panel = div()
        .justify_center()
        .items_center()
        .child(text(label).text_px(180.0).text_white());

    panel = if is_pink {
        panel.bg_pink()
    } else {
        panel.bg_blue()
    };

    panel.into()
}

#[component]
fn test(_ctx: &FrameCtx) -> Node {
    let current_frame = _ctx.frame;
    let opacity = (current_frame as f32 / 60.0).min(1.0);
    div()
        .flex_col()
        .justify_center()
        .items_center()
        .bg_gray()
        .text_black()
        .text_px(100.0)
        .child(
            div()
                .absolute()
                .left(100.0)
                .top(100.0)
                .w(100.0)
                .h(100.0)
                .rounded_full()
                .bg_green(),
        )
        .child(text("B").text_px(48.0).opacity(opacity))
        .child(text("C").text_red())
        .into()
}

#[component]
fn slide_transition_demo(_ctx: &FrameCtx) -> Node {
    transition_series()
        .sequence(40, test())
        .transition(slide().timing(linear().duration(30)))
        .sequence(60, scene_panel("B", true))
        .transition(light_leak().timing(linear().duration(120)))
        .sequence(60, scene_panel("A", false))
        .into()
}

fn main() -> anyhow::Result<()> {
    let composition = Composition::new("slide_transition")
        .size(1280, 720)
        .fps(30)
        .root(|_ctx| slide_transition_demo())
        .build()?;

    let encode_config = EncodingConfig::default();
    std::fs::create_dir_all("out")?;
    composition.render_to_mp4("out/slide_transition.mp4", &encode_config)?;
    println!("Rendered out/slide_transition.mp4");

    Ok(())
}
