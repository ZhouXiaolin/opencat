import type { WasmFaacEncoder } from './media/faac-audio-encoder';
import { loadDefaultFontsIntoWasm } from './fonts';

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
  load_resource_bytes(path: string, name: string): Uint8Array | undefined;
  get_skottie_bundle_assets(bundle_id: string): Record<string, Uint8Array>;
  clear_blobs(): void;
  blob_count(): number;
  set_asset_reader(reader: AssetReader): void;
  clear_asset_reader(): void;
};

export type AssetReaderResult = Uint8Array | ArrayBuffer | number[];
export type AssetReader = (path: string) => AssetReaderResult | Promise<AssetReaderResult>;

export interface WebRendererInstance {
  open_design(compositionSource: string): Promise<string>;
  build_frame_ir(frame: number): Uint8Array;
  prepare_frame(frame: number): string;
  load_default_fonts(sans_sc: Uint8Array, color_emoji: Uint8Array): void;
  load_font_data(bytes: Uint8Array): void;
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
  const { bindResourceProviderApi } = await import('./resource-provider');
  bindResourceProviderApi({
    load_resource_bytes: (path, name) => wasmModule!.load_resource_bytes(path, name),
    get_skottie_bundle_assets: (bundleId) =>
      wasmModule!.get_skottie_bundle_assets(bundleId),
  });
  await loadDefaultFontsIntoWasm(renderer);
}

// ── 资源预加载（下载 + 元数据探测在 Rust 侧完成） ──

export async function preloadAssets(compositionSource: string): Promise<string> {
  if (!wasmModule) throw new Error('WASM not initialized');
  return await wasmModule.preload_assets(compositionSource);
}

// ── Host-owned persistent pipeline (issue #8) ──
// `openDesign` fetches all declared resources, builds the prepared catalog,
// hydrates captions, injects the font database, and opens the persistent core
// pipeline. Subsequent `build_frame_ir(frame)` calls render against it.

export async function openDesign(compositionSource: string): Promise<string> {
  if (!renderer) throw new Error('WASM renderer not initialized');
  return await renderer.open_design(compositionSource);
}

export function getBlobBytes(assetId: string): Uint8Array | undefined {
  if (!wasmModule) return undefined;
  return wasmModule.get_blob_bytes(assetId);
}

/** Same bytes as getBlobBytes, via Skottie protocol (`opencat`, assetId). */
export function loadResourceBytes(path: string, name: string): Uint8Array | undefined {
  if (!wasmModule) return undefined;
  return wasmModule.load_resource_bytes(path, name);
}

export function getSkottieBundleAssets(
  bundleId: string,
): Record<string, Uint8Array> {
  if (!wasmModule) return {};
  return wasmModule.get_skottie_bundle_assets(bundleId);
}

export function clearBlobs(): void {
  if (!wasmModule) return;
  wasmModule.clear_blobs();
}

export function blobCount(): number {
  if (!wasmModule) return 0;
  return wasmModule.blob_count();
}

export function setAssetReader(reader: AssetReader): void {
  if (!wasmModule) throw new Error('WASM not initialized');
  wasmModule.set_asset_reader(reader);
}

export function clearAssetReader(): void {
  if (!wasmModule) return;
  wasmModule.clear_asset_reader();
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
