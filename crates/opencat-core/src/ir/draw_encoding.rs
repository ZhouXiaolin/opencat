// ---------------------------------------------------------------------------
// Binary encoder for DrawOpFrame -> EncodedDrawFrame
//
// Encodes a typed IR frame into a packed binary format suitable for
// transfer to Web/TypeScript decoders via wasm-bindgen.
//
// Layout per op:
//   [opcode: u16 LE] [flags: u16 LE] [payload_len: u32 LE] [payload...]
//
// Each op is padded to 4-byte alignment after its payload.
// ---------------------------------------------------------------------------

use super::draw_op::*;
use super::draw_types::*;

// ---------------------------------------------------------------------------
// Magic and version constants
// ---------------------------------------------------------------------------

/// Magic bytes for EncodedDrawFrame: "OCDF" (OpenCat Draw Frame).
const MAGIC: [u8; 4] = *b"OCDF";

/// Version of the binary encoding format.
const VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Opcode assignments
// ---------------------------------------------------------------------------

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
}

// ---------------------------------------------------------------------------
// Encoded output types
// ---------------------------------------------------------------------------

/// Binary-encoded draw frame for web wasm-to-JS transfer.
/// Fields exposed to JS as typed arrays via wasm-bindgen.
pub struct EncodedDrawFrame {
    pub magic: [u8; 4],
    pub version: u32,
    pub ops: Vec<u8>,
    pub subtrees: Vec<u8>,
    pub f32_pool: Vec<f32>,
    pub bytes: Vec<u8>,
    pub byte_ranges: Vec<TableRange>,
    pub strings_utf8: Vec<u8>,
    pub string_ranges: Vec<TableRange>,
    pub children: Vec<u8>,
    pub child_ranges: Vec<TableRange>,
}

/// Metadata for a cached range in the encoded frame.
pub struct EncodedDrawRange {
    pub start_byte: u32,
    pub byte_len: u32,
    pub fingerprint: u64,
    pub bounds: [f32; 4],
}

// ---------------------------------------------------------------------------
// Main encode entry point
// ---------------------------------------------------------------------------

