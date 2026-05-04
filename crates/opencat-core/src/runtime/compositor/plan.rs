use crate::layout::LayoutPassStats;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneRenderPlan {
    pub allows_scene_snapshot_cache: bool,
}

impl SceneRenderPlan {
    pub fn from_layout_pass(
        layout_pass: &LayoutPassStats,
        contains_time_variant_paint: bool,
    ) -> Self {
        let has_structural_change = layout_pass.structure_rebuild
            || layout_pass.layout_dirty_nodes > 0
            || layout_pass.raster_dirty_nodes > 0
            || layout_pass.composite_dirty_nodes > 0;

        Self {
            allows_scene_snapshot_cache: !contains_time_variant_paint
                && !has_structural_change,
        }
    }
}

pub fn plan_for_scene(
    layout_pass: &LayoutPassStats,
    contains_time_variant_paint: bool,
) -> SceneRenderPlan {
    SceneRenderPlan::from_layout_pass(layout_pass, contains_time_variant_paint)
}

#[cfg(test)]
mod tests {
    use super::SceneRenderPlan;
    use crate::layout::LayoutPassStats;

    #[test]
    fn time_variant_paint_scene_disables_scene_snapshot_cache() {
        let plan = SceneRenderPlan::from_layout_pass(&LayoutPassStats::default(), true);
        assert_eq!(
            plan,
            SceneRenderPlan {
                allows_scene_snapshot_cache: false,
            }
        );
    }

    #[test]
    fn composite_only_scene_disables_scene_snapshot_cache() {
        let layout_pass = LayoutPassStats {
            composite_dirty_nodes: 2,
            ..LayoutPassStats::default()
        };

        let plan = SceneRenderPlan::from_layout_pass(&layout_pass, false);
        assert_eq!(
            plan,
            SceneRenderPlan {
                allows_scene_snapshot_cache: false,
            }
        );
    }

    #[test]
    fn clean_scene_reuses_scene_snapshot_cache() {
        let plan = SceneRenderPlan::from_layout_pass(&LayoutPassStats::default(), false);
        assert_eq!(
            plan,
            SceneRenderPlan {
                allows_scene_snapshot_cache: true,
            }
        );
    }
}
