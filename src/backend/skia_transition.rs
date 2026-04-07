use std::{cell::RefCell, thread_local, time::Instant};

use anyhow::{Result, anyhow};
use skia_safe::{
    AlphaType, Canvas, ColorType, Data, FilterMode, ImageInfo, Matrix, Paint, PathBuilder, Picture,
    RRect, Rect, RuntimeEffect, TileMode, runtime_effect::ChildPtr, surfaces,
};

use crate::{
    scene_snapshot::SceneSnapshot,
    transitions::{LightLeakTransition, SlideDirection, TransitionKind, WipeDirection},
};

const LIGHT_LEAK_MASK_SKSL: &str = r#"
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

half4 main(float2 coord) {
    float refScale = 1.92;
    float2 uv = (coord / resolution) *
        float2(refScale, refScale * resolution.y / resolution.x);

    float3 patA = computePattern(uv, seed, evolveProgress * PI);
    float threshA = 1.0 - evolveProgress;
    float revealAlpha = smoothstep(threshA, threshA + 0.3, patA.z);

    float2 maxUv = float2(refScale, refScale * resolution.y / resolution.x);
    float2 retractUv = maxUv - uv;
    float3 patB = computePattern(retractUv, seed + 42.0, retractProgress * PI);
    float threshB = 1.0 - retractProgress;
    float eraseAlpha = smoothstep(threshB, threshB + 0.3, patB.z);

    float alpha = revealAlpha * (1.0 - eraseAlpha);

    float3 yellow = float3(1.0, 0.85, 0.2);
    float3 orange = float3(1.0, 0.5, 0.05);
    float3 col = mix(yellow, orange, patA.y);
    col *= 0.6 + 0.6 * patA.x;

    float angle = hueShift * PI / 180.0;
    float cosA = cos(angle);
    float sinA = sin(angle);
    mat3 hueRot = mat3(
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
    col = clamp(hueRot * col, 0.0, 1.0);

    return half4(col.x, col.y, col.z, alpha);
}
"#;

const LIGHT_LEAK_COMPOSITE_SKSL: &str = r#"
uniform shader fromScene;
uniform shader toScene;
uniform shader leakMask;
uniform float progress;

half4 main(float2 coord) {
    half4 mask = leakMask.eval(coord);
    half4 fromColor = fromScene.eval(coord);
    half4 toColor = toScene.eval(coord);
    half alpha = mask.a;
    half4 sceneColor = mix(fromColor, toColor, half(progress));
    half3 leakColor = mask.rgb;
    half3 finalColor = mix(sceneColor.rgb, leakColor, alpha);

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
    hue_shift: f32,
    resolution: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LightLeakCompositeUniforms {
    progress: f32,
}

impl LightLeakMaskUniforms {
    fn new(progress: f32, params: LightLeakTransition, width: i32, height: i32) -> Self {
        let normalized = progress.clamp(0.0, 1.0);
        Self {
            evolve_progress: (normalized * 2.0).min(1.0),
            retract_progress: (normalized * 2.0 - 1.0).max(0.0),
            seed: params.seed,
            retract_seed: params.seed + 42.0,
            hue_shift: params.hue_shift,
            resolution: [width as f32, height as f32],
        }
    }
}

impl LightLeakCompositeUniforms {
    fn new(progress: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
        }
    }
}

pub fn draw_transition(
    canvas: &Canvas,
    from: &SceneSnapshot,
    to: &SceneSnapshot,
    progress: f32,
    kind: TransitionKind,
    width: i32,
    height: i32,
    mut profile: Option<&mut crate::profile::BackendProfile>,
) -> Result<()> {
    let from = from.picture()?;
    let to = to.picture()?;
    match kind {
        TransitionKind::Slide(direction) => {
            draw_slide_transition(canvas, from, to, progress, direction, width, height)
        }
        TransitionKind::LightLeak(params) => draw_light_leak_transition(
            canvas,
            from,
            to,
            progress,
            params,
            width,
            height,
            profile.as_deref_mut(),
        ),
        TransitionKind::Fade => draw_fade_transition(canvas, from, to, progress),
        TransitionKind::Wipe(direction) => {
            draw_wipe_transition(canvas, from, to, progress, direction, width, height)
        }
        TransitionKind::ClockWipe => {
            draw_clock_wipe_transition(canvas, from, to, progress, width, height)
        }
        TransitionKind::Iris => draw_iris_transition(canvas, from, to, progress, width, height),
    }
}

fn draw_slide_transition(
    canvas: &Canvas,
    from: &Picture,
    to: &Picture,
    progress: f32,
    direction: SlideDirection,
    width: i32,
    height: i32,
) -> Result<()> {
    let progress = progress.clamp(0.0, 1.0);
    let w = width as f32;
    let h = height as f32;

    let to_offset = match direction {
        SlideDirection::FromLeft => (w * (progress - 1.0), 0.0),
        SlideDirection::FromRight => (-w * progress, 0.0),
        SlideDirection::FromTop => (0.0, h * (progress - 1.0)),
        SlideDirection::FromBottom => (0.0, -h * progress),
    };

    let from_offset = match direction {
        SlideDirection::FromLeft => (w * progress, 0.0),
        SlideDirection::FromRight => (w * (1.0 - progress), 0.0),
        SlideDirection::FromTop => (0.0, h * progress),
        SlideDirection::FromBottom => (0.0, h * (1.0 - progress)),
    };

    canvas.save();
    canvas.translate(to_offset);
    canvas.draw_picture(to, None, None);
    canvas.restore();

    canvas.save();
    canvas.translate(from_offset);
    canvas.draw_picture(from, None, None);
    canvas.restore();

    Ok(())
}

fn draw_fade_transition(
    canvas: &Canvas,
    from: &Picture,
    to: &Picture,
    progress: f32,
) -> Result<()> {
    let progress = progress.clamp(0.0, 1.0);

    let mut from_paint = Paint::default();
    from_paint.set_alpha(((1.0 - progress) * 255.0).round() as u8);
    canvas.draw_picture(from, None, Some(&from_paint));

    let mut to_paint = Paint::default();
    to_paint.set_alpha((progress * 255.0).round() as u8);
    canvas.draw_picture(to, None, Some(&to_paint));

    Ok(())
}

fn draw_wipe_transition(
    canvas: &Canvas,
    from: &Picture,
    to: &Picture,
    progress: f32,
    direction: WipeDirection,
    width: i32,
    height: i32,
) -> Result<()> {
    let progress = progress.clamp(0.0, 1.0);
    let w = width as f32;
    let h = height as f32;

    canvas.draw_picture(from, None, None);

    let clip_rect = match direction {
        WipeDirection::FromLeft => Rect::from_point_and_size((0.0, 0.0), (w * progress, h)),
        WipeDirection::FromRight => {
            Rect::from_point_and_size((w * (1.0 - progress), 0.0), (w * progress, h))
        }
        WipeDirection::FromTop => Rect::from_point_and_size((0.0, 0.0), (w, h * progress)),
        WipeDirection::FromBottom => {
            Rect::from_point_and_size((0.0, h * (1.0 - progress)), (w, h * progress))
        }
        WipeDirection::FromTopLeft => {
            Rect::from_point_and_size((0.0, 0.0), (w * progress, h * progress))
        }
        WipeDirection::FromTopRight => {
            Rect::from_point_and_size((w * (1.0 - progress), 0.0), (w * progress, h * progress))
        }
        WipeDirection::FromBottomLeft => {
            Rect::from_point_and_size((0.0, h * (1.0 - progress)), (w * progress, h * progress))
        }
        WipeDirection::FromBottomRight => Rect::from_point_and_size(
            (w * (1.0 - progress), h * (1.0 - progress)),
            (w * progress, h * progress),
        ),
    };

    canvas.save();
    canvas.clip_rect(clip_rect, None, Some(true));
    canvas.draw_picture(to, None, None);
    canvas.restore();

    Ok(())
}

fn draw_clock_wipe_transition(
    canvas: &Canvas,
    from: &Picture,
    to: &Picture,
    progress: f32,
    width: i32,
    height: i32,
) -> Result<()> {
    let progress = progress.clamp(0.0, 1.0);

    canvas.draw_picture(from, None, None);

    if progress <= 0.0 {
        return Ok(());
    }

    let w = width as f32;
    let h = height as f32;
    let cx = w / 2.0;
    let cy = h / 2.0;
    let radius = (cx * cx + cy * cy).sqrt();

    let start_angle_deg: f32 = -90.0;
    let sweep_angle_deg: f32 = progress * 360.0;

    let mut builder = PathBuilder::new();
    builder.move_to((cx, cy));

    let start_rad = start_angle_deg.to_radians();
    builder.line_to((cx + radius * start_rad.cos(), cy + radius * start_rad.sin()));

    let arc_rect =
        Rect::from_point_and_size((cx - radius, cy - radius), (radius * 2.0, radius * 2.0));
    builder.arc_to(arc_rect, start_angle_deg, sweep_angle_deg, false);
    builder.close();

    let path = builder.detach();

    canvas.save();
    canvas.clip_path(&path, None, Some(true));
    canvas.draw_picture(to, None, None);
    canvas.restore();

    Ok(())
}

fn draw_iris_transition(
    canvas: &Canvas,
    from: &Picture,
    to: &Picture,
    progress: f32,
    width: i32,
    height: i32,
) -> Result<()> {
    let progress = progress.clamp(0.0, 1.0);

    canvas.draw_picture(from, None, None);

    if progress <= 0.0 {
        return Ok(());
    }

    let w = width as f32;
    let h = height as f32;
    let cx = w / 2.0;
    let cy = h / 2.0;
    let max_radius = (cx * cx + cy * cy).sqrt();
    let radius = progress * max_radius;

    let oval_rect =
        Rect::from_point_and_size((cx - radius, cy - radius), (radius * 2.0, radius * 2.0));
    let oval = RRect::new_oval(oval_rect);

    canvas.save();
    canvas.clip_rrect(&oval, None, Some(true));
    canvas.draw_picture(to, None, None);
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
    mut profile: Option<&mut crate::profile::BackendProfile>,
) -> Result<()> {
    let mask_scale = params.mask_scale.clamp(0.03125, 1.0);
    let mask_size = scaled_mask_size(width, height, mask_scale);
    let mask_started = Instant::now();
    let mask_image = render_light_leak_mask(progress, params, mask_size.0, mask_size.1)?;
    if let Some(profile) = profile.as_deref_mut() {
        profile.light_leak_mask_ms += mask_started.elapsed().as_secs_f64() * 1000.0;
    }

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

    let scale_matrix = Matrix::scale((1.0 / mask_scale, 1.0 / mask_scale));
    let mask_shader = mask_image
        .to_shader(
            Some((TileMode::Clamp, TileMode::Clamp)),
            FilterMode::Linear,
            Some(&scale_matrix),
        )
        .ok_or_else(|| anyhow!("failed to create light leak mask shader"))?;

    let uniforms = LightLeakCompositeUniforms::new(progress);
    let children = [
        ChildPtr::from(from_shader),
        ChildPtr::from(to_shader),
        ChildPtr::from(mask_shader),
    ];
    let shader = light_leak_composite_effect()?
        .make_shader(uniform_data(&uniforms), &children, Option::<&Matrix>::None)
        .ok_or_else(|| anyhow!("failed to create light leak composite shader"))?;

    let composite_started = Instant::now();
    let mut paint = Paint::default();
    paint.set_shader(shader);
    canvas.draw_paint(&paint);
    if let Some(profile) = profile.as_deref_mut() {
        profile.light_leak_composite_ms += composite_started.elapsed().as_secs_f64() * 1000.0;
    }
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

fn scaled_mask_size(width: i32, height: i32, mask_scale: f32) -> (i32, i32) {
    let scaled_width = ((width as f32) * mask_scale).round() as i32;
    let scaled_height = ((height as f32) * mask_scale).round() as i32;
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
