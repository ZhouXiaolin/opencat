#[allow(unused_imports)]
use crate::canvas::paint::BlendMode;
use super::types::{
    PaintId, PathId, ImageRef, EffectId, BytesRangeId, ChildRange, DrawOpRange, PathOp,
};

// ---------------------------------------------------------------------------
// Line rendering enums
// ---------------------------------------------------------------------------

/// Line cap style for stroke operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// Line join style for stroke operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// Point rendering mode — controls how point vertices are interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointMode {
    Points,
    Lines,
    Polygon,
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// A 4-component rectangle: (x, y, width, height).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect4 {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A 4-corner radius specification (top-left, top-right, bottom-right, bottom-left).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii4 {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

/// Specification for a rounded rectangle with separate outer/inner rects and radii.
/// Used by `DrawOp::DRRect` for compound rounded rectangle drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DRRectSpec {
    pub rect: Rect4,
    pub radii: Radii4,
}

// ---------------------------------------------------------------------------
// Transform helpers
// ---------------------------------------------------------------------------

/// A 3x3 affine transform matrix stored as [f32; 9] in row-major order.
pub type Matrix3 = [f32; 9];

// ---------------------------------------------------------------------------
// Color types
// ---------------------------------------------------------------------------

/// An 8-bit-per-channel RGBA color (0-255).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorU8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// A float RGBA color (0.0-1.0 per channel).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorF32 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ColorF32 {
    pub const TRANSPARENT: Self = ColorF32 {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
}

// ---------------------------------------------------------------------------
// Pool range helpers
// ---------------------------------------------------------------------------

/// Range into the f32 pool. `start` and `len` index `DrawOpFrame.f32_pool`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct F32Range {
    pub start: u32,
    pub len: u32,
}

// ---------------------------------------------------------------------------
// DrawOp — canonical draw instruction
// ---------------------------------------------------------------------------

