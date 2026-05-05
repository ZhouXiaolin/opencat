// ── Native Bridge ──
// Browser-native implementation of the __record_*, __canvas_*, __animate_*,
// and __text_source_* functions that the core JS runtime files call into.
//
// This mirrors the engine's QuickJS native bindings but runs in the browser's
// JS engine. Mutations are accumulated in-memory and serialized to JSON
// matching Rust's StyleMutations format for WASM's PrecomputedScriptHost.

import {
  computeProgress,
  animateValue,
  parseEasing,
} from './animator';

// ── Types matching Rust NodeStyleMutations ──

interface TransformEntry {
  type: string;
  x?: number;
  y?: number;
  value?: number;
}

interface NodeStyleRecord {
  position?: string;
  insetLeft?: number;
  insetTop?: number;
  insetRight?: number;
  insetBottom?: number;
  width?: number;
  height?: number;
  padding?: number;
  paddingX?: number;
  paddingY?: number;
  margin?: number;
  marginX?: number;
  marginY?: number;
  flexDirection?: string;
  justifyContent?: string;
  alignItems?: string;
  gap?: number;
  flexGrow?: number;
  opacity?: number;
  bgColor?: string;
  fillColor?: string;
  strokeColor?: string;
  strokeWidth?: number;
  strokeDasharray?: number;
  strokeDashoffset?: number;
  borderRadius?: number;
  borderWidth?: number;
  borderTopWidth?: number;
  borderRightWidth?: number;
  borderBottomWidth?: number;
  borderLeftWidth?: number;
  borderColor?: string;
  borderStyle?: string;
  objectFit?: string;
  transforms: TransformEntry[];
  textColor?: string;
  textPx?: number;
  fontWeight?: number;
  letterSpacing?: number;
  textAlign?: string;
  lineHeight?: number;
  boxShadow?: string;
  boxShadowColor?: string;
  insetShadow?: string;
  insetShadowColor?: string;
  dropShadow?: string;
  dropShadowColor?: string;
  textContent?: string;
  svgPath?: string;
}

interface CollectedOutput {
  mutations: Record<string, any>;
  canvasMutations: Record<string, any>;
}

interface AnimateState {
  duration: number;
  delay: number;
  clamp: boolean;
  easing: string;
  repeat: number;
  yoyo: boolean;
  repeatDelay: number;
  fromValues: Record<string, number>;
  toValues: Record<string, number>;
}

// ── NativeBridge class ──

export class NativeBridge {
  private styles = new Map<string, NodeStyleRecord>();
  private canvases = new Map<string, any[]>();
  private textSources = new Map<string, string>();
  private animateStates = new Map<number, AnimateState>();
  private nextAnimateHandle = 1;

  /** Current frame context (set per-frame before script evaluation). */
   frame = 0;
   totalFrames = 0;
   sceneFrames = 0;

  setFrameCtx(frame: number, totalFrames: number, sceneFrames: number): void {
    this.frame = frame;
    this.totalFrames = totalFrames;
    this.sceneFrames = sceneFrames;
  }

  // ── Node style recorders ── (62 methods, mirrors node_style.rs bindings)

  private getStyle(id: string): NodeStyleRecord {
    if (!this.styles.has(id)) {
      this.styles.set(id, { transforms: [] });
    }
    return this.styles.get(id)!;
  }

