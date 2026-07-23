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
// Ops layout (section 1):
//   [opcode: u16 LE] [flags: u16 LE] [payload_len: u32 LE] [payload...]
//   each op padded to 4-byte alignment.
//
// WASM transport only copies these bytes to JS — it does not re-encode
// protocol semantics. TypeScript decodeFrame must match this schema field-for-field.
// ---------------------------------------------------------------------------

use super::draw_op::*;
use super::draw_types::*;
use super::media_plan::FrameGeneratedImage;
use crate::canvas::paint::{
    BlendMode, BlurStyle, ColorFilterSpec, FillSpec, ImageFilterSpec, MaskFilterSpec, PaintSpec,
    PaintStyle, PathEffectSpec, ShaderSpec as PaintShaderSpec, StrokeCap, StrokeJoin, TileMode,
};
use crate::ir::RenderFrame;

// ---------------------------------------------------------------------------
// Magic, version, sections
// ---------------------------------------------------------------------------

/// Magic bytes for the unified DrawOp wire envelope.
pub const IR_MAGIC: [u8; 4] = *b"OCIR";

/// Wire protocol version.
///
/// v5 (issue #45): No pipeline_epoch in the header. OCIR is a pure, self-
/// contained encoding of RenderFrame: encode(RenderFrame) is a pure function,
/// requires no epoch/delta/history state, and a fresh decoder can independently
/// decode any single frame. Generated-image RGBA is fully encoded every frame
/// in SECTION_GENERATED_IMAGES (12). Section 12 is always present (count may be 0).
///
/// v4: pipeline_epoch in the header + SECTION_GENERATED_IMAGES (12) for the
/// per-frame generated-image delta.
pub const IR_VERSION: u32 = 5;

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

    /// Number of f32 values for each PathOp sub-opcode by its stored kind.
    /// Indexed by PATH_* value (0=MoveTo … 8=AddArc).
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
            ImageRef::Generated { .. } => {}
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

/// Write the 8-byte op header: opcode (u16), flags (u16), payload_len (u32).
#[inline]
fn write_op_header(buf: &mut Vec<u8>, op: u16, payload_len: u32) {
    buf.extend_from_slice(&op.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // flags (reserved)
    buf.extend_from_slice(&payload_len.to_le_bytes());
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
        &sections,
    )
}

