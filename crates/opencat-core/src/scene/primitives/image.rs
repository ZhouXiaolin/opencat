use std::path::{Path, PathBuf};

use crate::style::{NodeStyle, impl_node_style_api};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OpenverseQuery {
    pub query: String,
    pub count: usize,
    pub aspect_ratio: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageSource {
    Unset,
    Path(PathBuf),
    Url(String),
    Query(OpenverseQuery),
}

#[derive(Clone)]
pub struct Image {
    source: ImageSource,
    pub(crate) style: NodeStyle,
}

impl Image {
    pub fn path(mut self, path: impl AsRef<Path>) -> Self {
        self.source = ImageSource::Path(path.as_ref().to_path_buf());
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.source = ImageSource::Url(url.into());
        self
    }

    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.source = ImageSource::Query(OpenverseQuery {
            query: query.into(),
            count: 1,
            aspect_ratio: None,
        });
        self
    }

    pub fn query_count(mut self, count: usize) -> Self {
        if let ImageSource::Query(query) = &mut self.source {
            query.count = count.max(1);
        }
        self
    }

    pub fn aspect_ratio(mut self, aspect_ratio: impl Into<String>) -> Self {
        if let ImageSource::Query(query) = &mut self.source {
            query.aspect_ratio = Some(aspect_ratio.into());
        }
        self
    }

    pub fn source(&self) -> &ImageSource {
        &self.source
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

pub fn image() -> Image {
    Image {
        source: ImageSource::Unset,
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Image);

#[cfg(test)]
mod tests {
    use super::{ImageSource, image};

    #[test]
    fn image_query_builder_keeps_openverse_options() {
        let image = image().query("cats").query_count(3).aspect_ratio("square");

        let ImageSource::Query(query) = image.source() else {
            panic!("expected query image source");
        };

        assert_eq!(query.query, "cats");
        assert_eq!(query.count, 3);
        assert_eq!(query.aspect_ratio.as_deref(), Some("square"));
    }
}
