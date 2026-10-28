// ---------------------------------------------------------------------------
// Versioned DrawOp wire protocol (OCIR envelope) — canonical self-contained
// encoding (issue #45).
//
// Single Skia-compatible binary contract owned by core (issue #22):
//
//   header: magic "OCIR" | version u32 | section_count u32
//   section directory: section_count × { id u32, offset u32, len u32 }
//   payloads: 4-byte aligned sections for ops, pools, paints, paths, ...
//
// The envelope is a pure function of RenderFrame: encode(RenderFrame) produces
// byte-identical output for the same RenderFrame, requires no epoch/delta/
// history state, and a fresh decoder can independently decode any single frame.
// Generated-image RGBA is fully encoded every frame (section 12).
//
// Ops layout (section 1, v6):
//   [opcode: u8] [payload...]   payload length fixed per opcode, no length
//   field, no alignment padding. A decoder advances by
//   1 + OPCODE_PAYLOAD_LEN[opcode] and may read at any byte offset.
//
// WASM transport only copies these bytes to JS — it does not re-encode
// protocol semantics. TypeScript decodeFrame must match this schema field-for-field.
// ---------------------------------------------------------------------------

use super::draw_op::*;
use super::draw_types::*;
use super::media_plan::FrameGeneratedImage;
use crate::canvas::paint::{
    BlendMode, BlurStyle, ColorFilterSpec, FillSpec, FilterChainSpec, FilterInput, FilterPrimKind,
    ImageFilterSpec, LightSpec, MaskFilterSpec, PaintSpec, PaintStyle, PathEffectSpec,
    ShaderSpec as PaintShaderSpec, StrokeCap, StrokeJoin, TileMode, TransferFn,
};
use crate::ir::RenderFrame;

// ---------------------------------------------------------------------------
// Magic, version, sections
// ---------------------------------------------------------------------------

/// Magic bytes for the unified DrawOp wire envelope.
pub const IR_MAGIC: [u8; 4] = *b"OCIR";

/// Wire protocol version.
///
/// v6 (issue #46): Dense op packing. The 8-byte op header
/// (`opcode u16 | flags u16 | payload_len u32`) and 4-byte alignment padding
/// are gone. Every op is now `[opcode: u8][payload]` where the payload length
/// is derived from [`opcode::OPCODE_PAYLOAD_LEN`] — no length field, no
/// padding, ops are unpacked op-by-op and may straddle any byte offset (the JS
/// decoder reads through `DataView`, which is offset-agnostic). `PathOp` is no
/// longer a sub-typed `PATH_OP` op; each path command is promoted to its own
/// top-level opcode (40..=48). Still byte-deterministic and self-contained
/// (no epoch/delta/history): encode(RenderFrame) is a pure function.
///
/// v5 (issue #45): No pipeline_epoch in the header. OCIR is a pure, self-
/// contained encoding of RenderFrame: encode(RenderFrame) is a pure function,
/// requires no epoch/delta/history state, and a fresh decoder can independently
/// decode any single frame. Generated-image RGBA is fully encoded every frame
/// in SECTION_GENERATED_IMAGES (12). Section 12 is always present (count may be 0).
///
/// v4: pipeline_epoch in the header + SECTION_GENERATED_IMAGES (12) for the
/// per-frame generated-image delta.
///
/// v8: SAVE_BACKDROP_LAYER payload grows by 4×f32 border-box corner radii
/// (CSS clips the filtered backdrop to the rounded border box).
///
/// v9 (GPU 协议面): SECTION_GPU_STREAMS (13) — WebGL2 命令流段，承载
/// `getContext('webgl2')` 录制面的位编码命令（见 `ir::gpu_stream`）。
pub const IR_VERSION: u32 = 9;

/// Section identifiers in the OCIR directory.
pub mod section {
    pub const OPS: u32 = 1;
    pub const F32_POOL: u32 = 2;
    pub const BYTES: u32 = 3;
    pub const BYTE_RANGES: u32 = 4;
    pub const STRINGS_UTF8: u32 = 5;
    pub const STRING_RANGES: u32 = 6;
    pub const PAINTS: u32 = 7;
    pub const PATHS: u32 = 8;
    pub const CHILDREN: u32 = 9;
    pub const EFFECTS: u32 = 10;
    pub const SUBTREES: u32 = 11;
    pub const GENERATED_IMAGES: u32 = 12;
    pub const GPU_STREAMS: u32 = 13;
}

/// Encoding failure (unknown required string id, oversized envelope, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeError(pub String);

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for EncodeError {}

/// Opcode assignments — each DrawOp variant gets a unique u16.
/// These MUST match the opcode constants in the TypeScript decoder.
pub mod opcode {
    // DrawOp opcodes
    pub const SAVE: u16 = 0;
    pub const SAVE_LAYER: u16 = 1;
    pub const RESTORE: u16 = 2;
    pub const RESTORE_TO_COUNT: u16 = 3;
    pub const TRANSLATE: u16 = 4;
    pub const SCALE: u16 = 5;
    pub const ROTATE: u16 = 6;
    pub const SKEW: u16 = 7;
    pub const CONCAT: u16 = 8;
    pub const SET_FILL_STYLE: u16 = 9;
    pub const SET_STROKE_STYLE: u16 = 10;
    pub const SET_LINE_WIDTH: u16 = 11;
    pub const SET_LINE_CAP: u16 = 12;
    pub const SET_LINE_JOIN: u16 = 13;
    pub const SET_LINE_DASH: u16 = 14;
    pub const CLEAR_LINE_DASH: u16 = 15;
    pub const SET_GLOBAL_ALPHA: u16 = 16;
    pub const SET_ANTI_ALIAS: u16 = 17;
    pub const BEGIN_PATH: u16 = 18;
    pub const PATH_OP: u16 = 19;
    pub const FILL_PATH: u16 = 20;
    pub const STROKE_PATH: u16 = 21;
    pub const CLIP_PATH: u16 = 22;
    pub const CLEAR: u16 = 23;
    pub const PAINT: u16 = 24;
    pub const RECT: u16 = 25;
    pub const R_RECT: u16 = 26;
    pub const D_RRECT: u16 = 27;
    pub const OVAL: u16 = 28;
    pub const CIRCLE: u16 = 29;
    pub const ARC: u16 = 30;
    pub const LINE: u16 = 31;
    pub const POINTS: u16 = 32;
    pub const DRAW_PATH: u16 = 33;
    pub const IMAGE: u16 = 34;
    pub const IMAGE_RECT: u16 = 35;
    pub const RUNTIME_EFFECT: u16 = 36;
    pub const REPLAY_RANGE: u16 = 37;
    pub const DRAW_SUBTREE_PICTURE: u16 = 38;
    pub const LOTTIE_RECT: u16 = 39;
    /// v7 extension of the v6 stream: CSS `backdrop-filter` layer. Same
    /// 25-byte payload layout as `SAVE_LAYER` (alpha always written as 1.0).
    pub const SAVE_BACKDROP_LAYER: u16 = 49;

    // PathOp sub-opcodes (embedded in PATH_OP payload)
    pub const PATH_MOVE_TO: u16 = 0;
    pub const PATH_LINE_TO: u16 = 1;
    pub const PATH_QUAD_TO: u16 = 2;
    pub const PATH_CUBIC_TO: u16 = 3;
    pub const PATH_CLOSE: u16 = 4;
    pub const PATH_ADD_RECT: u16 = 5;
    pub const PATH_ADD_RRECT: u16 = 6;
    pub const PATH_ADD_OVAL: u16 = 7;
    pub const PATH_ADD_ARC: u16 = 8;

    // v6: PathOp sub-opcodes promoted to top-level opcodes (40..=48). In v6 an
    // opcode is a u8 on the wire; these replace the old `PATH_OP` (19) op with
    // its embedded u16 sub-opcode, so a path command costs 1 byte of header
    // instead of 10.
    pub const PATH_MOVE_TO_OP: u16 = 40;
    pub const PATH_LINE_TO_OP: u16 = 41;
    pub const PATH_QUAD_TO_OP: u16 = 42;
    pub const PATH_CUBIC_TO_OP: u16 = 43;
    pub const PATH_CLOSE_OP: u16 = 44;
    pub const PATH_ADD_RECT_OP: u16 = 45;
    pub const PATH_ADD_RRECT_OP: u16 = 46;
    pub const PATH_ADD_OVAL_OP: u16 = 47;
    pub const PATH_ADD_ARC_OP: u16 = 48;
    /// svg `<pattern>` fill: tile picture recording + tile metadata.
    pub const FILL_PATTERN: u16 = 50;
    /// svg `<pattern>` stroke: same tile layout + inline stroke params.
    pub const STROKE_PATTERN: u16 = 52;
    /// svg `<mask>`: content subtree → per-pixel coverage clip.
    pub const MASK_CLIP: u16 = 53;

