use opencat::{Composition, EncodingConfig, FrameCtx, Node, canvas, div, style::ColorToken, text};

const TYPEWRITER_SCRIPT: &str = r##"
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

const headlineFont = new CK.Font(null, 46);
headlineFont.setEdging(CK.FontEdging.SubpixelAntiAlias);

const bodyFont = new CK.Font(null, 24);
bodyFont.setEdging(CK.FontEdging.SubpixelAntiAlias);

const uiFont = new CK.Font(null, 18);
uiFont.setEdging(CK.FontEdging.SubpixelAntiAlias);

const message = "Canvas text API is live. This line is being typed on the Skia backend at 30 fps.";
const maxTypedFrames = 240;
const typing = ctx.animate({
    from: { chars: 0 },
    to: { chars: message.length },
    duration: maxTypedFrames,
    easing: 'linear',
    clamp: true,
});

const visibleCount = Math.max(0, Math.min(message.length, Math.floor(typing.chars)));
const visible = message.slice(0, visibleCount);
const caretBlink = (ctx.frame % 20) < 10;
const caretX = 84 + headlineFont.measureText(visible);
const progress = visibleCount / Math.max(1, message.length);

canvas.clear("#020617");

canvas.drawRect(CK.XYWHRect(48, 48, 1184, 624), fill("#0f172a"));
canvas.drawRRect(CK.RRectXY(CK.XYWHRect(48, 48, 1184, 624), 28, 28), stroke("#1e293b", 2));
canvas.drawRect(CK.XYWHRect(48, 48, 1184, 68), fill("#111827"));
canvas.drawRRect(CK.RRectXY(CK.XYWHRect(48, 48, 1184, 68), 28, 28), stroke("#1e293b", 2));

canvas.drawCircle(90, 82, 8, fill("#fb7185"));
canvas.drawCircle(118, 82, 8, fill("#f59e0b"));
canvas.drawCircle(146, 82, 8, fill("#22c55e"));

canvas.drawText("typewriter_canvas.rs", 182, 90, fill("#cbd5e1"), uiFont);
canvas.drawText(`frame ${ctx.frame + 1} / ${ctx.totalFrames}`, 1028, 90, fill("#64748b"), uiFont);

canvas.drawText("> opencat demo", 84, 168, fill("#38bdf8"), bodyFont);
canvas.drawText(visible, 84, 248, fill("#e2e8f0"), headlineFont);

if (caretBlink) {
    canvas.drawLine(caretX + 8, 202, caretX + 8, 258, stroke("#38bdf8", 4));
}

canvas.drawRect(CK.XYWHRect(84, 314, 1012, 10), fill("#1e293b"));
canvas.drawRRect(
    CK.RRectXY(CK.XYWHRect(84, 314, 1012 * progress, 10), 5, 5),
    fill("#38bdf8"),
);

canvas.drawText("System default font | CanvasKit-style drawText(font, paint)", 84, 382, fill("#94a3b8"), bodyFont);
canvas.drawText("Typing runs for the first 8 seconds. The last 2 seconds hold on the finished frame.", 84, 420, fill("#64748b"), uiFont);
canvas.drawText("measureText() is used to place the caret after the visible substring.", 84, 452, fill("#64748b"), uiFont);
"##;

fn typewriter_scene(_ctx: &FrameCtx) -> Node {
    div()
        .id("typewriter-root")
        .w_full()
        .h_full()
        .bg_slate_950()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id("typewriter-shell")
                .w(1280.0)
                .h(720.0)
                .bg_slate_950()
                .child(
                    canvas()
                        .id("typewriter-canvas")
                        .w_full()
                        .h_full()
                        .rounded_2xl()
                        .overflow_hidden()
                        .border()
                        .border_color(ColorToken::Slate800)
                        .script_source(TYPEWRITER_SCRIPT)
                        .expect("typewriter script should compile"),
                ),
        )
        .child(
            div()
                .id("typewriter-caption")
                .absolute()
                .bottom(28.0)
                .left(36.0)
                .child(
                    text("Canvas typewriter demo with system default font")
                        .id("typewriter-caption-text")
                        .text_px(16.0)
                        .text_slate_500(),
                ),
        )
        .into()
}

fn main() -> anyhow::Result<()> {
    let composition = Composition::new("typewriter_canvas")
        .size(1280, 720)
        .fps(30)
        .frames(300)
        .root(typewriter_scene)
        .build()?;

    std::fs::create_dir_all("out")?;
    composition.render("out/typewriter_canvas.mp4", &EncodingConfig::mp4())?;
    println!("Rendered out/typewriter_canvas.mp4");
    Ok(())
}
