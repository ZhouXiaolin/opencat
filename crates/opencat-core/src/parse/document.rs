use crate::fonts::FontManifest;
use crate::parse::composition::CompositionAudioSource;
use crate::parse::node::Node;

mod builder;

pub use builder::{build_font_resources, join_scripts, script_driver_from_decls};

/// Script declaration collected during parse. External paths stay logical;
/// host supplies text via lifecycle [`crate::lifecycle::HostInputs`] (issue #20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclaredScript {
    /// Inline `src` / `content` / markup `<script>` body.
    Inline(String),
    /// Logical path or URL locator — not read by core.
    External { locator: String },
}

/// html 前端的构建上下文快照：运行期 `appendChild` 的宿主子树重建
/// （resolve 期 `build_node`）需要与主树同一套级联输入。构建后不可变，
/// Arc 共享进渲染管线。
#[derive(Clone)]
pub struct HtmlBuildCtx {
    pub css: Option<std::sync::Arc<crate::css::CssEngine>>,
    pub manifest: crate::fonts::FontManifest,
    pub base_dir: Option<std::path::PathBuf>,
    pub html_frontend: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedComposition {
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub duration: f64,
    pub root: Node,
    pub script: Option<String>,
    pub audio_sources: Vec<CompositionAudioSource>,
    pub font_manifest: FontManifest,
    /// html 前端才有：结构突变重建子树用的构建上下文。
    pub html_build: Option<std::sync::Arc<HtmlBuildCtx>>,
}

impl std::fmt::Debug for HtmlBuildCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HtmlBuildCtx")
            .field("css", &self.css.is_some())
            .field("manifest", &self.manifest)
            .field("base_dir", &self.base_dir)
            .field("html_frontend", &self.html_frontend)
            .finish()
    }
}
