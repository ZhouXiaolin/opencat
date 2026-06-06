// ── Composition ──

export interface CompositionInfo {
  width: number;
  height: number;
  fps: number;
  duration: number;
}

export function compositionFrameCount(comp: CompositionInfo): number {
  return Math.max(1, Math.ceil(comp.duration * Math.max(1, comp.fps)));
}

export interface CompositionFile {
  name: string;
  path: string;
}

export interface ResourceMeta {
  kind: 'image' | 'video' | 'audio' | 'icon';
  width?: number;
  height?: number;
  durationSecs?: number;
}