/// Pack pre-encoded op/string sections with paints/paths/… into OCIR bytes.
pub fn pack_ir_envelope(
    draw: &super::draw_frame::DrawOpFrame,
    generated_images: &[FrameGeneratedImage],
    encoded: &EncodedDrawSections,
) -> Result<Vec<u8>, EncodeError> {
    let generated_images_section = encode_generated_images(generated_images);
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
        } => {
            write_u8(out, 0);
            write_u8(out, encode_tile_mode(*tile_mode));
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
        } => {
            write_u8(out, 1);
            write_u8(out, encode_tile_mode(*tile_mode));
            write_f32_array(out, center);
            write_f32(out, *radius);
            write_f32_vec(out, stops);
            write_color_vec(out, colors);
            encode_optional_matrix(out, local_matrix);
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
        } => {
            write_u8(out, 0);
            write_f32(out, *sigma_x);
            write_f32(out, *sigma_y);
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
        } => {
            write_u8(out, 1);
            write_f32(out, *dx);
            write_f32(out, *dy);
            write_f32(out, *sigma_x);
            write_f32(out, *sigma_y);
            write_f32_array(out, color);
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
/// Each op is padded to 4-byte alignment.
fn encode_op(op: &DrawOp, buf: &mut Vec<u8>, _f32_pool: &mut Vec<f32>, strings: &[String]) {
    match op {
        // ===================================================================
        // Stack management — zero payload
        // ===================================================================
        DrawOp::Save => write_op_header(buf, opcode::SAVE, 0),

        // ===================================================================
        // SaveLayer — fixed 25-byte payload
        //   [u8 flags] [4xf32 bounds] [u32 paint_id] [f32 alpha]
        //   flags: bit 0 = has_bounds, bit 1 = has_paint
        // ===================================================================
        DrawOp::SaveLayer {
            bounds,
            paint,
            alpha,
        } => {
            write_op_header(buf, opcode::SAVE_LAYER, 25);
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
            write_f32(buf, *alpha);
        }

        // ===================================================================
        // Stack management — zero payload
        // ===================================================================
        DrawOp::Restore => write_op_header(buf, opcode::RESTORE, 0),

        // ===================================================================
        // RestoreToCount { count: i32 } — 4 byte payload
        // ===================================================================
        DrawOp::RestoreToCount { count } => {
            write_op_header(buf, opcode::RESTORE_TO_COUNT, 4);
            write_u32(buf, *count as u32);
        }

        // ===================================================================
        // Transforms
        // ===================================================================
        DrawOp::Translate { x, y } => {
            write_op_header(buf, opcode::TRANSLATE, 8);
            write_f32(buf, *x);
            write_f32(buf, *y);
        }

        DrawOp::Scale { x, y } => {
            write_op_header(buf, opcode::SCALE, 8);
            write_f32(buf, *x);
            write_f32(buf, *y);
        }

        DrawOp::Rotate { degrees, cx, cy } => {
            write_op_header(buf, opcode::ROTATE, 12);
            write_f32(buf, *degrees);
            write_f32(buf, *cx);
            write_f32(buf, *cy);
        }

        DrawOp::Skew { sx, sy } => {
            write_op_header(buf, opcode::SKEW, 8);
            write_f32(buf, *sx);
            write_f32(buf, *sy);
        }

        DrawOp::Concat { matrix } => {
            write_op_header(buf, opcode::CONCAT, 36); // 9 x f32
            for &v in matrix.iter() {
                write_f32(buf, v);
            }
        }

        // ===================================================================
        // Paint state setters
        // ===================================================================
        DrawOp::SetFillStyle { color } => {
            write_op_header(buf, opcode::SET_FILL_STYLE, 4);
            write_u32(buf, encode_color_u8(*color));
        }

        DrawOp::SetStrokeStyle { color } => {
            write_op_header(buf, opcode::SET_STROKE_STYLE, 4);
            write_u32(buf, encode_color_u8(*color));
        }

        DrawOp::SetLineWidth { width } => {
            write_op_header(buf, opcode::SET_LINE_WIDTH, 4);
            write_f32(buf, *width);
        }

        DrawOp::SetLineCap { cap } => {
            write_op_header(buf, opcode::SET_LINE_CAP, 4);
            write_u32(buf, encode_line_cap(*cap));
        }

        DrawOp::SetLineJoin { join } => {
            write_op_header(buf, opcode::SET_LINE_JOIN, 4);
            write_u32(buf, encode_line_join(*join));
        }

        DrawOp::SetLineDash { intervals, phase } => {
            write_op_header(buf, opcode::SET_LINE_DASH, 12); // u32 start + u32 len + f32 phase
            write_u32(buf, intervals.start);
            write_u32(buf, intervals.len);
            write_f32(buf, *phase);
        }

        DrawOp::ClearLineDash => write_op_header(buf, opcode::CLEAR_LINE_DASH, 0),

        DrawOp::SetGlobalAlpha { alpha } => {
            write_op_header(buf, opcode::SET_GLOBAL_ALPHA, 4);
            write_f32(buf, *alpha);
        }

        DrawOp::SetAntiAlias { enabled } => {
            write_op_header(buf, opcode::SET_ANTI_ALIAS, 1);
            write_u8(buf, if *enabled { 1 } else { 0 });
        }

        // ===================================================================
        // Path construction
        // ===================================================================
        DrawOp::BeginPath => write_op_header(buf, opcode::BEGIN_PATH, 0),

        DrawOp::Path(path_op) => encode_path_op(buf, path_op),

        DrawOp::FillPath => write_op_header(buf, opcode::FILL_PATH, 0),

        DrawOp::StrokePath => write_op_header(buf, opcode::STROKE_PATH, 0),

        DrawOp::ClipPath { anti_alias } => {
            write_op_header(buf, opcode::CLIP_PATH, 1);
            write_u8(buf, if *anti_alias { 1 } else { 0 });
        }

        // ===================================================================
        // Drawing — immediate-mode primitives
        // ===================================================================
        DrawOp::Clear { color } => {
            write_op_header(buf, opcode::CLEAR, 16); // ColorF32: 4 x f32
            write_f32(buf, color.r);
            write_f32(buf, color.g);
            write_f32(buf, color.b);
            write_f32(buf, color.a);
        }

        DrawOp::Paint { paint } => {
            write_op_header(buf, opcode::PAINT, 4);
            write_u32(buf, paint.0);
        }

        DrawOp::Rect { rect, paint } => {
            write_op_header(buf, opcode::RECT, 20); // 4xf32 + u32
            write_rect4(buf, *rect);
            write_u32(buf, paint.0);
        }

        DrawOp::RRect { rect, radii, paint } => {
            write_op_header(buf, opcode::R_RECT, 36); // 4xf32 + 4xf32 + u32
            write_rect4(buf, *rect);
            write_radii4(buf, *radii);
            write_u32(buf, paint.0);
        }

        DrawOp::DRRect {
            outer,
            inner,
            paint,
        } => {
            write_op_header(buf, opcode::D_RRECT, 68); // (4+4)xf32 outer + (4+4)xf32 inner + u32
            write_drrect_spec(buf, *outer);
            write_drrect_spec(buf, *inner);
            write_u32(buf, paint.0);
        }

        DrawOp::Oval { rect, paint } => {
            write_op_header(buf, opcode::OVAL, 20); // 4xf32 + u32
            write_rect4(buf, *rect);
            write_u32(buf, paint.0);
        }

        DrawOp::Circle {
            cx,
            cy,
            radius,
            paint,
        } => {
            write_op_header(buf, opcode::CIRCLE, 16); // 3xf32 + u32
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
            write_op_header(buf, opcode::ARC, 29);
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
            write_op_header(buf, opcode::LINE, 20); // 4xf32 + u32
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
            write_op_header(buf, opcode::POINTS, 16); // u32 mode + 2xu32 range + u32 paint
            write_u32(buf, encode_point_mode(*mode));
            write_u32(buf, points.start);
            write_u32(buf, points.len);
            write_u32(buf, paint.0);
        }

        DrawOp::DrawPath { path, paint } => {
            write_op_header(buf, opcode::DRAW_PATH, 8); // 2 x u32
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
            write_op_header(buf, opcode::IMAGE, 25);
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
            write_op_header(buf, opcode::IMAGE_RECT, 50);
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
            write_op_header(buf, opcode::LOTTIE_RECT, 4 + 4 + 16);
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
            write_op_header(buf, opcode::RUNTIME_EFFECT, 32);
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
            write_op_header(buf, opcode::REPLAY_RANGE, 8);
            write_u32(buf, range.start_op);
            write_u32(buf, range.op_len);
        }

        DrawOp::ReplaySubtreePicture { subtree, x, y } => {
            write_op_header(buf, opcode::DRAW_SUBTREE_PICTURE, 12);
            write_u32(buf, subtree.0);
            write_f32(buf, *x);
            write_f32(buf, *y);
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

    // Pad to 4-byte alignment after each op
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
}

// ---------------------------------------------------------------------------
// PathOp sub-encoder
// ---------------------------------------------------------------------------

/// Encode a PathOp as the payload of a PATH_OP DrawOp.
/// Layout: [sub_opcode: u16 LE] [sub_payload...]
fn encode_path_op(buf: &mut Vec<u8>, path_op: &PathOp) {
    match path_op {
        PathOp::MoveTo { x, y } => {
            write_op_header(buf, opcode::PATH_OP, 10); // u16 sub + 2xf32
            buf.extend_from_slice(&opcode::PATH_MOVE_TO.to_le_bytes());
            write_f32(buf, *x);
            write_f32(buf, *y);
        }
        PathOp::LineTo { x, y } => {
            write_op_header(buf, opcode::PATH_OP, 10);
            buf.extend_from_slice(&opcode::PATH_LINE_TO.to_le_bytes());
            write_f32(buf, *x);
            write_f32(buf, *y);
        }
        PathOp::QuadTo { cx, cy, x, y } => {
            write_op_header(buf, opcode::PATH_OP, 18); // u16 sub + 4xf32
            buf.extend_from_slice(&opcode::PATH_QUAD_TO.to_le_bytes());
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
            write_op_header(buf, opcode::PATH_OP, 26); // u16 sub + 6xf32
            buf.extend_from_slice(&opcode::PATH_CUBIC_TO.to_le_bytes());
            write_f32(buf, *c1x);
            write_f32(buf, *c1y);
            write_f32(buf, *c2x);
            write_f32(buf, *c2y);
            write_f32(buf, *x);
            write_f32(buf, *y);
        }
        PathOp::Close => {
            write_op_header(buf, opcode::PATH_OP, 2); // u16 sub
            buf.extend_from_slice(&opcode::PATH_CLOSE.to_le_bytes());
        }
        PathOp::AddRect {
            x,
            y,
            width,
            height,
        } => {
            write_op_header(buf, opcode::PATH_OP, 18); // u16 sub + 4xf32
            buf.extend_from_slice(&opcode::PATH_ADD_RECT.to_le_bytes());
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
            write_op_header(buf, opcode::PATH_OP, 22); // u16 sub + 5xf32
            buf.extend_from_slice(&opcode::PATH_ADD_RRECT.to_le_bytes());
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
            write_op_header(buf, opcode::PATH_OP, 18); // u16 sub + 4xf32
            buf.extend_from_slice(&opcode::PATH_ADD_OVAL.to_le_bytes());
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
            write_op_header(buf, opcode::PATH_OP, 26); // u16 sub + 6xf32
            buf.extend_from_slice(&opcode::PATH_ADD_ARC.to_le_bytes());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::draw_frame::DrawFrameScratch;
    use crate::ir::draw_frame::DrawOpFrame;
    use crate::ir::draw_op::{
        ColorF32, ColorU8, F32Range, LineCap, LineJoin, PointMode, Radii4, Rect4,
    };
    use crate::ir::draw_types::{
        ChildRange, DrawOpRange, EffectRef, EncodedPath, FillType, ImageRef, PaintId, PathOp,
    };
    use crate::ir::media_plan::FrameGeneratedImage;
    use crate::render::builder::DrawOpBuilder;

    /// Wrap a DrawOpFrame + generated images into a RenderFrame for encoding.
    fn to_render_frame(
        draw: DrawOpFrame,
        generated_images: Vec<FrameGeneratedImage>,
    ) -> RenderFrame {
        RenderFrame {
            draw,
            media: crate::ir::media_plan::FrameMediaPlan {
                generated_images,
                ..Default::default()
            },
        }
    }

    // -----------------------------------------------------------------------
    // Required tests from the task specification
    // -----------------------------------------------------------------------

    #[test]
    fn encode_empty_frame_produces_valid_header() {
        let frame = DrawOpFrame::default();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);
        assert!(encoded.ops.is_empty());
        let envelope = pack_ir_envelope(&frame, &[], &encoded).expect("envelope");
        assert_eq!(&envelope[0..4], b"OCIR");
        assert_eq!(
            u32::from_le_bytes(envelope[4..8].try_into().unwrap()),
            IR_VERSION
        );
    }

    #[test]
    fn encode_save_restore_roundtrips_op_count() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::Save);
        builder.push(DrawOp::Restore);
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);
        // Each op has a 8-byte header + payload, so ops buffer should be non-empty
        assert!(!encoded.ops.is_empty());
    }

    #[test]
    fn encoded_envelope_magic_is_constant() {
        let frame = to_render_frame(DrawOpFrame::default(), vec![]);
        let mut scratch = DrawFrameScratch::default();
        let a = encode_ir_envelope(&frame, &mut scratch).unwrap();
        scratch.clear();
        let b = encode_ir_envelope(&frame, &mut scratch).unwrap();
        assert_eq!(&a[0..4], b"OCIR");
        assert_eq!(&b[0..4], b"OCIR");
        assert_eq!(a, b);
    }

    #[test]
    fn encode_translate_produces_correct_payload_length() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::Translate { x: 10.0, y: 20.0 });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);
        // Op header: 2(u16 opcode) + 2(u16 flags) + 4(u32 payload_len) = 8 bytes
        // Translate payload: 2 x f32 = 8 bytes
        // Total = 16 bytes (padded to 4-byte alignment)
        assert_eq!(encoded.ops.len(), 16);
    }

    // -----------------------------------------------------------------------
    // Additional tests for all DrawOp variants
    // -----------------------------------------------------------------------

    #[test]
    fn encode_all_variants_produces_non_empty_ops() {
        let mut builder = DrawOpBuilder::default();

        // Stack management
        builder.push(DrawOp::Save);
        builder.push(DrawOp::Restore);
        builder.push(DrawOp::RestoreToCount { count: 5 });

        // SaveLayer
        builder.push(DrawOp::SaveLayer {
            bounds: Some(Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            }),
            paint: Some(PaintId(0)),
            alpha: 0.5,
        });

        // Transforms
        builder.push(DrawOp::Translate { x: 10.0, y: 20.0 });
        builder.push(DrawOp::Scale { x: 2.0, y: 2.0 });
        builder.push(DrawOp::Rotate {
            degrees: 45.0,
            cx: 0.0,
            cy: 0.0,
        });
        builder.push(DrawOp::Skew { sx: 0.5, sy: 0.0 });
        builder.push(DrawOp::Concat {
            matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        });

        // Paint state setters
        builder.push(DrawOp::SetFillStyle {
            color: ColorU8 {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
        });
        builder.push(DrawOp::SetStrokeStyle {
            color: ColorU8 {
                r: 0,
                g: 0,
                b: 255,
                a: 255,
            },
        });
        builder.push(DrawOp::SetLineWidth { width: 2.0 });
        builder.push(DrawOp::SetLineCap {
            cap: LineCap::Round,
        });
        builder.push(DrawOp::SetLineJoin {
            join: LineJoin::Bevel,
        });
        builder.push(DrawOp::SetLineDash {
            intervals: F32Range { start: 0, len: 4 },
            phase: 1.0,
        });
        builder.push(DrawOp::ClearLineDash);
        builder.push(DrawOp::SetGlobalAlpha { alpha: 0.8 });
        builder.push(DrawOp::SetAntiAlias { enabled: true });

        // Path construction
        builder.push(DrawOp::BeginPath);
        builder.push(DrawOp::Path(PathOp::MoveTo { x: 0.0, y: 0.0 }));
        builder.push(DrawOp::Path(PathOp::LineTo { x: 10.0, y: 10.0 }));
        builder.push(DrawOp::Path(PathOp::QuadTo {
            cx: 5.0,
            cy: 0.0,
            x: 10.0,
            y: 10.0,
        }));
        builder.push(DrawOp::Path(PathOp::CubicTo {
            c1x: 0.0,
            c1y: 5.0,
            c2x: 5.0,
            c2y: 10.0,
            x: 10.0,
            y: 10.0,
        }));
        builder.push(DrawOp::Path(PathOp::Close));
        builder.push(DrawOp::Path(PathOp::AddRect {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        }));
        builder.push(DrawOp::Path(PathOp::AddRRect {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
            radius: 5.0,
        }));
        builder.push(DrawOp::Path(PathOp::AddOval {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        }));
        builder.push(DrawOp::Path(PathOp::AddArc {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
            start_angle: 0.0,
            sweep_angle: 180.0,
        }));
        builder.push(DrawOp::FillPath);
        builder.push(DrawOp::StrokePath);
        builder.push(DrawOp::ClipPath { anti_alias: true });

        // Drawing primitives
        builder.push(DrawOp::Clear {
            color: ColorF32::TRANSPARENT,
        });
        builder.push(DrawOp::Paint { paint: PaintId(0) });
        builder.push(DrawOp::Rect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            paint: PaintId(0),
        });
        builder.push(DrawOp::RRect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            radii: Radii4 {
                top_left: 5.0,
                top_right: 5.0,
                bottom_right: 5.0,
                bottom_left: 5.0,
            },
            paint: PaintId(0),
        });
        builder.push(DrawOp::DRRect {
            outer: DRRectSpec {
                rect: Rect4 {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                radii: Radii4 {
                    top_left: 10.0,
                    top_right: 10.0,
                    bottom_right: 10.0,
                    bottom_left: 10.0,
                },
            },
            inner: DRRectSpec {
                rect: Rect4 {
                    x: 10.0,
                    y: 10.0,
                    width: 80.0,
                    height: 80.0,
                },
                radii: Radii4 {
                    top_left: 5.0,
                    top_right: 5.0,
                    bottom_right: 5.0,
                    bottom_left: 5.0,
                },
            },
            paint: PaintId(0),
        });
        builder.push(DrawOp::Oval {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            },
            paint: PaintId(0),
        });
        builder.push(DrawOp::Circle {
            cx: 50.0,
            cy: 50.0,
            radius: 25.0,
            paint: PaintId(0),
        });
        builder.push(DrawOp::Arc {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            start: 0.0,
            sweep: 90.0,
            use_center: true,
            paint: PaintId(0),
        });
        builder.push(DrawOp::Line {
            x0: 0.0,
            y0: 0.0,
            x1: 100.0,
            y1: 100.0,
            paint: PaintId(0),
        });
        builder.push(DrawOp::Points {
            mode: PointMode::Lines,
            points: F32Range { start: 0, len: 6 },
            paint: PaintId(0),
        });
        builder.push(DrawOp::DrawPath {
            path: PathId(0),
            paint: PaintId(0),
        });

        // Image (uses interned string)
        let _img_id = builder.intern_string("test.png");
        builder.push(DrawOp::Image {
            image: ImageRef::Static {
                asset_id: "test.png".to_string(),
            },
            x: 10.0,
            y: 20.0,
            paint: None,
        });
        builder.push(DrawOp::ImageRect {
            image: ImageRef::VideoFrame {
                asset_id: "clip.mp4".to_string(),
                time_micros: 1_400_000,
            },
            src: None,
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 640.0,
                height: 480.0,
            },
            paint: Some(PaintId(1)),
        });
        // Intern the video frame string too
        let _vid_id = builder.intern_string("clip.mp4");

        // RuntimeEffect placeholder
        builder.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 0 },
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        });

        // ReplayRange
        builder.push(DrawOp::ReplayRange {
            range: DrawOpRange {
                start_op: 0,
                op_len: 3,
            },
        });

        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // Should have encoded all ops
        assert!(!encoded.ops.is_empty());
        // Verify the ops buffer is 4-byte aligned
        assert_eq!(encoded.ops.len() % 4, 0);
    }

    #[test]
    fn encode_strings_produces_valid_ranges() {
        let mut builder = DrawOpBuilder::default();
        builder.intern_string("hello");
        builder.intern_string("world");
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // Two string ranges should be present
        assert_eq!(encoded.string_ranges.len(), 2);

        // First string "hello" = 5 bytes
        assert_eq!(encoded.string_ranges[0].start, 0);
        assert_eq!(encoded.string_ranges[0].len, 5);
        assert_eq!(&encoded.strings_utf8[0..5], b"hello");

        // Second string "world" = 5 bytes, starts at offset 5
        assert_eq!(encoded.string_ranges[1].start, 5);
        assert_eq!(encoded.string_ranges[1].len, 5);
        assert_eq!(&encoded.strings_utf8[5..10], b"world");
    }

    #[test]
    fn encode_clear_line_dash_zero_payload() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::ClearLineDash);
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // ClearLineDash: 8 byte header + 0 byte payload = 8 bytes
        assert_eq!(encoded.ops.len(), 8);
    }

    #[test]
    fn encode_set_anti_alias_padded_to_4_byte_alignment() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::SetAntiAlias { enabled: true });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // SetAntiAlias: 8 byte header + 1 byte payload, padded to 12 bytes
        assert_eq!(encoded.ops.len(), 12);
        assert_eq!(encoded.ops.len() % 4, 0);
    }

    #[test]
    fn encode_color_u8_packing_is_correct() {
        let color = ColorU8 {
            r: 0x12,
            g: 0x34,
            b: 0x56,
            a: 0x78,
        };
        let packed = encode_color_u8(color);
        // Little-endian: R|G<<8|B<<16|A<<24
        assert_eq!(packed, 0x78563412);

        let bytes = packed.to_le_bytes();
        assert_eq!(bytes[0], 0x12);
        assert_eq!(bytes[1], 0x34);
        assert_eq!(bytes[2], 0x56);
        assert_eq!(bytes[3], 0x78);
    }

    #[test]
    fn encode_line_cap_discriminants() {
        assert_eq!(encode_line_cap(LineCap::Butt), 0);
        assert_eq!(encode_line_cap(LineCap::Round), 1);
        assert_eq!(encode_line_cap(LineCap::Square), 2);
    }

    #[test]
    fn encode_line_join_discriminants() {
        assert_eq!(encode_line_join(LineJoin::Miter), 0);
        assert_eq!(encode_line_join(LineJoin::Round), 1);
        assert_eq!(encode_line_join(LineJoin::Bevel), 2);
    }

    #[test]
    fn encode_point_mode_discriminants() {
        assert_eq!(encode_point_mode(PointMode::Points), 0);
        assert_eq!(encode_point_mode(PointMode::Lines), 1);
        assert_eq!(encode_point_mode(PointMode::Polygon), 2);
    }

    #[test]
    fn scratch_reuse_across_frames_produces_consistent_results() {
        let mut scratch = DrawFrameScratch::default();

        // First frame: just Save
        let mut b1 = DrawOpBuilder::default();
        b1.push(DrawOp::Save);
        let f1 = b1.finish();
        let e1 = encode_draw_sections(&f1, &mut scratch);
        assert!(!e1.ops.is_empty());

        // Second frame: same op, reused scratch
        let mut b2 = DrawOpBuilder::default();
        b2.push(DrawOp::Save);
        let f2 = b2.finish();
        let e2 = encode_draw_sections(&f2, &mut scratch);
        assert!(!e2.ops.is_empty());

        // Both encodes should produce same ops content
        assert_eq!(e1.ops.len(), e2.ops.len());
    }

    #[test]
    fn encode_concat_matrix_is_36_bytes_payload() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::Concat {
            matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 20.0, 1.0],
        });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // 8 header + 36 payload = 44 bytes
        assert_eq!(encoded.ops.len(), 44);
    }

    #[test]
    fn encode_restore_to_count_is_4_bytes_payload() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::RestoreToCount { count: 3 });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // 8 header + 4 payload = 12 bytes
        assert_eq!(encoded.ops.len(), 12);
    }

    #[test]
    fn encode_save_layer_fixed_25_bytes() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::SaveLayer {
            bounds: None,
            paint: None,
            alpha: 1.0,
        });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // 8 header + 25 payload = 33, padded to 36
        assert_eq!(encoded.ops.len(), 36);
        assert_eq!(encoded.ops.len() % 4, 0);
    }

    #[test]
    fn encode_image_produces_correct_size() {
        let mut builder = DrawOpBuilder::default();
        builder.intern_string("test.png");
        builder.push(DrawOp::Image {
            image: ImageRef::Static {
                asset_id: "test.png".to_string(),
            },
            x: 10.0,
            y: 20.0,
            paint: None,
        });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // 8 header + 25 payload = 33, padded to 36
        assert_eq!(encoded.ops.len(), 36);
        assert_eq!(encoded.ops.len() % 4, 0);
    }

    #[test]
    fn encode_image_rect_produces_correct_size() {
        let mut builder = DrawOpBuilder::default();
        builder.intern_string("clip.mp4");
        builder.push(DrawOp::ImageRect {
            image: ImageRef::VideoFrame {
                asset_id: "clip.mp4".to_string(),
                time_micros: 233_333,
            },
            src: None,
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 640.0,
                height: 480.0,
            },
            paint: Some(PaintId(0)),
        });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // 8 header + 50 payload = 58, padded to 60 (4-byte alignment)
        assert_eq!(encoded.ops.len(), 60);
        assert_eq!(encoded.ops.len() % 4, 0);
    }

    #[test]
    fn encode_runtime_effect_is_32_bytes_payload() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 1 },
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // 8 header + 32 payload = 40 bytes (already aligned)
        assert_eq!(encoded.ops.len(), 40);
    }

    #[test]
    fn encode_replay_range_is_8_bytes_payload() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::ReplayRange {
            range: DrawOpRange {
                start_op: 0,
                op_len: 10,
            },
        });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // 8 header + 8 payload = 16 bytes
        assert_eq!(encoded.ops.len(), 16);
    }

    #[test]
    fn encode_f32_pool_preserves_set_line_dash_data() {
        // Verify that F32Range references in SetLineDash actually point to
        // valid data in the encoded f32_pool.
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::SetLineDash {
            intervals: F32Range { start: 0, len: 3 },
            phase: 2.0,
        });
        // Manually populate the frame's f32_pool with line dash intervals
        let mut frame = builder.finish();
        frame.f32_pool = vec![5.0, 3.0, 5.0]; // alternating dash pattern

        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_sections(&frame, &mut scratch);

        // The f32_pool should contain the dash intervals
        assert_eq!(encoded.f32_pool.len(), 3);
        assert_eq!(encoded.f32_pool[0], 5.0);
        assert_eq!(encoded.f32_pool[1], 3.0);
        assert_eq!(encoded.f32_pool[2], 5.0);
    }

    // -----------------------------------------------------------------------
    // Canonical self-contained OCIR envelope (issue #45)
    // -----------------------------------------------------------------------

    fn read_u32(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    fn section_payload(envelope: &[u8], want_id: u32) -> &[u8] {
        assert_eq!(&envelope[0..4], b"OCIR");
        assert_eq!(read_u32(envelope, 4), IR_VERSION);
        let section_count = read_u32(envelope, 8) as usize;
        for i in 0..section_count {
            let base = 12 + i * 12;
            let id = read_u32(envelope, base);
            let offset = read_u32(envelope, base + 4) as usize;
            let len = read_u32(envelope, base + 8) as usize;
            if id == want_id {
                return &envelope[offset..offset + len];
            }
        }
        panic!("missing section {want_id}");
    }

    #[test]
    fn envelope_header_has_no_pipeline_epoch() {
        // v5 header is only 12 bytes (magic + version + section_count).
        // No pipeline_epoch field exists — OCIR is self-contained.
        let frame = to_render_frame(DrawOpFrame::default(), vec![]);
        let mut scratch = DrawFrameScratch::default();
        let bytes = encode_ir_envelope(&frame, &mut scratch).unwrap();

        // Header: 4 magic + 4 version + 4 section_count = 12 bytes
        assert_eq!(&bytes[0..4], b"OCIR");
        assert_eq!(read_u32(&bytes, 4), IR_VERSION);
        // section_count at offset 8
        let section_count = read_u32(&bytes, 8);
        assert!(
            section_count > 0,
            "empty envelope still has required sections"
        );
        // Header is exactly 12 bytes; directory starts at 12
        let dir_start = 12;
        // First section entry at offset 12
        let first_id = read_u32(&bytes, dir_start);
        assert!(first_id >= 1 && first_id <= 12, "valid section id");
        let generated = section_payload(&bytes, section::GENERATED_IMAGES);
        assert_eq!(generated, &0u32.to_le_bytes());
    }

    #[test]
    fn generated_images_fully_encoded_every_frame() {
        use crate::ir::generated_image::GeneratedImageId;
        use std::sync::Arc;

        let render_frame = to_render_frame(
            DrawOpFrame::default(),
            vec![
                FrameGeneratedImage {
                    id: GeneratedImageId(0x0123_4567_89ab_cdef),
                    width: 3,
                    height: 2,
                    rgba: Arc::from(vec![0xAB; 3 * 2 * 4]),
                },
                FrameGeneratedImage {
                    id: GeneratedImageId(42),
                    width: 1,
                    height: 1,
                    rgba: Arc::from(vec![0x11; 4]),
                },
            ],
        );
        let mut scratch = DrawFrameScratch::default();
        let bytes = encode_ir_envelope(&render_frame, &mut scratch).unwrap();

        // No pipeline_epoch in header
        assert_eq!(&bytes[0..4], b"OCIR");
        assert_eq!(read_u32(&bytes, 4), IR_VERSION);

        let generated = section_payload(&bytes, section::GENERATED_IMAGES);
        assert_eq!(read_u32(generated, 0), 2);
        assert_eq!(
            u64::from_le_bytes(generated[4..12].try_into().unwrap()),
            0x0123_4567_89ab_cdef
        );
        assert_eq!(read_u32(generated, 12), 3);
        assert_eq!(read_u32(generated, 16), 2);
        assert_eq!(read_u32(generated, 20), 24);
        assert_eq!(&generated[24..48], &[0xAB; 24]);
    }

    #[test]
    fn paint_and_path_sections_round_trip_layout() {
        use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle};

        let mut frame = DrawOpFrame::default();
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([1.0, 0.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            stroke: None,
            anti_alias: true,
            blend_mode: BlendMode::SrcOver,
            image_filter: None,
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        });
        frame.paths.push(EncodedPath {
            fill_type: FillType::EvenOdd,
            ops: vec![PathOp::MoveTo { x: 1.0, y: 2.0 }, PathOp::Close],
        });
        let render_frame = to_render_frame(frame, vec![]);
        let mut scratch = DrawFrameScratch::default();
        let bytes = encode_ir_envelope(&render_frame, &mut scratch).unwrap();
        let paints = section_payload(&bytes, section::PAINTS);
        assert_eq!(read_u32(paints, 0), 1);
        let paths = section_payload(&bytes, section::PATHS);
        assert_eq!(read_u32(paths, 0), 1);
    }

    #[test]
    fn intern_image_strings_makes_image_ops_encodable() {
        let mut frame = DrawOpFrame::default();
        // Image ref without interning the asset id into frame.strings would panic
        // inside encode_op; intern_image_strings is the host/pre-encode step.
        frame.ops.push(DrawOp::Image {
            image: ImageRef::Static {
                asset_id: "missing.png".into(),
            },
            x: 0.0,
            y: 0.0,
            paint: None,
        });
        intern_image_strings(&mut frame);
        let render_frame = to_render_frame(frame, vec![]);
        let mut scratch = DrawFrameScratch::default();
        encode_ir_envelope(&render_frame, &mut scratch).expect("encodable after intern");
    }

    #[test]
    fn all_required_sections_present_in_empty_envelope() {
        let frame = to_render_frame(DrawOpFrame::default(), vec![]);
        let mut scratch = DrawFrameScratch::default();
        let bytes = encode_ir_envelope(&frame, &mut scratch).unwrap();
        for id in [
            section::OPS,
            section::F32_POOL,
            section::BYTES,
            section::BYTE_RANGES,
            section::STRINGS_UTF8,
            section::STRING_RANGES,
            section::PAINTS,
            section::PATHS,
            section::CHILDREN,
            section::EFFECTS,
            section::SUBTREES,
            section::GENERATED_IMAGES,
        ] {
            let _ = section_payload(&bytes, id);
        }
    }

    /// Build the fixed AC5 fixture frame: ops, paint, path, string, image ref,
    /// effect, and generated images. Used by the Rust snapshot and by the
    /// committed binary that TypeScript decodes field-for-field.
    fn roundtrip_fixture_render_frame() -> RenderFrame {
        use crate::canvas::paint::{
            BlendMode, FillSpec, PaintSpec, PaintStyle, StrokeCap, StrokeJoin, StrokeSpec,
        };
        use crate::ir::generated_image::GeneratedImageId;
        use std::sync::Arc;

        let mut draw = DrawOpFrame::default();
        draw.ops.push(DrawOp::Save);
        draw.ops.push(DrawOp::Translate { x: 10.0, y: 20.0 });
        draw.ops.push(DrawOp::Image {
            image: ImageRef::Static {
                asset_id: "hero.png".into(),
            },
            x: 1.0,
            y: 2.0,
            paint: Some(PaintId(0)),
        });
        draw.ops.push(DrawOp::Restore);
        draw.strings.push("hero.png".into());
        draw.paints.push(PaintSpec {
            fill: FillSpec::Solid([1.0, 0.25, 0.0, 1.0]),
            style: PaintStyle::Stroke,
            stroke: Some(StrokeSpec {
                width: 2.5,
                cap: StrokeCap::Round,
                join: StrokeJoin::Bevel,
                miter_limit: 4.0,
            }),
            anti_alias: true,
            blend_mode: BlendMode::SrcOver,
            image_filter: None,
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        });
        draw.paths.push(EncodedPath {
            fill_type: FillType::EvenOdd,
            ops: vec![
                PathOp::MoveTo { x: 1.0, y: 2.0 },
                PathOp::LineTo { x: 3.0, y: 4.0 },
                PathOp::Close,
            ],
        });
        draw.effects.push(EffectRef {
            hash: 0xdead_beef_cafe_u64,
            sksl: "half4 main() { return half4(1); }".into(),
        });
        to_render_frame(
            draw,
            vec![FrameGeneratedImage {
                id: GeneratedImageId(0x1111_2222_3333_4444),
                width: 2,
                height: 1,
                rgba: Arc::from([0x10u8, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80]),
            }],
        )
    }

    #[test]
    fn paint_path_string_effect_generated_fields_round_trip_in_core() {
        use crate::canvas::paint::{FillSpec, PaintStyle, StrokeCap, StrokeJoin};

        let mut render_frame = roundtrip_fixture_render_frame();
        intern_image_strings(&mut render_frame.draw);
        let mut scratch = DrawFrameScratch::default();
        let bytes = encode_ir_envelope(&render_frame, &mut scratch).unwrap();

        assert_eq!(&bytes[0..4], b"OCIR");
        assert_eq!(read_u32(&bytes, 4), IR_VERSION);

        // Strings: one range for "hero.png"
        let strings_utf8 = section_payload(&bytes, section::STRINGS_UTF8);
        let string_ranges = section_payload(&bytes, section::STRING_RANGES);
        assert_eq!(string_ranges.len(), 8);
        let start = read_u32(string_ranges, 0) as usize;
        let len = read_u32(string_ranges, 4) as usize;
        assert_eq!(&strings_utf8[start..start + len], b"hero.png");

        // Paints: count 1, solid fill R=1 G=0.25, stroke style
        let paints = section_payload(&bytes, section::PAINTS);
        assert_eq!(read_u32(paints, 0), 1);
        let rec_len = read_u32(paints, 4) as usize;
        let rec = &paints[8..8 + rec_len];
        assert_eq!(rec[0], 0); // solid
        let r = f32::from_le_bytes(rec[1..5].try_into().unwrap());
        let g = f32::from_le_bytes(rec[5..9].try_into().unwrap());
        assert!((r - 1.0).abs() < 1e-6);
        assert!((g - 0.25).abs() < 1e-6);
        // after 4xf32 color: style=Stroke(1), aa=1, blend=SrcOver(3), has_stroke=1
        let after_color = 1 + 16;
        assert_eq!(rec[after_color], 1); // Stroke
        assert_eq!(rec[after_color + 1], 1); // aa
        assert_eq!(rec[after_color + 2], 3); // SrcOver
        assert_eq!(rec[after_color + 3], 1); // has stroke

        // Paths: EvenOdd + 3 ops
        let paths = section_payload(&bytes, section::PATHS);
        assert_eq!(read_u32(paths, 0), 1);
        let path_rec_len = read_u32(paths, 4) as usize;
        let path_rec = &paths[8..8 + path_rec_len];
        assert_eq!(path_rec[0], 1); // EvenOdd
        assert_eq!(read_u32(path_rec, 1), 3);

        // Effects: one sksl string
        let effects = section_payload(&bytes, section::EFFECTS);
        assert_eq!(read_u32(effects, 0), 1);
        let hash = u64::from_le_bytes(effects[4..12].try_into().unwrap());
        assert_eq!(hash, 0xdead_beef_cafe_u64);
        let sksl_len = read_u32(effects, 12) as usize;
        assert_eq!(
            &effects[16..16 + sksl_len],
            b"half4 main() { return half4(1); }"
        );

        // Generated images
        let generated = section_payload(&bytes, section::GENERATED_IMAGES);
        assert_eq!(read_u32(generated, 0), 1);
        assert_eq!(
            u64::from_le_bytes(generated[4..12].try_into().unwrap()),
            0x1111_2222_3333_4444
        );
        assert_eq!(read_u32(generated, 12), 2);
        assert_eq!(read_u32(generated, 16), 1);
        assert_eq!(read_u32(generated, 20), 8);
        assert_eq!(
            &generated[24..32],
            &[0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80]
        );

        // Ops stream non-empty (Save/Translate/Image/Restore)
        assert!(!section_payload(&bytes, section::OPS).is_empty());
        let _ = (
            FillSpec::Solid([0.0; 4]),
            PaintStyle::Fill,
            StrokeCap::Butt,
            StrokeJoin::Miter,
        );
    }

    #[test]
    fn write_ts_roundtrip_fixture_bytes() {
        // Writes the fixed binary fixture that vitest loads for AC5 core→TS
        // round-trip. Always rewrites so the committed file stays in lockstep
        // with encode_ir_envelope; the test also asserts a stable non-empty size.
        let mut render_frame = roundtrip_fixture_render_frame();
        intern_image_strings(&mut render_frame.draw);
        let mut scratch = DrawFrameScratch::default();
        let bytes = encode_ir_envelope(&render_frame, &mut scratch).unwrap();

        let fixture_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/fixtures/ocir");
        std::fs::create_dir_all(&fixture_dir).expect("fixture dir");
        // v5: canonical self-contained OCIR — no pipeline_epoch in header
        let path = fixture_dir.join("roundtrip_v5.ocir");
        std::fs::write(&path, &bytes).expect("write fixture");
        assert!(bytes.len() > 64, "fixture must be non-trivial");
        // Sanity: re-read matches.
        let reread = std::fs::read(&path).unwrap();
        assert_eq!(reread, bytes);
    }

    /// AC #48: OCIR encoding is byte-deterministic for a non-trivial RenderFrame.
    /// Encoding the same frame must produce byte-identical output regardless of
    /// scratch reuse or fresh allocation.
    #[test]
    fn encode_non_trivial_frame_is_byte_deterministic() {
        let mut render_frame = roundtrip_fixture_render_frame();
        intern_image_strings(&mut render_frame.draw);

        // Encode twice with the same reused scratch.
        let mut scratch = DrawFrameScratch::default();
        let bytes_a = encode_ir_envelope(&render_frame, &mut scratch).unwrap();
        scratch.clear();
        let bytes_b = encode_ir_envelope(&render_frame, &mut scratch).unwrap();
        assert_eq!(
            bytes_a, bytes_b,
            "same RenderFrame encoded twice (reused scratch) must produce byte-identical OCIR"
        );

        // Encode again with a fresh scratch — still identical.
        let mut fresh_scratch = DrawFrameScratch::default();
        let bytes_c = encode_ir_envelope(&render_frame, &mut fresh_scratch).unwrap();
        assert_eq!(
            bytes_a, bytes_c,
            "same RenderFrame encoded with fresh scratch must produce byte-identical OCIR"
        );

        // Each envelope is a valid self-contained OCIR v5.
        assert_eq!(&bytes_a[0..4], b"OCIR");
        assert_eq!(
            u32::from_le_bytes(bytes_a[4..8].try_into().unwrap()),
            IR_VERSION,
        );
        let section_count = u32::from_le_bytes(bytes_a[8..12].try_into().unwrap());
        assert!(section_count > 0, "non-trivial envelope must have sections");
    }
}
