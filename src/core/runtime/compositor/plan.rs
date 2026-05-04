#[cfg(feature = "host-default")]
use crate::host::runtime::profile::SceneBuildStats;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneRenderPlan {
    pub allows_scene_snapshot_cache: bool,
}

impl SceneRenderPlan {
    #[cfg(feature = "host-default")]
    pub fn from_scene(scene_stats: &SceneBuildStats) -> Self {
        let has_structural_change = scene_stats.layout_pass.structure_rebuild
            || scene_stats.layout_pass.layout_dirty_nodes > 0
            || scene_stats.layout_pass.raster_dirty_nodes > 0
            || scene_stats.layout_pass.composite_dirty_nodes > 0;

        Self {
            allows_scene_snapshot_cache: !scene_stats.contains_time_variant_paint
                && !has_structural_change,
        }
    }
}

#[cfg(feature = "host-default")]
pub fn plan_for_scene(scene_stats: &SceneBuildStats) -> SceneRenderPlan {
    SceneRenderPlan::from_scene(scene_stats)
}

#[cfg(feature = "host-default")]
#[cfg(test)]
mod tests {
    use super::SceneRenderPlan;
    use crate::runtime::profile::SceneBuildStats;

    #[test]
    fn time_variant_paint_scene_disables_scene_snapshot_cache() {
        let stats = SceneBuildStats {
            contains_time_variant_paint: true,
            ..SceneBuildStats::default()
        };

        let plan = SceneRenderPlan::from_scene(&stats);
        assert_eq!(
            plan,
            SceneRenderPlan {
                allows_scene_snapshot_cache: false,
            }
        );
    }

    #[test]
    fn composite_only_scene_disables_scene_snapshot_cache() {
        let stats = SceneBuildStats {
            layout_pass: crate::core::layout::LayoutPassStats {
                composite_dirty_nodes: 2,
                ..crate::core::layout::LayoutPassStats::default()
            },
            ..SceneBuildStats::default()
        };

        let plan = SceneRenderPlan::from_scene(&stats);
        assert_eq!(
            plan,
            SceneRenderPlan {
                allows_scene_snapshot_cache: false,
            }
        );
    }

    #[test]
    fn clean_scene_reuses_scene_snapshot_cache() {
        let stats = SceneBuildStats::default();

        let plan = SceneRenderPlan::from_scene(&stats);
        assert_eq!(
            plan,
            SceneRenderPlan {
                allows_scene_snapshot_cache: true,
            }
        );
    }
}
