/**
 * Skottie-aligned resource access after preload_assets().
 *
 * Flat OpenCat assets: loadResourceBytes('opencat', assetId)
 * Lottie bundle deps: getSkottieBundleAssets('lottie:hero')
 */

type WasmResourceApi = {
  load_resource_bytes(path: string, name: string): Uint8Array | undefined;
  get_skottie_bundle_assets(bundle_id: string): Record<string, Uint8Array>;
};

function wasm(): WasmResourceApi {
  const m = (globalThis as { __opencatWasm?: WasmResourceApi }).__opencatWasm;
  if (!m) {
    throw new Error('WASM module not initialized');
  }
  return m;
}

/** Register wasm exports (called from wasm.ts after init). */
export function bindResourceProviderApi(api: WasmResourceApi): void {
  (globalThis as { __opencatWasm?: WasmResourceApi }).__opencatWasm = api;
}

export function loadResourceBytes(path: string, name: string): Uint8Array | undefined {
  return wasm().load_resource_bytes(path, name);
}

/** CanvasKit MakeManagedAnimation(json, assets) — dependency map only. */
export function getSkottieBundleAssets(
  bundleId: string,
): Record<string, Uint8Array> {
  return wasm().get_skottie_bundle_assets(bundleId);
}