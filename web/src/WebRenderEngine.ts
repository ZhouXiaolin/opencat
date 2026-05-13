// ── WebRenderEngine ──
// Receives an OrderedSceneProgram (serialized JSON from Rust) and draws it
// using CanvasKit. Class-based replacement for the module-level renderer.ts.

import type {
  DisplayNodeJson,
  DisplayItemJson,
  DisplayTransformJson,
  DisplayRect,
  RectPaintJson,
  BackgroundFillJson,
  Color4f,
  BorderRadius,
  CanvasCommandJson,
  CompositionInfo,
  DisplayGlyphCommand,
  DropShadowJson,
} from './types';

import { WasmCacheBridge } from './WasmCacheBridge';
import { recordPicture, drawTransition, setCanvasKitForTransition } from './transition';

// ── OrderedSceneProgram types ──

type ItemExecution = 'Direct' | 'Cached';

interface LiveSubtree {
  LiveSubtree: {
    handle: number;
    item_execution: ItemExecution;
    children: OrderedSceneOp[];
  };
}

interface CachedSubtree {
  CachedSubtree: {
    handle: number;
  };
}

type OrderedSceneOp = LiveSubtree | CachedSubtree;

interface OrderedSceneProgram {
  root: OrderedSceneOp;
}

// ── DrawScript state ──

interface DrawScriptState {
  fillColor?: Color4f;
  strokeColor?: Color4f;
  lineWidth: number;
  globalAlpha: number;
  lineCap: string;
  lineJoin: string;
  pathStr: string;
}

// ── WebRenderEngine ──

export class WebRenderEngine {
  private CK: any;              // CanvasKit instance
  private canvas: HTMLCanvasElement;
  private cacheBridge: WasmCacheBridge | null;

  private surface: any = null;  // CK.Surface
  private ckCanvas: any = null; // CK.Canvas

  // Loaded images (assetId -> CK.Image)
  private loadedImages = new Map<string, any>();

  // Subtree caches (handle -> CK.Picture | CK.Image)
  private subtreePicCache = new Map<number, any>();
  private subtreeImgCache = new Map<number, any>();

  // Per-item picture cache (handle -> CK.Picture)
  private itemPicCache = new Map<number, any>();

  // Glyph caches
  private glyphPathCache = new Map<number, any>();   // cacheKey -> CK.Path
  private glyphImgCache = new Map<number, any>();    // cacheKey -> CK.Image

  // General image cache (url -> CK.Image)
  private imageCache = new Map<string, any>();

  constructor(
    ck: any,
    canvas: HTMLCanvasElement,
    cacheBridge: WasmCacheBridge | null = null,
  ) {
    this.CK = ck;
    this.canvas = canvas;
    this.cacheBridge = cacheBridge;
    setCanvasKitForTransition(ck);
  }

  // ── Public API ──

  registerImage(assetId: string, ckImage: any): void {
    this.loadedImages.set(assetId, ckImage);
  }

  ensureSurface(width: number, height: number): void {
    if (this.surface) {
      if (this.surface.width() === width && this.surface.height() === height) return;
      this.surface.delete();
      this.surface = null;
      this.ckCanvas = null;
    }
    this.surface = this.CK.MakeWebGLCanvasSurface(this.canvas);
    if (!this.surface) {
      this.surface = this.CK.MakeSWCanvasSurface(this.canvas);
    }
    this.ckCanvas = this.surface.getCanvas();
  }

  dispose(): void {
    this.clearCaches();
    if (this.surface) {
      this.surface.delete();
      this.surface = null;
      this.ckCanvas = null;
    }
  }

  getCanvas(): any {
    return this.ckCanvas;
  }

  getSurface(): any {
    return this.surface;
  }

  /**
   * Main entry point: draw an OrderedSceneProgram.
   *
   * @param opsJson  The OrderedSceneProgram JSON from Rust.
   * @param frameView  { displayTree, comp, frame } — the existing display
   *                   tree JSON that provides display-item data for each node.
   */
  drawOrderedScene(
    opsJson: OrderedSceneProgram,
    frameView: {
      displayTree: DisplayNodeJson;
      comp: CompositionInfo;
      frame: number;
    },
    clearColor?: Color4f | null,
  ): void {
    if (!this.ckCanvas || !this.surface) return;

    const { comp, frame } = frameView;
    const cc = clearColor || { r: 0, g: 0, b: 0, a: 255 };
    this.ckCanvas.clear(this.CK.Color4f(cc.r / 255, cc.g / 255, cc.b / 255, cc.a / 255));

    // Build a flat handle-to-node map from the display tree for O(1) lookup.
    const nodeMap = new Map<number, DisplayNodeJson>();
    this.buildNodeMap(frameView.displayTree, nodeMap);

    // Walk the ordered scene tree.
    this.walkOp(opsJson.root, nodeMap);

    this.surface.flush();
  }

  /**
   * Legacy path: draw a plain DisplayNodeJson tree (used during migration).
   */
  drawDisplayTree(
    displayTree: DisplayNodeJson,
    comp: CompositionInfo,
    frame: number,
    clearColor?: Color4f | null,
  ): void {
    if (!this.ckCanvas || !this.surface) return;

    const cc = clearColor || { r: 0, g: 0, b: 0, a: 255 };
    this.ckCanvas.clear(this.CK.Color4f(cc.r / 255, cc.g / 255, cc.b / 255, cc.a / 255));

    this.applyTransform(displayTree.transform);
    this.drawDisplayNode(displayTree);
    this.ckCanvas.restore();

    this.surface.flush();
  }

  captureFramePixels(w: number, h: number): Uint8Array | null {
    if (!this.surface) return null;
    const image = this.surface.makeImageSnapshot();
    if (!image) return null;
    const pixels = image.readPixels(0, 0, {
      width: w,
      height: h,
      colorType: this.CK.ColorType.RGBA_8888,
      alphaType: this.CK.AlphaType.Unpremul,
      colorSpace: this.CK.ColorSpace.SRGB,
    });
    image.delete();
    return pixels;
  }

  // ── OrderedScene tree walk ──

  private buildNodeMap(
    node: DisplayNodeJson,
    map: Map<number, DisplayNodeJson>,
  ): void {
    map.set(node.elementId, node);
    for (const child of node.children) {
      this.buildNodeMap(child, map);
    }
  }

