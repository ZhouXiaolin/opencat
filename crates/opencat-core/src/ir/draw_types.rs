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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// Index into DrawOpFrame.subtrees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubtreeId(pub u32);

/// Generic range into a side table: (start, len).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TableRange {
    pub start: u32,
    pub len: u32,
}

/// Reference to an image source.
///
/// - [`ImageRef::Static`] — an external (host-fetched) image asset, by canonical id.
/// - [`ImageRef::VideoFrame`] — a host-decoded video frame, by canonical id + the
///   authoritative `time_micros`.
/// - [`ImageRef::Generated`] — a core-rasterized image (e.g. color-emoji bitmap
///   glyph) owned by the pipeline's [`GeneratedImageTable`](super::GeneratedImageTable).
///
/// A video frame reference carries only the canonical `AssetId` and the
/// authoritative target `time_micros`. It intentionally does NOT carry a
/// source `frame_index`: composition frame index is not source video frame
/// index, and host decoders resolve frames from `time_micros` using their
/// own seek/keyframe/cache strategy.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageRef {
    Static {
        asset_id: String,
    },
    VideoFrame {
        asset_id: String,
        time_micros: u64,
    },
    /// A core-generated image (color emoji). The RGBA lives in the pipeline's
    /// generated-image table, not in any external asset store; `id` is the
    /// deterministic glyph cache key. Hosts never re-parse fonts for this.
    Generated {
        id: super::generated_image::GeneratedImageId,
    },
}

/// Persistent runtime effect metadata.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EffectRef {
    pub hash: u64,
    pub sksl: String,
}

/// Frame-local resource reference used by side tables.
///
/// Video frames are not represented here: they are identified by
/// `ImageRef::VideoFrame { asset_id, time_micros }` in the draw op stream and
/// collected into `FrameMediaPlan.video_frames`. A source `frame_index` is
/// intentionally not part of any resource identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResourceRef {
    StaticImage(String),
    RuntimeEffect(EffectId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PathOp {
    MoveTo {
        x: f32,
        y: f32,
    },
    LineTo {
        x: f32,
        y: f32,
    },
    QuadTo {
        cx: f32,
        cy: f32,
        x: f32,
        y: f32,
    },
    CubicTo {
        c1x: f32,
        c1y: f32,
        c2x: f32,
        c2y: f32,
        x: f32,
        y: f32,
    },
    Close,
    AddRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    AddRRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    },
    AddOval {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    AddArc {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        start_angle: f32,
        sweep_angle: f32,
    },
}

impl std::hash::Hash for PathOp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            PathOp::MoveTo { x, y } => {
                0_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            PathOp::LineTo { x, y } => {
                1_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            PathOp::QuadTo { cx, cy, x, y } => {
                2_u8.hash(state);
                cx.to_bits().hash(state);
                cy.to_bits().hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            PathOp::CubicTo {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            } => {
                3_u8.hash(state);
                c1x.to_bits().hash(state);
                c1y.to_bits().hash(state);
                c2x.to_bits().hash(state);
                c2y.to_bits().hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            PathOp::Close => {
                4_u8.hash(state);
            }
            PathOp::AddRect {
                x,
                y,
                width,
                height,
            } => {
                5_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
            }
            PathOp::AddRRect {
                x,
                y,
                width,
                height,
                radius,
            } => {
                6_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
                radius.to_bits().hash(state);
            }
            PathOp::AddOval {
                x,
                y,
                width,
                height,
            } => {
                7_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
            }
            PathOp::AddArc {
                x,
                y,
                width,
                height,
                start_angle,
                sweep_angle,
            } => {
                8_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
                start_angle.to_bits().hash(state);
                sweep_angle.to_bits().hash(state);
            }
        }
    }
}

/// Path fill type for EncodedPath.
/// Intentionally separate from crate::canvas::FillType to keep the draw IR
/// module independent of platform-specific canvas types.
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
    SubtreePicture(SubtreeId),
    Shader(ShaderSpec),
}

/// Deferred child reference recorded by `DrawOp::ScriptRuntimeEffect`.
/// `PictureSubtree` carries a canvas owner id rather than a `DrawOpRange`,
/// because the range cannot be materialised until the subtree ops are emitted
/// into the main builder during `render_draw_script` expansion.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptRuntimeEffectChild {
    Image(ImageRef),
    PictureSubtree { owner_id: String },
}

impl std::hash::Hash for ScriptRuntimeEffectChild {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ScriptRuntimeEffectChild::Image(img) => {
                0_u8.hash(state);
                img.hash(state);
            }
            ScriptRuntimeEffectChild::PictureSubtree { owner_id } => {
                1_u8.hash(state);
                owner_id.hash(state);
            }
        }
    }
}

