use anyhow::{anyhow, Result};
use skia_safe::{
    runtime_effect::ChildPtr, Canvas, Data, FilterMode, Matrix, Paint, Picture, PictureRecorder,
    Rect, RuntimeEffect,
};

use crate::{
    assets::AssetsMap,
    backend::skia::SkiaBackend,
    display::list::{DisplayList, DisplayTransitionCommand},
    frame_ctx::FrameCtx,
    media::MediaContext,
    transitions::{LightLeakTransition, TransitionKind},
};

const LIGHT_LEAK_SKSL: &str = r#"
uniform shader fromScene;
uniform shader toScene;

uniform float evolveProgress;
uniform float retractProgress;
uniform float seed;
uniform float retractSeed;
uniform float hueShift;
uniform float2 resolution;

const float PI = 3.14159265;

float3 computePattern(float2 uv, float s, float t) {
    float2 p = uv * 0.8;
    p += float2(sin(s * 1.61803) * 5.0, cos(s * 2.71828) * 5.0);

    for (int i = 1; i < 5; i++) {
        float fi = float(i);
        float phase = s * 0.7 * fi;
        float2 nextP = p;
        nextP.x += 0.6 / fi * cos(fi * p.y + t * 0.7 + 0.3 * fi + phase) + 20.0;
        nextP.y += 0.6 / fi * cos(fi * p.x + t * 0.7 + 0.3 * float(i + 10) + phase) - 5.0;
        p = nextP;
    }

    float v1 = 0.5 * sin(2.0 * p.x) + 0.5;
    float v2 = 0.5 * sin(2.0 * p.y) + 0.5;
    float blend = sin(p.x + p.y) * 0.5 + 0.5;
    float brightness = v1 * 0.5 + v2 * 0.5;
    float patternValue = brightness * 0.6 + blend * 0.4;

    return float3(brightness, blend, patternValue);
}

float3 rotateHue(float3 color, float degrees) {
    float angle = degrees * PI / 180.0;
    float cosA = cos(angle);
    float sinA = sin(angle);

    float3x3 hueRot = float3x3(
        cosA + (1.0 - cosA) / 3.0,
        (1.0 - cosA) / 3.0 - sinA * 0.57735,
        (1.0 - cosA) / 3.0 + sinA * 0.57735,
        (1.0 - cosA) / 3.0 + sinA * 0.57735,
        cosA + (1.0 - cosA) / 3.0,
        (1.0 - cosA) / 3.0 - sinA * 0.57735,
        (1.0 - cosA) / 3.0 - sinA * 0.57735,
        (1.0 - cosA) / 3.0 + sinA * 0.57735,
        cosA + (1.0 - cosA) / 3.0
    );

    return clamp(hueRot * color, 0.0, 1.0);
}

half4 main(float2 coord) {
    float refScale = 1.92;
    float2 uv = (coord / resolution) *
        float2(refScale, refScale * resolution.y / resolution.x);

    float3 patA = computePattern(uv, seed, evolveProgress * PI);
    float threshA = 1.0 - evolveProgress;
    float revealAlpha = smoothstep(threshA, threshA + 0.3, patA.z);

    float2 maxUv = float2(refScale, refScale * resolution.y / resolution.x);
    float2 retractUv = maxUv - uv;
    float3 patB = computePattern(retractUv, retractSeed, retractProgress * PI);
    float threshB = 1.0 - retractProgress;
    float eraseAlpha = smoothstep(threshB, threshB + 0.3, patB.z);

    float leakAlpha = clamp(revealAlpha * (1.0 - eraseAlpha), 0.0, 1.0);

    float3 yellow = float3(1.0, 0.85, 0.2);
    float3 orange = float3(1.0, 0.5, 0.05);
    float3 leakColor = mix(yellow, orange, patA.y);
    leakColor *= 0.6 + 0.6 * patA.x;
    leakColor = rotateHue(leakColor, hueShift);

    half4 fromColor = fromScene.eval(coord);
    half4 toColor = toScene.eval(coord);
    half4 sceneColor = mix(fromColor, toColor, half(revealAlpha));

    float glowWindow =
        smoothstep(0.0, 0.2, leakAlpha) *
        (1.0 - smoothstep(0.65, 1.0, leakAlpha));
    half3 finalColor = mix(sceneColor.rgb, half3(leakColor), half(glowWindow * 0.85));

    return half4(finalColor, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy)]
