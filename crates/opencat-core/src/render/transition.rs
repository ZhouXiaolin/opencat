use crate::canvas::paint::{
    BlendMode, FillSpec, PaintSpec, PaintStyle,
};
use crate::display::list::{
    DisplayRect, TimelineDisplayItem, TimelineTransitionDisplay,
};
use crate::ir::draw_op::{
    DrawOp, Rect4,
};
use crate::ir::draw_types::{
    ChildRange, DrawOpRange, PathOp, RuntimeEffectChildRef,
};
use crate::parse::gl_transition;
use crate::parse::transition::{
    GlTransition, LightLeakTransition, SlideDirection, TransitionKind, WipeDirection,
};
use crate::render::builder::DrawOpBuilder;

use super::RenderError;
use super::ctx::RenderCtx;


use super::items::{kurbo_rect, rect_to_rect4};

fn render_transition_overlay(
    builder: &mut DrawOpBuilder,
    bounds: DisplayRect,
    transition: &TimelineTransitionDisplay,
) {
    let bounded_rect4 = rect_to_rect4(kurbo_rect(bounds));
    let p = transition.progress.clamp(0.0, 1.0);

    match &transition.kind {
        TransitionKind::Fade => {
            // Alpha blends into the SaveLayer created in render_timeline
        }
        TransitionKind::Cut => {
            // No effect: the incoming scene is already drawn over the outgoing
            // one by the compositor.
        }
        TransitionKind::Slide(dir) => {
            let (dx, dy) = slide_offset(dir, bounds, 1.0 - p);
            builder.push(DrawOp::Translate { x: dx, y: dy });
        }
        TransitionKind::Wipe(dir) => {
            let clip_rect4 = wipe_clip_rect(dir, bounds, p);
            builder.push(DrawOp::BeginPath);
            builder.push(DrawOp::Path(PathOp::AddRect {
                x: clip_rect4.x,
                y: clip_rect4.y,
                width: clip_rect4.width,
                height: clip_rect4.height,
            }));
            builder.push(DrawOp::ClipPath { anti_alias: false, fill_type: Default::default() });
        }
        TransitionKind::ClockWipe => {
            let sweep = (1.0 - p) * 360.0;
            if sweep > 0.0 {
                let overlay = PaintSpec {
                    fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
                    style: PaintStyle::Fill,
                    stroke: None,
                    anti_alias: true,
                    blend_mode: BlendMode::SrcOver,
                    image_filter: None,
                    color_filter: None,
                    mask_filter: None,
                    path_effect: None,
                };
                let paint_id = builder.intern_paint(overlay);
                builder.push(DrawOp::Save);
                builder.push(DrawOp::BeginPath);
                builder.push(DrawOp::Path(PathOp::AddRect {
                    x: bounded_rect4.x,
                    y: bounded_rect4.y,
                    width: bounded_rect4.width,
                    height: bounded_rect4.height,
                }));
                builder.push(DrawOp::ClipPath { anti_alias: false, fill_type: Default::default() });
                builder.push(DrawOp::Arc {
                    rect: bounded_rect4,
                    start: -90.0,
                    sweep,
                    use_center: true,
                    paint: paint_id,
                });
                builder.push(DrawOp::Restore);
            }
        }
        TransitionKind::Iris => {
            let cx = bounded_rect4.x + bounded_rect4.width / 2.0;
            let cy = bounded_rect4.y + bounded_rect4.height / 2.0;
            let scale = p.max(0.001);
            builder.push(DrawOp::Translate { x: cx, y: cy });
            builder.push(DrawOp::Scale { x: scale, y: scale });
            builder.push(DrawOp::Translate { x: -cx, y: -cy });
        }
        TransitionKind::LightLeak(leak) => {
            let r = sinusoid_noise(p, leak.seed);
            let g = sinusoid_noise(p, leak.seed + 1.0);
            let b = sinusoid_noise(p, leak.seed + 2.0);
            let alpha = (1.0 - p) * 0.3 * leak.mask_scale;
            let paint = PaintSpec {
                fill: FillSpec::Solid([r, g, b, alpha]),
                style: PaintStyle::Fill,
                stroke: None,
                anti_alias: false,
                blend_mode: BlendMode::SrcOver,
                image_filter: None,
                color_filter: None,
                mask_filter: None,
                path_effect: None,
            };
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::Rect {
                rect: bounded_rect4,
                paint: paint_id,
            });
        }
        TransitionKind::Gl(gl) => {
            log::warn!("GL transition '{}' not supported in render layer", gl.name);
        }
    }
}

