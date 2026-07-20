import { beforeEach, describe, expect, test } from 'vitest';
import { __generatedImageTestSeam } from '../../crates/opencat-web/web/src/draw-ir';

// Issue #10: the OCIR v4 envelope carries a `pipeline_epoch` in its header and
// a generated-image delta (section 12) so core-rasterized color-emoji glyphs
// flow to JS without per-glyph JS→WASM→JS round-trips. These tests build a
// minimal v4 envelope by hand and exercise the decoder's epoch + delta + cache
// semantics directly — the behavioral mirror of the Rust encoder tests in
// `opencat_web/src/wasm_bridge.rs`.

// --- v4 envelope builder ---------------------------------------------------

// Section ids must match draw-ir.ts.
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
function align4(n: number): number {
  return (n + 3) & ~3;
}

type GeneratedRecord = { id: bigint; width: number; height: number; rgba: number[] };

function encodeGeneratedDelta(delta: GeneratedRecord[]): number[] {
  const out: number[] = [];
  out.push(...u32(delta.length));
  for (const r of delta) {
    out.push(...u64(r.id));
    out.push(...u32(r.width));
    out.push(...u32(r.height));
    out.push(...u32(r.rgba.length));
    out.push(...r.rgba);
  }
  return out;
}

/// Build a v4 OCIR envelope with only the sections the decoder strictly
/// requires plus a generated-image delta. All other sections are present but
/// empty so `requireSection` is satisfied.
function buildV4Envelope(epoch: number, delta: GeneratedRecord[]): Uint8Array {
  // Each section payload is length-prefixed where the decoder expects it; for
  // the empty-section cases we still need the decoder's expected count prefix
  // (u32 0). Subtrees payload is `count(u32)` then count×bytesWithLen.
  const sections: [number, number[]][] = [
    [SECTION_OPS, []],
    [SECTION_SUBTREES, u32(0)],
    [SECTION_F32_POOL, []],
    [SECTION_BYTES, []],
    [SECTION_BYTE_RANGES, []],
    // One empty string: UTF8 = "" and one range {0,0} so `strings` = [''].
    [SECTION_STRINGS_UTF8, []],
    [SECTION_STRING_RANGES, [...u32(0), ...u32(0)]],
    // paints: count 0.
    [SECTION_PAINTS, u32(0)],
    // paths: count 0.
    [SECTION_PATHS, u32(0)],
    // children: count 0.
    [SECTION_CHILDREN, u32(0)],
    // effects: count 0.
    [SECTION_EFFECTS, u32(0)],
    [SECTION_GENERATED_IMAGES, encodeGeneratedDelta(delta)],
  ];

  const headerLen = 16 + sections.length * 12;
  const offsets: number[] = [];
  let cursor = headerLen;
  for (const [, payload] of sections) {
    cursor = align4(cursor);
    offsets.push(cursor);
    cursor += payload.length;
  }

  const out: number[] = [];
  out.push(0x4f, 0x43, 0x49, 0x52); // "OCIR"
  out.push(...u32(4)); // version 4
  out.push(...u32(sections.length));
  out.push(...u32(epoch));
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

describe('OCIR v4 generated-image delta decoder (#10)', () => {
  beforeEach(() => {
    __generatedImageTestSeam.reset();
  });

  test('reads the pipeline epoch from the envelope header', () => {
    const bytes = buildV4Envelope(7, []);
    __generatedImageTestSeam.decode(bytes);
    expect(__generatedImageTestSeam.currentEpoch()).toBe(7);
  });

  test('registers a delta glyph under (epoch, id) with faithful fields', () => {
    const rgba = [0xff, 0x00, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff];
    const bytes = buildV4Envelope(3, [
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

  test('an empty delta registers no glyphs (section 12 still present)', () => {
    const bytes = buildV4Envelope(1, []);
    __generatedImageTestSeam.decode(bytes);
    expect(__generatedImageTestSeam.cacheSize()).toBe(0);
  });

  test('multiple glyphs in one delta are all registered', () => {
    const bytes = buildV4Envelope(2, [
      { id: 1n, width: 1, height: 1, rgba: [0x10, 0x20, 0x30, 0x40] },
      { id: 2n, width: 3, height: 2, rgba: Array(24).fill(0xab) },
    ]);
    __generatedImageTestSeam.decode(bytes);
    expect(__generatedImageTestSeam.cacheSize()).toBe(2);
    expect(__generatedImageTestSeam.rgbaFor(1n)!.width).toBe(1);
    expect(__generatedImageTestSeam.rgbaFor(2n)!.width).toBe(3);
    expect(__generatedImageTestSeam.rgbaFor(2n)!.rgba.length).toBe(24);
  });

  test('a second frame under the same epoch re-publishes nothing for known ids', () => {
    // Simulate the Rust delta bookkeeping: frame 0 publishes the glyph; frame 1
    // (same epoch) carries an empty delta because the id was already sent.
    const glyph = { id: 99n, width: 2, height: 2, rgba: Array(16).fill(0x55) };
    __generatedImageTestSeam.decode(buildV4Envelope(5, [glyph]));
    expect(__generatedImageTestSeam.cacheSize()).toBe(1);

    __generatedImageTestSeam.decode(buildV4Envelope(5, []));
    // Still registered from frame 0 — the empty delta must not clear it.
    expect(__generatedImageTestSeam.cacheSize()).toBe(1);
    expect(__generatedImageTestSeam.rgbaFor(99n)).toBeDefined();
  });

  test('a new epoch evicts stale glyphs and accepts a fresh republish', () => {
    // Epoch 5: glyph 99 published.
    __generatedImageTestSeam.decode(buildV4Envelope(5, [
      { id: 99n, width: 1, height: 1, rgba: [0xff, 0xff, 0xff, 0xff] },
    ]));
    expect(__generatedImageTestSeam.currentEpoch()).toBe(5);
    expect(__generatedImageTestSeam.rgbaFor(99n)).toBeDefined();

    // Epoch 6 (new design): glyph 99's stale entry is gone until re-sent.
    __generatedImageTestSeam.decode(buildV4Envelope(6, []));
    expect(__generatedImageTestSeam.currentEpoch()).toBe(6);
    expect(__generatedImageTestSeam.rgbaFor(99n)).toBeUndefined();

    // Epoch 6 frame 1: glyph 99 republished under the new epoch.
    __generatedImageTestSeam.decode(buildV4Envelope(6, [
      { id: 99n, width: 1, height: 1, rgba: [0xff, 0xff, 0xff, 0xff] },
    ]));
    expect(__generatedImageTestSeam.rgbaFor(99n)).toBeDefined();
  });

  test('a new epoch also drops pending RGBA that was never built into an image', () => {
    // A glyph whose delta arrived but whose CanvasKit Image was never resolved
    // (e.g. it was offscreen) still has a pending RGBA record. The epoch bump
    // must drop it too, or the cache leaks across designs.
    __generatedImageTestSeam.decode(buildV4Envelope(1, [
      { id: 7n, width: 1, height: 1, rgba: [0x12, 0x34, 0x56, 0x78] },
    ]));
    expect(__generatedImageTestSeam.rgbaFor(7n)).toBeDefined();

    __generatedImageTestSeam.decode(buildV4Envelope(2, []));
    expect(__generatedImageTestSeam.rgbaFor(7n)).toBeUndefined();
  });

  test('rejects a v3 envelope (no epoch header)', () => {
    // A v3 envelope would place section descriptors at offset 12; the v4
    // decoder reads the epoch at offset 12, so accepting v3 silently would
    // misread the first section id as an epoch. The version gate is the guard.
    const v3 = new Uint8Array([
      0x4f, 0x43, 0x49, 0x52,
      ...u32(3),
      ...u32(0),
    ]);
    expect(() => __generatedImageTestSeam.decode(v3)).toThrow(/version 3/);
  });
});
