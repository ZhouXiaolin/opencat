use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::media::{VideoColorMatrix, VideoFrameTiming};
use crate::parse::{
    composition::{AudioAttachment, CompositionAudioSource},
    document::{
        ParsedAudioElement, ParsedComposition, ParsedElement, ParsedElementKind, ParsedTransition,
    },
    primitives::{AudioSource, ImageSource, OpenverseQuery, VideoSource},
};
use crate::style::NodeStyle;


use crate::parse::document::{
    DeclaredScript, build_tree, build_tree_with_tl, join_scripts, script_driver_from_decls,
    validate_unique_ids,
};

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(tag = "type")]
pub enum JsonLine {
    #[serde(rename = "composition")]
    Composition {
        width: i32,
        height: i32,
        fps: i32,
        duration: f64,
    },
    #[serde(rename = "script")]
    Script {
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(alias = "content")]
        src: Option<String>,
        path: Option<String>,
    },
    #[serde(rename = "div")]
    Div {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "text")]
    Text {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        text: String,
        duration: Option<f64>,
    },
    #[serde(rename = "canvas")]
    Canvas {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "image")]
    Image {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        path: Option<String>,
        url: Option<String>,
        query: Option<String>,
        #[serde(rename = "queryCount")]
        query_count: Option<usize>,
        #[serde(rename = "aspectRatio")]
        aspect_ratio: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "audio")]
    Audio {
        id: String,
        attach: String,
        #[allow(dead_code)]
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        path: Option<String>,
        url: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "video")]
    Video {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        path: Option<String>,
        url: Option<String>,
        duration: Option<f64>,
        /// Media window, timing and color-matrix overrides, mirroring the
        /// markup `data-*` attributes on `<video>`. Both the kebab-case
        /// markup spelling and the camelCase JSX spelling are accepted.
        /// All optional; absent fields keep the legacy behaviour (window
        /// opens at 0s, auto color matrix, node holds its last frame).
        #[serde(rename = "data-start", alias = "dataStart")]
        data_start: Option<f64>,
        #[serde(rename = "data-duration", alias = "dataDuration")]
        data_duration: Option<f64>,
        #[serde(rename = "data-media-start", alias = "dataMediaStart")]
        data_media_start: Option<f64>,
        #[serde(rename = "data-color-matrix", alias = "dataColorMatrix")]
        data_color_matrix: Option<String>,
        #[serde(rename = "data-hide-after-end")]
        hide_after_end: Option<bool>,
        #[serde(rename = "loop")]
        looping: Option<bool>,
    },
    #[serde(rename = "icon")]
    Icon {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        icon: String,
        duration: Option<f64>,
    },
    #[serde(rename = "transition")]
    Transition {
        #[serde(rename = "parentId")]
        parent_id: String,
        from: String,
        to: String,
        effect: String,
        duration: f64,
        direction: Option<String>,
        timing: Option<String>,
        damping: Option<f32>,
        stiffness: Option<f32>,
        mass: Option<f32>,
        seed: Option<f32>,
        #[serde(rename = "hueShift")]
        hue_shift: Option<f32>,
        #[serde(rename = "maskScale")]
        mask_scale: Option<f32>,
    },
    #[serde(rename = "tl")]
    Tl {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        /// A `<tl>` is a scene like any other element, so it declares its own
        /// length. Required when the timeline is a direct child of another
        /// timeline — the enclosing sequence reads each child's duration to
        /// place it. Inert when the timeline is a plain child of a div.
        duration: Option<f64>,
    },
    #[serde(rename = "caption")]
    Caption {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        path: String,
        duration: Option<f64>,
    },
    #[serde(rename = "path")]
    Path {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        d: String,
        /// Optional shared SVG view box, `"minx,miny,w,h"` — same grammar as the
        /// markup `view-box` attribute.
        #[serde(rename = "viewBox")]
        view_box: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "lottie")]
    Lottie {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        path: Option<String>,
        url: Option<String>,
        duration: Option<f64>,
    },
}