fn slide_offset(dir: &SlideDirection, bounds: DisplayRect, amount: f32) -> (f32, f32) {
    match dir {
        SlideDirection::FromLeft => (bounds.width * -amount, 0.0),
        SlideDirection::FromRight => (bounds.width * amount, 0.0),
        SlideDirection::FromTop => (0.0, bounds.height * -amount),
        SlideDirection::FromBottom => (0.0, bounds.height * amount),
    }
}

fn wipe_clip_rect(dir: &WipeDirection, bounds: DisplayRect, progress: f32) -> Rect4 {
    let p = progress;
    let x = bounds.x;
    let y = bounds.y;
    let w = bounds.width;
    let h = bounds.height;
    match dir {
        WipeDirection::FromLeft => Rect4 {
            x,
            y,
            width: w * p,
            height: h,
        },
        WipeDirection::FromRight => Rect4 {
            x: x + w * (1.0 - p),
            y,
            width: w * p,
            height: h,
        },
        WipeDirection::FromTop => Rect4 {
            x,
            y,
            width: w,
            height: h * p,
        },
        WipeDirection::FromBottom => Rect4 {
            x,
            y: y + h * (1.0 - p),
            width: w,
            height: h * p,
        },
        WipeDirection::FromTopLeft => Rect4 {
            x,
            y,
            width: w * p,
            height: h * p,
        },
        WipeDirection::FromTopRight => Rect4 {
            x: x + w * (1.0 - p),
            y,
            width: w * p,
            height: h * p,
        },
        WipeDirection::FromBottomLeft => Rect4 {
            x,
            y: y + h * (1.0 - p),
            width: w * p,
            height: h * p,
        },
        WipeDirection::FromBottomRight => Rect4 {
            x: x + w * (1.0 - p),
            y: y + h * (1.0 - p),
            width: w * p,
            height: h * p,
        },
    }
}

fn sinusoid_noise(x: f32, seed: f32) -> f32 {
    ((x * std::f32::consts::TAU + seed * 12.9898).sin() * 43_758.547).fract()
}

pub fn render_timeline(ctx: &mut RenderCtx, item: &TimelineDisplayItem) -> Result<(), RenderError> {
    let rect_item = crate::display::list::RectDisplayItem {
        bounds: item.bounds,
        paint: item.paint.clone(),
    };

    if let Some(ref transition) = item.transition {
        let bounds = item.bounds;
        let rect4 = rect_to_rect4(kurbo_rect(bounds));

        // Solid white backdrop for transition compositing
        let paint = PaintSpec {
            fill: FillSpec::Solid([1.0; 4]),
            style: PaintStyle::Fill,
            stroke: None,
            anti_alias: true,
            blend_mode: BlendMode::SrcOver,
            image_filter: None,
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        };
        let paint_id = ctx.builder.intern_paint(paint);

        let layer_alpha = match &transition.kind {
            TransitionKind::Fade => transition.progress.clamp(0.0, 1.0),
            _ => 1.0,
        };

        ctx.builder.push(DrawOp::Save);
        ctx.builder.push(DrawOp::BeginPath);
        ctx.builder.push(DrawOp::Path(PathOp::AddRect {
            x: rect4.x,
            y: rect4.y,
            width: rect4.width,
            height: rect4.height,
        }));
        ctx.builder.push(DrawOp::ClipPath { anti_alias: false, fill_type: Default::default() });

        ctx.builder.push(DrawOp::SaveLayer {
            bounds: Some(rect4),
            paint: Some(paint_id),
            alpha: layer_alpha,
            backdrop: false,
        });

        render_transition_overlay(ctx.builder, bounds, transition);
        super::render_rect_with_shadows(ctx, &rect_item)?;

        ctx.builder.push(DrawOp::Restore);
        ctx.builder.push(DrawOp::Restore);
    } else {
        super::render_rect_with_shadows(ctx, &rect_item)?;
    }

    Ok(())
}

// ── Transition ───────────────────────────────────────────────────────

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

const MASK_EFFECT_KEY: u64 = 0xAA01_0001;
const COMPOSITE_EFFECT_KEY: u64 = 0xAA01_0002;

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

fn as_bytes<T: Copy>(val: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts(val as *const T as *const u8, std::mem::size_of::<T>()) }
}

