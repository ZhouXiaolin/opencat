import type { CompositionInfo, ParsedResult } from './types';

type WasmModule = typeof import('../wasm/opencat_web.js');
let wasmModule: WasmModule | null = null;

export async function initWasm(): Promise<void> {
  if (wasmModule) return;
  const mod = await import('../wasm/opencat_web.js');
  // The default export is __wbg_init — it fetches & instantiates the .wasm binary
  await mod.default();
  wasmModule = mod;
}

export function parseJsonl(input: string): ParsedResult {
  if (!wasmModule) {
    return {
      composition: null,
      elements: [],
      elementCount: 0,
    };
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