  private walkOp(
    op: OrderedSceneOp,
    nodeMap: Map<number, DisplayNodeJson>,
  ): void {
    if ('LiveSubtree' in op) {
      const { handle, children } = op.LiveSubtree;
      const node = nodeMap.get(handle);
      if (node) {
        this.ckCanvas.save();
        this.applyTransform(node.transform);

        if (node.opacity < 1.0) {
          const paint = new this.CK.Paint();
          paint.setAlphaf(node.opacity);
          this.ckCanvas.saveLayer(paint);
          paint.delete();
        }

        if (node.clip) {
          this.applyClip(node.clip);
        }

        const isTransition = node.item.type === 'timeline'
          && node.item.transition
          && children.length >= 2;

        if (isTransition) {
          // Draw timeline base (rect background) then composite children
          this.drawRectItem(node.item);
          this.drawTimelineTransition(node.item, children, nodeMap);
        } else {
          this.drawDisplayItem(node.item);
          for (const child of children) {
            this.walkOp(child, nodeMap);
          }
        }

        if (node.opacity < 1.0) {
          this.ckCanvas.restore();
        }
        this.ckCanvas.restore();
      }
    } else if ('CachedSubtree' in op) {
      const { handle } = op.CachedSubtree;
      this.drawCachedSubtree(handle);
    }
  }

  /// Record from/to children into Pictures and composite with transition effect.
  private drawTimelineTransition(
    item: DisplayItemJson,
    children: OrderedSceneOp[],
    nodeMap: Map<number, DisplayNodeJson>,
  ): void {
    const transition = item.transition!;
    const bounds = item.bounds;

    const recordChild = (child: OrderedSceneOp): any | null => {
      return recordPicture(bounds, (recCanvas: any) => {
        const savedCanvas = this.ckCanvas;
        this.ckCanvas = recCanvas;
        try {
          this.walkOp(child, nodeMap);
        } finally {
          this.ckCanvas = savedCanvas;
        }
      });
    };

    const fromPic = recordChild(children[0]);
    const toPic = recordChild(children[1]);

    if (fromPic && toPic) {
      drawTransition(this.ckCanvas, fromPic, toPic, transition, bounds);
    } else {
      // Fallback: draw children directly
      for (let i = 0; i < children.length; i++) {
        this.walkOp(children[i], nodeMap);
      }
    }

    fromPic?.delete();
    toPic?.delete();
  }

  private drawCachedSubtree(handle: number): void {
    // 1. Try local picture cache.
    const cached = this.subtreePicCache.get(handle);
    if (cached) {
      this.ckCanvas.drawPicture(cached);
      return;
    }

    // 2. Try WasmCacheBridge.
    if (this.cacheBridge) {
      const snapshot = this.cacheBridge.querySubtreeSnapshot(handle);
      if (snapshot) {
        this.cacheBridge.reportSubtreeSnapshotHit(handle);
        // The snapshot is raw pixel data; build a CK.Image from it.
        // For now, if the bridge returns pre-built image data we draw it.
        // This path will be fleshed out as the Rust side stabilises.
        return;
      }
    }

    // 3. Cache miss — in the future we would record a new picture here.
    // TODO: D6 - implement cache miss recording pipeline
    // recordSubtreePicture → store to JS map + Rust → drawCachedPicture
    // For now, just skip. As the pipeline matures, this will be replaced
    // with actual picture recording logic.
  }

  // ── DisplayNode recursion (legacy path) ──

  private drawDisplayNode(node: DisplayNodeJson): void {
    const { item, opacity, clip, children } = node;

    this.ckCanvas.save();
    this.applyTransform(node.transform);

    if (opacity < 1.0) {
      const paint = new this.CK.Paint();
      paint.setAlphaf(opacity);
      this.ckCanvas.saveLayer(paint);
      paint.delete();
    }

    if (clip) {
      this.applyClip(clip);
    }

    const isTransition = item.type === 'timeline'
      && item.transition
      && children.length >= 2;

    if (isTransition) {
      this.drawRectItem(item);
      this.drawTransitionForNode(item, children);
    } else {
      this.drawDisplayItem(item);
      for (const child of children) {
        this.drawDisplayNode(child);
      }
    }

    if (opacity < 1.0) {
      this.ckCanvas.restore();
    }

    this.ckCanvas.restore();
  }

  private drawTransitionForNode(
    item: DisplayItemJson,
    children: DisplayNodeJson[],
  ): void {
    const transition = item.transition!;
    const bounds = item.bounds;

    const drawNodeToPic = (node: DisplayNodeJson): any | null => {
      return recordPicture(bounds, (recCanvas: any) => {
        const savedCanvas = this.ckCanvas;
        this.ckCanvas = recCanvas;
        try {
          this.drawDisplayNode(node);
        } finally {
          this.ckCanvas = savedCanvas;
        }
      });
    };

    const fromPic = drawNodeToPic(children[0]);
    const toPic = drawNodeToPic(children[1]);

    if (fromPic && toPic) {
      drawTransition(this.ckCanvas, fromPic, toPic, transition, bounds);
    } else {
      for (const child of children) {
        this.drawDisplayNode(child);
      }
    }

    fromPic?.delete();
    toPic?.delete();
  }

  // ── DisplayItem router ──

  private drawDisplayItem(item: DisplayItemJson): void {
    switch (item.type) {
      case 'rect':
        this.drawRectItem(item);
        break;
      case 'timeline':
        this.drawRectItem(item);
        break;
      case 'text':
        this.drawTextItem(item);
        break;
      case 'bitmap':
        this.drawBitmapItem(item);
        break;
      case 'drawScript':
        this.drawScriptItem(item);
        break;
      case 'svgPath':
        this.drawSvgPathItem(item);
        break;
    }
  }

  // ── Rect / Timeline ──

  private drawRectItem(item: DisplayItemJson): void {
    const { bounds, paint } = item;
    if (!paint) return;

    const b = bounds;

    if (paint.boxShadow) {
      this.drawBoxShadow(b, paint.boxShadow, paint.borderRadius);
    }

    if (paint.background) {
      const fillPaint = this.makeFillPaint(paint.background);
      this.drawRoundRect(b, paint.borderRadius, fillPaint);
      fillPaint.delete();
    }

    if (paint.borderWidth && paint.borderWidth > 0 && paint.borderColor) {
      this.drawBorder(b, paint);
    }

    if (paint.insetShadow) {
      this.drawInsetShadow(b, paint.insetShadow, paint.borderRadius);
    }
  }

  // ── Text ──

  private drawTextItem(item: DisplayItemJson): void {
    if (item.glyphs) {
      this.drawTextWithGlyphs(item);
      return;
    }
    this.drawTextWithCanvasKitFont(item);
  }