pub(crate) fn render_light_leak_transition(
    ctx: &mut RenderCtx,
    from_range: DrawOpRange,
    to_range: DrawOpRange,
    progress: f32,
    params: &LightLeakTransition,
    bounds: DisplayRect,
) {
    let builder = &mut ctx.builder;
    let w = bounds.width.max(1.0).round() as u32;
    let h = bounds.height.max(1.0).round() as u32;

    let mask_effect_id = builder.intern_effect(MASK_EFFECT_KEY, LIGHT_LEAK_MASK_SKSL);
    let composite_effect_id =
        builder.intern_effect(COMPOSITE_EFFECT_KEY, LIGHT_LEAK_COMPOSITE_SKSL);

    let normalized = progress.clamp(0.0, 1.0);
    let mask_uniforms = LightLeakMaskUniforms {
        evolve_progress: (normalized * 2.0).min(1.0),
        retract_progress: (normalized * 2.0 - 1.0).max(0.0),
        seed: params.seed,
        retract_seed: params.seed + 42.0,
        hue_shift: params.hue_shift,
        resolution: [w as f32, h as f32],
    };
    let mask_uniforms_range = builder.intern_bytes(as_bytes(&mask_uniforms));

    let mask_dst = Rect4 {
        x: 0.0,
        y: 0.0,
        width: w as f32,
        height: h as f32,
    };

    {
        #[cfg(feature = "profile")]
        let _mask_span = span!(target: "render.backend", Level::TRACE, "light_leak_mask").entered();

        let mask_marker = builder.begin_range();
        builder.push(DrawOp::RuntimeEffect {
            effect: mask_effect_id,
            uniforms: mask_uniforms_range,
            children: ChildRange { start: 0, len: 0 },
            dst: mask_dst,
        });
        let mask_range = builder.end_range(mask_marker);

        let composite_uniforms = LightLeakCompositeUniforms {
            progress: normalized,
        };
        let composite_uniforms_range = builder.intern_bytes(as_bytes(&composite_uniforms));

        let child_start = builder.push_child(RuntimeEffectChildRef::Picture(from_range));
        builder.push_child(RuntimeEffectChildRef::Picture(to_range));
        builder.push_child(RuntimeEffectChildRef::Picture(mask_range));

        let dst_rect4 = Rect4 {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        };

        #[cfg(feature = "profile")]
        let _composite_span =
            span!(target: "render.backend", Level::TRACE, "light_leak_composite").entered();

        builder.push(DrawOp::RuntimeEffect {
            effect: composite_effect_id,
            uniforms: composite_uniforms_range,
            children: ChildRange {
                start: child_start,
                len: 3,
            },
            dst: dst_rect4,
        });
    }
}

// ── GL Transition ──────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
struct GlTransitionUniforms {
    progress: f32,
    resolution: [f32; 2],
}

pub(crate) fn render_gl_transition(
    ctx: &mut RenderCtx,
    from_range: DrawOpRange,
    to_range: DrawOpRange,
    progress: f32,
    effect: &GlTransition,
    bounds: DisplayRect,
) {
    let builder = &mut ctx.builder;
    let w = bounds.width.max(1.0).round() as u32;
    let h = bounds.height.max(1.0).round() as u32;

    let dst_rect4 = Rect4 {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
    };

    let sksl = effect
        .sksl
        .as_deref()
        .map(String::from)
        .or_else(|| gl_transition::gl_transition_sksl(&effect.name).ok());
    let sksl = match sksl {
        Some(s) => s,
        None => {
            builder.push(DrawOp::ReplayRange { range: from_range });
            builder.push(DrawOp::SaveLayer {
                bounds: Some(dst_rect4),
                paint: None,
                alpha: progress,
                backdrop: false,
            });
            builder.push(DrawOp::ReplayRange { range: to_range });
            builder.push(DrawOp::Restore);
            return;
        }
    };

    let cache_key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = ahash::AHasher::default();
        effect.name.hash(&mut hasher);
        hasher.finish() | 0xBB00_0000_0000_0000
    };

    let effect_id = builder.intern_effect(cache_key, &sksl);

    let uniforms = GlTransitionUniforms {
        progress: progress.clamp(0.0, 1.0),
        resolution: [w as f32, h as f32],
    };
    let uniforms_range = builder.intern_bytes(as_bytes(&uniforms));

    let child_start = builder.push_child(RuntimeEffectChildRef::Picture(from_range));
    builder.push_child(RuntimeEffectChildRef::Picture(to_range));

    builder.push(DrawOp::RuntimeEffect {
        effect: effect_id,
        uniforms: uniforms_range,
        children: ChildRange {
            start: child_start,
            len: 2,
        },
        dst: dst_rect4,
    });
}

// ── Bitmap ───────────────────────────────────────────────────────────


#[cfg(feature = "profile")]
use tracing::{Level, event, span};
