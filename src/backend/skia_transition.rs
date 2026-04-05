use std::{cell::RefCell, thread_local};

use anyhow::{Result, anyhow};
use skia_safe::{
    AlphaType, Canvas, ColorType, Data, FilterMode, ImageInfo, Matrix, Paint, Picture, Rect,
    RuntimeEffect, TileMode, runtime_effect::ChildPtr, surfaces,
};

use crate::transitions::{LightLeakTransition, TransitionKind};

const LIGHT_LEAK_MASK_SCALE: f32 = 0.5;

const LIGHT_LEAK_MASK_SKSL: &str = r#"
uniform float evolveProgress;
uniform float retractProgress;
uniform float seed;
uniform float retractSeed;
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
    float glowAlpha =
        smoothstep(0.0, 0.2, leakAlpha) *
        (1.0 - smoothstep(0.65, 1.0, leakAlpha)) *
        0.85;

    return half4(half(patA.x), half(patA.y), half(revealAlpha), half(glowAlpha));
}
"#;

const LIGHT_LEAK_COMPOSITE_SKSL: &str = r#"
uniform shader fromScene;
uniform shader toScene;
uniform shader leakMask;

uniform half3 yellow;
uniform half3 orange;

half4 main(float2 coord) {
    half4 mask = leakMask.eval(coord);

    half brightness = mask.r;
    half blend = mask.g;
    half revealAlpha = mask.b;
    half glowAlpha = mask.a;

    half4 fromColor = fromScene.eval(coord);
    half4 toColor = toScene.eval(coord);
    half4 sceneColor = mix(fromColor, toColor, revealAlpha);

    half3 leakColor = mix(yellow, orange, blend) * (0.6 + 0.6 * brightness);
    half3 finalColor = mix(sceneColor.rgb, leakColor, glowAlpha);

    return half4(finalColor, 1.0);
}
"#;