    /// Per-opcode payload length in bytes for the v6 dense op stream.
    ///
    /// Index by opcode (0..=52). An op is encoded as `[opcode: u8][N payload
    /// bytes]` with no length field and no padding, so `N` must be a fixed
    /// constant for every opcode variant — which holds: every optional-looking
    /// payload (SaveLayer bounds/paint, Image/ImageRect src) writes its fields
    /// unconditionally and signals absence via a flag byte, and `ImageRef`
    /// always writes tag(1) + id-slot(12).
    pub const OPCODE_PAYLOAD_LEN: [u16; 54] = [
        0,  // 0  SAVE
        25, // 1  SAVE_LAYER  [u8 flags][4xf32 bounds][u32 paint][f32 alpha]
        0,  // 2  RESTORE
        4,  // 3  RESTORE_TO_COUNT
        8,  // 4  TRANSLATE
        8,  // 5  SCALE
        12, // 6  ROTATE
        8,  // 7  SKEW
        36, // 8  CONCAT
        4,  // 9  SET_FILL_STYLE
        4,  // 10 SET_STROKE_STYLE
        4,  // 11 SET_LINE_WIDTH
        4,  // 12 SET_LINE_CAP
        4,  // 13 SET_LINE_JOIN
        12, // 14 SET_LINE_DASH
        0,  // 15 CLEAR_LINE_DASH
        4,  // 16 SET_GLOBAL_ALPHA
        1,  // 17 SET_ANTI_ALIAS
        0,  // 18 BEGIN_PATH
        0,  // 19 PATH_OP        (v5 only; unused in v6)
        0,  // 20 FILL_PATH
        0,  // 21 STROKE_PATH
        2,  // 22 CLIP_PATH [u8 anti_alias][u8 fill_type]
        16, // 23 CLEAR
        4,  // 24 PAINT
        20, // 25 RECT
        36, // 26 R_RECT
        68, // 27 D_RRECT
        20, // 28 OVAL
        16, // 29 CIRCLE
        29, // 30 ARC
        20, // 31 LINE
        16, // 32 POINTS
        8,  // 33 DRAW_PATH
        25, // 34 IMAGE
        50, // 35 IMAGE_RECT
        32, // 36 RUNTIME_EFFECT
        8,  // 37 REPLAY_RANGE
        12, // 38 DRAW_SUBTREE_PICTURE
        24, // 39 LOTTIE_RECT  [u32 bundle_id][f32 frame][4xf32 dst]
        8,  // 40 PATH_MOVE_TO   [2xf32]
        8,  // 41 PATH_LINE_TO   [2xf32]
        16, // 42 PATH_QUAD_TO   [4xf32]
        24, // 43 PATH_CUBIC_TO  [6xf32]
        0,  // 44 PATH_CLOSE
        16, // 45 PATH_ADD_RECT  [4xf32]
        20, // 46 PATH_ADD_RRECT [5xf32]
        16, // 47 PATH_ADD_OVAL  [4xf32]
        24, // 48 PATH_ADD_ARC   [6xf32]
        41, // 49 SAVE_BACKDROP_LAYER — SAVE_LAYER layout + 4×f32 corner radii
        52, // 50 FILL_PATTERN [u32 path][u32 subtree][4xf32 tile][6xf32 pt][f32 opacity]
        0,  // 51 (unused)
        62, // 52 STROKE_PATTERN — FILL_PATTERN layout + [f32 width][u8 cap][u8 join][f32 miter]
        21, // 53 MASK_CLIP [u32 subtree][u8 kind][4xf32 region]
    ];

    /// Highest opcode in use. v6 encoders never emit an opcode above this, so
    /// it must fit in the wire's 1-byte opcode field.
    pub const MAX_OPCODE: u16 = PATH_ADD_ARC_OP;

    /// Map a PathOp sub-opcode (0..=8) to its v6 top-level opcode.
    #[inline]
    pub fn path_sub_to_opcode(sub: u16) -> u16 {
        PATH_MOVE_TO_OP + sub
    }

    /// Number of f32 values for each PathOp sub-opcode by its stored kind.
    /// Indexed by PATH_* value (0=MoveTo … 8=AddArc). Retained for the
    /// section-8 PATHS table (whose path commands still use the u16 sub-opcode).
    pub const PATH_OP_F32_WIDTHS: [u8; 9] = [2, 2, 4, 6, 0, 4, 5, 4, 6];
}

// ---------------------------------------------------------------------------
// Intermediate section payloads (not a second on-wire envelope)
// ---------------------------------------------------------------------------

/// Binary section payloads produced before packing the OCIR envelope.
///
/// This is an encoder intermediate — not the dual OCDF protocol deleted in #22.
/// Tests inspect these fields; hosts only ever receive the packed `Vec<u8>`.
#[derive(Clone, Debug, Default)]
pub struct EncodedDrawSections {
    pub ops: Vec<u8>,
    pub subtrees: Vec<u8>,
    pub f32_pool: Vec<f32>,
    pub bytes: Vec<u8>,
    pub byte_ranges: Vec<TableRange>,
    pub strings_utf8: Vec<u8>,
    pub string_ranges: Vec<TableRange>,
}

/// Encode DrawOpFrame tables into section payloads, reusing scratch buffers.
///
/// Does **not** encode paints/paths/children/effects — those are packed from
/// the typed frame when building the envelope (see [`encode_ir_envelope`]).
pub fn encode_draw_sections(
    frame: &super::draw_frame::DrawOpFrame,
    scratch: &mut super::draw_frame::DrawFrameScratch,
) -> EncodedDrawSections {
    scratch.clear();

    scratch.f32_pool.extend_from_slice(&frame.f32_pool);

    let encoded_ops = &mut scratch.encoded_ops;
    for op in &frame.ops {
        encode_op(op, encoded_ops, &mut scratch.f32_pool, &frame.strings);
    }
    let encoded_subtrees = &mut scratch.encoded_subtrees;
    write_u32(encoded_subtrees, frame.subtrees.len() as u32);
    for subtree in &frame.subtrees {
        let start = encoded_subtrees.len();
        write_u32(encoded_subtrees, 0);
        let body_start = encoded_subtrees.len();
        for op in subtree {
            encode_op(op, encoded_subtrees, &mut scratch.f32_pool, &frame.strings);
        }
        let byte_len = (encoded_subtrees.len() - body_start) as u32;
        encoded_subtrees[start..start + 4].copy_from_slice(&byte_len.to_le_bytes());
    }
    let f32_pool_out = std::mem::take(&mut scratch.f32_pool);

    let strings_utf8 = &mut scratch.strings_utf8;
    let string_ranges = &mut scratch.string_ranges;
    for s in &frame.strings {
        let start = strings_utf8.len() as u32;
        let bytes = s.as_bytes();
        strings_utf8.extend_from_slice(bytes);
        string_ranges.push(TableRange {
            start,
            len: bytes.len() as u32,
        });
    }

    EncodedDrawSections {
        ops: std::mem::take(encoded_ops),
        subtrees: std::mem::take(encoded_subtrees),
        f32_pool: f32_pool_out,
        bytes: frame.bytes.clone(),
        byte_ranges: frame.byte_ranges.clone(),
        strings_utf8: std::mem::take(strings_utf8),
        string_ranges: std::mem::take(string_ranges),
    }
}

