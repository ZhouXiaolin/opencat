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
            this._shader = null;
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
            copy._shader = this._shader;
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

        setShader(shader) {
            if (shader == null) {
                this._shader = null;
                return this;
            }
            if (shader.__opencatShader !== 'runtime'
                && shader.__opencatShader !== 'image'
                && shader.__opencatShader !== 'lambda'
                && shader.__opencatShader !== 'generated') {
                throw new Error('setShader expects a shader handle');
            }
            this._shader = shader;
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

    class RuntimeEffect {
        constructor(sksl) {
            this.__opencatRuntimeEffect = true;
            this._sksl = String(sksl);
        }
        delete() {}

        makeShader(uniforms) {
            return makeRuntimeShader(this, uniforms, []);
        }

        makeShaderWithChildren(uniforms, children) {
            return makeRuntimeShader(this, uniforms, children);
        }
    }

    function makeRuntimeShader(effect, uniforms, children) {
        return {
            __opencatShader: 'runtime',
            effect,
            uniforms: Array.from(uniforms || [], toFiniteNumber),
            children: (children || []).map(ensureChildShader),
        };
    }

    /* ── Effect lambda（编译型效果 DSL）────────────────────────────────
       fn 只被读取源码（fn.toString()）、从不被执行：Rust 侧解析该源码并
       重建计算，自动派发 SKSL（RuntimeEffect）或纯 CPU 逐像素后端。
       约束：lambda 必须是源码内联的箭头函数（不可是原生函数/被压缩）；
       白名单语法 —— 数值/布尔/数组字面量、算术/比较/逻辑/三目、if/else、
       let/const、std 内建与 Math.*、u.<name>、child.eval(pos)、swizzle 读。
       spec: { uniforms: [['t','float'], ...], backend: 'auto'|'sksl'|'cpu' }。
       uniforms 展平顺序必须与 spec 声明序一致（向量按分量展开）。 */
    class LambdaEffect {
        constructor(fn, spec) {
            this._fnSource = assertLambdaFn(fn, 'Effect.fromLambda');
            this.__opencatLambdaEffect = true;
            const s = spec || {};
            this._spec = {
                uniforms: Array.isArray(s.uniforms) ? s.uniforms : [],
                backend: s.backend == null ? 'auto' : String(s.backend),
            };
        }
        delete() {}

        makeShader(uniforms) {
            return makeLambdaShader(this, uniforms, []);
        }

        makeShaderWithChildren(uniforms, children) {
            return makeLambdaShader(this, uniforms, children);
        }
    }

    /* lambda 源码可用性检查（编译型契约：源码被 Rust 取走编译，从不执行）。
       `this._fnSource = ...` 之前必须先过这里。 */
    function assertLambdaFn(fn, who) {
        if (typeof fn !== 'function') {
            throw new Error(who + ': fn must be a function');
        }
        const src = fn.toString();
        if (src.length === 0 || src.includes('[native code]')) {
            throw new Error(who + ': lambda source unavailable (native or minified fn)');
        }
        return src;
    }

    /* spec 声明序展开 uniforms：float→1 个数，float2/3/4→2/3/4 个数。
       输入可以是平铺数组（元素为数或数）或 {name: value} 映射。 */
    function flattenLambdaUniforms(effect, uniforms, who) {
        const label = who || 'Effect.fromLambda';
        const specUniforms = effect._spec.uniforms;
        const out = [];
        const pushValue = (ty, v, name) => {
            const n = ty === 'float' ? 1 : Number(String(ty).slice(5)) || 0;
            if (!(n >= 1 && n <= 4)) {
                throw new Error(`${label}: unsupported uniform type '${ty}'`);
            }
            if (n === 1) {
                if (Array.isArray(v)) {
                    throw new Error(`uniform '${name}' expects a number`);
                }
                out.push(toFiniteNumber(v));
                return;
            }
            if (!isArrayLike(v) || v.length !== n) {
                throw new Error(`uniform '${name}' expects ${n} numbers`);
            }
            for (let i = 0; i < n; i++) out.push(toFiniteNumber(v[i]));
        };
        if (uniforms == null) {
            for (const [name, ty] of specUniforms) pushValue(ty, 0, name);
            return out;
        }
        if (isArrayLike(uniforms)) {
            let flat = [];
            for (const v of uniforms) {
                if (isArrayLike(v)) flat.push(...v);
                else flat.push(v);
            }
            let k = 0;
            for (const [name, ty] of specUniforms) {
                const n = ty === 'float' ? 1 : Number(String(ty).slice(5)) || 0;
                if (k + n > flat.length) {
                    throw new Error(
                        `${label}: not enough uniform values for '${name}'`
                    );
                }
                pushValue(ty, n === 1 ? flat[k] : flat.slice(k, k + n), name);
                k += n;
            }
            return out;
        }
        if (typeof uniforms === 'object') {
            for (const [name, ty] of specUniforms) {
                if (!(name in uniforms)) {
                    throw new Error(`${label}: missing uniform '${name}'`);
                }
                pushValue(ty, uniforms[name], name);
            }
            return out;
        }
        throw new Error(`${label}: uniforms must be an array or object`);
    }

    function makeLambdaShader(effect, uniforms, children) {
        return {
            __opencatShader: 'lambda',
            effect,
            uniforms: flattenLambdaUniforms(effect, uniforms),
            children: (children || []).map(ensureChildShader),
        };
    }

    /* 帧级生成图像 child（surface.bake 的产物）：key 即 bake 的 key。 */
    function makeGeneratedShader(key) {
        return { __opencatShader: 'generated', key: String(key) };
    }

    function ensureChildShader(c) {
        if (!c) throw new Error('child shader is null');
        if (c.__opencatShader === 'image') return c;
        if (c.__opencatShader === 'picture') return c;
        if (c.__opencatShader === 'generated') {
            if (typeof c.key !== 'string' || c.key.length === 0) {
                throw new Error('generated child shader requires a frame-scoped key');
            }
            return c;
        }
        if (c.__opencatShader === 'surface') {
            if (typeof c.id !== 'string' || c.id.length === 0) {
                throw new Error('surface child shader requires a surface id');
            }
            return c;
        }
        throw new Error(
            'only image, picture, generated and surface child shaders are supported; gradient not implemented'
        );
    }

    function normalizeTileMode(value) {
        if (value == null) return 'clamp';
        const v = String(value).toLowerCase();
        if (v === 'clamp' || v === 'repeat' || v === 'mirror' || v === 'decal') return v;
        throw new Error(`unsupported TileMode: ${value}`);
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
                    return n > 0 ? n : 1e-6;
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
        TileMode: {
            Clamp: 'clamp',
            Repeat: 'repeat',
            Mirror: 'mirror',
            Decal: 'decal'
        },
        RuntimeEffect: {
            Make(sksl) {
                if (typeof sksl !== 'string' || sksl.length === 0) return null;
                return new RuntimeEffect(sksl);
            }
        },
        /* 效果 lambda：统一 SKSL 与逐像素效果的编译型 DSL。
           后端由 Rust 依 lambda 用到的内建自动派发（spec.backend 可强制）。 */
        Effect: {
            fromLambda(fn, spec) {
                return new LambdaEffect(fn, spec);
            }
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

    function makeSubTreeHandle(ownerId) {
        const handle = {
            __opencatSubTreePicture: true,
            ownerId: String(ownerId)
        };
        // Picture-as-shader child for RuntimeEffect.makeShaderWithChildren.
        // Tile modes are accepted for CanvasKit API parity but the engine
        // currently samples picture pictures with TileMode::Clamp.
        handle.makeShader = function(tileX, tileY) {
            return {
                __opencatShader: 'picture',
                ownerId: handle.ownerId,
                tileX: normalizeTileMode(tileX),
                tileY: normalizeTileMode(tileY),
            };
        };
        return handle;
    }

    function ensureSubTreeHandle(value) {
        if (!value || value.__opencatSubTreePicture !== true) {
            throw new Error("drawPicture expects a handle returned by getSubTree()");
        }
        return value;
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
                if (resolvedPaint._shader
                    && resolvedPaint._shader.__opencatShader === 'runtime') {
                    const sh = resolvedPaint._shader;
                    __canvas_runtime_effect_draw(
                        id,
                        sh.effect._sksl,
                        sh.uniforms,
                        JSON.stringify(sh.children),
                        normalized.x,
                        normalized.y,
                        normalized.width,
                        normalized.height,
                    );
                    return this;
                }
                if (resolvedPaint._shader
                    && resolvedPaint._shader.__opencatShader === 'lambda') {
                    const sh = resolvedPaint._shader;
                    __canvas_lambda_effect_draw(
                        id,
                        sh.effect._fnSource,
                        JSON.stringify(sh.effect._spec),
                        sh.uniforms,
                        JSON.stringify(sh.children),
                        normalized.x,
                        normalized.y,
                        normalized.width,
                        normalized.height,
                    );
                    return this;
                }
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
            },

            getSubTree() {
                if (!ctx.__targetRegistry || !ctx.__targetRegistry.canvas || !ctx.__targetRegistry.canvas[String(id)]) {
                    throw new Error("getSubTree is only available for canvas nodes: " + id);
                }
                return makeSubTreeHandle(id);
            },

            drawPicture(handle, x = 0, y = 0) {
                const resolved = ensureSubTreeHandle(handle);
                __canvas_draw_picture(id, resolved.ownerId, toFiniteNumber(x), toFiniteNumber(y));
                return this;
            },

            /* ── 2D-canvas pixel shims (k3 dissolve port) ─────────────────
               createImageData: zero-init pixel buffer (Uint8ClampedArray).
               putImageData: uploads the buffer as a frame-scoped generated
               image drawn at (dx, dy) in canvas-node coordinates. The engine
               registers `__opencatPutImageData` (binary fast path, bypasses
               the JSON dispatcher); `key` MUST be frame-unique because the
               generated-image table treats a repeated key as an idempotent
               no-op (identical pixels) or hard error (differing pixels). */
            createImageData(w, h) {
                const ww = Math.max(1, Math.round(toFiniteNumber(w)));
                const hh = Math.max(1, Math.round(toFiniteNumber(h)));
                return { width: ww, height: hh, data: new Uint8ClampedArray(ww * hh * 4) };
            },

            putImageData(img, dx, dy, key) {
                if (typeof __opencatPutImageData !== 'function') {
                    throw new Error('putImageData: engine native __opencatPutImageData unavailable');
                }
                if (!img || !isArrayLike(img.data)) {
                    throw new Error('putImageData: expected an ImageData-like value');
                }
                if (key == null) {
                    throw new Error('putImageData: a frame-scoped key is required');
                }
                const src = img.data;
                const u8 = src instanceof Uint8Array
                    ? src
                    : new Uint8Array(src.buffer, src.byteOffset, src.byteLength);
                __opencatPutImageData(
                    id,
                    String(key),
                    u8,
                    toFiniteNumber(dx),
                    toFiniteNumber(dy),
                    img.width,
                    img.height
                );
                return this;
            },
        };
    }

    globalThis.CanvasKit = CanvasKit;
    ctx.CanvasKit = CanvasKit;
    ctx.getImage = function(assetId) {
        const handle = {
            __opencatImage: true,
            assetId: String(assetId),
            delete() {}
        };
        handle.makeShader = function(tileX, tileY) {
            return {
                __opencatShader: 'image',
                assetId: handle.assetId,
                tileX: normalizeTileMode(tileX),
                tileY: normalizeTileMode(tileY),
            };
        };
        return handle;
    };
    function assertCanvasTarget(id, apiName) {
        var key = String(id);
        var registry = ctx.__targetRegistry || {};
        if (registry.visual && registry.visual[key]) return key;
        if (registry.nonVisual && registry.nonVisual[key]) {
            throw new Error(apiName + ": non-visual id '" + key + "' cannot be targeted");
        }
        throw new Error(apiName + ": unknown id '" + key + "'");
    }

    ctx.getCanvas = function() {
        throw new Error("ctx.getCanvas is not available; use ctx.getCanvasById(id)");
    };

    ctx.getCanvasById = function(id) {
        id = assertCanvasTarget(id, 'ctx.getCanvasById');
        if (!canvasCache[id]) {
            canvasCache[id] = makeCanvas(id);
        }
        canvasCache[id].__saveCount = 1;
        return canvasCache[id];
    };

    /* ══ Offscreen 2D surface facade (k3 dissolve port) ══════════════════
       Backed by `opencat_core::text::surface` — real-font glyph
       rasterization (swash over the scoped fontdb), kern-free hmtx advances
       with trailing letter-spacing, and canvas-2D ink-box metrics. Pixel
       readback goes through the engine's binary native
       `__opencatSurfaceRead` (ArrayBuffer; no JSON marshaling).

       Supported 2D subset (exactly what the reference drawDissolve uses):
       save / restore / translate / scale(sx,1) / clearRect / fillStyle /
       font / letterSpacing / textBaseline='alphabetic' / measureText /
       fillText / getImageData. Rotated or y-scaled text is rejected. */
    const surfaceCache = {};

    function parseCssFont(font) {
        const m = /^\s*(\d+)\s+([0-9.]+)px\s+(.+?)\s*$/.exec(String(font));
        if (!m) {
            throw new Error('unsupported font shorthand: ' + font);
        }
        // CSS family list: resolve the FIRST family only (no font fallback
        // in the core surface; unknown families error at the binding).
        const family = m[3].split(',')[0].replace(/["']/g, '').trim();
        return { weight: parseInt(m[1], 10), size: parseFloat(m[2]), family };
    }

    function makeSurface(id) {
        const state = {
            font: '400 16px sans-serif',
            letterSpacing: '0px',
            fillStyle: '#000',
            textBaseline: 'alphabetic'
        };
        // translate + uniform-ish scaleX subset: fillText device position is
        // (tx + a*x, ty + y); the x-scale also squeezes glyphs (scale_x).
        const matrix = { tx: 0, ty: 0, a: 1 };
        const stack = [];
        function fontSpec() {
            const f = parseCssFont(state.font);
            const ls = parseFloat(String(state.letterSpacing));
            return {
                weight: f.weight,
                size: f.size,
                family: f.family,
                letterSpacing: Number.isFinite(ls) ? ls : 0
            };
        }
        return {
            __opencatSurface: true,
            save() {
                stack.push({ tx: matrix.tx, ty: matrix.ty, a: matrix.a });
            },
            restore() {
                const s = stack.pop();
                if (s) { matrix.tx = s.tx; matrix.ty = s.ty; matrix.a = s.a; }
            },
            translate(x, y) {
                matrix.tx += matrix.a * toFiniteNumber(x);
                matrix.ty += toFiniteNumber(y);
            },
            scale(sx, sy) {
                if (toFiniteNumber(sy, 1) !== 1) {
                    throw new Error('surface scale: only scale(sx, 1) is supported');
                }
                matrix.a *= toFiniteNumber(sx, 1);
            },
            clearRect(x, y, w, h) {
                __surface_clear(id, toFiniteNumber(x), toFiniteNumber(y), toFiniteNumber(w), toFiniteNumber(h));
            },
            measureText(text) {
                const f = fontSpec();
                const m = __surface_measure_text(String(text), f.family, f.weight, f.size, f.letterSpacing);
                return {
                    width: m.width,
                    actualBoundingBoxLeft: m.ink_left,
                    actualBoundingBoxRight: m.ink_right,
                    actualBoundingBoxAscent: m.ink_ascent,
                    actualBoundingBoxDescent: m.ink_descent
                };
            },
            fillText(text, x, y) {
                if (state.textBaseline !== 'alphabetic') {
                    throw new Error('surface fillText: only the alphabetic baseline is supported');
                }
                const f = fontSpec();
                __surface_fill_text(
                    id,
                    String(text),
                    matrix.tx + matrix.a * toFiniteNumber(x),
                    matrix.ty + toFiniteNumber(y),
                    f.family,
                    f.weight,
                    f.size,
                    f.letterSpacing,
                    matrix.a,
                    colorToCss(state.fillStyle)
                );
            },
            getImageData(x, y, w, h) {
                if (typeof __opencatSurfaceReadInto !== 'function') {
                    throw new Error('getImageData: engine native __opencatSurfaceReadInto unavailable');
                }
                const ww = Math.round(toFiniteNumber(w));
                const hh = Math.round(toFiniteNumber(h));
                // Fill-in-place: the engine native writes bytes into this
                // preallocated buffer (a returning-native form leaks QuickJS
                // GC objects at runtime teardown).
                const u8 = new Uint8Array(Math.max(0, ww * hh * 4));
                __opencatSurfaceReadInto(id, toFiniteNumber(x), toFiniteNumber(y), ww, hh, u8);
                return {
                    width: ww,
                    height: hh,
                    data: new Uint8ClampedArray(u8.buffer)
                };
            },
            /* ── Render target（§render target）────────────────────────
               JS 拥有算法：lambda 源码只被 Rust 编译、从不执行，像素缓冲
               永不跨进 JS；引擎侧只有通用执行器（逐像素重绘 / 顺序扫描 /
               帧级烘焙），不含任何具体效果算法。
               - runEffect(fn, spec, uniforms, children)：pixel 类 lambda
                 逐像素重绘本目标（f64 解释器，rayon 按行并行）。children
                 可采样 generated（本帧烘焙）或其它 surface（session 级
                 render target），如 {__opencatShader:'surface', id:'x'}。
               - scanPass(fn, direction, spec, uniforms)：scan 类 lambda
                 （首参 get）就地顺序扫描一遍；get(dx,dy) 读 in-progress
                 字节（clamp-to-edge）。遍历顺序由 executor 拥有
                 （'forward'/'backward'），单线程。
               - bake(key)：当前像素注册为帧级生成图像（幂等契约：同 key
                 异像素 = 硬错误），返回 generated shader 供 SKSL/绘制
                 路径采样。CPU lambda 可直接用 surface child 免烘焙。 */
            runEffect(fn, spec, uniforms, children) {
                const src = assertLambdaFn(fn, 'surface.runEffect');
                const s = spec || {};
                if (String(s.backend) === 'sksl') {
                    throw new Error(
                        'surface.runEffect: target ops always run on the CPU interpreter (sksl is a draw-time backend)'
                    );
                }
                if (s.kind === 'scan') {
                    throw new Error(
                        'surface.runEffect is for pixel lambdas; ordered scans belong in surface.scanPass'
                    );
                }
                const specJson = JSON.stringify(
                    Object.assign({}, s, { kind: s.kind || 'pixel' })
                );
                const flat = flattenLambdaUniforms(
                    { _spec: { uniforms: s.uniforms || [] } },
                    uniforms,
                    'surface.runEffect'
                );
                const kids = (children || []).map(ensureChildShader);
                if (__surface_apply_effect(id, src, specJson, flat, JSON.stringify(kids)) !== true) {
                    throw new Error('surface.runEffect: engine rejected the lambda');
                }
            },
            scanPass(fn, direction, spec, uniforms) {
                const src = assertLambdaFn(fn, 'surface.scanPass');
                const s = spec || {};
                if (String(s.backend) === 'sksl') {
                    throw new Error(
                        'surface.scanPass: scan lambdas have no SKSL form (ordered scan is CPU-interpreter only)'
                    );
                }
                if (s.kind != null && s.kind !== 'scan') {
                    throw new Error(
                        'surface.scanPass requires spec.kind "scan" (per-pixel rewrites belong in surface.runEffect)'
                    );
                }
                const specJson = JSON.stringify(Object.assign({}, s, { kind: 'scan' }));
                const flat = flattenLambdaUniforms(
                    { _spec: { uniforms: s.uniforms || [] } },
                    uniforms,
                    'surface.scanPass'
                );
                if (__surface_scan_pass(id, src, specJson, flat, String(direction || 'forward')) !== true) {
                    throw new Error('surface.scanPass: engine rejected the lambda');
                }
            },
            bake(key) {
                if (__surface_bake(id, String(key)) !== true) {
                    throw new Error('surface.bake: engine rejected surface ' + id);
                }
                return makeGeneratedShader(key);
            },
            get font() { return state.font; },
            set font(v) { state.font = String(v); },
            get letterSpacing() { return state.letterSpacing; },
            set letterSpacing(v) { state.letterSpacing = String(v); },
            get fillStyle() { return state.fillStyle; },
            set fillStyle(v) { state.fillStyle = v; },
            get textBaseline() { return state.textBaseline; },
            set textBaseline(v) { state.textBaseline = String(v); }
        };
    }

    ctx.createSurface = function(id, w, h) {
        id = String(id);
        if (!surfaceCache[id]) {
            __surface_create(id, toFiniteNumber(w), toFiniteNumber(h));
            surfaceCache[id] = makeSurface(id);
        }
        return surfaceCache[id];
    };
})();
