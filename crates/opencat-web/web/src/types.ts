// ── Composition ──

export interface CompositionInfo {
  width: number;
  height: number;
  fps: number;
  duration: number;
}

/** One scheduled audio clip from core AudioPlan (issue #18). */
export interface AudioPlanSegment {
  assetId: string;
  startMicros: number;
  endMicros: number;
  durationMicros: number;
}

export interface AudioPlan {
  segments: AudioPlanSegment[];
}

export function compositionFrameCount(comp: CompositionInfo): number {
  return Math.max(1, Math.ceil(comp.duration * Math.max(1, comp.fps)));
}

export interface CompositionFile {
  name: string;
  path: string;
}

export interface ResourceMeta {
  kind: 'image' | 'video' | 'audio' | 'lottie' | 'icon';
  width?: number;
  height?: number;
  durationSecs?: number;
  lottieFps?: number;
  lottieInFrame?: number;
  lottieOutFrame?: number;
  lottieDependencies?: string[];
}
