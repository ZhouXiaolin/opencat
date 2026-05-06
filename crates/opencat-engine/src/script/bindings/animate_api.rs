use std::sync::{Arc, Mutex};

use rquickjs::{Array, Function};

use opencat_core::script::animate::{
    AnimateState,
    MorphSvgState,
    PathMeasureState,
    parse_easing_from_tag,
    random_from_seed,
};
use opencat_core::script::recorder::{MutationRecorder, MutationStore};

pub(crate) const ANIMATE_RUNTIME: &str = opencat_core::script::runtime::ANIMATION_RUNTIME;

pub(crate) fn install_animate_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &Arc<Mutex<MutationStore>>,
    animate_state: &Arc<Mutex<AnimateState>>,
    morph_svg_state: &Arc<Mutex<MorphSvgState>>,
    path_measure_state: &Arc<Mutex<PathMeasureState>>,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    let s = store.clone();
    let a = animate_state.clone();
    globals.set(
        "__animate_create",
        Function::new(
            ctx.clone(),
            move |duration: f32,
                  delay: f32,
                  clamp_flag: i32,
                  easing_tag: String,
                  repeat: i32,
                  yoyo_flag: i32,
                  repeat_delay: f32|
                  -> i32 {
                let clamp = clamp_flag != 0;
                let yoyo = yoyo_flag != 0;
                let current_frame = {
                    let guard = s.lock().unwrap();
                    guard.current_frame()
                };
                let mut animate = a.lock().unwrap();
                animate.create(current_frame, duration, delay, clamp, &easing_tag, repeat, yoyo, repeat_delay)
            },
        )?,
    )?;

    let s = store.clone();
    let a = animate_state.clone();
    globals.set(
        "__animate_value",
        Function::new(
            ctx.clone(),
            move |handle: i32, _key: String, from: f32, to: f32| -> f32 {
                let current_frame = {
                    let guard = s.lock().unwrap();
                    guard.current_frame()
                };
                let animate = a.lock().unwrap();
                animate.value(current_frame, handle, from, to)
            },
        )?,
    )?;

    let a = animate_state.clone();
    globals.set(
        "__animate_color",
        Function::new(
            ctx.clone(),
            move |handle: i32, _key: String, from: String, to: String| -> String {
                let animate = a.lock().unwrap();
                animate.color(handle, &from, &to)
            },
        )?,
    )?;

    let a = animate_state.clone();
    globals.set(
        "__animate_progress",
        Function::new(ctx.clone(), move |handle: i32| -> f32 {
            let animate = a.lock().unwrap();
            animate.progress(handle)
        })?,
    )?;

    let a = animate_state.clone();
    globals.set(
        "__animate_settled",
        Function::new(ctx.clone(), move |handle: i32| -> bool {
            let animate = a.lock().unwrap();
            animate.settled(handle)
        })?,
    )?;

    let a = animate_state.clone();
    globals.set(
        "__animate_settle_frame",
        Function::new(ctx.clone(), move |handle: i32| -> u32 {
            let animate = a.lock().unwrap();
            animate.settle_frame(handle)
        })?,
    )?;

    let m = morph_svg_state.clone();
    globals.set(
        "__morph_svg_create",
        Function::new(
            ctx.clone(),
            move |from_svg: String, to_svg: String, grid_size: f32| -> i32 {
                let mut state = m.lock().unwrap();
                state.create(&from_svg, &to_svg, grid_size as u32).unwrap_or(-1)
            },
        )?,
    )?;

    let m = morph_svg_state.clone();
    globals.set(
        "__morph_svg_sample",
        Function::new(
            ctx.clone(),
            move |handle: i32, t: f32, tolerance: f32| -> String {
                let state = m.lock().unwrap();
                state.sample(handle, t, tolerance)
            },
        )?,
    )?;

    let m = morph_svg_state.clone();
    globals.set(
        "__morph_svg_dispose",
        Function::new(ctx.clone(), move |handle: i32| {
            let mut state = m.lock().unwrap();
            state.dispose(handle);
        })?,
    )?;

    globals.set(
        "__util_random_seeded",
        Function::new(ctx.clone(), |seed: f32| -> f32 { random_from_seed(seed) })?,
    )?;

    globals.set(
        "__text_graphemes",
        Function::new(
            ctx.clone(),
            |ctx_inner: rquickjs::Ctx<'js>, text: String| -> Result<Array<'js>, rquickjs::Error> {
                let result = Array::new(ctx_inner)?;
                for (index, grapheme) in
                    unicode_segmentation::UnicodeSegmentation::graphemes(text.as_str(), true)
                        .enumerate()
                {
                    result.set(index, grapheme.to_string())?;
                }
                Ok(result)
            },
        )?,
    )?;

    globals.set(
        "__easing_apply",
        Function::new(ctx.clone(), |tag: String, t: f32| -> f32 {
            let easing = parse_easing_from_tag(&tag);
            easing.apply(t)
        })?,
    )?;

    let p = path_measure_state.clone();
    globals.set(
        "__along_path_create",
        Function::new(
            ctx.clone(),
            move |svg: String| -> Result<i32, rquickjs::Error> {
                let mut state = p.lock().unwrap();
                state.create(&svg).ok_or_else(|| {
                    rquickjs::Error::new_from_js_message(
                        "alongPath",
                        "create",
                        "invalid SVG path",
                    )
                })
            },
        )?,
    )?;

    let p = path_measure_state.clone();
    globals.set(
        "__along_path_length",
        Function::new(ctx.clone(), move |handle: i32| -> f32 {
            let state = p.lock().unwrap();
            state.length(handle)
        })?,
    )?;

    let p = path_measure_state.clone();
    globals.set(
        "__along_path_at",
        Function::new(ctx.clone(), move |handle: i32, t: f32| -> Vec<f32> {
            let state = p.lock().unwrap();
            let (x, y, angle) = state.sample(handle, t);
            vec![x, y, angle]
        })?,
    )?;

    let p = path_measure_state.clone();
    globals.set(
        "__along_path_dispose",
        Function::new(ctx.clone(), move |handle: i32| {
            let mut state = p.lock().unwrap();
            state.dispose(handle);
        })?,
    )?;

    Ok(())
}