  recordOpacity(id: string, v: number): void {
    this.getStyle(id).opacity = v;
  }
  recordTranslateX(id: string, v: number): void {
    this.getStyle(id).transforms.push({ type: 'translateX', value: v });
  }
  recordTranslateY(id: string, v: number): void {
    this.getStyle(id).transforms.push({ type: 'translateY', value: v });
  }
  recordTranslate(id: string, x: number, y: number): void {
    this.getStyle(id).transforms.push({ type: 'translate', x, y });
  }
  recordScale(id: string, v: number): void {
    this.getStyle(id).transforms.push({ type: 'scale', value: v });
  }
  recordScaleX(id: string, v: number): void {
    this.getStyle(id).transforms.push({ type: 'scaleX', value: v });
  }
  recordScaleY(id: string, v: number): void {
    this.getStyle(id).transforms.push({ type: 'scaleY', value: v });
  }
  recordRotate(id: string, v: number): void {
    this.getStyle(id).transforms.push({ type: 'rotateDeg', value: v });
  }
  recordSkewX(id: string, v: number): void {
    this.getStyle(id).transforms.push({ type: 'skewXDeg', value: v });
  }
  recordSkewY(id: string, v: number): void {
    this.getStyle(id).transforms.push({ type: 'skewYDeg', value: v });
  }
  recordSkew(id: string, xDeg: number, yDeg: number): void {
    this.getStyle(id).transforms.push({ type: 'skewDeg', x: xDeg, y: yDeg });
  }
  recordPosition(id: string, v: string): void {
    this.getStyle(id).position = v;
  }
  recordLeft(id: string, v: number): void {
    this.getStyle(id).insetLeft = v;
  }
  recordTop(id: string, v: number): void {
    this.getStyle(id).insetTop = v;
  }
  recordRight(id: string, v: number): void {
    this.getStyle(id).insetRight = v;
  }
  recordBottom(id: string, v: number): void {
    this.getStyle(id).insetBottom = v;
  }
  recordWidth(id: string, v: number): void {
    this.getStyle(id).width = v;
  }
  recordHeight(id: string, v: number): void {
    this.getStyle(id).height = v;
  }
  recordPadding(id: string, v: number): void {
    this.getStyle(id).padding = v;
  }
  recordPaddingX(id: string, v: number): void {
    this.getStyle(id).paddingX = v;
  }
  recordPaddingY(id: string, v: number): void {
    this.getStyle(id).paddingY = v;
  }
  recordMargin(id: string, v: number): void {
    this.getStyle(id).margin = v;
  }
  recordMarginX(id: string, v: number): void {
    this.getStyle(id).marginX = v;
  }
  recordMarginY(id: string, v: number): void {
    this.getStyle(id).marginY = v;
  }
  recordFlexDirection(id: string, v: string): void {
    this.getStyle(id).flexDirection = v;
  }
  recordJustifyContent(id: string, v: string): void {
    this.getStyle(id).justifyContent = v;
  }
  recordAlignItems(id: string, v: string): void {
    this.getStyle(id).alignItems = v;
  }
  recordGap(id: string, v: number): void {
    this.getStyle(id).gap = v;
  }
  recordFlexGrow(id: string, v: number): void {
    this.getStyle(id).flexGrow = v;
  }
  recordBg(id: string, v: string): void {
    this.getStyle(id).bgColor = v;
  }
  recordBorderRadius(id: string, v: number): void {
    this.getStyle(id).borderRadius = v;
  }
  recordBorderWidth(id: string, v: number): void {
    this.getStyle(id).borderWidth = v;
  }
  recordBorderTopWidth(id: string, v: number): void {
    this.getStyle(id).borderTopWidth = v;
  }
  recordBorderRightWidth(id: string, v: number): void {
    this.getStyle(id).borderRightWidth = v;
  }
  recordBorderBottomWidth(id: string, v: number): void {
    this.getStyle(id).borderBottomWidth = v;
  }
  recordBorderLeftWidth(id: string, v: number): void {
    this.getStyle(id).borderLeftWidth = v;
  }
  recordBorderStyle(id: string, v: string): void {
    this.getStyle(id).borderStyle = v;
  }
  recordBorderColor(id: string, v: string): void {
    this.getStyle(id).borderColor = v;
  }
  recordStrokeWidth(id: string, v: number): void {
    this.getStyle(id).strokeWidth = v;
  }
  recordStrokeColor(id: string, v: string): void {
    this.getStyle(id).strokeColor = v;
  }
  recordFillColor(id: string, v: string): void {
    this.getStyle(id).fillColor = v;
  }
  recordObjectFit(id: string, v: string): void {
    this.getStyle(id).objectFit = v;
  }
  recordTextColor(id: string, v: string): void {
    this.getStyle(id).textColor = v;
  }
  recordTextSize(id: string, v: number): void {
    this.getStyle(id).textPx = v;
  }
  recordFontWeight(id: string, v: number): void {
    this.getStyle(id).fontWeight = v;
  }
  recordLetterSpacing(id: string, v: number): void {
    this.getStyle(id).letterSpacing = v;
  }
  recordTextAlign(id: string, v: string): void {
    this.getStyle(id).textAlign = v;
  }
  recordLineHeight(id: string, v: number): void {
    this.getStyle(id).lineHeight = v;
  }
  recordShadow(id: string, v: string): void {
    this.getStyle(id).boxShadow = v;
  }
  recordShadowColor(id: string, v: string): void {
    this.getStyle(id).boxShadowColor = v;
  }
  recordInsetShadow(id: string, v: string): void {
    this.getStyle(id).insetShadow = v;
  }
  recordInsetShadowColor(id: string, v: string): void {
    this.getStyle(id).insetShadowColor = v;
  }
  recordDropShadow(id: string, v: string): void {
    this.getStyle(id).dropShadow = v;
  }
  recordDropShadowColor(id: string, v: string): void {
    this.getStyle(id).dropShadowColor = v;
  }
  recordTextContent(id: string, v: string): void {
    this.getStyle(id).textContent = v;
  }
  recordStrokeDasharray(id: string, v: number): void {
    this.getStyle(id).strokeDasharray = v;
  }
  recordStrokeDashoffset(id: string, v: number): void {
    this.getStyle(id).strokeDashoffset = v;
  }
  recordSvgPath(id: string, v: string): void {
    this.getStyle(id).svgPath = v;
  }

