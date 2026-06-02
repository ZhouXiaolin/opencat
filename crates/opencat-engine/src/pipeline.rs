//! Open compositions with engine-embedded default fonts + document `<fonts>`.

use std::sync::Arc;

use anyhow::Result;

use opencat_core::parse::preflight::collect_external_manifest;
use opencat_core::parse::{
    BuildOptions, CanvasChildrenMode, build_font_resources, build_parsed_document,
    parse_parts_with_base_dir,
};
use opencat_core::pipeline::DefaultPipeline;

use crate::fonts::engine_default_font_db;
use crate::js_context::RqJsContext;
use crate::resource::loader::EngineLoader;
use crate::EnginePipeline;

/// Parse and open a composition with default Noto fonts and any `<fonts>` from markup.
pub fn open(input: &str, mut loader: EngineLoader, scripts: RqJsContext) -> Result<EnginePipeline> {
    if input.trim().starts_with('{') {
        return DefaultPipeline::open_with_font_db(input, loader, scripts, engine_default_font_db());
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
        let (db, index) =
            build_font_resources((*font_db).clone(), &parts.font_manifest, &bytes)?;
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

    let (_, external_manifest) =
        collect_external_manifest(pipeline.composition(), &font_manifest);
    pipeline
        .loader_mut()
        .build_resource_provider(&external_manifest);

    Ok(pipeline)
}