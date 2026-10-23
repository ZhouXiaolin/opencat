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
            this._edging = CanvasKit.FontEdging.AntiAlias;
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
                fontEdgingName(this._edging)
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
            this._style = CanvasKit.PaintStyle.Fill;
            this._strokeWidth = 1;
            this._strokeCap = CanvasKit.StrokeCap.Butt;
            this._strokeJoin = CanvasKit.StrokeJoin.Miter;
            this._pathEffect = null;
            this._antiAlias = true;
            this._shader = null;
            // False until any setter runs. g-canvaskit builds stroke paints
            // before checking `lineWidth > 0`, so a skipped block leaves a
            // never-configured paint that then draws as an opaque default-black
            // fill (the G2 bar-rendered-black bug). Callers that care — the G2
            // surface proxy — check this flag and drop the draw.
            this._configured = false;
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
            copy._configured = this._configured;
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
            this._configured = true;
            this._color[3] = clamp(alpha, 0, 1);
        }

        setAntiAlias(aa) {
            this._configured = true;
            this._antiAlias = !!aa;
        }

        setColor(color) {
            this._configured = true;
            this._color = normalizeColor(color);
        }

        setColorComponents(r, g, b, a = 1) {
            this._configured = true;
            this._color = [
                clamp(r, 0, 1),
                clamp(g, 0, 1),
                clamp(b, 0, 1),
                clamp(a, 0, 1)
            ];
        }

        setColorInt(color) {
            this._configured = true;
            const value = Number(color) >>> 0;
            this._color = [
                ((value >>> 16) & 0xff) / 255,
                ((value >>> 8) & 0xff) / 255,
                (value & 0xff) / 255,
                ((value >>> 24) & 0xff) / 255
            ];
        }

        setStrokeCap(cap) {
            this._configured = true;
            this._strokeCap = normalizeStrokeCap(cap);
        }

        setStrokeDash(intervals, phase = 0) {
            return this.setPathEffect(CanvasKit.PathEffect.MakeDash(intervals, phase));
        }

        setStrokeJoin(join) {
            this._configured = true;
            this._strokeJoin = normalizeStrokeJoin(join);
        }

        // Skia joins the stroke with a miter up to `miterLimit * halfWidth`,
        // then falls back to bevel. The recorder models stroke joins but not a
        // miter limit, so the value is admitted (G2 sets it on every stroke
        // paint) and the drawn join stays what the resolve pass picks.
        setStrokeMiter(miter) {
            this._configured = true;
            this._strokeMiter = toFiniteNumber(miter, 4);
        }

        setStrokeWidth(width) {
            this._configured = true;
            this._strokeWidth = Math.max(0, toFiniteNumber(width, 1));
        }

        setStyle(style) {
            this._configured = true;
            this._style = normalizePaintStyle(style);
        }

        setPathEffect(effect) {
            this._configured = true;
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
            this._configured = true;
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

        setMaskFilter(filter) {
            throw new Error('Paint.setMaskFilter is not available — blur effects go through RuntimeEffect / ly.pixel kernels');
        }

        setImageFilter(filter) {
            throw new Error('Paint.setImageFilter is not available — filter effects go through RuntimeEffect / ly.pixel kernels');
        }

        setColorFilter(filter) {
            throw new Error('Paint.setColorFilter is not available — bake the color transform into the paint color or a kernel');
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
            children: (children || []).map((c) =>
                normalizeKernelSource(c, 'RuntimeEffect.makeShaderWithChildren')),
        };
    }

    /* ── Liangyi 内核（编译型效果 DSL，ly.pixel / ly.scan）────────────
       fn 只被读取源码（fn.toString()）、从不被执行：Rust 侧解析该源码并
       重建计算，自动派发 SKSL（RuntimeEffect）或纯 CPU 逐像素后端。
       约束：lambda 必须是源码内联的箭头函数（不可是原生函数/被压缩）；
       白名单语法 —— 数值/布尔/数组字面量、算术/比较/逻辑/三目、if/else、
       let/const、有界 `for (let i=a; i<b; i++)` 循环、顶层用户函数
       `const f = (p) => …`（单态、无捕获、先声明后使用）、std 内建与
       Math.*、u.<name>、child.eval(pos)、swizzle 读。
       spec: { uniforms: {t:'float', ...} | [['t','float'], ...],
               backend: 'auto'|'sksl'|'cpu' }；scan 内核另有 dir:
       'forward'|'backward'（声明期固定，遍历顺序归 executor 所有）。
       uniforms 展平顺序 = spec 声明序（向量按分量展开）。

       执行动词在 `ly` 上，不在内核上：`ly.draw(dst, kernel, opts)` 把
       内核画到画布，`ly.surface(w,h).apply / .sweep` 把它跑进离屏目标。
       CanvasKit 的 Paint / drawRect / getSubTree / TileMode 一概不出现在
       内核路径里 —— 后端（Skia / CPU）由引擎按内核能力自行选择。 */
    class Kernel {
        constructor(fn, spec, kind, who) {
            this._fnSource = assertLambdaFn(fn, who);
            this.__opencatKernel = true;
            this.kind = kind;
            const s = spec || {};
            this.uniforms = normalizeUniformSpec(s.uniforms, who);
            this.backend = s.backend == null ? 'auto' : String(s.backend);
            if (kind === 'scan') {
                this.dir = s.dir == null ? 'forward' : String(s.dir);
                if (this.dir !== 'forward' && this.dir !== 'backward') {
                    throw new Error(who + ": spec.dir must be 'forward' or 'backward'");
                }
            }
        }

        /* spec JSON（数组保序；kind/backend 声明期固定）*/
        specJson() {
            return JSON.stringify({
                kind: this.kind,
                backend: this.backend,
                uniforms: this.uniforms,
            });
        }
    }

    /* uniforms 声明序归一：对象形式 {name: type} → [['name','type'], ...]
       （JS 字符串键保插入序，即 f32 打包序）；数组形式原样通过。 */
    function normalizeUniformSpec(uniforms, who) {
        if (uniforms == null) return [];
        if (Array.isArray(uniforms)) return uniforms;
        if (typeof uniforms === 'object') {
            const out = [];
            for (const name of Object.keys(uniforms)) {
                out.push([name, String(uniforms[name])]);
            }
            return out;
        }
        throw new Error(who + ': spec.uniforms must be an object or array');
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
        const label = who || 'ly.pixel';
        const specUniforms = effect.uniforms;
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

    /* 内核 source（opts.sources）归一：把用户交出的东西收敛成引擎认识的
       child 描述。ly.source(...) 已产出规范句柄，这里再收一遍裸值
       （<canvas> 句柄 / surface 句柄 / image 句柄）以免用户必须显式包一层。
       —— Skia 的 TileMode / makeShader 不在此暴露；采样模式由后端定。 */
    function normalizeKernelSource(c, who) {
        if (c == null) throw new Error(who + ': source is null');
        if (c.__opencatShader === 'image'
            || c.__opencatShader === 'picture'
            || c.__opencatShader === 'surface') {
            return c;
        }
        /* surface（ly.surface 产出）：session 级像素目标 */
        if (c.__opencatSurface === true) {
            if (typeof c.id !== 'string' || c.id.length === 0) {
                throw new Error(who + ': surface source requires an id');
            }
            return { __opencatShader: 'surface', id: c.id };
        }
        /* <canvas> 句柄（ly.source(canvas) 或直接交 canvas）：子树即像素源 */
        if (c.__opencatCanvas === true) {
            if (typeof c.id !== 'string' || c.id.length === 0) {
                throw new Error(who + ': canvas source requires an id');
            }
            return {
                __opencatShader: 'picture',
                ownerId: c.id,
                tileX: 'clamp',
                tileY: 'clamp',
            };
        }
        /* getSubTree() 句柄：与 canvas 同义（保留兼容，非文档路径） */
        if (c.__opencatSubTreePicture === true) {
            return {
                __opencatShader: 'picture',
                ownerId: String(c.ownerId),
                tileX: 'clamp',
                tileY: 'clamp',
            };
        }
        /* 图片句柄（ctx.getImage 产出）*/
        if (c.__opencatImage === true) {
            return {
                __opencatShader: 'image',
                assetId: String(c.assetId),
                tileX: 'clamp',
                tileY: 'clamp',
            };
        }
        throw new Error(
            who + ': unsupported source (expected ly.source(...), ly.surface(...), '
            + 'an <canvas> handle, or an image handle)'
        );
    }

    function normalizeKernelSources(list, who) {
        if (list == null) return [];
        const arr = isArrayLike(list) ? Array.from(list) : [list];
        return arr.map((c) => normalizeKernelSource(c, who));
    }

    function normalizeTileMode(value) {
        if (value == null) return 'clamp';
        if (value === CanvasKit.TileMode.Clamp) return 'clamp';
        if (value === CanvasKit.TileMode.Repeat) return 'repeat';
        if (value === CanvasKit.TileMode.Mirror) return 'mirror';
        if (value === CanvasKit.TileMode.Decal) return 'decal';
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
            // Official shape: [l, t, r, b, (rx, ry) x 4 corners].
            const normalized = normalizeRect(rect);
            const rxv = toFiniteNumber(rx);
            const ryv = toFiniteNumber(ry);
            return [
                normalized.left,
                normalized.top,
                normalized.right,
                normalized.bottom,
                rxv, ryv, rxv, ryv, rxv, ryv, rxv, ryv
            ];
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
        /* Enum values are the Skia C++ constants (canvaskit-wasm exposes them
           as embind numbers), so `CK.PaintStyle.Fill === 0` etc. holds. */
        PaintStyle: {
            Fill: 0,
            Stroke: 1
        },
        StrokeCap: {
            Butt: 0,
            Round: 1,
            Square: 2
        },
        StrokeJoin: {
            Miter: 0,
            Round: 1,
            Bevel: 2
        },
        BlendMode: {
            Clear: 0,
            Src: 1,
            Dst: 2,
            SrcOver: 3,
            DstOver: 4,
            SrcIn: 5,
            SrcOut: 6,
            DstIn: 7,
            DstOut: 8,
            SrcATop: 9,
            DstATop: 10,
            Xor: 11,
            Plus: 12,
            Modulate: 13,
            Multiply: 14,
            Screen: 15,
            Overlay: 16,
            Darken: 17,
            Lighten: 18,
            ColorDodge: 19,
            ColorBurn: 20,
            HardLight: 21,
            SoftLight: 22,
            Difference: 23,
            Exclusion: 24,
            Hue: 25,
            Saturation: 26,
            Color: 27,
            Luminosity: 28
        },
        FontEdging: {
            Alias: 0,
            AntiAlias: 1,
            SubpixelAntiAlias: 2
        },
        FontHinting: {
            None: 0,
            Slight: 1,
            Normal: 2,
            Full: 3
        },
        ClipOp: {
            Difference: 0,
            Intersect: 1
        },
        PointMode: {
            Points: 0,
            Lines: 1,
            Polygon: 2
        },
        TileMode: {
            Clamp: 0,
            Repeat: 1,
            Mirror: 2,
            Decal: 3
        },
        RuntimeEffect: {
            Make(sksl) {
                if (typeof sksl !== 'string' || sksl.length === 0) return null;
                return new RuntimeEffect(sksl);
            }
        },
        /* Signature-only surface (zero corpus): the official gradient shaders
           error with the supported alternative instead of silently diverging. */
        Shader: {
            MakeLinearGradient(start, end, colors, positions, mode, localMatrix) {
                throw new Error('Shader.MakeLinearGradient is not available — gradient fills are backed by RuntimeEffect / ly.pixel kernels');
            },
            MakeRadialGradient(center, radius, colors, positions, mode, localMatrix) {
                throw new Error('Shader.MakeRadialGradient is not available — gradient fills are backed by RuntimeEffect / ly.pixel kernels');
            },
            MakeBlend(mode, dst, src) {
                throw new Error('Shader.MakeBlend is not available — gradient fills are backed by RuntimeEffect / ly.pixel kernels');
            },
        },
        /* Signature-only factories (zero corpus): the record objects exist so
           the call resolves; Paint.setMaskFilter / setImageFilter fail loud. */
        MaskFilter: {
            MakeBlur(style, sigma, respectCTM) {
                return { __opencatMaskFilter: true, style: style, sigma: sigma, respectCTM: respectCTM };
            },
        },
        ImageFilter: {
            MakeBlur(sigmaX, sigmaY, mode) {
                return { __opencatImageFilter: true, sigmaX: sigmaX, sigmaY: sigmaY, mode: mode };
            },
        },
        /* Signature-only (zero corpus): paragraph text is data-only in OpenCat —
           core lays out glyph runs (cosmic-text); draw text through the 2D
           surface or declare it in JSX. */
        ParagraphStyle: function ParagraphStyle(style) {
            throw new Error('ParagraphStyle is not available — draw text with the 2D surface fillText / JSX text');
        },
        ParagraphBuilder: {
            Make(style, fontMgr) {
                throw new Error('ParagraphBuilder.Make is not available — draw text with the 2D surface fillText / JSX text');
            },
        },
        /* Official signatures: `canvas` is an id string, a useRef() element
           handle, or a DOM handle with .id. Returns a recording Surface —
           draw calls land on the canvas node's DrawOp stream. */
        MakeCanvasSurface(canvas) {
            return makeRecordingSurface(canvas);
        },
        MakeWebGLCanvasSurface(canvas) {
            return makeRecordingSurface(canvas);
        },
        BLACK: [0, 0, 0, 1],
        WHITE: [1, 1, 1, 1]
    };

    function applyFillPaint(id, paint) {
        __canvas_set_anti_alias(id, paint._antiAlias);
        __canvas_set_fill_style(id, colorToCss(paint._color));
    }

    // The official enum values are numbers (Skia constants); the `__canvas_*`
    // bindings take the historical string names, so calls translate here.
    const STROKE_CAP_NAMES = ['butt', 'round', 'square'];
    const STROKE_JOIN_NAMES = ['miter', 'round', 'bevel'];
    const FONT_EDGING_NAMES = ['alias', 'antiAlias', 'subpixelAntiAlias'];

    function strokeCapName(value) {
        if (typeof value === 'string') return value;
        const name = STROKE_CAP_NAMES[value];
        if (!name) throw new Error(`unsupported StrokeCap: ${value}`);
        return name;
    }

    function strokeJoinName(value) {
        if (typeof value === 'string') return value;
        const name = STROKE_JOIN_NAMES[value];
        if (!name) throw new Error(`unsupported StrokeJoin: ${value}`);
        return name;
    }

    function fontEdgingName(value) {
        if (typeof value === 'string') return value;
        const name = FONT_EDGING_NAMES[value];
        if (!name) throw new Error(`unsupported FontEdging: ${value}`);
        return name;
    }

    function applyStrokePaint(id, paint) {
        __canvas_set_anti_alias(id, paint._antiAlias);
        __canvas_set_stroke_style(id, colorToCss(paint._color));
        __canvas_set_line_width(id, paint._strokeWidth);
        __canvas_set_line_cap(id, strokeCapName(paint._strokeCap));
        __canvas_set_line_join(id, strokeJoinName(paint._strokeJoin));
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
            __opencatCanvas: true,
            id: String(id),
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
                        sh.effect.specJson(),
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
                    fontEdgingName(resolvedFont._edging)
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
                    : (typeof mode === 'string' ? mode : null);
                if (modeStr === null) {
                    throw new Error(`unsupported PointMode: ${mode}`);
                }
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

            /* Signature placeholder (zero corpus, see ARCHITECTURE.md §2):
               paragraph text is laid out in core (cosmic-text) and drawn as
               data; there is no SkParagraph on the canvas path. */
            drawParagraph(paragraph, x, y) {
                throw new Error('drawParagraph is not available — draw text with the 2D surface fillText / JSX text');
            },
        };
    }

    globalThis.CanvasKit = CanvasKit;
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
        throw new Error("ctx.getCanvas is not available; use CanvasKitInit().then(CK => CK.MakeCanvasSurface(id))");
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
        // font+letterSpacing and fillStyle are parsed with a regex / string
        // scan on every fillText. Cache the parsed forms, invalidated by the
        // setters, so the hot path (canvas-close calls fillText ~13k times a
        // frame) stays on the precomputed values.
        let specCache = null;
        let fillCssCache = null;
        function fontSpec() {
            if (
                specCache !== null &&
                specCache.font === state.font &&
                specCache.letterSpacing === state.letterSpacing
            ) {
                return specCache.spec;
            }
            const f = parseCssFont(state.font);
            const ls = parseFloat(String(state.letterSpacing));
            const spec = {
                weight: f.weight,
                size: f.size,
                family: f.family,
                letterSpacing: Number.isFinite(ls) ? ls : 0
            };
            specCache = { font: state.font, letterSpacing: state.letterSpacing, spec };
            return spec;
        }
        return {
            __opencatSurface: true,
            id: id,
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
                    actualBoundingBoxDescent: m.ink_descent,
                    fontBoundingBoxAscent: m.font_ascent,
                    fontBoundingBoxDescent: m.font_descent
                };
            },
            fillText(text, x, y) {
                if (state.textBaseline !== 'alphabetic') {
                    throw new Error('surface fillText: only the alphabetic baseline is supported');
                }
                const f = fontSpec();
                const css = fillCssCache !== null ? fillCssCache : (fillCssCache = colorToCss(state.fillStyle));
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
                    css
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
            /* ── Render target 执行（§render target）────────────────────
               JS 拥有算法：内核源码只被 Rust 编译、从不执行，像素缓冲
               永不跨进 JS；引擎侧只有通用执行器（逐像素重绘 / 顺序扫描），
               不含任何具体效果算法。
               - apply(kernel, opts)：ly.pixel 内核逐像素重绘本目标（f64
                 解释器，rayon 按行并行）。opts: { uniforms, sources }。
                 sources 可采样其它 surface（session 级，不上 wire）、
                 图片或 <canvas> 子树。
               - sweep(kernel, opts)：ly.scan 内核（首参 get）就地顺序
                 扫描一遍；方向声明在内核上（spec.dir），get(dx,dy) 读
                 in-progress 字节（clamp-to-edge）。单线程。
               本目标像素直接作 ly.draw 的 source 采样 —— 无需注册成帧级
               生成图像，绘制路径会自行烘焙（见 ly.draw）。 */
            apply(kernel, opts) {
                if (!kernel || kernel.__opencatKernel !== true || kernel.kind !== 'pixel') {
                    throw new Error('surface.apply: expected a ly.pixel kernel');
                }
                if (kernel.backend === 'sksl') {
                    throw new Error(
                        'surface.apply: target ops always run on the CPU interpreter (sksl is a draw-time backend)'
                    );
                }
                const o = opts || {};
                const flat = flattenLambdaUniforms(kernel, o.uniforms, 'surface.apply');
                const kids = normalizeKernelSources(o.sources, 'surface.apply');
                if (__surface_apply_effect(id, kernel._fnSource, kernel.specJson(), flat, JSON.stringify(kids)) !== true) {
                    throw new Error('surface.apply: engine rejected the kernel');
                }
                return this;
            },
            sweep(kernel, opts) {
                if (!kernel || kernel.__opencatKernel !== true || kernel.kind !== 'scan') {
                    throw new Error('surface.sweep: expected a ly.scan kernel');
                }
                if (kernel.backend === 'sksl') {
                    throw new Error(
                        'surface.sweep: scan kernels have no SKSL form (ordered scan is CPU-interpreter only)'
                    );
                }
                const o = opts || {};
                const flat = flattenLambdaUniforms(kernel, o.uniforms, 'surface.sweep');
                if (__surface_scan_pass(id, kernel._fnSource, kernel.specJson(), flat, kernel.dir) !== true) {
                    throw new Error('surface.sweep: engine rejected the kernel');
                }
                return this;
            },
            get font() { return state.font; },
            set font(v) { state.font = String(v); specCache = null; },
            get letterSpacing() { return state.letterSpacing; },
            set letterSpacing(v) { state.letterSpacing = String(v); specCache = null; },
            get fillStyle() { return state.fillStyle; },
            set fillStyle(v) { state.fillStyle = v; fillCssCache = null; },
            get textBaseline() { return state.textBaseline; },
            set textBaseline(v) { state.textBaseline = String(v); }
        };
    }

    /* 内部：按 id 取或建一张 surface 句柄。id 由 ly.surface 生成（尺寸派生），
       脚本不再直接持有它 —— 这是「隐藏 canvas 细节」的一部分。 */
    function surfaceById(id, w, h) {
        id = String(id);
        if (!surfaceCache[id]) {
            __surface_create(id, toFiniteNumber(w), toFiniteNumber(h));
            surfaceCache[id] = makeSurface(id);
        }
        return surfaceCache[id];
    }

    /* ── ly：Liangyi（两仪）效果内核语言命名空间 ─────────────────────
       后端无关门面：内核只被编译、从不执行；像素由后端实例化（CPU 解释器
       / Skia RuntimeEffect / 将来的 wgpu）。**CanvasKit 不出现在 ly 路径
       里** —— 没有 Paint、没有 drawRect、没有 getSubTree().makeShader、
       没有 TileMode。内核画到画布用 `ly.draw`，跑进离屏目标用
       `ly.surface(w,h).apply/.sweep`。
       - ly.pixel(fn, spec)：pixel 类内核。spec.uniforms 用对象按名声明
         （{t:'float', pa:'float4'}，键序即 f32 打包序）；spec.backend
         'auto'（默认，Rust 依内建能力派发）| 'sksl' | 'cpu'。
       - ly.scan(fn, spec)：scan 类内核（首参 get）。spec.dir 声明
         遍历方向（'forward' | 'backward'，声明期固定）。
       - ly.draw(dst, kernel, opts)：把内核画到 canvas（dst 是 useCanvas
         回调收到的画布句柄）。opts: { rect?, uniforms?, sources? }。
         rect 缺省 = 整画布；sources 依 child 声明序。
       - ly.surface(w, h)：离屏目标（按尺寸 get-or-create）。2D 子集
         （fillText / measureText / getImageData / …）与 .apply/.sweep。
       - ly.source(x)：把 canvas 句柄 / image 句柄 / surface 包成像素源，
         供 opts.sources 采样。 */
    const ly = {        pixel(fn, spec) {
            return new Kernel(fn, spec, 'pixel', 'ly.pixel');
        },
        scan(fn, spec) {
            return new Kernel(fn, spec, 'scan', 'ly.scan');
        },

        /* 把内核画到画布。dst 必须是 canvas 句柄（useCanvas 的 c）；
           rect 缺省为整画布（composition 尺寸），显式给时是 [x, y, w, h]
           —— 与 CK.XYWHRect 的实参序一致，不含 [left,top,right,bottom]
           的歧义。 */
        draw(dst, kernel, opts) {
            if (!dst || dst.__opencatCanvas !== true) {
                throw new Error('ly.draw: dst must be a canvas handle (the first arg of useCanvas)');
            }
            if (!kernel || kernel.__opencatKernel !== true) {
                throw new Error('ly.draw: expected a ly.pixel / ly.scan kernel');
            }
            if (kernel.kind === 'scan') {
                throw new Error('ly.draw: scan kernels are ordered passes; run them with surface.sweep');
            }
            const o = opts || {};
            const [rx, ry, rw, rh] = o.rect == null
                ? ly._canvasRect(dst)
                : ly._xywh(o.rect, 'ly.draw');
            const flat = flattenLambdaUniforms(kernel, o.uniforms, 'ly.draw');
            const kids = normalizeKernelSources(o.sources, 'ly.draw');
            if (__canvas_lambda_effect_draw(
                dst.id,
                kernel._fnSource,
                kernel.specJson(),
                flat,
                JSON.stringify(kids),
                rx,
                ry,
                rw,
                rh,
            ) !== true) {
                throw new Error('ly.draw: engine rejected the kernel');
            }
            return dst;
        },

        /* 离屏目标：按尺寸 get-or-create，所以同一句 `ly.surface(1460, 222)`
           跨帧拿到同一张目标 —— 无需 id、无需跨帧挂载点。 */
        surface(w, h) {
            const ww = Math.max(1, Math.round(toFiniteNumber(w)));
            const hh = Math.max(1, Math.round(toFiniteNumber(h)));
            const key = 'ly-' + ww + 'x' + hh;
            return surfaceById(key, ww, hh);
        },

        /* 像素源句柄：canvas / image / surface 统一入口，供 opts.sources。 */
        source(x) {
            return normalizeKernelSource(x, 'ly.source');
        },

        /* 整画布 rect：优先读 composition meta，回退 1920x1080。 */
        _canvasRect(dst) {
            const meta = globalThis.__ocMeta || {};
            const w = toFiniteNumber(meta.width, 0) || 1920;
            const h = toFiniteNumber(meta.height, 0) || 1080;
            return [0, 0, w, h];
        },

        /* 显式 rect 归一：只接受 [x, y, w, h]。 */
        _xywh(rect, who) {
            if (!isArrayLike(rect) || rect.length < 4) {
                throw new Error(who + ': rect must be [x, y, width, height]');
            }
            return [
                toFiniteNumber(rect[0]),
                toFiniteNumber(rect[1]),
                toFiniteNumber(rect[2]),
                toFiniteNumber(rect[3]),
            ];
        },
    };
    globalThis.ly = ly;
    ctx.ly = ly;

    // ── ctx.surface: the standard offscreen 2D text surface ─────────
    // Same facade ly.surface hands out (real-font fillText / measureText /
    // getImageData over the scoped fontdb), exposed under its own name so
    // compositions don't have to reach into the liangyi namespace. The
    // surface itself was never a liangyi kernel — only the name was.
    ctx.surface = function (w, h) {
        const ww = Math.max(1, Math.round(toFiniteNumber(w)));
        const hh = Math.max(1, Math.round(toFiniteNumber(h)));
        return surfaceById('surface-' + ww + 'x' + hh, ww, hh);
    };

    // ── OffscreenCanvas: the standard web spelling of `ctx.surface` ──
    // `new OffscreenCanvas(w, h).getContext('2d')` is how a browser script
    // allocates a detached 2D drawing surface; here it resolves to the same
    // facade `ctx.surface(w, h)` returns, so both spellings share one target.
    function OffscreenCanvas(w, h) {
        const ww = Math.max(1, Math.round(toFiniteNumber(w)));
        const hh = Math.max(1, Math.round(toFiniteNumber(h)));
        this.width = ww;
        this.height = hh;
        this.getContext = function (kind) {
            if (kind !== '2d') {
                throw new Error("OffscreenCanvas.getContext: only '2d' is supported, got '" + kind + "'");
            }
            return ctx.surface(ww, hh);
        };
    }
    globalThis.OffscreenCanvas = OffscreenCanvas;

    // ── Official surface entry: CK.MakeCanvasSurface(id) ─────────────
    // `canvas` accepts an id string, a useRef() element handle, or a DOM
    // handle carrying .id — the official MakeCanvasSurface signature takes a
    // canvas or an id. The returned Surface records: getCanvas() is the
    // CanvasKit Canvas facade whose draw calls land on the canvas node's
    // DrawOp stream; flush()/dispose() are no-ops (no live GPU surface).
    // Pixel-buffer style sizes error (the offscreen pixel target is
    // ly.surface(w, h)); makeImageSnapshot / requestAnimationFrame have no
    // engine path and error with a pointer.
    function resolveCanvasTargetId(canvas) {
        if (typeof canvas === 'string' && canvas) {
            return canvas;
        }
        if (canvas && canvas.__ocEl === true) {
            const declared = canvas.props && canvas.props.id;
            if (declared === undefined || declared === null) {
                throw new Error('MakeCanvasSurface: the <canvas> element needs an id="..." so the surface can target it');
            }
            return String(declared);
        }
        if (canvas && typeof canvas.id === 'string' && canvas.id) {
            return canvas.id;
        }
        throw new Error('MakeCanvasSurface: expected an id string or a <canvas> element handle');
    }

    function surfaceMetaNumber(name) {
        const meta = globalThis.__ocMeta || {};
        return Math.max(0, toFiniteNumber(meta[name], 0));
    }

    function makeRecordingSurface(canvas) {
        const id = resolveCanvasTargetId(canvas);
        const record = ctx.getCanvasById(id);
        return {
            getCanvas() {
                return record;
            },
            flush() {},
            dispose() {},
            width() {
                return surfaceMetaNumber('width');
            },
            height() {
                return surfaceMetaNumber('height');
            },
            makeImageSnapshot() {
                throw new Error('Surface.makeImageSnapshot is not available — read pixels back with ly.surface(w, h) + getImageData, or draw with putImageData');
            },
            requestAnimationFrame() {
                throw new Error('Surface.requestAnimationFrame is not available — frames are f(t); register per-frame drawing with useEffect/useGSAP');
            },
        };
    }

    // ── Official wasm-loader stub ────────────────────────────────────
    // `import CanvasKitInit from 'canvaskit-wasm'` resolves here. The official
    // JS layer is API sugar + a WASM loader; drawing semantics live in DrawOp
    // plus the two replay backends (engine / web), so the stub resolves
    // immediately with the facade — deterministic, first-frame visible.
    function CanvasKitInit(options) {
        return Promise.resolve(globalThis.CanvasKit);
    }
    globalThis.CanvasKitInit = CanvasKitInit;
})();