  private drawTextWithGlyphs(item: DisplayItemJson): void {
    const { bounds, style, glyphs, dropShadow } = item;
    if (!style || !glyphs) return;

    const CK = this.CK;
    const canvas = this.ckCanvas;

    const textColor = style.color;
    const fillPaint = new CK.Paint();
    fillPaint.setStyle(CK.PaintStyle.Fill);
    fillPaint.setAntiAlias(true);
    fillPaint.setColor(
      CK.Color4f(textColor.r / 255, textColor.g / 255, textColor.b / 255, textColor.a / 255),
    );

    // Build lookup map from cache_key -> glyph data
    const glyphMap = new Map<number, any>();
    for (const entry of glyphs.entries) {
      glyphMap.set(entry.cacheKey, entry.data);
    }

    // Parse text unit overrides
    const rawText = (item as any).text || '';
    const text = style.textTransform === 'uppercase' ? rawText.toUpperCase() : rawText;
    const unitOverrides = (item as any).textUnitOverrides;
    const overrides = unitOverrides?.overrides ?? null;
    const granularity = unitOverrides?.granularity;
    const textAlign = style.textAlign || 'left';

    // Group glyphs by unit
    const unitGroups = this.buildGlyphUnitGroups(
      text,
      glyphs,
      granularity,
      textAlign,
      bounds.width,
      glyphMap,
    );

    canvas.save();

    // Render each unit
    for (const group of unitGroups) {
      const override = overrides?.[group.unitIndex];
      const hasNonDefault =
        override &&
        ((override.translateX ?? 0) !== 0 ||
          (override.translateY ?? 0) !== 0 ||
          (override.scale ?? 1) !== 1 ||
          (override.rotationDeg ?? 0) !== 0 ||
          (override.opacity ?? 1) < 1 ||
          override.color != null);

      if (!hasNonDefault) {
        // Fast path: draw glyphs at layout positions
        for (const slot of group.slots) {
          this.drawSingleGlyph(slot, fillPaint, dropShadow, 1.0);
        }
        continue;
      }

      // Engine-style: T(pivot + trans) -> R -> S -> T(-pivot)
      const bx = group.bbox;
      const pivotX = (bx.minX + bx.maxX) / 2;
      const pivotY = (bx.minY + bx.maxY) / 2;
      const transX = override.translateX ?? 0;
      const transY = override.translateY ?? 0;
      const scale = override.scale ?? 1;
      const rotationDeg = override.rotationDeg ?? 0;
      const opacity = override.opacity ?? 1;

      canvas.save();
      canvas.translate(transX, transY);
      if (rotationDeg !== 0) canvas.rotate(rotationDeg, pivotX, pivotY);
      if (scale !== 1) canvas.scale(scale, scale, pivotX, pivotY);

      // Opacity via saveLayer
      if (opacity < 1) {
        const alphaPaint = new CK.Paint();
        alphaPaint.setAlphaf(opacity);
        canvas.saveLayer(alphaPaint);
        alphaPaint.delete();
      }

      // Use override color or default
      let unitPaint = fillPaint;
      let ownsUnitPaint = false;
      if (override?.color) {
        const c = override.color;
        unitPaint = new CK.Paint();
        unitPaint.setStyle(CK.PaintStyle.Fill);
        unitPaint.setAntiAlias(true);
        unitPaint.setColor(CK.Color4f(c.r / 255, c.g / 255, c.b / 255, c.a / 255));
        ownsUnitPaint = true;
      }

      for (const slot of group.slots) {
        this.drawSingleGlyph(slot, unitPaint, dropShadow, 1.0);
      }

      if (ownsUnitPaint) unitPaint.delete();
      if (opacity < 1) canvas.restore();
      canvas.restore();
    }

    canvas.restore();
    fillPaint.delete();
  }

  /// Group glyphs into text units (grapheme or word) and compute per-unit bounding box.
  private buildGlyphUnitGroups(
    text: string,
    glyphs: any,
    granularity: string | undefined,
    textAlign: string,
    boundsWidth: number,
    glyphMap: Map<number, any>,
  ): Array<{ unitIndex: number; slots: Array<{ cacheKey: number; glyphData: any; gx: number; gy: number; byteStart: number }>; bbox: { minX: number; minY: number; maxX: number; maxY: number } }> {
    const encoder = new TextEncoder();

    // Segment text into units and get byte ranges
    const units: Array<{ byteStart: number; byteEnd: number }> = [];

    if (granularity === 'word' || granularity === 'words') {
      const wordRanges: number[][] =
        ((window as any).__text_word_ranges?.(text) as number[][]) || [];
      for (const [start, end] of wordRanges) {
        units.push({ byteStart: start, byteEnd: end });
      }
    } else {
      const graphemes: string[] =
        ((window as any).__text_graphemes?.(text) as string[]) || [...text];
      let byteOffset = 0;
      for (const g of graphemes) {
        const byteLen = encoder.encode(g).length;
        units.push({ byteStart: byteOffset, byteEnd: byteOffset + byteLen });
        byteOffset += byteLen;
      }
    }

    // Collect all glyph slots from the layout
    const allSlots: Array<{ cacheKey: number; glyphData: any; gx: number; gy: number; byteStart: number }> = [];
    for (let li = 0; li < glyphs.lines.length; li++) {
      const line = glyphs.lines[li];
      const xShift = this.computeTextXShift(line.width, boundsWidth, textAlign);
      for (let pi = 0; pi < line.positions.length; pi++) {
        const pos = line.positions[pi];
        const glyphData = glyphMap.get(pos.cacheKey);
        if (!glyphData) continue;
        allSlots.push({
          cacheKey: pos.cacheKey,
          glyphData,
          gx: pos.x + xShift,
          gy: pos.y,
          byteStart: pos.byteStart,
        });
      }
    }

    // Assign glyph slots to their corresponding unit
    const unitGroups: Array<{
      unitIndex: number;
      slots: Array<{ cacheKey: number; glyphData: any; gx: number; gy: number; byteStart: number }>;
      bbox: { minX: number; minY: number; maxX: number; maxY: number };
    }> = [];
    for (let ui = 0; ui < units.length; ui++) {
      const unit = units[ui];
      const slots = allSlots.filter(
        (s) => s.byteStart >= unit.byteStart && s.byteStart < unit.byteEnd,
      );
      if (slots.length === 0) continue;

      // Compute bounding box of this unit's glyphs
      let minX = Infinity;
      let minY = Infinity;
      let maxX = -Infinity;
      let maxY = -Infinity;

      for (const slot of slots) {
        if (slot.glyphData.kind === 'outline') {
          const path = this.buildGlyphPathCached(slot.cacheKey, slot.glyphData.commands);
          if (path) {
            const b = path.getBounds(); // Float32Array [left, top, right, bottom]
            const gx = slot.gx + b[0];
            const gy = slot.gy + b[1];
            const gw = b[2] - b[0];
            const gh = b[3] - b[1];
            if (gw > 0 && gh > 0) {
              minX = Math.min(minX, gx);
              minY = Math.min(minY, gy);
              maxX = Math.max(maxX, gx + gw);
              maxY = Math.max(maxY, gy + gh);
            } else {
              minX = Math.min(minX, slot.gx);
              minY = Math.min(minY, slot.gy);
              maxX = Math.max(maxX, slot.gx);
              maxY = Math.max(maxY, slot.gy);
            }
          } else {
            minX = Math.min(minX, slot.gx);
            minY = Math.min(minY, slot.gy);
            maxX = Math.max(maxX, slot.gx);
            maxY = Math.max(maxY, slot.gy);
          }
        } else if (slot.glyphData.kind === 'colorImage') {
          const { placementLeft, placementTop, width, height } = slot.glyphData;
          const ix = slot.gx + (placementLeft || 0);
          const iy = slot.gy - (placementTop || 0);
          const iw = width || 0;
          const ih = height || 0;
          if (iw > 0 && ih > 0) {
            minX = Math.min(minX, ix);
            minY = Math.min(minY, iy);
            maxX = Math.max(maxX, ix + iw);
            maxY = Math.max(maxY, iy + ih);
          } else {
            minX = Math.min(minX, slot.gx);
            minY = Math.min(minY, slot.gy);
            maxX = Math.max(maxX, slot.gx);
            maxY = Math.max(maxY, slot.gy);
          }
        }
      }

      // Fallback if bbox is empty
      if (slots.length > 0 && (minX === Infinity || minY === Infinity)) {
        minX = slots[0].gx - 5;
        minY = slots[0].gy - 5;
        maxX = slots[0].gx + 5;
        maxY = slots[0].gy + 5;
      }

      unitGroups.push({
        unitIndex: ui,
        slots,
        bbox: { minX, minY, maxX, maxY },
      });
    }

    return unitGroups;
  }

