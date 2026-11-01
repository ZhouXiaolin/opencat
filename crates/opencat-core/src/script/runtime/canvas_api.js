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
        /* `typeface` 是惰性句柄（Typeface.MakeFreeTypeFaceFromData 的返回，
           当前为 null）：字体字节由宿主 scoped fontdb 供给，句柄不跨桥。
           接受任意值让 g 系 fontLoader 原样跑通。 */
        constructor(typeface = null, size = 16, scaleX = 1, skewX = 0) {
            this._typeface = typeface;
            this._size = Math.max(1, toFiniteNumber(size, 16));
            this._scaleX = toFiniteNumber(scaleX, 1);
            this._skewX = toFiniteNumber(skewX, 0);
            this._subpixel = true;
            this._edging = CanvasKit.FontEdging.AntiAlias;
        }

        copy() {
            const copy = new Font(this._typeface, this._size, this._scaleX, this._skewX);
            copy._subpixel = this._subpixel;
            copy._edging = this._edging;
            return copy;
        }

        delete() {}

        getSize() {
            return this._size;
        }

        /* 文字基线盒。度量绑定 `__canvas_measure_paragraph` 缺席时（web 重放
           无该绑定）按 Skia 常见比例兜底——过渡面，批次 B 统一走 core 文本
           引擎后由真度量供给。 */
        getMetrics() {
            const measured = typeof __canvas_measure_paragraph === 'function'
                ? __canvas_measure_paragraph('', this._size, 'sans-serif', 400)
                : null;
            return {
                ascent: toFiniteNumber(measured && measured.ascent, this._size * 0.8),
                descent: toFiniteNumber(measured && measured.descent, this._size * 0.2),
                leading: 0,
            };
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
            // SkPaint 默认态：不透明黑、Fill、hairline(0)、无 AA、Butt/Miter。
            // g 系渲染器在 `lineWidth > 0` 的 guard 之前就构造 stroke paint，
            // guard 跳过整个配置块后 paint 保持默认——真 CanvasKit 画出的
            // 就是默认样式（G2 柱状图全黑的真上游行为），这里必须同默认。
            this._color = [0, 0, 0, 1];
            this._style = CanvasKit.PaintStyle.Fill;
            this._strokeWidth = 0;
            this._strokeCap = CanvasKit.StrokeCap.Butt;
            this._strokeJoin = CanvasKit.StrokeJoin.Miter;
            this._pathEffect = null;
            this._antiAlias = false;
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
                && shader.__opencatShader !== 'generated') {
                throw new Error('setShader expects a shader handle');
            }
            this._shader = shader;
            return this;
        }

        setMaskFilter(filter) {
            throw new Error('Paint.setMaskFilter is not available — blur effects go through RuntimeEffect shaders');
        }

        setImageFilter(filter) {
            throw new Error('Paint.setImageFilter is not available — filter effects go through RuntimeEffect shaders');
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

        /* 官方签名：transform(scaleX, skewX, transX, skewY, scaleY, transY,
           pers0, pers1, pers2) 或 transform(matrix9, resultPath?)——数组按
           行主序读。g-canvaskit 的阴影路径传 gl-matrix mat3（列主序）原样
           进来，官方 canvaskit 同样按行主序误读——两端保持同一误读即对齐。
           仿射六元生效，透视分量忽略（阴影是纯平移）。 */
        transform(...args) {
            let m;
            if (args.length === 1 && Array.isArray(args[0])) {
                m = args[0];
            } else if (args.length >= 6) {
                m = args;
            } else {
                throw new Error('Path.transform: expected a 9-entry matrix or 9 numbers');
            }
            const [a, b, c, d, e, f] = m.map((v) => toFiniteNumber(v, 0));
            const apply = (x, y) => [a * x + b * y + c, d * x + e * y + f];
            const rebuilt = [];
            for (const op of this._ops) {
                if (op[0] === 'close') {
                    rebuilt.push(op.slice());
                    continue;
                }
                if (op[0] === 'addRect') {
                    const [x, y, w, h] = [op[1], op[2], op[3], op[4]];
                    const [x0, y0] = apply(x, y);
                    const [x1, y1] = apply(x + w, y + h);
                    rebuilt.push(['addRect', Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0)]);
                    continue;
                }
                if (op[0] === 'addRRect') {
                    // 圆角矩阵变换后不再是标准 RRect；退化为外接矩形。
                    const [x, y, w, h] = [op[1], op[2], op[3], op[4]];
                    const [x0, y0] = apply(x, y);
                    const [x1, y1] = apply(x + w, y + h);
                    rebuilt.push(['addRect', Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0)]);
                    continue;
                }
                const next = op.slice();
                for (let i = 1; i + 1 < next.length; i += 2) {
                    const [px, py] = apply(next[i], next[i + 1]);
                    next[i] = px;
                    next[i + 1] = py;
                }
                rebuilt.push(next);
            }
            this._ops = rebuilt;
            return this;
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

        /* 当前画笔落点：扫描 ops 取最后一个端点；close 回到子路径起点。
           arcToRotated 的椭圆弧以它为起点。 */
        _lastPoint() {
            let x = 0;
            let y = 0;
            let subpathStart = null;
            for (const op of this._ops) {
                if (op[0] === 'close') {
                    if (subpathStart) {
                        x = subpathStart[0];
                        y = subpathStart[1];
                    }
                    continue;
                }
                if (op[0] === 'addRect' || op[0] === 'addRRect') {
                    x = op[1];
                    y = op[2];
                    subpathStart = [x, y];
                    continue;
                }
                x = op[op.length - 2];
                y = op[op.length - 1];
                if (op[0] === 'moveTo') subpathStart = [x, y];
            }
            return { x, y };
        }

        /* 官方签名 arcToRotated(rx, ry, xAxisDeg, largeArc, sweep, x, y)：
           SVG 椭圆弧语义，分解为三次贝塞尔（90° 段近似）。饼图/theta 坐标
           系的弧形 mark 走这里。 */
        arcToRotated(rx, ry, xAxisDegree, largeArc, sweep, x, y) {
            rx = Math.abs(toFiniteNumber(rx));
            ry = Math.abs(toFiniteNumber(ry));
            const x2 = toFiniteNumber(x);
            const y2 = toFiniteNumber(y);
            const cur = this._lastPoint();
            const x1 = cur.x;
            const y1 = cur.y;
            if (rx === 0 || ry === 0 || (x1 === x2 && y1 === y2)) {
                this.lineTo(x2, y2);
                return this;
            }
            const phi = toFiniteNumber(xAxisDegree) * Math.PI / 180;
            const cosPhi = Math.cos(phi);
            const sinPhi = Math.sin(phi);
            const dx2 = (x1 - x2) / 2;
            const dy2 = (y1 - y2) / 2;
            const x1p = cosPhi * dx2 + sinPhi * dy2;
            const y1p = -sinPhi * dx2 + cosPhi * dy2;
            let rx2 = rx * rx;
            let ry2 = ry * ry;
            const lambda = (x1p * x1p) / rx2 + (y1p * y1p) / ry2;
            if (lambda > 1) {
                const s = Math.sqrt(lambda);
                rx *= s;
                ry *= s;
                rx2 = rx * rx;
                ry2 = ry * ry;
            }
            const sign = largeArc !== sweep ? 1 : -1;
            const num = rx2 * ry2 - rx2 * y1p * y1p - ry2 * x1p * x1p;
            const den = rx2 * y1p * y1p + ry2 * x1p * x1p;
            const coef = sign * Math.sqrt(Math.max(0, num / den));
            const cxp = (coef * rx * y1p) / ry;
            const cyp = (-coef * ry * x1p) / rx;
            const cx = cosPhi * cxp - sinPhi * cyp + (x1 + x2) / 2;
            const cy = sinPhi * cxp + cosPhi * cyp + (y1 + y2) / 2;
            const angleBetween = (ux, uy, vx, vy) => {
                const dot = ux * vx + uy * vy;
                const len = Math.sqrt(ux * ux + uy * uy) * Math.sqrt(vx * vx + vy * vy);
                let a = Math.acos(Math.min(1, Math.max(-1, dot / len)));
                if (ux * vy - uy * vx < 0) a = -a;
                return a;
            };
            const theta1 = angleBetween(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry);
            let dTheta = angleBetween((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
            if (!sweep && dTheta > 0) dTheta -= 2 * Math.PI;
            if (sweep && dTheta < 0) dTheta += 2 * Math.PI;
            const segments = Math.ceil(Math.abs(dTheta / (Math.PI / 2)));
            const delta = dTheta / segments;
            const t = (4 / 3) * Math.tan(delta / 4);
            let px = x1;
            let py = y1;
            for (let i = 0; i < segments; i++) {
                const th0 = theta1 + i * delta;
                const th1 = th0 + delta;
                const cos0 = Math.cos(th0);
                const sin0 = Math.sin(th0);
                const cos1 = Math.cos(th1);
                const sin1 = Math.sin(th1);
                const ex = cosPhi * rx * cos1 - sinPhi * ry * sin1 + cx;
                const ey = sinPhi * rx * cos1 + cosPhi * ry * sin1 + cy;
                const d0x = t * (-cosPhi * rx * sin0 - sinPhi * ry * cos0);
                const d0y = t * (-sinPhi * rx * sin0 + cosPhi * ry * cos0);
                const d1x = t * (cosPhi * rx * sin1 + sinPhi * ry * cos1);
                const d1y = t * (sinPhi * rx * sin1 - cosPhi * ry * cos1);
                this.cubicTo(px + d0x, py + d0y, ex + d1x, ey + d1y, ex, ey);
                px = ex;
                py = ey;
            }
            return this;
        }

        /* 官方签名 addPoly(points, close)：points 是 [x0,y0,x1,y1,…] 平铺。 */
        addPoly(points, close) {
            const flat = Array.prototype.slice.call(points);
            for (let i = 0; i + 1 < flat.length; i += 2) {
                if (i === 0) this.moveTo(flat[0], flat[1]);
                else this.lineTo(flat[i], flat[i + 1]);
            }
            if (close) this.close();
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

    /* Web 标准 Path2D：context2d.stroke/fill 直接收它。构造器支持 SVG
       path 字符串（custom-pattern-with-canvas 的斜线纹理 d）、另一个
       Path2D（拷贝）、或空。内部与 CanvasKit Path 同款 `_ops` 形态，
       context2d 端 replayPath 原样可用；SVG 构造走 `__oc_path_from_svg`
       （usvg 解析，与 makePathFromSVGString 同通道）。 */
    class Path2D {
        constructor(source) {
            this._ops = [];
            if (source === undefined || source === null) return;
            if (typeof source === 'string') {
                if (typeof __oc_path_from_svg !== 'function') {
                    throw new Error('Path2D(string) requires the host path binding');
                }
                const flat = __oc_path_from_svg(String(source));
                if (!flat) return;
                for (let i = 0; i < flat.length;) {
                    const cmd = flat[i++];
                    if (cmd === 0) { this.moveTo(flat[i++], flat[i++]); }
                    else if (cmd === 1) { this.lineTo(flat[i++], flat[i++]); }
                    else if (cmd === 2) { this.quadTo(flat[i++], flat[i++], flat[i++], flat[i++]); }
                    else if (cmd === 3) { this.cubicTo(flat[i++], flat[i++], flat[i++], flat[i++], flat[i++], flat[i++]); }
                    else if (cmd === 4) { this.close(); }
                    else { throw new Error(`Path2D: unknown verb ${cmd}`); }
                }
                return;
            }
            if (source instanceof Path2D || (source && Array.isArray(source._ops))) {
                this._ops = source._ops.map((op) => op.slice());
                return;
            }
            throw new Error('Path2D: constructor expects an SVG path string or another Path2D');
        }

        moveTo(x, y) { this._ops.push(['moveTo', toFiniteNumber(x), toFiniteNumber(y)]); }
        lineTo(x, y) { this._ops.push(['lineTo', toFiniteNumber(x), toFiniteNumber(y)]); }
        quadTo(x1, y1, x2, y2) {
            this._ops.push(['quadTo', toFiniteNumber(x1), toFiniteNumber(y1), toFiniteNumber(x2), toFiniteNumber(y2)]);
        }
        bezierCurveTo(x1, y1, x2, y2, x3, y3) {
            this._ops.push([
                'cubicTo',
                toFiniteNumber(x1), toFiniteNumber(y1),
                toFiniteNumber(x2), toFiniteNumber(y2),
                toFiniteNumber(x3), toFiniteNumber(y3)
            ]);
        }
        closePath() { this._ops.push(['close']); }

        /* Web 标准矩阵语义（DOMMatrix 顺序：m11 m12 m21 m22 dx dy，列主）。
           只支持仿射六元——Path2D.transform 不带透视。 */
        transform(m11, m12, m21, m22, dx, dy) {
            const apply = (x, y) => [m11 * x + m21 * y + dx, m12 * x + m22 * y + dy];
            const rebuilt = [];
            for (const op of this._ops) {
                if (op[0] === 'close') { rebuilt.push(op.slice()); continue; }
                const next = op.slice();
                for (let i = 1; i + 1 < next.length; i += 2) {
                    const [px, py] = apply(next[i], next[i + 1]);
                    next[i] = px;
                    next[i + 1] = py;
                }
                rebuilt.push(next);
            }
            this._ops = rebuilt;
        }

        addPath(path, transform) {
            if (!(path instanceof Path2D)) {
                throw new Error('Path2D.addPath: expected a Path2D');
            }
            let t = null;
            if (transform) {
                t = [
                    toFiniteNumber(transform.a, 1), toFiniteNumber(transform.b, 0),
                    toFiniteNumber(transform.c, 0), toFiniteNumber(transform.d, 1),
                    toFiniteNumber(transform.e, 0), toFiniteNumber(transform.f, 0)
                ];
            }
            for (const op of path._ops) {
                if (!t || op[0] === 'close') { this._ops.push(op.slice()); continue; }
                const apply = (x, y) => [t[0] * x + t[2] * y + t[4], t[1] * x + t[3] * y + t[5]];
                const next = op.slice();
                for (let i = 1; i + 1 < next.length; i += 2) {
                    const [px, py] = apply(next[i], next[i + 1]);
                    next[i] = px;
                    next[i + 1] = py;
                }
                this._ops.push(next);
            }
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

    /* RuntimeEffect child（makeShaderWithChildren）归一：把用户交出的东西
       收敛成引擎认识的 child 描述 —— <canvas> 句柄（子树 picture）、
       getSubTree() 句柄或 ctx.getImage 句柄。 */
    function normalizeKernelSource(c, who) {
        if (c == null) throw new Error(who + ': source is null');
        if (c.__opencatShader === 'image'
            || c.__opencatShader === 'picture') {
            return c;
        }
        /* <canvas> 句柄：子树即像素源 */
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
        /* surface 句柄（ctx.surface 产出）：session 级像素，不跨线 —— 要当
           纹理采样先 getImageData + putImageData 走 generated image。 */
        if (c.__opencatSurface === true) {
            throw new Error(who
                + ': a surface is a session buffer and cannot be a shader child — '
                + 'read it with surface.getImageData() and putImageData() it onto the canvas');
        }
        throw new Error(
            who + ': unsupported source (expected canvas.getSubTree().makeShader() '
            + 'or an image handle from ctx.getImage)'
        );
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

    /* ── Paragraph 实现（批次 B：shape 载荷 → path，度量与绘制同源）──
       一个段落 = 一次度量 + 一次绘制。度量与绘制都吃 `__canvas_glyph_paths`
       的 shape 载荷；emoji（ColorImage）段落走生成图位图。 */
    function paragraphStyleOf(textStyle) {
        const style = textStyle || {};
        const family = Array.isArray(style.fontFamilies) && style.fontFamilies.length
            ? String(style.fontFamilies[0]).replace(/["']/g, '')
            : 'sans-serif';
        const size = toFiniteNumber(style.fontSize, 12);
        const weight = toFiniteNumber(style.fontStyle && style.fontStyle.weight, 400);
        return { family, size, weight, color: style.color };
    }

    function paragraphMetricsOf(text, textStyle) {
        const style = paragraphStyleOf(textStyle);
        /* 批次 B：优先 shape 绑定——glyph path 载荷 + 真排版度量，度量与
           绘制同源。绑定缺席（旧宿主）时落回 surface 的 kern-free 简排。 */
        if (typeof __canvas_glyph_paths === 'function') {
            const shaped = __canvas_glyph_paths(text, style.size, style.family, Math.round(style.weight) || 400) || {};
            const glyphMap = {};
            for (const g of shaped.glyphs || []) glyphMap[g.k] = g.ops;
            const ascent = toFiniteNumber(shaped.ascent, style.size * 0.8);
            const descent = toFiniteNumber(shaped.descent, style.size * 0.2);
            return {
                ...style,
                shaped: true,
                glyphMap,
                positions: shaped.positions || [],
                colorOnly: shaped.color === true,
                width: toFiniteNumber(shaped.width, 0),
                height: ascent + descent,
                ascent,
                descent,
            };
        }
        const measured = typeof __canvas_measure_paragraph === 'function'
            ? (__canvas_measure_paragraph(text, style.size, style.family, style.weight) || {})
            : {};
        const ascent = toFiniteNumber(measured.ascent, style.size * 0.8);
        const descent = toFiniteNumber(measured.descent, style.size * 0.2);
        return {
            ...style,
            width: toFiniteNumber(measured.width, 0),
            height: ascent + descent,
            ascent,
            descent,
        };
    }

    class ParagraphImpl {
        constructor(text, paragraphStyle, textStyle) {
            this.text = String(text == null ? '' : text);
            this.paragraphStyle = paragraphStyle || {};
            this.textStyle = textStyle || {};
            this._laidWidth = 0;
            this.__metrics = null;
        }

        get _metrics() {
            if (!this.__metrics) this.__metrics = paragraphMetricsOf(this.text, this.textStyle);
            return this.__metrics;
        }

        layout(width) {
            this._laidWidth = Math.max(0, toFiniteNumber(width, 0));
        }

        getMaxWidth() {
            return this._metrics.width;
        }

        getHeight() {
            return this._metrics.height;
        }

        /* CanvasKit 的行盒是 ascent（基线上方，正值）+ descent。g-canvaskit
           按 textBaseline 平移段落，其基线在 `ascent` 处——其它取值会把
           每个标签整体移动。 */
        getLineMetrics() {
            const m = this._metrics;
            return [{
                startIndex: 0,
                endIndex: this.text.length,
                ascent: m.ascent,
                descent: m.descent,
                left: 0,
                width: m.width,
                height: m.height,
                baseline: m.ascent,
            }];
        }

        getRectsForRange() {
            const m = this._metrics;
            return [{ rect: { x: 0, y: 0, w: m.width, h: m.height } }];
        }

        delete() {}
    }

    class ParagraphBuilderImpl {
        constructor(paragraphStyle) {
            this.paragraphStyle = paragraphStyle || {};
            this.textStyle = {};
            this.parts = [];
        }

        addText(text) {
            this.parts.push(String(text));
            return this;
        }

        pushStyle(style) {
            if (style) {
                const next = Object.assign({}, this.textStyle);
                for (const key in style) {
                    if (Object.prototype.hasOwnProperty.call(style, key)) {
                        const value = style[key];
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
        }

        pop() {
            return this;
        }

        build() {
            return new ParagraphImpl(this.parts.join(''), this.paragraphStyle, this.textStyle);
        }

        delete() {}
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
        /* ── Matrix：官方 canvaskit-wasm helpers 的 3x3 row-major 展开 ──
           x' = m0·x + m1·y + m2（透视 w = m6·x + m7·y + m8，见 mapPoints），
           内存序与 `__canvas_concat` → SkMatrix 一致。原样展开自
           canvaskit.js：rotated/scaled/skewed 的 pivot 平移补正、invert 的
           伴随除行列式（不可逆 → null）、multiply 的左到右 reduce。 */
        Matrix: {
            identity() {
                return [1, 0, 0, 0, 1, 0, 0, 0, 1];
            },
            invert(m) {
                const d = m[0] * m[4] * m[8] + m[1] * m[5] * m[6] + m[2] * m[3] * m[7]
                    - m[2] * m[4] * m[6] - m[1] * m[3] * m[8] - m[0] * m[5] * m[7];
                if (!d) return null;
                return [
                    (m[4] * m[8] - m[5] * m[7]) / d, (m[2] * m[7] - m[1] * m[8]) / d, (m[1] * m[5] - m[2] * m[4]) / d,
                    (m[5] * m[6] - m[3] * m[8]) / d, (m[0] * m[8] - m[2] * m[6]) / d, (m[2] * m[3] - m[0] * m[5]) / d,
                    (m[3] * m[7] - m[4] * m[6]) / d, (m[1] * m[6] - m[0] * m[7]) / d, (m[0] * m[4] - m[1] * m[3]) / d,
                ];
            },
            mapPoints(m, pts) {
                for (let i = 0; i < pts.length; i += 2) {
                    const x = pts[i];
                    const y = pts[i + 1];
                    const w = m[6] * x + m[7] * y + m[8];
                    const yRow = m[3] * x + m[4] * y + m[5];
                    pts[i] = (m[0] * x + m[1] * y + m[2]) / w;
                    pts[i + 1] = yRow / w;
                }
                return pts;
            },
            multiply(...ms) {
                if (ms.length < 2) {
                    throw new Error('Matrix.multiply expects at least two matrices');
                }
                let acc = ms[0].slice();
                for (let i = 1; i < ms.length; i++) {
                    const a = acc;
                    const b = ms[i];
                    const out = new Array(9);
                    for (let r = 0; r < 3; r++) {
                        for (let c = 0; c < 3; c++) {
                            let sum = 0;
                            for (let k = 0; k < 3; k++) sum += a[r * 3 + k] * b[k * 3 + c];
                            out[r * 3 + c] = sum;
                        }
                    }
                    acc = out;
                }
                return acc;
            },
            rotated(radians, px = 0, py = 0) {
                const s = Math.sin(radians);
                const c = Math.cos(radians);
                return [c, -s, s * py + (1 - c) * px, s, c, -s * px + (1 - c) * py, 0, 0, 1];
            },
            scaled(sx, sy, px = 0, py = 0) {
                return [sx, 0, px - sx * px, 0, sy, py - sy * py, 0, 0, 1];
            },
            skewed(kx, ky, px = 0, py = 0) {
                return [1, kx, -kx * px, ky, 1, -ky * py, 0, 0, 1];
            },
            translated(dx, dy) {
                return [1, 0, dx, 0, 1, dy, 0, 0, 1];
            },
        },
        /* Image 编码字节直通（`ImageRef::Encoded` 先例）：core 不解释载荷，
           解码归两端渲染器。句柄携带原始字节，drawImage 时进帧 bytes 表。 */
        Image: {
            MakeFromEncoded(data) {
                if (!(data instanceof Uint8Array)) {
                    throw new Error('MakeFromEncoded expects a Uint8Array of encoded (PNG/JPEG/...) bytes');
                }
                const handle = {
                    __opencatImage: true,
                    __opencatEncoded: data,
                    assetId: '',
                    width() { return 0; },
                    height() { return 0; },
                    delete() {},
                };
                handle.makeShader = function () {
                    throw new Error('an encoded image cannot be a shader child — declare the asset in JSX (ctx.getImage)');
                };
                return handle;
            },
            MakeFromImage(image) {
                return image;
            },
        },
        /* SVG path 字符串 → 官方 Path。usvg 解析（svg-align 管线），段
           命令经 `__oc_path_from_svg` 回来后组装。 */
        makePathFromSVGString(svg) {
            if (typeof __oc_path_from_svg !== 'function') {
                throw new Error('makePathFromSVGString requires the host path binding');
            }
            const flat = __oc_path_from_svg(String(svg));
            if (!flat) return null;
            const path = new Path();
            for (let i = 0; i < flat.length;) {
                const cmd = flat[i++];
                if (cmd === 0) { path.moveTo(flat[i++], flat[i++]); }
                else if (cmd === 1) { path.lineTo(flat[i++], flat[i++]); }
                else if (cmd === 2) { path.quadTo(flat[i++], flat[i++], flat[i++], flat[i++]); }
                else if (cmd === 3) { path.cubicTo(flat[i++], flat[i++], flat[i++], flat[i++], flat[i++], flat[i++]); }
                else if (cmd === 4) { path.close(); }
                else { throw new Error(`makePathFromSVGString: unknown verb ${cmd}`); }
            }
            return path;
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
                throw new Error('Shader.MakeLinearGradient is not available — gradient fills are backed by the CSS engine');
            },
            MakeRadialGradient(center, radius, colors, positions, mode, localMatrix) {
                throw new Error('Shader.MakeRadialGradient is not available — gradient fills are backed by the CSS engine');
            },
            MakeBlend(mode, dst, src) {
                throw new Error('Shader.MakeBlend is not available — gradient fills are backed by the CSS engine');
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
        /* ── Paragraph 签名面（过渡：位图贴回，批次 B 换血 cosmic-text→path）──
           g 系渲染器（@antv/g-canvaskit）经此面排版文字。度量走真字体绑定
           `__canvas_measure_paragraph`；绘制经 Canvas.drawParagraph 把文字
           光栅成帧内生成位图贴回节点 DrawOp 流（g2-para 通道）。签名与
           canvaskit-wasm 的 Paragraph 一致：layout/getMaxWidth/getHeight/
           getLineMetrics/getRectsForRange/delete。 */
        ParagraphStyle: function ParagraphStyle(style) {
            this.style = style || {};
        },
        ParagraphBuilder: {
            Make(style) {
                return new ParagraphBuilderImpl(style);
            },
        },
        TextDirection: { LTR: 0, RTL: 1 },
        TextAlign: { Left: 0, Right: 1, Center: 2, Justify: 3, Start: 4, End: 5 },
        TextBaseline: { Alphabetic: 0, Ideographic: 1 },
        RectHeightStyle: { Tight: 0, Max: 1 },
        // g-canvaskit 的文本装饰构建（TextRenderer → DecorationStyle.Solid）
        // 会读这个枚举；官方 canvaskit 的数值。
        DecorationStyle: { Solid: 0, Double: 1, Dotted: 2, Dashed: 3, Wavy: 4 },
        /* FontMgr/Typeface 是惰性签名面：字体字节由宿主 scoped fontdb 供给
           （face 查找按 family 名走 `__canvas_*` 度量/绘制绑定），管理器对象
           不携带字体。批次 B 换血为 core 文本引擎后维持同签名。 */
        FontMgr: {
            FromData() {
                return {
                    matchFamilyStyle() { return null; },
                    countFamilies() { return 0; },
                    getFamilyName() { return ''; },
                    delete() {},
                };
            },
        },
        Typeface: {
            MakeDefault() { return null; },
            MakeFreeTypeFaceFromData() { return null; },
            MakeFromName() { return null; },
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
                if (resolvedImage.__opencatEncoded) {
                    __canvas_draw_image_encoded(
                        id,
                        Array.from(resolvedImage.__opencatEncoded),
                        toFiniteNumber(x),
                        toFiniteNumber(y),
                        imagePaint.alpha,
                        imagePaint.antiAlias
                    );
                    return this;
                }
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

            /* 段落绘制（批次 B 转路径）：shape 载荷烘成一个 Path 一次
               drawPath——glyph 位置平移进 path 坐标，DrawOp 流里只剩
               path，两端各自重放（web 零文本栈）。基线 = y + ascent
               （官方 drawParagraph 的 y 是段落顶）。emoji（ColorImage）
               段落走生成图位图——Generated 是 core 宪法允许的先例。 */
            drawParagraph(paragraph, x, y) {
                if (!(paragraph instanceof ParagraphImpl)) {
                    throw new Error('drawParagraph expects a CanvasKit.Paragraph');
                }
                const text = paragraph.text;
                if (!text) return this;
                const m = paragraph._metrics;
                const color = Array.isArray(m.color) ? m.color : [0, 0, 0, 1];
                if (!m.shaped) {
                    // shape 绑定（__canvas_glyph_paths）在 core 宿主常在；
                    // 缺席 = 宿主装配损坏，loud fail 不静默回位图。
                    throw new Error('drawParagraph: glyph shaping binding unavailable');
                }
                if (m.colorOnly) {
                    __canvas_draw_paragraph(
                        id,
                        text,
                        m.size,
                        m.family,
                        Math.round(m.weight) || 400,
                        colorToCss(color),
                        toFiniteNumber(x),
                        toFiniteNumber(y),
                        m.width,
                        m.height
                    );
                    return this;
                }
                const bx = toFiniteNumber(x);
                const by = toFiniteNumber(y) + m.ascent;
                const path = new Path();
                for (const pos of m.positions) {
                    const ops = m.glyphMap[pos.k];
                    if (!ops) continue;
                    const gx = bx + pos.x;
                    const gy = by + pos.y;
                    for (const op of ops) {
                        switch (op[0]) {
                            case 0: path.moveTo(gx + op[1], gy + op[2]); break;
                            case 1: path.lineTo(gx + op[1], gy + op[2]); break;
                            case 2: path.quadTo(gx + op[1], gy + op[2], gx + op[3], gy + op[4]); break;
                            case 3: path.cubicTo(gx + op[1], gy + op[2], gx + op[3], gy + op[4], gx + op[5], gy + op[6]); break;
                            case 4: path.close(); break;
                            default: throw new Error(`drawParagraph: unknown glyph op ${op[0]}`);
                        }
                    }
                }
                const paint = new Paint();
                paint.setColor(color);
                paint.setAntiAlias(true);
                this.drawPath(path, paint);
                return this;
            },
        };
    }

    globalThis.CanvasKit = CanvasKit;
    globalThis.Path2D = Path2D;
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

    /* CSS font shorthand (css-fonts-4 §2): `[style] [variant] [weight]
       [stretch] size[/line-height] family`. TinySDF sets exactly
       "normal normal 24px sans-serif", so the leading keyword slots must be
       tolerated instead of demanding a numeric weight. Only the weight and
       size reach the rasterizer; style/variant/stretch are parsed and
       dropped (upright small-caps/stretch text is not in the surface
       subset — see the file header). */
    function parseCssFont(font) {
        const src = String(font).trim().replace(/\s+/g, ' ');
        const tokens = src.split(' ');
        let i = 0;
        let weight = 400;
        const WEIGHTS = { normal: 400, bold: 700, bolder: 700, lighter: 300 };
        while (i < tokens.length) {
            const tok = tokens[i].toLowerCase();
            if (/^(italic|oblique(\s+[-0-9.]+deg)?|small-caps|ultra-condensed|extra-condensed|condensed|semi-condensed|semi-expanded|expanded|extra-expanded|ultra-expanded)$/.test(tok)) {
                i += 1;
                continue;
            }
            if (Object.prototype.hasOwnProperty.call(WEIGHTS, tok)) {
                weight = WEIGHTS[tok];
                i += 1;
                continue;
            }
            if (/^[1-9]00$/.test(tok)) {
                weight = parseInt(tok, 10);
                i += 1;
                continue;
            }
            break;
        }
        const sizeTok = tokens[i] || '';
        const sm = /^([0-9.]+)(px|pt|em|rem|%)?(?:\/[-0-9.a-z%]+)?$/.exec(sizeTok);
        if (!sm || !tokens[i + 1]) {
            throw new Error('unsupported font shorthand: ' + font);
        }
        const UNIT = { px: 1, pt: 4 / 3, em: 16, rem: 16, '%': 0.16 };
        const size = parseFloat(sm[1]) * (UNIT[(sm[2] || 'px').toLowerCase()] || 1);
        const family = tokens.slice(i + 1).join(' ').split(',')[0].replace(/["']/g, '').trim();
        if (!family) {
            throw new Error('unsupported font shorthand: ' + font);
        }
        return { weight, size, family };
    }

    function makeSurface(id, w, h) {
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
        // fillStyle 的两种形态：CSS 颜色字符串，或 createLinearGradient 的
        // 返回值（只支持线性渐变；轴向声明在 gradient 上）。
        function makeGradient(x0, y0, x1, y1) {
            const g = {
                __opencatGradient: true,
                x0: toFiniteNumber(x0), y0: toFiniteNumber(y0),
                x1: toFiniteNumber(x1), y1: toFiniteNumber(y1),
                stops: [],
                addColorStop(offset, color) {
                    g.stops.push([toFiniteNumber(offset), String(color)]);
                    return g;
                }
            };
            return g;
        }
        function fillShape(x, y, w, h, radius) {
            const style = state.fillStyle;
            let flat = [];
            let color = '#000000';
            if (style && style.__opencatGradient) {
                for (let i = 0; i < style.stops.length; i += 1) {
                    const [off, css] = style.stops[i];
                    const rgba = normalizeColor(css);
                    flat.push(off, rgba[0], rgba[1], rgba[2], rgba[3]);
                }
            } else {
                color = colorToCss(style);
            }
            const g = style && style.__opencatGradient ? style : { x0: 0, y0: 0, x1: 1, y1: 0 };
            __surface_fill_shape(
                id,
                matrix.tx + matrix.a * toFiniteNumber(x),
                matrix.ty + toFiniteNumber(y),
                toFiniteNumber(w) * matrix.a,
                toFiniteNumber(h),
                toFiniteNumber(radius),
                g.x0, g.y0, g.x1, g.y1,
                flat,
                color
            );
        }

        return {
            __opencatSurface: true,
            id: id,
            /* three 的 CanvasTexture/上传路径读 image.width/height 推导
               texImage2D 的 6 参形态,并校验尺寸——表面创建时已知。 */
            width: w,
            height: h,
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
            /* fillRect / roundRect 都走同一个 binding：圆角半径 + 线性渐变轴 +
               色标表。fillStyle 是字符串时按纯色，是 gradient 对象时按色标。 */
            fillRect(x, y, w, h) {
                fillShape(x, y, w, h, 0);
            },
            roundRect(x, y, w, h, radius) {
                fillShape(x, y, w, h, radius);
            },
            createLinearGradient(x0, y0, x1, y1) {
                return makeGradient(x0, y0, x1, y1);
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
               已退役：效果内核只走官方 RuntimeEffect（SKSL）——像素处理
               需要逐字节访问时用 getImageData（读）与 putImageData（写）。 */
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

    /* 内部：按 id 取或建一张 surface 句柄。id 由 ctx.surface 生成（尺寸派生），
       脚本不再直接持有它。 */
    function surfaceById(id, w, h) {
        id = String(id);
        if (!surfaceCache[id]) {
            __surface_create(id, toFiniteNumber(w), toFiniteNumber(h));
            surfaceCache[id] = makeSurface(id, w, h);
        }
        return surfaceCache[id];
    }

    // ── ctx.surface: the standard offscreen 2D text surface ─────────
    // Same facade (real-font fillText / measureText / getImageData over the
    // scoped fontdb), exposed under its own name. The surface is a session
    // pixel buffer: read it with getImageData, upload it with canvas
    // putImageData — official Canvas2D semantics.
    ctx.surface = function (w, h) {
        const ww = Math.max(1, Math.round(toFiniteNumber(w)));
        const hh = Math.max(1, Math.round(toFiniteNumber(h)));
        return surfaceById('surface-' + ww + 'x' + hh, ww, hh);
    };

    // asset_api 的 createImageBitmap 垫用同一条 surface 通道：解码位图和
    // 手绘 surface 是同一种对象，texImage2D 的上传桥只认一种形态。
    ctx.__surfaceById = surfaceById;

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
    // Keep the browser's real OffscreenCanvas reachable: the WebGL recorder
    // facade below replaces the global, but the web host can run a real GL
    // context on the composition canvas (standard HTML semantics) and must
    // still be able to allocate the backing OffscreenCanvas.
    if (typeof globalThis.OffscreenCanvas === 'function' && !globalThis.__ocRealOffscreenCanvas) {
        globalThis.__ocRealOffscreenCanvas = globalThis.OffscreenCanvas;
    }
    globalThis.OffscreenCanvas = OffscreenCanvas;

    // ── Official surface entry: CK.MakeCanvasSurface(id) ─────────────
    // `canvas` accepts an id string, a useRef() element handle, or a DOM
    // handle carrying .id — the official MakeCanvasSurface signature takes a
    // canvas or an id. The returned Surface records: getCanvas() is the
    // CanvasKit Canvas facade whose draw calls land on the canvas node's
    // DrawOp stream; flush()/dispose() are no-ops (no live GPU surface).
    // Pixel-buffer style sizes error (the offscreen pixel target is
    // ctx.surface(w, h)); makeImageSnapshot / requestAnimationFrame have no
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
            /* Emscripten 堆 API：g 系渲染器销毁路径（RenderingService.destroy
               → surface.deleteLater）逐个调用；录制 surface 没有堆，no-op。 */
            deleteLater() {},
            delete() {},
            /* g-canvaskit 每帧 drawFrame 第一行调 isDeleted()——缺失会把整帧
               绘制打成静默 TypeError（2026-10-28 corpus 首跑定位）。 */
            isDeleted() {
                return false;
            },
            width() {
                return surfaceMetaNumber('width');
            },
            height() {
                return surfaceMetaNumber('height');
            },
            makeImageSnapshot() {
                throw new Error('Surface.makeImageSnapshot is not available — a recording surface has no pixels; use ctx.surface(w, h) + getImageData for pixel access');
            },
            /* g 系渲染器的 draw loop 经此入队（CanvasKit 语义：回调收到
               surface 的 canvas）。realm 的同步 rAF 队列在本次 host flush
               中排空（`__ocFlushRaf`），回调拿到本 surface 的 canvas。 */
            requestAnimationFrame(callback) {
                if (typeof globalThis.requestAnimationFrame !== 'function') {
                    throw new Error('Surface.requestAnimationFrame requires the realm frame queue');
                }
                return globalThis.requestAnimationFrame(() => {
                    callback(this.getCanvas());
                });
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
    // Path2D/fill(path)/stroke(path) 复用：context2d.js 的 fill(path)/stroke(path)
    // 接收 Path2D（同款 _ops 形态），需要这条重放通道。canvas_api 的 Path 与
    // context2d 的 Path2D 都以 `_ops` 数组为唯一载体。
    globalThis.__ocReplayPathOps = replayPath;
})();