  // ── Canvas recorders ── (stub — accumulated for future use)
  // The canvas_api.js runtime calls these. For now, accumulate commands.

  private getCanvas(id: string): any[] {
    if (!this.canvases.has(id)) {
      this.canvases.set(id, []);
    }
    return this.canvases.get(id)!;
  }

  canvasFillRect(id: string, x: number, y: number, w: number, h: number): void {
    this.getCanvas(id).push({ type: 'fillRect', x, y, width: w, height: h });
  }
  canvasStrokeRect(id: string, x: number, y: number, w: number, h: number): void {
    this.getCanvas(id).push({ type: 'strokeRect', x, y, width: w, height: h });
  }
  canvasClear(id: string, color: string): void {
    this.getCanvas(id).push({ type: 'clear', color });
  }
  canvasSetFillStyle(id: string, color: string): void {
    this.getCanvas(id).push({ type: 'setFillStyle', color });
  }
  canvasSetStrokeStyle(id: string, color: string): void {
    this.getCanvas(id).push({ type: 'setStrokeStyle', color });
  }
  canvasSetLineWidth(id: string, width: number): void {
    this.getCanvas(id).push({ type: 'setLineWidth', width });
  }
  canvasSetGlobalAlpha(id: string, alpha: number): void {
    this.getCanvas(id).push({ type: 'setGlobalAlpha', alpha });
  }
  canvasSave(id: string): void {
    this.getCanvas(id).push({ type: 'save' });
  }
  canvasRestore(id: string): void {
    this.getCanvas(id).push({ type: 'restore' });
  }
  canvasTranslate(id: string, x: number, y: number): void {
    this.getCanvas(id).push({ type: 'translate', x, y });
  }
  canvasScale(id: string, x: number, y: number): void {
    this.getCanvas(id).push({ type: 'scale', x, y });
  }
  canvasRotate(id: string, degrees: number): void {
    this.getCanvas(id).push({ type: 'rotate', degrees });
  }
  canvasFillRRect(id: string, x: number, y: number, w: number, h: number, r: number): void {
    this.getCanvas(id).push({ type: 'fillRRect', x, y, width: w, height: h, radius: r });
  }
  canvasFillCircle(id: string, cx: number, cy: number, radius: number): void {
    this.getCanvas(id).push({ type: 'fillCircle', cx, cy, radius });
  }
  canvasStrokeCircle(id: string, cx: number, cy: number, radius: number): void {
    this.getCanvas(id).push({ type: 'strokeCircle', cx, cy, radius });
  }
  canvasDrawLine(id: string, x0: number, y0: number, x1: number, y1: number): void {
    this.getCanvas(id).push({ type: 'drawLine', x0, y0, x1, y1 });
  }
  canvasDrawText(id: string, text: string, x: number, y: number, fontSize: number, antiAlias: boolean, stroke: boolean, strokeWidth: number): void {
    this.getCanvas(id).push({ type: 'drawText', text, x, y, fontSize, antiAlias, stroke, strokeWidth });
  }
  canvasBeginPath(id: string): void {
    this.getCanvas(id).push({ type: 'beginPath' });
  }
  canvasMoveTo(id: string, x: number, y: number): void {
    this.getCanvas(id).push({ type: 'moveTo', x, y });
  }
  canvasLineTo(id: string, x: number, y: number): void {
    this.getCanvas(id).push({ type: 'lineTo', x, y });
  }
  canvasQuadTo(id: string, cx: number, cy: number, x: number, y: number): void {
    this.getCanvas(id).push({ type: 'quadTo', cx, cy, x, y });
  }
  canvasCubicTo(id: string, c1x: number, c1y: number, c2x: number, c2y: number, x: number, y: number): void {
    this.getCanvas(id).push({ type: 'cubicTo', c1x, c1y, c2x, c2y, x, y });
  }
  canvasClosePath(id: string): void {
    this.getCanvas(id).push({ type: 'closePath' });
  }
  canvasFillPath(id: string): void {
    this.getCanvas(id).push({ type: 'fillPath' });
  }
  canvasStrokePath(id: string): void {
    this.getCanvas(id).push({ type: 'strokePath' });
  }
  canvasClipRect(id: string, x: number, y: number, w: number, h: number, antiAlias: boolean): void {
    this.getCanvas(id).push({ type: 'clipRect', x, y, width: w, height: h, antiAlias });
  }
  canvasDrawImage(id: string, assetId: string, x: number, y: number, w: number, h: number, alpha: number, srcRect?: number[]): void {
    this.getCanvas(id).push({ type: 'drawImage', assetId, x, y, width: w, height: h, alpha, srcRect });
  }
  canvasDrawImageSimple(id: string, assetId: string, x: number, y: number, alpha: number): void {
    this.getCanvas(id).push({ type: 'drawImageSimple', assetId, x, y, alpha });
  }

