#[derive(Clone, Copy, Debug)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug)]
pub struct LayoutNode {
    pub id: String,
    pub rect: LayoutRect,
    pub output_fingerprint: LayoutOutputFingerprint,
    pub children: Vec<LayoutNode>,
}

#[derive(Clone, Debug)]
pub struct LayoutTree {
    pub root: LayoutNode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LayoutOutputFingerprint {
    pub local: u64,
    pub subtree: u64,
    pub record_size: u64,
}
