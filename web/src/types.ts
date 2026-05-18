// ── Composition ──

export interface CompositionInfo {
  width: number;
  height: number;
  fps: number;
  frames: number;
}

// ── Parsed JSONL (from WASM) ──

export interface ParsedElement {
  type: string;
  id?: string;
  parentId?: string | null;
  className?: string | null;
  text?: string;
  d?: string;
  path?: string;
  src?: string;
  icon?: string;
  from?: string;
  to?: string;
  effect?: string;
  duration?: number;
  [key: string]: unknown;
}

export interface ParsedResult {
  composition: CompositionInfo | null;
  elements: ParsedElement[];
  elementCount: number;
}

export interface JsonlFile {
  name: string;
  path: string;
}

// ── Resource Management ──

export interface ResourceRequests {
  images: string[];
  videos: string[];
  audios: string[];
  icons: string[];
}

export interface LoadedImage {
  path: string;
  ckImage: any;
  width: number;
  height: number;
}

export interface VideoFrame {
  data: Uint8Array;
  width: number;
  height: number;
}

export interface ResourceMeta {
  kind: 'image' | 'video' | 'audio' | 'icon';
  width?: number;
  height?: number;
  durationSecs?: number;
}