/// Intern asset_id / bundle_id strings referenced by image and Lottie ops so
/// binary image refs can address them via the string table.
pub fn intern_image_strings(draw: &mut super::draw_frame::DrawOpFrame) {
    fn push_unique(strings: &mut Vec<String>, asset_id: &str) {
        if !strings.iter().any(|item| item == asset_id) {
            strings.push(asset_id.to_string());
        }
    }

    for op in &draw.ops {
        intern_image_strings_in_ops(&mut draw.strings, std::slice::from_ref(op));
    }
    for subtree in &draw.subtrees {
        intern_image_strings_in_ops(&mut draw.strings, subtree);
    }
    for child in &draw.children {
        if let RuntimeEffectChildRef::Image(image) = child {
            intern_image_ref(&mut draw.strings, image);
        }
    }

    fn intern_image_strings_in_ops(strings: &mut Vec<String>, ops: &[DrawOp]) {
        for op in ops {
            match op {
                DrawOp::Image { image, .. } | DrawOp::ImageRect { image, .. } => {
                    intern_image_ref(strings, image);
                }
                DrawOp::LottieRect { bundle_id, .. } => push_unique(strings, bundle_id),
                _ => {}
            }
        }
    }

    fn intern_image_ref(strings: &mut Vec<String>, image: &ImageRef) {
        match image {
            ImageRef::Static { asset_id } | ImageRef::VideoFrame { asset_id, .. } => {
                push_unique(strings, asset_id);
            }
            ImageRef::Generated { .. } | ImageRef::Encoded { .. } => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Low-level encoding helpers
// ---------------------------------------------------------------------------

/// Write a f32 into a u8 buffer in little-endian.
#[inline]
fn write_f32(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Write a u32 into a u8 buffer in little-endian.
#[inline]
fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Write a u64 into a u8 buffer in little-endian.
#[inline]
fn write_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Write a u8 into a u8 buffer.
#[inline]
fn write_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}

/// Write an i32 into a u8 buffer in little-endian.
#[inline]
fn write_i32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Write a v6 op header: a single `[opcode: u8]`. The payload that follows is
/// exactly `OPCODE_PAYLOAD_LEN[opcode]` bytes (see [`opcode`]); there is no
/// length field and ops are not padded, so a decoder advances by
/// `1 + OPCODE_PAYLOAD_LEN[opcode]` and ops may straddle any byte offset.
#[inline]
fn write_op_header(buf: &mut Vec<u8>, op: u16) {
    debug_assert!(
        op <= opcode::MAX_OPCODE,
        "opcode {op} exceeds MAX_OPCODE ({}); must fit in a u8",
        opcode::MAX_OPCODE
    );
    buf.push(op as u8);
}

/// Encode a ColorU8 as a packed u32: R | G<<8 | B<<16 | A<<24.
#[inline]
fn encode_color_u8(c: ColorU8) -> u32 {
    (c.r as u32) | ((c.g as u32) << 8) | ((c.b as u32) << 16) | ((c.a as u32) << 24)
}

/// Encode a LineCap variant as its u32 discriminant.
/// Mapping: 0=Butt, 1=Round, 2=Square.
#[inline]
pub(crate) fn encode_line_cap(cap: LineCap) -> u32 {
    match cap {
        LineCap::Butt => 0,
        LineCap::Round => 1,
        LineCap::Square => 2,
    }
}

/// Encode a LineJoin variant as its u32 discriminant.
/// Mapping: 0=Miter, 1=Round, 2=Bevel.
#[inline]
pub(crate) fn encode_line_join(join: LineJoin) -> u32 {
    match join {
        LineJoin::Miter => 0,
        LineJoin::Round => 1,
        LineJoin::Bevel => 2,
    }
}

/// Encode a PointMode variant as its u32 discriminant.
/// Mapping: 0=Points, 1=Lines, 2=Polygon.
#[inline]
pub(crate) fn encode_point_mode(mode: PointMode) -> u32 {
    match mode {
        PointMode::Points => 0,
        PointMode::Lines => 1,
        PointMode::Polygon => 2,
    }
}

/// Write a Rect4 into the buffer as 4 x f32 (x, y, width, height).
#[inline]
fn write_rect4(buf: &mut Vec<u8>, r: Rect4) {
    write_f32(buf, r.x);
    write_f32(buf, r.y);
    write_f32(buf, r.width);
    write_f32(buf, r.height);
}

/// Write a Radii4 into the buffer as 4 x f32 (top_left, top_right, bottom_right, bottom_left).
#[inline]
fn write_radii4(buf: &mut Vec<u8>, r: Radii4) {
    write_f32(buf, r.top_left);
    write_f32(buf, r.top_right);
    write_f32(buf, r.bottom_right);
    write_f32(buf, r.bottom_left);
}

/// Write a DRRectSpec into the buffer: rect (4xf32) + radii (4xf32) = 32 bytes.
#[inline]
fn write_drrect_spec(buf: &mut Vec<u8>, s: DRRectSpec) {
    write_rect4(buf, s.rect);
    write_radii4(buf, s.radii);
}

/// Look up an asset_id string in frame.strings, returning its index as u32.
/// Panics if the string is not found (should never happen with well-formed frames).
fn lookup_string_id(strings: &[String], s: &str) -> u32 {
    strings
        .iter()
        .position(|item| item == s)
        .expect("asset_id not found in frame.strings; ensure it is interned via DrawOpBuilder::intern_string")
        as u32
}

// ---------------------------------------------------------------------------
// OCIR section encoders + envelope packer
// ---------------------------------------------------------------------------

/// Pack a typed render frame into the single self-contained OCIR envelope.
///
/// Canonical encoding (issue #45): the same `RenderFrame` always produces
/// byte-identical output; no epoch/delta/history state is needed.
///
/// Callers that need image/Lottie string ids in the table should run
/// [`intern_image_strings`] on the frame's draw first.
pub fn encode_ir_envelope(
    render_frame: &RenderFrame,
    scratch: &mut super::draw_frame::DrawFrameScratch,
) -> Result<Vec<u8>, EncodeError> {
    let sections = encode_draw_sections(&render_frame.draw, scratch);
    pack_ir_envelope(
        &render_frame.draw,
        &render_frame.media.generated_images,
        render_frame.gpu.as_ref(),
        &sections,
    )
}

/// Pack pre-encoded op/string sections with paints/paths/… into OCIR bytes.
pub fn pack_ir_envelope(
    draw: &super::draw_frame::DrawOpFrame,
    generated_images: &[FrameGeneratedImage],
    gpu: Option<&super::gpu_stream::GpuSection>,
    encoded: &EncodedDrawSections,
) -> Result<Vec<u8>, EncodeError> {
    let generated_images_section = encode_generated_images(generated_images);
    let gpu_streams_section = encode_gpu_streams(gpu);
    let section_payloads = [
        (section::OPS, encoded.ops.clone()),
        (section::SUBTREES, encoded.subtrees.clone()),
        (section::F32_POOL, encode_f32_slice(&encoded.f32_pool)),
        (section::BYTES, encoded.bytes.clone()),
        (section::BYTE_RANGES, encode_ranges(&encoded.byte_ranges)),
        (section::STRINGS_UTF8, encoded.strings_utf8.clone()),
        (
            section::STRING_RANGES,
            encode_ranges(&encoded.string_ranges),
        ),
        (section::PAINTS, encode_paints(&draw.paints)?),
        (section::PATHS, encode_paths(&draw.paths)),
        (
            section::CHILDREN,
            encode_children(&draw.children, &draw.strings)?,
        ),
        (section::EFFECTS, encode_effects(&draw.effects)),
        (section::GENERATED_IMAGES, generated_images_section),
        (section::GPU_STREAMS, gpu_streams_section),
    ];

    let header_len = 12 + section_payloads.len() * 12;
    let mut offsets = Vec::with_capacity(section_payloads.len());
    let mut cursor = header_len as u32;
    for (_, bytes) in &section_payloads {
        cursor = align4(cursor);
        offsets.push(cursor);
        cursor = cursor
            .checked_add(bytes.len() as u32)
            .ok_or_else(|| EncodeError("IR envelope too large".into()))?;
    }

    let mut out = Vec::with_capacity(cursor as usize);
    out.extend_from_slice(&IR_MAGIC);
    write_u32(&mut out, IR_VERSION);
    write_u32(&mut out, section_payloads.len() as u32);
    for ((id, bytes), offset) in section_payloads.iter().zip(offsets.iter()) {
        write_u32(&mut out, *id);
        write_u32(&mut out, *offset);
        write_u32(&mut out, bytes.len() as u32);
    }

    for ((_, bytes), offset) in section_payloads.iter().zip(offsets.iter()) {
        while out.len() < *offset as usize {
            out.push(0);
        }
        out.extend_from_slice(bytes);
    }

    Ok(out)
}

fn encode_generated_images(delta: &[FrameGeneratedImage]) -> Vec<u8> {
    let mut out = Vec::new();
    write_u32(&mut out, delta.len() as u32);
    for record in delta {
        write_u64(&mut out, record.id.0);
        write_u32(&mut out, record.width);
        write_u32(&mut out, record.height);
        write_bytes_with_len(&mut out, &record.rgba);
    }
    out
}

/// GPU 命令流段（section 13）：
/// `u32 count` × per stream `{ u64 image_id, str target_id, u32 w, u32 h,
/// u8 backend, u8 attr_bits, u32 ops_len + ops, u32 bytes_len + bytes,
/// u32 ranges_count + (u32 start, u32 len)×n }`。
fn encode_gpu_streams(gpu: Option<&super::gpu_stream::GpuSection>) -> Vec<u8> {
    let mut out = Vec::new();
    match gpu {
        None => write_u32(&mut out, 0),
        Some(section) => {
            write_u32(&mut out, section.streams.len() as u32);
            for stream in &section.streams {
                write_u64(&mut out, stream.image_id.0);
                write_u32(&mut out, stream.target_id.len() as u32);
                out.extend_from_slice(stream.target_id.as_bytes());
                write_u32(&mut out, stream.width);
                write_u32(&mut out, stream.height);
                write_u8(&mut out, stream.backend as u8);
                write_u8(&mut out, stream.attrs.pack());
                write_u32(&mut out, stream.ops.ops.len() as u32);
                out.extend_from_slice(&stream.ops.ops);
                write_u32(&mut out, stream.ops.bytes.len() as u32);
                out.extend_from_slice(&stream.ops.bytes);
                write_u32(&mut out, stream.ops.byte_ranges.len() as u32);
                for range in &stream.ops.byte_ranges {
                    write_u32(&mut out, range.start);
                    write_u32(&mut out, range.len);
                }
            }
        }
    }
    out
}

fn encode_paints(paints: &[PaintSpec]) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    write_u32(&mut out, paints.len() as u32);
    for paint in paints {
        let mut record = Vec::new();
        encode_paint(&mut record, paint)?;
        write_u32(&mut out, record.len() as u32);
        out.extend_from_slice(&record);
    }
    Ok(out)
}

fn encode_paint(out: &mut Vec<u8>, paint: &PaintSpec) -> Result<(), EncodeError> {
    encode_fill(out, &paint.fill);
    write_u8(out, encode_paint_style(paint.style));
    write_u8(out, if paint.anti_alias { 1 } else { 0 });
    write_u8(out, encode_blend_mode(paint.blend_mode));
    match &paint.stroke {
        Some(stroke) => {
            write_u8(out, 1);
            write_f32(out, stroke.width);
            write_u8(out, encode_stroke_cap(stroke.cap));
            write_u8(out, encode_stroke_join(stroke.join));
            write_f32(out, stroke.miter_limit);
        }
        None => write_u8(out, 0),
    }
    encode_optional(out, paint.image_filter.as_ref(), encode_image_filter)?;
    encode_optional(out, paint.color_filter.as_ref(), encode_color_filter)?;
    encode_optional(out, paint.mask_filter.as_ref(), encode_mask_filter)?;
    encode_optional(out, paint.path_effect.as_ref(), encode_path_effect)?;
    Ok(())
}

fn encode_fill(out: &mut Vec<u8>, fill: &FillSpec) {
    match fill {
        FillSpec::Solid(color) => {
            write_u8(out, 0);
            write_f32_array(out, color);
        }
        FillSpec::Shader(shader) => {
            write_u8(out, 1);
            encode_paint_shader(out, shader);
        }
    }
}

fn encode_paint_shader(out: &mut Vec<u8>, shader: &PaintShaderSpec) {
    match shader {
        PaintShaderSpec::LinearGradient {
            from,
            to,
            stops,
            colors,
            tile_mode,
            local_matrix,
            interpolate_premul,
        } => {
            write_u8(out, 0);
            write_u8(out, encode_tile_mode(*tile_mode));
            write_u8(out, *interpolate_premul as u8);
            write_f32_array(out, from);
            write_f32_array(out, to);
            write_f32_vec(out, stops);
            write_color_vec(out, colors);
            encode_optional_matrix(out, local_matrix);
        }
        PaintShaderSpec::RadialGradient {
            center,
            radius,
            stops,
            colors,
            tile_mode,
            local_matrix,
            interpolate_premul,
            focal,
        } => {
            write_u8(out, 1);
            write_u8(out, encode_tile_mode(*tile_mode));
            write_u8(out, *interpolate_premul as u8);
            write_f32_array(out, center);
            write_f32(out, *radius);
            write_f32_vec(out, stops);
            write_color_vec(out, colors);
            encode_optional_matrix(out, local_matrix);
            match focal {
                Some(f) => {
                    write_u8(out, 1);
                    write_f32_array(out, f);
                }
                None => write_u8(out, 0),
            }
        }
        PaintShaderSpec::Turbulence {
            base_frequency,
            num_octaves,
            seed,
            tile,
            origin,
        } => {
            write_u8(out, 2);
            write_f32_array(out, base_frequency);
            write_u32(out, *num_octaves);
            write_u32(out, *seed);
            match tile {
                Some(t) => {
                    write_u8(out, 1);
                    write_f32_array(out, t);
                }
                None => write_u8(out, 0),
            }
            write_f32_array(out, origin);
        }
        PaintShaderSpec::SweepGradient {
            center,
            start_angle_deg,
            end_angle_deg,
            stops,
            colors,
            tile_mode,
            interpolate_premul,
        } => {
            write_u8(out, 3);
            write_u8(out, encode_tile_mode(*tile_mode));
            write_u8(out, *interpolate_premul as u8);
            write_f32_array(out, center);
            write_f32(out, *start_angle_deg);
            write_f32(out, *end_angle_deg);
            write_f32_vec(out, stops);
            write_color_vec(out, colors);
        }
    }
}

fn encode_optional_matrix(out: &mut Vec<u8>, matrix: &Option<[f32; 9]>) {
    match matrix {
        Some(m) => {
            write_u8(out, 1);
            for v in m {
                write_f32(out, *v);
            }
        }
        None => write_u8(out, 0),
    }
}

fn encode_image_filter(out: &mut Vec<u8>, filter: &ImageFilterSpec) -> Result<(), EncodeError> {
    match filter {
        ImageFilterSpec::Blur {
            sigma_x,
            sigma_y,
            crop_rect,
            decal,
        } => {
            write_u8(out, 0);
            write_f32(out, *sigma_x);
            write_f32(out, *sigma_y);
            write_u8(out, u8::from(*decal));
            match crop_rect {
                Some(rect) => {
                    write_u8(out, 1);
                    write_f32(out, rect.x0 as f32);
                    write_f32(out, rect.y0 as f32);
                    write_f32(out, rect.x1 as f32);
                    write_f32(out, rect.y1 as f32);
                }
                None => write_u8(out, 0),
            }
        }
        ImageFilterSpec::DropShadow {
            dx,
            dy,
            sigma_x,
            sigma_y,
            color,
            keep_content,
        } => {
            write_u8(out, 1);
            write_f32(out, *dx);
            write_f32(out, *dy);
            write_f32(out, *sigma_x);
            write_f32(out, *sigma_y);
            write_f32_array(out, color);
            write_u8(out, if *keep_content { 1 } else { 0 });
        }
        ImageFilterSpec::ColorFilter(filter) => {
            write_u8(out, 2);
            encode_color_filter(out, filter)?;
        }
        ImageFilterSpec::Compose(outer, inner) => {
            write_u8(out, 3);
            encode_image_filter(out, outer)?;
            encode_image_filter(out, inner)?;
        }
        ImageFilterSpec::Chain(chains) => {
            write_u8(out, 4);
            write_u32(out, chains.len() as u32);
            for chain in chains {
                encode_filter_chain(out, chain)?;
            }
        }
    }
    Ok(())
}

/// FilterChainSpec wire 格式（P4，engine 端在消费点展开成 skia filter 树）：
/// `u8 has_source_crop + [4xf32]`、`u16 output`、`u32 n`、随后 n 个
/// primitive（crop 前缀 + srgb + kind tag + 字段）。Input 统一 `u8 tag`
/// （0 SourceGraphic / 1 SourceAlpha / 2 Prim+u16）。
fn encode_filter_chain(out: &mut Vec<u8>, chain: &FilterChainSpec) -> Result<(), EncodeError> {
    match chain.source_crop {
        Some(r) => {
            write_u8(out, 1);
            write_f32(out, r.x0 as f32);
            write_f32(out, r.y0 as f32);
            write_f32(out, r.x1 as f32);
            write_f32(out, r.y1 as f32);
        }
        None => write_u8(out, 0),
    }
    write_u16(out, chain.output);
    write_u32(out, chain.primitives.len() as u32);
    for p in &chain.primitives {
        match p.crop {
            Some(r) => {
                write_u8(out, 1);
                write_f32(out, r.x0 as f32);
                write_f32(out, r.y0 as f32);
                write_f32(out, r.x1 as f32);
                write_f32(out, r.y1 as f32);
            }
            None => write_u8(out, 0),
        }
        write_u8(out, p.srgb as u8);
        encode_filter_prim_kind(out, &p.kind)?;
    }
    Ok(())
}

fn encode_filter_input(out: &mut Vec<u8>, input: &FilterInput) {
    match input {
        FilterInput::SourceGraphic => write_u8(out, 0),
        FilterInput::SourceAlpha => write_u8(out, 1),
        FilterInput::Prim(i) => {
            write_u8(out, 2);
            write_u16(out, *i);
        }
    }
}

fn encode_transfer_fn(out: &mut Vec<u8>, f: &TransferFn) {
    match f {
        TransferFn::Identity => write_u8(out, 0),
        TransferFn::Table(v) => {
            write_u8(out, 1);
            write_f32_vec(out, v);
        }
        TransferFn::Discrete(v) => {
            write_u8(out, 2);
            write_f32_vec(out, v);
        }
        TransferFn::Linear { slope, intercept } => {
            write_u8(out, 3);
            write_f32(out, *slope);
            write_f32(out, *intercept);
        }
        TransferFn::Gamma {
            amplitude,
            exponent,
            offset,
        } => {
            write_u8(out, 4);
            write_f32(out, *amplitude);
            write_f32(out, *exponent);
            write_f32(out, *offset);
        }
    }
}

fn encode_light_spec(out: &mut Vec<u8>, light: &LightSpec) {
    match light {
        LightSpec::Distant { azimuth, elevation } => {
            write_u8(out, 0);
            write_f32(out, *azimuth);
            write_f32(out, *elevation);
        }
        LightSpec::Point { x, y, z } => {
            write_u8(out, 1);
            write_f32(out, *x);
            write_f32(out, *y);
            write_f32(out, *z);
        }
        LightSpec::Spot {
            x,
            y,
            z,
            points_at_x,
            points_at_y,
            points_at_z,
            specular_exponent,
            limiting_cone_angle,
        } => {
            write_u8(out, 2);
            write_f32(out, *x);
            write_f32(out, *y);
            write_f32(out, *z);
            write_f32(out, *points_at_x);
            write_f32(out, *points_at_y);
            write_f32(out, *points_at_z);
            write_f32(out, *specular_exponent);
            write_f32(out, *limiting_cone_angle);
        }
    }
}

fn encode_filter_prim_kind(out: &mut Vec<u8>, kind: &FilterPrimKind) -> Result<(), EncodeError> {
    match kind {
        FilterPrimKind::Blend { mode, in1, in2 } => {
            write_u8(out, 0);
            write_u8(out, encode_blend_mode(*mode));
            encode_filter_input(out, in1);
            encode_filter_input(out, in2);
        }
        FilterPrimKind::ColorMatrix { row_major, input } => {
            write_u8(out, 1);
            write_f32_array(out, row_major);
            encode_filter_input(out, input);
        }
        FilterPrimKind::ComponentTransfer { fns, input } => {
            write_u8(out, 2);
            for f in fns {
                encode_transfer_fn(out, f);
            }
            encode_filter_input(out, input);
        }
        FilterPrimKind::Composite { op, k, in1, in2 } => {
            write_u8(out, 3);
            write_u8(out, *op);
            write_f32_array(out, k);
            encode_filter_input(out, in1);
            encode_filter_input(out, in2);
        }
        FilterPrimKind::ConvolveMatrix {
            order,
            kernel,
            divisor,
            bias,
            target,
            kernel_unit_length,
            edge,
            preserve_alpha,
            input,
        } => {
            write_u8(out, 4);
            write_u32(out, order.0);
            write_u32(out, order.1);
            write_f32_vec(out, kernel);
            write_f32(out, *divisor);
            write_f32(out, *bias);
            write_i32(out, target.0);
            write_i32(out, target.1);
            write_f32(out, kernel_unit_length.0);
            write_f32(out, kernel_unit_length.1);
            write_u8(out, *edge);
            write_u8(out, *preserve_alpha as u8);
            encode_filter_input(out, input);
        }
        FilterPrimKind::DiffuseLighting {
            surface_scale,
            diffuse_constant,
            lighting_color,
            light,
            input,
        } => {
            write_u8(out, 5);
            write_f32(out, *surface_scale);
            write_f32(out, *diffuse_constant);
            write_f32_array(out, lighting_color);
            encode_light_spec(out, light);
            encode_filter_input(out, input);
        }
        FilterPrimKind::DisplacementMap {
            x_channel,
            y_channel,
            scale,
            input,
            in2,
        } => {
            write_u8(out, 6);
            write_u8(out, *x_channel);
            write_u8(out, *y_channel);
            write_f32(out, *scale);
            encode_filter_input(out, input);
            encode_filter_input(out, in2);
        }
        FilterPrimKind::DropShadow {
            dx,
            dy,
            sigma_x,
            sigma_y,
            color,
            input,
        } => {
            write_u8(out, 7);
            write_f32(out, *dx);
            write_f32(out, *dy);
            write_f32(out, *sigma_x);
            write_f32(out, *sigma_y);
            write_f32_array(out, color);
            encode_filter_input(out, input);
        }
        FilterPrimKind::Flood { color } => {
            write_u8(out, 8);
            write_f32_array(out, color);
        }
        FilterPrimKind::GaussianBlur {
            sigma_x,
            sigma_y,
            input,
        } => {
            write_u8(out, 9);
            write_f32(out, *sigma_x);
            write_f32(out, *sigma_y);
            encode_filter_input(out, input);
        }
        FilterPrimKind::Merge { inputs } => {
            write_u8(out, 10);
            write_u32(out, inputs.len() as u32);
            for input in inputs {
                encode_filter_input(out, input);
            }
        }
        FilterPrimKind::Morphology {
            erode,
            radius_x,
            radius_y,
            input,
        } => {
            write_u8(out, 11);
            write_u8(out, *erode as u8);
            write_f32(out, *radius_x);
            write_f32(out, *radius_y);
            encode_filter_input(out, input);
        }
        FilterPrimKind::Offset { dx, dy, input } => {
            write_u8(out, 12);
            write_f32(out, *dx);
            write_f32(out, *dy);
            encode_filter_input(out, input);
        }
        FilterPrimKind::SpecularLighting {
            surface_scale,
            specular_constant,
            specular_exponent,
            lighting_color,
            light,
            input,
        } => {
            write_u8(out, 13);
            write_f32(out, *surface_scale);
            write_f32(out, *specular_constant);
            write_f32(out, *specular_exponent);
            write_f32_array(out, lighting_color);
            encode_light_spec(out, light);
            encode_filter_input(out, input);
        }
        FilterPrimKind::Tile { input } => {
            write_u8(out, 14);
            encode_filter_input(out, input);
        }
        FilterPrimKind::Image { subtree } => {
            write_u8(out, 16);
            write_u32(out, *subtree);
        }
        FilterPrimKind::Turbulence {
            fractal,
            base_frequency,
            num_octaves,
            seed,
            stitch,
        } => {
            write_u8(out, 15);
            write_u8(out, *fractal as u8);
            write_f32_array(out, base_frequency);
            write_u32(out, *num_octaves);
            write_f32(out, *seed);
            write_u8(out, *stitch as u8);
        }
    }
    Ok(())
}

fn encode_color_filter(out: &mut Vec<u8>, filter: &ColorFilterSpec) -> Result<(), EncodeError> {
    match filter {
        ColorFilterSpec::Matrix(matrix) => {
            write_u8(out, 0);
            write_f32_array(out, matrix);
        }
        ColorFilterSpec::BlendColor { color, mode } => {
            write_u8(out, 1);
            write_f32_array(out, color);
            write_u8(out, encode_blend_mode(*mode));
        }
        ColorFilterSpec::LinearToSrgbGamma => write_u8(out, 2),
        ColorFilterSpec::SrgbToLinearGamma => write_u8(out, 3),
    }
    Ok(())
}

fn encode_mask_filter(out: &mut Vec<u8>, filter: &MaskFilterSpec) -> Result<(), EncodeError> {
    match filter {
        MaskFilterSpec::Blur {
            sigma,
            style,
            respect_ctm,
        } => {
            write_u8(out, 0);
            write_f32(out, *sigma);
            write_u8(out, encode_blur_style(*style));
            write_u8(out, if *respect_ctm { 1 } else { 0 });
        }
    }
    Ok(())
}

fn encode_path_effect(out: &mut Vec<u8>, effect: &PathEffectSpec) -> Result<(), EncodeError> {
    match effect {
        PathEffectSpec::Dash { intervals, phase } => {
            write_u8(out, 0);
            write_f32_vec(out, intervals);
            write_f32(out, *phase);
        }
    }
    Ok(())
}

fn encode_paths(paths: &[EncodedPath]) -> Vec<u8> {
    let mut out = Vec::new();
    write_u32(&mut out, paths.len() as u32);
    for path in paths {
        let mut record = Vec::new();
        write_u8(&mut record, encode_path_fill_type(path.fill_type));
        write_u32(&mut record, path.ops.len() as u32);
        for op in &path.ops {
            encode_section_path_op(&mut record, op);
        }
        write_u32(&mut out, record.len() as u32);
        out.extend_from_slice(&record);
    }
    out
}

fn encode_section_path_op(out: &mut Vec<u8>, op: &PathOp) {
    match op {
        PathOp::MoveTo { x, y } => {
            write_u16(out, 0);
            write_f32(out, *x);
            write_f32(out, *y);
        }
        PathOp::LineTo { x, y } => {
            write_u16(out, 1);
            write_f32(out, *x);
            write_f32(out, *y);
        }
        PathOp::QuadTo { cx, cy, x, y } => {
            write_u16(out, 2);
            write_f32(out, *cx);
            write_f32(out, *cy);
            write_f32(out, *x);
            write_f32(out, *y);
        }
        PathOp::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y,
        } => {
            write_u16(out, 3);
            write_f32(out, *c1x);
            write_f32(out, *c1y);
            write_f32(out, *c2x);
            write_f32(out, *c2y);
            write_f32(out, *x);
            write_f32(out, *y);
        }
        PathOp::Close => write_u16(out, 4),
        PathOp::AddRect {
            x,
            y,
            width,
            height,
        } => {
            write_u16(out, 5);
            write_f32(out, *x);
            write_f32(out, *y);
            write_f32(out, *width);
            write_f32(out, *height);
        }
        PathOp::AddRRect {
            x,
            y,
            width,
            height,
            radius,
        } => {
            write_u16(out, 6);
            write_f32(out, *x);
            write_f32(out, *y);
            write_f32(out, *width);
            write_f32(out, *height);
            write_f32(out, *radius);
        }
        PathOp::AddOval {
            x,
            y,
            width,
            height,
        } => {
            write_u16(out, 7);
            write_f32(out, *x);
            write_f32(out, *y);
            write_f32(out, *width);
            write_f32(out, *height);
        }
        PathOp::AddArc {
            x,
            y,
            width,
            height,
            start_angle,
            sweep_angle,
        } => {
            write_u16(out, 8);
            write_f32(out, *x);
            write_f32(out, *y);
            write_f32(out, *width);
            write_f32(out, *height);
            write_f32(out, *start_angle);
            write_f32(out, *sweep_angle);
        }
    }
}

fn encode_children(
    children: &[RuntimeEffectChildRef],
    strings: &[String],
) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    write_u32(&mut out, children.len() as u32);
    for child in children {
        let mut record = Vec::new();
        match child {
            RuntimeEffectChildRef::Image(image) => {
                write_u8(&mut record, 0);
                encode_image_ref(&mut record, image, strings)?;
            }
            RuntimeEffectChildRef::Picture(range) => {
                write_u8(&mut record, 1);
                write_u32(&mut record, range.start_op);
                write_u32(&mut record, range.op_len);
            }
            RuntimeEffectChildRef::SubtreePicture(subtree) => {
                write_u8(&mut record, 3);
                write_u32(&mut record, subtree.0);
            }
            RuntimeEffectChildRef::Shader(shader) => {
                write_u8(&mut record, 2);
                encode_ir_shader(&mut record, shader);
            }
        }
        write_u32(&mut out, record.len() as u32);
        out.extend_from_slice(&record);
    }
    Ok(out)
}

