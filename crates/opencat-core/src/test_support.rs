//! src/core/test_support.rs

use std::sync::Arc;

pub fn mock_font_provider() -> impl crate::text::FontProvider {
    crate::text::DefaultFontProvider::from_arc(Arc::new(
        crate::text::default_font_db_with_embedded_only(),
    ))
}

#[derive(Default)]
pub struct MockScriptHost {
    next_id: u64,
    map: std::collections::HashMap<String, u64>,
}

impl crate::scene::script::ScriptHost for MockScriptHost {
    fn install(
        &mut self,
        source: &str,
    ) -> anyhow::Result<crate::scene::script::ScriptDriverId> {
        let id = *self
            .map
            .entry(source.to_string())
            .or_insert_with(|| {
                self.next_id += 1;
                self.next_id
            });
        Ok(crate::scene::script::ScriptDriverId(id))
    }
    fn register_text_source(
        &mut self,
        _: &str,
        _: crate::scene::script::ScriptTextSource,
    ) {
    }
    fn clear_text_sources(&mut self) {}
    fn run_frame(
        &mut self,
        _: crate::scene::script::ScriptDriverId,
        _: &crate::frame_ctx::ScriptFrameCtx,
        _current_node_id: Option<&str>,
    ) -> anyhow::Result<crate::scene::script::StyleMutations> {
        Ok(crate::scene::script::StyleMutations::default())
    }
}
