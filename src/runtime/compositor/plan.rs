use crate::runtime::profile::SceneBuildStats;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SceneRenderStrategy {
    DisplayTreeSnapshot,
    LayeredScene,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SceneRenderPlan {
    pub strategy: SceneRenderStrategy,
    pub allows_scene_snapshot_cache: bool,
}

impl SceneRenderPlan {
    pub(crate) fn from_scene(scene_stats: &SceneBuildStats) -> Self {
        let has_structural_change = scene_stats.layout_pass.structure_rebuild
            || scene_stats.layout_pass.layout_dirty_nodes > 0
            || scene_stats.layout_pass.raster_dirty_nodes > 0;

        let strategy = if scene_stats.contains_video {
            SceneRenderStrategy::LayeredScene
        } else {
            SceneRenderStrategy::DisplayTreeSnapshot
        };

        Self {
            strategy,
            allows_scene_snapshot_cache: !scene_stats.contains_video && !has_structural_change,
        }
    }

    pub(crate) fn renders_layered_scene(self) -> bool {
        self.strategy == SceneRenderStrategy::LayeredScene
    }
}

pub(crate) fn plan_for_scene(scene_stats: &SceneBuildStats) -> SceneRenderPlan {
    SceneRenderPlan::from_scene(scene_stats)
}

#[cfg(test)]
mod tests {
    use super::{SceneRenderPlan, SceneRenderStrategy};
    use crate::{layout::LayoutPassStats, runtime::profile::SceneBuildStats};

    #[test]
    fn video_scene_uses_layered_strategy() {
        let stats = SceneBuildStats {
            contains_video: true,
            ..SceneBuildStats::default()
        };

        let plan = SceneRenderPlan::from_scene(&stats);
        assert_eq!(plan.strategy, SceneRenderStrategy::LayeredScene);
        assert!(!plan.allows_scene_snapshot_cache);
    }

    #[test]
    fn composite_only_scene_uses_display_tree_snapshot() {
        let stats = SceneBuildStats {
            layout_pass: LayoutPassStats {
                composite_dirty_nodes: 2,
                ..LayoutPassStats::default()
            },
            ..SceneBuildStats::default()
        };

        let plan = SceneRenderPlan::from_scene(&stats);
        assert_eq!(plan.strategy, SceneRenderStrategy::DisplayTreeSnapshot);
        assert!(!plan.allows_scene_snapshot_cache);
    }

    #[test]
    fn clean_scene_reuses_display_list_snapshot_cache() {
        let stats = SceneBuildStats::default();

        let plan = SceneRenderPlan::from_scene(&stats);
        assert_eq!(plan.strategy, SceneRenderStrategy::DisplayTreeSnapshot);
        assert!(plan.allows_scene_snapshot_cache);
    }
}