  // ── Text source ──

  recordTextSourceGet(nodeId: string): string {
    return this.textSources.get(nodeId) || '';
  }

  recordTextSourceSet(nodeId: string, text: string): void {
    this.textSources.set(nodeId, text);
  }

  // ── Animate bindings ──

  animateCreate(
    duration: number,
    delay: number,
    clamp: boolean,
    easing: string,
    repeat: number,
    yoyo: boolean,
    repeatDelay: number,
  ): number {
    const handle = this.nextAnimateHandle++;
    this.animateStates.set(handle, {
      duration,
      delay,
      clamp,
      easing,
      repeat,
      yoyo,
      repeatDelay,
      fromValues: {},
      toValues: {},
    });
    return handle;
  }

  animateValue(handle: number, key: string, from: number, to: number): number {
    const state = this.animateStates.get(handle);
    if (!state) return from;

    // Store value range
    state.fromValues[key] = from;
    state.toValues[key] = to;

    const { easing, spring } = parseEasing(state.easing || 'linear');
    const progress = computeProgress(
      this.frame, state.duration, state.delay,
      easing, spring, state.clamp, state.repeat, state.yoyo, state.repeatDelay,
    );
    return from + (to - from) * progress;
  }

  animateColor(_handle: number, _key: string, from: string, _to: string): string {
    // Simplified: return from color (full color interpolation in future)
    return from;
  }

