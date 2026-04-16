(function() {
    const canvasCache = {};

    function clamp(value, min, max) {
        return Math.min(max, Math.max(min, Number(value)));
    }

    function toFiniteNumber(value, fallback = 0) {
        const number = Number(value);
        return Number.isFinite(number) ? number : fallback;
    }

    function isArrayLike(value) {
        return Array.isArray(value) || ArrayBuffer.isView(value);
    }

    function cloneColor(color) {
        return [color[0], color[1], color[2], color[3]];
    }

    function colorFromHex(hex) {
        const value = String(hex).trim().replace(/^#/, '');
        if (value.length === 3) {
            const r = parseInt(value[0] + value[0], 16);
            const g = parseInt(value[1] + value[1], 16);
            const b = parseInt(value[2] + value[2], 16);
            return [r / 255, g / 255, b / 255, 1];
        }
        if (value.length === 6) {
            const r = parseInt(value.slice(0, 2), 16);
            const g = parseInt(value.slice(2, 4), 16);
            const b = parseInt(value.slice(4, 6), 16);
            return [r / 255, g / 255, b / 255, 1];
        }
        if (value.length === 8) {
            const r = parseInt(value.slice(0, 2), 16);
            const g = parseInt(value.slice(2, 4), 16);
            const b = parseInt(value.slice(4, 6), 16);
            const a = parseInt(value.slice(6, 8), 16);
            return [r / 255, g / 255, b / 255, a / 255];
        }
        throw new Error(`unsupported color literal: ${hex}`);
    }

    function colorFromRgbFunction(input) {
        const value = String(input).trim();
        const rgba = value.match(/^rgba\((.+)\)$/i);
        const rgb = value.match(/^rgb\((.+)\)$/i);
        const body = rgba ? rgba[1] : rgb ? rgb[1] : null;
        if (!body) {
            return null;
        }
        const parts = body.split(',').map((part) => part.trim());
        if (parts.length !== (rgba ? 4 : 3)) {
            throw new Error(`unsupported color literal: ${input}`);
        }
        const r = clamp(parts[0], 0, 255) / 255;
        const g = clamp(parts[1], 0, 255) / 255;
        const b = clamp(parts[2], 0, 255) / 255;
        const a = rgba ? clamp(parts[3], 0, 1) : 1;
        return [r, g, b, a];
    }

    function parseColorString(input, colorMap) {
        const value = String(input).trim();
        if (colorMap && Object.prototype.hasOwnProperty.call(colorMap, value)) {
            return normalizeColor(colorMap[value]);
        }
        const lower = value.toLowerCase();
        if (lower === 'black') {
            return [0, 0, 0, 1];
        }
        if (lower === 'white') {
            return [1, 1, 1, 1];
        }
        if (lower.startsWith('#')) {
            return colorFromHex(lower);
        }
        const rgb = colorFromRgbFunction(lower);
        if (rgb) {
            return rgb;
        }
        throw new Error(`unsupported color literal: ${input}`);
    }

    function normalizeColor(value) {
        if (typeof value === 'string') {
            return parseColorString(value);
        }
        if (!isArrayLike(value) || value.length < 4) {
            throw new Error('expected an InputColor-compatible value');
        }
        return [
            clamp(value[0], 0, 1),
            clamp(value[1], 0, 1),
            clamp(value[2], 0, 1),
            clamp(value[3], 0, 1)
        ];
    }

    function colorToCss(value) {
        const color = normalizeColor(value);
        return `rgba(${Math.round(color[0] * 255)},${Math.round(color[1] * 255)},${Math.round(color[2] * 255)},${color[3]})`;
    }

    function normalizeRect(rect) {
        if (!isArrayLike(rect) || rect.length < 4) {
            throw new Error('expected an InputRect-compatible value');
        }
        const left = toFiniteNumber(rect[0]);
        const top = toFiniteNumber(rect[1]);
        const right = toFiniteNumber(rect[2]);
        const bottom = toFiniteNumber(rect[3]);
        return {
            left,
            top,
            right,
            bottom,
            x: left,
            y: top,
            width: right - left,
            height: bottom - top
        };
    }

    function normalizeRRect(rrect) {
        if (rrect && rrect.__opencatRRect === true) {
            const rect = normalizeRect(rrect.rect);
            return {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                radius: Math.min(
                    Math.abs(toFiniteNumber(rrect.rx)),
                    Math.abs(toFiniteNumber(rrect.ry))
                )
            };
        }
        if (isArrayLike(rrect) && rrect.length >= 12) {
            const rect = normalizeRect(rrect);
            return {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                radius: Math.min(
                    Math.abs(toFiniteNumber(rrect[4])),
                    Math.abs(toFiniteNumber(rrect[5]))
                )
            };
        }
        throw new Error('expected an InputRRect-compatible value');
    }

    function normalizePaintStyle(value) {
        if (value === CanvasKit.PaintStyle.Fill || value === 'fill') {
            return CanvasKit.PaintStyle.Fill;
        }
        if (value === CanvasKit.PaintStyle.Stroke || value === 'stroke') {
            return CanvasKit.PaintStyle.Stroke;
        }
        throw new Error(`unsupported PaintStyle: ${value}`);
    }

    function normalizeStrokeCap(value) {
        if (value === CanvasKit.StrokeCap.Butt || value === 'butt') {
            return CanvasKit.StrokeCap.Butt;
        }
        if (value === CanvasKit.StrokeCap.Round || value === 'round') {
            return CanvasKit.StrokeCap.Round;
        }
        if (value === CanvasKit.StrokeCap.Square || value === 'square') {
            return CanvasKit.StrokeCap.Square;
        }
        throw new Error(`unsupported StrokeCap: ${value}`);
    }

    function normalizeStrokeJoin(value) {
        if (value === CanvasKit.StrokeJoin.Miter || value === 'miter') {
            return CanvasKit.StrokeJoin.Miter;
        }
        if (value === CanvasKit.StrokeJoin.Round || value === 'round') {
            return CanvasKit.StrokeJoin.Round;
        }
        if (value === CanvasKit.StrokeJoin.Bevel || value === 'bevel') {
            return CanvasKit.StrokeJoin.Bevel;
        }
        throw new Error(`unsupported StrokeJoin: ${value}`);
    }

    function ensurePaint(paint) {
        if (!(paint instanceof Paint)) {
            throw new Error('expected a CanvasKit.Paint instance');
        }
        return paint;
    }

    function ensurePath(path) {
        if (!(path instanceof Path)) {
            throw new Error('expected a CanvasKit.Path instance');
        }
        return path;
    }

    function ensureFont(font) {
        if (!(font instanceof Font)) {
            throw new Error('expected a CanvasKit.Font instance');
        }
        return font;
    }

    function ensureImage(image) {
        if (!image || image.__opencatImage !== true) {
            throw new Error('expected an image from ctx.getImage(assetId)');
        }
        return image;
    }

    function ensurePathEffect(effect) {
        if (!effect || effect.__opencatPathEffect !== true) {
            throw new Error('expected a CanvasKit.PathEffect instance');
        }
        return effect;
    }

    function resolveImagePaint(paint) {
        if (paint == null) {
            return {
                alpha: 1,
                antiAlias: true
            };
        }
        const resolved = ensurePaint(paint);
        return {
            alpha: clamp(resolved._color[3], 0, 1),
            antiAlias: resolved._antiAlias
        };
    }

    class Font {
        constructor(typeface = null, size = 16, scaleX = 1, skewX = 0) {
            if (typeface != null) {
                throw new Error('custom typeface is not supported yet; pass null for system default');
            }
            this._size = Math.max(1, toFiniteNumber(size, 16));
            this._scaleX = toFiniteNumber(scaleX, 1);
            this._skewX = toFiniteNumber(skewX, 0);
            this._subpixel = true;
            this._edging = 'antiAlias';
        }

        copy() {
            const copy = new Font(null, this._size, this._scaleX, this._skewX);
            copy._subpixel = this._subpixel;
            copy._edging = this._edging;
            return copy;
        }

        delete() {}

        getSize() {
            return this._size;
        }

        measureText(str) {
            return __canvas_measure_text(
                String(str),
                this._size,
                this._scaleX,
                this._skewX,
                this._subpixel,
                this._edging
            );
        }

        setEdging(edging) {
            if (edging !== CanvasKit.FontEdging.Alias
                && edging !== CanvasKit.FontEdging.AntiAlias
                && edging !== CanvasKit.FontEdging.SubpixelAntiAlias) {
                throw new Error(`unsupported FontEdging: ${edging}`);
            }
            this._edging = edging;
            return this;
        }

        setScaleX(scaleX) {
            this._scaleX = toFiniteNumber(scaleX, 1);
            return this;
        }

        setSize(size) {
            this._size = Math.max(1, toFiniteNumber(size, 16));
            return this;
        }

        setSkewX(skewX) {
            this._skewX = toFiniteNumber(skewX, 0);
            return this;
        }

        setSubpixel(subpixel) {
            this._subpixel = !!subpixel;
            return this;
        }
    }

    class Paint {
        constructor() {
            this._color = [0, 0, 0, 1];
            this._style = 'fill';
            this._strokeWidth = 1;
            this._strokeCap = 'butt';
            this._strokeJoin = 'miter';
            this._pathEffect = null;
            this._antiAlias = true;
        }

        copy() {
            const copy = new Paint();
            copy._color = cloneColor(this._color);
            copy._style = this._style;
            copy._strokeWidth = this._strokeWidth;
            copy._strokeCap = this._strokeCap;
            copy._strokeJoin = this._strokeJoin;
            copy._pathEffect = this._pathEffect
                ? {
                    __opencatPathEffect: true,
                    kind: this._pathEffect.kind,
                    intervals: this._pathEffect.intervals.slice(),
                    phase: this._pathEffect.phase,
                    delete() {}
                }
                : null;
            copy._antiAlias = this._antiAlias;
            return copy;
        }

        delete() {}

        getColor() {
            return cloneColor(this._color);
        }

        getStrokeCap() {
            return this._strokeCap;
        }

        getStrokeJoin() {
            return this._strokeJoin;
        }

        getStrokeWidth() {
            return this._strokeWidth;
        }

        setAlphaf(alpha) {
            this._color[3] = clamp(alpha, 0, 1);
        }

        setAntiAlias(aa) {
            this._antiAlias = !!aa;
        }

        setColor(color) {
            this._color = normalizeColor(color);
        }

        setColorComponents(r, g, b, a = 1) {
            this._color = [
                clamp(r, 0, 1),
                clamp(g, 0, 1),
                clamp(b, 0, 1),
                clamp(a, 0, 1)
            ];
        }

        setColorInt(color) {
            const value = Number(color) >>> 0;
            this._color = [
                ((value >>> 16) & 0xff) / 255,
                ((value >>> 8) & 0xff) / 255,
                (value & 0xff) / 255,
                ((value >>> 24) & 0xff) / 255
            ];
        }

        setStrokeCap(cap) {
            this._strokeCap = normalizeStrokeCap(cap);
        }

        setStrokeDash(intervals, phase = 0) {
            return this.setPathEffect(CanvasKit.PathEffect.MakeDash(intervals, phase));
        }

        setStrokeJoin(join) {
            this._strokeJoin = normalizeStrokeJoin(join);
        }

        setStrokeWidth(width) {
            this._strokeWidth = Math.max(0, toFiniteNumber(width, 1));
        }

        setStyle(style) {
            this._style = normalizePaintStyle(style);
        }

        setPathEffect(effect) {
            if (effect == null) {
                this._pathEffect = null;
                return this;
            }
            const resolved = ensurePathEffect(effect);
            if (resolved.kind !== 'dash') {
                throw new Error(`unsupported PathEffect kind: ${resolved.kind}`);
            }
            this._pathEffect = {
                __opencatPathEffect: true,
                kind: resolved.kind,
                intervals: resolved.intervals.slice(),
                phase: resolved.phase,
                delete() {}
            };
            return this;
        }
    }

    class Path {
        constructor() {
            this._ops = [];
        }

        copy() {
            const copy = new Path();
            copy._ops = this._ops.map((op) => op.slice());
            return copy;
        }

        delete() {}

        moveTo(x, y) {
            this._ops.push(['moveTo', toFiniteNumber(x), toFiniteNumber(y)]);
            return this;
        }

        lineTo(x, y) {
            this._ops.push(['lineTo', toFiniteNumber(x), toFiniteNumber(y)]);
            return this;
        }

        quadTo(x1, y1, x2, y2) {
            this._ops.push([
                'quadTo',
                toFiniteNumber(x1),
                toFiniteNumber(y1),
                toFiniteNumber(x2),
                toFiniteNumber(y2)
            ]);
            return this;
        }

        cubicTo(x1, y1, x2, y2, x3, y3) {
            this._ops.push([
                'cubicTo',
                toFiniteNumber(x1),
                toFiniteNumber(y1),
                toFiniteNumber(x2),
                toFiniteNumber(y2),
                toFiniteNumber(x3),
                toFiniteNumber(y3)
            ]);
            return this;
        }

        close() {
            this._ops.push(['close']);
            return this;
        }

        addRect(rect) {
            const normalized = normalizeRect(rect);
            this._ops.push([
                'addRect',
                normalized.x,
                normalized.y,
                normalized.width,
                normalized.height
            ]);
            return this;
        }

        addRRect(rrect) {
            const normalized = normalizeRRect(rrect);
            this._ops.push([
                'addRRect',
                normalized.x,
                normalized.y,
                normalized.width,
                normalized.height,
                normalized.radius
            ]);
            return this;
        }

        addOval(oval) {
            const normalized = normalizeRect(oval);
            this._ops.push([
                'addOval',
                normalized.x,
                normalized.y,
                normalized.width,
                normalized.height
            ]);
            return this;
        }

        addArc(oval, startAngle, sweepAngle) {
            const normalized = normalizeRect(oval);
            this._ops.push([
                'addArc',
                normalized.x,
                normalized.y,
                normalized.width,
                normalized.height,
                toFiniteNumber(startAngle),
                toFiniteNumber(sweepAngle)
            ]);
            return this;
        }

        reset() {
            this._ops = [];
            return this;
        }

        rewind() {
            this._ops = [];
            return this;
        }
    }

    const CanvasKit = {
        Color(r, g, b, a = 1) {
            return [
                clamp(r, 0, 255) / 255,
                clamp(g, 0, 255) / 255,
                clamp(b, 0, 255) / 255,
                clamp(a, 0, 1)
            ];
        },
        Color4f(r, g, b, a = 1) {
            return [
                clamp(r, 0, 1),
                clamp(g, 0, 1),
                clamp(b, 0, 1),
                clamp(a, 0, 1)
            ];
        },
        ColorAsInt(r, g, b, a = 1) {
            return (
                ((Math.round(clamp(a, 0, 1) * 255) & 0xff) << 24) |
                ((Math.round(clamp(r, 0, 255)) & 0xff) << 16) |
                ((Math.round(clamp(g, 0, 255)) & 0xff) << 8) |
                (Math.round(clamp(b, 0, 255)) & 0xff)
            ) >>> 0;
        },
        parseColorString,
        multiplyByAlpha(color, alpha) {
            const normalized = normalizeColor(color);
            return [
                normalized[0],
                normalized[1],
                normalized[2],
                clamp(normalized[3] * toFiniteNumber(alpha, 1), 0, 1)
            ];
        },
        LTRBRect(left, top, right, bottom) {
            return [
                toFiniteNumber(left),
                toFiniteNumber(top),
                toFiniteNumber(right),
                toFiniteNumber(bottom)
            ];
        },
        XYWHRect(x, y, width, height) {
            const left = toFiniteNumber(x);
            const top = toFiniteNumber(y);
            return [
                left,
                top,
                left + toFiniteNumber(width),
                top + toFiniteNumber(height)
            ];
        },
        RRectXY(rect, rx, ry) {
            const normalized = normalizeRect(rect);
            return {
                __opencatRRect: true,
                rect: [normalized.left, normalized.top, normalized.right, normalized.bottom],
                rx: toFiniteNumber(rx),
                ry: toFiniteNumber(ry)
            };
        },
        Paint,
        Font,
        Path,
        PathEffect: {
            MakeDash(intervals, phase = 0) {
                if (!Array.isArray(intervals) || intervals.length < 2) {
                    throw new Error('MakeDash expects at least two dash intervals');
                }
                const normalized = intervals.map((value) => {
                    const n = toFiniteNumber(value);
                    if (n <= 0) {
                        throw new Error('MakeDash intervals must be positive');
                    }
                    return n;
                });
                return {
                    __opencatPathEffect: true,
                    kind: 'dash',
                    intervals: normalized,
                    phase: toFiniteNumber(phase, 0),
                    delete() {}
                };
            }
        },
        PaintStyle: {
            Fill: 'fill',
            Stroke: 'stroke'
        },
        StrokeCap: {
            Butt: 'butt',
            Round: 'round',
            Square: 'square'
        },
        StrokeJoin: {
            Miter: 'miter',
            Round: 'round',
            Bevel: 'bevel'
        },
        BlendMode: {
            SrcOver: 'srcOver'
        },
        FontEdging: {
            Alias: 'alias',
            AntiAlias: 'antiAlias',
            SubpixelAntiAlias: 'subpixelAntiAlias'
        },
        ClipOp: {
            Difference: 'difference',
            Intersect: 'intersect'
        },
        PointMode: {
            Points: 'points',
            Lines: 'lines',
            Polygon: 'polygon'
        },
        BLACK: [0, 0, 0, 1],
        WHITE: [1, 1, 1, 1]
    };

    function applyFillPaint(id, paint) {
        __canvas_set_anti_alias(id, paint._antiAlias);
        __canvas_set_fill_style(id, colorToCss(paint._color));
    }

    function applyStrokePaint(id, paint) {
        __canvas_set_anti_alias(id, paint._antiAlias);
        __canvas_set_stroke_style(id, colorToCss(paint._color));
        __canvas_set_line_width(id, paint._strokeWidth);
        __canvas_set_line_cap(id, paint._strokeCap);
        __canvas_set_line_join(id, paint._strokeJoin);
        if (paint._pathEffect && paint._pathEffect.kind === 'dash') {
            __canvas_set_line_dash(id, paint._pathEffect.intervals, paint._pathEffect.phase);
        } else {
            __canvas_clear_line_dash(id);
        }
    }

    function replayPath(id, path) {
        __canvas_begin_path(id);
        for (const op of path._ops) {
            switch (op[0]) {
                case 'moveTo':
                    __canvas_move_to(id, op[1], op[2]);
                    break;
                case 'lineTo':
                    __canvas_line_to(id, op[1], op[2]);
                    break;
                case 'quadTo':
                    __canvas_quad_to(id, op[1], op[2], op[3], op[4]);
                    break;
                case 'cubicTo':
                    __canvas_cubic_to(id, op[1], op[2], op[3], op[4], op[5], op[6]);
                    break;
                case 'close':
                    __canvas_close_path(id);
                    break;
                case 'addRect':
                    __canvas_path_add_rect(id, op[1], op[2], op[3], op[4]);
                    break;
                case 'addRRect':
                    __canvas_path_add_rrect(id, op[1], op[2], op[3], op[4], op[5]);
                    break;
                case 'addOval':
                    __canvas_path_add_oval(id, op[1], op[2], op[3], op[4]);
                    break;
                case 'addArc':
                    __canvas_path_add_arc(id, op[1], op[2], op[3], op[4], op[5], op[6]);
                    break;
                default:
                    throw new Error(`unsupported path verb: ${op[0]}`);
            }
        }
    }

    function makeCanvas(id) {
        return {
            __saveCount: 1,

            clear(color) {
                if (arguments.length === 0 || color == null) {
                    __canvas_clear(id, null);
                } else {
                    __canvas_clear(id, colorToCss(color));
                }
                return this;
            },

            clipRect(rect, op = CanvasKit.ClipOp.Intersect, doAntiAlias = true) {
                if (op !== CanvasKit.ClipOp.Intersect) {
                    throw new Error('only CanvasKit.ClipOp.Intersect is supported');
                }
                const normalized = normalizeRect(rect);
                __canvas_clip_rect(
                    id,
                    normalized.x,
                    normalized.y,
                    normalized.width,
                    normalized.height,
                    !!doAntiAlias
                );
                return this;
            },

            drawCircle(cx, cy, radius, paint) {
                const resolvedPaint = ensurePaint(paint);
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    applyStrokePaint(id, resolvedPaint);
                    __canvas_stroke_circle(
                        id,
                        toFiniteNumber(cx),
                        toFiniteNumber(cy),
                        Math.max(0, toFiniteNumber(radius))
                    );
                } else {
                    applyFillPaint(id, resolvedPaint);
                    __canvas_fill_circle(
                        id,
                        toFiniteNumber(cx),
                        toFiniteNumber(cy),
                        Math.max(0, toFiniteNumber(radius))
                    );
                }
                return this;
            },

            drawColor(color, blendMode = CanvasKit.BlendMode.SrcOver) {
                if (blendMode !== CanvasKit.BlendMode.SrcOver) {
                    throw new Error('only CanvasKit.BlendMode.SrcOver is supported');
                }
                __canvas_draw_paint(id, colorToCss(color), true);
                return this;
            },

            drawColorComponents(r, g, b, a = 1, blendMode = CanvasKit.BlendMode.SrcOver) {
                return this.drawColor(CanvasKit.Color4f(r, g, b, a), blendMode);
            },

            drawColorInt(color, blendMode = CanvasKit.BlendMode.SrcOver) {
                const value = Number(color) >>> 0;
                return this.drawColor([
                    ((value >>> 16) & 0xff) / 255,
                    ((value >>> 8) & 0xff) / 255,
                    (value & 0xff) / 255,
                    ((value >>> 24) & 0xff) / 255
                ], blendMode);
            },

            drawPaint(paint) {
                const resolvedPaint = ensurePaint(paint);
                __canvas_draw_paint(id, colorToCss(resolvedPaint._color), resolvedPaint._antiAlias);
                return this;
            },

            drawImageRect(image, src, dest, paint = null, fastSample = false) {
                const resolvedImage = ensureImage(image);
                const source = normalizeRect(src);
                const normalized = normalizeRect(dest);
                const imagePaint = resolveImagePaint(paint);
                __canvas_draw_image(
                    id,
                    resolvedImage.assetId,
                    [
                        normalized.x,
                        normalized.y,
                        normalized.width,
                        normalized.height,
                        source.x,
                        source.y,
                        source.width,
                        source.height
                    ],
                    'fill',
                    imagePaint.alpha,
                    imagePaint.antiAlias,
                    !!fastSample
                );
                return this;
            },

            drawLine(x0, y0, x1, y1, paint) {
                const resolvedPaint = ensurePaint(paint);
                applyStrokePaint(id, resolvedPaint);
                __canvas_draw_line(
                    id,
                    toFiniteNumber(x0),
                    toFiniteNumber(y0),
                    toFiniteNumber(x1),
                    toFiniteNumber(y1)
                );
                return this;
            },

            drawPath(path, paint) {
                const resolvedPath = ensurePath(path);
                const resolvedPaint = ensurePaint(paint);
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    applyStrokePaint(id, resolvedPaint);
                    replayPath(id, resolvedPath);
                    __canvas_stroke_path(id);
                } else {
                    applyFillPaint(id, resolvedPaint);
                    replayPath(id, resolvedPath);
                    __canvas_fill_path(id);
                }
                return this;
            },

            drawRect(rect, paint) {
                const normalized = normalizeRect(rect);
                const resolvedPaint = ensurePaint(paint);
                __canvas_set_anti_alias(id, resolvedPaint._antiAlias);
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    __canvas_stroke_rect(
                        id,
                        normalized.x,
                        normalized.y,
                        normalized.width,
                        normalized.height,
                        colorToCss(resolvedPaint._color),
                        resolvedPaint._strokeWidth
                    );
                } else {
                    __canvas_fill_rect(
                        id,
                        normalized.x,
                        normalized.y,
                        normalized.width,
                        normalized.height,
                        colorToCss(resolvedPaint._color)
                    );
                }
                return this;
            },

            drawText(str, x, y, paint, font) {
                const resolvedPaint = ensurePaint(paint);
                const resolvedFont = ensureFont(font);
                __canvas_draw_text(
                    id,
                    String(str),
                    [
                        toFiniteNumber(x),
                        toFiniteNumber(y),
                        resolvedFont._size,
                        resolvedFont._scaleX,
                        resolvedFont._skewX,
                        resolvedPaint._strokeWidth
                    ],
                    colorToCss(resolvedPaint._color),
                    [
                        resolvedPaint._antiAlias,
                        resolvedPaint._style === CanvasKit.PaintStyle.Stroke,
                        resolvedFont._subpixel
                    ],
                    resolvedFont._edging
                );
                return this;
            },

            drawRRect(rrect, paint) {
                const normalized = normalizeRRect(rrect);
                const resolvedPaint = ensurePaint(paint);
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    applyStrokePaint(id, resolvedPaint);
                    __canvas_stroke_rrect(
                        id,
                        normalized.x,
                        normalized.y,
                        normalized.width,
                        normalized.height,
                        normalized.radius
                    );
                } else {
                    applyFillPaint(id, resolvedPaint);
                    __canvas_fill_rrect(
                        id,
                        normalized.x,
                        normalized.y,
                        normalized.width,
                        normalized.height,
                        normalized.radius
                    );
                }
                return this;
            },

            restore() {
                this.__saveCount = Math.max(1, this.__saveCount - 1);
                __canvas_restore(id);
                return this;
            },

            restoreToCount(saveCount) {
                const target = Math.max(1, Math.min(
                    this.__saveCount,
                    Math.floor(toFiniteNumber(saveCount, this.__saveCount))
                ));
                __canvas_restore_to_count(id, target);
                this.__saveCount = target;
                return this;
            },

            rotate(degrees, rx, ry) {
                if (arguments.length >= 3) {
                    __canvas_translate(id, toFiniteNumber(rx), toFiniteNumber(ry));
                    __canvas_rotate(id, toFiniteNumber(degrees));
                    __canvas_translate(id, -toFiniteNumber(rx), -toFiniteNumber(ry));
                } else {
                    __canvas_rotate(id, toFiniteNumber(degrees));
                }
                return this;
            },

            save() {
                __canvas_save(id);
                this.__saveCount += 1;
                return this.__saveCount;
            },

            saveLayer(paint = null, bounds = null) {
                let layerPaint = paint;
                let layerBounds = bounds;
                if (layerBounds == null && isArrayLike(layerPaint)) {
                    layerBounds = layerPaint;
                    layerPaint = null;
                }
                const resolvedPaint = layerPaint == null ? null : ensurePaint(layerPaint);
                const normalizedBounds = layerBounds == null
                    ? null
                    : (() => {
                        const rect = normalizeRect(layerBounds);
                        return [rect.x, rect.y, rect.width, rect.height];
                    })();
                __canvas_save_layer(
                    id,
                    resolvedPaint ? clamp(resolvedPaint._color[3], 0, 1) : 1,
                    normalizedBounds
                );
                this.__saveCount += 1;
                return this.__saveCount;
            },

            setAlphaf(alpha) {
                __canvas_set_global_alpha(id, toFiniteNumber(alpha, 1));
                return this;
            },

            scale(sx, sy) {
                const scaleX = toFiniteNumber(sx, 1);
                const scaleY = arguments.length >= 2 ? toFiniteNumber(sy, 1) : scaleX;
                __canvas_scale(id, scaleX, scaleY);
                return this;
            },

            translate(dx, dy) {
                __canvas_translate(id, toFiniteNumber(dx), toFiniteNumber(dy));
                return this;
            },

            drawArc(oval, startAngle, sweepAngle, useCenter, paint) {
                const resolvedPaint = ensurePaint(paint);
                const normalized = normalizeRect(oval);
                const cx = (normalized.left + normalized.right) / 2;
                const cy = (normalized.top + normalized.bottom) / 2;
                const rx = Math.max(0, normalized.width / 2);
                const ry = Math.max(0, normalized.height / 2);
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    applyStrokePaint(id, resolvedPaint);
                    __canvas_stroke_arc(id, cx, cy, rx, ry, toFiniteNumber(startAngle), toFiniteNumber(sweepAngle));
                } else {
                    applyFillPaint(id, resolvedPaint);
                    const fn = useCenter ? __canvas_draw_arc_to_center : __canvas_draw_arc;
                    fn(id, cx, cy, rx, ry, toFiniteNumber(startAngle), toFiniteNumber(sweepAngle));
                }
                return this;
            },

            drawOval(oval, paint) {
                const resolvedPaint = ensurePaint(paint);
                const normalized = normalizeRect(oval);
                const cx = (normalized.left + normalized.right) / 2;
                const cy = (normalized.top + normalized.bottom) / 2;
                const rx = Math.max(0, normalized.width / 2);
                const ry = Math.max(0, normalized.height / 2);
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    applyStrokePaint(id, resolvedPaint);
                    __canvas_stroke_oval(id, cx, cy, rx, ry);
                } else {
                    applyFillPaint(id, resolvedPaint);
                    __canvas_fill_oval(id, cx, cy, rx, ry);
                }
                return this;
            },

            clipPath(path, op = CanvasKit.ClipOp.Intersect, doAntiAlias = true) {
                if (op !== CanvasKit.ClipOp.Intersect) {
                    throw new Error('only CanvasKit.ClipOp.Intersect is supported');
                }
                const resolvedPath = ensurePath(path);
                replayPath(id, resolvedPath);
                __canvas_clip_path(id, !!doAntiAlias);
                return this;
            },

            clipRRect(rrect, op = CanvasKit.ClipOp.Intersect, doAntiAlias = true) {
                if (op !== CanvasKit.ClipOp.Intersect) {
                    throw new Error('only CanvasKit.ClipOp.Intersect is supported');
                }
                const normalized = normalizeRRect(rrect);
                __canvas_clip_rrect(
                    id,
                    normalized.x,
                    normalized.y,
                    normalized.width,
                    normalized.height,
                    normalized.radius,
                    !!doAntiAlias
                );
                return this;
            },

            drawPoints(mode, points, paint) {
                if (!isArrayLike(points)) {
                    throw new Error('drawPoints expects an array of coordinates');
                }
                const resolvedPaint = ensurePaint(paint);
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    applyStrokePaint(id, resolvedPaint);
                } else {
                    applyFillPaint(id, resolvedPaint);
                }
                const flat = [];
                for (let i = 0; i < points.length; i++) {
                    flat.push(toFiniteNumber(points[i]));
                }
                const modeStr = mode === CanvasKit.PointMode.Points ? 'points'
                    : mode === CanvasKit.PointMode.Lines ? 'lines'
                    : mode === CanvasKit.PointMode.Polygon ? 'polygon'
                    : mode;
                __canvas_draw_points(id, modeStr, flat);
                return this;
            },

            drawDRRect(outer, inner, paint) {
                const outerNorm = normalizeRRect(outer);
                const innerNorm = normalizeRRect(inner);
                const resolvedPaint = ensurePaint(paint);
                const coords = [
                    outerNorm.x, outerNorm.y, outerNorm.width, outerNorm.height, outerNorm.radius,
                    innerNorm.x, innerNorm.y, innerNorm.width, innerNorm.height, innerNorm.radius
                ];
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    applyStrokePaint(id, resolvedPaint);
                    __canvas_stroke_drrect(id, coords);
                } else {
                    applyFillPaint(id, resolvedPaint);
                    __canvas_fill_drrect(id, coords);
                }
                return this;
            },

            skew(sx, sy) {
                __canvas_skew(id, toFiniteNumber(sx), toFiniteNumber(sy));
                return this;
            },

            drawImage(image, x, y, paint) {
                const resolvedImage = ensureImage(image);
                const imagePaint = resolveImagePaint(paint);
                __canvas_draw_image_simple(
                    id,
                    resolvedImage.assetId,
                    toFiniteNumber(x),
                    toFiniteNumber(y),
                    imagePaint.alpha,
                    imagePaint.antiAlias
                );
                return this;
            },

            concat(matrix) {
                if (!isArrayLike(matrix) || matrix.length < 9) {
                    throw new Error('concat expects a 9-element matrix array');
                }
                const values = [];
                for (let i = 0; i < 9; i++) {
                    values.push(toFiniteNumber(matrix[i]));
                }
                __canvas_concat(id, values);
                return this;
            }
        };
    }

    globalThis.CanvasKit = CanvasKit;
    ctx.CanvasKit = CanvasKit;
    ctx.getImage = function(assetId) {
        return {
            __opencatImage: true,
            assetId: String(assetId),
            delete() {}
        };
    };
    ctx.getCanvas = function() {
        const id = ctx.__currentCanvasTarget;
        if (!id) {
            return null;
        }
        if (!canvasCache[id]) {
            canvasCache[id] = makeCanvas(id);
        }
        canvasCache[id].__saveCount = 1;
        return canvasCache[id];
    };
})();
