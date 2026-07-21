import type { AudioPlan, AudioPlanSegment } from 'opencat.js';

export interface PlaybackPosition {
  frame: number;
  loopIndex: number;
}

export interface AudioPlaybackWindow {
  /** Offset inside the decoded audio asset (seconds from clip start). */
  offsetSecs: number;
  /** How long to play from that offset (seconds). */
  durationSecs: number;
}

export function playbackPosition(
  startFrame: number,
  elapsedSecs: number,
  fps: number,
  totalFrames: number,
): PlaybackPosition {
  const frames = Math.max(1, Math.floor(totalFrames));
  const clampedStart = Math.min(Math.max(0, Math.floor(startFrame)), frames - 1);
  const elapsedFrames = Math.max(0, Math.floor(elapsedSecs * Math.max(1, fps)));
  const absoluteFrame = clampedStart + elapsedFrames;
  const frame = absoluteFrame % frames;
  const loopIndex = Math.floor(absoluteFrame / frames);
  return { frame, loopIndex };
}

/** Composition-time window from the current frame to the end of the composition. */
export function audioPlaybackWindow(
  frame: number,
  fps: number,
  totalFrames: number,
): AudioPlaybackWindow {
  const safeFps = Math.max(1, fps);
  const frames = Math.max(1, Math.floor(totalFrames));
  const clampedFrame = Math.min(Math.max(0, Math.floor(frame)), frames - 1);
  const totalDuration = frames / safeFps;
  const offsetSecs = clampedFrame / safeFps;
  return {
    offsetSecs,
    durationSecs: Math.max(0, totalDuration - offsetSecs),
  };
}

/**
 * Intersection of a core AudioPlan segment with the composition playback
 * window starting at `compositionTimeSecs`. Returns how much of the *source*
 * clip to play (offset inside the asset + duration), or null if no overlap.
 *
 * Segment ranges are composition-timeline times from core; source offset is
 * relative to the start of the decoded asset (clip plays from t=0 at segment start).
 */
export function segmentPlaybackAt(
  segment: AudioPlanSegment,
  compositionTimeSecs: number,
  compositionEndSecs: number,
): AudioPlaybackWindow | null {
  const segStart = segment.startMicros / 1_000_000;
  const segEnd = segment.endMicros / 1_000_000;
  const playStart = Math.max(compositionTimeSecs, segStart);
  const playEnd = Math.min(compositionEndSecs, segEnd);
  if (playEnd <= playStart) return null;
  return {
    offsetSecs: playStart - segStart,
    durationSecs: playEnd - playStart,
  };
}

/** Active segments at composition time (for preview scheduling). */
export function activeSegmentsAt(
  plan: AudioPlan,
  compositionTimeSecs: number,
  compositionEndSecs: number,
): Array<{ assetId: string; window: AudioPlaybackWindow }> {
  const out: Array<{ assetId: string; window: AudioPlaybackWindow }> = [];
  for (const seg of plan.segments) {
    const window = segmentPlaybackAt(seg, compositionTimeSecs, compositionEndSecs);
    if (window) out.push({ assetId: seg.assetId, window });
  }
  return out;
}

/**
 * For export: map a composition-time slice `[startSecs, startSecs+durationSecs)`
 * onto source samples for one segment. Returns null if no overlap.
 * `offsetSecs` is the position inside the decoded asset; duration is the
 * overlapping composition duration (hosts sample that length from the asset).
 */
export function segmentExportSlice(
  segment: AudioPlanSegment,
  startSecs: number,
  durationSecs: number,
): AudioPlaybackWindow | null {
  return segmentPlaybackAt(segment, startSecs, startSecs + durationSecs);
}