  animateDispose(_handle: number): void {
    // No-op: animations reset per frame
  }

  flushTimelines(): void {
    // No-op: animations compute on-demand via animate_value
  }

  // ── Collection ──

  /** Collect and clear all accumulated mutations. Returns JSON matching Rust's StyleMutations. */
  collectJson(): string {
    const mutations: Record<string, any> = {};

    for (const [id, style] of this.styles) {
      if (style.transforms.length === 0 && Object.keys(style).length <= 1) continue;
      // Only include non-default fields
      const entry: Record<string, any> = {};
      for (const [key, val] of Object.entries(style)) {
        if (key === 'transforms') {
          if ((val as TransformEntry[]).length > 0) {
            entry.transforms = val;
          }
        } else if (val !== undefined && val !== null && val !== '') {
          entry[key] = val;
        }
      }
      if (Object.keys(entry).length > 0) {
        mutations[id] = entry;
      }
    }

    const canvasMutations: Record<string, any> = {};
    for (const [id, cmds] of this.canvases) {
      if (cmds.length > 0) {
        canvasMutations[id] = { commands: cmds };
      }
    }

    const result: CollectedOutput = { mutations, canvasMutations };

    // Reset for next frame
    this.reset();

    return JSON.stringify(result);
  }

  /** Reset all accumulated state for a new frame. */
  reset(): void {
    this.styles.clear();
    this.canvases.clear();
    // Keep text sources across frames (they represent the document text state)
    // Reset animate state
    this.animateStates.clear();
    this.nextAnimateHandle = 1;
  }