thread_local! {
    static LIGHT_LEAK_MASK_EFFECT: RefCell<Option<RuntimeEffect>> = const { RefCell::new(None) };
    static LIGHT_LEAK_COMPOSITE_EFFECT: RefCell<Option<RuntimeEffect>> = const { RefCell::new(None) };
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LightLeakMaskUniforms {
    evolve_progress: f32,
    retract_progress: f32,
    seed: f32,
    retract_seed: f32,
    resolution: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LightLeakCompositeUniforms {
    yellow: [f32; 3],
    orange: [f32; 3],
}

impl LightLeakMaskUniforms {
    fn new(progress: f32, params: LightLeakTransition, width: i32, height: i32) -> Self {
        let normalized = progress.clamp(0.0, 1.0);
        Self {
            evolve_progress: (normalized * 2.0).min(1.0),
            retract_progress: (normalized * 2.0 - 1.0).max(0.0),
            seed: params.seed,
            retract_seed: params.retract_seed,
            resolution: [width as f32, height as f32],
        }
    }
}

impl LightLeakCompositeUniforms {
    fn new(params: LightLeakTransition) -> Self {
        Self {
            yellow: rotate_hue([1.0, 0.85, 0.2], params.hue_shift),
            orange: rotate_hue([1.0, 0.5, 0.05], params.hue_shift),
        }
    }
}

pub fn draw_transition(
    canvas: &Canvas,
    from: &Picture,
    to: &Picture,
    progress: f32,
    kind: TransitionKind,
    width: i32,
    height: i32,
) -> Result<()> {
    match kind {
        TransitionKind::Slide => draw_slide_transition(canvas, from, to, progress, width),
        TransitionKind::LightLeak(params) => {
            draw_light_leak_transition(canvas, from, to, progress, params, width, height)
        }
    }
}

fn draw_slide_transition(
    canvas: &Canvas,
    from: &Picture,
    to: &Picture,
    progress: f32,
    width: i32,
) -> Result<()> {
    let progress = progress.clamp(0.0, 1.0);
    let width_f = width as f32;

    canvas.save();
    canvas.translate(((progress - 1.0) * width_f, 0.0));
    canvas.draw_picture(to, None, None);
    canvas.restore();

    canvas.save();
    canvas.translate((progress * width_f, 0.0));
    canvas.draw_picture(from, None, None);
    canvas.restore();

    Ok(())
}

fn draw_light_leak_transition(
    canvas: &Canvas,
    from: &Picture,
    to: &Picture,
    progress: f32,
    params: LightLeakTransition,
    width: i32,
    height: i32,
) -> Result<()> {
    let mask_size = scaled_mask_size(width, height);
    let mask_image = render_light_leak_mask(progress, params, mask_size.0, mask_size.1)?;

    let from_shader = from.to_shader(
        None,
        FilterMode::Linear,
        Option::<&Matrix>::None,
        Option::<&Rect>::None,
    );
    let to_shader = to.to_shader(
        None,
        FilterMode::Linear,
        Option::<&Matrix>::None,
        Option::<&Rect>::None,
    );

    let scale_matrix = Matrix::scale((
        width as f32 / mask_size.0 as f32,
        height as f32 / mask_size.1 as f32,
    ));
    let mask_shader = mask_image
        .to_shader(
            Some((TileMode::Clamp, TileMode::Clamp)),
            FilterMode::Linear,
            Some(&scale_matrix),
        )
        .ok_or_else(|| anyhow!("failed to create light leak mask shader"))?;

    let uniforms = LightLeakCompositeUniforms::new(params);
    let children = [
        ChildPtr::from(from_shader),
        ChildPtr::from(to_shader),
        ChildPtr::from(mask_shader),
    ];
    let shader = light_leak_composite_effect()?
        .make_shader(uniform_data(&uniforms), &children, Option::<&Matrix>::None)
        .ok_or_else(|| anyhow!("failed to create light leak composite shader"))?;

    let mut paint = Paint::default();
    paint.set_shader(shader);
    canvas.draw_paint(&paint);
    Ok(())
}

fn render_light_leak_mask(
    progress: f32,
    params: LightLeakTransition,
    width: i32,
    height: i32,
) -> Result<skia_safe::Image> {
    let info = ImageInfo::new(
        (width, height),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let mut surface = surfaces::raster(&info, None, None)
        .ok_or_else(|| anyhow!("failed to create light leak mask surface"))?;
    let uniforms = LightLeakMaskUniforms::new(progress, params, width, height);
    let shader = light_leak_mask_effect()?
        .make_shader(uniform_data(&uniforms), &[], Option::<&Matrix>::None)
        .ok_or_else(|| anyhow!("failed to create light leak mask shader"))?;

    let mut paint = Paint::default();
    paint.set_shader(shader);
    surface.canvas().draw_paint(&paint);
    Ok(surface.image_snapshot())
}

fn scaled_mask_size(width: i32, height: i32) -> (i32, i32) {
    let scaled_width = ((width as f32) * LIGHT_LEAK_MASK_SCALE).round() as i32;
    let scaled_height = ((height as f32) * LIGHT_LEAK_MASK_SCALE).round() as i32;
    (scaled_width.max(1), scaled_height.max(1))
}

fn light_leak_mask_effect() -> Result<RuntimeEffect> {
    cached_effect(
        &LIGHT_LEAK_MASK_EFFECT,
        LIGHT_LEAK_MASK_SKSL,
        "failed to compile light leak mask SKSL",
    )
}

fn light_leak_composite_effect() -> Result<RuntimeEffect> {
    cached_effect(
        &LIGHT_LEAK_COMPOSITE_EFFECT,
        LIGHT_LEAK_COMPOSITE_SKSL,
        "failed to compile light leak composite SKSL",
    )
}

fn cached_effect(
    slot: &'static std::thread::LocalKey<RefCell<Option<RuntimeEffect>>>,
    source: &str,
    error_context: &str,
) -> Result<RuntimeEffect> {
    slot.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(effect) = slot.as_ref() {
            return Ok(effect.clone());
        }

        let effect = RuntimeEffect::make_for_shader(source, None)
            .map_err(|err| anyhow!("{error_context}: {err}"))?;
        *slot = Some(effect.clone());
        Ok(effect)
    })
}

fn uniform_data<T>(value: &T) -> Data {
    let size = std::mem::size_of::<T>();
    let bytes = unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), size) };
    Data::new_copy(bytes)
}

fn rotate_hue(color: [f32; 3], degrees: f32) -> [f32; 3] {
    let radians = degrees.to_radians();
    let cos_a = radians.cos();
    let sin_a = radians.sin();
    let sqrt_third = 0.57735_f32;
    let matrix = [
        [
            cos_a + (1.0 - cos_a) / 3.0,
            (1.0 - cos_a) / 3.0 - sin_a * sqrt_third,
            (1.0 - cos_a) / 3.0 + sin_a * sqrt_third,
        ],
        [
            (1.0 - cos_a) / 3.0 + sin_a * sqrt_third,
            cos_a + (1.0 - cos_a) / 3.0,
            (1.0 - cos_a) / 3.0 - sin_a * sqrt_third,
        ],
        [
            (1.0 - cos_a) / 3.0 - sin_a * sqrt_third,
            (1.0 - cos_a) / 3.0 + sin_a * sqrt_third,
            cos_a + (1.0 - cos_a) / 3.0,
        ],
    ];

    [
        dot3(matrix[0], color).clamp(0.0, 1.0),
        dot3(matrix[1], color).clamp(0.0, 1.0),
        dot3(matrix[2], color).clamp(0.0, 1.0),
    ]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
