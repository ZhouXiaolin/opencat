use std::sync::{Arc, Mutex};

use rquickjs::{Array, Function};

use opencat_core::script::animate::{parse_easing_from_tag, random_from_seed};
use opencat_core::script::recorder::MutationRecorder;
use opencat_core::script::recorder::MutationStore;
use opencat_core::script::text_units::grapheme_strings;

pub(crate) const ANIMATE_RUNTIME: &str = opencat_core::script::runtime::ANIMATION_RUNTIME;

pub(crate) fn install_animate_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &Arc<Mutex<MutationStore>>,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    let s = store.clone();
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
                let mut guard = s.lock().unwrap();
                let cf = guard.current_frame();
                guard.animate_create(
                    cf,
                    duration,
                    delay,
                    clamp,
                    &easing_tag,
                    repeat,
                    yoyo,
                    repeat_delay,
                )
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__animate_value",
        Function::new(
            ctx.clone(),
            move |handle: i32, _key: String, from: f32, to: f32| -> f32 {
                let guard = s.lock().unwrap();
                let cf = guard.current_frame();
                guard.animate_value(cf, handle, from, to)
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__animate_color",
        Function::new(
            ctx.clone(),
            move |handle: i32, _key: String, from: String, to: String| -> String {
                let guard = s.lock().unwrap();
                guard.animate_color(handle, &from, &to)
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__animate_progress",
        Function::new(ctx.clone(), move |handle: i32| -> f32 {
            let guard = s.lock().unwrap();
            guard.animate_progress(handle)
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__animate_settled",
        Function::new(ctx.clone(), move |handle: i32| -> bool {
            let guard = s.lock().unwrap();
            guard.animate_settled(handle)
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__animate_settle_frame",
        Function::new(ctx.clone(), move |handle: i32| -> u32 {
            let guard = s.lock().unwrap();
            guard.animate_settle_frame(handle)
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__morph_svg_create",
        Function::new(
            ctx.clone(),
            move |from_svg: String, to_svg: String, grid_size: f32| -> i32 {
                let mut guard = s.lock().unwrap();
                guard
                    .morph_svg_create(&from_svg, &to_svg, grid_size as u32)
                    .unwrap_or(-1)
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__morph_svg_sample",
        Function::new(
            ctx.clone(),
            move |handle: i32, t: f32, tolerance: f32| -> String {
                let guard = s.lock().unwrap();
                guard.morph_svg_sample(handle, t, tolerance)
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__morph_svg_dispose",
        Function::new(ctx.clone(), move |handle: i32| {
            let mut guard = s.lock().unwrap();
            guard.morph_svg_dispose(handle);
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
                for (index, grapheme) in grapheme_strings(&text).into_iter().enumerate() {
                    result.set(index, grapheme)?;
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

    let s = store.clone();
    globals.set(
        "__along_path_create",
        Function::new(
            ctx.clone(),
            move |svg: String| -> Result<i32, rquickjs::Error> {
                let mut guard = s.lock().unwrap();
                guard.along_path_create(&svg).ok_or_else(|| {
                    rquickjs::Error::new_from_js_message("alongPath", "create", "invalid SVG path")
                })
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__along_path_length",
        Function::new(ctx.clone(), move |handle: i32| -> f32 {
            let guard = s.lock().unwrap();
            guard.along_path_length(handle)
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__along_path_at",
        Function::new(ctx.clone(), move |handle: i32, t: f32| -> Vec<f32> {
            let guard = s.lock().unwrap();
            let (x, y, angle) = guard.along_path_at(handle, t);
            vec![x, y, angle]
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__along_path_dispose",
        Function::new(ctx.clone(), move |handle: i32| {
            let mut guard = s.lock().unwrap();
            guard.along_path_dispose(handle);
        })?,
    )?;

    Ok(())
}