/// Test-only line-text helper: core unit tests describe fixtures in the line
/// shape without a product text frontend. Decodes text lines into
/// [`JsonLine`] and runs the shared assembly.
#[cfg(test)]
pub(crate) fn parse_lines_for_tests(
    input: &str,
    base_dir: Option<&Path>,
) -> anyhow::Result<ParsedComposition> {
    let lines = input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<JsonLine>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    // 测试行里的 className 与生产一致：交给标准 CSS 引擎。夹具样式表是
    // 布局/解析测试用到的类经 tailwindcss CLI 预编译的产物。
    let engine = crate::css::CssEngine::from_css_text(
        crate::test_support::layout_test_css(),
        crate::css::EvalContext::default(),
    )
    .expect("test css compiles");
    composition_from_lines(lines, base_dir, None, Some(&engine))
}

/// Shared assembly for the line shape ([`JsonLine`]): the JSX frontend's
/// extracted tree arrives as a `Vec<JsonLine>` and lowers here directly.
/// `line_index` (1-based to the Tailwind diagnostics) counts the lines as
/// given, so the JSX frontend keeps its composition line at index 0 exactly
/// like a text document would.
pub(crate) fn composition_from_lines(
    lines: Vec<JsonLine>,
    base_dir: Option<&Path>,
    manifest: Option<&crate::fonts::FontManifest>,
    css: Option<&crate::css::CssEngine>,
) -> anyhow::Result<ParsedComposition> {
    let mut width = 1920;
    let mut height = 1080;
    let mut fps = 30;
    let mut duration = 3.0;
    let mut global_scripts: Vec<DeclaredScript> = Vec::new();
    let mut audio_elements = Vec::new();
    let mut scripts_by_parent: HashMap<String, Vec<DeclaredScript>> = HashMap::new();
    let mut elements: Vec<ParsedElement> = Vec::new();
    let mut transitions = Vec::new();
    let mut timeline_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (line_index, line) in lines.into_iter().enumerate() {
        match line {
            JsonLine::Composition {
                width: w,
                height: h,
                fps: f,
                duration: d,
            } => {
                width = w;
                height = h;
                fps = f;
                duration = d;
            }
            JsonLine::Script {
                parent_id,
                src,
                path,
            } => {
                let decl = resolve_script_declaration(src, path)?;
                if let Some(parent_id) = parent_id {
                    scripts_by_parent.entry(parent_id).or_default().push(decl);
                } else {
                    global_scripts.push(decl);
                }
            }
            JsonLine::Div {
                id,
                parent_id,
                class_name,
                style: inline_style,
                duration,
            } => {
                let style = resolve_style(
                    css,
                    class_name.as_deref().unwrap_or(""),
                    inline_style.as_deref(),
                    &id,
                    "div",
                    line_index + 1,
                );
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Div,
                });
            }
            JsonLine::Text {
                id,
                parent_id,
                class_name,
                style: inline_style,
                text,
                duration,
            } => {
                let style = resolve_style(
                    css,
                    class_name.as_deref().unwrap_or(""),
                    inline_style.as_deref(),
                    &id,
                    "text",
                    line_index + 1,
                );
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Text { content: text },
                });
            }
            JsonLine::Canvas {
                id,
                parent_id,
                class_name,
                style: inline_style,
                duration,
            } => {
                let style = resolve_style(
                    css,
                    class_name.as_deref().unwrap_or(""),
                    inline_style.as_deref(),
                    &id,
                    "canvas",
                    line_index + 1,
                );
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Canvas,
                });
            }
            JsonLine::Image {
                id,
                parent_id,
                class_name,
                style: inline_style,
                path,
                url,
                query,
                query_count,
                aspect_ratio,
                duration,
            } => {
                let style = resolve_style(
                    css,
                    class_name.as_deref().unwrap_or(""),
                    inline_style.as_deref(),
                    &id,
                    "image",
                    line_index + 1,
                );
                let source = parse_image_source(path, url, query, query_count, aspect_ratio)?;
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Image { source },
                });
            }
            JsonLine::Audio {
                id,
                attach,
                class_name: _,
                path,
                url,
                duration,
            } => {
                let source = parse_audio_source(path, url)?;
                audio_elements.push(ParsedAudioElement {
                    id,
                    attach,
                    duration,
                    source,
                });
            }
            JsonLine::Video {
                id,
                parent_id,
                class_name,
                style: inline_style,
                path,
                url,
                duration,
                data_start,
                data_duration,
                data_media_start,
                data_color_matrix,
                looping,
                hide_after_end,
            } => {
                let style = resolve_style(
                    css,
                    class_name.as_deref().unwrap_or(""),
                    inline_style.as_deref(),
                    &id,
                    "video",
                    line_index + 1,
                );
                let source = match url {
                    Some(u) => VideoSource::Url(u),
                    None => VideoSource::Path(path.unwrap_or_default()),
                };
                let color_matrix = match data_color_matrix.as_deref() {
                    None => None,
                    Some(token) => VideoColorMatrix::parse_token(token).ok_or_else(|| {
                        anyhow::anyhow!(
                            "dataColorMatrix must be auto, bt601, or bt709 (got `{token}`)"
                        )
                    })?,
                };
                let timing = VideoFrameTiming {
                    timeline_start_secs: data_start.unwrap_or(0.0),
                    timeline_duration_secs: data_duration,
                    media_start_secs: data_media_start.unwrap_or(0.0),
                    playback_rate: 1.0,
                    looping: looping.unwrap_or(false),
                    color_matrix,
                    hide_after_end: hide_after_end.unwrap_or(false),
                };
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Video { source, timing },
                });
            }
            JsonLine::Icon {
                id,
                parent_id,
                class_name,
                style: inline_style,
                icon,
                duration,
            } => {
                let style = resolve_style(
                    css,
                    class_name.as_deref().unwrap_or(""),
                    inline_style.as_deref(),
                    &id,
                    "icon",
                    line_index + 1,
                );
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Icon { name: icon },
                });
            }
            JsonLine::Transition {
                parent_id,
                from,
                to,
                effect,
                duration,
                direction,
                timing,
                damping,
                stiffness,
                mass,
                seed,
                hue_shift,
                mask_scale,
            } => transitions.push(ParsedTransition {
                parent_id,
                from,
                to,
                effect,
                duration,
                direction,
                timing,
                damping,
                stiffness,
                mass,
                seed,
                hue_shift,
                mask_scale,
            }),
            JsonLine::Tl {
                id,
                parent_id,
                class_name,
                style: inline_style,
                duration,
            } => {
                let style = resolve_style(
                    css,
                    class_name.as_deref().unwrap_or(""),
                    inline_style.as_deref(),
                    &id,
                    "div",
                    line_index + 1,
                );
                timeline_ids.insert(id.clone());
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Timeline,
                });
            }
            JsonLine::Caption {
                id,
                parent_id,
                class_name,
                style: inline_style,
                path,
                duration,
            } => {
                let style = resolve_style(
                    css,
                    class_name.as_deref().unwrap_or(""),
                    inline_style.as_deref(),
                    &id,
                    "span",
                    line_index + 1,
                );
                let resolved_path = if let Some(base_dir) = base_dir {
                    base_dir.join(&path)
                } else {
                    PathBuf::from(path)
                };
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Caption {
                        path: resolved_path,
                    },
                });
            }
            JsonLine::Path {
                id,
                parent_id,
                class_name,
                style: inline_style,
                d,
                view_box,
                duration,
            } => {
                let style = resolve_style(
                    css,
                    class_name.as_deref().unwrap_or(""),
                    inline_style.as_deref(),
                    &id,
                    "path",
                    line_index + 1,
                );
                let view_box = view_box
                    .as_deref()
                    .map(parse_view_box_attr)
                    .transpose()?;
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Path { data: d, view_box },
                });
            }
            JsonLine::Lottie {
                id,
                parent_id,
                class_name,
                style: inline_style,
                path,
                url,
                duration,
            } => {
                let style = resolve_style(
                    css,
                    class_name.as_deref().unwrap_or(""),
                    inline_style.as_deref(),
                    &id,
                    "div",
                    line_index + 1,
                );
                let source = match (path, url) {
                    (Some(p), None) => crate::parse::primitives::LottieSource::Path(p),
                    (None, Some(u)) => crate::parse::primitives::LottieSource::Url(u),
                    (Some(_), Some(_)) => {
                        anyhow::bail!("lottie `{id}`: only one of path / url");
                    }
                    (None, None) => {
                        anyhow::bail!("lottie `{id}`: needs path or url");
                    }
                };
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Lottie {
                        source,
                        timing: VideoFrameTiming::default(),
                    },
                });
            }
        }
    }

    validate_unique_ids(&elements, &audio_elements)?;

    if let Some(manifest) = manifest.filter(|m| !m.is_empty()) {
        let index = manifest.build_family_index();
        manifest.apply_font_refs_to_styles(&index, &mut elements);
    }

    let has_explicit_tl = !timeline_ids.is_empty();
    if !has_explicit_tl && !transitions.is_empty() {
        anyhow::bail!("transition records require an explicit tl parent");
    }
    for transition in &transitions {
        if !timeline_ids.contains(transition.parent_id.as_str()) {
            anyhow::bail!(
                "transition parentId `{}` must reference a tl node",
                transition.parent_id
            );
        }
    }
    let elements_by_id = elements
        .iter()
        .map(|element| (element.id.as_str(), element))
        .collect::<HashMap<_, _>>();
    let audio_sources = audio_elements
        .into_iter()
        .map(|audio| resolve_audio_source(audio, &elements_by_id))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut root = if has_explicit_tl {
        let transitions_by_tl: HashMap<String, Vec<&ParsedTransition>> =
            transitions.iter().fold(HashMap::new(), |mut acc, t| {
                acc.entry(t.parent_id.clone()).or_default().push(t);
                acc
            });
        build_tree_with_tl(&elements, &scripts_by_parent, &transitions_by_tl)?
    } else {
        build_tree(&elements, &scripts_by_parent)?
    };

    if !global_scripts.is_empty() {
        // Merge with any scripts already attached to root (parentId == root).
        let mut decls = Vec::new();
        if let Some(existing) = root.style_ref().script_driver.as_ref() {
            for frag in &existing.inline_fragments {
                if !frag.is_empty() {
                    decls.push(DeclaredScript::Inline(frag.clone()));
                }
            }
            if existing.externals.is_empty()
                && !existing.source.is_empty()
                && existing.inline_fragments.is_empty()
            {
                decls.push(DeclaredScript::Inline(existing.source.clone()));
            }
            for ext in &existing.externals {
                decls.push(DeclaredScript::External {
                    locator: ext.locator.clone(),
                });
            }
        }
        decls.extend(global_scripts.clone());
        if let Some(driver) = script_driver_from_decls(decls)? {
            root = root.script_driver(driver);
        }
    }

    Ok(ParsedComposition {
        font_manifest: manifest.cloned().unwrap_or_default(),
        width,
        height,
        fps,
        duration,
        root,
        script: join_inline_global_scripts(global_scripts),
        audio_sources,
    })
}

