import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { beforeEach, describe, expect, test } from 'vitest';
import { __generatedImageTestSeam } from '../../crates/opencat-web/web/src/draw-ir';

// Issue #10 / #45: OCIR v5 self-contained envelope. Hand-built envelopes exercise
// decoder error paths and cache semantics; `roundtrip_v5.ocir` is produced by core
// `encode_ir_envelope` (see write_ts_roundtrip_fixture_bytes) for AC5.

const SECTION_OPS = 1;
const SECTION_F32_POOL = 2;
const SECTION_BYTES = 3;
const SECTION_BYTE_RANGES = 4;
const SECTION_STRINGS_UTF8 = 5;
const SECTION_STRING_RANGES = 6;
const SECTION_PAINTS = 7;
const SECTION_PATHS = 8;
const SECTION_CHILDREN = 9;
const SECTION_EFFECTS = 10;
const SECTION_SUBTREES = 11;
const SECTION_GENERATED_IMAGES = 12;

function u32(n: number): number[] {
  return [n & 0xff, (n >>> 8) & 0xff, (n >>> 16) & 0xff, (n >>> 24) & 0xff];
}
function u64(n: bigint): number[] {
  const bytes = [];
  for (let i = 0; i < 8; i++) bytes.push(Number((n >> BigInt(i * 8)) & 0xffn));
  return bytes;
}
function f32(n: number): number[] {
  const buf = new ArrayBuffer(4);
  new DataView(buf).setFloat32(0, n, true);
  return [...new Uint8Array(buf)];
}
function align4(n: number): number {
  return (n + 3) & ~3;
}

type GeneratedRecord = { id: bigint; width: number; height: number; rgba: number[] };

function encodeGeneratedImages(images: GeneratedRecord[]): number[] {
  const out: number[] = [];
  out.push(...u32(images.length));
  for (const r of images) {
    out.push(...u64(r.id));
    out.push(...u32(r.width));
    out.push(...u32(r.height));
    out.push(...u32(r.rgba.length));
    out.push(...r.rgba);
  }
  return out;
}

/** Pack section id → payload into a v5 OCIR envelope (no pipeline_epoch). Shared by hand-built tests. */
function packOcirEnvelope(sections: [number, number[]][]): Uint8Array {
  const headerLen = 12 + sections.length * 12;
  const offsets: number[] = [];
  let cursor = headerLen;
  for (const [, payload] of sections) {
    cursor = align4(cursor);
    offsets.push(cursor);
    cursor += payload.length;
  }
  const out: number[] = [];
  out.push(0x4f, 0x43, 0x49, 0x52); // "OCIR"
  out.push(...u32(5));               // version 5
  out.push(...u32(sections.length)); // section_count
  for (let i = 0; i < sections.length; i++) {
    out.push(...u32(sections[i][0]));
    out.push(...u32(offsets[i]));
    out.push(...u32(sections[i][1].length));
  }
  for (let i = 0; i < sections.length; i++) {
    while (out.length < offsets[i]) out.push(0);
    out.push(...sections[i][1]);
  }
  return new Uint8Array(out);
}

function emptySections(overrides: Partial<Record<number, number[]>> = {}): [number, number[]][] {
  const base: [number, number[]][] = [
    [SECTION_OPS, []],
    [SECTION_SUBTREES, u32(0)],
    [SECTION_F32_POOL, []],
    [SECTION_BYTES, []],
    [SECTION_BYTE_RANGES, []],
    [SECTION_STRINGS_UTF8, []],
    [SECTION_STRING_RANGES, [...u32(0), ...u32(0)]],
    [SECTION_PAINTS, u32(0)],
    [SECTION_PATHS, u32(0)],
    [SECTION_CHILDREN, u32(0)],
    [SECTION_EFFECTS, u32(0)],
    [SECTION_GENERATED_IMAGES, encodeGeneratedImages([])],
  ];
  return base.map(([id, payload]) => [id, overrides[id] ?? payload]);
}

function buildV5Envelope(images: GeneratedRecord[]): Uint8Array {
  return packOcirEnvelope(emptySections({
    [SECTION_GENERATED_IMAGES]: encodeGeneratedImages(images),
  }));
}

