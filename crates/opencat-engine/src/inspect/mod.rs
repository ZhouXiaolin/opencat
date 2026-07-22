//! Engine-side layout inspection facade.
//!
//! Prefer inspecting an already-opened host pipeline
//! ([`crate::pipeline::EnginePipelineHost::inspect_frame`] or
//! [`opencat_core::pipeline::DefaultPipeline::inspect_frame`]) so catalog,
//! fonts, scripts, and layout sessions match render.
//!
//! [`collect_frame_layout_rects`] is a **media-less fixture helper** for pure
//! layout comparisons (Tailwind/Taffy). It opens a one-shot lifecycle pipeline
//! with empty host inputs — not for scripted or media-bearing comps.

use anyhow::Result;

use opencat_core::lifecycle::{CompositionDraft, HostInputs};
use opencat_core::parse::ParsedComposition;
use opencat_core::pipeline::DefaultPipeline;
use opencat_core::script::js_context::JsContext;

pub use opencat_core::pipeline::FrameElementRect;

/// Open a lifecycle pipeline for pure layout inspection of a
/// [`ParsedComposition`] that needs no media bytes.
///
/// Uses empty host inputs and the engine default font database. For
/// compositions with images/video/fonts/scripts, open through the real engine
/// host path ([`crate::pipeline::open`]) and call
/// [`crate::pipeline::EnginePipelineHost::inspect_frame`] instead.
fn open_inspect_pipeline(
    parsed: ParsedComposition,
) -> Result<DefaultPipeline<crate::js_context::RqJsContext>> {
    let draft = CompositionDraft::from_parsed(parsed);
    let inputs = HostInputs::empty()
        .with_base_font_faces(crate::fonts::engine_default_font_faces())
        .with_sans_serif_family("Noto Sans SC");
    let prepared = draft.prepare(inputs).map_err(|e| anyhow::anyhow!("{e}"))?;
    let scripts = crate::js_context::RqJsContext::new()?;
    prepared.open_pipeline(scripts)
}

/// Collect layout rects for a media-less fixture composition.
///
/// Builds a one-shot inspect pipeline from the composition root (empty catalog,
/// default fonts). Prefer
/// [`crate::pipeline::EnginePipelineHost::inspect_frame`] when a live prepared
/// pipeline already exists so scripts / catalog / layout session stay shared
/// with render.
pub fn collect_frame_layout_rects(
    composition: &opencat_core::parse::composition::Composition,
    frame_index: u32,
) -> Result<Vec<FrameElementRect>> {
    use opencat_core::parse::ParsedComposition;
    use opencat_core::resource::fonts::FontManifest;

    let max_frame = composition.frames.max(1).saturating_sub(1);
    let clamped = frame_index.min(max_frame);
    // Freeze the composition root for the requested frame into a one-shot
    // media-less pipeline. Suitable for static Tailwind fixtures only.
    let root = composition.root_node(&opencat_core::frame_ctx::FrameCtx {
        frame: clamped,
        fps: composition.fps,
        width: composition.width,
        height: composition.height,
        frames: composition.frames.max(1),
    });
    let parsed = ParsedComposition {
        width: composition.width,
        height: composition.height,
        fps: composition.fps as i32,
        duration: composition.duration,
        root,
        script: None,
        audio_sources: composition.audio_sources().to_vec(),
        font_manifest: FontManifest::default(),
    };
    let mut pipeline = open_inspect_pipeline(parsed)?;
    pipeline.inspect_frame(clamped)
}

pub mod browser;

#[cfg(test)]
mod tests;
