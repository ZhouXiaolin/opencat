use anyhow::Result;

use crate::frame_ctx::ScriptFrameCtx;
use crate::scene::script::{ScriptTextSource, StyleMutations};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScriptDriverId(pub u64);

pub trait ScriptHost {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId>;
    fn register_text_source(&mut self, node_id: &str, source: ScriptTextSource);
    fn clear_text_sources(&mut self);
    fn run_frame(
        &mut self,
        driver: ScriptDriverId,
        frame_ctx: &ScriptFrameCtx,
        current_node_id: Option<&str>,
    ) -> Result<StyleMutations>;
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
