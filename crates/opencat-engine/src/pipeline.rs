//! Open compositions with engine-embedded default fonts + document `<fonts>`.

use std::sync::Arc;

use anyhow::Result;

use opencat_core::parse::preflight::collect_external_manifest;
use opencat_core::parse::{
    BuildOptions, CanvasChildrenMode, build_font_resources, build_parsed_document,
    parse_parts_with_base_dir,
};
use opencat_core::pipeline::DefaultPipeline;

use crate::EnginePipeline;
use crate::fonts::engine_default_font_db;
use crate::js_context::RqJsContext;
use crate::resource::loader::EngineLoader;

/// Parse and open a composition with default Noto fonts and any `<fonts>` from markup.
pub fn open(input: &str, mut loader: EngineLoader, scripts: RqJsContext) -> Result<EnginePipeline> {
    if input.trim().starts_with('{') {
        let base_dir = loader
            .base_dir()
            .canonicalize()
            .unwrap_or_else(|_| loader.base_dir().to_path_buf());
        let parsed = crate::source_io::parse_with_base_dir(input, Some(&base_dir))?;
        return DefaultPipeline::open_parsed(parsed, loader, scripts, engine_default_font_db());
    }

    let base_dir = loader.base_dir();
    let parts = parse_parts_with_base_dir(input, Some(base_dir))?;
    let font_manifest = parts.font_manifest.clone();
    let bytes = loader.load_font_manifest(&parts.font_manifest)?;
    loader.register_font_handles(&parts.font_manifest, &bytes)?;

    let mut font_db = engine_default_font_db();
    let font_index = if parts.font_manifest.is_empty() {
        None
    } else {
        let (db, index) = build_font_resources((*font_db).clone(), &parts.font_manifest, &bytes)?;
        font_db = Arc::new(db);
        Some(index)
    };

    let parsed = build_parsed_document(
        parts,
        BuildOptions {
            canvas_children_mode: CanvasChildrenMode::HiddenPictureSubtree,
        },
        font_index.as_ref(),
    )?;

    let mut pipeline = DefaultPipeline::open_parsed(parsed, loader, scripts, font_db)?;

    let (_, external_manifest) = collect_external_manifest(pipeline.composition(), &font_manifest);
    pipeline
        .loader_mut()
        .build_resource_provider(&external_manifest);

    Ok(pipeline)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use opencat_core::{
        frame_ctx::FrameCtx,
        parse::{
            node::{Node, NodeKind},
            primitives::CaptionNode,
        },
        script::js_context::JsContext,
    };

    use super::open;

    fn find_caption<'a>(node: &'a Node, id: &str) -> Option<&'a CaptionNode> {
        match node.kind() {
            NodeKind::Caption(caption) if caption.style_ref().id == id => Some(caption),
            NodeKind::Div(div) => div
                .children_ref()
                .iter()
                .find_map(|child| find_caption(child, id)),
            _ => None,
        }
    }

    #[test]
    fn jsonl_caption_path_resolves_relative_to_loader_base_dir() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let fixture_dir = std::path::PathBuf::from(format!("target/opencat-jsonl-caption-{nanos}"));
        let cache_dir = fixture_dir.join("cache");
        std::fs::create_dir_all(&cache_dir).expect("cache dir");
        std::fs::write(
            fixture_dir.join("sub.srt"),
            "1\n00:00:00,000 --> 00:00:00,500\nHello CLI\n",
        )
        .expect("srt fixture");

        let jsonl = r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":1}
{"id":"root","parentId":null,"type":"div","className":"relative w-[320px] h-[180px]"}
{"id":"subs","parentId":"root","type":"caption","className":"absolute left-[0px] top-[0px] text-white","path":"sub.srt"}"#;

        let loader = crate::resource::loader::EngineLoader::new(fixture_dir.clone(), cache_dir)
            .expect("loader");
        let ctx = crate::js_context::RqJsContext::new().expect("js context");
        let pipeline = open(jsonl, loader, ctx).expect("pipeline");

        let root = pipeline.composition().root_node(&FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 30,
        });
        let caption = find_caption(&root, "subs").expect("caption node");

        assert_eq!(caption.entries_ref().len(), 1);
        assert_eq!(caption.active_text(0), Some("Hello CLI"));

        std::fs::remove_dir_all(&fixture_dir).ok();
    }
}
