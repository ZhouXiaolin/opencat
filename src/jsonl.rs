use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::scene::{
    composition::{AudioAttachment, CompositionAudioSource},
    node::Node,
    primitives::{AudioSource, ImageSource, OpenverseQuery},
};
use crate::style::NodeStyle;

mod builder;
pub(crate) mod tailwind;

use builder::{build_timeline, build_tree, join_scripts};

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
enum JsonLine {
    #[serde(rename = "composition")]
    Composition {
        width: i32,
        height: i32,
        fps: i32,
        frames: i32,
    },
    #[serde(rename = "script")]
    Script {
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        content: String,
    },
    #[serde(rename = "div")]
    Div {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        duration: Option<u32>,
    },
    #[serde(rename = "text")]
    Text {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        text: String,
        duration: Option<u32>,
    },
    #[serde(rename = "canvas")]
    Canvas {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        duration: Option<u32>,
    },
    #[serde(rename = "image")]
    Image {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        path: Option<String>,
        url: Option<String>,
        query: Option<String>,
        #[serde(rename = "queryCount")]
        query_count: Option<usize>,
        #[serde(rename = "aspectRatio")]
        aspect_ratio: Option<String>,
        duration: Option<u32>,
    },
    #[serde(rename = "audio")]
    Audio {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[allow(dead_code)]
        #[serde(rename = "className")]
        class_name: Option<String>,
        path: Option<String>,
        url: Option<String>,
        duration: Option<u32>,
    },
    #[serde(rename = "video")]
    Video {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        path: String,
        duration: Option<u32>,
    },
    #[serde(rename = "icon")]
    Icon {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        icon: String,
        duration: Option<u32>,
    },
    #[serde(rename = "transition")]
    Transition {
        from: String,
        to: String,
        effect: String,
        duration: u32,
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
}

#[derive(Debug, Clone)]
enum ParsedElementKind {
    Div,
    Text { content: String },
    Canvas,
    Image { source: ImageSource },
    Icon { name: String },
    Video { path: PathBuf },
}

#[derive(Debug, Clone)]
struct ParsedElement {
    id: String,
    parent_id: Option<String>,
    duration: Option<u32>,
    style: NodeStyle,
    kind: ParsedElementKind,
}

#[derive(Debug, Clone)]
struct ParsedTransition {
    from: String,
    to: String,
    effect: String,
    duration: u32,
    direction: Option<String>,
    timing: Option<String>,
    damping: Option<f32>,
    stiffness: Option<f32>,
    mass: Option<f32>,
    seed: Option<f32>,
    hue_shift: Option<f32>,
    mask_scale: Option<f32>,
}

#[derive(Debug, Clone)]
struct ParsedAudioElement {
    id: String,
    parent_id: Option<String>,
    duration: Option<u32>,
    source: AudioSource,
}

#[derive(Debug, Clone)]
enum TimelineEntry {
    SequenceRoot { id: String },
    Transition(ParsedTransition),
}

pub struct ParsedComposition {
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub frames: i32,
    pub root: Node,
    pub script: Option<String>,
    pub audio_sources: Vec<CompositionAudioSource>,
}

pub fn parse(input: &str) -> anyhow::Result<ParsedComposition> {
    let mut width = 1920;
    let mut height = 1080;
    let mut fps = 30;
    let mut frames = 90;
    let mut global_scripts = Vec::new();
    let mut audio_elements = Vec::new();
    let mut scripts_by_parent: HashMap<String, Vec<String>> = HashMap::new();
    let mut elements: Vec<ParsedElement> = Vec::new();
    let mut timeline_entries = Vec::new();

    for (line_index, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parsed: JsonLine = serde_json::from_str(line)?;
        match parsed {
            JsonLine::Composition {
                width: w,
                height: h,
                fps: f,
                frames: fs,
            } => {
                width = w;
                height = h;
                fps = f;
                frames = fs;
            }
            JsonLine::Script { parent_id, content } => {
                if let Some(parent_id) = parent_id {
                    scripts_by_parent
                        .entry(parent_id)
                        .or_default()
                        .push(content);
                } else {
                    global_scripts.push(content);
                }
            }
            JsonLine::Div {
                id,
                parent_id,
                class_name,
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
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
                text,
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
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
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
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
                path,
                url,
                query,
                query_count,
                aspect_ratio,
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                let source = parse_image_source(path, url, query, query_count, aspect_ratio)?;
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
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
                parent_id,
                class_name: _,
                path,
                url,
                duration,
            } => {
                let source = parse_audio_source(path, url)?;
                audio_elements.push(ParsedAudioElement {
                    id,
                    parent_id,
                    duration,
                    source,
                });
            }
            JsonLine::Video {
                id,
                parent_id,
                class_name,
                path,
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Video {
                        path: PathBuf::from(path),
                    },
                });
            }
            JsonLine::Icon {
                id,
                parent_id,
                class_name,
                icon,
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Icon { name: icon },
                });
            }
            JsonLine::Transition {
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
            } => timeline_entries.push(TimelineEntry::Transition(ParsedTransition {
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
            })),
        }
    }

    let has_timeline = timeline_entries
        .iter()
        .any(|entry| matches!(entry, TimelineEntry::Transition(_)))
        || elements.iter().filter(|el| el.parent_id.is_none()).count() > 1;
    let elements_by_id = elements
        .iter()
        .map(|element| (element.id.as_str(), element))
        .collect::<HashMap<_, _>>();
    let audio_sources = audio_elements
        .into_iter()
        .map(|audio| resolve_audio_source(audio, &elements_by_id))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let (root, frames) = if has_timeline {
        build_timeline(&elements, &scripts_by_parent, &timeline_entries)?
    } else {
        (build_tree(&elements, &scripts_by_parent)?, frames)
    };

    Ok(ParsedComposition {
        width,
        height,
        fps,
        frames,
        root,
        script: join_scripts(global_scripts),
        audio_sources,
    })
}

