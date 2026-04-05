use opencat::{
    nodes::{div, text},
    Composition, EncodingConfig, FrameCtx, Node, ScriptDriver,
};

fn hello_world_demo(_ctx: &FrameCtx) -> Node {
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
                .data_id("2"),
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
                .data_id("3"),
        )
        .child(
            text("Ordered transforms")
                .text_px(72.0)
                .text_black()
                .data_id("4"),
        )
        .child(
            text("Blue: translate_x().scale()")
                .text_px(34.0)
                .data_id("5"),
        )
        .child(
            text("Pink: scale().translate_x()")
                .text_px(34.0)
                .text_pink()
                .data_id("6"),
        )
        .into()
}

fn login_screen_demo(_ctx: &FrameCtx) -> Node {
    div()
        .flex_col()
        .items_center()
        .justify_center()
        .min_h_full()
        .w_full()
        .gap(0.0)
        .bg_slate_50()
        .p(24.0)
        .child(
            div()
                .flex_col()
                .w(400.0)
                .max_w_full()
                .gap(28.0)
                .p(40.0)
                .rounded_2xl()
                .bg_white()
                .shadow_lg()
                .border()
                .border_slate_200()
                .child(
                    div()
                        .flex_col()
                        .items_center()
                        .gap(12.0)
                        .child(
                            div()
                                .w(56.0)
                                .h(56.0)
                                .rounded_2xl()
                                .bg_primary()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(text("◆").text_white().text_px(28.0)),
                        )
                        .child(text("Acme").text_px(22.0).font_semibold().text_slate_900())
                        .child(text("Sign in to continue").text_px(15.0).text_slate_500()),
                )
                .child(
                    div()
                        .flex_col()
                        .gap(20.0)
                        .child(
                            div()
                                .flex_col()
                                .gap(8.0)
                                .child(text("Email").text_px(13.0).font_medium().text_slate_700())
                                .child(
                                    div()
                                        .w_full()
                                        .h(48.0)
                                        .px(14.0)
                                        .flex()
                                        .items_center()
                                        .rounded_lg()
                                        .border()
                                        .border_slate_200()
                                        .bg_slate_50()
                                        .child(
                                            text("you@company.com").text_px(15.0).text_slate_400(),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex_col()
                                .gap(8.0)
                                .child(
                                    text("Password")
                                        .text_px(13.0)
                                        .font_medium()
                                        .text_slate_700(),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .h(48.0)
                                        .px(14.0)
                                        .flex()
                                        .items_center()
                                        .rounded_lg()
                                        .border()
                                        .border_slate_200()
                                        .bg_slate_50()
                                        .child(
                                            text("••••••••")
                                                .text_px(15.0)
                                                .text_slate_400()
                                                .tracking_wider(),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .w_full()
                                .child(
                                    div()
                                        .flex_row()
                                        .items_center()
                                        .gap(8.0)
                                        .child(
                                            div()
                                                .w(18.0)
                                                .h(18.0)
                                                .rounded_md()
                                                .border()
                                                .border_slate_300()
                                                .bg_white(),
                                        )
                                        .child(text("Remember me").text_px(13.0).text_slate_600()),
                                )
                                .child(
                                    text("Forgot password?")
                                        .text_px(13.0)
                                        .text_primary()
                                        .font_medium(),
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(52.0)
                                .rounded_xl()
                                .bg_primary()
                                .flex()
                                .items_center()
                                .justify_center()
                                .shadow_md()
                                .child(text("Log in").text_px(16.0).font_semibold().text_white()),
                        ),
                )
                .child(
                    div()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap(6.0)
                        .pt(4.0)
                        .child(
                            text("Don't have an account?")
                                .text_px(14.0)
                                .text_slate_500(),
                        )
                        .child(text("Sign up").text_px(14.0).font_semibold().text_primary()),
                ),
        )
        .into()
}

fn main() -> anyhow::Result<()> {
    let driver = ScriptDriver::from_file("examples/hello_world_anim.js")?;

    let composition = Composition::new("hello_world")
        .size(1280, 720)
        .fps(30)
        .frames(90)
        .root(|_ctx| hello_world_demo(_ctx))
        .script_driver(driver)
        .build()?;

    let encode_config = EncodingConfig::mp4();
    std::fs::create_dir_all("out")?;
    composition.render("out/hello_world.mp4", &encode_config)?;
    println!("Rendered out/hello_world.mp4");

    Ok(())
}
