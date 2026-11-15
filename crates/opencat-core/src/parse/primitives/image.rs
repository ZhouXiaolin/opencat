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
    /// `<img width height>` 属性给的内在尺寸（HTML 的呈现提示）：CSS 尺寸
    /// 优先，属性次之，都没有才用素材自身像素尺寸。
    pub(crate) attr_size: Option<(f32, f32)>,
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

    pub fn attr_size(mut self, width: f32, height: f32) -> Self {
        self.attr_size = Some((width, height));
        self
    }

    pub(crate) fn attr_size_ref(&self) -> Option<(f32, f32)> {
        self.attr_size
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
        attr_size: None,
    }
}

impl_node_style_api!(Image);


