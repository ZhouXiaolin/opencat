use std::collections::HashMap;

use crate::runtime::{annotation::RenderNodeKey, fingerprint::PaintVariance};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayNodeAnalysis {
    pub paint_variance: PaintVariance,
    pub subtree_contains_time_variant: bool,
    pub paint_fingerprint: Option<u64>,
    pub snapshot_fingerprint: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisplayNodeInvalidation {
    pub composite_dirty: bool,
    pub subtree_contains_dynamic: bool,
}

#[derive(Clone, Debug, Default)]
pub struct DisplayAnalysisTable {
    nodes: HashMap<RenderNodeKey, DisplayNodeAnalysis>,
}

impl DisplayAnalysisTable {
    pub fn insert(&mut self, key: RenderNodeKey, analysis: DisplayNodeAnalysis) {
        self.nodes.insert(key, analysis);
    }

    pub fn get(&self, key: RenderNodeKey) -> Option<DisplayNodeAnalysis> {
        self.nodes.get(&key).copied()
    }

    pub fn require(&self, key: RenderNodeKey) -> DisplayNodeAnalysis {
        self.get(key)
            .unwrap_or_else(|| panic!("missing display analysis for node {:?}", key))
    }
}

#[derive(Clone, Debug, Default)]
pub struct DisplayInvalidationTable {
    nodes: HashMap<RenderNodeKey, DisplayNodeInvalidation>,
}

impl DisplayInvalidationTable {
    pub fn insert(&mut self, key: RenderNodeKey, invalidation: DisplayNodeInvalidation) {
        self.nodes.insert(key, invalidation);
    }

    pub fn get(&self, key: RenderNodeKey) -> DisplayNodeInvalidation {
        self.nodes.get(&key).copied().unwrap_or_default()
    }
}