fn encode_image_ref(
    out: &mut Vec<u8>,
    image: &ImageRef,
    strings: &[String],
) -> Result<(), EncodeError> {
    match image {
        ImageRef::Static { asset_id } => {
            write_u8(out, 0);
            write_u32(out, lookup_string_id_checked(strings, asset_id)?);
            write_u64(out, 0);
        }
        ImageRef::VideoFrame {
            asset_id,
            time_micros,
        } => {
            write_u8(out, 1);
            write_u32(out, lookup_string_id_checked(strings, asset_id)?);
            write_u64(out, *time_micros);
        }
        ImageRef::Generated { id } => {
            write_u8(out, 2);
            write_u64(out, id.0);
            write_u32(out, 0);
        }
        ImageRef::Encoded { data } => {
            write_u8(out, 3);
            write_u32(out, data.0);
            write_u32(out, 0);
            write_u32(out, 0);
        }
    }
    Ok(())
}

fn encode_ir_shader(out: &mut Vec<u8>, shader: &ShaderSpec) {
    match &shader.shader_type {
        ShaderType::LinearGradient { start, end, colors } => {
            write_u8(out, 0);
            write_f32(out, start.0);
            write_f32(out, start.1);
            write_f32(out, end.0);
            write_f32(out, end.1);
            encode_ir_gradient_colors(out, colors);
        }
        ShaderType::RadialGradient {
            center,
            radius,
            colors,
        } => {
            write_u8(out, 1);
            write_f32(out, center.0);
            write_f32(out, center.1);
            write_f32(out, *radius);
            encode_ir_gradient_colors(out, colors);
        }
    }
}