  /// Draw a single glyph at its layout position (path or color image).
  private drawSingleGlyph(
    slot: { cacheKey: number; glyphData: any; gx: number; gy: number; byteStart: number },
    paint: any,
    dropShadow: DropShadowJson | null | undefined,
    _opacity: number,
  ): void {
    const glyphData = slot.glyphData;

    if (glyphData.kind === 'outline') {
      const path = this.buildGlyphPathCached(slot.cacheKey, glyphData.commands);
      if (!path) return;
      this.ckCanvas.save();
      this.ckCanvas.translate(slot.gx, slot.gy);
      if (dropShadow) {
        this.drawGlyphDropShadow(path, dropShadow);
      }
      this.ckCanvas.drawPath(path, paint);
      this.ckCanvas.restore();
    } else if (glyphData.kind === 'colorImage') {
      const { rgba, width, height, placementLeft, placementTop } = glyphData;
      let image = this.glyphImgCache.get(slot.cacheKey);
      if (!image) {
        image = this.makeImageFromRgba(rgba, width, height);
        if (image) {
          this.glyphImgCache.set(slot.cacheKey, image);
        }
      }
      if (image) {
        this.ckCanvas.save();
        this.ckCanvas.translate(slot.gx, slot.gy);
        this.ckCanvas.drawImage(image, placementLeft || 0, -(placementTop || 0));
        this.ckCanvas.restore();
      }
    }
  }

  /// Build a cached glyph path from outline commands.
  private buildGlyphPathCached(cacheKey: number, commands: any): any {
    let path = this.glyphPathCache.get(cacheKey);
    if (!path) {
      path = this.buildGlyphPath(commands);
      if (path) {
        this.glyphPathCache.set(cacheKey, path);
      }
    }
    return path;
  }

  private drawTextWithCanvasKitFont(item: DisplayItemJson): void {
    const { bounds, text, style, dropShadow } = item;
    if (!text || !style) return;

    const CK = this.CK;
    const canvas = this.ckCanvas;

    const fontSize = style.textPx || 16;
    const font = new CK.Font(null, fontSize);

    const textPaint = new CK.Paint();
    const c = style.color;
    textPaint.setColor(CK.Color4f(c.r / 255, c.g / 255, c.b / 255, c.a / 255));

    let x = bounds.x;
    if (style.textAlign === 'center' || style.textAlign === 'right') {
      const glyphs = font.getGlyphIDs(text);
      const widths = font.getGlyphWidths(glyphs);
      let textWidth = 0;
      for (let i = 0; i < widths.length; i++) textWidth += widths[i];
      if (style.textAlign === 'center') {
        x = bounds.x + (bounds.width - textWidth) / 2;
      } else {
        x = bounds.x + bounds.width - textWidth;
      }
    }

    if (dropShadow) {
      const shadowPaint = new CK.Paint();
      const sc = dropShadow.color;
      shadowPaint.setColor(CK.Color4f(sc.r / 255, sc.g / 255, sc.b / 255, sc.a / 255));
      canvas.drawText(
        text,
        x + (dropShadow.offsetX || 0),
        bounds.y + fontSize + (dropShadow.offsetY || 0),
        shadowPaint,
        font,
      );
      shadowPaint.delete();
    }

    const displayText = style.textTransform === 'uppercase' ? text.toUpperCase() : text;
    canvas.drawText(displayText, x, bounds.y + fontSize, textPaint, font);

    textPaint.delete();
    font.delete();
  }

  // ── Bitmap ──

  private drawBitmapItem(item: DisplayItemJson): void {
    const { bounds, assetId, paint, objectFit } = item;
    if (!assetId || !bounds) return;

    const img = this.loadedImages.get(assetId);
    if (!img) return;

    const CK = this.CK;
    const canvas = this.ckCanvas;
    const srcW = img.width();
    const srcH = img.height();

    let dst = bounds;
    if (objectFit && objectFit !== 'fill') {
      const srcAspect = srcW / srcH;
      const dstAspect = bounds.width / bounds.height;

      if (objectFit === 'contain') {
        if (srcAspect > dstAspect) {
          const h = bounds.width / srcAspect;
          dst = { ...bounds, y: bounds.y + (bounds.height - h) / 2, height: h };
        } else {
          const w = bounds.height * srcAspect;
          dst = { ...bounds, x: bounds.x + (bounds.width - w) / 2, width: w };
        }
      } else if (objectFit === 'cover') {
        if (srcAspect > dstAspect) {
          // wider than tall: scale to fill height, center horizontally
          const w = bounds.height * srcAspect;
          dst = { ...bounds, x: bounds.x - (w - bounds.width) / 2, width: w };
        } else {
          // taller than wide: scale to fill width, center vertically
          const h = bounds.width / srcAspect;
          dst = { ...bounds, y: bounds.y - (h - bounds.height) / 2, height: h };
        }
      }
    }

    if (paint?.borderRadius && this.hasNonZeroRadius(paint.borderRadius)) {
      canvas.save();
      this.applyClip({ bounds, borderRadius: paint.borderRadius });
    }

    const srcRect = CK.XYWHRect(0, 0, srcW, srcH);
    const dstRect = CK.XYWHRect(dst.x, dst.y, dst.width, dst.height);

    const drawPaint = new CK.Paint();
    drawPaint.setAlphaf(1.0);
    drawPaint.setAntiAlias(true);
    canvas.drawImageRect(img, srcRect, dstRect, drawPaint);
    drawPaint.delete();

    if (paint?.borderRadius && this.hasNonZeroRadius(paint.borderRadius)) {
      canvas.restore();
    }
  }

