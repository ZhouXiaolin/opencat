import type { CompositionInfo, ParsedResult, ResourceRequests } from './types';

type WasmModule = {
  default(): Promise<void>;
  parse_jsonl(input: string): string;
  get_composition_info(input: string): string;
  collect_resources_json(input: string): string;
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
