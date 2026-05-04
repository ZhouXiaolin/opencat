use anyhow::Result;

use crate::resource::assets::AssetId;
use crate::core::scene::primitives::{AudioSource, ImageSource};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoInfoMeta {
    pub width: u32,
    pub height: u32,
    pub duration_secs: Option<f64>,
}

pub trait ResourceCatalog {
    fn resolve_image(&mut self, src: &ImageSource) -> Result<AssetId>;
    fn resolve_audio(&mut self, src: &AudioSource) -> Result<AssetId>;
    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId;
    fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()>;
    fn dimensions(&self, id: &AssetId) -> (u32, u32);
    fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::assets::AssetsMap;
    use crate::core::scene::primitives::ImageSource;

    #[test]
    fn assets_map_implements_resource_catalog_register_dimensions_returns_stable_id() {
        let mut catalog: Box<dyn ResourceCatalog> = Box::new(AssetsMap::new());
        let id1 = catalog.register_dimensions("/tmp/a.png", 100, 200);
        let id2 = catalog.register_dimensions("/tmp/a.png", 100, 200);
        assert_eq!(id1, id2);
        assert_eq!(catalog.dimensions(&id1), (100, 200));
    }

    #[test]
    fn assets_map_resolve_image_returns_stable_id_for_path() {
        let mut catalog = AssetsMap::new();
        let src = ImageSource::Path(std::path::PathBuf::from("/tmp/b.png"));
        let id1 = (&mut catalog as &mut dyn ResourceCatalog).resolve_image(&src).unwrap();
        let id2 = (&mut catalog as &mut dyn ResourceCatalog).resolve_image(&src).unwrap();
        assert_eq!(id1, id2);
    }

    #[test]
    fn assets_map_video_info_returns_none_when_not_probed() {
        let mut catalog: Box<dyn ResourceCatalog> = Box::new(AssetsMap::new());
        let id = catalog.register_dimensions("/tmp/v.mp4", 0, 0);
        assert!(catalog.video_info(&id).is_none());
    }
}
