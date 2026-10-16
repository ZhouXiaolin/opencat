/* G2 API shim — AntV G2 charts on an OpenCat <canvas>.
 *
 * The G2 bundle (`g2.bundle.js`, built from @antv/g2 + @antv/g-canvaskit) runs
 * in the same script realm as every other runtime. It only ever talks to
 * `@antv/g-canvaskit`, whose renderer reaches CanvasKit through two hooks:
 *
 *   1. `CanvasKitInit` — the wasm loader. Here it returns our own facade
 *      (`canvas_api.js`'s globalThis.CanvasKit), extended with the few extras
 *      g-canvaskit needs that the 2D subset does not expose (Paragraph /
 *      FontMgr / Typeface / image shaders / Surface).
 *   2. `MakeWebGLCanvasSurface(canvas)` — a GL surface. Here it returns a
 *      surface whose `getCanvas()` writes every draw call straight into the
 *      node's DrawOp stream through the existing `__canvas_*` bindings, so a
 *      chart is just more canvas commands: same z-order, same clip/transform
 *      stack, same compositor as hand-written CanvasKit code.
 *
 * Because the surface records ops instead of rasterizing, nothing has to be
 * handed back across the bridge: the chart's pixels are produced by the normal
 * rendering path.
 *
 * Determinism: G2 drives its own rAF loop. In a composition every frame is
 * `frame = f(t)`, so this shim installs a virtual clock that is advanced by
 * the chart host (`__ocG2SetTime`) before each replay, and rAF queues are
 * drained once per advance. `performance.now()` reads the same clock.
 */