  /** Inject all native functions as global window properties for the JS runtime. */
  injectGlobals(): void {
    const bridge = this;

    // Node style (62 functions)
    (window as any).__record_opacity = (id: string, v: number) => bridge.recordOpacity(id, v);
    (window as any).__record_translate_x = (id: string, v: number) => bridge.recordTranslateX(id, v);
    (window as any).__record_translate_y = (id: string, v: number) => bridge.recordTranslateY(id, v);
    (window as any).__record_translate = (id: string, x: number, y: number) => bridge.recordTranslate(id, x, y);
    (window as any).__record_scale = (id: string, v: number) => bridge.recordScale(id, v);
    (window as any).__record_scale_x = (id: string, v: number) => bridge.recordScaleX(id, v);
    (window as any).__record_scale_y = (id: string, v: number) => bridge.recordScaleY(id, v);
    (window as any).__record_rotate = (id: string, v: number) => bridge.recordRotate(id, v);
    (window as any).__record_skew_x = (id: string, v: number) => bridge.recordSkewX(id, v);
    (window as any).__record_skew_y = (id: string, v: number) => bridge.recordSkewY(id, v);
    (window as any).__record_skew = (id: string, xDeg: number, yDeg: number) => bridge.recordSkew(id, xDeg, yDeg);
    (window as any).__record_position = (id: string, v: string) => bridge.recordPosition(id, v);
    (window as any).__record_left = (id: string, v: number) => bridge.recordLeft(id, v);
    (window as any).__record_top = (id: string, v: number) => bridge.recordTop(id, v);
    (window as any).__record_right = (id: string, v: number) => bridge.recordRight(id, v);
    (window as any).__record_bottom = (id: string, v: number) => bridge.recordBottom(id, v);
    (window as any).__record_width = (id: string, v: number) => bridge.recordWidth(id, v);
    (window as any).__record_height = (id: string, v: number) => bridge.recordHeight(id, v);
    (window as any).__record_padding = (id: string, v: number) => bridge.recordPadding(id, v);
    (window as any).__record_padding_x = (id: string, v: number) => bridge.recordPaddingX(id, v);
    (window as any).__record_padding_y = (id: string, v: number) => bridge.recordPaddingY(id, v);
    (window as any).__record_margin = (id: string, v: number) => bridge.recordMargin(id, v);
    (window as any).__record_margin_x = (id: string, v: number) => bridge.recordMarginX(id, v);
    (window as any).__record_margin_y = (id: string, v: number) => bridge.recordMarginY(id, v);
    (window as any).__record_flex_direction = (id: string, v: string) => bridge.recordFlexDirection(id, v);
    (window as any).__record_justify_content = (id: string, v: string) => bridge.recordJustifyContent(id, v);
    (window as any).__record_align_items = (id: string, v: string) => bridge.recordAlignItems(id, v);
    (window as any).__record_gap = (id: string, v: number) => bridge.recordGap(id, v);
    (window as any).__record_flex_grow = (id: string, v: number) => bridge.recordFlexGrow(id, v);
    (window as any).__record_bg = (id: string, v: string) => bridge.recordBg(id, v);
    (window as any).__record_border_radius = (id: string, v: number) => bridge.recordBorderRadius(id, v);
    (window as any).__record_border_width = (id: string, v: number) => bridge.recordBorderWidth(id, v);
    (window as any).__record_border_top_width = (id: string, v: number) => bridge.recordBorderTopWidth(id, v);
    (window as any).__record_border_right_width = (id: string, v: number) => bridge.recordBorderRightWidth(id, v);
    (window as any).__record_border_bottom_width = (id: string, v: number) => bridge.recordBorderBottomWidth(id, v);
    (window as any).__record_border_left_width = (id: string, v: number) => bridge.recordBorderLeftWidth(id, v);
    (window as any).__record_border_style = (id: string, v: string) => bridge.recordBorderStyle(id, v);
    (window as any).__record_border_color = (id: string, v: string) => bridge.recordBorderColor(id, v);
    (window as any).__record_stroke_width = (id: string, v: number) => bridge.recordStrokeWidth(id, v);
    (window as any).__record_stroke_color = (id: string, v: string) => bridge.recordStrokeColor(id, v);
    (window as any).__record_fill_color = (id: string, v: string) => bridge.recordFillColor(id, v);
    (window as any).__record_object_fit = (id: string, v: string) => bridge.recordObjectFit(id, v);
    (window as any).__record_text_color = (id: string, v: string) => bridge.recordTextColor(id, v);
    (window as any).__record_text_size = (id: string, v: number) => bridge.recordTextSize(id, v);
    (window as any).__record_font_weight = (id: string, v: number) => bridge.recordFontWeight(id, v);
    (window as any).__record_letter_spacing = (id: string, v: number) => bridge.recordLetterSpacing(id, v);
    (window as any).__record_text_align = (id: string, v: string) => bridge.recordTextAlign(id, v);
    (window as any).__record_line_height = (id: string, v: number) => bridge.recordLineHeight(id, v);
    (window as any).__record_shadow = (id: string, v: string) => bridge.recordShadow(id, v);
    (window as any).__record_shadow_color = (id: string, v: string) => bridge.recordShadowColor(id, v);
    (window as any).__record_inset_shadow = (id: string, v: string) => bridge.recordInsetShadow(id, v);
    (window as any).__record_inset_shadow_color = (id: string, v: string) => bridge.recordInsetShadowColor(id, v);
    (window as any).__record_drop_shadow = (id: string, v: string) => bridge.recordDropShadow(id, v);
    (window as any).__record_drop_shadow_color = (id: string, v: string) => bridge.recordDropShadowColor(id, v);
    (window as any).__record_text_content = (id: string, v: string) => bridge.recordTextContent(id, v);
    (window as any).__record_stroke_dasharray = (id: string, v: number) => bridge.recordStrokeDasharray(id, v);
    (window as any).__record_stroke_dashoffset = (id: string, v: number) => bridge.recordStrokeDashoffset(id, v);
    (window as any).__record_svg_path = (id: string, v: string) => bridge.recordSvgPath(id, v);

    // Text source
    (window as any).__text_source_get = (nodeId: string) => bridge.recordTextSourceGet(nodeId);
    (window as any).__text_source_set = (nodeId: string, text: string) => bridge.recordTextSourceSet(nodeId, text);

    // Canvas (30+ functions)
    (window as any).__canvas_fill_rect = (id: string, x: number, y: number, w: number, h: number) => bridge.canvasFillRect(id, x, y, w, h);
    (window as any).__canvas_stroke_rect = (id: string, x: number, y: number, w: number, h: number) => bridge.canvasStrokeRect(id, x, y, w, h);
    (window as any).__canvas_clear = (id: string, color: string) => bridge.canvasClear(id, color);
    (window as any).__canvas_set_fill_style = (id: string, color: string) => bridge.canvasSetFillStyle(id, color);
    (window as any).__canvas_set_stroke_style = (id: string, color: string) => bridge.canvasSetStrokeStyle(id, color);
    (window as any).__canvas_set_line_width = (id: string, width: number) => bridge.canvasSetLineWidth(id, width);
    (window as any).__canvas_set_global_alpha = (id: string, alpha: number) => bridge.canvasSetGlobalAlpha(id, alpha);
    (window as any).__canvas_save = (id: string) => bridge.canvasSave(id);
    (window as any).__canvas_restore = (id: string) => bridge.canvasRestore(id);
    (window as any).__canvas_translate = (id: string, x: number, y: number) => bridge.canvasTranslate(id, x, y);
    (window as any).__canvas_scale = (id: string, x: number, y: number) => bridge.canvasScale(id, x, y);
    (window as any).__canvas_rotate = (id: string, degrees: number) => bridge.canvasRotate(id, degrees);
    (window as any).__canvas_fill_rrect = (id: string, x: number, y: number, w: number, h: number, r: number) => bridge.canvasFillRRect(id, x, y, w, h, r);
    (window as any).__canvas_fill_circle = (id: string, cx: number, cy: number, radius: number) => bridge.canvasFillCircle(id, cx, cy, radius);
    (window as any).__canvas_stroke_circle = (id: string, cx: number, cy: number, radius: number) => bridge.canvasStrokeCircle(id, cx, cy, radius);
    (window as any).__canvas_draw_line = (id: string, x0: number, y0: number, x1: number, y1: number) => bridge.canvasDrawLine(id, x0, y0, x1, y1);
    (window as any).__canvas_draw_text = (id: string, text: string, x: number, y: number, fontSize: number, antiAlias: boolean, stroke: boolean, strokeWidth: number) => bridge.canvasDrawText(id, text, x, y, fontSize, antiAlias, stroke, strokeWidth);
    (window as any).__canvas_begin_path = (id: string) => bridge.canvasBeginPath(id);
    (window as any).__canvas_move_to = (id: string, x: number, y: number) => bridge.canvasMoveTo(id, x, y);
    (window as any).__canvas_line_to = (id: string, x: number, y: number) => bridge.canvasLineTo(id, x, y);
    (window as any).__canvas_quad_to = (id: string, cx: number, cy: number, x: number, y: number) => bridge.canvasQuadTo(id, cx, cy, x, y);
    (window as any).__canvas_cubic_to = (id: string, c1x: number, c1y: number, c2x: number, c2y: number, x: number, y: number) => bridge.canvasCubicTo(id, c1x, c1y, c2x, c2y, x, y);
    (window as any).__canvas_close_path = (id: string) => bridge.canvasClosePath(id);
    (window as any).__canvas_fill_path = (id: string) => bridge.canvasFillPath(id);
    (window as any).__canvas_stroke_path = (id: string) => bridge.canvasStrokePath(id);
    (window as any).__canvas_clip_rect = (id: string, x: number, y: number, w: number, h: number, antiAlias: boolean) => bridge.canvasClipRect(id, x, y, w, h, antiAlias);
    (window as any).__canvas_draw_image = (id: string, assetId: string, x: number, y: number, w: number, h: number, alpha: number, srcRect?: number[]) => bridge.canvasDrawImage(id, assetId, x, y, w, h, alpha, srcRect);
    (window as any).__canvas_draw_image_simple = (id: string, assetId: string, x: number, y: number, alpha: number) => bridge.canvasDrawImageSimple(id, assetId, x, y, alpha);

    // Animate (8 functions)
    (window as any).__animate_create = (duration: number, delay: number, clamp: boolean, easing: string, repeat: number, yoyo: boolean, repeatDelay: number) => bridge.animateCreate(duration, delay, clamp, easing, repeat, yoyo, repeatDelay);
    (window as any).__animate_value = (handle: number, key: string, from: number, to: number) => bridge.animateValue(handle, key, from, to);
    (window as any).__animate_color = (handle: number, key: string, from: string, to: string) => bridge.animateColor(handle, key, from, to);
    (window as any).__animate_dispose = (handle: number) => bridge.animateDispose(handle);
    (window as any).__flush_timelines = () => bridge.flushTimelines();
  }

