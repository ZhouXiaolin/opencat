import type { WasmFaacEncoder } from './media/faac-audio-encoder';

type WasmModule = {
  default(): Promise<void>;
  WebRenderer: {
    new(): WebRendererInstance;
  };
  WebFaacEncoder: {
    new(sample_rate: number, channels: number, bit_rate: number): WasmFaacEncoderInstance;
  };
  preload_assets(compositionSource: string): Promise<string>;
  get_blob_bytes(asset_id: string): Uint8Array | undefined;
  clear_blobs(): void;
  blob_count(): number;
};

export interface WebRendererInstance {
  build_frame_ir(compositionSource: string, frame: number, resources_json: string): Uint8Array;
  inject_video_frame(asset_id: string, frame: number, rgba: Uint8Array, width: number, height: number): void;
  clear_video_cache(asset_id: string): void;
  plan_video_frames(compositionSource: string, frame: number, resources_json: string): string;
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

interface WasmFaacEncoderInstance {
  readonly input_samples: number;
  readonly audio_specific_config: Uint8Array;
  encode_f32_interleaved(samples: Float32Array): Uint8Array[];
  flush(): Uint8Array[];
  free(): void;
}

let wasmModule: WasmModule | null = null;
let renderer: WebRendererInstance | null = null;
let configuredWasmBaseUrl: string | undefined;

export function setWasmBaseUrl(url: string): void {
  configuredWasmBaseUrl = url.endsWith('/') ? url : url + '/';
}

export async function initWasm(wasmBaseUrl?: string): Promise<void> {
  if (wasmModule) return;
  const base = wasmBaseUrl || configuredWasmBaseUrl || '';
  const mod = await import(/* @vite-ignore */ `${base}opencat_web.js`);
  await mod.default();
  wasmModule = mod as unknown as WasmModule;
  renderer = new wasmModule.WebRenderer();
}

// ── 资源预加载（下载 + 元数据探测在 Rust 侧完成） ──

export async function preloadAssets(compositionSource: string): Promise<string> {
  if (!wasmModule) throw new Error('WASM not initialized');
  return await wasmModule.preload_assets(compositionSource);
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

export function createWasmFaacEncoder(config: AudioEncoderConfig): WasmFaacEncoder {
  if (!wasmModule) throw new Error('WASM not initialized');
  const encoder = new wasmModule.WebFaacEncoder(
    config.sampleRate,
    config.numberOfChannels,
    config.bitrate ?? 128_000,
  );
  return {
    get inputSamples() {
      return encoder.input_samples;
    },
    get audioSpecificConfig() {
      return encoder.audio_specific_config;
    },
    encodeF32Interleaved(samples) {
      return encoder.encode_f32_interleaved(samples);
    },
    flush() {
      return encoder.flush();
    },
    free() {
      encoder.free();
    },
  };
}
