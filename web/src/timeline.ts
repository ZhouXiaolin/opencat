// ── Timeline segment computation ──
// Parses JSONL to extract scene/transition timing so we can compute
// per-scene local frames (matching the engine's ScriptFrameCtx::for_segment).
//
// Shared between preview (main.ts) and export (exporter.ts).

export interface TimelineSegment {
  type: 'scene' | 'transition';
  startFrame: number;
  duration: number;
}

let cachedSegments: TimelineSegment[] | null = null;
let cachedSegmentsJsonl: string | null = null;

export function computeTimelineSegments(jsonlContent: string): TimelineSegment[] {
  if (jsonlContent === cachedSegmentsJsonl && cachedSegments) return cachedSegments;

  const lines = jsonlContent.split('\n');
  const elementsById = new Map<string, { duration?: number; parentId?: string }>();
  const tlChildren = new Map<string, string[]>();
  const transitions: { parentId: string; from: string; to: string; duration: number }[] = [];

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    let obj: any;
    try { obj = JSON.parse(trimmed); } catch { continue; }

    if (obj.id) elementsById.set(obj.id, obj);
    if (obj.type === 'tl' && obj.id) tlChildren.set(obj.id, []);
    if (obj.type === 'transition') transitions.push(obj);
  }

  for (const [id, el] of elementsById) {
    if (el.parentId && tlChildren.has(el.parentId)) {
      tlChildren.get(el.parentId)!.push(id);
    }
  }

  const tlId = tlChildren.keys().next().value;
  if (!tlId) {
    cachedSegmentsJsonl = jsonlContent;
    cachedSegments = [];
    return [];
  }

  const childIds = tlChildren.get(tlId) ?? [];
  const transitionsByPair = new Map<string, any>();
  for (const t of transitions) {
    transitionsByPair.set(`${t.from}->${t.to}`, t);
  }

  const segments: TimelineSegment[] = [];
  let cursor = 0;

  for (let i = 0; i < childIds.length; i++) {
    const sceneId = childIds[i];
    const sceneEl = elementsById.get(sceneId);
    const sceneDuration = sceneEl?.duration ?? 0;

    segments.push({ type: 'scene', startFrame: cursor, duration: sceneDuration });
    cursor += sceneDuration;

    if (i < childIds.length - 1) {
      const nextId = childIds[i + 1];
      const trans = transitionsByPair.get(`${sceneId}->${nextId}`);
      const transDuration = trans?.duration ?? 0;
      if (transDuration > 0) {
        segments.push({ type: 'transition', startFrame: cursor, duration: transDuration });
        cursor += transDuration;
      }
    }
  }

  cachedSegmentsJsonl = jsonlContent;
  cachedSegments = segments;
  return segments;
}

/** Find the scene-local frame and scene duration for a global frame number. */
export function sceneFrameCtx(
  globalFrame: number,
  jsonlContent: string,
): { localFrame: number; sceneFrames: number } {
  const segments = computeTimelineSegments(jsonlContent);
  for (const seg of segments) {
    if (seg.type !== 'scene') continue;
    const end = seg.startFrame + seg.duration;
    if (globalFrame < end) {
      const local = Math.min(globalFrame - seg.startFrame, seg.duration - 1);
      return { localFrame: Math.max(0, local), sceneFrames: seg.duration };
    }
  }
  for (let i = segments.length - 1; i >= 0; i--) {
    if (segments[i].type === 'scene') {
      return { localFrame: segments[i].duration - 1, sceneFrames: segments[i].duration };
    }
  }
  return { localFrame: globalFrame, sceneFrames: 0 };
}

export function clearTimelineCache(): void {
  cachedSegments = null;
  cachedSegmentsJsonl = null;
}
