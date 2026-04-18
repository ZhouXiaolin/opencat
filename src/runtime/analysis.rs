use crate::runtime::{
    annotation::AnnotatedNodeHandle,
    fingerprint::{PaintVariance, SubtreeSnapshotFingerprint},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayNodeAnalysis {
    pub paint_variance: PaintVariance,
    pub subtree_contains_time_variant: bool,
    pub paint_fingerprint: Option<u64>,
    pub snapshot_fingerprint: Option<SubtreeSnapshotFingerprint>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisplayNodeInvalidation {
    pub composite_dirty: bool,
}

#[derive(Clone, Debug, Default)]
pub struct DisplayAnalysisTable {
    nodes: Vec<DisplayNodeAnalysis>,
}

impl DisplayAnalysisTable {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, handle: AnnotatedNodeHandle, analysis: DisplayNodeAnalysis) {
        if handle.0 == self.nodes.len() {
            self.nodes.push(analysis);
        } else {
            self.nodes[handle.0] = analysis;
        }
    }

    pub fn get(&self, handle: AnnotatedNodeHandle) -> Option<DisplayNodeAnalysis> {
        self.nodes.get(handle.0).copied()
    }

    pub fn require(&self, handle: AnnotatedNodeHandle) -> DisplayNodeAnalysis {
        self.get(handle)
            .unwrap_or_else(|| panic!("missing display analysis for node {:?}", handle))
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }
}

#[derive(Clone, Debug, Default)]
pub struct DisplayInvalidationTable {
    nodes: Vec<DisplayNodeInvalidation>,
}

impl DisplayInvalidationTable {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
        }
    }

    pub fn with_len(len: usize) -> Self {
        Self {
            nodes: vec![DisplayNodeInvalidation::default(); len],
        }
    }

    pub fn insert(&mut self, handle: AnnotatedNodeHandle, invalidation: DisplayNodeInvalidation) {
        if handle.0 == self.nodes.len() {
            self.nodes.push(invalidation);
        } else {
            self.nodes[handle.0] = invalidation;
        }
    }

}
