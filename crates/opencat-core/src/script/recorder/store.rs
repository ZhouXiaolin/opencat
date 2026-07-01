use serde_json::json;
use std::collections::HashMap;

use super::{MutationRecorder, TextUnitValues};
use crate::frame_ctx::duration_secs_to_frames;
use crate::parse::easing::Easing;
use crate::script::ScriptTextSource;
use crate::script::animate::color::{hsla_to_rgba_string, lerp_hsla_clamped, parse_color};
use crate::script::animate::morph_svg::MorphSvgEntry;
use crate::script::animate::path_measure::PathMeasureEntry;
use crate::script::mutations::{
    CanvasMutations, NodeStyleMutations, StyleMutations, TextUnitGranularity, TextUnitOverride,
    TextUnitOverrideBatch,
};
use crate::script::{
    align_items_from_name, box_shadow_from_name, drop_shadow_from_name, flex_direction_from_name,
    inset_shadow_from_name, justify_content_from_name, object_fit_from_name, position_from_name,
    text_align_from_name,
};
use crate::style::{
    AlignItems, BorderStyle, BoxShadow, ClipPath, ColorToken, CssFilter, CssFilterKind, DropShadow,
    FlexDirection, FontWeight, InsetShadow, JustifyContent, NodeStyle, ObjectFit, Position,
    TextAlign, Transform, color_token_from_script_string,
};

#[derive(Default)]
pub struct MutationStore {
    styles: HashMap<String, NodeStyleMutations>,
    canvases: HashMap<String, CanvasMutations>,
    /// Base-style values keyed by (node_id, property_name), set once before
    /// scripts run. Not cleared during reset_for_frame so read_style_value
    /// always sees Tailwind/className defaults. Stores JSON values (f32, string, etc.)
    initial_styles: HashMap<String, HashMap<String, serde_json::Value>>,
    text_sources: HashMap<String, ScriptTextSource>,
    animate_entries: HashMap<i32, AnimateEntry>,
    animate_next_id: i32,
    morph_entries: HashMap<i32, MorphSvgEntry>,
    morph_next_id: i32,
    path_entries: HashMap<i32, PathMeasureEntry>,
    path_next_id: i32,
    current_frame: u32,
    fps: u32,
}

pub struct AnimateEntry {
    pub progress: f32,
    pub settled: bool,
    pub settle_frame: u32,
    pub duration: u32,
    pub delay: u32,
    pub clamp: bool,
    pub easing: Easing,
    pub repeat: i32,
    pub yoyo: bool,
    pub repeat_delay: u32,
}

fn css_filter_from_value(value: &serde_json::Value) -> Option<CssFilter> {
    if let Some(filter) = value.as_str() {
        return Some(CssFilter::parse_css_filter_string(filter));
    }

    if let Some(ops) = value.get("ops").and_then(|ops| ops.as_array()) {
        let mut filter = CssFilter::default();
        for op in ops {
            let Some(kind) = op
                .get("kind")
                .and_then(|kind| kind.as_str())
                .and_then(CssFilterKind::from_property)
            else {
                continue;
            };
            let Some(value) = op.get("value").and_then(|value| value.as_f64()) else {
                continue;
            };
            filter.push(kind, value as f32);
        }
        return Some(filter);
    }

    let mut filter = CssFilter::default();
    let object = value.as_object()?;
    for (name, value) in object {
        let Some(kind) = CssFilterKind::from_property(name) else {
            continue;
        };
        let Some(value) = value.as_f64() else {
            continue;
        };
        filter.push(kind, value as f32);
    }
    Some(filter)
}

fn css_filter_to_value(filter: &CssFilter) -> serde_json::Value {
    json!({
        "ops": filter.ops.iter().map(|op| {
            json!({
                "kind": op.kind.property_name(),
                "value": op.value,
            })
        }).collect::<Vec<_>>(),
    })
}

fn clip_path_from_value(value: &serde_json::Value) -> Option<Option<ClipPath>> {
    let raw = value.as_str()?;
    if raw.trim().eq_ignore_ascii_case("none") {
        return Some(None);
    }
    ClipPath::parse_css(raw).map(Some)
}

impl MutationStore {
    fn entry(&mut self, id: &str) -> &mut NodeStyleMutations {
        self.styles.entry(id.to_string()).or_default()
    }

    fn canvas_entry(&mut self, id: &str) -> &mut CanvasMutations {
        self.canvases.entry(id.to_string()).or_default()
    }

    fn seconds_to_frames(&self, seconds: f32) -> u32 {
        duration_secs_to_frames(seconds as f64, self.fps.max(1))
    }

    /// Set a base-style value (from Tailwind/resolve) for later reading.
    pub fn set_initial_style(&mut self, id: &str, property: &str, value: serde_json::Value) {
        self.initial_styles
            .entry(id.to_string())
            .or_default()
            .insert(property.to_string(), value);
    }

