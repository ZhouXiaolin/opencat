export interface PlaybackPosition {
  frame: number;
  loopIndex: number;
}

export interface AudioPlaybackWindow {
  offsetSecs: number;
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