/// Keep external scripts as logical locators. Host supplies text via lifecycle
/// HostInputs — core never reads script files or rewrites the input string.
fn resolve_script_declaration(
    src: Option<String>,
    path: Option<String>,
) -> anyhow::Result<DeclaredScript> {
    match (src, path) {
        (Some(source), None) => Ok(DeclaredScript::Inline(source)),
        (None, Some(locator)) => Ok(DeclaredScript::External { locator }),
        (Some(_), Some(_)) => anyhow::bail!("script node must use only one of src/content or path"),
        (None, None) => anyhow::bail!("script node requires either src/content or path"),
    }
}

fn join_inline_global_scripts(scripts: Vec<DeclaredScript>) -> Option<String> {
    let mut inlines = Vec::new();
    for s in scripts {
        if let DeclaredScript::Inline(t) = s {
            inlines.push(t);
        }
    }
    join_scripts(inlines)
}

fn resolve_audio_source(
    audio: ParsedAudioElement,
    elements_by_id: &HashMap<&str, &ParsedElement>,
) -> anyhow::Result<CompositionAudioSource> {
    let attach = audio.attach;
    let element = elements_by_id.get(attach.as_str()).copied();
    let attachment = match element {
        Some(el) if matches!(el.kind, ParsedElementKind::Timeline) => AudioAttachment::Timeline,
        Some(_) => AudioAttachment::Scene { scene_id: attach },
        None => anyhow::bail!(
            "audio `{}` attach references non-existent element `{}`",
            audio.id,
            attach
        ),
    };
    Ok(CompositionAudioSource {
        id: audio.id,
        source: audio.source,
        attach: attachment,
        duration_secs: audio.duration,
    })
}