struct LightLeakUniforms {
    evolve_progress: f32,
    retract_progress: f32,
    seed: f32,
    retract_seed: f32,
    hue_shift: f32,
    resolution: [f32; 2],
}

impl LightLeakUniforms {
    fn new(progress: f32, params: LightLeakTransition, width: i32, height: i32) -> Self {
        let normalized = progress.clamp(0.0, 1.0);
        Self {
            evolve_progress: (normalized * 2.0).min(1.0),
            retract_progress: (normalized * 2.0 - 1.0).max(0.0),
            seed: params.seed,
            retract_seed: params.retract_seed,
            hue_shift: params.hue_shift,
            resolution: [width as f32, height as f32],
        }
    }
}

pub fn draw_transition<'a>(
    canvas: &Canvas,
    transition: &DisplayTransitionCommand,
    width: i32,
    height: i32,
    assets: &'a AssetsMap,
    media_ctx: &mut Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
) -> Result<()> {
    match transition.kind {
        TransitionKind::Slide => draw_slide_transition(
            canvas, transition, width, height, assets, media_ctx, frame_ctx,
        ),
        TransitionKind::LightLeak(params) => draw_light_leak_transition(
            canvas, transition, params, width, height, assets, media_ctx, frame_ctx,
        ),
    }
}

fn draw_slide_transition<'a>(
    canvas: &Canvas,
    transition: &DisplayTransitionCommand,
    width: i32,
    height: i32,
    assets: &'a AssetsMap,
    media_ctx: &mut Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
) -> Result<()> {
    let from_picture = record_picture(
        &transition.from,
        width,
        height,
        assets,
        media_ctx,
        frame_ctx,
    )?;
    let to_picture = record_picture(&transition.to, width, height, assets, media_ctx, frame_ctx)?;
    let progress = transition.progress.clamp(0.0, 1.0);
    let width_f = width as f32;

    canvas.save();
    canvas.translate(((progress - 1.0) * width_f, 0.0));
    canvas.draw_picture(&to_picture, None, None);
    canvas.restore();

    canvas.save();
    canvas.translate((progress * width_f, 0.0));
    canvas.draw_picture(&from_picture, None, None);
    canvas.restore();

    Ok(())
}

fn draw_light_leak_transition<'a>(
    canvas: &Canvas,
    transition: &DisplayTransitionCommand,
    params: LightLeakTransition,
    width: i32,
    height: i32,
    assets: &'a AssetsMap,
    media_ctx: &mut Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
) -> Result<()> {
    let from_shader = record_picture(
        &transition.from,
        width,
        height,
        assets,
        media_ctx,
        frame_ctx,
    )?
    .to_shader(
        None,
        FilterMode::Linear,
        Option::<&Matrix>::None,
        Option::<&Rect>::None,
    );
    let to_shader = record_picture(&transition.to, width, height, assets, media_ctx, frame_ctx)?
        .to_shader(
            None,
            FilterMode::Linear,
            Option::<&Matrix>::None,
            Option::<&Rect>::None,
        );

    let effect = RuntimeEffect::make_for_shader(LIGHT_LEAK_SKSL, None)
        .map_err(|err| anyhow!("failed to compile light leak SKSL: {err}"))?;

    let uniforms = LightLeakUniforms::new(transition.progress, params, width, height);
    let children = [ChildPtr::from(from_shader), ChildPtr::from(to_shader)];
    let shader = effect
        .make_shader(uniform_data(&uniforms), &children, Option::<&Matrix>::None)
        .ok_or_else(|| anyhow!("failed to create light leak shader"))?;

    let mut paint = Paint::default();
    paint.set_shader(shader);
    canvas.draw_paint(&paint);
    Ok(())
}

fn record_picture<'a>(
    list: &DisplayList,
    width: i32,
    height: i32,
    assets: &'a AssetsMap,
    _media_ctx: &mut Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
) -> Result<Picture> {
    let bounds = Rect::from_xywh(0.0, 0.0, width as f32, height as f32);
    let mut recorder = PictureRecorder::new();
    let recording_canvas = recorder.begin_recording(bounds, false);
    let mut backend = SkiaBackend::new(recording_canvas, width, height, assets, None, frame_ctx);
    backend.execute(list)?;
    recorder
        .finish_recording_as_picture(None)
        .ok_or_else(|| anyhow!("failed to record transition picture"))
}

fn uniform_data<T>(value: &T) -> Data {
    let size = std::mem::size_of::<T>();
    let bytes = unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), size) };
    Data::new_copy(bytes)
}