/// The canonical draw instruction consumed by platform executors.
///
/// `DrawOp` is the core IR type for the rendering layer. It encodes every
/// graphical command that a platform executor needs to replay, including
/// stack management, affine transforms, paint state, path construction,
/// and immediate-mode drawing primitives.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawOp {
    // =======================================================================
    // Stack management
    // =======================================================================

    /// Push a save point onto the transform/paint stack.
    Save,

    /// Push a save point and create a transparent layer with optional bounds,
    /// paint filter, and alpha.
    SaveLayer {
        bounds: Option<Rect4>,
        paint: Option<PaintId>,
        alpha: f32,
    },

    /// Pop the most recent save point, restoring transforms and paint state.
    Restore,

    /// Pop save points until the stack reaches `count` entries.
    RestoreToCount {
        count: i32,
    },

    // =======================================================================
    // Transforms
    // =======================================================================

    /// Apply a 2D translation to the current transform.
    Translate {
        x: f32,
        y: f32,
    },

    /// Apply a 2D uniform scale to the current transform.
    Scale {
        x: f32,
        y: f32,
    },

    /// Apply a 2D rotation (in degrees) around (cx, cy).
    Rotate {
        degrees: f32,
        cx: f32,
        cy: f32,
    },

    /// Apply a 2D skew along the x and y axes.
    Skew {
        sx: f32,
        sy: f32,
    },

    /// Concatenate a 3x3 affine matrix with the current transform.
    Concat {
        matrix: Matrix3,
    },

    // =======================================================================
    // Paint state setters (script canvas immediate-mode)
    // =======================================================================

    /// Set the fill color.
    SetFillStyle {
        color: ColorU8,
    },

    /// Set the stroke color.
    SetStrokeStyle {
        color: ColorU8,
    },

    /// Set the stroke width.
    SetLineWidth {
        width: f32,
    },

    /// Set the line cap style.
    SetLineCap {
        cap: LineCap,
    },

    /// Set the line join style.
    SetLineJoin {
        join: LineJoin,
    },

    /// Set the dash pattern from a range of floats and a phase offset.
    SetLineDash {
        intervals: F32Range,
        phase: f32,
    },

    /// Clear any active dash pattern (restore solid lines).
    ClearLineDash,

    /// Set the global alpha multiplier (0.0-1.0).
    SetGlobalAlpha {
        alpha: f32,
    },

    /// Toggle anti-aliasing.
    SetAntiAlias {
        enabled: bool,
    },

    // =======================================================================
    // Path construction
    // =======================================================================

    /// Begin a new path, discarding any previously recorded path.
    BeginPath,

    /// Append a path operation to the current path.
    Path(PathOp),

    /// Fill the current path with the current fill paint.
    FillPath,

    /// Stroke the current path with the current stroke paint.
    StrokePath,

    /// Clip to the current path, with optional anti-aliasing.
    ClipPath {
        anti_alias: bool,
    },

    // =======================================================================
    // Drawing — immediate-mode primitives
    // =======================================================================

    /// Clear the canvas to a solid float color.
    Clear {
        color: ColorF32,
    },

    /// Draw a pre-built paint (covers the current clip).
    Paint {
        paint: PaintId,
    },

    /// Draw a filled rectangle.
    Rect {
        rect: Rect4,
        paint: PaintId,
    },

    /// Draw a filled rounded rectangle.
    RRect {
        rect: Rect4,
        radii: Radii4,
        paint: PaintId,
    },

    /// Draw a filled double rounded rectangle (outer minus inner).
    DRRect {
        outer: DRRectSpec,
        inner: DRRectSpec,
        paint: PaintId,
    },

    /// Draw a filled oval inscribed in `rect`.
    Oval {
        rect: Rect4,
        paint: PaintId,
    },

    /// Draw a filled circle centered at (cx, cy).
    Circle {
        cx: f32,
        cy: f32,
        radius: f32,
        paint: PaintId,
    },

    /// Draw a filled arc.
    Arc {
        rect: Rect4,
        start: f32,
        sweep: f32,
        use_center: bool,
        paint: PaintId,
    },

    /// Draw a single line segment.
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        paint: PaintId,
    },

    /// Draw a set of points (or lines/polygon) from a float pool range.
    Points {
        mode: PointMode,
        points: F32Range,
        paint: PaintId,
    },

    /// Draw a pre-encoded path from the frame path table.
    DrawPath {
        path: PathId,
        paint: PaintId,
    },

    /// Draw an image at (x, y) with optional paint filter.
    Image {
        image: ImageRef,
        x: f32,
        y: f32,
        paint: Option<PaintId>,
    },

    /// Draw an image with source and destination rectangles.
    ImageRect {
        image: ImageRef,
        src: Option<Rect4>,
        dst: Rect4,
        paint: Option<PaintId>,
    },

    /// Draw a runtime shader effect with uniform data and child inputs.
    RuntimeEffect {
        effect: EffectId,
        uniforms: BytesRangeId,
        children: ChildRange,
        dst: Rect4,
    },

    /// Replay a previously-encoded range of DrawOps.
    ReplayRange {
        range: DrawOpRange,
    },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::types::{PaintId, ImageRef};

    #[test]
    fn draw_op_roundtrip_save_restore_variants_exist() {
        let save = DrawOp::Save;
        let restore = DrawOp::Restore;
        assert_ne!(
            std::mem::discriminant(&save),
            std::mem::discriminant(&restore)
        );
    }

    #[test]
    fn draw_op_rect_holds_coordinates() {
        let rect = Rect4 {
            x: 1.0,
            y: 2.0,
            width: 10.0,
            height: 20.0,
        };
        let paint = PaintId(0);
        let op = DrawOp::Rect { rect, paint };
        match op {
            DrawOp::Rect { rect: r, paint: p } => {
                assert_eq!(r.x, 1.0);
                assert_eq!(r.y, 2.0);
                assert_eq!(p.0, 0);
            }
            _ => panic!("expected Rect"),
        }
    }

    #[test]
    fn draw_op_image_holds_ref() {
        let op = DrawOp::Image {
            image: ImageRef::Static {
                asset_id: "test.png".into(),
            },
            x: 10.0,
            y: 20.0,
            paint: Some(PaintId(1)),
        };
        assert!(matches!(op, DrawOp::Image { .. }));
    }

    #[test]
    fn draw_op_path_op_variant_exists() {
        let op = DrawOp::Path(PathOp::MoveTo { x: 0.0, y: 0.0 });
        assert!(matches!(op, DrawOp::Path(..)));
    }

    #[test]
    fn line_cap_join_point_mode_variants_exist() {
        assert_ne!(LineCap::Butt, LineCap::Round);
        assert_ne!(LineJoin::Miter, LineJoin::Round);
        assert_ne!(PointMode::Points, PointMode::Lines);
    }
}
