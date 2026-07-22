//! src/core/test_support.rs

use std::collections::HashMap;
use std::sync::Arc;

use crate::ir::asset_id::{AssetId, ResourceKind};
use crate::parse::primitives::{AudioSource, ImageSource};
use crate::resource::catalog::VideoInfoMeta;

pub fn mock_font_provider() -> impl crate::text::FontProvider {
    crate::text::DefaultFontProvider::from_arc(Arc::new(crate::text::test_default_font_db()))
}

/// Default font face bytes for testing: CJK sans + color emoji.
/// Use with [`HostInputs::with_base_font_faces`] and
/// [`HostInputs::with_sans_serif_family`].
#[cfg(any(test, feature = "test-support"))]
pub fn test_font_faces() -> Vec<Vec<u8>> {
    vec![
        include_bytes!("../../../assets/NotoSansSC-Regular.otf").to_vec(),
        include_bytes!("../../../assets/NotoColorEmoji.ttf").to_vec(),
    ]
}

pub struct TestCatalog {
    dims: HashMap<AssetId, (u32, u32)>,
    video_info: HashMap<AssetId, VideoInfoMeta>,
    aliases: HashMap<AssetId, AssetId>,
}

impl TestCatalog {
    pub fn new() -> Self {
        Self {
            dims: HashMap::new(),
            video_info: HashMap::new(),
            aliases: HashMap::new(),
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
        let dims = self
            .dims
            .get(target)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("alias target {target:?} is not a declared asset"))?;
        self.dims.insert(alias.clone(), dims);
        self.aliases.insert(alias, target.clone());
        Ok(())
    }

    pub fn is_known_asset(&self, id: &AssetId) -> bool {
        self.dims.contains_key(id) || self.aliases.contains_key(id)
    }

    pub fn resolve_alias(&self, alias: &AssetId) -> Option<AssetId> {
        self.aliases.get(alias).cloned()
    }

    pub fn register_image_source(&mut self, source: &ImageSource) -> anyhow::Result<AssetId> {
        match source {
            ImageSource::Unset => anyhow::bail!("image source is required"),
            ImageSource::Path(path) => {
                let id = AssetId::new(ResourceKind::Image, path.clone());
                self.dims.entry(id.clone()).or_insert((0, 0));
                Ok(id)
            }
            ImageSource::Url(url) => {
                let id = crate::ir::asset_id::asset_id_for_url(url);
                self.dims.entry(id.clone()).or_insert((0, 0));
                Ok(id)
            }
            ImageSource::Query(query) => {
                let id = crate::ir::asset_id::asset_id_for_query(query);
                self.dims.entry(id.clone()).or_insert((0, 0));
                Ok(id)
            }
        }
    }

    pub fn register_audio_source(&mut self, source: &AudioSource) -> anyhow::Result<AssetId> {
        match source {
            AudioSource::Unset => anyhow::bail!("audio source is required"),
            AudioSource::Path(path) => {
                let id = AssetId::new(
                    ResourceKind::Audio,
                    format!("audio:path:{}", path.to_string_lossy()),
                );
                self.dims.entry(id.clone()).or_insert((0, 0));
                Ok(id)
            }
            AudioSource::Url(url) => Ok(crate::ir::asset_id::asset_id_for_audio_url(url)),
        }
    }

    pub fn resolve_image(&mut self, src: &ImageSource) -> anyhow::Result<AssetId> {
        self.register_image_source(src)
    }

    pub fn resolve_audio(&mut self, src: &AudioSource) -> anyhow::Result<AssetId> {
        self.register_audio_source(src)
    }

    pub fn register_video_dimensions(
        &mut self,
        locator: &str,
        width: u32,
        height: u32,
        duration_secs: Option<f64>,
    ) -> AssetId {
        let id = AssetId::new(ResourceKind::Video, locator.to_string());
        self.dims.insert(id.clone(), (width, height));
        if let Some(duration) = duration_secs {
            self.video_info.insert(
                id.clone(),
                VideoInfoMeta {
                    width,
                    height,
                    duration_micros: crate::time::optional_secs_to_duration_micros(Some(duration)),
                },
            );
        }
        id
    }

    pub fn register_audio(&mut self, locator: &str) -> AssetId {
        let id = AssetId::new(ResourceKind::Audio, locator.to_string());
        self.dims.entry(id.clone()).or_insert((0, 0));
        id
    }

    pub fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta> {
        self.video_info.get(id).copied()
    }

    pub fn resolve_lottie(&mut self, src: &crate::parse::primitives::LottieSource) -> anyhow::Result<AssetId> {
        crate::ir::asset_id::asset_id_for_lottie(src)
            .ok_or_else(|| anyhow::anyhow!("unset lottie source"))
    }
}

impl Default for TestCatalog {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct MockScriptHost {
    next_id: u64,
    map: std::collections::HashMap<String, u64>,
}

impl crate::script::ScriptHost for MockScriptHost {
    fn install(&mut self, source: &str) -> anyhow::Result<crate::script::ScriptDriverId> {
        let id = *self.map.entry(source.to_string()).or_insert_with(|| {
            self.next_id += 1;
            self.next_id
        });
        Ok(crate::script::ScriptDriverId(id))
    }
    fn register_text_source(&mut self, _: &str, _: crate::script::ScriptTextSource) {}
    fn clear_text_sources(&mut self) {}
    fn run_frame(
        &mut self,
        _: crate::script::ScriptDriverId,
        _: &crate::frame_ctx::ScriptFrameCtx,
        _current_node_id: Option<&str>,
        _recorder: &mut dyn crate::script::recorder::MutationRecorder,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn set_target_registry(&mut self, _: crate::script::ScriptTargetRegistry) {}
}
