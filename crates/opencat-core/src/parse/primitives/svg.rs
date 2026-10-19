use std::path::PathBuf;

use crate::style::{NodeStyle, impl_node_style_api};

/// `<svg src="…">` / `<svg inline>`：SVG 作为一个元素渲染（PLAN §4）。
///
/// `src` 在 parse 层已相对 jsx base_dir 解析成绝对路径；`inline` 是内联
/// markup（打包进 bundle 的 react 图标），二者互斥、inline 优先。
/// `width`/`height` 覆盖 svg 的 intrinsic 尺寸参与布局；
/// `preserve_aspect_ratio` 保留透传。
#[derive(Clone)]
pub struct Svg {
    pub(crate) src: PathBuf,
    pub(crate) inline: Option<String>,
    pub(crate) width: Option<f32>,
    pub(crate) height: Option<f32>,
    pub(crate) preserve_aspect_ratio: Option<String>,
    pub(crate) style: NodeStyle,
}

impl Svg {
    pub fn src(&self) -> &PathBuf {
        &self.src
    }

    pub fn inline(&self) -> Option<&str> {
        self.inline.as_deref()
    }

    pub fn width(&self) -> Option<f32> {
        self.width
    }

    pub fn height(&self) -> Option<f32> {
        self.height
    }

    pub fn preserve_aspect_ratio(&self) -> Option<&str> {
        self.preserve_aspect_ratio.as_deref()
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

pub fn svg(src: impl Into<PathBuf>) -> Svg {
    Svg {
        src: src.into(),
        inline: None,
        width: None,
        height: None,
        preserve_aspect_ratio: None,
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Svg);