fn parse_image_source(
    path: Option<String>,
    url: Option<String>,
    query: Option<String>,
    query_count: Option<usize>,
    aspect_ratio: Option<String>,
) -> anyhow::Result<ImageSource> {
    let sources = [
        path.as_ref().map(|_| "path"),
        url.as_ref().map(|_| "url"),
        query.as_ref().map(|_| "query"),
    ]
    .into_iter()
    .flatten()
    .count();

    if sources == 0 {
        return Err(anyhow::anyhow!(
            "image node requires one of: path, url, query"
        ));
    }

    if sources > 1 {
        return Err(anyhow::anyhow!(
            "image node accepts only one source: path, url, or query"
        ));
    }

    if let Some(path) = path {
        return Ok(ImageSource::Path(path));
    }

    if let Some(url) = url {
        return Ok(ImageSource::Url(url));
    }

    let Some(query) = query else {
        return Err(anyhow::anyhow!(
            "image node requires one of: path, url, query"
        ));
    };

    Ok(ImageSource::Query(OpenverseQuery {
        query,
        count: query_count.unwrap_or(1).max(1),
        aspect_ratio,
    }))
}

/// Optional shared SVG view box, `"min-x,min-y,width,height"` (commas or
/// whitespace as separators), carried by the `viewBox` line field.
fn parse_view_box_attr(raw: &str) -> anyhow::Result<[f32; 4]> {
    let parts: Vec<&str> = raw
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 4 {
        anyhow::bail!(
            "`viewBox` must be `min-x,min-y,width,height` (4 numbers, got {})",
            parts.len()
        );
    }
    let mut out = [0.0f32; 4];
    for (slot, part) in out.iter_mut().zip(parts.iter()) {
        *slot = part
            .parse::<f32>()
            .map_err(|e| anyhow::anyhow!("`viewBox`: invalid number `{part}`: {e}"))?;
        if !slot.is_finite() {
            anyhow::bail!("`viewBox` values must be finite");
        }
    }
    if out[2] <= 0.0 || out[3] <= 0.0 {
        anyhow::bail!("`viewBox` width and height must be positive");
    }
    Ok(out)
}