  // ── SVG Path ──

  private drawSvgPathItem(item: DisplayItemJson): void {
    const { bounds, pathData, viewBox } = item;
    const svgPaint = (item as any).paint || item.svgPaint;
    if (!pathData || pathData.length === 0) return;

    const CK = this.CK;
    const canvas = this.ckCanvas;

    const combinedSvg = pathData.join(' ');
    const path = CK.Path.MakeFromSVGString(combinedSvg);
    if (!path) return;

    const vb = viewBox || [0, 0, 100, 100];
    const scaleX = bounds.width / (vb[2] || 100);
    const scaleY = bounds.height / (vb[3] || 100);

    canvas.save();
    canvas.translate(bounds.x, bounds.y);
    canvas.scale(scaleX, scaleY);
    canvas.translate(-vb[0], -vb[1]);

    if (svgPaint?.fill) {
      const fillPaint = this.makeFillPaint(svgPaint.fill);
      path.setFillType(CK.FillType.Winding);
      canvas.drawPath(path, fillPaint);
      fillPaint.delete();
    }

    if (svgPaint?.strokeWidth && svgPaint.strokeWidth > 0 && svgPaint.strokeColor) {
      const strokePaint = new CK.Paint();
      const sc = svgPaint.strokeColor;
      strokePaint.setColor(CK.Color4f(sc.r / 255, sc.g / 255, sc.b / 255, sc.a / 255));
      strokePaint.setStyle(CK.PaintStyle.Stroke);
      strokePaint.setStrokeWidth(svgPaint.strokeWidth);
      if (svgPaint.strokeDasharray) {
        strokePaint.setPathEffect(
          CK.PathEffect.MakeDash(
            [svgPaint.strokeDasharray],
            svgPaint.strokeDashoffset || 0,
          ),
        );
      }
      canvas.drawPath(path, strokePaint);
      strokePaint.delete();
    }

    path.delete();
    canvas.restore();
  }

  // ── DrawScript (CanvasCommands) ──

  private drawScriptItem(item: DisplayItemJson): void {
    const { commands, bounds } = item;
    if (!commands || commands.length === 0) return;

    this.ckCanvas.save();
    this.ckCanvas.translate(bounds.x, bounds.y);

    const state: DrawScriptState = {
      lineWidth: 1,
      globalAlpha: 1,
      lineCap: 'butt',
      lineJoin: 'miter',
      pathStr: '',
    };

    for (const cmd of commands) {
      this.executeCanvasCommand(cmd, state);
    }

    this.ckCanvas.restore();
  }

