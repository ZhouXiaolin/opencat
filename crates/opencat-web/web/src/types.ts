// ── Composition ──

export interface CompositionInfo {
  width: number;
  height: number;
  fps: number;
  frames: number;
}

export interface JsonlFile {
  name: string;
  path: string;
}

export interface ResourceMeta {
  kind: 'image' | 'video' | 'audio' | 'icon';
  width?: number;
  height?: number;
  durationSecs?: number;
}
