use std::path::Path;

use crate::{
    nodes::{ImageSource, OpenverseQuery},
    style::{NodeStyle, impl_node_style_api},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CanvasAsset {
    pub asset_id: String,
    pub source: ImageSource,
}

#[derive(Clone)]
pub struct Canvas {
    pub(crate) style: NodeStyle,
    pub(crate) assets: Vec<CanvasAsset>,
}

impl Canvas {
    pub fn asset(mut self, asset_id: impl Into<String>, source: ImageSource) -> Self {
        self.assets.push(CanvasAsset {
            asset_id: asset_id.into(),
            source,
        });
        self
    }

    pub fn asset_path(self, asset_id: impl Into<String>, path: impl AsRef<Path>) -> Self {
        self.asset(asset_id, ImageSource::Path(path.as_ref().to_path_buf()))
    }

    pub fn asset_url(self, asset_id: impl Into<String>, url: impl Into<String>) -> Self {
        self.asset(asset_id, ImageSource::Url(url.into()))
    }

    pub fn asset_query(self, asset_id: impl Into<String>, query: impl Into<String>) -> Self {
        self.asset(
            asset_id,
            ImageSource::Query(OpenverseQuery {
                query: query.into(),
                count: 1,
                aspect_ratio: None,
            }),
        )
    }

    pub fn asset_query_count(mut self, asset_id: &str, count: usize) -> Self {
        if let Some(asset) = self.assets.iter_mut().find(|asset| asset.asset_id == asset_id) {
            if let ImageSource::Query(query) = &mut asset.source {
                query.count = count.max(1);
            }
        }
        self
    }

    pub fn asset_aspect_ratio(mut self, asset_id: &str, aspect_ratio: impl Into<String>) -> Self {
        if let Some(asset) = self.assets.iter_mut().find(|asset| asset.asset_id == asset_id) {
            if let ImageSource::Query(query) = &mut asset.source {
                query.aspect_ratio = Some(aspect_ratio.into());
            }
        }
        self
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    pub fn assets_ref(&self) -> &[CanvasAsset] {
        &self.assets
    }
}

pub fn canvas() -> Canvas {
    Canvas {
        style: NodeStyle::default(),
        assets: Vec::new(),
    }
}

impl_node_style_api!(Canvas);