fn parse_audio_source(path: Option<String>, url: Option<String>) -> anyhow::Result<AudioSource> {
    let sources = [path.as_ref().map(|_| "path"), url.as_ref().map(|_| "url")]
        .into_iter()
        .flatten()
        .count();

    if sources == 0 {
        return Err(anyhow::anyhow!("audio node requires one of: path, url"));
    }

    if sources > 1 {
        return Err(anyhow::anyhow!(
            "audio node accepts only one source: path or url"
        ));
    }

    if let Some(path) = path {
        return Ok(AudioSource::Path(PathBuf::from(path)));
    }

    let Some(url) = url else {
        return Err(anyhow::anyhow!("audio node requires one of: path, url"));
    };

    Ok(AudioSource::Url(url))
}

/// class + inline style → NodeStyle 的统一入口。
///
/// class 由标准 CSS 引擎层叠（tailwind 写法的类经 tailwindcss CLI 编译成
/// 标准 CSS，见 `scripts/build-css.sh`）。内联 `style` 是标准 CSS 的一部分，
/// 层叠优先级高于任何选择器规则，因此作为最后一段声明应用。
///
/// 没有样式表时仍要解析内联声明：建一个空表，根变量为空，层叠只剩内联
/// 声明本身。样式表规模与构建代价无关（规则为空）。
fn resolve_style(
    css: Option<&crate::css::CssEngine>,
    class_name: &str,
    inline_style: Option<&str>,
    node_id: &str,
    tag: &str,
    _line_number: usize,
) -> NodeStyle {
    match css {
        Some(engine) => {
            let mut style = engine.compute_node_style(class_name, node_id, tag);
            if let Some(text) = inline_style {
                engine.apply_inline_style(&mut style, text, node_id, tag);
            }
            style
        }
        None => {
            let mut style = NodeStyle {
                id: node_id.to_string(),
                ..NodeStyle::default()
            };
            if let Some(text) = inline_style {
                match crate::css::CssEngine::from_css_text("", crate::css::EvalContext::default()) {
                    Ok(engine) => engine.apply_inline_style(&mut style, text, node_id, tag),
                    Err(err) => log::warn!("css: 内联 style 引擎不可用：{err}"),
                }
            }
            style
        }
    }
}

