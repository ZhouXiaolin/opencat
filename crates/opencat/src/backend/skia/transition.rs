use std::{cell::RefCell, collections::HashMap, sync::OnceLock, thread_local};

use anyhow::{Result, anyhow};
use serde::Deserialize;
use skia_safe::{
    AlphaType, Canvas, ColorType, Data, FilterMode, ImageInfo, Matrix, Paint, PathBuilder, Picture,
    RRect, Rect, RuntimeEffect, TileMode, runtime_effect::ChildPtr, surfaces,
};
use tracing::{Level, span};

use opencat_core::scene::transition::{
    GlTransition, LightLeakTransition, SlideDirection, TransitionKind, WipeDirection,
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
    static GL_TRANSITION_EFFECTS: RefCell<HashMap<String, RuntimeEffect>> = RefCell::new(HashMap::new());
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

#[repr(C)]
#[derive(Clone, Copy)]
struct GlTransitionUniforms {
    progress: f32,
    resolution: [f32; 2],
}

#[derive(Deserialize)]
struct GlTransitionJsonEntry {
    name: String,
    #[serde(default, rename = "defaultParams")]
    default_params: serde_json::Map<String, serde_json::Value>,
    #[serde(default, rename = "paramsTypes")]
    params_types: serde_json::Map<String, serde_json::Value>,
    glsl: String,
}

#[derive(Clone)]
struct GlTransitionSource {
    glsl: String,
    default_params: serde_json::Map<String, serde_json::Value>,
    params_types: serde_json::Map<String, serde_json::Value>,
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

impl GlTransitionUniforms {
    fn new(progress: f32, width: i32, height: i32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            resolution: [width as f32, height as f32],
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
        TransitionKind::Slide(direction) => {
            draw_slide_transition(canvas, from, to, progress, direction, width, height)
        }
        TransitionKind::LightLeak(params) => {
            draw_light_leak_transition(canvas, from, to, progress, params, width, height)
        }
        TransitionKind::Gl(effect) => {
            draw_gl_transition(canvas, from, to, progress, &effect, width, height)
        }
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
    let (to_offset, from_offset) = slide_offsets(direction, progress, width as f32, height as f32);

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

fn slide_offsets(
    direction: SlideDirection,
    progress: f32,
    width: f32,
    height: f32,
) -> ((f32, f32), (f32, f32)) {
    let to_offset = match direction {
        SlideDirection::FromLeft => (width * (progress - 1.0), 0.0),
        SlideDirection::FromRight => (width * (1.0 - progress), 0.0),
        SlideDirection::FromTop => (0.0, height * (progress - 1.0)),
        SlideDirection::FromBottom => (0.0, height * (1.0 - progress)),
    };

    let from_offset = match direction {
        SlideDirection::FromLeft => (width * progress, 0.0),
        SlideDirection::FromRight => (-width * progress, 0.0),
        SlideDirection::FromTop => (0.0, height * progress),
        SlideDirection::FromBottom => (0.0, -height * progress),
    };

    (to_offset, from_offset)
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
) -> Result<()> {
    let mask_scale = params.mask_scale.clamp(0.03125, 1.0);
    let mask_size = scaled_mask_size(width, height, mask_scale);
    let mask_image = {
        let mask_span = span!(target: "render.backend", Level::TRACE, "light_leak_mask");
        let _mask_guard = mask_span.enter();
        render_light_leak_mask(progress, params, mask_size.0, mask_size.1)?
    };

    let from_matrix = picture_shader_local_matrix(from);
    let to_matrix = picture_shader_local_matrix(to);
    let from_shader = from.to_shader(
        None,
        FilterMode::Linear,
        Some(&from_matrix),
        Option::<&Rect>::None,
    );
    let to_shader = to.to_shader(
        None,
        FilterMode::Linear,
        Some(&to_matrix),
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

    {
        let composite_span = span!(target: "render.backend", Level::TRACE, "light_leak_composite");
        let _composite_guard = composite_span.enter();
        let mut paint = Paint::default();
        paint.set_shader(shader);
        canvas.draw_paint(&paint);
    }
    Ok(())
}

fn draw_gl_transition(
    canvas: &Canvas,
    from: &Picture,
    to: &Picture,
    progress: f32,
    effect: &GlTransition,
    width: i32,
    height: i32,
) -> Result<()> {
    let from_matrix = picture_shader_local_matrix(from);
    let to_matrix = picture_shader_local_matrix(to);
    let from_shader = from.to_shader(
        None,
        FilterMode::Linear,
        Some(&from_matrix),
        Option::<&Rect>::None,
    );
    let to_shader = to.to_shader(
        None,
        FilterMode::Linear,
        Some(&to_matrix),
        Option::<&Rect>::None,
    );

    let uniforms = GlTransitionUniforms::new(progress, width, height);
    let children = [ChildPtr::from(from_shader), ChildPtr::from(to_shader)];
    let shader = gl_transition_effect(effect)?
        .make_shader(uniform_data(&uniforms), &children, Option::<&Matrix>::None)
        .ok_or_else(|| anyhow!("failed to create GLTransition shader `{}`", effect.name))?;

    let mut paint = Paint::default();
    paint.set_shader(shader);
    canvas.draw_paint(&paint);
    Ok(())
}

fn picture_shader_local_matrix(picture: &Picture) -> Matrix {
    let cull = picture.cull_rect();
    Matrix::translate((cull.left(), cull.top()))
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

fn gl_transition_effect(effect: &GlTransition) -> Result<RuntimeEffect> {
    GL_TRANSITION_EFFECTS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let key = normalize_gltransition_name(&effect.name);
        if let Some(runtime_effect) = slot.get(&key) {
            return Ok(runtime_effect.clone());
        }

        let source = gl_transition_source(&effect.name)?;
        let sksl =
            gl_transition_glsl_to_sksl(&source.glsl, &source.default_params, &source.params_types)?;
        let effect = RuntimeEffect::make_for_shader(&sksl, None).map_err(|err| {
            anyhow!(
                "failed to compile GLTransition `{}` as SKSL: {err}\n{sksl}",
                effect.name
            )
        })?;
        slot.insert(key, effect.clone());
        Ok(effect)
    })
}

fn gl_transition_source(name: &str) -> Result<GlTransitionSource> {
    static GLTRANSITION_JSON: &str = include_str!("../../../../../gltransition.json");
    static SOURCES_BY_NAME: OnceLock<Result<HashMap<String, GlTransitionSource>, String>> =
        OnceLock::new();

    let map = SOURCES_BY_NAME.get_or_init(|| {
        let entries: Vec<GlTransitionJsonEntry> =
            serde_json::from_str(GLTRANSITION_JSON).map_err(|error| error.to_string())?;
        Ok(entries
            .into_iter()
            .map(|entry| {
                (
                    normalize_gltransition_name(&entry.name),
                    GlTransitionSource {
                        glsl: entry.glsl,
                        default_params: entry.default_params,
                        params_types: entry.params_types,
                    },
                )
            })
            .collect())
    });
    let map = map
        .as_ref()
        .map_err(|error| anyhow!("failed to parse gltransition.json: {error}"))?;
    let key = normalize_gltransition_name(name);
    map.get(&key)
        .cloned()
        .ok_or_else(|| anyhow!("gltransition.json is missing `{name}`"))
}

fn gl_transition_glsl_to_sksl(
    glsl: &str,
    default_params: &serde_json::Map<String, serde_json::Value>,
    params_types: &serde_json::Map<String, serde_json::Value>,
) -> Result<String> {
    let mut source = expand_defines(glsl);
    source = strip_precision_blocks(&source);
    source = replace_transition_uniforms(&source, default_params, params_types)?;
    source = source.replace("getFromColor", "getFromColor");
    source = source.replace("getToColor", "getToColor");
    source = replace_glsl_types(&source);
    source = source.replace("float2(1.0).xy", "float2(1.0)");
    source = replace_swizzle(&source, "uv.xy", "uv");
    source = replace_swizzle(&source, "p.xy", "p");
    source = inline_global_initializers(&source);

    if !source.contains("transition") {
        return Err(anyhow!(
            "GLTransition source does not define transition(vec2)"
        ));
    }

    Ok(format!(
        r#"
uniform shader fromScene;
uniform shader toScene;
uniform float progress;
uniform float2 resolution;

half4 getFromColor(float2 uv) {{
    return fromScene.eval(uv * resolution);
}}

half4 getToColor(float2 uv) {{
    return toScene.eval(uv * resolution);
}}

const float ratio = 1.0;

{source}

half4 main(float2 coord) {{
    float2 uv = coord / resolution;
    return transition(uv);
}}
"#
    ))
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn replace_word(src: &str, from: &str, to: &str) -> String {
    let mut result = String::with_capacity(src.len());
    let mut i = 0;
    let bytes = src.as_bytes();
    while i < src.len() {
        if let Some(pos) = src[i..].find(from) {
            let abs = i + pos;
            let after = abs + from.len();
            let before_ok = abs == 0 || !is_word_char(bytes[abs - 1]);
            let after_ok = after >= src.len() || !is_word_char(bytes[after]);
            if before_ok && after_ok {
                result.push_str(&src[i..abs]);
                result.push_str(to);
                i = after;
            } else {
                let next = abs + 1;
                result.push_str(&src[i..next]);
                i = next;
            }
        } else {
            result.push_str(&src[i..]);
            break;
        }
    }
    result
}

fn find_matching_paren(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if start >= s.len() || bytes[start] != b'(' {
        return None;
    }
    let mut depth = 1i32;
    let mut i = start + 1;
    while i < s.len() && depth > 0 {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    if depth == 0 { Some(i - 1) } else { None }
}

fn split_args(s: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                args.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        args.push(cur.trim().to_string());
    }
    args
}

fn expand_func_macro(src: &str, name: &str, params: &[String], body: &str) -> String {
    let mut result = String::new();
    let mut i = 0;
    let bytes = src.as_bytes();
    while i < src.len() {
        if let Some(pos) = src[i..].find(name) {
            let abs = i + pos;
            let after_name = abs + name.len();
            let before_ok = abs == 0 || !is_word_char(bytes[abs - 1]);
            if before_ok && after_name < src.len() && bytes[after_name] == b'(' {
                if let Some(close) = find_matching_paren(src, after_name) {
                    let args_str = &src[after_name + 1..close];
                    let args = split_args(args_str);
                    let mut expanded = body.to_string();
                    for (pi, param) in params.iter().enumerate() {
                        if let Some(arg) = args.get(pi) {
                            expanded = replace_word(&expanded, param, arg);
                        }
                    }
                    result.push_str(&src[i..abs]);
                    result.push_str(&expanded);
                    i = close + 1;
                    continue;
                }
            }
            result.push_str(&src[i..after_name]);
            i = after_name;
        } else {
            result.push_str(&src[i..]);
            break;
        }
    }
    result
}

fn expand_defines(input: &str) -> String {
    let mut simple_defines: Vec<(String, String)> = Vec::new();
    let mut func_defines: Vec<(String, Vec<String>, String)> = Vec::new();
    let mut lines: Vec<String> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#ifdef")
            || trimmed.starts_with("#endif")
            || trimmed.starts_with("precision ")
        {
            continue;
        }
        if trimmed.starts_with("#define") {
            let rest = trimmed.trim_start_matches("#define").trim();
            if rest.is_empty() {
                continue;
            }
            let name_end = rest
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            let name = rest[..name_end].trim();
            if name.is_empty() {
                continue;
            }
            let after_name = rest[name_end..].trim_start();
            if after_name.starts_with('(') {
                let inner = &after_name[1..];
                if let Some(close) = inner.find(')') {
                    let params_str = inner[..close].trim();
                    let params: Vec<String> = params_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    let raw_body = inner[close + 1..].trim();
                    let body = raw_body
                        .split_once("//")
                        .map(|(code, _)| code.trim())
                        .unwrap_or(raw_body)
                        .to_string();
                    func_defines.push((name.to_string(), params, body));
                    continue;
                }
            }
            let value = after_name
                .split_once("//")
                .map(|(code, _)| code.trim())
                .unwrap_or(after_name)
                .to_string();
            simple_defines.push((name.to_string(), value));
            continue;
        }
        lines.push(line.to_string());
    }

    let mut result = lines.join("\n");

    for (name, params, body) in &func_defines {
        result = expand_func_macro(&result, name, params, body);
    }

    for (name, value) in &simple_defines {
        result = replace_word(&result, name, value);
    }

    result
}

fn is_swizzle_char(b: u8) -> bool {
    matches!(b, b'x' | b'y' | b'z' | b'w' | b'r' | b'g' | b'b' | b'a')
}

fn replace_swizzle(src: &str, swizzle: &str, replacement: &str) -> String {
    let mut result = String::with_capacity(src.len());
    let mut i = 0;
    let bytes = src.as_bytes();
    while i < src.len() {
        if let Some(pos) = src[i..].find(swizzle) {
            let abs = i + pos;
            let after = abs + swizzle.len();
            let ok = after >= src.len() || !is_swizzle_char(bytes[after]);
            if ok {
                result.push_str(&src[i..abs]);
                result.push_str(replacement);
                i = after;
            } else {
                let next = abs + 1;
                result.push_str(&src[i..next]);
                i = next;
            }
        } else {
            result.push_str(&src[i..]);
            break;
        }
    }
    result
}

fn strip_precision_blocks(input: &str) -> String {
    input.replace("GL_ES", "")
}

fn extract_inline_default(line: &str) -> (String, Option<String>) {
    let no_line = line.split_once("//").map(|(code, _)| code).unwrap_or(line);
    let mut result = String::with_capacity(no_line.len());
    let mut default = None;
    let mut chars = no_line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut content = String::new();
            loop {
                match chars.next() {
                    Some('*') if chars.peek() == Some(&'/') => {
                        chars.next();
                        break;
                    }
                    Some(c) => content.push(c),
                    None => break,
                }
            }
            if let Some(eq_pos) = content.find('=') {
                default = Some(content[eq_pos + 1..].trim().to_string());
            }
        } else {
            result.push(c);
        }
    }
    (result, default)
}

fn replace_transition_uniforms(
    input: &str,
    default_params: &serde_json::Map<String, serde_json::Value>,
    params_types: &serde_json::Map<String, serde_json::Value>,
) -> Result<String> {
    for name in default_params.keys() {
        if !params_types.contains_key(name) {
            return Err(anyhow!(
                "GLTransition default parameter `{name}` is missing from paramsTypes"
            ));
        }
    }

    let mut output = String::new();
    let mut extra_params = String::new();
    let mut emitted_params = std::collections::HashSet::new();
    for line in input.lines() {
        let (clean_line, inline_default) = extract_inline_default(line);
        let trimmed = clean_line.trim();
        if !trimmed.starts_with("uniform ") {
            output.push_str(line);
            output.push('\n');
            continue;
        }

        let declaration = trimmed.trim_end_matches(';');
        let mut parts = declaration.split_whitespace();
        let _uniform = parts.next();
        let Some(ty) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };

        if matches!(name, "progress" | "resolution") {
            continue;
        }

        let ty = params_types
            .get(name)
            .and_then(serde_json::Value::as_str)
            .unwrap_or(ty);
        let value = if let Some(v) = default_params.get(name) {
            default_param_to_sksl(ty, v).ok_or_else(|| {
                anyhow!("GLTransition parameter `{name}` has unsupported default value `{v}`")
            })?
        } else if let Some(inline_val) = &inline_default {
            let glsl_to_sksl = |s: &str| -> String {
                s.replace("vec2", "float2")
                    .replace("vec3", "float3")
                    .replace("vec4", "float4")
                    .replace("ivec2", "int2")
                    .replace("ivec3", "int3")
                    .replace("ivec4", "int4")
                    .replace("bvec2", "bool2")
                    .replace("bvec3", "bool3")
                    .replace("bvec4", "bool4")
            };
            glsl_to_sksl(inline_val)
        } else {
            return Err(anyhow!(
                "GLTransition parameter `{name}` is missing a default value"
            ));
        };
        emit_const_param(&mut output, ty, name, &value);
        emitted_params.insert(name.to_string());
    }

    for (name, ty) in params_types {
        if emitted_params.contains(name) || matches!(name.as_str(), "progress" | "resolution") {
            continue;
        }
        let Some(ty) = ty.as_str() else {
            continue;
        };
        let value = default_params
            .get(name)
            .ok_or_else(|| anyhow!("GLTransition parameter `{name}` is missing a default value"))?;
        let value = default_param_to_sksl(ty, value).ok_or_else(|| {
            anyhow!("GLTransition parameter `{name}` has unsupported default value `{value}`")
        })?;
        emit_const_param(&mut extra_params, ty, name, &value);
    }

    Ok(format!("{extra_params}{output}"))
}

fn emit_const_param(output: &mut String, ty: &str, name: &str, value: &str) {
    output.push_str("const ");
    output.push_str(ty);
    output.push(' ');
    output.push_str(name);
    output.push_str(" = ");
    output.push_str(value);
    output.push_str(";\n");
}

fn default_param_to_sksl(ty: &str, value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Number(value) => {
            let literal = number_literal_for_type(ty, value)?;
            Some(match ty {
                "vec2" | "ivec2" => format!("{ty}({literal})"),
                "vec3" => format!("{ty}({literal})"),
                "vec4" => format!("{ty}({literal})"),
                _ => literal,
            })
        }
        serde_json::Value::Array(values) => {
            let args = values
                .iter()
                .map(|value| default_param_to_sksl(vector_scalar_type(ty), value))
                .collect::<Option<Vec<_>>>()?
                .join(", ");
            Some(format!("{ty}({args})"))
        }
        serde_json::Value::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn number_literal_for_type(ty: &str, value: &serde_json::Number) -> Option<String> {
    if matches!(ty, "int" | "ivec2" | "ivec3" | "ivec4") {
        return value
            .as_i64()
            .or_else(|| value.as_f64().map(|value| value.round() as i64))
            .map(|value| value.to_string());
    }
    value.as_f64().map(format_float_literal)
}

fn vector_scalar_type(ty: &str) -> &str {
    match ty {
        "ivec2" | "ivec3" | "ivec4" => "int",
        _ => "float",
    }
}

fn format_float_literal(value: f64) -> String {
    let mut literal = value.to_string();
    if !literal.contains('.') && !literal.contains('e') && !literal.contains('E') {
        literal.push_str(".0");
    }
    literal
}

fn replace_glsl_types(input: &str) -> String {
    replace_identifier_tokens(input, |token| match token {
        "ivec2" => Some("int2"),
        "ivec3" => Some("int3"),
        "ivec4" => Some("int4"),
        "vec4" => Some("half4"),
        "vec3" => Some("float3"),
        "vec2" => Some("float2"),
        "mat2" => Some("float2x2"),
        "mat3" => Some("float3x3"),
        _ => None,
    })
}

fn replace_identifier_tokens<F>(input: &str, mut replace: F) -> String
where
    F: FnMut(&str) -> Option<&'static str>,
{
    let mut output = String::with_capacity(input.len());
    let mut token = String::new();

    for char in input.chars() {
        if is_identifier_char(char) {
            token.push(char);
            continue;
        }

        if !token.is_empty() {
            if let Some(replacement) = replace(&token) {
                output.push_str(replacement);
            } else {
                output.push_str(&token);
            }
            token.clear();
        }
        output.push(char);
    }

    if !token.is_empty() {
        if let Some(replacement) = replace(&token) {
            output.push_str(replacement);
        } else {
            output.push_str(&token);
        }
    }

    output
}

fn is_identifier_char(char: char) -> bool {
    char == '_' || char.is_ascii_alphanumeric()
}

fn is_inlineable_global(trimmed: &str) -> Option<(String, String)> {
    if trimmed.starts_with("const ") || trimmed.starts_with("uniform ") {
        return None;
    }
    if !trimmed.ends_with(';') || !trimmed.contains('=') {
        return None;
    }
    let Some((left, right)) = trimmed.split_once('=') else {
        return None;
    };
    if left.contains('(') {
        return None;
    }
    let type_name = left.split_whitespace().next()?;
    let valid_types: &[&str] = &[
        "float", "half", "int", "bool", "float2", "float3", "half4", "int2", "int3", "int4",
    ];
    if !valid_types.contains(&type_name) {
        return None;
    }
    let name = left.split_whitespace().nth(1)?.to_string();
    let expr = right.trim_end_matches(';').trim().to_string();
    Some((name, expr))
}

fn inline_global_initializers(input: &str) -> String {
    let mut hoistable: Vec<(String, String)> = Vec::new();
    let mut kept: Vec<String> = Vec::new();
    let mut brace_depth = 0_i32;

    for line in input.lines() {
        if brace_depth == 0 {
            if let Some(pair) = is_inlineable_global(line.trim()) {
                hoistable.push(pair);
                brace_depth += line.chars().filter(|c| *c == '{').count() as i32;
                brace_depth -= line.chars().filter(|c| *c == '}').count() as i32;
                continue;
            }
        }
        kept.push(line.to_string());
        brace_depth += line.chars().filter(|c| *c == '{').count() as i32;
        brace_depth -= line.chars().filter(|c| *c == '}').count() as i32;
    }

    if hoistable.is_empty() {
        return input.to_string();
    }

    let mut result = kept.join("\n");
    for (name, expr) in hoistable.into_iter().rev() {
        result = replace_word(&result, &name, &format!("({expr})"));
    }
    result
}

fn normalize_gltransition_name(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|char| *char != '-' && *char != '_' && !char.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
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

#[cfg(test)]
mod tests {
    use super::slide_offsets;
    use opencat_core::scene::transition::SlideDirection;

    #[test]
    fn slide_offsets_match_expected_directions() {
        let width = 100.0;
        let height = 60.0;
        let progress = 0.25;

        assert_eq!(
            slide_offsets(SlideDirection::FromLeft, progress, width, height),
            ((-75.0, 0.0), (25.0, 0.0))
        );
        assert_eq!(
            slide_offsets(SlideDirection::FromRight, progress, width, height),
            ((75.0, 0.0), (-25.0, 0.0))
        );
        assert_eq!(
            slide_offsets(SlideDirection::FromTop, progress, width, height),
            ((0.0, -45.0), (0.0, 15.0))
        );
        assert_eq!(
            slide_offsets(SlideDirection::FromBottom, progress, width, height),
            ((0.0, 45.0), (0.0, -15.0))
        );
    }
}
