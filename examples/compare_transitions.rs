use std::time::Instant;

use opencat::{
    Composition, FrameCtx, Node, RenderSession, canvas, div, image, light_leak, linear,
    render_frame_rgba, slide, text, timeline, video,
};

const VIDEO_PATH: &str = "/Users/solaren/Resources/mp4/2.mp4";
const IMAGE_PATH: &str = "/Users/solaren/Resources/png/3.png";
const TRANSITION_FRAMES: u32 = 24;

const CANVAS_SCRIPT_A: &str = r##"
const CK = ctx.CanvasKit;
const canvas = ctx.getCanvas();
const fill = (color) => {
    const paint = new CK.Paint();
    paint.setStyle(CK.PaintStyle.Fill);
    paint.setColor(CK.parseColorString(color));
    return paint;
};
const stroke = (color, width = 1, cap = CK.StrokeCap.Butt, join = CK.StrokeJoin.Miter) => {
    const paint = new CK.Paint();
    paint.setStyle(CK.PaintStyle.Stroke);
    paint.setColor(CK.parseColorString(color));
    paint.setStrokeWidth(width);
    paint.setStrokeCap(cap);
    paint.setStrokeJoin(join);
    return paint;
};
const width = 220;
const height = 140;
const t = ctx.frame / ctx.fps;
const orbit = Math.sin(t * Math.PI * 2.0 * 0.25);
const pulse = (Math.sin(t * Math.PI * 2.0 * 0.75) + 1.0) * 0.5;
const image = ctx.getImage("stage-thumb");

canvas.clear();
canvas.drawRRect(CK.RRectXY(CK.XYWHRect(0, 0, width, height), 20, 20), fill("#0f172ac7"));
canvas.drawRRect(
    CK.RRectXY(CK.XYWHRect(0, 0, width, height), 20, 20),
    stroke("#94a3b86b", 1.5),
);
canvas.drawRect(CK.XYWHRect(18, 26, width - 36, 2), fill("#2dd4bf59"));
canvas.drawRect(CK.XYWHRect(18, 70, width - 36, 2), fill("#2dd4bf29"));

canvas.save();
canvas.translate(width * 0.5, height * 0.5);
canvas.rotate(orbit * 10.0);
canvas.drawCircle(0, 0, 26 + pulse * 10, fill("#2dd4bf24"));
canvas.drawCircle(0, 0, 32 + pulse * 8, stroke("#2dd4bf8c", 2));
canvas.drawLine(-68, 0, 68, 0, stroke("#e2e8f0b8"));
canvas.drawLine(0, -40, 0, 40, stroke("#e2e8f05c", 1.5));
canvas.drawCircle(orbit * 68, 0, 8 + pulse * 4, fill("#f8fafc"));
canvas.restore();

canvas.drawImageRect(image, CK.XYWHRect(0, 0, 1, 1), CK.XYWHRect(144, 24, 56, 56));
canvas.drawRRect(
    CK.RRectXY(CK.XYWHRect(144, 24, 56, 56), 12, 12),
    stroke("#f8fafc61", 1.5),
);
"##;

const CANVAS_SCRIPT_B: &str = r##"
const CK = ctx.CanvasKit;
const canvas = ctx.getCanvas();
const fill = (color) => {
    const paint = new CK.Paint();
    paint.setStyle(CK.PaintStyle.Fill);
    paint.setColor(CK.parseColorString(color));
    return paint;
};
const stroke = (color, width = 1, cap = CK.StrokeCap.Butt, join = CK.StrokeJoin.Miter) => {
    const paint = new CK.Paint();
    paint.setStyle(CK.PaintStyle.Stroke);
    paint.setColor(CK.parseColorString(color));
    paint.setStrokeWidth(width);
    paint.setStrokeCap(cap);
    paint.setStrokeJoin(join);
    return paint;
};
const width = 240;
const height = 160;
const t = ctx.frame / ctx.fps;
const wave = Math.sin(t * Math.PI * 2.0 * 0.33);
const bob = (Math.sin(t * Math.PI * 2.0 * 0.9) + 1.0) * 0.5;
const image = ctx.getImage("panel-thumb");

