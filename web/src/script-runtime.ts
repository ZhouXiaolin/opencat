// ── Script Runtime (browser-native ctx API for animation scripts) ──
// Mirrors Rust QuickJS exposure to scripts

import { computeProgress, parseEasing, SpringConfig, animateValue } from './animator';

// ── Types ──

export interface NodeMutations {
  opacity?: number;
  width?: number;
  height?: number;
  bgColor?: string;
  textColor?: string;
  textPx?: number;
  // Temporary accumulators (flushed to transforms before serialization)
  x?: number;
  y?: number;
  scale?: number;
  rotate?: number;
  transforms?: TransformEntry[];
}

export interface TransformEntry {
  type: string;   // "translate", "translateX", "translateY", "scale", "rotate", etc.
  x?: number;
  y?: number;
  value?: number;
}

export interface CollectedMutations {
  mutations: Record<string, NodeMutations>;
  canvasMutations: Record<string, never>;
}

export interface AnimInput {
  duration?: number;
  delay?: number;
  clamp?: boolean;
  ease?: string;
  stagger?: number;
  repeat?: number;
  yoyo?: boolean;
  repeatDelay?: number;
}

export interface AnimOutput {
  opacity: number;
  y: number;
  scale: number;
  rotate: number;
  x: number;
}

interface KeyframeEntry {
  at: number;
  value: number;
  easing?: string;
}

// ── ScriptCtx ──

export class ScriptCtx {
  frame: number;
  totalFrames: number;
  currentFrame: number;
  sceneFrames: number;
  private mutations: Record<string, NodeMutations> = {};
  /** Track auto-increment IDs for splitText (deferred) / anonymous nodes */
  private nextId = 10000;

  constructor(frame: number, totalFrames: number, sceneFrames: number) {
    this.frame = frame;
    this.totalFrames = totalFrames;
    this.currentFrame = frame;
    this.sceneFrames = sceneFrames;
  }

  /**
   * Animate from one state to another.
   * Mirrors Rust ctx.fromTo() — computes value at current frame and stores as mutation.
   */
  fromTo(
    nodeIds: string | string[],
    from: Record<string, number | ((i: number) => number)>,
    to: Record<string, number | ((i: number) => number)>,
    opts: AnimInput = {},
  ): AnimOutput[] {
    const ids = Array.isArray(nodeIds) ? nodeIds : [nodeIds];
    const duration = opts.duration ?? this.sceneFrames;
    const delay = opts.delay ?? 0;
    const clamp = opts.clamp ?? false;
    const stagger = opts.stagger ?? 0;
    const repeat = opts.repeat ?? 0;
    const yoyo = opts.yoyo ?? false;
    const repeatDelay = opts.repeatDelay ?? 0;
    const { easing, spring } = parseEasing(opts.ease || 'ease');

    return ids.map((id, i) => {
      const staggeredDelay = delay + i * stagger;
      const progress = computeProgress(
        this.currentFrame, duration, staggeredDelay,
        easing, spring, clamp, repeat, yoyo, repeatDelay,
      );

      const result: AnimOutput = { opacity: 0, y: 0, scale: 1, rotate: 0, x: 0 };
      if (!this.mutations[id]) this.mutations[id] = {};
      const mut = this.mutations[id];

      for (const key of Object.keys(to)) {
        if (key === 'text') continue;
        const toVal = to[key];
        if (typeof toVal === 'function') continue; // skip function-valued props for now
        const fromRaw = from[key];
        const fromVal = typeof fromRaw === 'function' ? 0 : (fromRaw ?? 0);
        const val = fromVal + (toVal - fromVal) * progress;
        applyMutation(mut, key, val);
        (result as any)[key] = val;
      }

      return result;
    });
  }