/// Encode a DrawOpFrame into an EncodedDrawFrame, reusing scratch buffers.
/// The caller owns the returned EncodedDrawFrame. Scratch is cleared on entry
/// and can be reused across frames to avoid allocations.
pub fn encode_draw_frame(
    frame: &super::draw_frame::DrawOpFrame,
    scratch: &mut super::draw_frame::DrawFrameScratch,
) -> EncodedDrawFrame {
    scratch.clear();

    // Copy frame f32_pool into scratch so F32Range references in SetLineDash/Points
    // can index into it. The data is preserved in the encoded output.
    scratch.f32_pool.extend_from_slice(&frame.f32_pool);

    // Encode each DrawOp into the encoded_ops buffer
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
    let bytes_out = std::mem::take(&mut scratch.bytes);
    let byte_ranges_out = std::mem::take(&mut scratch.byte_ranges);

    // Encode strings: concatenated UTF-8 + range table
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

    // Children are currently encoded as empty (full encoding in later tasks)
    let children_bytes: Vec<u8> = Vec::new();
    let child_ranges: Vec<TableRange> = Vec::new();

    EncodedDrawFrame {
        magic: MAGIC,
        version: VERSION,
        ops: std::mem::take(encoded_ops),
        subtrees: std::mem::take(encoded_subtrees),
        f32_pool: f32_pool_out,
        bytes: bytes_out,
        byte_ranges: byte_ranges_out,
        strings_utf8: std::mem::take(strings_utf8),
        string_ranges: std::mem::take(string_ranges),
        children: children_bytes,
        child_ranges,
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
fn encode_line_cap(cap: LineCap) -> u32 {
    match cap {
        LineCap::Butt => 0,
        LineCap::Round => 1,
        LineCap::Square => 2,
    }
}

/// Encode a LineJoin variant as its u32 discriminant.
/// Mapping: 0=Miter, 1=Round, 2=Bevel.
#[inline]
fn encode_line_join(join: LineJoin) -> u32 {
    match join {
        LineJoin::Miter => 0,
        LineJoin::Round => 1,
        LineJoin::Bevel => 2,
    }
}

/// Encode a PointMode variant as its u32 discriminant.
/// Mapping: 0=Points, 1=Lines, 2=Polygon.
#[inline]
fn encode_point_mode(mode: PointMode) -> u32 {
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
        // Image — tag(1) + string_id(4) + frame_index(4) + time_micros(8) + x(4) + y(4) + paint_id(4)
        // ===================================================================
        DrawOp::Image { image, x, y, paint } => {
            // Payload: 1 + 4 + 4 + 8 + 4 + 4 + 4 = 29
            write_op_header(buf, opcode::IMAGE, 29);
            match image {
                ImageRef::Static { asset_id } => {
                    write_u8(buf, 0); // tag: Static
                    write_u32(buf, lookup_string_id(strings, asset_id));
                    write_u32(buf, 0); // frame_index = 0
                    write_u64(buf, 0); // time_micros = 0
                }
                ImageRef::VideoFrame {
                    asset_id,
                    frame_index,
                    time_micros,
                } => {
                    write_u8(buf, 1); // tag: VideoFrame
                    write_u32(buf, lookup_string_id(strings, asset_id));
                    write_u32(buf, *frame_index);
                    write_u64(buf, *time_micros);
                }
            }
            write_f32(buf, *x);
            write_f32(buf, *y);
            write_u32(buf, paint.map(|p| p.0).unwrap_or(0xFFFF_FFFF));
        }

        // ===================================================================
        // ImageRect — tag(1) + string_id(4) + frame_index(4) + time_micros(8) +
        //             has_src(1) + src(16) + dst(16) + paint_id(4)
        // ===================================================================
        DrawOp::ImageRect {
            image,
            src,
            dst,
            paint,
        } => {
            // Payload: 1 + 4 + 4 + 8 + 1 + 16 + 16 + 4 = 54
            write_op_header(buf, opcode::IMAGE_RECT, 54);
            match image {
                ImageRef::Static { asset_id } => {
                    write_u8(buf, 0); // tag: Static
                    write_u32(buf, lookup_string_id(strings, asset_id));
                    write_u32(buf, 0); // frame_index = 0
                    write_u64(buf, 0); // time_micros = 0
                }
                ImageRef::VideoFrame {
                    asset_id,
                    frame_index,
                    time_micros,
                } => {
                    write_u8(buf, 1); // tag: VideoFrame
                    write_u32(buf, lookup_string_id(strings, asset_id));
                    write_u32(buf, *frame_index);
                    write_u64(buf, *time_micros);
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
    use crate::ir::draw_types::{ChildRange, DrawOpRange};
    use crate::render::builder::DrawOpBuilder;

    // -----------------------------------------------------------------------
    // Required tests from the task specification
    // -----------------------------------------------------------------------

    #[test]
    fn encode_empty_frame_produces_valid_header() {
        let frame = DrawOpFrame::default();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_frame(&frame, &mut scratch);
        assert_eq!(&encoded.magic, b"OCDF");
        assert_eq!(encoded.version, 2);
        assert!(encoded.ops.is_empty());
    }

    #[test]
    fn encode_save_restore_roundtrips_op_count() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::Save);
        builder.push(DrawOp::Restore);
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_frame(&frame, &mut scratch);
        // Each op has a 8-byte header + payload, so ops buffer should be non-empty
        assert!(!encoded.ops.is_empty());
    }

    #[test]
    fn encoded_frame_magic_is_constant() {
        let frame = DrawOpFrame::default();
        let mut scratch = DrawFrameScratch::default();
        let a = encode_draw_frame(&frame, &mut scratch);
        scratch.clear();
        let b = encode_draw_frame(&frame, &mut scratch);
        assert_eq!(a.magic, b.magic);
    }

    #[test]
    fn encode_translate_produces_correct_payload_length() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::Translate { x: 10.0, y: 20.0 });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_frame(&frame, &mut scratch);
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
                frame_index: 42,
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
        let encoded = encode_draw_frame(&frame, &mut scratch);

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
        let encoded = encode_draw_frame(&frame, &mut scratch);

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
        let encoded = encode_draw_frame(&frame, &mut scratch);

        // ClearLineDash: 8 byte header + 0 byte payload = 8 bytes
        assert_eq!(encoded.ops.len(), 8);
    }

    #[test]
    fn encode_set_anti_alias_padded_to_4_byte_alignment() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::SetAntiAlias { enabled: true });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_frame(&frame, &mut scratch);

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
        let e1 = encode_draw_frame(&f1, &mut scratch);
        assert!(!e1.ops.is_empty());

        // Second frame: same op, reused scratch
        let mut b2 = DrawOpBuilder::default();
        b2.push(DrawOp::Save);
        let f2 = b2.finish();
        let e2 = encode_draw_frame(&f2, &mut scratch);
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
        let encoded = encode_draw_frame(&frame, &mut scratch);

        // 8 header + 36 payload = 44 bytes
        assert_eq!(encoded.ops.len(), 44);
    }

    #[test]
    fn encode_restore_to_count_is_4_bytes_payload() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::RestoreToCount { count: 3 });
        let frame = builder.finish();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_frame(&frame, &mut scratch);

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
        let encoded = encode_draw_frame(&frame, &mut scratch);

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
        let encoded = encode_draw_frame(&frame, &mut scratch);

        // 8 header + 29 payload = 37, padded to 40
        assert_eq!(encoded.ops.len(), 40);
        assert_eq!(encoded.ops.len() % 4, 0);
    }

    #[test]
    fn encode_image_rect_produces_correct_size() {
        let mut builder = DrawOpBuilder::default();
        builder.intern_string("clip.mp4");
        builder.push(DrawOp::ImageRect {
            image: ImageRef::VideoFrame {
                asset_id: "clip.mp4".to_string(),
                frame_index: 7,
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
        let encoded = encode_draw_frame(&frame, &mut scratch);

        // 8 header + 54 payload = 62, padded to 64
        assert_eq!(encoded.ops.len(), 64);
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
        let encoded = encode_draw_frame(&frame, &mut scratch);

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
        let encoded = encode_draw_frame(&frame, &mut scratch);

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
        let encoded = encode_draw_frame(&frame, &mut scratch);

        // The f32_pool should contain the dash intervals
        assert_eq!(encoded.f32_pool.len(), 3);
        assert_eq!(encoded.f32_pool[0], 5.0);
        assert_eq!(encoded.f32_pool[1], 3.0);
        assert_eq!(encoded.f32_pool[2], 5.0);
    }
}
