use anyhow::{Result, anyhow};
use skia_safe::{Canvas, Picture};

use crate::{cache_policy::CacheInvalidationScope, profile::BackendProfile};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SceneSnapshotStrategy {
    DisplayList,
    LayoutTreeWithSubtreeCache,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SceneSnapshotPlan {
    pub strategy: SceneSnapshotStrategy,
    pub cache_scope: CacheInvalidationScope,
    pub contains_video: bool,
}

impl SceneSnapshotPlan {
    pub(crate) fn from_scene(cache_scope: CacheInvalidationScope, contains_video: bool) -> Self {
        let strategy = if contains_video || cache_scope.prefers_subtree_cache() {
            SceneSnapshotStrategy::LayoutTreeWithSubtreeCache
        } else {
            SceneSnapshotStrategy::DisplayList
        };
        Self {
            strategy,
            cache_scope,
            contains_video,
        }
    }

    pub(crate) fn allows_cache_reuse(self) -> bool {
        self.cache_scope.allows_picture_reuse()
    }
}

#[derive(Clone)]
pub(crate) struct SceneSnapshot {
    picture: Picture,
}

impl SceneSnapshot {
    pub(crate) fn new(picture: Picture) -> Self {
        Self { picture }
    }

    pub(crate) fn draw(
        &self,
        canvas: &Canvas,
        mut profile: Option<&mut BackendProfile>,
    ) -> Result<()> {
        let started = std::time::Instant::now();
        canvas.draw_picture(&self.picture, None, None);
        if let Some(profile) = profile.as_deref_mut() {
            profile.picture_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
        }
        Ok(())
    }

    pub(crate) fn picture(&self) -> Result<&Picture> {
        if self.picture.cull_rect().is_empty() {
            return Err(anyhow!("scene snapshot picture has empty bounds"));
        }
        Ok(&self.picture)
    }
}
