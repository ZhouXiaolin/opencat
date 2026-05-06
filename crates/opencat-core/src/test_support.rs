//! src/core/test_support.rs

use std::collections::HashMap;
use std::sync::Arc;

use crate::resource::asset_id::AssetId;
use crate::resource::catalog::{ResourceCatalog, VideoInfoMeta};
use crate::scene::primitives::{AudioSource, ImageSource};

pub fn mock_font_provider() -> impl crate::text::FontProvider {
    crate::text::DefaultFontProvider::from_arc(Arc::new(
        crate::text::default_font_db_with_embedded_only(),
    ))
}

pub struct TestCatalog {
    dims: HashMap<AssetId, (u32, u32)>,
    video_info: HashMap<AssetId, VideoInfoMeta>,
}

impl TestCatalog {
    pub fn new() -> Self {
        Self {
            dims: HashMap::new(),
            video_info: HashMap::new(),
        }
    }

    pub fn register_dimensions(&mut self, id: AssetId, width: u32, height: u32) -> AssetId {
        self.dims.insert(id.clone(), (width, height));
        id
    }

    pub fn register_video_info(&mut self, id: AssetId, info: VideoInfoMeta) -> AssetId {
        self.dims.insert(id.clone(), (info.width, info.height));
        self.video_info.insert(id.clone(), info);
        id
    }

    pub fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        self.dims.get(id).copied().unwrap_or((0, 0))
    }

    pub fn alias(&mut self, alias: AssetId, target: &AssetId) -> anyhow::Result<()> {
        if let Some(&dims) = self.dims.get(target) {
            self.dims.insert(alias, dims);
        }
        Ok(())
    }

    pub fn register_image_source(&mut self, source: &ImageSource) -> anyhow::Result<AssetId> {
        match source {
            ImageSource::Unset => anyhow::bail!("image source is required"),
            ImageSource::Path(path) => {
                let id = AssetId(path.to_string_lossy().into_owned());
                self.dims.entry(id.clone()).or_insert((0, 0));
                Ok(id)
            }
            ImageSource::Url(url) => {
                let id = crate::resource::asset_id::asset_id_for_url(url);
                self.dims.entry(id.clone()).or_insert((0, 0));
                Ok(id)
            }
            ImageSource::Query(query) => {
                let id = crate::resource::asset_id::asset_id_for_query(query);
                self.dims.entry(id.clone()).or_insert((0, 0));
                Ok(id)
            }
        }
    }

    pub fn register_audio_source(&mut self, source: &AudioSource) -> anyhow::Result<AssetId> {
        match source {
            AudioSource::Unset => anyhow::bail!("audio source is required"),
            AudioSource::Path(path) => {
                let id = AssetId(format!("audio:path:{}", path.to_string_lossy()));
                self.dims.entry(id.clone()).or_insert((0, 0));
                Ok(id)
            }
            AudioSource::Url(url) => Ok(crate::resource::asset_id::asset_id_for_audio_url(url)),
        }
    }
}

impl ResourceCatalog for TestCatalog {
    fn resolve_image(&mut self, src: &ImageSource) -> anyhow::Result<AssetId> {
        self.register_image_source(src)
    }

    fn resolve_audio(&mut self, src: &AudioSource) -> anyhow::Result<AssetId> {
        self.register_audio_source(src)
    }

    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId {
        TestCatalog::register_dimensions(self, AssetId(locator.to_string()), width, height)
    }

    fn alias(&mut self, alias: AssetId, target: &AssetId) -> anyhow::Result<()> {
        TestCatalog::alias(self, alias, target)
    }

    fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        TestCatalog::dimensions(self, id)
    }

    fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta> {
        self.video_info.get(id).copied()
    }
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
        _recorder: &mut dyn crate::script::recorder::MutationRecorder,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
