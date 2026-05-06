// ── Script Engine ──
// Evaluates the shared core JS runtime files (node_style.js, canvas_api.js,
// animation/*.js) in the browser, using NativeBridge to collect mutations.
//
// This replaces the old script-runtime.ts which manually reimplemented
// a subset of the animation API. Now we use the SAME JS code as the
// desktop engine, evaluated via the browser's native JS engine.
//
// Usage:
//   const engine = new ScriptEngine();
//   engine.init();                          // once on startup
//   engine.setFrameCtx(frame, total, scene); // per frame
//   engine.runScript(userSource);          // per script element
//   const json = engine.collectJson();     // pass to WASM buildFrame

import { NativeBridge } from './native-bridge';

// Import core JS runtime files as raw strings (via Vite ?raw)
import NODE_STYLE_RUNTIME from '../script-runtime/node_style.js?raw';
import CANVAS_API_RUNTIME from '../script-runtime/canvas_api.js?raw';
import ANIMATION_BOOTSTRAP from '../script-runtime/animation/bootstrap.js?raw';
import ANIMATION_CORE from '../script-runtime/animation/core.js?raw';
import ANIMATION_FACADE from '../script-runtime/animation/facade.js?raw';
import PLUGIN_STYLE_PROPS from '../script-runtime/animation/plugins/style_props.js?raw';
import PLUGIN_COLOR from '../script-runtime/animation/plugins/color.js?raw';
import PLUGIN_TEXT from '../script-runtime/animation/plugins/text.js?raw';
import PLUGIN_SPLIT_TEXT from '../script-runtime/animation/plugins/split_text.js?raw';
import PLUGIN_MOTION_PATH from '../script-runtime/animation/plugins/motion_path.js?raw';
import PLUGIN_MORPH_SVG from '../script-runtime/animation/plugins/morph_svg.js?raw';
import PLUGIN_UTILS from '../script-runtime/animation/plugins/utils.js?raw';

export class ScriptEngine {
  private bridge = new NativeBridge();
  private initialized = false;

  /** Initialize once: init wasm bridge, inject native globals and evaluate core JS runtimes. */
  async init(): Promise<void> {
    if (this.initialized) return;
    this.initialized = true;

    // 1. Init wasm bridge (creates WebMutationRecorder)
    await this.bridge.init();

    // 2. Inject native functions as window globals
    this.bridge.injectGlobals();

    // 3. Evaluate animation runtime in order
    //    These files are IIFE-wrapped and build on each other:
    //    bootstrap -> core -> plugins -> facade
    eval(ANIMATION_BOOTSTRAP);   // globalThis.__opencatAnimation
    eval(ANIMATION_CORE);       // runtime.core (timeline engine)
    eval(PLUGIN_STYLE_PROPS);   // register style properties
    eval(PLUGIN_COLOR);         // color interpolation
    eval(PLUGIN_TEXT);          // text content animation
    eval(PLUGIN_SPLIT_TEXT);    // per-character/word splitting
    eval(PLUGIN_MOTION_PATH);   // motion path following
    eval(PLUGIN_MORPH_SVG);     // SVG morphing
    eval(PLUGIN_UTILS);         // utility functions
    eval(ANIMATION_FACADE);     // ctx.set/ctx.animate/ctx.timeline/flushTimelines

    // 4. Evaluate node style runtime
    eval(NODE_STYLE_RUNTIME);   // ctx.getNode(id).prop(val) chainable API

    // 5. Evaluate canvas API runtime
    eval(CANVAS_API_RUNTIME);   // ctx.canvas(id).fillRect(...) API
  }

  /** Set the frame context for the upcoming script evaluation. */
  setFrameCtx(frame: number, totalFrames: number, sceneFrames: number): void {
    // Reset bridge state for the new frame
    this.bridge.reset();
    this.bridge.setFrameCtx(frame, totalFrames, sceneFrames);

    // Update ctx properties for user scripts
    (window as any).ctx = (window as any).ctx || {};
    const c = (window as any).ctx;
    c.frame = frame;
    c.totalFrames = totalFrames;
    c.currentFrame = frame;
    c.sceneFrames = sceneFrames;
  }

  /** Evaluate a user animation script source. */
  runScript(source: string): void {
    try {
      const fn = new Function('ctx', source);
      fn((window as any).ctx);
    } catch (err) {
      console.error('Script execution error:', err);
      throw err;
    }
  }

  /** Collect accumulated mutations as JSON for WASM PrecomputedScriptHost. */
  collectJson(): string {
    return this.bridge.collectJson();
  }

  /** Shutdown: remove native globals from window. */
  destroy(): void {
    this.bridge.removeGlobals();
    this.initialized = false;
  }
}

// ── Backward-compatible exports for main.ts ──

let sharedEngine: ScriptEngine | null = null;

/** Get or create the shared ScriptEngine singleton. Call init() separately. */
export function getScriptEngine(): ScriptEngine {
  if (!sharedEngine) {
    sharedEngine = new ScriptEngine();
  }
  return sharedEngine;
}

/** Create a per-frame script context (backward-compatible wrapper). */
export function createContext(
  frame: number,
  totalFrames: number,
  sceneFrames: number,
): { frame: number; totalFrames: number; sceneFrames: number; collectMutations: () => any } {
  const engine = getScriptEngine();
  engine.setFrameCtx(frame, totalFrames, sceneFrames);

  const ctx = (window as any).ctx;
  return {
    frame,
    totalFrames,
    sceneFrames,
    collectMutations() {
      return JSON.parse(engine.collectJson());
    },
  };
}

/** Run a user script (backward-compatible wrapper). */
export function runScript(
  ctx: any,
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
