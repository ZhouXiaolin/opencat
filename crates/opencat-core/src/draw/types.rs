/// Index into the frame paint table (DrawOpFrame.paints).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PaintId(pub u32);

/// Index into the frame path table (DrawOpFrame.paths).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathId(pub u32);

/// Index into the frame string table (DrawOpFrame.strings).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StringId(pub u32);

/// Index into DrawOpFrame.byte_ranges for variable-length byte data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BytesRangeId(pub u32);

/// Index into FrameMediaPlan.runtime_effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EffectId(pub u32);

/// A range into the child table (DrawOpFrame.children).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildRange {
    pub start: u32,
    pub len: u32,
}

/// Reference to a cached DrawOp range within the current frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DrawOpRange {
    pub start_op: u32,
    pub op_len: u32,
}

/// Generic range into a side table: (start, len).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableRange {
    pub start: u32,
    pub len: u32,
}

/// Reference to an image source — either a static asset or a video frame.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageRef {
    Static { asset_id: String },
    VideoFrame { asset_id: String, frame_index: u32 },
}

/// Persistent runtime effect metadata.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EffectRef {
    pub hash: u64,
    pub sksl: String,
}

/// Frame-local resource reference used by side tables.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResourceRef {
    StaticImage(String),
    VideoFrame(String, u32),
    RuntimeEffect(EffectId),
}

/// Path construction operation.
#[derive(Clone, Debug, PartialEq)]
pub enum PathOp {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CubicTo(f32, f32, f32, f32, f32, f32),
    Close,
}

/// Path fill type for EncodedPath.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillType {
    Winding,
    EvenOdd,
}

/// An encoded path stored in the frame path table.
#[derive(Clone, Debug, PartialEq)]
pub struct EncodedPath {
    pub fill_type: FillType,
    pub ops: Vec<PathOp>,
}

/// Child reference for RuntimeEffect inputs.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeEffectChildRef {
    Image(ImageRef),
    Picture(DrawOpRange),
    Shader(ShaderSpec),
}

/// Shader specification (used by RuntimeEffect children and PaintSpec).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderSpec {
    pub shader_type: ShaderType,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ShaderType {
    LinearGradient {
        start: (f32, f32),
        end: (f32, f32),
        colors: Vec<(f32, [f32; 4])>,
    },
    RadialGradient {
        center: (f32, f32),
        radius: f32,
        colors: Vec<(f32, [f32; 4])>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_id_is_index_into_table() {
        let id = PaintId(5);
        assert_eq!(id.0, 5);
    }

    #[test]
    fn path_id_is_index_into_table() {
        let id = PathId(3);
        assert_eq!(id.0, 3);
    }

    #[test]
    fn image_ref_static_holds_asset_id() {
        let ref_ = ImageRef::Static { asset_id: "test.png".into() };
        match ref_ {
            ImageRef::Static { asset_id } => assert_eq!(asset_id, "test.png"),
            _ => panic!("expected Static"),
        }
    }

    #[test]
    fn table_range_defaults() {
        let range = TableRange { start: 0, len: 10 };
        assert_eq!(range.start, 0);
        assert_eq!(range.len, 10);
    }

    #[test]
    fn effect_ref_holds_hash_and_sksl() {
        let ref_ = EffectRef {
            hash: 0xDEADBEEF,
            sksl: "half4 main(float2 uv) { return half4(1.0); }".into(),
        };
        assert_eq!(ref_.hash, 0xDEADBEEF);
        assert!(ref_.sksl.contains("half4"));
    }
}
