import type { CompositionInfo, ParsedResult, ResourceRequests } from './types';

type WasmModule = {
  default(): Promise<void>;
  parse_jsonl(input: string): string;
  get_composition_info(input: string): string;
  collect_resources_json(input: string): string;
  build_frame(jsonl_input: string, frame: number, resource_meta: string, mutations_json: string): string;
  preload_assets(jsonl: string): Promise<string>;
  get_blob_bytes(asset_id: string): Uint8Array | undefined;
  clear_blobs(): void;
  blob_count(): number;
};

let wasmModule: WasmModule | null = null;

export async function initWasm(): Promise<void> {
  if (wasmModule) return;
  const mod = await import('../wasm/opencat_web.js');
  await mod.default();
  wasmModule = mod as unknown as WasmModule;
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
