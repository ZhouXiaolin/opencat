use opencat::{
    Composition, EncodingConfig, FrameCtx, Node,
    nodes::{canvas, div, text},
    style::ColorToken,
};

const PENDULUM_SCRIPT: &str = r##"
const CK = ctx.CanvasKit;
const canvas = ctx.getCanvas();
const fill = (color) => {
    const paint = new CK.Paint();
    paint.setStyle(CK.PaintStyle.Fill);
    paint.setColor(CK.parseColorString(color));
    return paint;
};
const stroke = (color, width = 1) => {
    const paint = new CK.Paint();
    paint.setStyle(CK.PaintStyle.Stroke);
    paint.setColor(CK.parseColorString(color));
    paint.setStrokeWidth(width);
    return paint;
};
const t = ctx.frame / 30.0;
const gravity = 9.81;
const length = 220.0;
const amplitude = 0.55;
const omega = Math.sqrt(gravity / length) * 3.2;
const angle = amplitude * Math.cos(omega * t);

canvas.clear("#f8fafc");

canvas.drawRect(CK.XYWHRect(0, 0, 960, 44), fill("#0f172a"));
canvas.drawRect(CK.XYWHRect(0, 44, 960, 4), fill("#334155"));

const pivotX = 480;
const pivotY = 96;
const bobSize = 44;

canvas.save();
canvas.translate(pivotX, pivotY);
canvas.rotate(angle * 180 / Math.PI);
canvas.drawRect(CK.XYWHRect(-4, 0, 8, length), fill("#0f172a"));
canvas.drawRect(
    CK.XYWHRect(-bobSize / 2, length - bobSize / 2, bobSize, bobSize),
    fill("#0ea5e9"),
);
canvas.drawRect(
    CK.XYWHRect(-bobSize / 2, length - bobSize / 2, bobSize, bobSize),
    stroke("#082f49", 3),
);
canvas.restore();

canvas.drawRect(CK.XYWHRect(pivotX - 10, pivotY - 10, 20, 20), fill("#e2e8f0"));
canvas.drawRect(CK.XYWHRect(pivotX - 10, pivotY - 10, 20, 20), stroke("#0f172a", 3));

const energy = (1 - Math.cos(angle)) * 100;
canvas.drawRect(CK.XYWHRect(120, 560, 720, 14), fill("#cbd5e1"));
canvas.drawRect(
    CK.XYWHRect(120, 560, Math.max(0, Math.min(720, energy / 100 * 720)), 14),
    fill("#38bdf8"),
);
"##;

fn pendulum_scene(_ctx: &FrameCtx) -> Node {
    div()
        .id("pendulum-root")
        .w_full()
        .h_full()
        .bg_slate_100()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id("pendulum-card")
                .w(1100.0)
                .h(660.0)
                .rounded_2xl()
                .bg_white()
                .border()
                .border_slate_200()
                .shadow_md()
                .p(28.0)
                .flex_col()
                .gap(20.0)
                .child(
                    div()
                        .id("pendulum-copy")
                        .w_full()
                        .h(64.0)
                        .flex_col()
                        .justify_center()
                        .gap(6.0)
                        .child(
                            text("Canvas Pendulum")
                                .id("pendulum-title")
                                .text_px(34.0)
                                .font_bold()
                                .text_slate_900(),
                        )
                        .child(
                            text("这个示例直接在 backend 的 Skia canvas 上执行脚本。摆角由 ctx.frame 驱动，图形通过 CanvasKit 风格的 ctx.getCanvas() 子集逐帧绘制。")
                                .id("pendulum-subtitle")
                                .text_px(16.0)
                                .line_height(1.55)
                                .text_slate_600(),
                        ),
                )
                .child(
                    canvas()
                        .id("pendulum-canvas")
                        .w_full()
                        .flex_1()
                        .rounded_xl()
                        .overflow_hidden()
                        .border()
                        .border_color(ColorToken::Slate200)
                        .script_source(PENDULUM_SCRIPT)
                        .expect("pendulum script should compile"),
                ),
        )
        .into()
}

fn main() -> anyhow::Result<()> {
    let composition = Composition::new("pendulum_canvas")
        .size(1280, 720)
        .fps(30)
        .frames(180)
        .root(pendulum_scene)
        .build()?;

    std::fs::create_dir_all("out")?;
    composition.render("out/pendulum_canvas.mp4", &EncodingConfig::mp4())?;
    println!("Rendered out/pendulum_canvas.mp4");
    Ok(())
}