fn encode_ir_gradient_colors(out: &mut Vec<u8>, colors: &[(f32, [f32; 4])]) {
    write_u32(out, colors.len() as u32);
    for (stop, color) in colors {
        write_f32(out, *stop);
        write_f32_array(out, color);
    }
}

fn encode_effects(effects: &[EffectRef]) -> Vec<u8> {
    let mut out = Vec::new();
    write_u32(&mut out, effects.len() as u32);
    for effect in effects {
        write_u64(&mut out, effect.hash);
        write_bytes_with_len(&mut out, effect.sksl.as_bytes());
    }
    out
}

fn encode_optional<T>(
    out: &mut Vec<u8>,
    value: Option<&T>,
    encode: fn(&mut Vec<u8>, &T) -> Result<(), EncodeError>,
) -> Result<(), EncodeError> {
    match value {
        Some(value) => {
            write_u8(out, 1);
            encode(out, value)?;
        }
        None => write_u8(out, 0),
    }
    Ok(())
}

fn lookup_string_id_checked(strings: &[String], s: &str) -> Result<u32, EncodeError> {
    strings
        .iter()
        .position(|item| item == s)
        .map(|idx| idx as u32)
        .ok_or_else(|| EncodeError(format!("asset_id not interned in IR strings: {s}")))
}

