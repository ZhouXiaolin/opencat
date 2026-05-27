use super::draw_types::{
    BytesRangeId, ChildRange, DrawOpRange, EffectId, ImageRef, PaintId, PathId, PathOp,
    ScriptRuntimeEffectChild, SubtreeId,
};
#[allow(unused_imports)]
use crate::canvas::paint::BlendMode;

// ---------------------------------------------------------------------------
// Line rendering enums
// ---------------------------------------------------------------------------

/// Line cap style for stroke operations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// Line join style for stroke operations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// Point rendering mode — controls how point vertices are interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

impl std::hash::Hash for Rect4 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.x.to_bits().hash(state);
        self.y.to_bits().hash(state);
        self.width.to_bits().hash(state);
        self.height.to_bits().hash(state);
    }
}

/// A 4-corner radius specification (top-left, top-right, bottom-right, bottom-left).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii4 {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl std::hash::Hash for Radii4 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.top_left.to_bits().hash(state);
        self.top_right.to_bits().hash(state);
        self.bottom_right.to_bits().hash(state);
        self.bottom_left.to_bits().hash(state);
    }
}

/// Specification for a rounded rectangle with separate outer/inner rects and radii.
/// Used by `DrawOp::DRRect` for compound rounded rectangle drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DRRectSpec {
    pub rect: Rect4,
    pub radii: Radii4,
}

impl std::hash::Hash for DRRectSpec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.rect.hash(state);
        self.radii.hash(state);
    }
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

impl std::hash::Hash for ColorF32 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.r.to_bits().hash(state);
        self.g.to_bits().hash(state);
        self.b.to_bits().hash(state);
        self.a.to_bits().hash(state);
    }
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

    DrawSubtreePicture {
        owner_id: String,
        x: f32,
        y: f32,
    },

    ReplaySubtreePicture {
        subtree: SubtreeId,
        x: f32,
        y: f32,
    },

    /// Script-originated runtime effect (pre-intern intermediate form).
    /// Translated to `DrawOp::RuntimeEffect` during `execute_draw_op`.
    /// Engine replay treats this as a no-op; the binary encoder must never see it.
    ScriptRuntimeEffect {
        sksl: String,
        uniforms_bytes: Vec<u8>,
        children: Vec<ScriptRuntimeEffectChild>,
        dst: Rect4,
    },
}

// ---------------------------------------------------------------------------
// Manual Hash impl for DrawOp (f32 fields can't derive Hash)
// ---------------------------------------------------------------------------