fn resolve_audio_source(
    audio: ParsedAudioElement,
    elements_by_id: &HashMap<&str, &ParsedElement>,
) -> anyhow::Result<CompositionAudioSource> {
    let attach = match audio.parent_id.as_deref() {
        None => AudioAttachment::Timeline,
        Some(parent_id) => AudioAttachment::Scene {
            scene_id: resolve_scene_root_id(parent_id, elements_by_id)?.to_string(),
        },
    };

    Ok(CompositionAudioSource {
        id: audio.id,
        source: audio.source,
        attach,
        duration: audio.duration,
    })
}

fn resolve_scene_root_id<'a>(
    start_id: &'a str,
    elements_by_id: &HashMap<&'a str, &'a ParsedElement>,
) -> anyhow::Result<&'a str> {
    let mut current_id = start_id;

    loop {
        let element = elements_by_id.get(current_id).copied().ok_or_else(|| {
            anyhow::anyhow!("audio node references missing visual parent `{current_id}`")
        })?;

        match element.parent_id.as_deref() {
            Some(parent_id) => current_id = parent_id,
            None => return Ok(element.id.as_str()),
        }
    }
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
        return Ok(ImageSource::Path(PathBuf::from(path)));
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

#[cfg_attr(not(test), allow(dead_code))]
fn parse_class_name(class_name: &str) -> NodeStyle {
    tailwind::parse_class_name(class_name)
}

fn parse_class_name_with_context(class_name: &str, node_id: &str, line_number: usize) -> NodeStyle {
    tailwind::parse_class_name_with_context(class_name, node_id, line_number)
}

#[cfg(test)]
mod tests {
    use super::{parse, parse_class_name};
    use crate::{
        FrameCtx,
        scene::{
            composition::AudioAttachment,
            node::NodeKind,
            time::{FrameState, frame_state_for_root},
        },
        style::{
            AlignItems, ColorToken, FlexDirection, FlexWrap, GradientDirection, JustifyContent,
            LengthPercentageAuto, TextAlign,
        },
    };