describe('OCIR v5 generated-image self-contained decoder (#45)', () => {
  beforeEach(() => {
    __generatedImageTestSeam.reset();
  });

  test('reads the version and section_count from a 12-byte header', () => {
    const bytes = buildV5Envelope([]);
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    expect(String.fromCharCode(bytes[0], bytes[1], bytes[2], bytes[3])).toBe('OCIR');
    expect(view.getUint32(4, true)).toBe(5);
    // No pipeline_epoch at offset 12; directory starts at offset 12.
    const sectionCount = view.getUint32(8, true);
    expect(sectionCount).toBe(12); // all required sections
  });

  test('registers a glyph under its id with faithful fields', () => {
    const rgba = [0xff, 0x00, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff];
    const bytes = buildV5Envelope([
      { id: 0x0123_4567_89ab_cdefn, width: 2, height: 1, rgba },
    ]);
    __generatedImageTestSeam.decode(bytes);

    expect(__generatedImageTestSeam.cacheSize()).toBe(1);
    const entry = __generatedImageTestSeam.rgbaFor(0x0123_4567_89ab_cdefn);
    expect(entry).toBeDefined();
    expect(entry!.width).toBe(2);
    expect(entry!.height).toBe(1);
    expect(Array.from(entry!.rgba)).toEqual(rgba);
  });

  test('an empty generated-images section registers no glyphs', () => {
    const bytes = buildV5Envelope([]);
    __generatedImageTestSeam.decode(bytes);
    expect(__generatedImageTestSeam.cacheSize()).toBe(0);
  });

  test('multiple glyphs are all registered', () => {
    const bytes = buildV5Envelope([
      { id: 1n, width: 1, height: 1, rgba: [0x10, 0x20, 0x30, 0x40] },
      { id: 2n, width: 3, height: 2, rgba: Array(24).fill(0xab) },
    ]);
    __generatedImageTestSeam.decode(bytes);
    expect(__generatedImageTestSeam.cacheSize()).toBe(2);
    expect(__generatedImageTestSeam.rgbaFor(1n)!.width).toBe(1);
    expect(__generatedImageTestSeam.rgbaFor(2n)!.width).toBe(3);
    expect(__generatedImageTestSeam.rgbaFor(2n)!.rgba.length).toBe(24);
  });

  test('same glyph re-encoded on every frame is idempotent (self-contained)', () => {
    // In v5, every frame carries the full generated-image RGBA — no delta.
    // The second decode of the same glyph is a no-op in the cache.
    const glyph = { id: 99n, width: 2, height: 2, rgba: Array(16).fill(0x55) };
    __generatedImageTestSeam.decode(buildV5Envelope([glyph]));
    expect(__generatedImageTestSeam.cacheSize()).toBe(1);

    __generatedImageTestSeam.decode(buildV5Envelope([]));
    expect(__generatedImageTestSeam.cacheSize()).toBe(1);
    expect(__generatedImageTestSeam.rgbaFor(99n)).toBeDefined();
  });

  test('rejects a v4 envelope (no longer supported)', () => {
    const v4 = new Uint8Array([
      0x4f, 0x43, 0x49, 0x52,
      ...u32(4),
      ...u32(0),
      ...u32(0), // pipeline_epoch in v4
    ]);
    expect(() => __generatedImageTestSeam.decode(v4)).toThrow(/Unsupported OpenCat IR version/);
  });
});

describe('OCIR protocol errors (#22)', () => {
  beforeEach(() => {
    __generatedImageTestSeam.reset();
  });

  test('rejects truncated envelope (no header)', () => {
    expect(() => __generatedImageTestSeam.decode(new Uint8Array([0x4f, 0x43, 0x49]))).toThrow(
      /Truncated OpenCat IR envelope/,
    );
  });

  test('rejects truncated section directory', () => {
    const bytes = new Uint8Array([
      0x4f, 0x43, 0x49, 0x52,
      ...u32(5),
      ...u32(1),
    ]);
    expect(() => __generatedImageTestSeam.decode(bytes)).toThrow(/Truncated OpenCat IR envelope/);
  });

  test('rejects illegal section range past envelope end', () => {
    const headerLen = 12 + 12; // v5: 12-byte header + one directory entry
    const out: number[] = [];
    out.push(0x4f, 0x43, 0x49, 0x52);
    out.push(...u32(5));
    out.push(...u32(1));
    // directory entry at offset 12
    out.push(...u32(1));
    out.push(...u32(headerLen + 100));
    out.push(...u32(4));
    expect(() => __generatedImageTestSeam.decode(new Uint8Array(out))).toThrow(
      /Illegal OpenCat IR section 1 range/,
    );
  });

  test('rejects missing required section', () => {
    const out: number[] = [];
    out.push(0x4f, 0x43, 0x49, 0x52);
    out.push(...u32(5));
    out.push(...u32(0));
    expect(() => __generatedImageTestSeam.decode(new Uint8Array(out))).toThrow(
      /Missing OpenCat IR section/,
    );
  });

  test('rejects illegal string range', () => {
    const bytes = packOcirEnvelope(emptySections({
      [SECTION_STRINGS_UTF8]: [],
      [SECTION_STRING_RANGES]: [...u32(0), ...u32(5)],
    }));
    expect(() => __generatedImageTestSeam.decode(bytes)).toThrow(
      /Illegal OpenCat IR string range/,
    );
  });
});