impl std::hash::Hash for DrawOp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            DrawOp::Save => 0_u8.hash(state),
            DrawOp::SaveLayer {
                bounds,
                paint,
                alpha,
            } => {
                1_u8.hash(state);
                bounds.hash(state);
                paint.hash(state);
                alpha.to_bits().hash(state);
            }
            DrawOp::Restore => 2_u8.hash(state),
            DrawOp::RestoreToCount { count } => {
                3_u8.hash(state);
                count.hash(state);
            }
            DrawOp::Translate { x, y } => {
                4_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            DrawOp::Scale { x, y } => {
                5_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            DrawOp::Rotate { degrees, cx, cy } => {
                6_u8.hash(state);
                degrees.to_bits().hash(state);
                cx.to_bits().hash(state);
                cy.to_bits().hash(state);
            }
            DrawOp::Skew { sx, sy } => {
                7_u8.hash(state);
                sx.to_bits().hash(state);
                sy.to_bits().hash(state);
            }
            DrawOp::Concat { matrix } => {
                8_u8.hash(state);
                matrix.map(f32::to_bits).hash(state);
            }
            DrawOp::SetFillStyle { color } => {
                9_u8.hash(state);
                color.hash(state);
            }
            DrawOp::SetStrokeStyle { color } => {
                10_u8.hash(state);
                color.hash(state);
            }
            DrawOp::SetLineWidth { width } => {
                11_u8.hash(state);
                width.to_bits().hash(state);
            }
            DrawOp::SetLineCap { cap } => {
                12_u8.hash(state);
                cap.hash(state);
            }
            DrawOp::SetLineJoin { join } => {
                13_u8.hash(state);
                join.hash(state);
            }
            DrawOp::SetLineDash { intervals, phase } => {
                14_u8.hash(state);
                intervals.hash(state);
                phase.to_bits().hash(state);
            }
            DrawOp::ClearLineDash => 15_u8.hash(state),
            DrawOp::SetGlobalAlpha { alpha } => {
                16_u8.hash(state);
                alpha.to_bits().hash(state);
            }
            DrawOp::SetAntiAlias { enabled } => {
                17_u8.hash(state);
                enabled.hash(state);
            }
            DrawOp::BeginPath => 18_u8.hash(state),
            DrawOp::Path(p) => {
                19_u8.hash(state);
                p.hash(state);
            }
            DrawOp::FillPath => 20_u8.hash(state),
            DrawOp::StrokePath => 21_u8.hash(state),
            DrawOp::ClipPath { anti_alias } => {
                22_u8.hash(state);
                anti_alias.hash(state);
            }
            DrawOp::Clear { color } => {
                23_u8.hash(state);
                color.hash(state);
            }
            DrawOp::Paint { paint } => {
                24_u8.hash(state);
                paint.hash(state);
            }
            DrawOp::Rect { rect, paint } => {
                25_u8.hash(state);
                rect.hash(state);
                paint.hash(state);
            }
            DrawOp::RRect { rect, radii, paint } => {
                26_u8.hash(state);
                rect.hash(state);
                radii.hash(state);
                paint.hash(state);
            }
            DrawOp::DRRect {
                outer,
                inner,
                paint,
            } => {
                27_u8.hash(state);
                outer.hash(state);
                inner.hash(state);
                paint.hash(state);
            }
            DrawOp::Oval { rect, paint } => {
                28_u8.hash(state);
                rect.hash(state);
                paint.hash(state);
            }
            DrawOp::Circle {
                cx,
                cy,
                radius,
                paint,
            } => {
                29_u8.hash(state);
                cx.to_bits().hash(state);
                cy.to_bits().hash(state);
                radius.to_bits().hash(state);
                paint.hash(state);
            }
            DrawOp::Arc {
                rect,
                start,
                sweep,
                use_center,
                paint,
            } => {
                30_u8.hash(state);
                rect.hash(state);
                start.to_bits().hash(state);
                sweep.to_bits().hash(state);
                use_center.hash(state);
                paint.hash(state);
            }
            DrawOp::Line {
                x0,
                y0,
                x1,
                y1,
                paint,
            } => {
                31_u8.hash(state);
                x0.to_bits().hash(state);
                y0.to_bits().hash(state);
                x1.to_bits().hash(state);
                y1.to_bits().hash(state);
                paint.hash(state);
            }
            DrawOp::Points {
                mode,
                points,
                paint,
            } => {
                32_u8.hash(state);
                mode.hash(state);
                points.hash(state);
                paint.hash(state);
            }
            DrawOp::DrawPath { path, paint } => {
                33_u8.hash(state);
                path.hash(state);
                paint.hash(state);
            }
            DrawOp::Image { image, x, y, paint } => {
                34_u8.hash(state);
                image.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                paint.hash(state);
            }
            DrawOp::ImageRect {
                image,
                src,
                dst,
                paint,
            } => {
                35_u8.hash(state);
                image.hash(state);
                src.hash(state);
                dst.hash(state);
                paint.hash(state);
            }
            DrawOp::RuntimeEffect {
                effect,
                uniforms,
                children,
                dst,
            } => {
                36_u8.hash(state);
                effect.hash(state);
                uniforms.hash(state);
                children.hash(state);
                dst.hash(state);
            }
            DrawOp::ReplayRange { range } => {
                37_u8.hash(state);
                range.hash(state);
            }
            DrawOp::DrawSubtreePicture { owner_id, x, y } => {
                38_u8.hash(state);
                owner_id.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            DrawOp::ReplaySubtreePicture { subtree, x, y } => {
                39_u8.hash(state);
                subtree.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            DrawOp::ScriptRuntimeEffect {
                sksl,
                uniforms_bytes,
                children,
                dst,
            } => {
                40_u8.hash(state);
                sksl.hash(state);
                uniforms_bytes.hash(state);
                children.hash(state);
                dst.hash(state);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::draw_types::{ImageRef, PaintId};

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

    #[test]
    fn script_runtime_effect_holds_inline_payload() {
        use crate::ir::draw_types::{ImageRef, ScriptRuntimeEffectChild};
        let op = DrawOp::ScriptRuntimeEffect {
            sksl: "half4 main(float2 p){return half4(1);}".to_string(),
            uniforms_bytes: vec![0u8, 1, 2, 3],
            children: vec![ScriptRuntimeEffectChild::Image(ImageRef::Static {
                asset_id: "img".into(),
            })],
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        };
        match &op {
            DrawOp::ScriptRuntimeEffect {
                sksl,
                uniforms_bytes,
                children,
                dst,
            } => {
                assert!(sksl.contains("half4"));
                assert_eq!(uniforms_bytes.len(), 4);
                assert_eq!(children.len(), 1);
                assert_eq!(dst.width, 10.0);
            }
            _ => panic!("expected ScriptRuntimeEffect"),
        }
    }

    #[test]
    fn script_runtime_effect_hash_differs_by_sksl() {
        use ahash::AHasher;
        use std::hash::{Hash, Hasher};
        let make = |s: &str| DrawOp::ScriptRuntimeEffect {
            sksl: s.to_string(),
            uniforms_bytes: Vec::new(),
            children: Vec::new(),
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
        };
        let mut h1 = AHasher::default();
        make("a").hash(&mut h1);
        let mut h2 = AHasher::default();
        make("b").hash(&mut h2);
        assert_ne!(h1.finish(), h2.finish());
    }
}
