use crate::style::{NodeStyle, impl_node_style_api};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OpenverseQuery {
    pub query: String,
    pub count: usize,
    pub aspect_ratio: Option<String>,
}

/// Image source locator. Paths are **logical** (document-relative strings), not
/// host filesystem paths — core never joins a base directory or stores `PathBuf`.
/// Hosts interpret `Path` against their own document base (FS, VFS, URL).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageSource {
    Unset,
    /// Logical locator (e.g. `"photos/a.png"`). Not a resolved filesystem path.
    Path(String),
    Url(String),
    Query(OpenverseQuery),
}

#[derive(Clone)]
pub struct Image {
    source: ImageSource,
    pub(crate) style: NodeStyle,
}

impl Image {
    /// Set a logical path locator. Accepts any string-like value; does not
    /// resolve against a filesystem base.
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.source = ImageSource::Path(path.into());
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

    #[test]
    fn image_path_stores_logical_string() {
        let image = image().path("photos/a.png");
        assert_eq!(
            image.source(),
            &ImageSource::Path("photos/a.png".to_string())
        );
    }
}
