use opencat::{
    Composition, EncodingConfig, FrameCtx, Node, lucide,
    nodes::{div, text},
    style::ColorToken,
};

fn icon_card(id: &str, title: &str, detail: &str, surface: ColorToken, icon: Node) -> Node {
    div()
        .id(id)
        .w(360.0)
        .h(220.0)
        .p(24.0)
        .rounded_2xl()
        .bg_white()
        .border()
        .border_slate_200()
        .shadow_sm()
        .flex_col()
        .justify_between()
        .child(
            div()
                .id(&format!("{id}-header"))
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    text(title)
                        .id(&format!("{id}-title"))
                        .text_px(22.0)
                        .font_semibold()
                        .text_slate_900(),
                )
                .child(
                    text("lucide")
                        .id(&format!("{id}-tag"))
                        .text_px(12.0)
                        .font_medium()
                        .text_slate_500(),
                ),
        )
        .child(
            div()
                .id(&format!("{id}-body"))
                .flex_row()
                .items_center()
                .gap(20.0)
                .child(
                    div()
                        .id(&format!("{id}-surface"))
                        .w(112.0)
                        .h(112.0)
                        .rounded_2xl()
                        .bg(surface)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon),
                )
                .child(
                    div()
                        .id(&format!("{id}-copy"))
                        .w(180.0)
                        .flex_col()
                        .gap(10.0)
                        .child(
                            text(detail)
                                .id(&format!("{id}-detail"))
                                .w_full()
                                .text_px(14.0)
                                .line_height(1.45)
                                .text_slate_600(),
                        ),
                ),
        )
        .into()
}

fn hello_world_demo(_ctx: &FrameCtx) -> Node {
    let play = icon_card(
        "card-play",
        "Play",
        "stroke_color(Blue), stroke_width(3.0), fill_color(Sky200)",
        ColorToken::Blue50,
        lucide("play")
            .id("icon-play")
            .size(72.0, 72.0)
            .stroke_color(ColorToken::Blue)
            .stroke_width(3.0)
            .fill_color(ColorToken::Sky200)
            .into(),
    );

    let heart = icon_card(
        "card-heart",
        "Heart",
        "text_color(Rose500) as stroke fallback, fill_color(Rose100)",
        ColorToken::Rose50,
        lucide("heart")
            .id("icon-heart")
            .size(72.0, 72.0)
            .text_color(ColorToken::Rose500)
            .stroke_width(2.0)
            .fill_color(ColorToken::Rose100)
            .into(),
    );

    let star = icon_card(
        "card-star",
        "Star",
        "thin stroke, warm fill, plus rotate_deg(-8) and opacity(0.9)",
        ColorToken::Amber50,
        lucide("star")
            .id("icon-star")
            .size(76.0, 76.0)
            .stroke_color(ColorToken::Amber600)
            .stroke_width(1.5)
            .fill_color(ColorToken::Amber100)
            .rotate_deg(-8.0)
            .opacity(0.9)
            .into(),
    );

    let badge = icon_card(
        "card-badge",
        "Badge Check",
        "bg(Slate900) behind the icon, white stroke, emerald fill",
        ColorToken::Slate100,
        lucide("badge-check")
            .id("icon-badge")
            .size(72.0, 72.0)
            .bg(ColorToken::Slate900)
            .stroke_color(ColorToken::White)
            .stroke_width(2.5)
            .fill_color(ColorToken::Emerald400)
            .into(),
    );

    let bell = icon_card(
        "card-bell",
        "Bell",
        "outline only: no fill_color, just stroke_color(Slate700) and stroke_width(4.0)",
        ColorToken::Amber50,
        lucide("bell")
            .id("icon-bell")
            .size(68.0, 68.0)
            .stroke_color(ColorToken::Slate700)
            .stroke_width(4.0)
            .into(),
    );

    let shield = icon_card(
        "card-shield",
        "Shield Check",
        "stroke_color(Teal600), fill_color(Teal100), translate_y(-2)",
        ColorToken::Teal50,
        lucide("shield-check")
            .id("icon-shield")
            .size(72.0, 72.0)
            .stroke_color(ColorToken::Teal600)
            .stroke_width(2.0)
            .fill_color(ColorToken::Teal100)
            .translate_y(-2.0)
            .into(),
    );

    div()
        .id("hello-world-root")
        .w_full()
        .h_full()
        .bg_slate_50()
        .p(40.0)
        .flex_col()
        .gap(24.0)
        .child(
            div()
                .id("showcase-copy")
                .flex_col()
                .gap(10.0)
                .child(
                    text("Lucide Showcase")
                        .id("showcase-title")
                        .text_px(40.0)
                        .font_bold()
                        .text_slate_900(),
                )
                .child(
                    text("Each card uses a different icon setup so fill, stroke, stroke width, icon background, opacity, and transforms are easy to compare.")
                        .id("showcase-subtitle")
                        .w_full()
                        .text_px(18.0)
                        .line_height(1.5)
                        .text_slate_600(),
                ),
        )
        .child(
            div()
                .id("showcase-row-top")
                .flex_row()
                .gap(24.0)
                .child(play)
                .child(heart)
                .child(star),
        )
        .child(
            div()
                .id("showcase-row-bottom")
                .flex_row()
                .gap(24.0)
                .child(badge)
                .child(bell)
                .child(shield),
        )
        .into()
}

fn main() -> anyhow::Result<()> {
    let composition = Composition::new("hello_world")
        .size(1280, 720)
        .fps(30)
        .frames(1)
        .root(hello_world_demo)
        .build()?;

    let encode_config = EncodingConfig::png();
    std::fs::create_dir_all("out")?;
    composition.render("out/hello_world.png", &encode_config)?;
    println!("Rendered out/hello_world.png");

    Ok(())
}
