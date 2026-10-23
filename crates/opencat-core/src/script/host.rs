use std::collections::HashSet;

use anyhow::Result;

use crate::frame_ctx::ScriptFrameCtx;
use crate::script::ScriptTextSource;
use crate::script::recorder::MutationRecorder;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScriptDriverId(pub u64);

pub fn driver_id_from_source(source: &str) -> ScriptDriverId {
    use ahash::AHasher;
    use std::hash::{Hash, Hasher};
    let mut h = AHasher::default();
    source.hash(&mut h);
    ScriptDriverId(h.finish())
}

/// Static per-node facts the DOM host needs but the composition declares
/// outside the style store: what kind of element it is, its parent (so
/// `parentNode`/`children` work), and the shape data SVG plugins read through
/// `getAttribute('d')` / `getTotalLength()`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptNodeFacts {
    /// Lowercase HTML-ish tag name (`div`, `text`, `path`, `svg`, …).
    pub tag: String,
    /// Parent node id, empty for the root.
    pub parent: String,
    /// `<path d="…">` data, empty for non-path nodes.
    pub path_data: String,
}

#[derive(Clone, Debug, Default)]
pub struct ScriptTargetRegistry {
    pub visual_ids: HashSet<String>,
    pub canvas_ids: HashSet<String>,
    pub non_visual_ids: HashSet<String>,
    /// Per-node static facts, keyed by node id. Sorted iteration is not
    /// required — consumers look facts up by id.
    pub facts: std::collections::HashMap<String, ScriptNodeFacts>,
    /// Declared tree in document order — the base view the script-side
    /// [`TreeOverlay`](crate::script::tree::TreeOverlay) builds child lists
    /// from before any script mutation.
    pub declared_nodes: Vec<crate::script::tree::DeclaredNode>,
    /// Resolved text style per node, from the declarative tree (script
    /// mutations not applied). SplitText reads element boxes, and a box needs
    /// shaping — which needs the style. `ComputedTextStyle` has no `PartialEq`,
    /// so it lives here rather than inside `ScriptNodeFacts`.
    pub text_styles: std::collections::HashMap<String, crate::style::ComputedTextStyle>,
}

impl ScriptTargetRegistry {
    pub fn contains_visual(&self, id: &str) -> bool {
        self.visual_ids.contains(id)
    }

    pub fn contains_canvas(&self, id: &str) -> bool {
        self.canvas_ids.contains(id)
    }

    pub fn contains_non_visual(&self, id: &str) -> bool {
        self.non_visual_ids.contains(id)
    }

    pub fn facts(&self, id: &str) -> Option<&ScriptNodeFacts> {
        self.facts.get(id)
    }
}

pub trait ScriptHost {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId>;
    fn register_text_source(&mut self, node_id: &str, source: ScriptTextSource);
    fn clear_text_sources(&mut self);
    fn run_frame(
        &mut self,
        driver: ScriptDriverId,
        frame_ctx: &ScriptFrameCtx,
        current_node_id: Option<&str>,
        recorder: &mut dyn MutationRecorder,
    ) -> Result<()>;
    fn set_target_registry(&mut self, registry: ScriptTargetRegistry);
    /// Set base style values (from Tailwind/className) for all visible nodes.
    fn set_style_defaults(
        &mut self,
        _defaults: &std::collections::HashMap<
            String,
            std::collections::HashMap<String, serde_json::Value>,
        >,
    ) {
    }
    /// Set base style for a single node from its resolved NodeStyle.
    fn set_initial_style_from_node(&mut self, _id: &str, _style: &crate::style::NodeStyle) {}
    /// Declared text node → its dynamic leaf byte ranges with the wrapper
    /// chain above each leaf. The resolve bridge turns wrapper styles into
    /// per-unit overrides; a host without a script tree returns nothing.
    fn text_leaf_ranges(&mut self, _id: &str) -> Vec<(Vec<String>, usize, usize)> {
        Vec::new()
    }
    /// Drain script-recorded generated images (canvas `putImageData` /
    /// surface auto-bake path). Called once per rendered frame by the
    /// render pipeline; drained entries are inserted into the pipeline
    /// `GeneratedImageTable` and carried to hosts via `FrameMediaPlan`.
    fn take_frame_generated_images(&mut self) -> Vec<crate::ir::FrameGeneratedImage> {
        Vec::new()
    }
    /// Drain the frame's 3D scene IR section (`ctx.three` commits: declared
    /// [`Scene3`](crate::ir::scene3::Scene3) snapshots + placeholder generated
    /// ids already recorded as `DrawOp::Image` on the target canvases).
    /// Called once per rendered frame by the render pipeline; the section
    /// rides `RenderFrame.scene3` to the executing host (engine →
    /// opencat-three, web → real three.js).
    fn take_scene3_commits(&mut self) -> Option<crate::ir::draw_frame::Scene3Section> {
        None
    }
    /// Drain the frame's shader section (`<Shader>` layer stacks + placeholder
    /// generated ids already recorded as `DrawOp::Image`). Rides
    /// `RenderFrame.shader` to the executing host.
    fn take_shader_section(&mut self) -> Option<crate::ir::shader::ShaderSection> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_returns_stable_id() {
        use crate::test_support::MockScriptHost;
        let mut host: Box<dyn ScriptHost> = Box::new(MockScriptHost::default());
        let id1 = host.install("ctx => {}").unwrap();
        let id2 = host.install("ctx => {}").unwrap();
        assert_eq!(id1, id2);
    }
}