canvas.clear();
canvas.drawRRect(CK.RRectXY(CK.XYWHRect(0, 0, width, height), 22, 22), fill("#111827dd"));
canvas.drawRRect(
    CK.RRectXY(CK.XYWHRect(0, 0, width, height), 22, 22),
    stroke("#60a5fa66", 2),
);

const path = new CK.Path();
path.moveTo(22, 102);
path.quadTo(78, 34 + wave * 20, 130, 96);
path.cubicTo(160, 132, 196, 56 - wave * 18, 218, 88);
canvas.drawPath(path, stroke("#38bdf8", 3));

canvas.drawCircle(22 + bob * 180, 118 - bob * 36, 9 + bob * 5, fill("#f8fafc"));
canvas.drawImageRect(image, CK.XYWHRect(0, 0, 1, 1), CK.XYWHRect(20, 18, 64, 40));
"##;

#[derive(Clone, Copy)]
enum CaseKind {
    Slide,
    LightLeak { mask_scale: f32 },
}

impl CaseKind {
    fn name(self) -> &'static str {
        match self {
            Self::Slide => "slide",
            Self::LightLeak { mask_scale } if (mask_scale - 0.5).abs() < f32::EPSILON => {
                "light_leak(scale=0.5)"
            }
            Self::LightLeak { mask_scale } if (mask_scale - 0.25).abs() < f32::EPSILON => {
                "light_leak(scale=0.25)"
            }
            Self::LightLeak { mask_scale } if (mask_scale - 0.125).abs() < f32::EPSILON => {
                "light_leak(scale=0.125)"
            }
            Self::LightLeak { .. } => "light_leak(custom)",
        }
    }
}

fn scene_a(ctx: &FrameCtx) -> Node {
    let orbit = ((ctx.frame as f32 / ctx.fps as f32) * std::f32::consts::PI * 2.0 * 0.25).sin();

    div()
        .id("compare-scene-a")
        .w_full()
        .h_full()
        .bg_slate_900()
        .flex_row()
        .justify_between()
        .items_center()
        .px(56.0)
        .py(48.0)
        .child(
            div()
                .id("compare-scene-a-copy")
                .flex_col()
                .w(470.0)
                .gap(18.0)
                .child(
                    text("Transition Compare A")
                        .id("compare-scene-a-title")
                        .text_px(58.0)
                        .font_bold()
                        .text_white(),
                )
                .child(
                    text("Video + image + canvas overlay. Same scene pair is reused for every transition benchmark.")
                        .id("compare-scene-a-subtitle")
                        .text_px(20.0)
                        .text_slate_300()
                        .leading(1.5),
                ),
        )
        .child(
            div()
                .id("compare-scene-a-stage")
                .relative()
                .w(660.0)
                .h(560.0)
                .child(
                    video(VIDEO_PATH)
                        .id("compare-scene-a-video")
                        .w(660.0)
                        .h(560.0)
                        .cover()
                        .rounded_2xl()
                        .translate_x(orbit * 12.0),
                )
                .child(
                    image()
                        .id("compare-scene-a-image")
                        .path(IMAGE_PATH)
                        .absolute()
                        .right(24.0)
                        .top(24.0)
                        .w(190.0)
                        .h(190.0)
                        .cover()
                        .rounded_xl()
                        .rotate_deg(orbit * 7.0),
                )
                .child(
                    canvas()
                        .id("compare-canvas-a")
                        .asset_path("stage-thumb", IMAGE_PATH)
                        .absolute()
                        .left(24.0)
                        .top(24.0)
                        .w(220.0)
                        .h(140.0)
                        .rounded_xl()
                        .overflow_hidden()
                        .script_source(CANVAS_SCRIPT_A)
                        .expect("canvas script a should compile"),
                ),
        )
        .into()
}