    /// Populate initial styles from a resolved NodeStyle. All animatable
    /// property extraction is centralized here — callers just pass (id, &NodeStyle).
    pub fn set_initial_style_from_node(&mut self, id: &str, style: &NodeStyle) {
        let mut props = HashMap::new();

        macro_rules! add_f32 {
            ($key:expr, $field:ident) => {
                if let Some(v) = style.$field {
                    props.insert($key.into(), json!(v));
                }
            };
        }
        macro_rules! add_color {
            ($key:expr, $field:ident) => {
                if let Some(ref c) = style.$field {
                    let (r, g, b, a) = c.rgba();
                    props.insert(
                        $key.into(),
                        json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a)),
                    );
                }
            };
        }

        // f32
        add_f32!("opacity", opacity);
        add_f32!("strokeDasharray", stroke_dasharray);
        add_f32!("strokeDashoffset", stroke_dashoffset);
        add_f32!("strokeWidth", stroke_width);
        add_f32!("borderWidth", border_width);
        add_f32!("textSize", text_px);
        add_f32!("letterSpacing", letter_spacing);
        add_f32!("lineHeight", line_height);
        add_f32!("width", width);
        add_f32!("height", height);
        add_f32!("gap", gap);
        add_f32!("flexGrow", flex_grow);
        for op in &style.css_filter.ops {
            props.insert(op.kind.property_name().into(), json!(op.value));
        }
        if !style.css_filter.is_empty() {
            props.insert("filter".into(), css_filter_to_value(&style.css_filter));
        }
        if let Some(clip_path) = style.clip_path {
            props.insert("clipPath".into(), json!(clip_path.to_css_string()));
        }
        add_f32!("backdropBlur", backdrop_blur_sigma);

        // colors
        add_color!("fillColor", fill_color);
        add_color!("strokeColor", stroke_color);
        add_color!("backgroundColor", bg_color);
        add_color!("textColor", text_color);
        add_color!("borderColor", border_color);

        // strings
        if let Some(ref s) = style.svg_path {
            props.insert("morphSVG".into(), json!(s));
        }

        if !props.is_empty() {
            self.initial_styles.insert(id.to_string(), props);
        }
    }

    /// Read the current value: mutation first, then base-style, then None.
    /// Returns JSON: f32 as Number, color as String, path as String.
    pub fn read_style_value(&self, id: &str, property: &str) -> Option<serde_json::Value> {
        // 1. Check current-frame mutations
        if let Some(mutations) = self.styles.get(id) {
            let v: Option<serde_json::Value> = match property {
                // f32
                "opacity" => mutations.opacity.map(|f| json!(f)),
                "strokeDasharray" => mutations.stroke_dasharray.map(|f| json!(f)),
                "strokeDashoffset" => mutations.stroke_dashoffset.map(|f| json!(f)),
                "strokeWidth" => mutations.stroke_width.map(|f| json!(f)),
                "borderRadius" => mutations.border_radius.map(|f| json!(f)),
                "borderWidth" => mutations.border_width.map(|f| json!(f)),
                "textSize" => mutations.text_px.map(|f| json!(f)),
                "letterSpacing" => mutations.letter_spacing.map(|f| json!(f)),
                "lineHeight" => mutations.line_height.map(|f| json!(f)),
                "width" => mutations.width.map(|f| json!(f)),
                "height" => mutations.height.map(|f| json!(f)),
                "gap" => mutations.gap.map(|f| json!(f)),
                "flexGrow" => mutations.flex_grow.map(|f| json!(f)),
                "left" => mutations.inset_left.map(|f| json!(f)),
                "top" => mutations.inset_top.map(|f| json!(f)),
                "right" => mutations.inset_right.map(|f| json!(f)),
                "bottom" => mutations.inset_bottom.map(|f| json!(f)),
                "filter" => (!mutations.css_filter.is_empty())
                    .then(|| css_filter_to_value(&mutations.css_filter)),
                "clipPath" | "clip-path" => mutations
                    .clip_path
                    .map(|clip_path| json!(clip_path.to_css_string())),
                "blur" | "blurSigma" | "brightness" | "contrast" | "grayscale" | "hueRotate"
                | "invert" | "saturate" | "sepia" => {
                    mutations.css_filter.value(property).map(|f| json!(f))
                }
                "backdropBlur" | "backdropBlurSigma" => {
                    mutations.backdrop_blur_sigma.map(|f| json!(f))
                }
                // colors (serialize as "#RRGGBBAA" or "transparent")
                "fillColor" => mutations.fill_color.as_ref().map(|c| {
                    let (r, g, b, a) = c.rgba();
                    json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
                }),
                "strokeColor" => mutations.stroke_color.as_ref().map(|c| {
                    let (r, g, b, a) = c.rgba();
                    json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
                }),
                "backgroundColor" | "bg" => mutations.bg_color.as_ref().map(|c| {
                    let (r, g, b, a) = c.rgba();
                    json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
                }),
                "textColor" => mutations.text_color.as_ref().map(|c| {
                    let (r, g, b, a) = c.rgba();
                    json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
                }),
                "borderColor" => mutations.border_color.as_ref().map(|c| {
                    let (r, g, b, a) = c.rgba();
                    json!(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
                }),
                // strings
                "morphSVG" | "d" => mutations.svg_path.as_ref().map(|s| json!(s)),
                "text" => mutations.text_content.as_ref().map(|s| json!(s)),
                _ => None,
            };
            if v.is_some() {
                return v;
            }
        }
        // 2. Fall back to initial styles (Tailwind base values)
        self.initial_styles
            .get(id)
            .and_then(|props| props.get(property).cloned())
    }

    /// Unified write: set any animatable property from (id, property_name, JSON value).
    /// Centralizes all property→field mapping in one place.
    pub fn write_style_value(&mut self, id: &str, property: &str, value: serde_json::Value) {
        let entry = self.styles.entry(id.to_string()).or_default();
        match property {
            // ── f32 ──
            "opacity" => {
                entry.opacity = value.as_f64().map(|v| v as f32);
            }
            "translateX" => {
                if let Some(v) = value.as_f64() {
                    entry
                        .transforms
                        .push(Transform::TranslateX { value: v as f32 });
                }
            }
            "translateY" => {
                if let Some(v) = value.as_f64() {
                    entry
                        .transforms
                        .push(Transform::TranslateY { value: v as f32 });
                }
            }
            "scale" => {
                if let Some(v) = value.as_f64() {
                    entry.transforms.push(Transform::Scale { value: v as f32 });
                }
            }
            "scaleX" => {
                if let Some(v) = value.as_f64() {
                    entry.transforms.push(Transform::ScaleX { value: v as f32 });
                }
            }
            "scaleY" => {
                if let Some(v) = value.as_f64() {
                    entry.transforms.push(Transform::ScaleY { value: v as f32 });
                }
            }
            "rotate" | "rotation" => {
                if let Some(v) = value.as_f64() {
                    entry
                        .transforms
                        .push(Transform::RotateDeg { value: v as f32 });
                }
            }
            "skewX" => {
                if let Some(v) = value.as_f64() {
                    entry
                        .transforms
                        .push(Transform::SkewXDeg { value: v as f32 });
                }
            }
            "skewY" => {
                if let Some(v) = value.as_f64() {
                    entry
                        .transforms
                        .push(Transform::SkewYDeg { value: v as f32 });
                }
            }
            "left" => {
                entry.inset_left = value.as_f64().map(|v| v as f32);
            }
            "top" => {
                entry.inset_top = value.as_f64().map(|v| v as f32);
            }
            "right" => {
                entry.inset_right = value.as_f64().map(|v| v as f32);
            }
            "bottom" => {
                entry.inset_bottom = value.as_f64().map(|v| v as f32);
            }
            "width" => {
                entry.width = value.as_f64().map(|v| v as f32);
            }
            "height" => {
                entry.height = value.as_f64().map(|v| v as f32);
            }
            "padding" => {
                entry.padding = value.as_f64().map(|v| v as f32);
            }
            "paddingX" => {
                entry.padding_x = value.as_f64().map(|v| v as f32);
            }
            "paddingY" => {
                entry.padding_y = value.as_f64().map(|v| v as f32);
            }
            "margin" => {
                entry.margin = value.as_f64().map(|v| v as f32);
            }
            "marginX" => {
                entry.margin_x = value.as_f64().map(|v| v as f32);
            }
            "marginY" => {
                entry.margin_y = value.as_f64().map(|v| v as f32);
            }
            "gap" => {
                entry.gap = value.as_f64().map(|v| v as f32);
            }
            "flexGrow" => {
                entry.flex_grow = value.as_f64().map(|v| v as f32);
            }
            "borderRadius" => {
                entry.border_radius = value.as_f64().map(|v| v as f32);
            }
            "borderWidth" => {
                entry.border_width = value.as_f64().map(|v| v as f32);
            }
            "borderTopWidth" => {
                entry.border_top_width = value.as_f64().map(|v| v as f32);
            }
            "borderRightWidth" => {
                entry.border_right_width = value.as_f64().map(|v| v as f32);
            }
            "borderBottomWidth" => {
                entry.border_bottom_width = value.as_f64().map(|v| v as f32);
            }
            "borderLeftWidth" => {
                entry.border_left_width = value.as_f64().map(|v| v as f32);
            }
            "strokeWidth" => {
                entry.stroke_width = value.as_f64().map(|v| v as f32).map(|v| v.max(0.0));
            }
            "strokeDasharray" => {
                entry.stroke_dasharray = value.as_f64().map(|v| v as f32).map(|v| v.max(0.0));
            }
            "strokeDashoffset" => {
                entry.stroke_dashoffset = value.as_f64().map(|v| v as f32);
            }
            "textSize" => {
                entry.text_px = value.as_f64().map(|v| v as f32);
            }
            "letterSpacing" => {
                entry.letter_spacing = value.as_f64().map(|v| v as f32);
            }
            "lineHeight" => {
                entry.line_height = value.as_f64().map(|v| v as f32);
            }
            "fontWeight" => {
                entry.font_weight = value.as_f64().map(|v| FontWeight(v as u16));
            }
            "filter" => {
                if let Some(filter) = css_filter_from_value(&value) {
                    entry.css_filter = filter;
                }
            }
            "clipPath" | "clip-path" => {
                if let Some(clip_path) = clip_path_from_value(&value) {
                    entry.clip_path = clip_path;
                }
            }
            "blur" | "blurSigma" | "brightness" | "contrast" | "grayscale" | "hueRotate"
            | "invert" | "saturate" | "sepia" => {
                if let Some(v) = value.as_f64() {
                    entry.css_filter.set_property(property, v as f32);
                }
            }
            "backdropBlur" | "backdropBlurSigma" => {
                entry.backdrop_blur_sigma = value.as_f64().map(|v| v as f32);
            }

            // ── colors ──
            "bg" | "backgroundColor" => {
                if let Some(s) = value.as_str() {
                    entry.bg_color = color_token_from_script_string(s);
                }
            }
            "fillColor" => {
                if let Some(s) = value.as_str() {
                    entry.fill_color = color_token_from_script_string(s);
                }
            }
            "strokeColor" => {
                if let Some(s) = value.as_str() {
                    entry.stroke_color = color_token_from_script_string(s);
                }
            }
            "textColor" => {
                if let Some(s) = value.as_str() {
                    entry.text_color = color_token_from_script_string(s);
                }
            }
            "borderColor" => {
                if let Some(s) = value.as_str() {
                    entry.border_color = color_token_from_script_string(s);
                }
            }
            "shadowColor" => {
                if let Some(s) = value.as_str() {
                    entry.box_shadow_color = color_token_from_script_string(s);
                }
            }
            "insetShadowColor" => {
                if let Some(s) = value.as_str() {
                    entry.inset_shadow_color = color_token_from_script_string(s);
                }
            }
            "dropShadowColor" => {
                if let Some(s) = value.as_str() {
                    entry.drop_shadow_color = color_token_from_script_string(s);
                }
            }

            // ── enums ──
            "position" => {
                if let Some(s) = value.as_str() {
                    entry.position = position_from_name(s);
                }
            }
            "flexDirection" => {
                if let Some(s) = value.as_str() {
                    entry.flex_direction = flex_direction_from_name(s);
                }
            }
            "justifyContent" => {
                if let Some(s) = value.as_str() {
                    entry.justify_content = justify_content_from_name(s);
                }
            }
            "alignItems" => {
                if let Some(s) = value.as_str() {
                    entry.align_items = align_items_from_name(s);
                }
            }
            "objectFit" => {
                if let Some(s) = value.as_str() {
                    entry.object_fit = object_fit_from_name(s);
                }
            }
            "borderStyle" => {
                if let Some(s) = value.as_str() {
                    entry.border_style = match s {
                        "solid" => Some(BorderStyle::Solid),
                        "dashed" => Some(BorderStyle::Dashed),
                        "dotted" => Some(BorderStyle::Dotted),
                        _ => None,
                    };
                }
            }
            "textAlign" => {
                if let Some(s) = value.as_str() {
                    entry.text_align = text_align_from_name(s);
                }
            }

            // ── strings ──
            "text" => {
                if let Some(s) = value.as_str() {
                    entry.text_content = Some(s.to_string());
                }
            }
            "morphSVG" | "d" => {
                if let Some(s) = value.as_str() {
                    entry.svg_path = Some(s.to_string());
                }
            }
            "shadow" => {
                if let Some(s) = value.as_str() {
                    if let Some(sh) = box_shadow_from_name(s) {
                        entry.box_shadow = Some(sh);
                    }
                }
            }
            "insetShadow" => {
                if let Some(s) = value.as_str() {
                    if let Some(sh) = inset_shadow_from_name(s) {
                        entry.inset_shadow = Some(sh);
                    }
                }
            }
            "dropShadow" => {
                if let Some(s) = value.as_str() {
                    if let Some(sh) = drop_shadow_from_name(s) {
                        entry.drop_shadow = Some(sh);
                    }
                }
            }

            // ── multi-arg as array ──
            "translate" => {
                if let Some(arr) = value.as_array() {
                    if let (Some(x), Some(y)) = (
                        arr.first().and_then(|v| v.as_f64()),
                        arr.get(1).and_then(|v| v.as_f64()),
                    ) {
                        entry
                            .transforms
                            .push(Transform::TranslateX { value: x as f32 });
                        entry
                            .transforms
                            .push(Transform::TranslateY { value: y as f32 });
                    }
                }
            }
            "skew" => {
                if let Some(arr) = value.as_array() {
                    if let (Some(x), Some(y)) = (
                        arr.first().and_then(|v| v.as_f64()),
                        arr.get(1).and_then(|v| v.as_f64()),
                    ) {
                        entry
                            .transforms
                            .push(Transform::SkewXDeg { value: x as f32 });
                        entry
                            .transforms
                            .push(Transform::SkewYDeg { value: y as f32 });
                    }
                }
            }

            _ => {}
        }
    }

    // ── Animate ──

    pub fn animate_create(
        &mut self,
        current_frame: u32,
        duration: f32,
        delay: f32,
        clamp: bool,
        easing_tag: &str,
        repeat: i32,
        yoyo: bool,
        repeat_delay: f32,
    ) -> i32 {
        let easing = crate::script::animate::state::parse_easing_from_tag(easing_tag);
        let fps = self.fps.max(1) as f32;
        let duration_u32 = if duration < 0.0 {
            easing.default_duration(fps).unwrap_or(1)
        } else {
            self.seconds_to_frames(duration)
        };
        let delay_u32 = self.seconds_to_frames(delay);
        let repeat_delay_u32 = self.seconds_to_frames(repeat_delay.max(0.0));
        let progress = crate::parse::easing::compute_progress(
            current_frame,
            duration_u32,
            delay_u32,
            &easing,
            clamp,
            repeat,
            yoyo,
            repeat_delay_u32,
        );
        let total_frames = if repeat >= 0 {
            duration_u32
                .saturating_mul(repeat as u32 + 1)
                .saturating_add(repeat_delay_u32.saturating_mul(repeat as u32))
        } else {
            u32::MAX
        };
        let settled = repeat >= 0 && current_frame >= delay_u32.saturating_add(total_frames);
        let settle_frame = delay_u32.saturating_add(total_frames);
        let handle = self.animate_next_id;
        self.animate_next_id += 1;
        self.animate_entries.insert(
            handle,
            AnimateEntry {
                progress,
                settled,
                settle_frame,
                duration: duration_u32,
                delay: delay_u32,
                clamp,
                easing,
                repeat,
                yoyo,
                repeat_delay: repeat_delay_u32,
            },
        );
        handle
    }

    pub fn animate_value(&self, current_frame: u32, handle: i32, from: f32, to: f32) -> f32 {
        if let Some(entry) = self.animate_entries.get(&handle) {
            crate::parse::easing::animate_value(
                current_frame,
                entry.duration,
                entry.delay,
                from,
                to,
                &entry.easing,
                entry.clamp,
                entry.repeat,
                entry.yoyo,
                entry.repeat_delay,
            )
        } else {
            from
        }
    }

    pub fn animate_color(&self, handle: i32, from: &str, to: &str) -> String {
        let Some(entry) = self.animate_entries.get(&handle) else {
            return from.to_string();
        };
        match (parse_color(from), parse_color(to)) {
            (Some(f), Some(t)) => {
                let result = lerp_hsla_clamped(&f, &t, entry.progress);
                hsla_to_rgba_string(&result)
            }
            _ => from.to_string(),
        }
    }

    pub fn animate_progress(&self, handle: i32) -> f32 {
        self.animate_entries
            .get(&handle)
            .map(|e| e.progress)
            .unwrap_or(0.0)
    }

    pub fn animate_settled(&self, handle: i32) -> bool {
        self.animate_entries
            .get(&handle)
            .map(|e| e.settled)
            .unwrap_or(false)
    }

    pub fn animate_settle_frame(&self, handle: i32) -> u32 {
        self.animate_entries
            .get(&handle)
            .map(|e| e.settle_frame)
            .unwrap_or(0)
    }

    // ── Morph SVG ──

    pub fn morph_svg_create(&mut self, from_svg: &str, to_svg: &str, grid: u32) -> Option<i32> {
        let entry = MorphSvgEntry::new(from_svg, to_svg, grid)?;
        let handle = self.morph_next_id;
        self.morph_next_id += 1;
        self.morph_entries.insert(handle, entry);
        Some(handle)
    }

    pub fn morph_svg_sample(&self, handle: i32, t: f32, tolerance: f32) -> String {
        self.morph_entries
            .get(&handle)
            .map(|e| e.sample(t, tolerance))
            .unwrap_or_default()
    }

    pub fn morph_svg_dispose(&mut self, handle: i32) {
        self.morph_entries.remove(&handle);
    }

    // ── Along Path ──

    pub fn along_path_create(&mut self, svg: &str) -> Option<i32> {
        let entry = PathMeasureEntry::from_svg(svg)?;
        let handle = self.path_next_id;
        self.path_next_id += 1;
        self.path_entries.insert(handle, entry);
        Some(handle)
    }

    pub fn along_path_length(&self, handle: i32) -> f32 {
        self.path_entries
            .get(&handle)
            .map(|e| e.total_length)
            .unwrap_or(0.0)
    }

    pub fn along_path_at(&self, handle: i32, t: f32) -> (f32, f32, f32) {
        self.path_entries
            .get(&handle)
            .map(|e| e.sample(t))
            .unwrap_or((0.0, 0.0, 0.0))
    }

    pub fn along_path_dispose(&mut self, handle: i32) {
        self.path_entries.remove(&handle);
    }
}