    #[test]
    fn parser_maps_full_size_alignment_and_tailwind_colors() {
        let style = parse_class_name(
            "text-center w-full h-full text-teal-400 bg-slate-800 border-slate-700 leading-[1.8]",
        );

        assert_eq!(style.text_align, Some(TextAlign::Center));
        assert!(style.width_full);
        assert!(style.height_full);
        assert_eq!(style.text_color, Some(ColorToken::Teal400));
        assert_eq!(style.bg_color, Some(ColorToken::Slate800));
        assert_eq!(style.border_color, Some(ColorToken::Slate700));
        assert_eq!(style.line_height, Some(1.8));
    }

    #[test]
    fn parser_maps_absolute_px_line_height_separately() {
        let style = parse_class_name("leading-[18px]");

        assert_eq!(style.line_height, None);
        assert_eq!(style.line_height_px, Some(18.0));
    }

    #[test]
    fn parser_keeps_script_line() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":90}
{"id":"root","parentId":null,"type":"div","className":"flex","text":null}
{"type":"script","content":"ctx.getNode('root').opacity(0.5);"}"#,
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
    fn parser_builds_timeline_from_root_sequences_and_transitions() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":999}
{"id":"root-a","parentId":null,"type":"div","className":"","duration":10}
{"type":"script","parentId":"root-a","content":"ctx.getNode('root-a').opacity(0.6);"}
{"type":"transition","from":"root-a","to":"root-b","effect":"fade","duration":5}
{"id":"root-b","parentId":null,"type":"div","className":"","duration":10}"#,
        )
        .expect("timeline jsonl should parse");

        assert_eq!(parsed.frames, 25);
        assert!(matches!(parsed.root.kind(), NodeKind::Timeline(_)));

        let scene_frame = FrameCtx {
            frame: 0,
            fps: parsed.fps as u32,
            width: parsed.width,
            height: parsed.height,
            frames: parsed.frames as u32,
        };
        let transition_frame = FrameCtx {
            frame: 12,
            ..scene_frame
        };
        let final_scene_frame = FrameCtx {
            frame: 20,
            ..scene_frame
        };

        match frame_state_for_root(&parsed.root, &scene_frame) {
            FrameState::Scene { scene, .. } => {
                assert_eq!(scene.style_ref().id, "root-a");
                assert!(scene.style_ref().script_driver.is_some());
            }
            _ => panic!("frame 0 should resolve to the first sequence"),
        }

        match frame_state_for_root(&parsed.root, &transition_frame) {
            FrameState::Transition { from, to, .. } => {
                assert_eq!(from.style_ref().id, "root-a");
                assert_eq!(to.style_ref().id, "root-b");
            }
            _ => panic!("frame 12 should resolve to the transition"),
        }

        match frame_state_for_root(&parsed.root, &final_scene_frame) {
            FrameState::Scene { scene, .. } => {
                assert_eq!(scene.style_ref().id, "root-b");
            }
            _ => panic!("frame 20 should resolve to the final sequence"),
        }
    }

    #[test]
    fn parser_allows_transition_records_outside_scene_boundaries() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":999}
{"id":"root-a","parentId":null,"type":"div","className":"","duration":10}
{"id":"root-b","parentId":null,"type":"div","className":"","duration":10}
{"type":"transition","from":"root-a","to":"root-b","effect":"fade","duration":5}"#,
        )
        .expect("timeline jsonl should parse regardless of transition position");

        assert_eq!(parsed.frames, 25);

        let transition_frame = FrameCtx {
            frame: 12,
            fps: parsed.fps as u32,
            width: parsed.width,
            height: parsed.height,
            frames: parsed.frames as u32,
        };

        match frame_state_for_root(&parsed.root, &transition_frame) {
            FrameState::Transition { from, to, .. } => {
                assert_eq!(from.style_ref().id, "root-a");
                assert_eq!(to.style_ref().id, "root-b");
            }
            _ => panic!("frame 12 should resolve to the transition"),
        }
    }

    #[test]
    fn parser_rejects_non_adjacent_transition_pairs() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":999}
{"id":"root-a","parentId":null,"type":"div","className":"","duration":10}
{"id":"root-b","parentId":null,"type":"div","className":"","duration":10}
{"id":"root-c","parentId":null,"type":"div","className":"","duration":10}
{"type":"transition","from":"root-a","to":"root-c","effect":"fade","duration":5}"#,
        )
        .err()
        .expect("non-adjacent transitions should fail");

        assert!(err.to_string().contains("adjacent sequences"));
    }

    #[test]
    fn parser_requires_sequence_duration_when_building_timeline() {
        let err = parse(
            r#"{"id":"root-a","parentId":null,"type":"div","className":""}
{"type":"transition","from":"root-a","to":"root-b","effect":"fade","duration":5}
{"id":"root-b","parentId":null,"type":"div","className":"","duration":10}"#,
        )
        .err()
        .expect("timeline without durations should fail");

        assert!(err.to_string().contains("missing a duration"));
    }

    #[test]
    fn parser_maps_hex_colors() {
        let style = parse_class_name("bg-[#fff8f0] text-[#e85d04] border-[#5c4033]");

        assert_eq!(
            style.bg_color,
            Some(ColorToken::Custom(0xff, 0xf8, 0xf0, 0xff))
        );
        assert_eq!(
            style.text_color,
            Some(ColorToken::Custom(0xe8, 0x5d, 0x04, 0xff))
        );
        assert_eq!(
            style.border_color,
            Some(ColorToken::Custom(0x5c, 0x40, 0x33, 0xff))
        );
    }

    #[test]
    fn parser_maps_common_icon_styling_classes() {
        let style = parse_class_name("bg-emerald-400 text-white border-[3] border-blue");

        assert_eq!(style.bg_color, Some(ColorToken::Emerald400));
        assert_eq!(style.text_color, Some(ColorToken::White));
        assert_eq!(style.border_width, Some(3.0));
        assert_eq!(style.border_color, Some(ColorToken::Blue));
    }

    #[test]
    fn parser_maps_gradient_overflow_and_directional_spacing_classes() {
        let style = parse_class_name(
            "bg-gradient-to-r from-orange-500 to-amber-500 overflow-hidden mt-[4px] mb-[16px] pb-[20px]",
        );

        assert_eq!(
            style.bg_gradient_direction,
            Some(GradientDirection::ToRight)
        );
        assert_eq!(style.bg_gradient_from, Some(ColorToken::Orange500));
        assert_eq!(style.bg_gradient_to, Some(ColorToken::Amber500));
        assert!(style.overflow_hidden);
        assert_eq!(style.margin_top, Some(LengthPercentageAuto::Length(4.0)));
        assert_eq!(
            style.margin_bottom,
            Some(LengthPercentageAuto::Length(16.0))
        );
        assert_eq!(style.padding_bottom, Some(20.0));
    }

    #[test]
    fn parser_maps_tailwind_spacing_scale_and_text_sizes() {
        let style = parse_class_name("w-20 h-28 left-8 top-4 px-6 py-2 gap-3.5 text-3xl");

        assert_eq!(style.width, Some(80.0));
        assert_eq!(style.height, Some(112.0));
        assert_eq!(style.inset_left, Some(LengthPercentageAuto::length(32.0)));
        assert_eq!(style.inset_top, Some(LengthPercentageAuto::length(16.0)));
        assert_eq!(style.padding_x, Some(24.0));
        assert_eq!(style.padding_y, Some(8.0));
        assert_eq!(style.gap, Some(14.0));
        assert_eq!(style.text_px, Some(30.0));
        assert_eq!(style.line_height_px, Some(36.0));
    }

    #[test]
    fn parser_keeps_tailwind_text_size_default_line_height() {
        let style = parse_class_name("text-sm");

        assert_eq!(style.text_px, Some(14.0));
        assert_eq!(style.line_height_px, Some(20.0));
        assert_eq!(style.line_height, None);
    }

    #[test]
    fn parser_lets_explicit_leading_override_text_size_default() {
        let style = parse_class_name("leading-relaxed text-sm");

        assert_eq!(style.text_px, Some(14.0));
        assert_eq!(style.line_height_px, None);
        assert_eq!(style.line_height, Some(1.625));
    }

    #[test]
    fn parser_maps_extended_gradient_and_layering_classes() {
        let style = parse_class_name(
            "bg-gradient-to-br from-transparent via-pink-500 to-violet-400 inset-0 -top-[80px] -left-[24px] z-10 flex-shrink-0 tracking-wider",
        );

        assert_eq!(
            style.bg_gradient_direction,
            Some(GradientDirection::ToBottomRight)
        );
        assert_eq!(style.bg_gradient_from, Some(ColorToken::Transparent));
        assert_eq!(style.bg_gradient_via, Some(ColorToken::Pink500));
        assert_eq!(style.bg_gradient_to, Some(ColorToken::Violet400));
        assert_eq!(style.inset_right, Some(LengthPercentageAuto::length(0.0)));
        assert_eq!(style.inset_bottom, Some(LengthPercentageAuto::length(0.0)));
        assert_eq!(style.inset_top, Some(LengthPercentageAuto::length(-80.0)));
        assert_eq!(style.inset_left, Some(LengthPercentageAuto::length(-24.0)));
        assert_eq!(style.z_index, Some(10));
        assert_eq!(style.flex_shrink, Some(0.0));
        assert_eq!(style.letter_spacing, Some(1.0));
    }

    #[test]
    fn parser_aligns_inset_utilities_with_tailwind_layout_cases() {
        let style = parse_class_name(
            "inset-auto inset-x-4 -inset-y-full inset-s-3/4 inset-e-[12px] inset-bs-auto -inset-be-2",
        );

        assert_eq!(style.inset_left, Some(LengthPercentageAuto::percent(0.75)));
        assert_eq!(style.inset_right, Some(LengthPercentageAuto::length(12.0)));
        assert_eq!(style.inset_top, Some(LengthPercentageAuto::auto()));
        assert_eq!(style.inset_bottom, Some(LengthPercentageAuto::length(-8.0)));
    }

    #[test]
    fn parser_supports_edge_inset_full_fraction_and_negative_values() {
        let style = parse_class_name("top-auto -right-full bottom-3/4 left-[18px] -left-4");

        assert_eq!(style.inset_top, Some(LengthPercentageAuto::auto()));
        assert_eq!(style.inset_right, Some(LengthPercentageAuto::percent(-1.0)));
        assert_eq!(
            style.inset_bottom,
            Some(LengthPercentageAuto::percent(0.75))
        );
        assert_eq!(style.inset_left, Some(LengthPercentageAuto::length(-16.0)));
    }

    #[test]
    fn parser_supports_flex_basis_and_shorthand_layout_cases() {
        let basis = parse_class_name("basis-auto basis-full basis-11/12 basis-[123px]");
        assert_eq!(basis.flex_basis, Some(LengthPercentageAuto::length(123.0)));

        let full = parse_class_name("basis-full");
        assert_eq!(full.flex_basis, Some(LengthPercentageAuto::percent(1.0)));

        let fraction = parse_class_name("basis-11/12");
        assert_eq!(
            fraction.flex_basis,
            Some(LengthPercentageAuto::percent(11.0 / 12.0))
        );

        let auto = parse_class_name("basis-auto");
        assert_eq!(auto.flex_basis, Some(LengthPercentageAuto::auto()));

        let flex_auto = parse_class_name("flex-auto");
        assert_eq!(flex_auto.flex_grow, Some(1.0));
        assert_eq!(flex_auto.flex_shrink, Some(1.0));
        assert_eq!(flex_auto.flex_basis, Some(LengthPercentageAuto::auto()));

        let flex_none = parse_class_name("flex-none");
        assert_eq!(flex_none.flex_grow, Some(0.0));
        assert_eq!(flex_none.flex_shrink, Some(0.0));
        assert_eq!(flex_none.flex_basis, Some(LengthPercentageAuto::auto()));

        let flex_fraction = parse_class_name("flex-1/2");
        assert_eq!(flex_fraction.flex_grow, Some(1.0));
        assert_eq!(flex_fraction.flex_shrink, Some(1.0));
        assert_eq!(
            flex_fraction.flex_basis,
            Some(LengthPercentageAuto::percent(0.5))
        );

        let flex_numeric = parse_class_name("flex-[123]");
        assert_eq!(flex_numeric.flex_grow, Some(123.0));
        assert_eq!(flex_numeric.flex_shrink, Some(1.0));
        assert_eq!(
            flex_numeric.flex_basis,
            Some(LengthPercentageAuto::length(0.0))
        );
    }

    #[test]
    fn parser_maps_tailwind_alpha_blur_and_additional_gradients() {
        let style = parse_class_name(
            "flex-1 bg-gradient-to-b from-amber-100/30 via-transparent to-rose-100/20 blur-xl pointer-events-none",
        );

        assert_eq!(style.flex_grow, Some(1.0));
        assert_eq!(
            style.bg_gradient_direction,
            Some(GradientDirection::ToBottom)
        );
        assert_eq!(style.bg_gradient_via, Some(ColorToken::Transparent));
        assert_eq!(style.blur_sigma, Some(24.0));
        assert_eq!(style.bg_gradient_from.expect("from color").rgba().3, 77);
        assert_eq!(style.bg_gradient_to.expect("to color").rgba().3, 51);

        let overlay = parse_class_name("bg-gradient-to-t from-rose-900/10 to-amber-100/15");
        assert_eq!(
            overlay.bg_gradient_direction,
            Some(GradientDirection::ToTop)
        );
        assert_eq!(overlay.bg_gradient_from.expect("from color").rgba().3, 26);
        assert_eq!(overlay.bg_gradient_to.expect("to color").rgba().3, 38);
    }

    #[test]
    fn parser_covers_core_exact_layout_utilities() {
        let style = parse_class_name(
            "absolute flex flex-col justify-evenly items-end grow shrink-0 w-full h-full object-cover overflow-hidden z-10",
        );

        assert_eq!(style.position, Some(crate::style::Position::Absolute));
        assert!(style.is_flex);
        assert_eq!(style.flex_direction, Some(crate::style::FlexDirection::Col));
        assert_eq!(
            style.justify_content,
            Some(crate::style::JustifyContent::Evenly)
        );
        assert_eq!(style.align_items, Some(crate::style::AlignItems::End));
        assert_eq!(style.flex_grow, Some(1.0));
        assert_eq!(style.flex_shrink, Some(0.0));
        assert!(style.width_full);
        assert!(style.height_full);
        assert_eq!(style.object_fit, Some(crate::style::ObjectFit::Cover));
        assert!(style.overflow_hidden);
        assert_eq!(style.z_index, Some(10));
    }

    #[test]
    fn parser_supports_reverse_and_wrap_flex_layout_utilities() {
        let row_reverse = parse_class_name("flex-row-reverse");
        assert_eq!(row_reverse.flex_direction, Some(FlexDirection::RowReverse));

        let col_reverse = parse_class_name("flex-col-reverse");
        assert_eq!(col_reverse.flex_direction, Some(FlexDirection::ColReverse));

        let wrap = parse_class_name("flex-wrap");
        assert_eq!(wrap.flex_wrap, Some(FlexWrap::Wrap));

        let wrap_reverse = parse_class_name("flex-wrap-reverse");
        assert_eq!(wrap_reverse.flex_wrap, Some(FlexWrap::WrapReverse));

        let nowrap = parse_class_name("flex-nowrap");
        assert_eq!(nowrap.flex_wrap, Some(FlexWrap::NoWrap));
    }

    #[test]
    fn parser_maps_safe_and_extended_alignment_utilities() {
        let justify = parse_class_name("justify-center-safe justify-stretch");
        assert_eq!(justify.justify_content, Some(JustifyContent::Stretch));

        let justify_end_safe = parse_class_name("justify-end-safe");
        assert_eq!(justify_end_safe.justify_content, Some(JustifyContent::End));

        let items = parse_class_name("items-center-safe items-baseline-last");
        assert_eq!(items.align_items, Some(AlignItems::Baseline));

        let items_end_safe = parse_class_name("items-end-safe");
        assert_eq!(items_end_safe.align_items, Some(AlignItems::End));

        let place_items = parse_class_name("place-items-center-safe");
        assert_eq!(place_items.align_items, Some(AlignItems::Center));

        let place_items_baseline = parse_class_name("place-items-baseline");
        assert_eq!(place_items_baseline.align_items, Some(AlignItems::Baseline));
    }

    #[test]
    fn parser_maps_align_content_and_place_content_utilities() {
        let content = parse_class_name("content-center-safe content-evenly");
        assert_eq!(content.align_content, Some(JustifyContent::Evenly));

        let content_end_safe = parse_class_name("content-end-safe");
        assert_eq!(content_end_safe.align_content, Some(JustifyContent::End));

        let content_stretch = parse_class_name("content-stretch");
        assert_eq!(content_stretch.align_content, Some(JustifyContent::Stretch));

        let place_content = parse_class_name("place-content-between");
        assert_eq!(place_content.justify_content, Some(JustifyContent::Between));
        assert_eq!(place_content.align_content, Some(JustifyContent::Between));

        let place_content_safe = parse_class_name("place-content-center-safe");
        assert_eq!(
            place_content_safe.justify_content,
            Some(JustifyContent::Center)
        );
        assert_eq!(
            place_content_safe.align_content,
            Some(JustifyContent::Center)
        );

        let place_content_end = parse_class_name("place-content-end-safe");
        assert_eq!(place_content_end.justify_content, Some(JustifyContent::End));
        assert_eq!(place_content_end.align_content, Some(JustifyContent::End));
    }

    #[test]
    fn parser_maps_align_self_and_ignores_unsupported_normal_baseline_variants() {
        let self_end = parse_class_name("self-end-safe");
        assert_eq!(self_end.align_self, Some(AlignItems::End));

        let self_center = parse_class_name("self-center-safe");
        assert_eq!(self_center.align_self, Some(AlignItems::Center));

        let self_baseline = parse_class_name("self-baseline-last");
        assert_eq!(self_baseline.align_self, Some(AlignItems::Baseline));

        let self_stretch = parse_class_name("self-stretch");
        assert_eq!(self_stretch.align_self, Some(AlignItems::Stretch));

        let ignored =
            parse_class_name("content-normal content-baseline place-content-baseline self-auto");
        assert_eq!(ignored.align_content, None);
        assert_eq!(ignored.justify_content, None);
        assert_eq!(ignored.align_self, None);
    }

    #[test]
    fn parser_supports_margin_scales_brackets_and_negative_values() {
        use crate::style::LengthPercentageAuto as LPA;
        let style = parse_class_name("m-2.5 -mx-4 mt-[6px] -mb-[8px] mr-99");

        assert_eq!(style.margin, Some(LPA::Length(10.0)));
        assert_eq!(style.margin_x, Some(LPA::Length(-16.0)));
        assert_eq!(style.margin_top, Some(LPA::Length(6.0)));
        assert_eq!(style.margin_bottom, Some(LPA::Length(-8.0)));
        assert_eq!(style.margin_right, Some(LPA::Length(396.0)));
    }

    #[test]
    fn parser_maps_logical_margin_aliases_to_physical_edges() {
        use crate::style::LengthPercentageAuto as LPA;
        let style = parse_class_name("ms-3 me-[10px] -mbs-2 mbe-1 -ml-[12px]");

        assert_eq!(style.margin_left, Some(LPA::Length(-12.0)));
        assert_eq!(style.margin_right, Some(LPA::Length(10.0)));
        assert_eq!(style.margin_top, Some(LPA::Length(-8.0)));
        assert_eq!(style.margin_bottom, Some(LPA::Length(4.0)));
    }

    #[test]
    fn parser_supports_grid_display_and_columns() {
        let style = parse_class_name("grid");
        assert!(style.is_grid);
        assert_eq!(style.grid_template_columns, None);

        let style = parse_class_name("grid grid-cols-2");
        assert!(style.is_grid);
        assert_eq!(style.grid_template_columns, Some(2));

        let style = parse_class_name("grid grid-cols-4");
        assert!(style.is_grid);
        assert_eq!(style.grid_template_columns, Some(4));
    }

    #[test]
    fn parser_supports_max_width_bracket_and_scale() {
        let style = parse_class_name("max-w-[280px]");
        assert_eq!(style.max_width, Some(280.0));

        let style = parse_class_name("max-w-[120px]");
        assert_eq!(style.max_width, Some(120.0));

        // spacing scale: max-w-20 → 20 * 4 = 80
        let style = parse_class_name("max-w-20");
        assert_eq!(style.max_width, Some(80.0));
    }

    #[test]
    fn parser_supports_margin_auto_variants() {
        use crate::style::LengthPercentageAuto as LPA;

        let style = parse_class_name("mx-auto");
        assert_eq!(style.margin_x, Some(LPA::Auto));

        let style = parse_class_name("ml-auto");
        assert_eq!(style.margin_left, Some(LPA::Auto));

        let style = parse_class_name("mr-auto");
        assert_eq!(style.margin_right, Some(LPA::Auto));

        let style = parse_class_name("mt-auto");
        assert_eq!(style.margin_top, Some(LPA::Auto));

        let style = parse_class_name("mb-auto");
        assert_eq!(style.margin_bottom, Some(LPA::Auto));

        let style = parse_class_name("m-auto");
        assert_eq!(style.margin, Some(LPA::Auto));

        let style = parse_class_name("my-auto");
        assert_eq!(style.margin_y, Some(LPA::Auto));

        let style = parse_class_name("ms-auto");
        assert_eq!(style.margin_left, Some(LPA::Auto));

        let style = parse_class_name("me-auto");
        assert_eq!(style.margin_right, Some(LPA::Auto));
    }

    #[test]
    fn parser_accepts_image_query_nodes() {
        parse(
            r#"{"type":"composition","width":1280,"height":720,"fps":30,"frames":90}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full","text":null}
{"id":"hero","parentId":"root","type":"image","className":"w-[320px] h-[240px] object-cover","query":"pizza margherita"}"#,
        )
        .expect("jsonl with image query should parse");
    }

    #[test]
    fn parser_accepts_lucide_icon_nodes() {
        parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"frames":180}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"search-icon","parentId":"root","type":"icon","className":"w-[20px] h-[20px] text-slate-400","icon":"search"}"#,
        )
        .expect("jsonl with lucide icon should parse");
    }

    #[test]
    fn parser_accepts_audio_nodes() {
        parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"frames":180}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"bgm","parentId":"root","type":"audio","path":"/tmp/demo.mp3"}"#,
        )
        .expect("jsonl with audio path should parse");

        parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"frames":180}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"stream","parentId":"root","type":"audio","url":"https://example.com/demo.mp3"}"#,
        )
        .expect("jsonl with audio url should parse");
    }

    #[test]
    fn parser_treats_root_audio_as_timeline_audio_source() {
        let parsed = parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"frames":180}
{"id":"bgm","parentId":null,"type":"audio","path":"/tmp/demo.mp3"}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}"#,
        )
        .expect("jsonl with global audio should parse");

        assert_eq!(parsed.audio_sources.len(), 1);
        assert!(matches!(
            parsed.audio_sources[0].attach,
            AudioAttachment::Timeline
        ));
        assert!(matches!(parsed.root.kind(), NodeKind::Div(_)));
    }

    #[test]
    fn parser_attaches_nested_audio_to_owning_scene() {
        let parsed = parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"frames":180}
{"id":"scene-a","parentId":null,"type":"div","className":"w-full h-full","duration":30}
{"id":"content","parentId":"scene-a","type":"div","className":"w-full h-full"}
{"id":"voice","parentId":"content","type":"audio","path":"/tmp/voice.mp3"}"#,
        )
        .expect("jsonl with scene audio should parse");

        assert_eq!(parsed.audio_sources.len(), 1);
        assert!(matches!(
            &parsed.audio_sources[0].attach,
            AudioAttachment::Scene { scene_id } if scene_id == "scene-a"
        ));
    }
}
