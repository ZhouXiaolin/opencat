// ── Script Runtime (browser-native ctx API for animation scripts) ──
// Mirrors Rust QuickJS exposure to scripts

import { computeProgress, parseEasing, SpringConfig } from './animator';

// ── Types ──

export interface NodeMutations {
  opacity?: number;
  width?: number;
  height?: number;
  bgColor?: string;
  textColor?: string;
  textPx?: number;
  // Temporary accumulators (flushed to transforms before serialization)
  y?: number;
  scale?: number;
  rotate?: number;
  transforms?: TransformEntry[];
}

export interface TransformEntry {
  type: string;   // "translate", "scale", "rotate", etc.
  x?: number;
  y?: number;
  value?: number;
}

export interface CollectedMutations {
  mutations: Record<string, NodeMutations>;
  canvasMutations: Record<string, never>;  // Required by Rust StyleMutations (camelCase via serde rename_all)
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

// ── ScriptCtx ──

export class ScriptCtx {
  frame: number;
  totalFrames: number;
  currentFrame: number;
  sceneFrames: number;
  private mutations: Record<string, NodeMutations> = {};

  constructor(frame: number, totalFrames: number, sceneFrames: number) {
    this.frame = frame;
    this.totalFrames = totalFrames;
    this.currentFrame = frame;
    this.sceneFrames = sceneFrames;
  }

  fromTo(
    nodeIds: string | string[],
    from: Record<string, number>,
    to: Record<string, number>,
    opts: AnimInput = {},
  ): AnimOutput[] {
    const ids = Array.isArray(nodeIds) ? nodeIds : [nodeIds];
    const {
      duration = this.sceneFrames || this.totalFrames,
      delay = 0,
      clamp = false,
      ease,
      stagger = 0,
      repeat = 0,
      yoyo = false,
      repeatDelay = 0,
    } = opts;

    const { easing, spring } = parseEasing(ease || 'ease');

    return ids.map((_id, i) => {
      const staggeredDelay = delay + i * stagger;
      const progress = computeProgress(
        this.currentFrame,
        duration,
        staggeredDelay,
        easing,
        spring,
        clamp,
        repeat,
        yoyo,
        repeatDelay,
      );

      const result: AnimOutput = { opacity: 0, y: 0, scale: 1, rotate: 0, x: 0 };
      for (const [key, toVal] of Object.entries(to)) {
        const fromVal = from[key] ?? 0;
        const val = fromVal + (toVal - fromVal) * progress;
        (result as any)[key] = val;
      }
      return result;
    });
  }

  getNode(nodeId: string): NodeStyler {
    if (!this.mutations[nodeId]) {
      this.mutations[nodeId] = {};
    }
    return new NodeStyler(this.mutations[nodeId]);
  }

  collectMutations(): CollectedMutations {
    // Flush transform accumulations
    for (const nodeId of Object.keys(this.mutations)) {
      const styler = new NodeStyler(this.mutations[nodeId]);
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
