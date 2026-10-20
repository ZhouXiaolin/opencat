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
}
