#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use opencat_core::script::animate::color::{hsl_to_rgb, parse_color};
use opencat_core::script::animate::state::{parse_easing_from_tag, random_from_seed};
use opencat_core::script::animate::{AnimateState, MorphSvgState, PathMeasureState};
use opencat_core::script::recorder::{MutationRecorder, MutationStore, TextUnitValues};
use opencat_core::scene::script::mutations::{
    CanvasCommand, ScriptColor, ScriptFontEdging, TextUnitGranularity,
};
use opencat_core::scene::script::{
    align_items_from_name, box_shadow_from_name, drop_shadow_from_name, flex_direction_from_name,
    inset_shadow_from_name, justify_content_from_name, object_fit_from_name, position_from_name,
    text_align_from_name, ScriptTextSource, ScriptTextSourceKind,
};
use opencat_core::style::{color_token_from_script_name, BorderStyle, ColorToken, FontWeight, ObjectFit};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct WebMutationRecorder {
    inner: MutationStore,
    animate: AnimateState,
    morph: MorphSvgState,
    path_measure: PathMeasureState,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl WebMutationRecorder {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            inner: MutationStore::default(),
            animate: AnimateState::default(),
            morph: MorphSvgState::default(),
            path_measure: PathMeasureState::default(),
        }
    }

    pub fn reset_for_frame(&mut self, current_frame: u32) {
        self.inner.reset_for_frame(current_frame);
    }

    pub fn snapshot_mutations_json(&self) -> String {
        serde_json::to_string(&self.inner.snapshot_mutations()).unwrap_or_default()
    }

    // ── Numeric setters ──
    pub fn record_opacity(&mut self, id: &str, v: f32) {
        self.inner.record_opacity(id, v);
    }
    pub fn record_translate(&mut self, id: &str, x: f32, y: f32) {
        self.inner.record_translate(id, x, y);
    }
    pub fn record_translate_x(&mut self, id: &str, v: f32) {
        self.inner.record_translate_x(id, v);
    }
    pub fn record_translate_y(&mut self, id: &str, v: f32) {
        self.inner.record_translate_y(id, v);
    }
    pub fn record_scale(&mut self, id: &str, v: f32) {
        self.inner.record_scale(id, v);
    }
    pub fn record_scale_x(&mut self, id: &str, v: f32) {
        self.inner.record_scale_x(id, v);
    }
    pub fn record_scale_y(&mut self, id: &str, v: f32) {
        self.inner.record_scale_y(id, v);
    }
    pub fn record_rotate(&mut self, id: &str, deg: f32) {
        self.inner.record_rotate(id, deg);
    }
    pub fn record_skew_x(&mut self, id: &str, deg: f32) {
        self.inner.record_skew_x(id, deg);
    }
    pub fn record_skew_y(&mut self, id: &str, deg: f32) {
        self.inner.record_skew_y(id, deg);
    }
    pub fn record_skew(&mut self, id: &str, x: f32, y: f32) {
        self.inner.record_skew(id, x, y);
    }
    pub fn record_left(&mut self, id: &str, v: f32) {
        self.inner.record_left(id, v);
    }
    pub fn record_top(&mut self, id: &str, v: f32) {
        self.inner.record_top(id, v);
    }
    pub fn record_right(&mut self, id: &str, v: f32) {
        self.inner.record_right(id, v);
    }
    pub fn record_bottom(&mut self, id: &str, v: f32) {
        self.inner.record_bottom(id, v);
    }
    pub fn record_width(&mut self, id: &str, v: f32) {
        self.inner.record_width(id, v);
    }
    pub fn record_height(&mut self, id: &str, v: f32) {
        self.inner.record_height(id, v);
    }
    pub fn record_padding(&mut self, id: &str, v: f32) {
        self.inner.record_padding(id, v);
    }
    pub fn record_padding_x(&mut self, id: &str, v: f32) {
        self.inner.record_padding_x(id, v);
    }
    pub fn record_padding_y(&mut self, id: &str, v: f32) {
        self.inner.record_padding_y(id, v);
    }
    pub fn record_margin(&mut self, id: &str, v: f32) {
        self.inner.record_margin(id, v);
    }
    pub fn record_margin_x(&mut self, id: &str, v: f32) {
        self.inner.record_margin_x(id, v);
    }
    pub fn record_margin_y(&mut self, id: &str, v: f32) {
        self.inner.record_margin_y(id, v);
    }
    pub fn record_gap(&mut self, id: &str, v: f32) {
        self.inner.record_gap(id, v);
    }
    pub fn record_flex_grow(&mut self, id: &str, v: f32) {
        self.inner.record_flex_grow(id, v);
    }
    pub fn record_border_radius(&mut self, id: &str, v: f32) {
        self.inner.record_border_radius(id, v);
    }
    pub fn record_border_width(&mut self, id: &str, v: f32) {
        self.inner.record_border_width(id, v);
    }
    pub fn record_border_top_width(&mut self, id: &str, v: f32) {
        self.inner.record_border_top_width(id, v);
    }
    pub fn record_border_right_width(&mut self, id: &str, v: f32) {
        self.inner.record_border_right_width(id, v);
    }
    pub fn record_border_bottom_width(&mut self, id: &str, v: f32) {
        self.inner.record_border_bottom_width(id, v);
    }
    pub fn record_border_left_width(&mut self, id: &str, v: f32) {
        self.inner.record_border_left_width(id, v);
    }
    pub fn record_stroke_width(&mut self, id: &str, v: f32) {
        self.inner.record_stroke_width(id, v);
    }
    pub fn record_stroke_dasharray(&mut self, id: &str, v: f32) {
        self.inner.record_stroke_dasharray(id, v);
    }
    pub fn record_stroke_dashoffset(&mut self, id: &str, v: f32) {
        self.inner.record_stroke_dashoffset(id, v);
    }
    pub fn record_text_size(&mut self, id: &str, v: f32) {
        self.inner.record_text_size(id, v);
    }
    pub fn record_letter_spacing(&mut self, id: &str, v: f32) {
        self.inner.record_letter_spacing(id, v);
    }
    pub fn record_line_height(&mut self, id: &str, v: f32) {
        self.inner.record_line_height(id, v);
    }
    pub fn record_text_content(&mut self, id: &str, v: &str) {
        self.inner.record_text_content(id, v.to_string());
    }
    pub fn record_svg_path(&mut self, id: &str, v: &str) {
        self.inner.record_svg_path(id, v.to_string());
    }

    // ── Enum setters (string parse) ──
    pub fn record_position(&mut self, id: &str, v: &str) {
        if let Some(p) = position_from_name(v) {
            self.inner.record_position(id, p);
        }
    }
    pub fn record_flex_direction(&mut self, id: &str, v: &str) {
        if let Some(fd) = flex_direction_from_name(v) {
            self.inner.record_flex_direction(id, fd);
        }
    }
    pub fn record_justify_content(&mut self, id: &str, v: &str) {
        if let Some(jc) = justify_content_from_name(v) {
            self.inner.record_justify_content(id, jc);
        }
    }
    pub fn record_align_items(&mut self, id: &str, v: &str) {
        if let Some(ai) = align_items_from_name(v) {
            self.inner.record_align_items(id, ai);
        }
    }
    pub fn record_object_fit(&mut self, id: &str, v: &str) {
        if let Some(of) = object_fit_from_name(v) {
            self.inner.record_object_fit(id, of);
        }
    }
    pub fn record_text_align(&mut self, id: &str, v: &str) {
        if let Some(ta) = text_align_from_name(v) {
            self.inner.record_text_align(id, ta);
        }
    }
    pub fn record_border_style(&mut self, id: &str, v: &str) {
        let bs = match v {
            "solid" => BorderStyle::Solid,
            "dashed" => BorderStyle::Dashed,
            "dotted" => BorderStyle::Dotted,
            _ => return,
        };
        self.inner.record_border_style(id, bs);
    }
    pub fn record_font_weight(&mut self, id: &str, v: f64) {
        self.inner
            .record_font_weight(id, FontWeight(v as u16));
    }
    pub fn record_box_shadow(&mut self, id: &str, v: &str) {
        if let Some(s) = box_shadow_from_name(v) {
            self.inner.record_box_shadow(id, s);
        }
    }
    pub fn record_inset_shadow(&mut self, id: &str, v: &str) {
        if let Some(s) = inset_shadow_from_name(v) {
            self.inner.record_inset_shadow(id, s);
        }
    }
    pub fn record_drop_shadow(&mut self, id: &str, v: &str) {
        if let Some(s) = drop_shadow_from_name(v) {
            self.inner.record_drop_shadow(id, s);
        }
    }

    // ── Color setters (string parse) ──
    pub fn record_bg_color(&mut self, id: &str, v: &str) {
        if let Some(c) = parse_color_to_token(v) {
            self.inner.record_bg_color(id, c);
        }
    }
    pub fn record_fill_color(&mut self, id: &str, v: &str) {
        if let Some(c) = parse_color_to_token(v) {
            self.inner.record_fill_color(id, c);
        }
    }
    pub fn record_stroke_color(&mut self, id: &str, v: &str) {
        if let Some(c) = parse_color_to_token(v) {
            self.inner.record_stroke_color(id, c);
        }
    }
    pub fn record_border_color(&mut self, id: &str, v: &str) {
        if let Some(c) = parse_color_to_token(v) {
            self.inner.record_border_color(id, c);
        }
    }
    pub fn record_text_color(&mut self, id: &str, v: &str) {
        if let Some(c) = parse_color_to_token(v) {
            self.inner.record_text_color(id, c);
        }
    }
    pub fn record_box_shadow_color(&mut self, id: &str, v: &str) {
        if let Some(c) = parse_color_to_token(v) {
            self.inner.record_box_shadow_color(id, c);
        }
    }
    pub fn record_inset_shadow_color(&mut self, id: &str, v: &str) {
        if let Some(c) = parse_color_to_token(v) {
            self.inner.record_inset_shadow_color(id, c);
        }
    }
    pub fn record_drop_shadow_color(&mut self, id: &str, v: &str) {
        if let Some(c) = parse_color_to_token(v) {
            self.inner.record_drop_shadow_color(id, c);
        }
    }

    // ── Text unit override ──
    pub fn record_text_unit_override(
        &mut self,
        id: &str,
        granularity: &str,
        index: u32,
        opacity: Option<f32>,
        translate_x: Option<f32>,
        translate_y: Option<f32>,
        scale: Option<f32>,
        rotation_deg: Option<f32>,
        color: Option<String>,
    ) {
        let gran = match granularity {
            "graphemes" => TextUnitGranularity::Grapheme,
            "words" => TextUnitGranularity::Word,
            _ => return,
        };
        let color = color.and_then(|s| parse_color_to_token(&s));
        self.inner.record_text_unit_override(
            id,
            gran,
            index as usize,
            TextUnitValues {
                opacity,
                translate_x,
                translate_y,
                scale,
                rotation_deg,
                color,
            },
        );
    }

    // ── Canvas commands ──
    pub fn record_canvas_save(&mut self, id: &str) {
        self.inner.record_canvas_command(id, CanvasCommand::Save);
    }
    pub fn record_canvas_restore(&mut self, id: &str) {
        self.inner
            .record_canvas_command(id, CanvasCommand::Restore);
    }
    pub fn record_canvas_translate(&mut self, id: &str, x: f32, y: f32) {
        self.inner
            .record_canvas_command(id, CanvasCommand::Translate { x, y });
    }
    pub fn record_canvas_scale(&mut self, id: &str, x: f32, y: f32) {
        self.inner
            .record_canvas_command(id, CanvasCommand::Scale { x, y });
    }
    pub fn record_canvas_rotate(&mut self, id: &str, degrees: f32) {
        self.inner
            .record_canvas_command(id, CanvasCommand::Rotate { degrees });
    }
    pub fn record_canvas_clip_rect(
        &mut self,
        id: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        _aa: bool,
    ) {
        self.inner.record_canvas_command(
            id,
            CanvasCommand::ClipRect {
                x,
                y,
                width: w,
                height: h,
                anti_alias: true,
            },
        );
    }
    pub fn record_canvas_fill_rect(
        &mut self,
        id: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: &str,
    ) {
        let c = parse_script_color(color).unwrap_or(ScriptColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        });
        self.inner.record_canvas_command(
            id,
            CanvasCommand::FillRect {
                x,
                y,
                width: w,
                height: h,
                color: c,
            },
        );
    }
    pub fn record_canvas_fill_rrect(
        &mut self,
        id: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
    ) {
        self.inner.record_canvas_command(
            id,
            CanvasCommand::FillRRect {
                x,
                y,
                width: w,
                height: h,
                radius: r,
            },
        );
    }
    pub fn record_canvas_fill_circle(&mut self, id: &str, cx: f32, cy: f32, r: f32) {
        self.inner
            .record_canvas_command(id, CanvasCommand::FillCircle { cx, cy, radius: r });
    }
    pub fn record_canvas_stroke_circle(&mut self, id: &str, cx: f32, cy: f32, r: f32) {
        self.inner
            .record_canvas_command(id, CanvasCommand::StrokeCircle { cx, cy, radius: r });
    }
    pub fn record_canvas_draw_line(
        &mut self,
        id: &str,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    ) {
        self.inner
            .record_canvas_command(id, CanvasCommand::DrawLine { x0, y0, x1, y1 });
    }
    pub fn record_canvas_begin_path(&mut self, id: &str) {
        self.inner
            .record_canvas_command(id, CanvasCommand::BeginPath);
    }
    pub fn record_canvas_move_to(&mut self, id: &str, x: f32, y: f32) {
        self.inner
            .record_canvas_command(id, CanvasCommand::MoveTo { x, y });
    }
    pub fn record_canvas_line_to(&mut self, id: &str, x: f32, y: f32) {
        self.inner
            .record_canvas_command(id, CanvasCommand::LineTo { x, y });
    }
    pub fn record_canvas_quad_to(
        &mut self,
        id: &str,
        cx: f32,
        cy: f32,
        x: f32,
        y: f32,
    ) {
        self.inner
            .record_canvas_command(id, CanvasCommand::QuadTo { cx, cy, x, y });
    }
    pub fn record_canvas_cubic_to(
        &mut self,
        id: &str,
        c1x: f32,
        c1y: f32,
        c2x: f32,
        c2y: f32,
        x: f32,
        y: f32,
    ) {
        self.inner.record_canvas_command(
            id,
            CanvasCommand::CubicTo {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            },
        );
    }
    pub fn record_canvas_close_path(&mut self, id: &str) {
        self.inner
            .record_canvas_command(id, CanvasCommand::ClosePath);
    }
    pub fn record_canvas_fill_path(&mut self, id: &str) {
        self.inner
            .record_canvas_command(id, CanvasCommand::FillPath);
    }
    pub fn record_canvas_stroke_path(&mut self, id: &str) {
        self.inner
            .record_canvas_command(id, CanvasCommand::StrokePath);
    }
    pub fn record_canvas_set_fill_style(&mut self, id: &str, color: &str) {
        let c = parse_script_color(color).unwrap_or(ScriptColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        });
        self.inner
            .record_canvas_command(id, CanvasCommand::SetFillStyle { color: c });
    }
    pub fn record_canvas_set_stroke_style(&mut self, id: &str, color: &str) {
        let c = parse_script_color(color).unwrap_or(ScriptColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        });
        self.inner
            .record_canvas_command(id, CanvasCommand::SetStrokeStyle { color: c });
    }
    pub fn record_canvas_set_line_width(&mut self, id: &str, w: f32) {
        self.inner
            .record_canvas_command(id, CanvasCommand::SetLineWidth { width: w.max(0.0) });
    }
    pub fn record_canvas_set_global_alpha(&mut self, id: &str, alpha: f32) {
        self.inner.record_canvas_command(
            id,
            CanvasCommand::SetGlobalAlpha {
                alpha: alpha.clamp(0.0, 1.0),
            },
        );
    }
    pub fn record_canvas_clear(&mut self, id: &str, color: Option<String>) {
        let c = color.and_then(|s| parse_script_color(&s));
        self.inner
            .record_canvas_command(id, CanvasCommand::Clear { color: c });
    }
    pub fn record_canvas_draw_image(
        &mut self,
        id: &str,
        asset_id: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        alpha: f32,
    ) {
        self.inner.record_canvas_command(
            id,
            CanvasCommand::DrawImage {
                asset_id: asset_id.to_string(),
                x,
                y,
                width: w,
                height: h,
                src_rect: None,
                alpha,
                anti_alias: true,
                object_fit: ObjectFit::Cover,
            },
        );
    }
    pub fn record_canvas_draw_image_simple(
        &mut self,
        id: &str,
        asset_id: &str,
        x: f32,
        y: f32,
        alpha: f32,
    ) {
        self.inner.record_canvas_command(
            id,
            CanvasCommand::DrawImageSimple {
                asset_id: asset_id.to_string(),
                x,
                y,
                alpha,
                anti_alias: true,
            },
        );
    }
    pub fn record_canvas_draw_text(
        &mut self,
        id: &str,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        aa: bool,
        stroke: bool,
        sw: f32,
        color: &str,
    ) {
        let c = parse_script_color(color).unwrap_or(ScriptColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        });
        self.inner.record_canvas_command(
            id,
            CanvasCommand::DrawText {
                text: text.to_string(),
                x,
                y,
                color: c,
                anti_alias: aa,
                stroke,
                stroke_width: sw,
                font_size,
                font_scale_x: 1.0,
                font_skew_x: 0.0,
                font_subpixel: true,
                font_edging: ScriptFontEdging::AntiAlias,
            },
        );
    }

    // ── Text source ──
    pub fn get_text(&self, id: &str) -> Option<String> {
        self.inner.get_text_source(id).map(|s| s.text.clone())
    }
    pub fn set_text(&mut self, id: &str, text: &str) {
        self.inner.register_text_source(
            id,
            ScriptTextSource {
                text: text.to_string(),
                kind: ScriptTextSourceKind::TextNode,
            },
        );
    }

    // ── Animate state machine ──
    #[allow(clippy::too_many_arguments)]
    pub fn animate_create(
        &mut self,
        current_frame: u32,
        duration: f32,
        delay: f32,
        clamp: bool,
        easing: &str,
        repeat: i32,
        yoyo: bool,
        repeat_delay: f32,
    ) -> i32 {
        self.animate
            .create(current_frame, duration, delay, clamp, easing, repeat, yoyo, repeat_delay)
    }
    pub fn animate_value(&self, current_frame: u32, handle: i32, from: f32, to: f32) -> f32 {
        self.animate.value(current_frame, handle, from, to)
    }
    pub fn animate_color(&self, handle: i32, from: &str, to: &str) -> String {
        self.animate.color(handle, from, to)
    }
    pub fn animate_progress(&self, handle: i32) -> f32 {
        self.animate.progress(handle)
    }
    pub fn animate_settled(&self, handle: i32) -> bool {
        self.animate.settled(handle)
    }
    pub fn animate_settle_frame(&self, handle: i32) -> u32 {
        self.animate.settle_frame(handle)
    }

    // ── Morph SVG ──
    pub fn morph_svg_create(&mut self, from_svg: &str, to_svg: &str, grid: u32) -> i32 {
        self.morph.create(from_svg, to_svg, grid).unwrap_or(-1)
    }
    pub fn morph_svg_sample(&self, handle: i32, t: f32, tol: f32) -> String {
        self.morph.sample(handle, t, tol)
    }
    pub fn morph_svg_dispose(&mut self, handle: i32) {
        self.morph.dispose(handle);
    }

    // ── Along path ──
    pub fn along_path_create(&mut self, svg: &str) -> i32 {
        self.path_measure.create(svg).unwrap_or(-1)
    }
    pub fn along_path_length(&self, handle: i32) -> f32 {
        self.path_measure.length(handle)
    }
    pub fn along_path_at(&self, handle: i32, t: f32) -> Vec<f32> {
        let (x, y, a) = self.path_measure.sample(handle, t);
        vec![x, y, a]
    }
    pub fn along_path_dispose(&mut self, handle: i32) {
        self.path_measure.dispose(handle);
    }

    // ── Utils ──
    pub fn random_from_seed(&self, seed: f32) -> f32 {
        random_from_seed(seed)
    }
    pub fn easing_apply(&self, tag: &str, t: f32) -> f32 {
        let easing = parse_easing_from_tag(tag);
        easing.apply(t)
    }

    // ── Text units (wasm-only: requires js-sys) ──
    #[cfg(target_arch = "wasm32")]
    pub fn text_units_describe(
        &self,
        id: &str,
        granularity: &str,
    ) -> Result<Vec<js_sys::Array>, JsValue> {
        let text = self
            .inner
            .get_text_source(id)
            .map(|s| s.text.clone())
            .ok_or_else(|| JsValue::from_str("no text source"))?;
        let gran = match granularity {
            "graphemes" => TextUnitGranularity::Grapheme,
            "words" => TextUnitGranularity::Word,
            _ => return Err(JsValue::from_str("bad granularity")),
        };
        let units = opencat_core::script::text_units::describe_text_units(&text, gran);
        let result = units
            .iter()
            .map(|u| {
                let arr = js_sys::Array::new();
                arr.push(&JsValue::from_f64(u.index as f64));
                arr.push(&JsValue::from_str(&u.text));
                arr.push(&JsValue::from_f64(u.start as f64));
                arr.push(&JsValue::from_f64(u.end as f64));
                arr
            })
            .collect();
        Ok(result)
    }
}

fn parse_color_to_token(v: &str) -> Option<ColorToken> {
    if let Some(c) = color_token_from_script_name(v) {
        return Some(c);
    }
    let hsla = parse_color(v)?;
    let (r, g, b) = hsl_to_rgb(hsla.h, hsla.s, hsla.l);
    let a = (hsla.a.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(ColorToken::Custom(r, g, b, a))
}

fn parse_script_color(v: &str) -> Option<ScriptColor> {
    let token = parse_color_to_token(v)?;
    let (r, g, b, a) = token.rgba();
    Some(ScriptColor { r, g, b, a })
}
