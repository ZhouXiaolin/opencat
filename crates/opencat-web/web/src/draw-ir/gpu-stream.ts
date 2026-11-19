// OCIR section 13 (GPU_STREAMS) decoder.
//
// Authority: `crates/opencat-core/src/ir/draw_encoding.rs` `encode_gpu_streams`
// and `ir/gpu_stream.rs`. Layout per stream:
//
//   u64 image_id            — content-addressed generated-image id; the draw
//                             stream's `ImageRef::Generated` placeholder cites
//                             it and the executor's output lands under the same
//                             key (`generatedImageCanvas` / `generatedImageRgba`
//                             in decode.ts).
//   u32 target_id_len | target_id bytes
//   u32 width | u32 height
//   u8  backend             — WGPU_BACKEND_WEBGL2 (=0) / WGPU_BACKEND_WEBGPU (=1)
//   u8  attrs               — packed 6-bit: alpha|antialias|premultiplied|
//                             preserve|depth|stencil (see ContextAttrs::pack)
//   u32 ops_len | ops bytes — one command = `[op: u8][PAYLOAD_LEN[op] payload]`
//   u32 bytes_len | bytes   — intern pool for variable data (shader sources,
//                             texImage pixels, uniform payloads)
//   u32 range_count × { u32 start, u32 len } — byte_ranges into `bytes`
//
// All integers little-endian; no padding.

import {
  SECTION,
  WGPU_BACKEND_WEBGL2,
  WGPU_BACKEND_WEBGPU,
} from '../generated/ocir-schema.generated';
import { BinaryReader } from './decode';

export type GpuBackendKind = 'webgl2' | 'webgpu';

export type GpuContextAttrs = {
  alpha: boolean;
  antialias: boolean;
  premultipliedAlpha: boolean;
  preserveDrawingBuffer: boolean;
  depth: boolean;
  stencil: boolean;
};

export type GpuByteRange = { start: number; len: number };

export type GpuOpFrame = {
  /// Bit-encoded command stream: `[op: u8][payload]`.
  ops: Uint8Array;
  /// Intern pool the ops reference by byte-range id.
  bytes: Uint8Array;
  byteRanges: GpuByteRange[];
};

export type GpuStream = {
  /// Generated-image id as a decimal string — the same key space as
  /// `generatedImageRgba` / `resolveImage('generated')`.
  imageId: string;
  /// Target canvas node id.
  targetId: string;
  width: number;
  height: number;
  backend: GpuBackendKind;
  attrs: GpuContextAttrs;
  ops: GpuOpFrame;
};

export type GpuSection = { streams: GpuStream[] };

export class GpuStreamDecodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'GpuStreamDecodeError';
  }
}

/// Unpack the 6-bit ContextAttrs byte (alpha|antialias|premultiplied|
/// preserve|depth|stencil, LSB-first — mirrors `ContextAttrs::pack`).
function unpackAttrs(packed: number): GpuContextAttrs {
  return {
    alpha: (packed & 1 << 0) !== 0,
    antialias: (packed & 1 << 1) !== 0,
    premultipliedAlpha: (packed & 1 << 2) !== 0,
    preserveDrawingBuffer: (packed & 1 << 3) !== 0,
    depth: (packed & 1 << 4) !== 0,
    stencil: (packed & 1 << 5) !== 0,
  };
}

function parseBackend(backend: number): GpuBackendKind {
  if (backend === WGPU_BACKEND_WEBGL2) return 'webgl2';
  // WebGPU is the only other value on the wire; anything else is producer
  // drift — fail loud instead of silently skipping a stream.
  if (backend !== WGPU_BACKEND_WEBGPU) {
    throw new GpuStreamDecodeError(`Unknown GPU stream backend ${backend}`);
  }
  return 'webgpu';
}

export function parseGpuStreams(section: Uint8Array): GpuSection {
  const reader = new BinaryReader(section);
  const count = reader.u32();
  const streams: GpuStream[] = [];
  for (let i = 0; i < count; i++) {
    const imageId = reader.u64().toString();
    const targetId = reader.stringWithLen();
    const width = reader.u32();
    const height = reader.u32();
    const backend = parseBackend(reader.u8());
    const attrs = unpackAttrs(reader.u8());

    const opsLen = reader.u32();
    const ops = reader.take(opsLen);
    const bytesLen = reader.u32();
    const bytes = reader.take(bytesLen);

    const rangeCount = reader.u32();
    const byteRanges: GpuByteRange[] = [];
    for (let r = 0; r < rangeCount; r++) {
      byteRanges.push({ start: reader.u32(), len: reader.u32() });
    }
    // Range bounds are validated eagerly here so a truncated envelope fails
    // at decode time, not mid-frame.
    for (const range of byteRanges) {
      if (range.start > bytes.byteLength || range.len > bytes.byteLength - range.start) {
        throw new GpuStreamDecodeError(
          `GPU stream ${imageId}: byte range start=${range.start} len=${range.len} exceeds pool ${bytes.byteLength}`,
        );
      }
    }

    streams.push({ imageId, targetId, width, height, backend, attrs, ops: { ops, bytes, byteRanges } });
  }
  return { streams };
}

/// Decode section 13 if present; `null` when the frame carries no GPU streams
/// (section 13 is optional — 2D-only frames omit it entirely).
export function decodeGpuSection(sections: Map<number, Uint8Array>): GpuSection | null {
  const section = sections.get(SECTION.GPU_STREAMS);
  if (!section) return null;
  return parseGpuStreams(section);
}
