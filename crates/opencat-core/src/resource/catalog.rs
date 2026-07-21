use anyhow::Result;

use crate::ir::asset_id::AssetId;
use crate::parse::primitives::{AudioSource, ImageSource};
use crate::resource::lottie::LottieMeta;
use crate::time::DurationMicros;

/// Host-facing video metadata used during resolve/render.
///
/// Duration is always microsecond-based. Layout-critical width/height must be
/// positive; duration may be `None` when the probe could not determine length
/// (looping/clamp then treat the stream as open-ended).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoInfoMeta {
    pub width: u32,
    pub height: u32,
    pub duration_micros: Option<DurationMicros>,
}

impl VideoInfoMeta {
    pub fn duration_secs(&self) -> Option<f64> {
        self.duration_micros
            .map(|d| crate::time::timestamp_micros_to_secs(d.0))
    }
}

pub trait ResourceResolver {
    fn resolve_image(&mut self, src: &ImageSource) -> Result<AssetId>;
    fn resolve_audio(&mut self, src: &AudioSource) -> Result<AssetId>;
    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId;
    fn register_video_dimensions(
        &mut self,
        locator: &str,
        width: u32,
        height: u32,
        duration_secs: Option<f64>,
    ) -> AssetId;
    fn register_audio(&mut self, locator: &str) -> AssetId;
    fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()>;
    fn dimensions(&self, id: &AssetId) -> (u32, u32);
    fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta>;
    /// Resolve a pipeline-internal alias to its canonical `AssetId`.
    ///
    /// Returns `None` when `alias` is not a registered alias. Callers that
    /// need a canonical id should fall back to treating the input as already
    /// canonical and validate it against known assets separately.
    fn resolve_alias(&self, alias: &AssetId) -> Option<AssetId> {
        let _ = alias;
        None
    }
    /// Returns true when `id` is a known canonical asset (declared and probed)
    /// or a registered alias. Used to reject unknown alias references before
    /// they reach `DrawOpFrame` / `FrameMediaPlan`.
    fn is_known_asset(&self, id: &AssetId) -> bool {
        let _ = id;
        true
    }
    fn resolve_lottie(&mut self, element_id: &str) -> Result<AssetId> {
        let _ = element_id;
        anyhow::bail!("resolve_lottie not implemented")
    }
    fn lottie_meta(&self, id: &AssetId) -> Option<LottieMeta> {
        let _ = id;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::primitives::ImageSource;
    use crate::test_support::TestCatalog;

    #[test]
    fn assets_map_implements_resource_catalog_register_dimensions_returns_stable_id() {
        let mut catalog: Box<dyn ResourceResolver> = Box::new(TestCatalog::new());
        let id1 = catalog.register_dimensions("/tmp/a.png", 100, 200);
        let id2 = catalog.register_dimensions("/tmp/a.png", 100, 200);
        assert_eq!(id1, id2);
        assert_eq!(catalog.dimensions(&id1), (100, 200));
    }

    #[test]
    fn assets_map_resolve_image_returns_stable_id_for_path() {
        let mut catalog = TestCatalog::new();
        let src = ImageSource::Path("/tmp/b.png".into());
        let id1 = (&mut catalog as &mut dyn ResourceResolver)
            .resolve_image(&src)
            .unwrap();
        let id2 = (&mut catalog as &mut dyn ResourceResolver)
            .resolve_image(&src)
            .unwrap();
        assert_eq!(id1, id2);
    }

    #[test]
    fn assets_map_video_info_returns_none_when_not_probed() {
        let mut catalog: Box<dyn ResourceResolver> = Box::new(TestCatalog::new());
        let id = catalog.register_dimensions("/tmp/v.mp4", 0, 0);
        assert!(catalog.video_info(&id).is_none());
    }

    #[test]
    fn alias_rejects_unknown_target() {
        // AC3: an alias must point at a declared asset; unknown target is a
        // render error, not a silent no-op.
        let mut catalog: Box<dyn ResourceResolver> = Box::new(TestCatalog::new());
        let unknown = AssetId("does-not-exist".into());
        let alias = AssetId("aka".into());
        let err = catalog.alias(alias, &unknown).unwrap_err();
        assert!(
            err.to_string().contains("not a declared asset"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn alias_binds_to_canonical_and_resolves() {
        let mut catalog: Box<dyn ResourceResolver> = Box::new(TestCatalog::new());
        let canonical = catalog.register_dimensions("/tmp/a.png", 10, 20);
        let alias = AssetId("hero".into());
        catalog.alias(alias.clone(), &canonical).unwrap();

        assert_eq!(catalog.resolve_alias(&alias), Some(canonical.clone()));
        assert!(catalog.is_known_asset(&alias));
        assert!(catalog.is_known_asset(&canonical));
    }

    #[test]
    fn is_known_asset_rejects_unknown_id() {
        let catalog: Box<dyn ResourceResolver> = Box::new(TestCatalog::new());
        assert!(!catalog.is_known_asset(&AssetId("nope".into())));
    }
}