  /** Remove all native functions from global scope. */
  removeGlobals(): void {
    const globals = [
      '__record_opacity', '__record_translate_x', '__record_translate_y', '__record_translate',
      '__record_scale', '__record_scale_x', '__record_scale_y', '__record_rotate',
      '__record_skew_x', '__record_skew_y', '__record_skew',
      '__record_position', '__record_left', '__record_top', '__record_right', '__record_bottom',
      '__record_width', '__record_height', '__record_padding', '__record_padding_x', '__record_padding_y',
      '__record_margin', '__record_margin_x', '__record_margin_y',
      '__record_flex_direction', '__record_justify_content', '__record_align_items',
      '__record_gap', '__record_flex_grow', '__record_bg',
      '__record_border_radius', '__record_border_width', '__record_border_top_width',
      '__record_border_right_width', '__record_border_bottom_width', '__record_border_left_width',
      '__record_border_style', '__record_border_color',
      '__record_stroke_width', '__record_stroke_color', '__record_fill_color',
      '__record_object_fit', '__record_text_color', '__record_text_size', '__record_font_weight',
      '__record_letter_spacing', '__record_text_align', '__record_line_height',
      '__record_shadow', '__record_shadow_color', '__record_inset_shadow', '__record_inset_shadow_color',
      '__record_drop_shadow', '__record_drop_shadow_color',
      '__record_text_content', '__record_stroke_dasharray', '__record_stroke_dashoffset',
      '__record_svg_path', '__text_source_get', '__text_source_set',
      '__canvas_fill_rect', '__canvas_stroke_rect', '__canvas_clear',
      '__canvas_set_fill_style', '__canvas_set_stroke_style', '__canvas_set_line_width',
      '__canvas_set_global_alpha', '__canvas_save', '__canvas_restore',
      '__canvas_translate', '__canvas_scale', '__canvas_rotate',
      '__canvas_fill_rrect', '__canvas_fill_circle', '__canvas_stroke_circle',
      '__canvas_draw_line', '__canvas_draw_text',
      '__canvas_begin_path', '__canvas_move_to', '__canvas_line_to',
      '__canvas_quad_to', '__canvas_cubic_to', '__canvas_close_path',
      '__canvas_fill_path', '__canvas_stroke_path', '__canvas_clip_rect',
      '__canvas_draw_image', '__canvas_draw_image_simple',
      '__animate_create', '__animate_value', '__animate_color', '__animate_dispose',
      '__flush_timelines',
    ];
    for (const name of globals) {
      delete (window as any)[name];
    }
  }
}