fn encode_f32_slice(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for value in values {
        write_f32(&mut out, *value);
    }
    out
}

fn encode_ranges(ranges: &[TableRange]) -> Vec<u8> {
    let mut out = Vec::with_capacity(ranges.len() * 8);
    for range in ranges {
        write_u32(&mut out, range.start);
        write_u32(&mut out, range.len);
    }
    out
}

fn write_color_vec(out: &mut Vec<u8>, colors: &[[f32; 4]]) {
    write_u32(out, colors.len() as u32);
    for color in colors {
        write_f32_array(out, color);
    }
}

fn write_f32_vec(out: &mut Vec<u8>, values: &[f32]) {
    write_u32(out, values.len() as u32);
    for value in values {
        write_f32(out, *value);
    }
}

fn write_f32_array<const N: usize>(out: &mut Vec<u8>, values: &[f32; N]) {
    for value in values {
        write_f32(out, *value);
    }
}

fn write_bytes_with_len(out: &mut Vec<u8>, bytes: &[u8]) {
    write_u32(out, bytes.len() as u32);
    out.extend_from_slice(bytes);
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn align4(value: u32) -> u32 {
    (value + 3) & !3
}

pub(crate) fn encode_paint_style(style: PaintStyle) -> u8 {
    match style {
        PaintStyle::Fill => 0,
        PaintStyle::Stroke => 1,
    }
}

pub(crate) fn encode_stroke_cap(cap: StrokeCap) -> u8 {
    match cap {
        StrokeCap::Butt => 0,
        StrokeCap::Round => 1,
        StrokeCap::Square => 2,
    }
}

pub(crate) fn encode_stroke_join(join: StrokeJoin) -> u8 {
    match join {
        StrokeJoin::Miter => 0,
        StrokeJoin::Round => 1,
        StrokeJoin::Bevel => 2,
    }
}

pub(crate) fn encode_blend_mode(mode: BlendMode) -> u8 {
    match mode {
        BlendMode::Clear => 0,
        BlendMode::Src => 1,
        BlendMode::Dst => 2,
        BlendMode::SrcOver => 3,
        BlendMode::DstOver => 4,
        BlendMode::SrcIn => 5,
        BlendMode::DstIn => 6,
        BlendMode::SrcOut => 7,
        BlendMode::DstOut => 8,
        BlendMode::SrcATop => 9,
        BlendMode::DstATop => 10,
        BlendMode::Xor => 11,
        BlendMode::Plus => 12,
        BlendMode::Modulate => 13,
        BlendMode::Screen => 14,
        BlendMode::Overlay => 15,
        BlendMode::Darken => 16,
        BlendMode::Lighten => 17,
        BlendMode::ColorDodge => 18,
        BlendMode::ColorBurn => 19,
        BlendMode::HardLight => 20,
        BlendMode::SoftLight => 21,
        BlendMode::Difference => 22,
        BlendMode::Exclusion => 23,
        BlendMode::Multiply => 24,
        BlendMode::Hue => 25,
        BlendMode::Saturation => 26,
        BlendMode::Color => 27,
        BlendMode::Luminosity => 28,
    }
}

pub(crate) fn encode_tile_mode(mode: TileMode) -> u8 {
    match mode {
        TileMode::Clamp => 0,
        TileMode::Repeat => 1,
        TileMode::Mirror => 2,
        TileMode::Decal => 3,
    }
}

pub(crate) fn encode_blur_style(style: BlurStyle) -> u8 {
    match style {
        BlurStyle::Normal => 0,
        BlurStyle::Inner => 1,
        BlurStyle::Solid => 2,
        BlurStyle::Outer => 3,
    }
}

pub(crate) fn encode_path_fill_type(fill_type: FillType) -> u8 {
    match fill_type {
        FillType::Winding => 0,
        FillType::EvenOdd => 1,
    }
}

// ---------------------------------------------------------------------------
// Per-op encoder
// ---------------------------------------------------------------------------

/// Write a single DrawOp into the binary ops buffer with header format:
/// [opcode: u16 LE] [flags: u16 LE] [payload_len: u32 LE] [payload...]
/// v6: each op is `[opcode: u8][fixed-length payload]` with no padding.
fn encode_op(op: &DrawOp, buf: &mut Vec<u8>, _f32_pool: &mut Vec<f32>, strings: &[String]) {
    match op {
        // ===================================================================
        // Stack management — zero payload
        // ===================================================================
        DrawOp::Save => write_op_header(buf, opcode::SAVE),

        // ===================================================================
        // SaveLayer — fixed 25-byte payload
        //   [u8 flags] [4xf32 bounds] [u32 paint_id] [f32 alpha]
        //   flags: bit 0 = has_bounds, bit 1 = has_paint, bit 2 = backdrop
        // ===================================================================
        DrawOp::SaveLayer {
            bounds,
            paint,
            alpha,
            backdrop,
        } => {
            write_op_header(buf, opcode::SAVE_LAYER);
            let mut flags: u8 = 0;
            if bounds.is_some() {
                flags |= 0b01;
            }
            if paint.is_some() {
                flags |= 0b10;
            }
            if *backdrop {
                flags |= 0b100;
            }
            write_u8(buf, flags);
            if let Some(b) = bounds {
                write_rect4(buf, *b);
            } else {
                // Write zeroed bounds
                write_f32(buf, 0.0);
                write_f32(buf, 0.0);
                write_f32(buf, 0.0);
                write_f32(buf, 0.0);
            }
            write_u32(buf, paint.map(|p| p.0).unwrap_or(0));
            write_f32(buf, *alpha);
        }

        // ===================================================================
        // SaveBackdropLayer — fixed 41-byte payload: the SAVE_LAYER layout
        // (alpha always 1.0) followed by the element's border-box corner
        // radii (tl, tr, br, bl) — CSS clips the filtered backdrop to the
        // rounded border box.
        // ===================================================================
        DrawOp::SaveBackdropLayer { bounds, radius, paint } => {
            write_op_header(buf, opcode::SAVE_BACKDROP_LAYER);
            let mut flags: u8 = 0;
            if bounds.is_some() {
                flags |= 0b01;
            }
            if paint.is_some() {
                flags |= 0b10;
            }
            write_u8(buf, flags);
            if let Some(b) = bounds {
                write_rect4(buf, *b);
            } else {
                // Write zeroed bounds
                write_f32(buf, 0.0);
                write_f32(buf, 0.0);
                write_f32(buf, 0.0);
                write_f32(buf, 0.0);
            }
            write_u32(buf, paint.map(|p| p.0).unwrap_or(0));
            write_f32(buf, 1.0);
            for r in radius {
                write_f32(buf, *r);
            }
        }

        // ===================================================================
        // Stack management — zero payload
        // ===================================================================
        DrawOp::Restore => write_op_header(buf, opcode::RESTORE),

        // ===================================================================
        // RestoreToCount { count: i32 } — 4 byte payload
        // ===================================================================
        DrawOp::RestoreToCount { count } => {
            write_op_header(buf, opcode::RESTORE_TO_COUNT);
            write_u32(buf, *count as u32);
        }

        // ===================================================================
        // Transforms
        // ===================================================================
        DrawOp::Translate { x, y } => {
            write_op_header(buf, opcode::TRANSLATE);
            write_f32(buf, *x);
            write_f32(buf, *y);
        }

        DrawOp::Scale { x, y } => {
            write_op_header(buf, opcode::SCALE);
            write_f32(buf, *x);
            write_f32(buf, *y);
        }

        DrawOp::Rotate { degrees, cx, cy } => {
            write_op_header(buf, opcode::ROTATE);
            write_f32(buf, *degrees);
            write_f32(buf, *cx);
            write_f32(buf, *cy);
        }

        DrawOp::Skew { sx, sy } => {
            write_op_header(buf, opcode::SKEW);
            write_f32(buf, *sx);
            write_f32(buf, *sy);
        }

        DrawOp::Concat { matrix } => {
            write_op_header(buf, opcode::CONCAT); // 9 x f32
            for &v in matrix.iter() {
                write_f32(buf, v);
            }
        }

        // ===================================================================
        // Paint state setters
        // ===================================================================
        DrawOp::SetFillStyle { color } => {
            write_op_header(buf, opcode::SET_FILL_STYLE);
            write_u32(buf, encode_color_u8(*color));
        }

        DrawOp::SetStrokeStyle { color } => {
            write_op_header(buf, opcode::SET_STROKE_STYLE);
            write_u32(buf, encode_color_u8(*color));
        }

        DrawOp::SetLineWidth { width } => {
            write_op_header(buf, opcode::SET_LINE_WIDTH);
            write_f32(buf, *width);
        }

        DrawOp::SetLineCap { cap } => {
            write_op_header(buf, opcode::SET_LINE_CAP);
            write_u32(buf, encode_line_cap(*cap));
        }

        DrawOp::SetLineJoin { join } => {
            write_op_header(buf, opcode::SET_LINE_JOIN);
            write_u32(buf, encode_line_join(*join));
        }

        DrawOp::SetLineDash { intervals, phase } => {
            write_op_header(buf, opcode::SET_LINE_DASH); // u32 start + u32 len + f32 phase
            write_u32(buf, intervals.start);
            write_u32(buf, intervals.len);
            write_f32(buf, *phase);
        }

        DrawOp::ClearLineDash => write_op_header(buf, opcode::CLEAR_LINE_DASH),

        DrawOp::SetGlobalAlpha { alpha } => {
            write_op_header(buf, opcode::SET_GLOBAL_ALPHA);
            write_f32(buf, *alpha);
        }

        DrawOp::SetAntiAlias { enabled } => {
            write_op_header(buf, opcode::SET_ANTI_ALIAS);
            write_u8(buf, if *enabled { 1 } else { 0 });
        }

        // ===================================================================
        // Path construction
        // ===================================================================
        DrawOp::BeginPath => write_op_header(buf, opcode::BEGIN_PATH),

        DrawOp::Path(path_op) => encode_path_op(buf, path_op),

        DrawOp::FillPath => write_op_header(buf, opcode::FILL_PATH),

        DrawOp::StrokePath => write_op_header(buf, opcode::STROKE_PATH),

        DrawOp::ClipPath { anti_alias, fill_type } => {
            write_op_header(buf, opcode::CLIP_PATH);
            write_u8(buf, if *anti_alias { 1 } else { 0 });
            write_u8(buf, *fill_type as u8);
        }

        // ===================================================================
        // Drawing — immediate-mode primitives
        // ===================================================================
        DrawOp::Clear { color } => {
            write_op_header(buf, opcode::CLEAR); // ColorF32: 4 x f32
            write_f32(buf, color.r);
            write_f32(buf, color.g);
            write_f32(buf, color.b);
            write_f32(buf, color.a);
        }

        DrawOp::Paint { paint } => {
            write_op_header(buf, opcode::PAINT);
            write_u32(buf, paint.0);
        }

        DrawOp::Rect { rect, paint } => {
            write_op_header(buf, opcode::RECT); // 4xf32 + u32
            write_rect4(buf, *rect);
            write_u32(buf, paint.0);
        }

        DrawOp::RRect { rect, radii, paint } => {
            write_op_header(buf, opcode::R_RECT); // 4xf32 + 4xf32 + u32
            write_rect4(buf, *rect);
            write_radii4(buf, *radii);
            write_u32(buf, paint.0);
        }

        DrawOp::DRRect {
            outer,
            inner,
            paint,
        } => {
            write_op_header(buf, opcode::D_RRECT); // (4+4)xf32 outer + (4+4)xf32 inner + u32
            write_drrect_spec(buf, *outer);
            write_drrect_spec(buf, *inner);
            write_u32(buf, paint.0);
        }

        DrawOp::Oval { rect, paint } => {
            write_op_header(buf, opcode::OVAL); // 4xf32 + u32
            write_rect4(buf, *rect);
            write_u32(buf, paint.0);
        }

        DrawOp::Circle {
            cx,
            cy,
            radius,
            paint,
        } => {
            write_op_header(buf, opcode::CIRCLE); // 3xf32 + u32
            write_f32(buf, *cx);
            write_f32(buf, *cy);
            write_f32(buf, *radius);
            write_u32(buf, paint.0);
        }

        DrawOp::Arc {
            rect,
            start,
            sweep,
            use_center,
            paint,
        } => {
            // Payload: 4xf32(rect) + 2xf32(start,sweep) + 1u8(use_center) + 4u32(paint) = 29
            write_op_header(buf, opcode::ARC);
            write_rect4(buf, *rect);
            write_f32(buf, *start);
            write_f32(buf, *sweep);
            write_u8(buf, if *use_center { 1 } else { 0 });
            write_u32(buf, paint.0);
        }

        DrawOp::Line {
            x0,
            y0,
            x1,
            y1,
            paint,
        } => {
            write_op_header(buf, opcode::LINE); // 4xf32 + u32
            write_f32(buf, *x0);
            write_f32(buf, *y0);
            write_f32(buf, *x1);
            write_f32(buf, *y1);
            write_u32(buf, paint.0);
        }

        DrawOp::Points {
            mode,
            points,
            paint,
        } => {
            write_op_header(buf, opcode::POINTS); // u32 mode + 2xu32 range + u32 paint
            write_u32(buf, encode_point_mode(*mode));
            write_u32(buf, points.start);
            write_u32(buf, points.len);
            write_u32(buf, paint.0);
        }

        DrawOp::DrawPath { path, paint } => {
            write_op_header(buf, opcode::DRAW_PATH); // 2 x u32
            write_u32(buf, path.0);
            write_u32(buf, paint.0);
        }

        // ===================================================================
        // Image — tag(1) + id-slot(12) + x(4) + y(4) + paint_id(4)
        //   id-slot layout depends on the tag:
        //     Static    -> string_id(4) + time_micros=0(8)
        //     VideoFrame -> string_id(4) + time_micros(8)
        //     Generated  -> generated_id(8) + reserved(4)   [resolves RGBA via delta]
        // ===================================================================
        DrawOp::Image { image, x, y, paint } => {
            // Payload: 1 + 12 + 4 + 4 + 4 = 25
            write_op_header(buf, opcode::IMAGE);
            match image {
                ImageRef::Static { asset_id } => {
                    write_u8(buf, 0); // tag: Static
                    write_u32(buf, lookup_string_id(strings, asset_id));
                    write_u64(buf, 0); // time_micros = 0
                }
                ImageRef::VideoFrame {
                    asset_id,
                    time_micros,
                } => {
                    write_u8(buf, 1); // tag: VideoFrame
                    write_u32(buf, lookup_string_id(strings, asset_id));
                    write_u64(buf, *time_micros);
                }
                ImageRef::Generated { id } => {
                    // tag 2 reserved for Generated; RGBA is published separately
                    // via the generated-image delta (see issue #10). The JS
                    // decoder resolves the image from its id.
                    write_u8(buf, 2); // tag: Generated
                    write_u64(buf, id.0); // generated id
                    write_u32(buf, 0); // reserved (pad id-slot to 12 bytes)
                }
                ImageRef::Encoded { data } => {
                    // tag 3: bytes-range id into the frame byte table; the
                    // consumer owns decoding.
                    write_u8(buf, 3);
                    write_u32(buf, data.0);
                    write_u64(buf, 0); // pad id-slot to 12 bytes
                }
            }
            write_f32(buf, *x);
            write_f32(buf, *y);
            write_u32(buf, paint.map(|p| p.0).unwrap_or(0xFFFF_FFFF));
        }

        // ===================================================================
        // ImageRect — tag(1) + id-slot(12) + has_src(1) + src(16) + dst(16) + paint_id(4)
        //   id-slot layout: see DrawOp::Image (Static/VideoFrame/Generated).
        // ===================================================================
        DrawOp::ImageRect {
            image,
            src,
            dst,
            paint,
        } => {
            // Payload: 1 + 12 + 1 + 16 + 16 + 4 = 50
            write_op_header(buf, opcode::IMAGE_RECT);
            match image {
                ImageRef::Static { asset_id } => {
                    write_u8(buf, 0); // tag: Static
                    write_u32(buf, lookup_string_id(strings, asset_id));
                    write_u64(buf, 0); // time_micros = 0
                }
                ImageRef::VideoFrame {
                    asset_id,
                    time_micros,
                } => {
                    write_u8(buf, 1); // tag: VideoFrame
                    write_u32(buf, lookup_string_id(strings, asset_id));
                    write_u64(buf, *time_micros);
                }
                ImageRef::Generated { id } => {
                    write_u8(buf, 2); // tag: Generated
                    write_u64(buf, id.0);
                    write_u32(buf, 0); // reserved
                }
                ImageRef::Encoded { data } => {
                    write_u8(buf, 3); // tag: Encoded
                    write_u32(buf, data.0);
                    write_u64(buf, 0); // pad id-slot to 12 bytes
                }
            }
            write_u8(buf, if src.is_some() { 1 } else { 0 });
            if let Some(s) = src {
                write_rect4(buf, *s);
            } else {
                write_f32(buf, 0.0);
                write_f32(buf, 0.0);
                write_f32(buf, 0.0);
                write_f32(buf, 0.0);
            }
            write_rect4(buf, *dst);
            write_u32(buf, paint.map(|p| p.0).unwrap_or(0xFFFF_FFFF));
        }

        DrawOp::LottieRect {
            bundle_id,
            frame,
            dst,
        } => {
            write_op_header(buf, opcode::LOTTIE_RECT);
            write_u32(buf, lookup_string_id(strings, bundle_id));
            write_f32(buf, *frame);
            write_rect4(buf, *dst);
        }

        // ===================================================================
        // RuntimeEffect — u32 effect + u32 uniforms + 2xu32 children + 4xf32 dst = 32
        // ===================================================================
        DrawOp::RuntimeEffect {
            effect,
            uniforms,
            children,
            dst,
        } => {
            write_op_header(buf, opcode::RUNTIME_EFFECT);
            write_u32(buf, effect.0);
            write_u32(buf, uniforms.0);
            write_u32(buf, children.start);
            write_u32(buf, children.len);
            write_rect4(buf, *dst);
        }

        // ===================================================================
        // ReplayRange — 2 x u32 = 8
        // ===================================================================
        DrawOp::ReplayRange { range } => {
            write_op_header(buf, opcode::REPLAY_RANGE);
            write_u32(buf, range.start_op);
            write_u32(buf, range.op_len);
        }

        DrawOp::ReplaySubtreePicture { subtree, x, y } => {
            write_op_header(buf, opcode::DRAW_SUBTREE_PICTURE);
            write_u32(buf, subtree.0);
            write_f32(buf, *x);
            write_f32(buf, *y);
        }

        DrawOp::FillPattern {
            path,
            subtree,
            tile,
            pattern_transform,
            opacity,
        } => {
            write_op_header(buf, opcode::FILL_PATTERN);
            write_u32(buf, path.0);
            write_u32(buf, subtree.0);
            for v in [tile.x, tile.y, tile.width, tile.height] {
                write_f32(buf, v);
            }
            for v in pattern_transform {
                write_f32(buf, *v);
            }
            write_f32(buf, *opacity);
        }

        DrawOp::StrokePattern {
            path,
            subtree,
            tile,
            pattern_transform,
            opacity,
            stroke,
        } => {
            write_op_header(buf, opcode::STROKE_PATTERN);
            write_u32(buf, path.0);
            write_u32(buf, subtree.0);
            for v in [tile.x, tile.y, tile.width, tile.height] {
                write_f32(buf, v);
            }
            for v in pattern_transform {
                write_f32(buf, *v);
            }
            write_f32(buf, *opacity);
            write_f32(buf, stroke.width);
            write_u8(buf, stroke.cap as u8);
            write_u8(buf, stroke.join as u8);
            write_f32(buf, stroke.miter_limit);
        }

        DrawOp::MaskClip {
            subtree,
            kind,
            region,
        } => {
            write_op_header(buf, opcode::MASK_CLIP);
            write_u32(buf, subtree.0);
            write_u8(buf, *kind);
            for v in [region.x, region.y, region.width, region.height] {
                write_f32(buf, v);
            }
        }

        DrawOp::DrawSubtreePicture { .. } => {
            unreachable!(
                "DrawOp::DrawSubtreePicture must be translated into ReplaySubtreePicture \
                 before binary encoding"
            );
        }

        DrawOp::ScriptRuntimeEffect { .. } => {
            unreachable!(
                "DrawOp::ScriptRuntimeEffect must be translated into RuntimeEffect \
                 by execute_draw_op before binary encoding"
            );
        }
    }
    // v6: no trailing pad — the next op starts immediately after this op's
    // fixed-length payload.
}

// ---------------------------------------------------------------------------
// PathOp sub-encoder
// ---------------------------------------------------------------------------

/// Encode a PathOp as a top-level v6 op.
///
/// v6 promotes each path command to its own opcode (`PATH_MOVE_TO_OP` …), so the
/// wire layout is the same as any other op — `[opcode: u8][f32 payload]` — with
/// no embedded u16 sub-opcode. The section-8 PATHS table still uses the u16
/// sub-opcode form via [`encode_section_path_op`]; only the op stream changes.
fn encode_path_op(buf: &mut Vec<u8>, path_op: &PathOp) {
    match path_op {
        PathOp::MoveTo { x, y } => {
            write_op_header(buf, opcode::PATH_MOVE_TO_OP);
            write_f32(buf, *x);
            write_f32(buf, *y);
        }
        PathOp::LineTo { x, y } => {
            write_op_header(buf, opcode::PATH_LINE_TO_OP);
            write_f32(buf, *x);
            write_f32(buf, *y);
        }
        PathOp::QuadTo { cx, cy, x, y } => {
            write_op_header(buf, opcode::PATH_QUAD_TO_OP);
            write_f32(buf, *cx);
            write_f32(buf, *cy);
            write_f32(buf, *x);
            write_f32(buf, *y);
        }
        PathOp::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y,
        } => {
            write_op_header(buf, opcode::PATH_CUBIC_TO_OP);
            write_f32(buf, *c1x);
            write_f32(buf, *c1y);
            write_f32(buf, *c2x);
            write_f32(buf, *c2y);
            write_f32(buf, *x);
            write_f32(buf, *y);
        }
        PathOp::Close => {
            write_op_header(buf, opcode::PATH_CLOSE_OP);
        }
        PathOp::AddRect {
            x,
            y,
            width,
            height,
        } => {
            write_op_header(buf, opcode::PATH_ADD_RECT_OP);
            write_f32(buf, *x);
            write_f32(buf, *y);
            write_f32(buf, *width);
            write_f32(buf, *height);
        }
        PathOp::AddRRect {
            x,
            y,
            width,
            height,
            radius,
        } => {
            write_op_header(buf, opcode::PATH_ADD_RRECT_OP);
            write_f32(buf, *x);
            write_f32(buf, *y);
            write_f32(buf, *width);
            write_f32(buf, *height);
            write_f32(buf, *radius);
        }
        PathOp::AddOval {
            x,
            y,
            width,
            height,
        } => {
            write_op_header(buf, opcode::PATH_ADD_OVAL_OP);
            write_f32(buf, *x);
            write_f32(buf, *y);
            write_f32(buf, *width);
            write_f32(buf, *height);
        }
        PathOp::AddArc {
            x,
            y,
            width,
            height,
            start_angle,
            sweep_angle,
        } => {
            write_op_header(buf, opcode::PATH_ADD_ARC_OP);
            write_f32(buf, *x);
            write_f32(buf, *y);
            write_f32(buf, *width);
            write_f32(buf, *height);
            write_f32(buf, *start_angle);
            write_f32(buf, *sweep_angle);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


