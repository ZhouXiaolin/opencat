type WasmModule = {
  default(): Promise<void>;
  WebRenderer: {
    new(): WebRendererInstance;
  };
  init_canvaskit(): void;
  preload_assets(jsonl: string): Promise<string>;
  get_blob_bytes(asset_id: string): Uint8Array | undefined;
  clear_blobs(): void;
  blob_count(): number;
};

export interface WebRendererInstance {
  build_frame(jsonl: string, frame: number, ck_canvas: any, resources_json: string): void;
  inject_video_frame(asset_id: string, frame: number, rgba: Uint8Array, width: number, height: number): void;
  inject_video_texture(asset_id: string, image: any, width: number, height: number): void;
  clear_video_cache(asset_id: string): void;
  inject_image_bytes(asset_id: string, bytes: Uint8Array): void;
  clear_image_blobs(): void;
  decode_audio_file(asset_id: string, data: Uint8Array): Promise<void>;
  get_audio_samples(asset_id: string, start_secs: number, duration_secs: number, target_rate: number): string;
  play_audio_at(asset_id: string, offset_secs: number, duration_secs: number): void;
  stop_audio(): void;
  set_audio_volume(volume: number): void;
  clear_audio_cache(): void;
  audio_context_time(): number;
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

export function initCanvasKitWasm(): void {
  if (!wasmModule) throw new Error('WASM not initialized');
  wasmModule.init_canvaskit();
}

// ── 资源预加载（下载 + 元数据探测在 Rust 侧完成） ──

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