  private executeCanvasCommand(cmd: CanvasCommandJson, state: DrawScriptState): void {
    const CK = this.CK;
    const canvas = this.ckCanvas;

    switch (cmd.type) {
      case 'save':
        canvas.save();
        break;
      case 'restore':
        canvas.restore();
        break;
      case 'translate':
        canvas.translate((cmd.x as number) || 0, (cmd.y as number) || 0);
        break;
      case 'scale':
        canvas.scale((cmd.x as number) || 1, (cmd.y as number) || 1);
        break;
      case 'rotate':
        canvas.rotate((cmd.degrees as number) || 0, 0, 0);
        break;

      case 'setFillStyle': {
        state.fillColor = cmd.color as Color4f;
        break;
      }
      case 'setStrokeStyle': {
        state.strokeColor = cmd.color as Color4f;
        break;
      }
      case 'setLineWidth':
        state.lineWidth = (cmd.width as number) || 1;
        break;
      case 'setGlobalAlpha':
        state.globalAlpha = (cmd.alpha as number) || 1;
        break;

      case 'fillRect': {
        const paint = new CK.Paint();
        paint.setStyle(CK.PaintStyle.Fill);
        if (state.fillColor) {
          paint.setColor(CK.Color4f(
            state.fillColor.r / 255,
            state.fillColor.g / 255,
            state.fillColor.b / 255,
            (state.fillColor.a / 255) * state.globalAlpha,
          ));
        }
        canvas.drawRect(
          CK.XYWHRect(
            (cmd.x as number) || 0,
            (cmd.y as number) || 0,
            (cmd.width as number) || 0,
            (cmd.height as number) || 0,
          ),
          paint,
        );
        paint.delete();
        break;
      }

      case 'fillRRect': {
        const paint = this.makeStateFillPaint(state);
        const rrect = CK.RRectXY(
          CK.XYWHRect(
            (cmd.x as number) || 0,
            (cmd.y as number) || 0,
            (cmd.width as number) || 0,
            (cmd.height as number) || 0,
          ),
          (cmd.radius as number) || 0,
          (cmd.radius as number) || 0,
        );
        canvas.drawRRect(rrect, paint);
        paint.delete();
        break;
      }

      case 'strokeRect': {
        const paint = new CK.Paint();
        paint.setStyle(CK.PaintStyle.Stroke);
        paint.setStrokeWidth(state.lineWidth);
        if (state.strokeColor) {
          paint.setColor(CK.Color4f(
            state.strokeColor.r / 255,
            state.strokeColor.g / 255,
            state.strokeColor.b / 255,
            (state.strokeColor.a / 255) * state.globalAlpha,
          ));
        }
        canvas.drawRect(
          CK.XYWHRect(
            (cmd.x as number) || 0,
            (cmd.y as number) || 0,
            (cmd.width as number) || 0,
            (cmd.height as number) || 0,
          ),
          paint,
        );
        paint.delete();
        break;
      }

      case 'fillCircle': {
        const paint = this.makeStateFillPaint(state);
        canvas.drawCircle(
          (cmd.cx as number) || 0,
          (cmd.cy as number) || 0,
          (cmd.radius as number) || 0,
          paint,
        );
        paint.delete();
        break;
      }

      case 'strokeCircle': {
        const paint = new CK.Paint();
        paint.setStyle(CK.PaintStyle.Stroke);
        paint.setStrokeWidth(state.lineWidth);
        if (state.strokeColor) {
          paint.setColor(CK.Color4f(
            state.strokeColor.r / 255,
            state.strokeColor.g / 255,
            state.strokeColor.b / 255,
            (state.strokeColor.a / 255) * state.globalAlpha,
          ));
        }
        canvas.drawCircle(
          (cmd.cx as number) || 0,
          (cmd.cy as number) || 0,
          (cmd.radius as number) || 0,
          paint,
        );
        paint.delete();
        break;
      }

      case 'drawLine': {
        const paint = new CK.Paint();
        paint.setStyle(CK.PaintStyle.Stroke);
        paint.setStrokeWidth(state.lineWidth);
        if (state.strokeColor) {
          paint.setColor(CK.Color4f(
            state.strokeColor.r / 255,
            state.strokeColor.g / 255,
            state.strokeColor.b / 255,
            (state.strokeColor.a / 255) * state.globalAlpha,
          ));
        }
        canvas.drawLine(
          (cmd.x0 as number) || 0,
          (cmd.y0 as number) || 0,
          (cmd.x1 as number) || 1,
          (cmd.y1 as number) || 1,
          paint,
        );
        paint.delete();
        break;
      }

      case 'drawText': {
        const fontSize = (cmd.fontSize as number) || 16;
        const font = new CK.Font(null, fontSize);
        const paint = this.makeStateFillPaint(state);
        paint.setAntiAlias((cmd.antiAlias as boolean) !== false);
        if (cmd.stroke as boolean) {
          paint.setStyle(CK.PaintStyle.Stroke);
          paint.setStrokeWidth((cmd.strokeWidth as number) || 1);
        }
        canvas.drawText(
          (cmd.text as string) || '',
          (cmd.x as number) || 0,
          (cmd.y as number) || 0,
          paint,
          font,
        );
        font.delete();
        paint.delete();
        break;
      }

      case 'beginPath':
        state.pathStr = '';
        break;
      case 'moveTo':
        state.pathStr += `M${(cmd.x as number) || 0},${(cmd.y as number) || 0} `;
        break;
      case 'lineTo':
        state.pathStr += `L${(cmd.x as number) || 0},${(cmd.y as number) || 0} `;
        break;
      case 'quadTo':
        state.pathStr += `Q${(cmd.cx as number) || 0},${(cmd.cy as number) || 0},${(cmd.x as number) || 0},${(cmd.y as number) || 0} `;
        break;
      case 'cubicTo':
        state.pathStr += `C${(cmd.c1x as number) || 0},${(cmd.c1y as number) || 0},${(cmd.c2x as number) || 0},${(cmd.c2y as number) || 0},${(cmd.x as number) || 0},${(cmd.y as number) || 0} `;
        break;
      case 'closePath':
        state.pathStr += 'Z ';
        break;
      case 'fillPath': {
        if (state.pathStr) {
          const path = CK.Path.MakeFromSVGString(state.pathStr);
          if (path) {
            const paint = this.makeStateFillPaint(state);
            canvas.drawPath(path, paint);
            paint.delete();
            path.delete();
          }
        }
        state.pathStr = '';
        break;
      }
      case 'strokePath': {
        if (state.pathStr) {
          const path = CK.Path.MakeFromSVGString(state.pathStr);
          if (path) {
            const paint = new CK.Paint();
            paint.setStyle(CK.PaintStyle.Stroke);
            paint.setStrokeWidth(state.lineWidth);
            if (state.strokeColor) {
              paint.setColor(CK.Color4f(
                state.strokeColor.r / 255,
                state.strokeColor.g / 255,
                state.strokeColor.b / 255,
                (state.strokeColor.a / 255) * state.globalAlpha,
              ));
            }
            canvas.drawPath(path, paint);
            paint.delete();
            path.delete();
          }
        }
        state.pathStr = '';
        break;
      }

      case 'drawImage': {
        const assetId = cmd.assetId as string;
        const img = this.loadedImages.get(assetId);
        if (img) {
          const paint = new CK.Paint();
          paint.setAlphaf((cmd.alpha as number) || 1.0);
          const dstRect = CK.XYWHRect(
            (cmd.x as number) || 0,
            (cmd.y as number) || 0,
            (cmd.width as number) || img.width(),
            (cmd.height as number) || img.height(),
          );
          if (cmd.srcRect) {
            const srcArr = cmd.srcRect as number[];
            const srcRect = CK.XYWHRect(srcArr[0], srcArr[1], srcArr[2], srcArr[3]);
            canvas.drawImageRect(img, srcRect, dstRect, paint);
          } else {
            canvas.drawImage(img, (cmd.x as number) || 0, (cmd.y as number) || 0, paint);
          }
          paint.delete();
        }
        break;
      }

      case 'drawImageSimple': {
        const assetId = cmd.assetId as string;
        const img = this.loadedImages.get(assetId);
        if (img) {
          const paint = new CK.Paint();
          paint.setAlphaf((cmd.alpha as number) || 1.0);
          canvas.drawImage(img, (cmd.x as number) || 0, (cmd.y as number) || 0, paint);
          paint.delete();
        }
        break;
      }

      case 'clipRect':
        canvas.clipRect(
          CK.XYWHRect(
            (cmd.x as number) || 0,
            (cmd.y as number) || 0,
            (cmd.width as number) || 0,
            (cmd.height as number) || 0,
          ),
          CK.ClipOp.Intersect,
          (cmd.antiAlias as boolean) !== false,
        );
        break;

      case 'clear': {
        const color = cmd.color as Color4f | undefined;
        if (color) {
          canvas.clear(CK.Color4f(color.r / 255, color.g / 255, color.b / 255, color.a / 255));
        }
        break;
      }
    }
  }

  // ── Transform & Clip ──

