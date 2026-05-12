/**
 * WasmCacheBridge — TS wrapper around wasm-bindgen cache query exports.
 *
 * Provides a typed interface for the JS side to query/update the Rust-side
 * LRU caches (subtree snapshots, glyph paths/images, raster images).
 * The underlying wasm instance is injected after initialisation.
 */

export class WasmCacheBridge {
  private renderer: any; // Will be typed properly when wasm pkg is generated

  constructor(renderer: any) {
    this.renderer = renderer;
  }

  querySubtreeSnapshot(key: number): any {
    return this.renderer.query_subtree_snapshot(key);
  }

  reportSubtreeSnapshotHit(key: number): void {
    this.renderer.report_subtree_snapshot_hit(key);
  }

  storeSubtreeSnapshot(
    key: number,
    secondary: number,
    x: number,
    y: number,
    w: number,
    h: number,
  ): void {
    this.renderer.store_subtree_snapshot(key, secondary, x, y, w, h);
  }

  queryGlyphPath(key: number): any {
    return this.renderer.query_glyph_path(key);
  }

  queryGlyphImage(key: number): Uint8Array | null {
    return this.renderer.query_glyph_image(key);
  }

  queryImage(url: string): Uint8Array | null {
    return this.renderer.query_image(url);
  }

  buildFrame(
    jsonl: string,
    frame: number,
    resources: string,
    mutations: string,
  ): any {
    return this.renderer.build_frame(jsonl, frame, resources, mutations);
  }
}