fn scene_b(ctx: &FrameCtx) -> Node {
    let drift = ((ctx.frame as f32 / ctx.fps as f32) * std::f32::consts::PI * 2.0 * 0.18).sin();

    div()
        .id("compare-scene-b")
        .w_full()
        .h_full()
        .bg_slate_100()
        .relative()
        .child(
            image()
                .id("compare-scene-b-background")
                .path(IMAGE_PATH)
                .absolute()
                .left(0.0)
                .top(0.0)
                .w(1280.0)
                .h(720.0)
                .cover()
                .opacity(0.18),
        )
        .child(
            div()
                .id("compare-scene-b-card")
                .absolute()
                .left(72.0)
                .top(72.0)
                .w(620.0)
                .p(28.0)
                .rounded_2xl()
                .bg_white()
                .shadow_xl()
                .border()
                .border_slate_200()
                .translate_x(drift * 20.0)
                .child(
                    text("Transition Compare B")
                        .id("compare-scene-b-title")
                        .text_px(54.0)
                        .font_bold()
                        .text_slate_900(),
                )
                .child(
                    text("The transition changes, the scene payload does not.")
                        .id("compare-scene-b-subtitle")
                        .text_px(22.0)
                        .text_slate_600()
                        .pt(14.0),
                ),
        )
        .child(
            video(VIDEO_PATH)
                .id("compare-scene-b-video")
                .absolute()
                .right(88.0)
                .top(92.0)
                .w(420.0)
                .h(500.0)
                .cover()
                .rounded_2xl()
                .translate_y(drift * -18.0),
        )
        .child(
            canvas()
                .id("compare-canvas-b")
                .asset_path("panel-thumb", IMAGE_PATH)
                .absolute()
                .left(92.0)
                .bottom(70.0)
                .w(240.0)
                .h(160.0)
                .rounded_xl()
                .overflow_hidden()
                .script_source(CANVAS_SCRIPT_B)
                .expect("canvas script b should compile"),
        )
        .into()
}

fn composition_for(case: CaseKind) -> anyhow::Result<Composition> {
    let root = move |ctx: &FrameCtx| -> Node {
        let transition = match case {
            CaseKind::Slide => slide().timing(linear().duration(TRANSITION_FRAMES)),
            CaseKind::LightLeak { mask_scale } => light_leak()
                .seed(3.0)
                .hue_shift(30.0)
                .mask_scale(mask_scale)
                .timing(linear().duration(TRANSITION_FRAMES)),
        };

        timeline()
            .sequence(1, scene_a(ctx))
            .transition(transition)
            .sequence(1, scene_b(ctx))
            .into()
    };

    Composition::new(case.name())
        .size(1280, 720)
        .fps(30)
        .root(root)
        .build()
}

fn run_case(case: CaseKind) -> anyhow::Result<()> {
    let composition = composition_for(case)?;
    let mut session = RenderSession::new();
    let transition_start = 1_u32;
    let transition_end = transition_start + TRANSITION_FRAMES;
    let mut wall_ms = Vec::with_capacity(TRANSITION_FRAMES as usize);

    for frame in transition_start..transition_end {
        let started = Instant::now();
        let _rgba = render_frame_rgba(&composition, frame, &mut session)?;
        wall_ms.push(started.elapsed().as_secs_f64() * 1000.0);
    }

    let avg = wall_ms.iter().sum::<f64>() / wall_ms.len() as f64;
    let mut sorted = wall_ms.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95_index = ((sorted.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    let p95 = sorted[p95_index];

    println!("=== {} ===", case.name());
    println!(
        "wall ms/transition-frame: avg {:.2}, p95 {:.2}, frames {}",
        avg,
        p95,
        wall_ms.len()
    );
    session.print_profile_summary();
    println!();

    Ok(())
}

fn main() -> anyhow::Result<()> {
    for case in [
        CaseKind::Slide,
        CaseKind::LightLeak { mask_scale: 0.5 },
        CaseKind::LightLeak { mask_scale: 0.25 },
        CaseKind::LightLeak { mask_scale: 0.125 },
    ] {
        run_case(case)?;
    }

    Ok(())
}