  /**
   * Animate to a destination state (source is implicit 0).
   * Mirrors Rust ctx.to() — also handles keyframes.
   */
  to(
    nodeIds: string | string[],
    target: Record<string, any>,
    opts: AnimInput = {},
  ): AnimOutput[] {
    const ids = Array.isArray(nodeIds) ? nodeIds : [nodeIds];
    const duration = opts.duration ?? this.sceneFrames;
    const delay = opts.delay ?? 0;
    const clamp = opts.clamp ?? false;
    const stagger = opts.stagger ?? 0;
    const repeat = opts.repeat ?? 0;
    const yoyo = opts.yoyo ?? false;
    const repeatDelay = opts.repeatDelay ?? 0;
    const { easing, spring } = parseEasing(opts.ease || 'ease');

    // Check for keyframe-based animation
    const keyframes = target.keyframes as Record<string, KeyframeEntry[]> | undefined;

    return ids.map((id, i) => {
      const staggeredDelay = delay + i * stagger;
      const progress = computeProgress(
        this.currentFrame, duration, staggeredDelay,
        easing, spring, clamp, repeat, yoyo, repeatDelay,
      );

      const result: AnimOutput = { opacity: 0, y: 0, scale: 1, rotate: 0, x: 0 };
      if (!this.mutations[id]) this.mutations[id] = {};
      const mut = this.mutations[id];

      if (keyframes) {
        // Keyframe interpolation
        for (const prop of Object.keys(keyframes)) {
          const frames = keyframes[prop];
          if (!frames || frames.length === 0) continue;
          const val = interpolateKeyframes(frames, progress);
          applyMutation(mut, prop, val);
          (result as any)[prop] = val;
        }
      }

      // Handle direct properties (text, opacity, etc.) — simple animate to value
      for (const key of Object.keys(target)) {
        if (key === 'keyframes') continue;
        if (key === 'text') {
          // Typewriter: compute visible character count
          const text = String(target.text);
          const visibleLen = Math.min(text.length, Math.floor(progress * text.length));
          // text is stored as a string mutation — for now, skip numeric mutation
          continue;
        }
        const toVal = target[key];
        if (typeof toVal !== 'number') continue;
        const val = animateValue(
          this.currentFrame, duration, staggeredDelay,
          0, toVal, easing, spring, clamp, repeat, yoyo, repeatDelay,
        );
        applyMutation(mut, key, val);
        (result as any)[key] = val;
      }

      return result;
    });
  }

  /**
   * Get a node styler for direct property manipulation.
   */
  getNode(nodeId: string): NodeStyler {
    if (!this.mutations[nodeId]) {
      this.mutations[nodeId] = {};
    }
    return new NodeStyler(this.mutations[nodeId]);
  }

  /**
   * Create a timeline for chained/sequenced animations.
   * Mirrors Rust ctx.timeline().
   */
  timeline(): TimelineBuilder {
    return new TimelineBuilder(this);
  }

  /**
   * Split text into character/word elements.
   * Stub — returns a simple array of IDs (requires element tree access for full impl).
   */
  splitText(nodeId: string, _opts: { type?: string } = {}): string[] {
    // For now, return a single-element array as passthrough
    // Full implementation needs: find text node, split into graphemes/words,
    // create child elements with computed positions
    return [nodeId];
  }

  collectMutations(): CollectedMutations {
    // Flush transform accumulations
    for (const nodeId of Object.keys(this.mutations)) {
      const node = this.mutations[nodeId];
      const styler = new NodeStyler(node);
      styler.flushTransforms();
    }
    // Remove empty mutation entries
    const cleaned: Record<string, NodeMutations> = {};
    for (const [id, muts] of Object.entries(this.mutations)) {
      if (Object.keys(muts).length > 0) {
        cleaned[id] = muts;
      }
    }
    return { mutations: cleaned, canvasMutations: {} };
  }
}

// ── Timeline Builder ──

class TimelineBuilder {
  private ctx: ScriptCtx;
  private cursor = 0;

  constructor(ctx: ScriptCtx) {
    this.ctx = ctx;
  }

  fromTo(
    nodeIds: string | string[],
    from: Record<string, number>,
    to: Record<string, number>,
    opts: AnimInput | string = {},
  ): TimelineBuilder {
    let resolvedOpts: AnimInput;
    if (typeof opts === 'string') {
      // Relative offset like '-=12'
      resolvedOpts = {};
      if (opts.startsWith('-=')) {
        this.cursor += parseInt(opts.slice(2), 10) || 0;
      } else if (opts.startsWith('+=')) {
        this.cursor += parseInt(opts.slice(2), 10) || 0;
      }
    } else {
      resolvedOpts = opts;
    }

    // Apply the animation with a delay offset by the cursor
    this.ctx.fromTo(nodeIds, from, to, {
      ...resolvedOpts,
      delay: (resolvedOpts.delay ?? 0) + this.cursor,
    });

    // Advance cursor by duration
    const dur = resolvedOpts.duration ?? this.ctx.sceneFrames;
    this.cursor += dur;
    return this;
  }
}

