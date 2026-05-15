import type { CompositionInfo, ParsedResult, ResourceRequests } from './types';

type WasmModule = {
  default(): Promise<void>;
  WebRenderer: {
    new(): WebRendererInstance;
  };
  parse_jsonl(input: string): string;
  get_composition_info(input: string): string;
  collect_resources_json(input: string): string;
  build_frame(jsonl_input: string, frame: number, resource_meta: string, mutations_json: string): string;
  preload_assets(jsonl: string): Promise<string>;
  get_blob_bytes(asset_id: string): Uint8Array | undefined;
  clear_blobs(): void;
  blob_count(): number;
};

export interface WebRendererInstance {
  build_frame(jsonl: string, frame: number, resources: string, mutations: string): BuildFrameResult;
  inject_video_frame(asset_id: string, frame: number, rgba: Uint8Array, width: number, height: number): void;
  clear_video_cache(asset_id: string): void;
  decode_audio_file(asset_id: string, data: Uint8Array): Promise<void>;
  get_audio_samples(asset_id: string, start_secs: number, duration_secs: number, target_rate: number): string;
  play_audio_at(asset_id: string, offset_secs: number, duration_secs: number): void;
  stop_audio(): void;
  set_audio_volume(volume: number): void;
  clear_audio_cache(): void;
  audio_context_time(): number;
  query_subtree_snapshot(key: bigint): SubtreeCacheResult;
  query_glyph_path(key: bigint): string | undefined;
  free(): void;
}

interface BuildFrameResult {
  ops_json: string;
  frame_width: number;
  frame_height: number;
}

interface SubtreeCacheResult {
  found: boolean;
  secondary_fingerprint: number;
  recorded_bounds_x: number;
  recorded_bounds_y: number;
  recorded_bounds_w: number;
  recorded_bounds_h: number;
  consecutive_hits: number;
  render_mode: string;
}

let wasmModule: WasmModule | null = null;
let renderer: WebRendererInstance | null = null;

export async function initWasm(): Promise<void> {
  if (wasmModule) return;
  const mod = await import('../wasm/opencat_web.js');
  await mod.default();
  wasmModule = mod as unknown as WasmModule;
  renderer = new wasmModule.WebRenderer();
}

export function parseJsonl(input: string): ParsedResult {
  if (!wasmModule) {
    return { composition: null, elements: [], elementCount: 0 };
  }
  const json = wasmModule.parse_jsonl(input);
  return JSON.parse(json) as ParsedResult;
}

export function getCompositionInfo(input: string): CompositionInfo | null {
  if (!wasmModule) return null;
  const json = wasmModule.get_composition_info(input);
  const info = JSON.parse(json) as CompositionInfo;
  if (info.width === 0 || info.height === 0) return null;
  return info;
}

export function collectResources(input: string): ResourceRequests {
  if (!wasmModule) {
    return { images: [], videos: [], audios: [], icons: [] };
  }
  try {
    const json = wasmModule.collect_resources_json(input);
    return JSON.parse(json) as ResourceRequests;
  } catch {
    return { images: [], videos: [], audios: [], icons: [] };
  }
}

export function buildFrame(
  jsonlInput: string,
  frame: number,
  resourceMeta: string,
  mutationsJson: string,
): any {
  if (!wasmModule) throw new Error('WASM not initialized');
  const json = wasmModule.build_frame(jsonlInput, frame, resourceMeta, mutationsJson);
  const result = JSON.parse(json);
  if (result.error) throw new Error(result.error);
  return result;
}

// ── 资源预加载（下载 + 元数据探测在 Rust 侧完成） ──

/**
 * 由 Rust 侧通过 fetch + nom-exif/imagesize 完成下载与元数据读取。
 * 返回的字符串是 catalog JSON（{ "<assetId>": { width, height, kind, durationSecs? }, ... }），
 * 可直接作为 buildFrame 的 resource_meta 参数。
 * 下载的字节同时保存在 wasm 的 BlobStore 中，调 getBlobBytes(assetId) 可拿回。
 */
export async function preloadAssets(jsonl: string): Promise<string> {
  if (!wasmModule) throw new Error('WASM not initialized');
  return await wasmModule.preload_assets(jsonl);
}

export function getBlobBytes(assetId: string): Uint8Array | undefined {
  if (!wasmModule) return undefined;
  return wasmModule.get_blob_bytes(assetId);
}

export function clearBlobs(): void {
  if (!wasmModule) return;
  wasmModule.clear_blobs();
}

export function blobCount(): number {
  if (!wasmModule) return 0;
  return wasmModule.blob_count();
}

// ── WebRenderer access ──

export function getRenderer(): WebRendererInstance | null {
  return renderer;
}

export function getRendererOrThrow(): WebRendererInstance {
  if (!renderer) throw new Error('WASM renderer not initialized');
  return renderer;
}
