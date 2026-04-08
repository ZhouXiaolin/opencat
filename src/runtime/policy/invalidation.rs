use crate::{layout::LayoutPassStats, runtime::profile::SceneBuildStats};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SceneInvalidation {
    Clean,
    Composite,
    Raster,
    Layout,
    Structure,
    TimeVariant,
}

impl SceneInvalidation {
    pub(crate) fn allows_picture_reuse(self) -> bool {
        matches!(self, Self::Clean)
    }

    pub(crate) fn prefers_subtree_cache(self) -> bool {
        matches!(self, Self::Composite)
    }
}

pub(crate) fn scene_invalidation(
    layout_pass: &LayoutPassStats,
    contains_video: bool,
) -> SceneInvalidation {
    if contains_video {
        SceneInvalidation::TimeVariant
    } else if layout_pass.structure_rebuild {
        SceneInvalidation::Structure
    } else if layout_pass.layout_dirty_nodes > 0 {
        SceneInvalidation::Layout
    } else if layout_pass.raster_dirty_nodes > 0 {
        SceneInvalidation::Raster
    } else if layout_pass.composite_dirty_nodes > 0 {
        SceneInvalidation::Composite
    } else {
        SceneInvalidation::Clean
    }
}

pub(crate) fn invalidation_for_scene(scene_stats: &SceneBuildStats) -> SceneInvalidation {
    scene_invalidation(&scene_stats.layout_pass, scene_stats.contains_video)
}

#[cfg(test)]
mod tests {
    use super::{SceneInvalidation, invalidation_for_scene};
    use crate::{layout::LayoutPassStats, runtime::profile::SceneBuildStats};

    #[test]
    fn scene_invalidation_prioritizes_time_variant() {
        let scene_stats = SceneBuildStats {
            contains_video: true,
            ..SceneBuildStats::default()
        };

        assert_eq!(
            invalidation_for_scene(&scene_stats),
            SceneInvalidation::TimeVariant
        );
    }

    #[test]
    fn scene_invalidation_uses_layout_pass_order() {
        let scene_stats = SceneBuildStats {
            layout_pass: LayoutPassStats {
                composite_dirty_nodes: 3,
                ..LayoutPassStats::default()
            },
            ..SceneBuildStats::default()
        };

        assert_eq!(
            invalidation_for_scene(&scene_stats),
            SceneInvalidation::Composite
        );
    }
}