// ── NodeStyler ──

class NodeStyler {
  constructor(private node: NodeMutations) {}

  opacity(v: number): NodeStyler { this.node.opacity = v; return this; }
  width(v: number): NodeStyler { this.node.width = v; return this; }
  height(v: number): NodeStyler { this.node.height = v; return this; }
  bgColor(c: string): NodeStyler { this.node.bgColor = c; return this; }
  textColor(c: string): NodeStyler { this.node.textColor = c; return this; }
  textPx(px: number): NodeStyler { this.node.textPx = px; return this; }

  // Transform-based properties accumulate
  translateX(v: number): NodeStyler { this.node.x = v; return this; }
  x(v: number): NodeStyler { this.node.x = v; return this; }
  translateY(v: number): NodeStyler { this.node.y = v; return this; }
  y(v: number): NodeStyler { this.node.y = v; return this; }
  scale(v: number): NodeStyler { this.node.scale = v; return this; }
  rotate(v: number): NodeStyler { this.node.rotate = v; return this; }

  private addTransform(t: TransformEntry): void {
    if (!this.node.transforms) this.node.transforms = [];
    this.node.transforms.push(t);
  }

  flushTransforms(): void {
    if (this.node.scale !== undefined) {
      this.addTransform({ type: 'scale', value: this.node.scale });
      delete this.node.scale;
    }
    if (this.node.x !== undefined) {
      this.addTransform({ type: 'translateX', value: this.node.x });
      delete this.node.x;
    }
    if (this.node.y !== undefined) {
      this.addTransform({ type: 'translateY', value: this.node.y });
      delete this.node.y;
    }
    if (this.node.rotate !== undefined) {
      this.addTransform({ type: 'rotate', value: this.node.rotate });
      delete this.node.rotate;
    }
  }
}

// ── Helpers ──

/**
 * Store a mutation value on a node, mapping animation property names
 * to mutation field names.
 */
function applyMutation(mut: NodeMutations, key: string, val: number): void {
  switch (key) {
    case 'opacity':
      mut.opacity = val;
      break;
    case 'x':
      mut.x = val;
      break;
    case 'y':
      mut.y = val;
      break;
    case 'scale':
      mut.scale = val;
      break;
    case 'rotate':
      mut.rotate = val;
      break;
    case 'width':
      mut.width = val;
      break;
    case 'height':
      mut.height = val;
      break;
    case 'textPx':
      mut.textPx = val;
      break;
    // color/textColor handled via getNode().textColor()
    // bgColor handled via getNode().bgColor()
  }
}

/**
 * Interpolate a keyframe array at a given progress [0..1].
 */
function interpolateKeyframes(frames: KeyframeEntry[], progress: number): number {
  if (frames.length === 0) return 0;
  if (frames.length === 1) return frames[0].value;
  if (progress <= frames[0].at) return frames[0].value;
  if (progress >= frames[frames.length - 1].at) return frames[frames.length - 1].value;

  for (let i = 0; i < frames.length - 1; i++) {
    const a = frames[i];
    const b = frames[i + 1];
    if (progress >= a.at && progress <= b.at) {
      const t = (progress - a.at) / (b.at - a.at);
      return a.value + (b.value - a.value) * t;
    }
  }
  return frames[frames.length - 1].value;
}

// ── Entry Functions ──

export function createContext(
  frame: number,
  totalFrames: number,
  sceneFrames: number,
): ScriptCtx {
  return new ScriptCtx(frame, totalFrames, sceneFrames);
}

export function runScript(
  ctx: ScriptCtx,
  scriptSource: string,
): void {
  try {
    const fn = new Function('ctx', scriptSource);
    fn(ctx);
  } catch (err) {
    console.error('Script execution error:', err);
    throw err;
  }
}
