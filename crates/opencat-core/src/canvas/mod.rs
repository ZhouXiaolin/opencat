pub use kurbo::{Rect, RoundedRect as RRect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipOp {
    Intersect,
    Difference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillType {
    Winding,
    EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointMode {
    Points,
    Lines,
    Polygon,
}

pub mod glyph;
pub mod paint;

pub use glyph::*;
pub use paint::*;