/// IR-native shader specification for draw encoding.
/// Note: This is separate from crate::canvas::paint::ShaderSpec to avoid coupling
/// the draw IR to the canvas paint types. It uses a simpler encoding-oriented shape
/// (tuple colors instead of separate stops/colors vectors, no tile_mode).
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

impl std::hash::Hash for ShaderType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ShaderType::LinearGradient { start, end, colors } => {
                0_u8.hash(state);
                start.0.to_bits().hash(state);
                start.1.to_bits().hash(state);
                end.0.to_bits().hash(state);
                end.1.to_bits().hash(state);
                for (stop, rgba) in colors {
                    stop.to_bits().hash(state);
                    for c in rgba {
                        c.to_bits().hash(state);
                    }
                }
            }
            ShaderType::RadialGradient {
                center,
                radius,
                colors,
            } => {
                1_u8.hash(state);
                center.0.to_bits().hash(state);
                center.1.to_bits().hash(state);
                radius.to_bits().hash(state);
                for (stop, rgba) in colors {
                    stop.to_bits().hash(state);
                    for c in rgba {
                        c.to_bits().hash(state);
                    }
                }
            }
        }
    }
}

impl std::hash::Hash for ShaderSpec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.shader_type.hash(state);
    }
}

impl std::hash::Hash for RuntimeEffectChildRef {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            RuntimeEffectChildRef::Image(img) => {
                0_u8.hash(state);
                img.hash(state);
            }
            RuntimeEffectChildRef::Picture(range) => {
                1_u8.hash(state);
                range.hash(state);
            }
            RuntimeEffectChildRef::SubtreePicture(id) => {
                2_u8.hash(state);
                id.hash(state);
            }
            RuntimeEffectChildRef::Shader(spec) => {
                3_u8.hash(state);
                spec.hash(state);
            }
        }
    }
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
        let ref_ = ImageRef::Static {
            asset_id: "test.png".into(),
        };
        match ref_ {
            ImageRef::Static { asset_id } => assert_eq!(asset_id, "test.png"),
            _ => panic!("expected Static"),
        }
    }

    #[test]
    fn image_ref_video_frame_holds_asset_and_time() {
        let ref_ = ImageRef::VideoFrame {
            asset_id: "clip.mp4".into(),
            time_micros: 1_400_000,
        };
        match ref_ {
            ImageRef::VideoFrame {
                asset_id,
                time_micros,
            } => {
                assert_eq!(asset_id, "clip.mp4");
                assert_eq!(time_micros, 1_400_000);
            }
            _ => panic!("expected VideoFrame"),
        }
    }

    #[test]
    fn table_range_defaults() {
        let range = TableRange { start: 0, len: 10 };
        assert_eq!(range.start, 0);
        assert_eq!(range.len, 10);
    }

    #[test]
    fn draw_op_range_equality() {
        let a = DrawOpRange {
            start_op: 0,
            op_len: 5,
        };
        let b = DrawOpRange {
            start_op: 0,
            op_len: 5,
        };
        let c = DrawOpRange {
            start_op: 1,
            op_len: 5,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn resource_ref_equality() {
        let a = ResourceRef::StaticImage("img.png".into());
        let b = ResourceRef::StaticImage("img.png".into());
        let c = ResourceRef::RuntimeEffect(EffectId(0));
        assert_eq!(a, b);
        assert_ne!(a, c);
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

    #[test]
    fn encoded_path_constructs_with_ops() {
        let path = EncodedPath {
            fill_type: FillType::Winding,
            ops: vec![
                PathOp::MoveTo { x: 0.0, y: 0.0 },
                PathOp::LineTo { x: 10.0, y: 10.0 },
                PathOp::Close,
            ],
        };
        assert_eq!(path.fill_type, FillType::Winding);
        assert_eq!(path.ops.len(), 3);
    }

    #[test]
    fn path_op_variants_exist() {
        let move_to = PathOp::MoveTo { x: 1.0, y: 2.0 };
        match move_to {
            PathOp::MoveTo { x, y } => {
                assert_eq!(x, 1.0);
                assert_eq!(y, 2.0);
            }
            _ => panic!("expected MoveTo"),
        }

        let rect = PathOp::AddRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        };
        assert!(matches!(rect, PathOp::AddRect { .. }));

        let oval = PathOp::AddOval {
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 60.0,
        };
        assert!(matches!(oval, PathOp::AddOval { .. }));

        let arc = PathOp::AddArc {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            start_angle: 0.0,
            sweep_angle: 180.0,
        };
        assert!(matches!(arc, PathOp::AddArc { .. }));
    }
}