describe('OCIR paint/path section field round-trip (#22)', () => {
  beforeEach(() => {
    __generatedImageTestSeam.reset();
  });

  function encodeSolidPaint(): number[] {
    const rec: number[] = [];
    rec.push(0); // solid fill
    rec.push(...f32(1), ...f32(0), ...f32(0), ...f32(1));
    rec.push(0); // Fill
    rec.push(1); // antiAlias
    rec.push(3); // SrcOver
    rec.push(0); // no stroke
    rec.push(0, 0, 0, 0); // no filters
    return [...u32(1), ...u32(rec.length), ...rec];
  }

  function encodeSimplePath(): number[] {
    const rec: number[] = [];
    rec.push(1); // EvenOdd
    rec.push(...u32(2));
    rec.push(...[0, 0]); // MoveTo
    rec.push(...f32(1), ...f32(2));
    rec.push(...[4, 0]); // Close
    return [...u32(1), ...u32(rec.length), ...rec];
  }

  test('decodes solid paint and path records field-by-field', () => {
    const bytes = packOcirEnvelope(emptySections({
      [SECTION_PAINTS]: encodeSolidPaint(),
      [SECTION_PATHS]: encodeSimplePath(),
    }));

    const frame = __generatedImageTestSeam.decode(bytes) as {
      paints: Array<{ fill: { type: string; color: number[] }; style: number; antiAlias: boolean; blendMode: number }>;
      paths: Array<{ fillType: number; ops: Array<{ kind: number; values: number[] }> }>;
    };
    expect(frame.paints).toHaveLength(1);
    expect(frame.paints[0].fill.type).toBe('solid');
    expect(frame.paints[0].fill.color).toEqual([1, 0, 0, 1]);
    expect(frame.paints[0].style).toBe(0);
    expect(frame.paints[0].antiAlias).toBe(true);
    expect(frame.paints[0].blendMode).toBe(3);
    expect(frame.paths).toHaveLength(1);
    expect(frame.paths[0].fillType).toBe(1);
    expect(frame.paths[0].ops).toEqual([
      { kind: 0, values: [1, 2] },
      { kind: 4, values: [] },
    ]);
  });
});

describe('core encoder -> TS decoder fixture (#45 AC5)', () => {
  beforeEach(() => {
    __generatedImageTestSeam.reset();
  });

  const fixturePath = join(
    dirname(fileURLToPath(import.meta.url)),
    'fixtures/ocir/roundtrip_v5.ocir',
  );

  test('decodes committed core fixture field-by-field', () => {
    const bytes = new Uint8Array(readFileSync(fixturePath));
    // Header: v5 has no pipeline_epoch
    expect(String.fromCharCode(bytes[0], bytes[1], bytes[2], bytes[3])).toBe('OCIR');
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    expect(view.getUint32(4, true)).toBe(5);

    const frame = __generatedImageTestSeam.decode(bytes) as {
      strings: string[];
      paints: Array<{
        fill: { type: string; color: number[] };
        style: number;
        antiAlias: boolean;
        blendMode: number;
        stroke?: { width: number; cap: number; join: number; miterLimit: number };
      }>;
      paths: Array<{ fillType: number; ops: Array<{ kind: number; values: number[] }> }>;
      effects: Array<{ hash: bigint; sksl: string }>;
      ops: Uint8Array;
    };

    expect(frame.strings).toContain('hero.png');

    expect(frame.paints).toHaveLength(1);
    expect(frame.paints[0].fill.type).toBe('solid');
    expect(frame.paints[0].fill.color[0]).toBeCloseTo(1.0);
    expect(frame.paints[0].fill.color[1]).toBeCloseTo(0.25);
    expect(frame.paints[0].fill.color[2]).toBeCloseTo(0.0);
    expect(frame.paints[0].fill.color[3]).toBeCloseTo(1.0);
    expect(frame.paints[0].style).toBe(1); // Stroke
    expect(frame.paints[0].antiAlias).toBe(true);
    expect(frame.paints[0].blendMode).toBe(3); // SrcOver
    expect(frame.paints[0].stroke).toEqual({
      width: 2.5,
      cap: 1, // Round
      join: 2, // Bevel
      miterLimit: 4.0,
    });

    expect(frame.paths).toHaveLength(1);
    expect(frame.paths[0].fillType).toBe(1); // EvenOdd
    expect(frame.paths[0].ops).toEqual([
      { kind: 0, values: [1, 2] },
      { kind: 1, values: [3, 4] },
      { kind: 4, values: [] },
    ]);

    expect(frame.effects).toHaveLength(1);
    expect(frame.effects[0].hash).toBe(0xdead_beef_cafen);
    expect(frame.effects[0].sksl).toBe('half4 main() { return half4(1); }');

    // Generated images fully encoded (no epoch)
    const glyph = __generatedImageTestSeam.rgbaFor(0x1111_2222_3333_4444n);
    expect(glyph).toBeDefined();
    expect(glyph!.width).toBe(2);
    expect(glyph!.height).toBe(1);
    expect(Array.from(glyph!.rgba)).toEqual([0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80]);

    // Ops stream: Save / Translate / Image / Restore — non-empty
    expect(frame.ops.byteLength).toBeGreaterThan(0);
    // First op opcode is Save (0) — read as u16 LE from the ops section itself
    expect(new DataView(frame.ops.buffer, frame.ops.byteOffset, 2).getUint16(0, true)).toBe(0);
  });
});