  private applyTransform(t: DisplayTransformJson): void {
    // Always apply the base translation
    this.ckCanvas.translate(t.translationX, t.translationY);

    // Early return if no additional transforms (matching Rust's early return)
    if (t.transforms.length === 0) return;

    // Calculate center from bounds (matching Rust's layout_rect_to_skia)
    const centerX = t.bounds.width / 2;
    const centerY = t.bounds.height / 2;

    // REVERSE iteration to match Rust's .rev()
    for (let i = t.transforms.length - 1; i >= 0; i--) {
      const xf = t.transforms[i];
      switch (xf.type) {
        case 'translate':
          // Translate: no pivot
          this.ckCanvas.translate(xf.x || 0, xf.y || 0);
          break;
        case 'translateX':
          this.ckCanvas.translate(xf.value || 0, 0);
          break;
        case 'translateY':
          this.ckCanvas.translate(0, xf.value || 0);
          break;
        case 'scale':
          // T(center) * S * T(-center) — matching Rust's three-step sequence
          this.ckCanvas.translate(centerX, centerY);
          this.ckCanvas.scale(xf.value || 1, xf.value || 1);
          this.ckCanvas.translate(-centerX, -centerY);
          break;
        case 'scaleX':
          this.ckCanvas.translate(centerX, centerY);
          this.ckCanvas.scale(xf.value || 1, 1);
          this.ckCanvas.translate(-centerX, -centerY);
          break;
        case 'scaleY':
          this.ckCanvas.translate(centerX, centerY);
          this.ckCanvas.scale(1, xf.value || 1);
          this.ckCanvas.translate(-centerX, -centerY);
          break;
        case 'rotate':
          // CanvasKit rotate(degrees, px, py) — same as Skia, takes degrees
          this.ckCanvas.rotate(xf.value || 0, centerX, centerY);
          break;
        case 'skewX':
          this.ckCanvas.translate(centerX, centerY);
          this.ckCanvas.skew(Math.tan((xf.value || 0) * Math.PI / 180), 0);
          this.ckCanvas.translate(-centerX, -centerY);
          break;
        case 'skewY':
          this.ckCanvas.translate(centerX, centerY);
          this.ckCanvas.skew(0, Math.tan((xf.value || 0) * Math.PI / 180));
          this.ckCanvas.translate(-centerX, -centerY);
          break;
        case 'skew':
          this.ckCanvas.translate(centerX, centerY);
          this.ckCanvas.skew(
            Math.tan((xf.x || 0) * Math.PI / 180),
            Math.tan((xf.y || 0) * Math.PI / 180),
          );
          this.ckCanvas.translate(-centerX, -centerY);
          break;
      }
    }
  }

  private applyClip(clip: { bounds: DisplayRect; borderRadius: BorderRadius }): void {
    const { bounds, borderRadius } = clip;
    const CK = this.CK;

    if (this.isUniformRadius(borderRadius) && borderRadius.topLeft > 0) {
      const rrect = CK.RRectXY(
        CK.XYWHRect(bounds.x, bounds.y, bounds.width, bounds.height),
        borderRadius.topLeft,
        borderRadius.topLeft,
      );
      this.ckCanvas.clipRRect(rrect, CK.ClipOp.Intersect, true);
    } else if (this.hasNonZeroRadius(borderRadius)) {
      const radii = [
        borderRadius.topLeft, borderRadius.topLeft,
        borderRadius.topRight, borderRadius.topRight,
        borderRadius.bottomRight, borderRadius.bottomRight,
        borderRadius.bottomLeft, borderRadius.bottomLeft,
      ];
      const rect = CK.XYWHRect(bounds.x, bounds.y, bounds.width, bounds.height);
      const rrect = Float32Array.of(rect[0], rect[1], rect[2], rect[3], radii[0], radii[1], radii[2], radii[3], radii[4], radii[5], radii[6], radii[7]);
      this.ckCanvas.clipRRect(rrect, CK.ClipOp.Intersect, true);
    } else {
      this.ckCanvas.clipRect(
        CK.XYWHRect(bounds.x, bounds.y, bounds.width, bounds.height),
        CK.ClipOp.Intersect,
        true,
      );
    }
  }

  // ── Paint helpers ──

  private makeFillPaint(fill: BackgroundFillJson): any {
    const CK = this.CK;
    const paint = new CK.Paint();
    paint.setStyle(CK.PaintStyle.Fill);
    paint.setAntiAlias(true);

    if (fill.type === 'solid') {
      const r = fill.r ?? fill.color?.r ?? 0;
      const g = fill.g ?? fill.color?.g ?? 0;
      const b = fill.b ?? fill.color?.b ?? 0;
      const a = fill.a ?? fill.color?.a ?? 255;
      paint.setColor(CK.Color4f(r / 255, g / 255, b / 255, a / 255));
    } else if (fill.type === 'linearGradient' && fill.from && fill.to) {
      const from = fill.from;
      const to = fill.to;
      const colors = [
        CK.Color4f(from.r / 255, from.g / 255, from.b / 255, from.a / 255),
        CK.Color4f(to.r / 255, to.g / 255, to.b / 255, to.a / 255),
      ];
      if (fill.via) {
        const via = fill.via;
        colors.splice(1, 0,
          CK.Color4f(via.r / 255, via.g / 255, via.b / 255, via.a / 255),
        );
      }
      const pos = fill.via ? [0, 0.5, 1] : [0, 1];
      const shader = CK.Shader.MakeLinearGradient(
        [0, 0],
        fill.direction === 'toRight' ? [1, 0] : [0, 1],
        colors,
        pos,
        CK.TileMode.Clamp,
      );
      paint.setShader(shader);
    }

    return paint;
  }

  private makeStateFillPaint(state: { fillColor?: Color4f; globalAlpha: number }): any {
    const CK = this.CK;
    const paint = new CK.Paint();
    paint.setStyle(CK.PaintStyle.Fill);
    paint.setAntiAlias(true);
    if (state.fillColor) {
      paint.setColor(CK.Color4f(
        state.fillColor.r / 255,
        state.fillColor.g / 255,
        state.fillColor.b / 255,
        (state.fillColor.a / 255) * state.globalAlpha,
      ));
    }
    return paint;
  }

  private drawRoundRect(b: DisplayRect, br: BorderRadius, paint: any): void {
    const CK = this.CK;

    if (this.hasNonZeroRadius(br)) {
      if (this.isUniformRadius(br)) {
        const rrect = CK.RRectXY(
          CK.XYWHRect(b.x, b.y, b.width, b.height),
          br.topLeft,
          br.topLeft,
        );
        this.ckCanvas.drawRRect(rrect, paint);
      } else {
        const radii = [
          br.topLeft, br.topLeft,
          br.topRight, br.topRight,
          br.bottomRight, br.bottomRight,
          br.bottomLeft, br.bottomLeft,
        ];
        const rect = CK.XYWHRect(b.x, b.y, b.width, b.height);
        const rrect = Float32Array.of(rect[0], rect[1], rect[2], rect[3], radii[0], radii[1], radii[2], radii[3], radii[4], radii[5], radii[6], radii[7]);
        this.ckCanvas.drawRRect(rrect, paint);
      }
    } else {
      this.ckCanvas.drawRect(CK.XYWHRect(b.x, b.y, b.width, b.height), paint);
    }
  }

