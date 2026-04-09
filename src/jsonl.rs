use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::scene::{
    node::Node,
    primitives::{ImageSource, OpenverseQuery},
};
use crate::style::NodeStyle;

mod builder;
mod tailwind;

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
}

pub fn parse(input: &str) -> anyhow::Result<ParsedComposition> {
    let mut width = 1920;
    let mut height = 1080;
    let mut fps = 30;
    let mut frames = 90;
    let mut global_scripts = Vec::new();
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
        scene::node::NodeKind,
        scene::time::{FrameState, frame_state_for_root},
        style::{ColorToken, GradientDirection, TextAlign},
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
        assert_eq!(style.margin_top, Some(4.0));
        assert_eq!(style.margin_bottom, Some(16.0));
        assert_eq!(style.padding_bottom, Some(20.0));
    }

    #[test]
    fn parser_maps_tailwind_spacing_scale_and_text_sizes() {
        let style = parse_class_name("w-20 h-28 left-8 top-4 px-6 py-2 gap-3.5 text-3xl");

        assert_eq!(style.width, Some(80.0));
        assert_eq!(style.height, Some(112.0));
        assert_eq!(style.inset_left, Some(32.0));
        assert_eq!(style.inset_top, Some(16.0));
        assert_eq!(style.padding_x, Some(24.0));
        assert_eq!(style.padding_y, Some(8.0));
        assert_eq!(style.gap, Some(14.0));
        assert_eq!(style.text_px, Some(30.0));
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
        assert_eq!(style.inset_right, Some(0.0));
        assert_eq!(style.inset_bottom, Some(0.0));
        assert_eq!(style.inset_top, Some(-80.0));
        assert_eq!(style.inset_left, Some(-24.0));
        assert_eq!(style.z_index, Some(10));
        assert_eq!(style.flex_shrink, Some(0.0));
        assert_eq!(style.letter_spacing, Some(1.0));
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
}