impl MutationRecorder for MutationStore {
    fn record_opacity(&mut self, id: &str, v: f32) {
        self.entry(id).opacity = Some(v);
    }
    fn record_translate(&mut self, id: &str, x: f32, y: f32) {
        self.entry(id)
            .transforms
            .push(Transform::Translate { x, y });
    }
    fn record_translate_x(&mut self, id: &str, v: f32) {
        self.entry(id)
            .transforms
            .push(Transform::TranslateX { value: v });
    }
    fn record_translate_y(&mut self, id: &str, v: f32) {
        self.entry(id)
            .transforms
            .push(Transform::TranslateY { value: v });
    }
    fn record_scale(&mut self, id: &str, v: f32) {
        self.entry(id)
            .transforms
            .push(Transform::Scale { value: v });
    }
    fn record_scale_x(&mut self, id: &str, v: f32) {
        self.entry(id)
            .transforms
            .push(Transform::ScaleX { value: v });
    }
    fn record_scale_y(&mut self, id: &str, v: f32) {
        self.entry(id)
            .transforms
            .push(Transform::ScaleY { value: v });
    }
    fn record_rotate(&mut self, id: &str, deg: f32) {
        self.entry(id)
            .transforms
            .push(Transform::RotateDeg { value: deg });
    }
    fn record_skew_x(&mut self, id: &str, deg: f32) {
        self.entry(id)
            .transforms
            .push(Transform::SkewXDeg { value: deg });
    }
    fn record_skew_y(&mut self, id: &str, deg: f32) {
        self.entry(id)
            .transforms
            .push(Transform::SkewYDeg { value: deg });
    }
    fn record_skew(&mut self, id: &str, x_deg: f32, y_deg: f32) {
        self.entry(id)
            .transforms
            .push(Transform::SkewDeg { x: x_deg, y: y_deg });
    }
    fn record_left(&mut self, id: &str, v: f32) {
        self.entry(id).inset_left = Some(v);
    }
    fn record_top(&mut self, id: &str, v: f32) {
        self.entry(id).inset_top = Some(v);
    }
    fn record_right(&mut self, id: &str, v: f32) {
        self.entry(id).inset_right = Some(v);
    }
    fn record_bottom(&mut self, id: &str, v: f32) {
        self.entry(id).inset_bottom = Some(v);
    }
    fn record_width(&mut self, id: &str, v: f32) {
        self.entry(id).width = Some(v);
    }
    fn record_height(&mut self, id: &str, v: f32) {
        self.entry(id).height = Some(v);
    }
    fn record_padding(&mut self, id: &str, v: f32) {
        self.entry(id).padding = Some(v);
    }
    fn record_padding_x(&mut self, id: &str, v: f32) {
        self.entry(id).padding_x = Some(v);
    }
    fn record_padding_y(&mut self, id: &str, v: f32) {
        self.entry(id).padding_y = Some(v);
    }
    fn record_margin(&mut self, id: &str, v: f32) {
        self.entry(id).margin = Some(v);
    }
    fn record_margin_x(&mut self, id: &str, v: f32) {
        self.entry(id).margin_x = Some(v);
    }
    fn record_margin_y(&mut self, id: &str, v: f32) {
        self.entry(id).margin_y = Some(v);
    }
    fn record_gap(&mut self, id: &str, v: f32) {
        self.entry(id).gap = Some(v);
    }
    fn record_flex_grow(&mut self, id: &str, v: f32) {
        self.entry(id).flex_grow = Some(v);
    }
    fn record_border_radius(&mut self, id: &str, v: f32) {
        self.entry(id).border_radius = Some(v);
    }
    fn record_border_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_width = Some(v);
    }
    fn record_border_top_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_top_width = Some(v);
    }
    fn record_border_right_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_right_width = Some(v);
    }
    fn record_border_bottom_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_bottom_width = Some(v);
    }
    fn record_border_left_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_left_width = Some(v);
    }
    fn record_stroke_width(&mut self, id: &str, v: f32) {
        self.entry(id).stroke_width = Some(v.max(0.0));
    }
    fn record_stroke_dasharray(&mut self, id: &str, v: f32) {
        self.entry(id).stroke_dasharray = Some(v.max(0.0));
    }
    fn record_stroke_dashoffset(&mut self, id: &str, v: f32) {
        self.entry(id).stroke_dashoffset = Some(v);
    }
    fn record_text_size(&mut self, id: &str, v: f32) {
        self.entry(id).text_px = Some(v);
    }
    fn record_letter_spacing(&mut self, id: &str, v: f32) {
        self.entry(id).letter_spacing = Some(v);
    }
    fn record_line_height(&mut self, id: &str, v: f32) {
        self.entry(id).line_height = Some(v);
    }

    fn record_position(&mut self, id: &str, pos: Position) {
        self.entry(id).position = Some(pos);
    }
    fn record_flex_direction(&mut self, id: &str, fd: FlexDirection) {
        self.entry(id).flex_direction = Some(fd);
    }
    fn record_justify_content(&mut self, id: &str, jc: JustifyContent) {
        self.entry(id).justify_content = Some(jc);
    }
    fn record_align_items(&mut self, id: &str, ai: AlignItems) {
        self.entry(id).align_items = Some(ai);
    }
    fn record_object_fit(&mut self, id: &str, of: ObjectFit) {
        self.entry(id).object_fit = Some(of);
    }
    fn record_text_align(&mut self, id: &str, ta: TextAlign) {
        self.entry(id).text_align = Some(ta);
    }
    fn record_border_style(&mut self, id: &str, bs: BorderStyle) {
        self.entry(id).border_style = Some(bs);
    }
    fn record_font_weight(&mut self, id: &str, w: FontWeight) {
        self.entry(id).font_weight = Some(w);
    }
    fn record_box_shadow(&mut self, id: &str, sh: BoxShadow) {
        self.entry(id).box_shadow = Some(sh);
    }
    fn record_inset_shadow(&mut self, id: &str, sh: InsetShadow) {
        self.entry(id).inset_shadow = Some(sh);
    }
    fn record_drop_shadow(&mut self, id: &str, sh: DropShadow) {
        self.entry(id).drop_shadow = Some(sh);
    }

    fn record_bg_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).bg_color = Some(color);
    }
    fn record_fill_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).fill_color = Some(color);
    }
    fn record_stroke_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).stroke_color = Some(color);
    }
    fn record_border_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).border_color = Some(color);
    }
    fn record_text_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).text_color = Some(color);
    }
    fn record_box_shadow_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).box_shadow_color = Some(color);
    }
    fn record_inset_shadow_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).inset_shadow_color = Some(color);
    }
    fn record_drop_shadow_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).drop_shadow_color = Some(color);
    }

    fn record_transform(&mut self, id: &str, t: Transform) {
        self.entry(id).transforms.push(t);
    }

    fn record_text_content(&mut self, id: &str, text: String) {
        self.entry(id).text_content = Some(text.clone());
        self.text_sources.insert(
            id.to_string(),
            ScriptTextSource {
                text,
                kind: crate::script::ScriptTextSourceKind::TextNode,
            },
        );
    }

    fn record_text_unit_override(
        &mut self,
        id: &str,
        granularity: TextUnitGranularity,
        index: usize,
        values: TextUnitValues,
    ) {
        let mutations = self.entry(id);
        match &mut mutations.text_unit_overrides {
            Some(batch) => {
                if batch.granularity != granularity {
                    return;
                }
                if index >= batch.overrides.len() {
                    batch
                        .overrides
                        .resize_with(index + 1, TextUnitOverride::default);
                }
            }
            None => {
                let mut batch = TextUnitOverrideBatch {
                    granularity,
                    overrides: Vec::new(),
                };
                batch
                    .overrides
                    .resize_with(index + 1, TextUnitOverride::default);
                mutations.text_unit_overrides = Some(batch);
            }
        }
        let entry = &mut mutations.text_unit_overrides.as_mut().unwrap().overrides[index];
        if let Some(v) = values.opacity {
            entry.opacity = Some(v);
        }
        if let Some(v) = values.translate_x {
            entry.translate_x = Some(v);
        }
        if let Some(v) = values.translate_y {
            entry.translate_y = Some(v);
        }
        if let Some(v) = values.scale {
            entry.scale = Some(v);
        }
        if let Some(v) = values.rotation_deg {
            entry.rotation_deg = Some(v);
        }
        if let Some(c) = values.color {
            entry.color = Some(c);
        }
    }

    fn record_svg_path(&mut self, id: &str, data: String) {
        self.entry(id).svg_path = Some(data);
    }

    fn record_draw_op(&mut self, id: &str, cmd: crate::ir::draw_op::DrawOp) {
        self.canvas_entry(id).commands.push(cmd);
    }

    fn record_draw_picture(&mut self, target_id: &str, owner_id: &str, x: f32, y: f32) {
        self.record_draw_op(
            target_id,
            crate::ir::draw_op::DrawOp::DrawSubtreePicture {
                owner_id: owner_id.to_string(),
                x,
                y,
            },
        );
    }

    fn record_canvas_runtime_effect(
        &mut self,
        id: &str,
        sksl: String,
        uniforms_bytes: Vec<u8>,
        children: Vec<crate::ir::draw_types::ScriptRuntimeEffectChild>,
        dst: crate::ir::draw_op::Rect4,
    ) {
        self.record_draw_op(
            id,
            crate::ir::draw_op::DrawOp::ScriptRuntimeEffect {
                sksl,
                uniforms_bytes,
                children,
                dst,
            },
        );
    }

    fn reset_for_frame(&mut self, current_frame: u32, fps: u32) {
        self.styles.clear();
        self.canvases.clear();
        self.animate_entries.clear();
        self.animate_next_id = 0;
        self.current_frame = current_frame;
        self.fps = fps.max(1);
    }

    fn snapshot_mutations(&self) -> StyleMutations {
        StyleMutations {
            mutations: self.styles.clone(),
            canvas_mutations: self.canvases.clone(),
        }
    }

    fn read_style_value(&self, id: &str, property: &str) -> Option<serde_json::Value> {
        self.read_style_value(id, property)
    }

    fn write_style_value(&mut self, id: &str, property: &str, value: serde_json::Value) {
        self.write_style_value(id, property, value)
    }

    fn register_text_source(&mut self, id: &str, source: ScriptTextSource) {
        self.text_sources.insert(id.to_string(), source);
    }
    fn clear_text_sources(&mut self) {
        self.text_sources.clear();
    }
    fn get_text_source(&self, id: &str) -> Option<&ScriptTextSource> {
        self.text_sources.get(id)
    }
    fn current_frame(&self) -> u32 {
        self.current_frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::ColorToken;

    #[test]
    fn record_opacity_writes_into_styles() {
        let mut store = MutationStore::default();
        store.record_opacity("node-a", 0.7);
        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("node-a").expect("node-a recorded");
        assert_eq!(entry.opacity, Some(0.7));
    }

    #[test]
    fn record_translate_pushes_transform() {
        let mut store = MutationStore::default();
        store.record_translate("node-a", 12.0, -8.0);
        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("node-a").expect("node-a recorded");
        assert_eq!(entry.transforms.len(), 1);
    }

    #[test]
    fn record_bg_color_uses_color_token() {
        let mut store = MutationStore::default();
        store.record_bg_color("node-a", ColorToken::Custom(255, 0, 0, 255));
        let snap = store.snapshot_mutations();
        assert_eq!(
            snap.mutations.get("node-a").unwrap().bg_color,
            Some(ColorToken::Custom(255, 0, 0, 255))
        );
    }

    #[test]
    fn snapshot_does_not_clear_styles() {
        let mut store = MutationStore::default();
        store.record_opacity("node-a", 0.5);
        let _ = store.snapshot_mutations();
        let snap = store.snapshot_mutations();
        assert!(snap.mutations.contains_key("node-a"));
    }

    #[test]
    fn reset_for_frame_clears_styles_and_canvases() {
        let mut store = MutationStore::default();
        store.record_opacity("node-a", 0.5);
        store.reset_for_frame(7, 30);
        let snap = store.snapshot_mutations();
        assert!(snap.mutations.is_empty());
    }

    #[test]
    fn css_filter_values_round_trip_through_value_api() {
        let mut store = MutationStore::default();

        store.write_style_value("node-a", "brightness", json!(0.5));
        store.write_style_value("node-a", "hueRotate", json!(45.0));
        store.write_style_value("node-a", "blur", json!(6.0));

        assert_eq!(
            store.read_style_value("node-a", "brightness"),
            Some(json!(0.5))
        );
        assert_eq!(
            store.read_style_value("node-a", "hueRotate"),
            Some(json!(45.0))
        );
        assert_eq!(store.read_style_value("node-a", "blur"), Some(json!(6.0)));

        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("node-a").expect("node-a recorded");
        assert_eq!(entry.css_filter.value("brightness"), Some(0.5));
        assert_eq!(entry.css_filter.value("hueRotate"), Some(45.0));
        assert_eq!(entry.css_filter.value("blur"), Some(6.0));
    }

    #[test]
    fn css_filter_string_preserves_operation_order() {
        let mut store = MutationStore::default();

        store.write_style_value("node-a", "filter", json!("brightness(0.5) contrast(2)"));
        store.write_style_value("node-b", "filter", json!("contrast(2) brightness(0.5)"));

        let snap = store.snapshot_mutations();
        let a = &snap.mutations.get("node-a").unwrap().css_filter;
        let b = &snap.mutations.get("node-b").unwrap().css_filter;
        assert_ne!(a, b, "CSS filter order is semantically significant");
        assert_eq!(
            a.to_css_string(),
            "brightness(0.5) contrast(2)",
            "filter string order must round-trip"
        );
        assert_eq!(
            b.to_css_string(),
            "contrast(2) brightness(0.5)",
            "filter string order must round-trip"
        );
    }

    #[test]
    fn css_filter_ordered_value_preserves_operation_order() {
        let mut store = MutationStore::default();

        store.write_style_value(
            "node-a",
            "filter",
            json!({
                "ops": [
                    { "kind": "brightness", "value": 0.5 },
                    { "kind": "contrast", "value": 2.0 },
                    { "kind": "brightness", "value": 0.75 }
                ]
            }),
        );

        assert_eq!(
            store.read_style_value("node-a", "filter"),
            Some(json!({
                "ops": [
                    { "kind": "brightness", "value": 0.5 },
                    { "kind": "contrast", "value": 2.0 },
                    { "kind": "brightness", "value": 0.75 }
                ]
            }))
        );
        let snap = store.snapshot_mutations();
        let filter = &snap.mutations.get("node-a").unwrap().css_filter;
        assert_eq!(
            filter.to_css_string(),
            "brightness(0.5) contrast(2) brightness(0.75)"
        );
        assert_eq!(filter.value("brightness"), Some(0.75));
    }

    #[test]
    fn record_draw_picture_stores_subtree_reference() {
        let mut store = MutationStore::default();
        store.record_draw_picture("stage", "stage", 4.0, 5.0);

        let snap = store.snapshot_mutations();
        let commands = &snap.canvas_mutations.get("stage").unwrap().commands;
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            crate::ir::draw_op::DrawOp::DrawSubtreePicture { owner_id, x, y } => {
                assert_eq!(owner_id, "stage");
                assert_eq!(*x, 4.0);
                assert_eq!(*y, 5.0);
            }
            other => panic!("expected DrawSubtreePicture, got {:?}", other),
        }
    }

    #[test]
    fn record_canvas_runtime_effect_pushes_script_effect() {
        use crate::ir::draw_op::Rect4;
        use crate::ir::draw_types::{ImageRef, ScriptRuntimeEffectChild};
        let mut store = MutationStore::default();
        store.record_canvas_runtime_effect(
            "s1-canvas",
            "half4 main(float2 p){return half4(1);}".to_string(),
            vec![1u8, 2, 3, 4, 5, 6, 7, 8],
            vec![ScriptRuntimeEffectChild::Image(ImageRef::Static {
                asset_id: "decor".into(),
            })],
            Rect4 {
                x: 0.0,
                y: 0.0,
                width: 360.0,
                height: 480.0,
            },
        );

        let snap = store.snapshot_mutations();
        let commands = &snap.canvas_mutations.get("s1-canvas").unwrap().commands;
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            crate::ir::draw_op::DrawOp::ScriptRuntimeEffect {
                sksl,
                uniforms_bytes,
                children,
                dst,
            } => {
                assert!(sksl.contains("half4"));
                assert_eq!(uniforms_bytes.len(), 8);
                assert_eq!(children.len(), 1);
                assert_eq!(dst.width, 360.0);
            }
            other => panic!("expected ScriptRuntimeEffect, got {:?}", other),
        }
    }

    #[test]
    fn dispatch_canvas_runtime_effect_draw_records_script_effect() {
        use crate::script::dispatch::dispatch_binding;
        use serde_json::json;
        let mut store = MutationStore::default();
        let args = vec![
            json!("s1-canvas"),
            json!("half4 main(float2 p){return half4(1);}"),
            json!([0.5, 1.0, 0.0, 0.0]),
            json!(
                r#"[{"__opencatShader":"image","assetId":"decor","tileX":"clamp","tileY":"clamp"}]"#
            ),
            json!(0.0),
            json!(0.0),
            json!(360.0),
            json!(480.0),
        ];
        dispatch_binding(&mut store, "canvas_runtime_effect_draw", &args)
            .expect("binding should dispatch");
        let snap = store.snapshot_mutations();
        let commands = &snap.canvas_mutations.get("s1-canvas").unwrap().commands;
        assert_eq!(commands.len(), 1);
        matches!(
            commands[0],
            crate::ir::draw_op::DrawOp::ScriptRuntimeEffect { .. }
        );
    }
}