#[cfg(test)]
mod tests {

    fn parse(input: &str) -> anyhow::Result<crate::parse::document::ParsedComposition> {
        super::parse_lines_for_tests(input, None)
    }
    use crate::parse::{composition::AudioAttachment, node::NodeKind};
    use crate::style::Position;

    #[test]
    fn jsonl_parser_uses_shared_parsed_composition_type() {
        let parsed: crate::parse::document::ParsedComposition = parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":0.033333333333}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}"#,
        )
        .expect("jsonl should parse");

        assert_eq!(parsed.width, 320);
        assert_eq!(parsed.root.style_ref().id, "root");
    }

    #[test]
    fn parser_maps_path_view_box_when_present() {
        let parsed = parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"type":"path","id":"a","parentId":"root","className":"w-[115px] h-[115px]","d":"M0 0 L10 10","viewBox":"0 0 512 512"}
{"type":"path","id":"b","parentId":"root","className":"w-[10px] h-[10px]","d":"M0 0 L10 10"}"#,
        )
        .expect("jsonl should parse");

        let crate::parse::node::NodeKind::Div(root) = parsed.root.kind() else {
            panic!("root should be a div");
        };
        let find = |id: &str| {
            root.children_ref()
                .iter()
                .find(|n| n.style_ref().id == id)
                .unwrap_or_else(|| panic!("path `{id}` should exist"))
        };
        let crate::parse::node::NodeKind::Path(a) = find("a").kind() else {
            panic!("`a` should be a path");
        };
        assert_eq!(a.view_box(), Some([0.0, 0.0, 512.0, 512.0]));
        let crate::parse::node::NodeKind::Path(b) = find("b").kind() else {
            panic!("`b` should be a path");
        };
        assert_eq!(b.view_box(), None);
    }

    #[test]
    fn parser_keeps_script_line() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":3}
{"id":"root","parentId":null,"type":"div","className":"flex","text":null}
{"type":"script","src":"ctx.getNode('root').opacity(0.5);"}"#,
        )
        .expect("jsonl should parse");

        assert_eq!(parsed.width, 640);
        assert_eq!(parsed.height, 360);
        assert_eq!(
            parsed.script.as_deref(),
            Some("ctx.getNode('root').opacity(0.5);")
        );
    }

    #[test]
    fn parser_keeps_legacy_script_content_line() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":3}
{"id":"root","parentId":null,"type":"div","className":"flex","text":null}
{"type":"script","content":"ctx.getNode('root').opacity(0.5);"}"#,
        )
        .expect("jsonl with legacy content should parse");

        assert_eq!(
            parsed.script.as_deref(),
            Some("ctx.getNode('root').opacity(0.5);")
        );
    }

    #[test]
    fn parser_rejects_root_level_timeline_without_explicit_tl() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":33.3}
{"id":"root-a","parentId":null,"type":"div","className":"","duration":0.333333333333}
{"type":"script","parentId":"root-a","src":"ctx.getNode('root-a').opacity(0.6);"}
{"type":"transition","parentId":"main-tl","from":"root-a","to":"root-b","effect":"fade","duration":0.166666666666}
{"id":"root-b","parentId":null,"type":"div","className":"","duration":0.333333333333}"#,
        )
        .err()
        .expect("legacy root timeline should fail");

        assert!(err.to_string().contains("tl"));
    }

    #[test]
    fn parser_rejects_multiple_root_sequences_without_explicit_tl() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":33.3}
{"id":"root-a","parentId":null,"type":"div","className":"","duration":0.333333333333}
{"id":"root-b","parentId":null,"type":"div","className":"","duration":0.333333333333}"#,
        )
        .err()
        .expect("multiple roots without tl should fail");

        assert!(err.to_string().contains("multiple root"));
    }

    #[test]
    fn parser_rejects_non_adjacent_tl_transition_pairs() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":1.166666666667}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":1.166666666667}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-c","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-c","effect":"fade","duration":0.166666666666}"#,
        )
        .err()
        .expect("non-adjacent transitions should fail");

        assert!(err.to_string().contains("adjacent"));
    }

    #[test]
    fn parser_requires_sequence_duration_when_building_timeline() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.833333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":""}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-b","effect":"fade","duration":0.166666666666}"#,
        )
        .err()
        .expect("timeline without durations should fail");

        assert!(err.to_string().contains("missing a duration"));
    }

    #[test]
    fn parser_accepts_image_query_nodes() {
        parse(
            r#"{"type":"composition","width":1280,"height":720,"fps":30,"duration":3}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full","text":null}
{"id":"hero","parentId":"root","type":"image","className":"w-[320px] h-[240px] object-cover","query":"pizza margherita"}"#,
        )
        .expect("jsonl with image query should parse");
    }

    #[test]
    fn parser_accepts_lucide_icon_nodes() {
        parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"duration":6}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"search-icon","parentId":"root","type":"icon","className":"w-[20px] h-[20px] text-slate-400","icon":"search"}"#,
        )
        .expect("jsonl with lucide icon should parse");
    }

    #[test]
    fn parser_accepts_audio_nodes() {
        parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"duration":6}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"bgm","attach":"root","type":"audio","path":"/tmp/demo.mp3"}"#,
        )
        .expect("jsonl with audio path should parse");

        parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"duration":6}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"stream","attach":"root","type":"audio","url":"https://example.com/demo.mp3"}"#,
        )
        .expect("jsonl with audio url should parse");
    }

    #[test]
    fn parser_treats_timeline_attached_audio_as_timeline_audio_source() {
        let parsed = parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"duration":6}
{"id":"main-tl","parentId":null,"type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"w-full h-full","duration":1}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"w-full h-full","duration":1}
{"id":"bgm","attach":"main-tl","type":"audio","path":"/tmp/demo.mp3"}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-b","effect":"fade","duration":0.033333333333}"#,
        )
        .expect("jsonl with timeline-attached audio should parse");

        assert_eq!(parsed.audio_sources.len(), 1);
        assert!(matches!(
            parsed.audio_sources[0].attach,
            AudioAttachment::Timeline
        ));
    }

    #[test]
    fn parser_attaches_audio_to_owning_scene() {
        let parsed = parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"duration":6}
{"id":"scene-a","parentId":null,"type":"div","className":"w-full h-full","duration":1}
{"id":"voice","attach":"scene-a","type":"audio","path":"/tmp/voice.mp3"}"#,
        )
        .expect("jsonl with scene audio should parse");

        assert_eq!(parsed.audio_sources.len(), 1);
        assert!(matches!(
            &parsed.audio_sources[0].attach,
            AudioAttachment::Scene { scene_id } if scene_id == "scene-a"
        ));
    }

    #[test]
    fn parser_accepts_explicit_tl_root_and_local_transition() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.833333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-b","effect":"fade","duration":0.166666666666}"#,
        )
        .expect("explicit tl jsonl should parse");

        assert!((parsed.duration - 25.0 / 30.0).abs() < 1e-9);
        assert!(matches!(parsed.root.kind(), NodeKind::Div(_)));
    }

    #[test]
    fn parser_requires_transition_parent_id() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.833333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","from":"scene-a","to":"scene-b","effect":"fade","duration":0.166666666666}"#,
        )
        .err()
        .expect("missing transition parentId should fail");

        assert!(err.to_string().contains("parentId"));
    }

    #[test]
    fn parser_rejects_legacy_layer_records() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"scene-a","parentId":null,"type":"div","className":"","duration":0.333333333333}
{"id":"subs","parentId":null,"type":"caption","className":"absolute","path":"sub.srt"}
{"type":"layer","children":["scene-a","subs"]}"#,
        )
        .err()
        .expect("legacy layer input should fail");

        assert!(err.to_string().contains("layer"));
    }

    #[test]
    fn parser_builds_tl_node_from_direct_children() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.833333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"type":"script","parentId":"main-tl","src":"ctx.getNode('main-tl').opacity(0.5);"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-b","effect":"fade","duration":0.166666666666}"#,
        )
        .expect("tl jsonl should parse");

        let NodeKind::Div(root) = parsed.root.kind() else {
            panic!("root should be div");
        };
        let NodeKind::Timeline(tl) = root.children_ref()[0].kind() else {
            panic!("child should be timeline");
        };
        let frame_ctx = crate::frame_ctx::FrameCtx {
            frame: 0,
            fps: parsed.fps as u32,
            width: parsed.width,
            height: parsed.height,
            frames: crate::frame_ctx::duration_secs_to_frames(parsed.duration, parsed.fps as u32),
        };
        assert_eq!(tl.duration_in_frames(&frame_ctx), 25);
        assert_eq!(tl.style_ref().id, "main-tl");
        assert_eq!(tl.style_ref().position, Some(Position::Absolute));
        assert!(tl.style_ref().script_driver.is_some());
    }

    #[test]
    fn parser_rejects_transition_that_targets_non_adjacent_tl_child() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":1.166666666667}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":1.166666666667}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-c","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-c","effect":"fade","duration":0.166666666666}"#,
        )
        .err()
        .expect("non-adjacent transition should fail");

        assert!(err.to_string().contains("adjacent"));
    }

    #[test]
    fn parser_rejects_transition_to_non_direct_tl_descendant() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.833333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"nested","parentId":"scene-b","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"nested","effect":"fade","duration":0.166666666666}"#,
        )
        .err()
        .expect("transition to nested descendant should fail");

        assert!(err.to_string().contains("direct child"));
    }

    #[test]
    fn parser_rejects_tl_with_single_scene_child() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.333333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.333333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}"#,
        )
        .err()
        .expect("single-scene tl should fail");

        assert!(err.to_string().contains("at least two"));
    }

    #[test]
    fn parser_accepts_tl_hard_cut_between_adjacent_children() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.666666666667}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.666666666667}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}"#,
        )
        .expect("adjacent scenes without a transition should parse");

        let NodeKind::Div(root) = parsed.root.kind() else {
            panic!("root should be div");
        };
        let NodeKind::Timeline(timeline) = root.children_ref()[0].kind() else {
            panic!("expected a timeline node");
        };
        assert_eq!(timeline.segments().len(), 2);
    }

    #[test]
    fn jsonl_allows_canvas_hidden_visual_children() {
        // JSONL (and therefore the JSX frontend, which lowers to JSONL lines)
        // accepts canvas hidden picture subtrees, same as markup
        // (markup_allows_canvas_hidden_visual_children): visual children are
        // rasterized offscreen and reachable only through
        // canvas.getSubTree().makeShader(...).
        let parsed = parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":0.033333333333}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"stage","parentId":"root","type":"canvas","className":"w-full h-full"}
{"id":"hidden","parentId":"stage","type":"text","className":"text-[12px]","text":"hidden"}"#,
        )
        .expect("canvas hidden children should parse");

        assert_eq!(parsed.root.style_ref().id, "root");
    }

    #[test]
    fn jsonl_rejects_duplicate_visual_ids() {
        let err = parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":0.033333333333}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"dup","parentId":"root","type":"div","className":""}
{"id":"dup","parentId":"root","type":"text","className":"","text":"x"}"#,
        )
        .expect_err("duplicate ids should fail");

        assert!(err.to_string().contains("duplicate id `dup`"));
    }

    #[test]
    fn path_jsonl_parses_and_resolves_path_nodes() {
        let input = r#"{"type":"composition","width":1280,"height":720,"fps":30,"duration":3}
{"id":"root","parentId":null,"type":"div","className":"relative w-[1280px] h-[720px] bg-slate-950"}
{"id":"card","parentId":"root","type":"div","className":"flex items-center justify-center"}
{"id":"tri","parentId":"card","type":"path","className":"w-[120px] h-[104px] fill-cyan-500 stroke-cyan-200 stroke-2","d":"M0 0 L120 0 L60 104 Z"}
{"id":"star","parentId":"card","type":"path","className":"w-[100px] h-[95px] fill-amber-400","d":"M50 0 L62 35 L98 35 L68 57 L79 91 L50 70 L21 91 L32 57 L2 35 L38 35 Z"}"#;

        let parsed = parse(input).expect("path jsonl should parse");
        assert_eq!(parsed.width, 1280);
        assert_eq!(parsed.fps, 30);
        assert_eq!(parsed.duration, 3.0);
    }
}
