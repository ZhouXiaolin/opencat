//! src/core/test_support.rs
#![cfg(test)]

use std::sync::Arc;

pub fn mock_font_provider() -> impl crate::core::text::FontProvider {
    crate::core::text::DefaultFontProvider::from_arc(Arc::new(
        crate::core::text::default_font_db_with_embedded_only(),
    ))
}

#[derive(Default)]
pub struct MockScriptHost {
    next_id: u64,
    map: std::collections::HashMap<String, u64>,
}

impl crate::core::scene::script::ScriptHost for MockScriptHost {
    fn install(
        &mut self,
        source: &str,
    ) -> anyhow::Result<crate::core::scene::script::ScriptDriverId> {
        let id = *self
            .map
            .entry(source.to_string())
            .or_insert_with(|| {
                self.next_id += 1;
                self.next_id
            });
        Ok(crate::core::scene::script::ScriptDriverId(id))
    }
    fn register_text_source(
        &mut self,
        _: &str,
        _: crate::core::scene::script::ScriptTextSource,
    ) {
    }
    fn clear_text_sources(&mut self) {}
    fn run_frame(
        &mut self,
        _: crate::core::scene::script::ScriptDriverId,
        _: &crate::core::frame_ctx::ScriptFrameCtx,
        _current_node_id: Option<&str>,
    ) -> anyhow::Result<crate::core::scene::script::StyleMutations> {
        Ok(crate::core::scene::script::StyleMutations::default())
    }
}