  private drawBorder(b: DisplayRect, paint: RectPaintJson): void {
    const CK = this.CK;
    const stroke = new CK.Paint();
    stroke.setStyle(CK.PaintStyle.Stroke);
    stroke.setAntiAlias(true);
    const borderColor = paint.borderColor!;
    stroke.setColor(CK.Color4f(
      borderColor.r / 255,
      borderColor.g / 255,
      borderColor.b / 255,
      borderColor.a / 255,
    ));

    const bw = paint.borderWidth || 1;
    stroke.setStrokeWidth(bw);

    if (paint.borderTopWidth || paint.borderRightWidth
      || paint.borderBottomWidth || paint.borderLeftWidth) {
      const maxBw = Math.max(
        paint.borderTopWidth || bw,
        paint.borderRightWidth || bw,
        paint.borderBottomWidth || bw,
        paint.borderLeftWidth || bw,
      );
      stroke.setStrokeWidth(maxBw);
    }

    const inset = bw / 2;
    this.drawRoundRect(
      { x: b.x + inset, y: b.y + inset, width: b.width - bw, height: b.height - bw },
      paint.borderRadius,
      stroke,
    );
    stroke.delete();
  }

  private drawBoxShadow(
    b: DisplayRect,
    shadow: { offsetX: number; offsetY: number; blurSigma: number; spread: number; color: Color4f },
    br: BorderRadius,
  ): void {
    const CK = this.CK;
    const paint = new CK.Paint();
    const c = shadow.color;
    paint.setColor(CK.Color4f(c.r / 255, c.g / 255, c.b / 255, c.a / 255));

    if (shadow.blurSigma > 0) {
      paint.setMaskFilter(CK.MaskFilter.MakeBlur(
        CK.BlurStyle.Normal,
        shadow.blurSigma,
        false,
      ));
    }

    const spread = shadow.spread || 0;
    const shadowRect = {
      x: b.x + (shadow.offsetX || 0) - spread,
      y: b.y + (shadow.offsetY || 0) - spread,
      width: b.width + spread * 2,
      height: b.height + spread * 2,
    };

    this.drawRoundRect(shadowRect, br, paint);
    paint.delete();
  }

  private drawInsetShadow(
    b: DisplayRect,
    shadow: { offsetX: number; offsetY: number; blurSigma: number; spread: number; color: Color4f },
    br: BorderRadius,
  ): void {
    const CK = this.CK;
    const paint = new CK.Paint();
    const c = shadow.color;
    paint.setColor(CK.Color4f(c.r / 255, c.g / 255, c.b / 255, c.a / 255));

    const offsetX = shadow.offsetX || 0;
    const offsetY = shadow.offsetY || 0;
    const blur = shadow.blurSigma || 0;

    this.ckCanvas.save();
    this.applyClip({ bounds: b, borderRadius: br });
    const shadowRect = {
      x: b.x + offsetX - blur,
      y: b.y + offsetY - blur,
      width: b.width + blur * 2,
      height: b.height + blur * 2,
    };
    this.drawRoundRect(shadowRect, br, paint);
    this.ckCanvas.restore();

    paint.delete();
  }

  // ── Glyph helpers ──

  private buildGlyphPath(commands: DisplayGlyphCommand[]): any {
    const parts: string[] = [];
    for (const cmd of commands) {
      switch (cmd.type) {
        case 'moveTo':
          parts.push(`M${cmd.x},${-cmd.y}`);
          break;
        case 'lineTo':
          parts.push(`L${cmd.x},${-cmd.y}`);
          break;
        case 'quadTo':
          parts.push(`Q${cmd.cx},${-cmd.cy},${cmd.x},${-cmd.y}`);
          break;
        case 'curveTo':
          parts.push(`C${cmd.c1x},${-cmd.c1y},${cmd.c2x},${-cmd.c2y},${cmd.x},${-cmd.y}`);
          break;
        case 'close':
          parts.push('Z');
          break;
      }
    }
    return this.CK.Path.MakeFromSVGString(parts.join(' '));
  }

  private makeImageFromRgba(rgba: number[], width: number, height: number): any {
    if (rgba.length === 0 || width === 0 || height === 0) return null;
    const CK = this.CK;
    const imageInfo = {
      width,
      height,
      colorType: CK.ColorType.RGBA_8888,
      alphaType: CK.AlphaType.Unpremul,
      colorSpace: CK.ColorSpace.SRGB,
    };
    return CK.MakeImage(imageInfo, new Uint8Array(rgba), width * 4);
  }

  private drawGlyphDropShadow(path: any, shadow: DropShadowJson): void {
    const CK = this.CK;
    const paint = new CK.Paint();
    const c = shadow.color;
    paint.setColor(CK.Color4f(c.r / 255, c.g / 255, c.b / 255, c.a / 255));
    paint.setStyle(CK.PaintStyle.Fill);
    paint.setAntiAlias(true);
    if (shadow.blurSigma > 0) {
      paint.setMaskFilter(
        CK.MaskFilter.MakeBlur(CK.BlurStyle.Normal, shadow.blurSigma, false),
      );
    }
    this.ckCanvas.save();
    this.ckCanvas.translate(shadow.offsetX || 0, shadow.offsetY || 0);
    this.ckCanvas.drawPath(path, paint);
    this.ckCanvas.restore();
    paint.delete();
  }

  private computeTextXShift(
    lineWidth: number,
    containerWidth: number,
    align: string,
  ): number {
    switch (align) {
      case 'center':
        return (containerWidth - lineWidth) / 2;
      case 'right':
        return containerWidth - lineWidth;
      default:
        return 0;
    }
  }

  // ── Radius helpers ──

  private isUniformRadius(br: BorderRadius): boolean {
    return br.topLeft === br.topRight
      && br.topLeft === br.bottomRight
      && br.topLeft === br.bottomLeft;
  }

  private hasNonZeroRadius(br: BorderRadius): boolean {
    return br.topLeft > 0 || br.topRight > 0 || br.bottomRight > 0 || br.bottomLeft > 0;
  }

  // ── Cache management ──

  clearCaches(): void {
    for (const pic of this.subtreePicCache.values()) pic.delete();
    this.subtreePicCache.clear();

    for (const img of this.subtreeImgCache.values()) img.delete();
    this.subtreeImgCache.clear();

    for (const pic of this.itemPicCache.values()) pic.delete();
    this.itemPicCache.clear();

    for (const path of this.glyphPathCache.values()) path.delete();
    this.glyphPathCache.clear();

    for (const img of this.glyphImgCache.values()) img.delete();
    this.glyphImgCache.clear();

    for (const img of this.imageCache.values()) img.delete();
    this.imageCache.clear();
  }
}