(function () {
    'use strict';

    var hostLog = function (level, message) {
        if (typeof globalThis.__ocG2Log === 'function') {
            globalThis.__ocG2Log(String(level), String(message));
        }
    };

    // ── virtual clock ────────────────────────────────────────────────
    // g2-rs advances 100ms per rAF callback; here the composition host sets the
    // frame's time in milliseconds before the render pass, so G2's tween
    // engine, animation timeline and rAF-driven renderer all see one clock.
    var clockMs = 0;
    var rafId = 1;
    // The queue lives on `globalThis.__ocG2RafQueue` so that the raf-polyfill
    // inside the G2 bundle picks it up at bundle-eval time (it caches
    // `root.requestAnimationFrame` and re-assigns it to itself). g's Canvas
    // rAF and the shim below are then the same queue.
    var rafQueue = globalThis.__ocG2RafQueue;
    // The canvas node the chart being built targets. Set by `__ocG2Frame`
    // before `build` runs and read when G2 asks for a surface, which happens
    // inside the renderer it was constructed with. One chart is built at a
    // time, so a single slot is enough.
    var currentNodeId = '';
    var currentWidth = 0;
    var currentHeight = 0;
    globalThis.__ocG2SetTime = function (secs) {
        clockMs = Math.max(0, Number(secs) || 0) * 1000;
    };

    function now() { return clockMs; }

    /* Run the queued frame callbacks.

       One flush is one composition frame: the queue as it stands when the
       host asks, plus a bounded number of follow-up passes for callbacks that
       were queued *while* running. g's canvas keeps re-registering its draw
       loop at the end of every frame, so "queue is empty" never happens; the
       cap is what stops one host frame from rendering forever. Three passes
       cover g-canvaskit's draw pass plus the two nested `requestAnimationFrame`
       steps its ready protocol uses before the render promise resolves. */
    var FLUSH_PASSES = 3;

    function flushRaf() {
        for (var pass = 0; pass < FLUSH_PASSES; pass++) {
            var batch = globalThis.__ocG2RafQueue;
            globalThis.__ocG2RafQueue = [];
            if (!batch.length) break;
            for (var i = 0; i < batch.length; i++) {
                try {
                    batch[i](clockMs);
                } catch (e) {
                    hostLog('warn', 'g2: frame callback threw: ' + (e && e.message));
                }
            }
        }
    }
    globalThis.__ocG2Flush = flushRaf;

    var performanceShim = {
        now: now,
        mark: function () {},
        measure: function () {},
        getEntriesByName: function () { return []; },
        getEntriesByType: function () { return []; },
        clearMarks: function () {},
        clearMeasures: function () {},
    };

    // ── CanvasKit extras g-canvaskit needs ───────────────────────────
    // `globalThis.CanvasKit` (canvas_api.js) already has Paint / Path / Font /
    // Shader (gradient) / RuntimeEffect / the color and rect helpers. G2 also
    // needs text layout + font management. Text is already supported through
    // `__surface_*` (see `makeSurface` in canvas_api.js), so paragraphs are
    // measured with the same real-font path and drawn through the surface.

    var surfaceSeq = 0;
    var utf8 = new TextEncoder();

    function colorCss(color) {
        var c = globalThis.CanvasKit.multiplyByAlpha(color, 1);
        return 'rgba(' + Math.round(c[0] * 255) + ',' + Math.round(c[1] * 255) + ','
            + Math.round(c[2] * 255) + ',' + c[3] + ')';
    }

    function numberOr(value, fallback) {
        var n = Number(value);
        return Number.isFinite(n) ? n : fallback;
    }

    // One paragraph = one measurement + one draw. Align is resolved by the
    // caller into a layout x/y, matching how g-canvaskit lays text out before
    // it reaches CanvasKit.
    function FakeParagraph(text, paragraphStyle, textStyle) {
        this.text = String(text == null ? '' : text);
        this.paragraphStyle = paragraphStyle || {};
        this.textStyle = textStyle || {};
        this.width = 0;
        this.height = 0;
    }
    FakeParagraph.prototype.layout = function (width) {
        this.width = Math.max(0, numberOr(width, 0));
    };
    FakeParagraph.prototype.getMaxWidth = function () {
        return this._metrics().width;
    };
    FakeParagraph.prototype.getHeight = function () {
        return this._metrics().height;
    };
    FakeParagraph.prototype.getLineMetrics = function () {
        var m = this._metrics();
        return [{
            startIndex: 0,
            endIndex: this.text.length,
            ascent: m.height * 0.8,
            descent: m.height * 0.2,
            left: 0,
            width: m.width,
            height: m.height,
            baseline: m.height * 0.8,
        }];
    };
    FakeParagraph.prototype.getRectsForRange = function () {
        var m = this._metrics();
        return [{ rect: { x: 0, y: 0, w: m.width, h: m.height } }];
    };
    FakeParagraph.prototype._metrics = function () {
        if (this.__metrics) return this.__metrics;
        var style = this.textStyle || {};
        var family = Array.isArray(style.fontFamilies) && style.fontFamilies.length
            ? String(style.fontFamilies[0]).replace(/["']/g, '')
            : 'sans-serif';
        var size = numberOr(style.fontSize, 12);
        var weight = numberOr(style.fontStyle && style.fontStyle.weight, 400);
        // Measured through the same real-font path that draws the glyphs.
        var measured = __canvas_measure_paragraph(this.text, size, family, weight);
        this.__metrics = {
            width: numberOr(measured, 0),
            height: size * (numberOr(style.heightMultiplier, 1) || 1),
        };
        return this.__metrics;
    };
    FakeParagraph.prototype.delete = function () {};

    function FakeParagraphBuilder(paragraphStyle) {
        this.paragraphStyle = paragraphStyle || {};
        this.textStyle = {};
        this.parts = [];
        this.layout = {};
    }
    FakeParagraphBuilder.prototype.addText = function (text) {
        this.parts.push(String(text));
        return this;
    };
    FakeParagraphBuilder.prototype.pushStyle = function (style) {
        if (style) {
            var next = Object.assign({}, this.textStyle);
            for (var key in style) {
                if (Object.prototype.hasOwnProperty.call(style, key)) {
                    var value = style[key];
                    // Skia fontStyle arrives as { weight: { value } }.
                    if (key === 'fontStyle' && value && typeof value === 'object'
                        && value.weight && typeof value.weight === 'object') {
                        next.fontStyle = { weight: value.weight };
                    } else {
                        next[key] = value;
                    }
                }
            }
            this.textStyle = next;
        }
        return this;
    };
    FakeParagraphBuilder.prototype.pop = function () { return this; };
    FakeParagraphBuilder.prototype.build = function () {
        return new FakeParagraph(this.parts.join(''), this.paragraphStyle, this.textStyle);
    };
    FakeParagraphBuilder.prototype.delete = function () {};

    // g-canvaskit draws text through Paragraph shapes; we reuse the real-font
    // offscreen surface so glyphs and metrics match the rest of OpenCat.
    function drawParagraph(canvasId, paragraph, x, y) {
        var text = paragraph.text;
        if (!text) return;
        var style = paragraph.textStyle || {};
        var family = Array.isArray(style.fontFamilies) && style.fontFamilies.length
            ? String(style.fontFamilies[0]).replace(/["']/g, '')
            : 'sans-serif';
        var size = numberOr(style.fontSize, 12);
        var weight = numberOr(style.fontStyle && style.fontStyle.weight, 400);
        var color = Array.isArray(style.color) ? style.color : [0, 0, 0, 1];
        // The surface is a per-node scratch buffer sized by the caller
        // (one paragraph per chart node per frame), so the paragraph's own
        // measured box is the destination rect.
        var width = numberOr(paragraph.getMaxWidth(), 0);
        var height = numberOr(paragraph.getHeight(), size);
        __canvas_draw_paragraph(
            canvasId,
            text,
            size,
            family,
            weight,
            colorCss(color),
            numberOr(x, 0),
            numberOr(y, 0),
            width,
            height
        );
    }

    // g-canvaskit's `loadCanvaskit()` resolves through this. The bundle's
    // canvaskit-wasm glue is patched out (it cannot run in this realm), so the
    // only thing that answers is our own facade.
    globalThis.__ocCanvasKitInit = function (options) {
        return Promise.resolve(extendCanvasKit());
    };

    function extendCanvasKit() {
        var CK = globalThis.CanvasKit;
        if (!CK) throw new Error('g2: CanvasKit facade is not installed');
        CK.ParagraphStyle = function ParagraphStyle(style) { this.style = style || {}; };
        CK.ParagraphBuilder = { Make: function (style) { return new FakeParagraphBuilder(style); } };
        CK.FontMgr = {
            FromData: function () {
                return {
                    matchFamilyStyle: function () { return null; },
                    countFamilies: function () { return 0; },
                    getFamilyName: function () { return ''; },
                    delete: function () {},
                };
            },
        };
        CK.Typeface = {
            MakeDefault: function () { return null; },
            MakeFreeTypeFaceFromData: function () { return null; },
            MakeFromName: function () { return null; },
        };
        CK.TypefaceFontProvider = {
            Make: function () {
                return { registerFont: function () {}, matchFamilyStyle: function () { return null; } };
            },
        };
        // Font handles are inert: text goes through the surface path, which
        // resolves real families itself (the compositor owns the fonts).
        CK.Font = function FakeCKFont(typeface, size) {
            this._size = numberOr(size, 16);
        };
        CK.Font.prototype.getSize = function () { return this._size; };
        CK.Font.prototype.getMetrics = function () {
            return { ascent: this._size * 0.8, descent: this._size * 0.2 };
        };
        CK.Font.prototype.delete = function () {};
        CK.TextDirection = { LTR: 1, RTL: 0 };
        CK.TextAlign = { Left: 0, Right: 1, Center: 2, Justify: 3, Start: 4, End: 5 };
        CK.RectHeightStyle = { Tight: 0, Max: 1 };
        CK.FontStyle = FakeCKFontStyle;
        CK.MakeWebGLCanvasSurface = function (canvas) { return makeG2Surface(canvas); };
        CK.MakeCanvasSurface = CK.MakeWebGLCanvasSurface;
        CK.Color4f = function (r, g, b, a) { return [r, g, b, a == null ? 1 : a]; };
        // g-canvaskit clears the surface with a Color4f before drawing and
        // builds fills from gradient shaders; both are read by the recorder,
        // which resolves a [r,g,b,a] list into its own color representation.
        CK.TileMode = { Clamp: 0, Repeat: 1, Mirror: 2, Decal: 3 };
        CK.TextBaseline = { Alphabetic: 0, Top: 1, Bottom: 2, Middle: 3, Ideographic: 4, Hanging: 5 };
        CK.FontEdging = { Alias: 0, AntiAlias: 1, SubpixelAntiAlias: 2 };
        CK.FontHinting = { None: 0, Slight: 1, Normal: 2, Full: 3 };
        CK.BlurStyle = { Normal: 0, Solid: 1, Outer: 2, Inner: 3 };
        // Text decoration support is only read to map CSS keywords; the
        // recorder draws paragraphs as one run, so decoration is dropped
        // rather than modelled.
        CK.DecorationStyle = { Solid: 0, Double: 1, Dotted: 2, Dashed: 3, Wavy: 4 };
        CK.NoDecoration = 0;
        CK.UnderlineDecoration = 1;
        CK.OverlineDecoration = 2;
        CK.LineThroughDecoration = 4;
        CK.TextDirection = CK.TextDirection || { LTR: 1, RTL: 0 };
        CK.PathEffect = {
            MakeDash: function (intervals, phase) { return { __opencatPathEffect: true, kind: 'dash', intervals: intervals, phase: phase || 0 }; },
            MakeDiscrete: function () { return { __opencatPathEffect: true, kind: 'discrete' }; },
            MakeCompose: function (outer, inner) { return outer || inner; },
            MakeSum: function (a, b) { return a || b; },
            MakeCorner: function (radius) { return { __opencatPathEffect: true, kind: 'corner', radius: radius }; },
        };
        CK.MaskFilter = {
            MakeBlur: function (style, sigma, respectCTM) { return { __opencatMaskFilter: true, style: style, sigma: sigma, respectCTM: respectCTM }; },
        };
        CK.ImageFilter = {
            MakeBlur: function (sigmaX, sigmaY, mode) { return { __opencatImageFilter: true, mode: mode, sigmaX: sigmaX, sigmaY: sigmaY }; },
        };
        CK.PaintStyle = CK.PaintStyle || { Fill: 0, Stroke: 1 };
        CK.StrokeCap = CK.StrokeCap || { Butt: 0, Round: 1, Square: 2 };
        CK.StrokeJoin = CK.StrokeJoin || { Miter: 0, Round: 1, Bevel: 2 };
        CK.ClipOp = CK.ClipOp || { Difference: 0, Intersect: 1 };
        CK.makePathFromSVGString = function () { return null; };
        CK.makePathFromCmds = function () { return null; };
        CK.BlendMode = CK.BlendMode || { Clear: 0, Src: 1, Dst: 2, SrcOver: 3, DstOver: 4 };
        CK.Image = {
            MakeFromEncoded: function () { return { width: function () { return 0; }, height: function () { return 0; }, delete: function () {} }; },
            MakeFromImage: function (image) { return image; },
        };
        CK.Shader = CK.Shader || {};
        // 3x3 row-major is a good enough approximation for what G2 asks of the
        // matrix helpers; the actual shaders are built on the Rust side.
        CK.Matrix = {
            identity: function () { return [1, 0, 0, 0, 1, 0, 0, 0, 1]; },
            rotated: function (radians, px, py) {
                var c = Math.cos(radians), s = Math.sin(radians);
                return [c, s, -s * px, -c * py, s, c, px, py, 1];
            },
            scaled: function (sx, sy) { return [sx, 0, 0, 0, sy, 0, 0, 0, 1]; },
            translated: function (dx, dy) { return [1, 0, 0, 0, 1, 0, dx, dy, 1]; },
            multiply: function (a, b) {
                var out = new Array(9);
                for (var r = 0; r < 3; r++) {
                    for (var c = 0; c < 3; c++) {
                        var sum = 0;
                        for (var k = 0; k < 3; k++) sum += a[r * 3 + k] * b[k * 3 + c];
                        out[r * 3 + c] = sum;
                    }
                }
                return out;
            },
        };
        return CK;
    }


    function FakeCKFontStyle(weight, width, slant) {
        this.weight = weight;
        this.width = width;
        this.slant = slant;
    }
    globalThis.CanvasKit.FontStyle = FakeCKFontStyle;

    // ── the recording surface ────────────────────────────────────────
    // `getCanvas()` forwards to `globalThis.ctx.getCanvasById(nodeId)` — the
    // exact same facade a `useCanvas` callback receives, so G2's draws land on
    // the node's DrawOp stream. `flush()` is a no-op (ops are already in the
    // stream and are replayed by the renderer).
    function makeG2Surface(canvasConfig) {
        var id = currentNodeId;
        var saveIndex = 0;
        return {
            __ocG2Surface: true,
            getCanvas: function () {
                var canvas = globalThis.ctx.getCanvasById(id);
                canvas.__g2ParagraphMargin = 0;
                return {
                    __g2CanvasProxy: true,
                    // Paragraph drawing needs the raw-image binding, which the
                    // public facade does not expose (surface ids are internal).
                    drawParagraph: function (paragraph, x, y) {
                        drawParagraph(id, paragraph, x, y);
                    },
                    clear: function (color) { canvas.clear(color); },
                    flush: function () {},
                    save: function () { canvas.save(); },
                    restore: function () { canvas.restore(); },
                    translate: function (x, y) { canvas.translate(x, y); },
                    scale: function (x, y) { canvas.scale(x, y); },
                    rotate: function (degrees, rx, ry) { canvas.rotate(degrees, rx, ry); },
                    skew: function (x, y) { canvas.skew(x, y); },
                    concat: function (matrix) { canvas.concat(matrix); },
                    clipRect: function (rect, op, aa) { canvas.clipRect(rect, op, aa); },
                    clipRRect: function (rrect, op, aa) { canvas.clipRRect(rrect, op, aa); },
                    clipPath: function (path, op, aa) { canvas.clipPath(path, op, aa); },
                    drawRect: function (rect, paint) { canvas.drawRect(rect, paint); },
                    drawRRect: function (rrect, paint) { canvas.drawRRect(rrect, paint); },
                    drawDRRect: function (o, i, p) { canvas.drawDRRect(o, i, p); },
                    drawOval: function (rect, paint) { canvas.drawOval(rect, paint); },
                    drawCircle: function (cx, cy, r, paint) { canvas.drawCircle(cx, cy, r, paint); },
                    drawArc: function (rect, start, sweep, center, paint) { canvas.drawArc(rect, start, sweep, center, paint); },
                    drawLine: function (x0, y0, x1, y1, paint) { canvas.drawLine(x0, y0, x1, y1, paint); },
                    drawPoints: function (mode, points, paint) { canvas.drawPoints(mode, points, paint); },
                    drawPath: function (path, paint) {
                        globalThis.__ocG2MarkDraws = (globalThis.__ocG2MarkDraws || 0) + 1;
                        canvas.drawPath(path, paint);
                    },
                    drawImage: function (image, x, y, paint) { canvas.drawImage(image, x, y, paint); },
                    drawImageRect: function (image, src, dst, paint, fast) { canvas.drawImageRect(image, src, dst, paint, fast); },
                    drawPaint: function (paint) { canvas.drawPaint(paint); },
                    drawText: function (str, x, y, paint, font) { canvas.drawText(str, x, y, paint, font); },
                    drawTextBlob: function () { hostLog('warn', 'g2: drawTextBlob is unsupported'); },
                    getTotalMatrix: function () { return [1, 0, 0, 0, 1, 0, 0, 0, 1]; },
                    getSaveCount: function () { return saveIndex; },
                };
            },
            flush: function () {},
            width: function () { return currentWidth; },
            height: function () { return currentHeight; },
            getCanvasElement: function () { return canvasConfig; },
            makeImageSnapshot: function () {
                return { encodeToBytes: function () { return new Uint8Array(); }, delete: function () {} };
            },
            deleteLater: function () {},
            delete: function () {},
            isDeleted: function () { return false; },
            drawOnce: function () {},
            // CanvasKit's own `Surface.prototype.requestAnimationFrame` calls
            // the callback with the surface's canvas, which is the argument the
            // g-canvaskit draw loop draws into. The browser rAF the shim exposes
            // has no such argument, so it is handed over here.
            requestAnimationFrame: function (callback) {
                var surface = this;
                return globalThis.requestAnimationFrame(function () {
                    callback(surface.getCanvas());
                    surface.flush();
                });
            },
        };
    }

    // ── DOM / browser shims G2 needs ─────────────────────────────────
    // The chart only reads the container element for sizing and appends its
    // own canvas; nothing depends on layout.
    /* The canvas a frame callback should draw into. g-canvaskit's draw loop
       takes the canvas as the rAF argument, so the shim hands it the node's
       canvas rather than a timestamp. */
    function rafCanvasArg() {
        return globalThis.__ocG2SurfaceForRaf || null;
    }

    function installDomShims(nodeId, width, height) {
        if (globalThis.__ocG2DomInstalled) return;
        globalThis.__ocG2DomInstalled = true;

        function Element(tagName) {
            this.tagName = String(tagName || 'div').toUpperCase();
            this.children = [];
            this.style = {};
            this.attributes = {};
            this.parentElement = null;
            this.parentNode = null;
            this.classList = { add: function () {}, remove: function () {}, contains: function () { return false; } };
        }
        Element.prototype.appendChild = function (child) {
            if (!child) return child;
            child.parentElement = this;
            child.parentNode = this;
            this.children.push(child);
            return child;
        };
        Element.prototype.removeChild = function (child) {
            var index = this.children.indexOf(child);
            if (index >= 0) this.children.splice(index, 1);
            return child;
        };
        Element.prototype.setAttribute = function (name, value) { this.attributes[name] = String(value); };
        Element.prototype.getAttribute = function (name) {
            return Object.prototype.hasOwnProperty.call(this.attributes, name) ? this.attributes[name] : null;
        };
        Element.prototype.removeAttribute = function (name) { delete this.attributes[name]; };
        Element.prototype.addEventListener = function () {};
        Element.prototype.removeEventListener = function () {};
        Element.prototype.dispatchEvent = function () { return true; };
        Element.prototype.getBoundingClientRect = function () {
            return { x: 0, y: 0, left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0 };
        };
        Element.prototype.focus = function () {};
        Element.prototype.blur = function () {};
        Element.prototype.contains = function () { return false; };
        Object.defineProperty(Element.prototype, 'clientWidth', { get: function () { return numberOr(this.style.width, width); } });
        Object.defineProperty(Element.prototype, 'clientHeight', { get: function () { return numberOr(this.style.height, height); } });
        Object.defineProperty(Element.prototype, 'firstChild', { get: function () { return this.children[0] || null; } });
        Object.defineProperty(Element.prototype, 'nextSibling', { get: function () { return null; } });

        // A canvas element that reports the chart's own size; its 2D context is
        // a stub because every draw goes through the CanvasKit surface.
        function CanvasElement() {
            Element.call(this, 'canvas');
            this.width = width;
            this.height = height;
        }
        CanvasElement.prototype = Object.create(Element.prototype);
        CanvasElement.prototype.constructor = CanvasElement;
        CanvasElement.prototype.getContext = function () {
            return {
                canvas: this,
                measureText: function (text) {
                    return { width: String(text).length * 8 };
                },
                fillText: function () {},
                strokeText: function () {},
                clearRect: function () {},
                fillRect: function () {},
                strokeRect: function () {},
                beginPath: function () {},
                closePath: function () {},
                moveTo: function () {},
                lineTo: function () {},
                save: function () {},
                restore: function () {},
                translate: function () {},
                scale: function () {},
                rotate: function () {},
                drawImage: function () {},
                putImageData: function () {},
                getImageData: function (x, y, w, h) {
                    return { width: Math.max(0, w | 0), height: Math.max(0, h | 0), data: new Uint8ClampedArray(Math.max(0, w * h) * 4) };
                },
                createImageData: function (w, h) {
                    return { width: w, height: h, data: new Uint8ClampedArray(w * h * 4) };
                },
                setTransform: function () {},
                createLinearGradient: function () { return { addColorStop: function () {} }; },
                createRadialGradient: function () { return { addColorStop: function () {} }; },
                createPattern: function () { return null; },
                arc: function () {},
                rect: function () {},
                fill: function () {},
                stroke: function () {},
                clip: function () {},
                isPointInPath: function () { return false; },
            };
        };

        var container = new Element('div');
        container.id = nodeId;
        container.style = { position: 'relative', width: width + 'px', height: height + 'px' };

        var documentShim = {
            createElement: function (tag) {
                return String(tag).toLowerCase() === 'canvas' ? new CanvasElement() : new Element(tag);
            },
            createElementNS: function (_ns, tag) {
                return String(tag).toLowerCase() === 'canvas' ? new CanvasElement() : new Element(tag);
            },
            createTextNode: function (text) { return { nodeValue: String(text), parentNode: null }; },
            getElementById: function (id) { return id === nodeId ? container : null; },
            querySelector: function () { return null; },
            querySelectorAll: function () { return []; },
            body: new Element('body'),
            documentElement: new Element('html'),
            addEventListener: function () {},
            removeEventListener: function () {},
        };
        documentShim.body.appendChild = Element.prototype.appendChild;
        globalThis.document = documentShim;
        globalThis.window = globalThis;
        globalThis.self = globalThis;
        globalThis.devicePixelRatio = 1;
        globalThis.performance = performanceShim;
        globalThis.cancelAnimationFrame = function () {};
        globalThis.addEventListener = function () {};
        globalThis.removeEventListener = function () {};
        globalThis.getComputedStyle = function (element) {
            var style = (element && element.style) || {};
            return {
                getPropertyValue: function (name) { return style[name] != null ? style[name] : ''; },
                width: (style.width || 0) + 'px',
                height: (style.height || 0) + 'px',
            };
        };
        globalThis.ResizeObserver = function ResizeObserver() {
            this.observe = function () {};
            this.unobserve = function () {};
            this.disconnect = function () {};
        };
        globalThis.MutationObserver = function MutationObserver() {
            this.observe = function () {};
            this.disconnect = function () {};
        };
        // G2 probes for font support through a measured canvas.
        globalThis.HTMLCanvasElement = CanvasElement;
        globalThis.OffscreenCanvas = CanvasElement;
        // The realm pins the clock at 0 for determinism (see realm.rs); G2's
        // animation engine must read the virtual clock instead.
        globalThis.Date = function () { return new FakeDate(); };
        globalThis.Date.now = function () { return clockMs; };
        globalThis.Date.prototype = {
            getTime: function () { return clockMs; },
            valueOf: function () { return clockMs; },
        };
        FakeDate.prototype = globalThis.Date.prototype;
        function FakeDate() { this.t = clockMs; }
    }

    // ── public entry: one chart render on one canvas node ────────────
    // `__ocG2Frame(nodeId, t, build)`:
    //   1. install the DOM/clock shims and the extended CanvasKit;
    //   2. set the virtual clock to the frame's time;
    //   3. call `build(Chart, node)` with a Chart class whose default renderer
    //      is ours;
    //   4. drain one rAF pass so G2's deferred render commits its ops.
    // The container width/height come from the composition meta, because a
    // <canvas> node is laid out by the frame, not by the DOM.
    globalThis.__ocG2Frame = function (nodeId, t, build) {
        var meta = globalThis.__ocMeta || {};
        var width = numberOr(meta.width, 1920);
        var height = numberOr(meta.height, 1080);
        currentNodeId = nodeId;
        currentWidth = width;
        currentHeight = height;
        installDomShims(nodeId, width, height);
        extendCanvasKit();
        globalThis.__ocG2SetTime(t);

        var ChartClass = globalThis.__ocG2Chart;
        if (!ChartClass) throw new Error('g2: bundle is not installed (__ocG2Chart missing)');

        var container = globalThis.document.getElementById(nodeId);
        var RendererClass = globalThis.__ocG2Renderer;
        var BaseChart = ChartClass;
        // `Chart` is every user's entry point (`new Chart({...})`), so the
        // renderer and the container are injected here rather than asked for.
        // Interaction is meaningless for an offline frame, and G2 installs
        // pointer listeners and a tooltip DOM layer otherwise.
        var Chart = class extends BaseChart {
            constructor(config) {
                var next = Object.assign({}, config || {});
                next.container = container;
                if (!next.renderer) {
                    next.renderer = new RendererClass({
                        width: width,
                        height: height,
                        canvasKit: globalThis.CanvasKit,
                    });
                }
                super(next);
                this.__ocNodeId = nodeId;
            }
            interaction() { return this; }
        };

        build(Chart, container);
        flushRaf();
    };

    // G2's Chart class: the bundle is an IIFE that assigns its exports to
    // `__ocG2Bundle` (see the esbuild entry: Chart + the canvaskit Renderer).
    // Nothing else from the bundle is reachable, which is exactly the surface
    // a chart author needs.
    globalThis.__ocG2Chart = (globalThis.__ocG2Bundle && globalThis.__ocG2Bundle.Chart) || null;
    globalThis.__ocG2Renderer = (globalThis.__ocG2Bundle && globalThis.__ocG2Bundle.Renderer) || null;

})();
