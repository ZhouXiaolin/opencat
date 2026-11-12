// 2D-context adapter: `node.getContext('2d')` exposes the HTML canvas 2D
// *shape* of the API, but the backing engine is NOT a browser 2D canvas — it
// is OpenCat's CanvasKit (Skia) command stream, the same stream the official
// `CanvasKitInit().then(CK => CK.MakeCanvasSurface(id))` path records into.
// State (fillStyle /
// strokeStyle / lineWidth / font…) lives in one record per context and is
// lowered per call into CK paint state + draw ops, exactly like
// `applyFillPaint` does for CanvasKit Paint objects. Text shapes through
// `__canvas_glyph_paths` (the same payload the CanvasKit Paragraph path
// eats) and bakes glyphs into a 2D path fill — the DrawOp stream only ever
// sees a path.
//
// What this buys: real packages written against `getContext('2d')` (the
// @antv/g-canvas renderer, progress rings, sparklines) run unmodified on the
// CK backend. Anything the CK stream cannot express fails loud with the
// supported alternative instead of silently diverging: gradient/pattern
// paint objects, non-source-over composite ops, effective shadows and CSS
// filters are writer-checked (the @antv/g-canvas state cache only writes a
// property when it leaves its default, so the defaults below are exactly
// the values real packages render with).
(function () {
    'use strict';

    var contexts = {};

    // `__canvas_glyph_paths` 是纯函数（text/size/family/weight → 载荷），每次
    // 调用都重新排版并整包转成 JS 对象。页脚本每帧重画同一批字符串（逐词
    // measureText 折行尤甚），按参数记忆化；溢出整表清空。
    var shapeCache = Object.create(null);
    var shapeCacheSize = 0;
    var SHAPE_CACHE_CAP = 4096;

    // 字形轮廓按排版位置摊平成相对基线原点的 flat 路径（`__canvas_path_flat`
    // 编码，Float64Array），挂在记忆化的 shape 载荷上，同一字符串逐帧复用。
    // 宿主有 typed-array 快路径（`__opencatCanvasPathFlat`）时绕开 JSON 派发。
    var FLAT_ARGS = [2, 2, 4, 6, 0];
    function pathFlat(id, flat, dx, dy) {
        if (typeof __opencatCanvasPathFlat === 'function') __opencatCanvasPathFlat(id, flat, dx, dy);
        else __canvas_path_flat(id, Array.prototype.slice.call(flat), dx, dy);
    }
    function textFlat(shaped) {
        if (shaped.flat) return shaped.flat;
        var byGlyph = {};
        var gs = shaped.glyphs || [];
        for (var i = 0; i < gs.length; i++) byGlyph[gs[i].k] = gs[i].ops;
        var positions = shaped.positions || [];
        var flat = [];
        for (var p = 0; p < positions.length; p++) {
            var ops = byGlyph[positions[p].k];
            if (!ops) continue;
            var px = positions[p].x;
            var py = positions[p].y;
            for (var j = 0; j < ops.length; j++) {
                var op = ops[j];
                var n = FLAT_ARGS[op[0]];
                if (n === undefined) throw new Error('fillText: unknown glyph op ' + op[0]);
                flat.push(op[0]);
                for (var c = 1; c <= n; c += 2) flat.push(px + op[c], py + op[c + 1]);
            }
        }
        return (shaped.flat = new Float64Array(flat));
    }

    // Fresh per-frame paint state (the canvas spec defaults).
    function freshState() {
        return {
            fillStyle: '#000000',
            strokeStyle: '#000000',
            lineWidth: 1,
            lineCap: 'butt',
            lineJoin: 'miter',
            miterLimit: 10,
            globalAlpha: 1,
            globalCompositeOperation: 'source-over',
            shadowColor: 'rgba(0, 0, 0, 0)',
            shadowBlur: 0,
            shadowOffsetX: 0,
            shadowOffsetY: 0,
            filter: 'none',
            font: '10px sans-serif',
            textAlign: 'left',
            textBaseline: 'alphabetic',
            lineDashOffset: 0,
        };
    }

    function cssColor(value) {
        if (value === undefined || value === null) return '#000000';
        var v = String(value).trim();
        return v === '' ? '#000000' : v;
    }

    // CSS font shorthand → { weight, size, family }. Token-walks the
    // canonical `<style>? <variant>? <weight>? <size>px <family>` forms real
    // packages write (`normal normal normal 12px sans-serif` included).
    var FONT_KEYWORDS = {
        normal: 1, italic: 1, oblique: 1, 'small-caps': 1, bold: 1, bolder: 1, lighter: 1,
    };
    function parseFont(font) {
        var tokens = String(font).trim().split(/\s+/);
        var weight = 400;
        var size = null;
        var familyStart = -1;
        for (var i = 0; i < tokens.length; i++) {
            var tk = tokens[i];
            if (FONT_KEYWORDS[tk]) {
                if (tk === 'bold') weight = 700;
                continue;
            }
            var numeric = /^([0-9]{1,3})$/.exec(tk);
            if (numeric) { weight = parseInt(numeric[1], 10); continue; }
            var px = /^([0-9.]+)px(\/\S+)?$/.exec(tk);
            if (px) { size = parseFloat(px[1]); familyStart = i + 1; break; }
            throw new Error(
                "context2d.font: unexpected token '" + tk + "' in '" + String(font) + "'"
            );
        }
        if (size === null || familyStart >= tokens.length) {
            throw new Error(
                "context2d.font: expected '<weight> <size>px <family>', got '" + String(font) + "'"
            );
        }
        return {
            weight: weight,
            size: size,
            family: tokens.slice(familyStart).join(',').split(',')[0].replace(/["']/g, '').trim(),
        };
    }

    function num(v) {
        var n = Number(v);
        return Number.isFinite(n) ? n : 0;
    }

    // 2D affine matrix in canvas `[a, b, c, d, e, f]` form.
    function matMul(m, n) {
        return {
            a: m.a * n.a + m.c * n.b,
            b: m.b * n.a + m.d * n.b,
            c: m.a * n.c + m.c * n.d,
            d: m.b * n.c + m.d * n.d,
            e: m.a * n.e + m.c * n.f + m.e,
            f: m.b * n.e + m.d * n.f + m.f,
        };
    }

    function matInverse(m) {
        var det = m.a * m.d - m.b * m.c;
        if (!det) throw new Error('context2d: singular transform matrix');
        return {
            a: m.d / det,
            b: -m.b / det,
            c: -m.c / det,
            d: m.a / det,
            e: (m.c * m.f - m.d * m.e) / det,
            f: (m.b * m.e - m.a * m.f) / det,
        };
    }

    // Direction is part of the path: a wedge's inner arc traverses back toward
    // the outer start, so a ccw arc lowers to a negative sweep from the same
    // start (Skia and CanvasKit arcs both take signed sweeps) — flipping it to
    // a cw region-equivalent would jump the contour.
    function normalizeSweep(start, end, counterclockwise) {
        var sweep = num(end) - num(start);
        var tau = Math.PI * 2;
        if (counterclockwise) {
            while (sweep > 0) sweep -= tau;
            while (sweep < -tau) sweep += tau;
        } else {
            while (sweep < 0) sweep += tau;
            while (sweep > tau) sweep -= tau;
        }
        return { start: num(start), sweep: sweep };
    }

    // 离屏 canvas（`document.createElement('canvas')`，`__oc_offscreen_` id）
    // 的像素化：engine 绑定 `__opencatRasterCanvasPNG(id, w, h, out)` 用 Skia
    // 离屏重放该 canvas 的 op 流 → PNG 字节。这正是用户允许的「分配 surface，
    // 最后回读为 data」——像素化属于 op，不引入新 IR 指令。结果按 id 缓存
    // （pattern 源 canvas 建一次、每帧 fillPattern 重铺，不能每次光栅化）。
    var offscreenRasterCache = Object.create(null);
    // 画布被 width/height 赋值重置时由 dom_api 调用（浏览器语义：赋值即
    // 重置，旧像素作废）——不失效会让复用画布的下一个消费者拿到上一个
    // 内容的光栅化（G2 pattern 三瓦片全串成一个的根因）。
    globalThis.__ocInvalidateOffscreenRaster = function (id) {
        delete offscreenRasterCache[id];
        // 同一时刻 __opencatOffscreenReset 清空了该画布的持久命令流。
        if (contexts[id]) contexts[id].__ocForgetSent();
    };
    function rasterizeOffscreenCanvas(canvas) {
        var cached = offscreenRasterCache[canvas.__ocOffscreenId];
        if (cached) return cached;
        if (typeof __opencatRasterCanvasPNG !== 'function') {
            throw new Error('rasterizeOffscreenCanvas: engine native __opencatRasterCanvasPNG unavailable');
        }
        var w = Math.max(1, Math.ceil(canvas.width > 0 ? canvas.width : 1));
        var h = Math.max(1, Math.ceil(canvas.height > 0 ? canvas.height : 1));
        // PNG 上界：RGBA + 每行 filter 字节 + zlib stored 块开销 + 容器固定头尾
        var out = new Uint8Array(w * h * 4 + w + h + 4096);
        var len = __opencatRasterCanvasPNG(canvas.__ocOffscreenId, w, h, out);
        var record = {
            bytes: Array.from(out.subarray(0, len)),
            width: w,
            height: h,
        };
        offscreenRasterCache[canvas.__ocOffscreenId] = record;
        return record;
    }

    function makeContext2d(id) {
        var state = freshState();
        var dash = [];
        // putImageData 的帧唯一键序号（同 id 多次直写像素，见 putImageData）。
        var putImageSeq = 0;
        // 基 origin：canvas 元素在宿主容器里的块级流位置（如 rect-ring
        // 的图表画布被前序 <button> 推到 (0,21)）。浏览器里画布表面合成
        // 在元素盒上，表面自身的 (0,0) 就是这里——每帧在矩阵重置后重录
        // 一次 translate，让表面坐标系落回基 origin；setTransform 的
        // identity 同样回落到它，等价浏览器 surface 语义。
        var baseOrigin = null;
        // 当前路径点（arcTo 需要它求切点；其余路径方法只负责更新）。
        var cur = { x: 0, y: 0, known: false };
        var matrix = { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 };
        var matrixStack = [];
        var stateStack = [];
        // 当前路径的保守包围盒（用户空间）。只有图案填充要它：瓦片必须铺满
        // 整条路径，而流里没有 pattern 对象，core 只能按包围盒铺瓦片。曲线
        // 取控制点、圆弧取整圆——宁大勿小。
        var pathBounds = null;
        // 本 canvas 命令流里最近一次下发的 sticky 绘制状态（aa/alpha/fill/
        // shadow/blend/filter；缺键=未发）。执行端 restore 不回退这些状态，
        // 镜像因此不随 save/restore 回卷——回卷会让 save 块内的下发被快照
        // 覆盖、后续同值绘制被跳过，后端卡在旧值上（blend 探针 multiply 瓦片
        // 实证）。命令流清空时作废（`__ocForgetSent`）。同值跳过下发：
        // halftone 语料每帧 ~30 万次 fill，aa/alpha/shadow/blend/filter 全同值。
        var sent = {};
        // 挂起的路径头：beginPath 与紧随其后的空子路径满圈 arc 先不发，fill()
        // 撞上时合成一次 `__canvas_fill_oval_rect`（op 序列仍是 BeginPath+
        // AddOval+FillPath，跨界调用 3→1）。其余触碰路径或变换的调用先
        // flushPath()，流序与逐条下发一致。
        var beginPending = false;
        var ovalPending = null;

        // 满圈点的批量积攒面：快路 fill 不再逐点跨界，矩形/色进批数组，
        // flush 时一次 `__ocCanvasFillOvalBatch`（TypedArray 二进制通道，
        // 落流与逐点同序同参，位图逐位一致）。halftone 语料每帧 ~30 万点，
        // 每点两次 Value 化跨界是毫秒级热点；批把跨界压到 4096 点一次。
        // 色解析只认 rgba()/#hex（受限模板，语料实际面）；解析失败的点
        // 退回逐点快路（同语义，只慢）。flush 时机 = 任何触碰流序/状态的
        // 调用（flushPath/send*/绘制类/save/restore/变换）+ 帧尾
        // （`__ocFlushCanvasBatches`，批点必须进本帧 op 流）。
        var batchRects = null;
        var batchColors = null;
        var batchN = 0;
        var batchFillLast = '';
        var BATCH_CAP = 4096;

        function batchFlush() {
            if (!batchN) return;
            var n = batchN;
            batchN = 0;
            // 状态面与逐点路径的 applyFill 同形（aa/alpha/composite/shadow，
            // 镜像同值跳过）——批展开 op 之前先落状态，流序与逐点一致。
            // 先清计数再走 send*（它们开头会 batchFlush，已幂等）；buffer
            // 复用不释放（halftone 每帧 ~74 次 flush，跨界同步拷走后才回填）。
            if (!sent.aa) { sent.aa = true; __canvas_set_anti_alias(id, true); }
            sendAlpha(num(state.globalAlpha));
            lowerComposite(id, state, sent);
            lowerShadow(id, state, sent);
            __ocCanvasFillOvalBatch(
                id,
                batchRects.subarray(0, n * 4),
                batchColors.subarray(0, n * 4),
                n
            );
            sent.fill = batchFillLast;
        }

        function batchDiscard() {
            batchRects = null;
            batchColors = null;
            batchN = 0;
        }

        // 受限色解析 → [r,g,b,a]，不认识的格式返回 null（调用方退回逐点）。
        // rgba() 走手写字符扫描（halftone 每点一次，正则 exec 是微秒级、
        // 30 万次/帧会吃掉两百毫秒）；hex 走 parseInt。
        function batchColor(v) {
            if (v === undefined || v === null) return null;
            var s = String(v).trim();
            var c0 = s.charCodeAt(0);
            if (c0 === 35) { // '#'
                var hex = s.length === 4
                    ? s[1] + s[1] + s[2] + s[2] + s[3] + s[3]
                    : (s.length === 7 || s.length === 9 ? s.slice(1, 7) : null);
                if (!hex) return null;
                var h = parseInt(hex, 16);
                if (!(h >= 0)) return null;
                var a = s.length === 9 ? parseInt(s.slice(7, 9), 16) : 255;
                if (!(a >= 0)) return null;
                return [(h >> 16) & 255, (h >> 8) & 255, h & 255, a];
            }
            if (c0 === 114 && s.length > 10 && s.charCodeAt(1) === 103
                && s.charCodeAt(2) === 98 && s.charCodeAt(3) === 97
                && s.charCodeAt(4) === 40) { // rgba(
                var out = [0, 0, 0, 0];
                var p = 5;
                var n = s.length;
                for (var k = 0; k < 4; k++) {
                    while (p < n && (s.charCodeAt(p) === 32 || s.charCodeAt(p) === 9)) p++;
                    var start = p;
                    var seenDot = false;
                    while (p < n) {
                        var ch = s.charCodeAt(p);
                        if (ch === (k < 3 ? 44 : 41)) break; // ',' or ')'
                        if (ch >= 48 && ch <= 57) { p++; continue; }
                        if (ch === 46 && !seenDot) { seenDot = true; p++; continue; }
                        return null;
                    }
                    if (p === start || p >= n) return null;
                    var fv = parseFloat(s.slice(start, p));
                    if (!(fv >= 0)) return null;
                    out[k] = k < 3
                        ? (fv > 255 ? 255 : Math.round(fv))
                        : (fv > 1 ? 255 : Math.round(fv * 255));
                    p++;
                }
                if (p !== n) return null;
                return out;
            }
            return null;
        }

        function batchPush(o, fillStr, rgba) {
            if (!batchRects) {
                batchRects = new Float32Array(BATCH_CAP * 4);
                batchColors = new Uint8Array(BATCH_CAP * 4);
            }
            if (batchN >= BATCH_CAP) batchFlush();
            var b4 = batchN * 4;
            batchRects[b4] = o.x;
            batchRects[b4 + 1] = o.y;
            batchRects[b4 + 2] = o.w;
            batchRects[b4 + 3] = o.h;
            batchColors[b4] = rgba[0];
            batchColors[b4 + 1] = rgba[1];
            batchColors[b4 + 2] = rgba[2];
            batchColors[b4 + 3] = rgba[3];
            batchN++;
            batchFillLast = fillStr;
        }

        function flushPath() {
            batchFlush();
            if (beginPending) {
                beginPending = false;
                __canvas_begin_path(id);
            }
            if (ovalPending) {
                var o = ovalPending;
                ovalPending = null;
                __canvas_path_add_oval(id, o.x, o.y, o.w, o.h);
            }
        }

        function sendAlpha(alpha) {
            batchFlush();
            if (sent.alpha === alpha) return;
            sent.alpha = alpha;
            __canvas_set_global_alpha(id, alpha);
        }

        function sendFill(color) {
            batchFlush();
            if (sent.fill === color) return;
            sent.fill = color;
            __canvas_set_fill_style(id, color);
        }

        function expandPathBounds(x0, y0, x1, y1) {
            if (!(x0 <= x1) || !(y0 <= y1)) return;
            if (!pathBounds) {
                pathBounds = { x0: x0, y0: y0, x1: x1, y1: y1 };
                return;
            }
            if (x0 < pathBounds.x0) pathBounds.x0 = x0;
            if (y0 < pathBounds.y0) pathBounds.y0 = y0;
            if (x1 > pathBounds.x1) pathBounds.x1 = x1;
            if (y1 > pathBounds.y1) pathBounds.y1 = y1;
        }

        function lowerDash() {
            if (dash.length) __canvas_set_line_dash(id, dash, num(state.lineDashOffset));
            else __canvas_clear_line_dash(id);
        }

        function lowerGradientPaint(stroke, paint) {
            if (paint.__ocRadial) {
                __canvas_set_style_radial_gradient(
                    id, stroke, paint.x0, paint.y0, paint.r0, paint.x1, paint.y1, paint.r1,
                    paint.offsets, paint.colors
                );
            } else {
                __canvas_set_style_gradient(
                    id, stroke, paint.x0, paint.y0, paint.x1, paint.y1, paint.offsets, paint.colors
                );
            }
        }

        function applyStroke() {
            batchFlush();
            if (!sent.aa) { sent.aa = true; __canvas_set_anti_alias(id, true); }
            if (state.strokeStyle && state.strokeStyle.__ocGradient) {
                lowerGradientPaint(true, state.strokeStyle);
            } else {
                __canvas_set_stroke_style(id, cssColor(state.strokeStyle));
            }
            __canvas_set_line_width(id, num(state.lineWidth));
            __canvas_set_line_cap(id, state.lineCap);
            __canvas_set_line_join(id, state.lineJoin);
            lowerDash();
            // 执行端的 alpha 是持久侧状态：按下发镜像比较（不是按 JS state），
            // 上一个 save/restore 组里的 globalAlpha 不会泄漏到后续绘制。
            sendAlpha(num(state.globalAlpha));
            lowerComposite(id, state, sent);
            lowerShadow(id, state, sent);
        }

        function applyFill() {
            batchFlush();
            if (!sent.aa) { sent.aa = true; __canvas_set_anti_alias(id, true); }
            if (state.fillStyle && state.fillStyle.__ocGradient) {
                lowerGradientPaint(false, state.fillStyle);
                // 渐变 shader 盖在填充漆上：下一次纯色必须重发（SetFillStyle
                // 才清 shader）。
                delete sent.fill;
            } else {
                sendFill(cssColor(state.fillStyle));
            }
            sendAlpha(num(state.globalAlpha));
            lowerComposite(id, state, sent);
            lowerShadow(id, state, sent);
        }

        // Arc 下沉到流的 arc op（oval rect + 带符号 sweep；force_move_to 语义
        // 由执行端 arc_to(force_move_to=false) 承担）。空子路径上的 arc 自起新
        // 子路径（canvas 语义）：先 moveTo 弧起点给执行端一个轮廓起点，否则
        // arc_to 无线可续、几何消失。弧落点记回 cur（arcTo 接在 arc 后可续）。
        // 满圈（|sweep|>=2π，canvas 规范钳为整圆）拆两个半弧链接——Skia 的
        // arc_to 在 |sweep|>=2π 产空/回绕几何（arcsweep 探针实测）。
        function lowerArc(cx, cy, rx, ry, s) {
            var sweep = s.sweep;
            var full = Math.abs(sweep) >= Math.PI * 2 - 1e-6;
            if (!cur.known && full) {
                // 空子路径上的满圈 arc：整圆 = add_oval（closed，fill/stroke
                // 等价，1 op）。Skia 的 arc_to 在 |sweep|>=2π 产空/回绕几何
                // （arcsweep 探针实测），半弧链每点 3 op——halftone 语料
                // 7 万点/帧全走这条，必须走快路。紧跟 beginPath 时挂起，
                // 留给 fill() 合成。
                var oval = { x: cx - rx, y: cy - ry, w: rx * 2, h: ry * 2 };
                if (beginPending) {
                    ovalPending = oval;
                } else {
                    __canvas_path_add_oval(id, oval.x, oval.y, oval.w, oval.h);
                }
                var end = s.start + s.sweep;
                cur = { x: cx + Math.cos(end) * rx, y: cy + Math.sin(end) * ry, known: true };
                return;
            }
            flushPath();
            if (!cur.known) {
                var sx = cx + Math.cos(s.start) * rx;
                var sy = cy + Math.sin(s.start) * ry;
                expandPathBounds(sx, sy, sx, sy);
                cur = { x: sx, y: sy, known: true };
                __canvas_move_to(id, sx, sy);
            }
            if (full) {
                var half = sweep > 0 ? Math.PI : -Math.PI;
                __canvas_path_add_arc(id, cx - rx, cy - ry, rx * 2, ry * 2, s.start, half);
                __canvas_path_add_arc(id, cx - rx, cy - ry, rx * 2, ry * 2, s.start + half, half);
            } else {
                __canvas_path_add_arc(id, cx - rx, cy - ry, rx * 2, ry * 2, s.start, sweep);
            }
            var end = s.start + s.sweep;
            cur = { x: cx + Math.cos(end) * rx, y: cy + Math.sin(end) * ry, known: true };
        }

        // 图案填充。浏览器里 `fillStyle = createPattern(图, 'repeat')` 是「按
        // 图案坐标平铺瓦片图并裁到当前路径」；Skia 里没有 canvas pattern 这个
        // 对象，展开成标准 op 序列：save → clip(当前路径) → concat(图案矩阵)
        // → 包围盒内的每块瓦片 drawImage → restore。瓦片是普通 ImageRect，
        // 不引入新 op。
        function fillPattern(paint) {
            var b = pathBounds;
            if (!b) return;
            var m = paint.mat;
            var det = m.a * m.d - m.b * m.c;
            if (!det) return;
            // 图案空间 = 用户空间经 inv(m)：包围盒四角映过去取包围盒，瓦片
            // 按下标铺，纹理原点因此固定在图样自身 (0,0)。
            var corners = [[b.x0, b.y0], [b.x1, b.y0], [b.x0, b.y1], [b.x1, b.y1]];
            var u0 = Infinity, v0 = Infinity, u1 = -Infinity, v1 = -Infinity;
            for (var k = 0; k < 4; k++) {
                var px = corners[k][0] - m.e;
                var py = corners[k][1] - m.f;
                var u = (m.d * px - m.c * py) / det;
                var v = (m.a * py - m.b * px) / det;
                if (u < u0) u0 = u;
                if (u > u1) u1 = u;
                if (v < v0) v0 = v;
                if (v > v1) v1 = v;
            }
            var w = paint.w, h = paint.h;
            var alpha = num(state.globalAlpha);
            // pattern 分支在 applyFill 之前返回，透明度得自己下发（ScriptImage
            // 展开成 ImageRect 时沿用执行端 current_alpha）。
            sendAlpha(alpha);
            // 浏览器把 shadow 作用在「图案 × 路径」的整体 alpha 上，阴影不能
            // 挂在单次绘制上。图案不透明时轮廓就等于路径本身：用不透明黑漆把
            // 路径填一遍，只给滤镜提供 alpha 轮廓；层内内容被 keep_content=
            // false 丢弃，不会和瓦片叠出双重 AA 暗环。
            if (num(state.shadowBlur) > 0
                || num(state.shadowOffsetX) !== 0
                || num(state.shadowOffsetY) !== 0) {
                sendFill('#000');
                lowerShadow(id, state, sent);
                __canvas_begin_shadow_layer(id);
                // 层内填色不能再叠一次阴影，否则轮廓被模糊两次。
                sendShadow(id, sent, 'rgba(0, 0, 0, 0)', 0, 0, 0);
                __canvas_fill_path(id);
                __canvas_restore(id);
                __canvas_restore(id);
            }
            __canvas_save(id);
            __canvas_clip_path(id, true);
            __canvas_concat(id, [m.a, m.b, 0, m.c, m.d, 0, m.e, m.f, 1]);
            for (var i = Math.floor(u0 / w); i <= Math.floor(u1 / w); i++) {
                for (var j = Math.floor(v0 / h); j <= Math.floor(v1 / h); j++) {
                    // 参数顺序与 canvas.drawImage(img, sx, sy, sw, sh, dx, dy, dw, dh)
                    // 不同：绑定层收 (dst, src)，即 [dx, dy, dw, dh, sx, sy, sw, sh]。
                    // alpha 传 1：透明度已由上方 __canvas_set_global_alpha 进执行端
                    // current_alpha，drawImage 的 alpha 快照会再发一道 SrcOver α
                    // 漆——两道相乘 = ga²，高频 pattern 纹理对比度塌掉（custom-
                    // pattern 两案 0.027→0.100/0.204 的实证）。主画布的 ga 靠
                    // current_alpha 乘一次即可；离屏画布（heatmap 逐点图章）的
                    // globalAlpha 不进 current_alpha，仍靠 op 自带 alpha 漆。
                    __canvas_draw_image_bytes(
                        id, paint.bytes, [i * w, j * h, w, h, 0, 0, w, h], 1, true
                    );
                }
            }
            __canvas_restore(id);
        }

        function setMatrix(next) {
            // Stream ops compose on the right (`CTM · M`), so replacing the
            // matrix lowers to concatenating `inv(cur) · next`.
            flushPath();
            var delta = matMul(matInverse(matrix), next);
            __canvas_concat(id, [delta.a, delta.b, 0, delta.c, delta.d, 0, delta.e, delta.f, 1]);
            matrix = next;
        }

        return {
            state: state,

            // 批 flush 手柄（`__ocFlushCanvasBatches` 帧尾遍历用）。
            __flushBatch: batchFlush,

            // Per-frame reset: the executor's CTM starts at identity every
            // frame, so the JS-tracked matrix must too — the first absolute
            // `setTransform` of a frame otherwise lowers to a delta against
            // the stale matrix and translates the whole drawing. Paint state
            // resets with it. Mutates in place: the property descriptors on
            // the exported context close over this exact `state` object.
            __ocResetFrameState() {
                var fresh = freshState();
                for (var k in fresh) state[k] = fresh[k];
                dash.length = 0;
                matrix = { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 };
                matrixStack.length = 0;
                stateStack.length = 0;
                this.__ocForgetSent();
                // 基 origin 重录：executor CTM 每帧从 identity 开始，只在
                // 设置时 translate 一次第二帧就丢。JS `matrix` 仍保持相对
                // 基的 identity——后续 setMatrix 的 delta = inverse(prev)×next
                // 拼上去恒等于 T_base×matrix，即 setTransform/resetTransform
                // 回落到表面原点（基 origin），等价浏览器 surface 语义。
                if (baseOrigin && (baseOrigin.x || baseOrigin.y)) {
                    __canvas_translate(id, baseOrigin.x, baseOrigin.y);
                }
            },

            // HTML 规范：canvas 的 width/height 赋值（同值也算）清空位图并
            // 重置上下文状态。本帧已录命令整体作废，不再进光栅化。
            __ocResetBitmap() {
                __canvas_reset(id);
                cur = { x: 0, y: 0, known: false };
                pathBounds = null;
                this.__ocResetFrameState();
            },

            // 本 canvas 的命令流被清空（帧首 reset_for_frame / 画布重置）：
            // 下发镜像与挂起的路径头随流作废。未 flush 的批点属于已清掉的
            // 流，一并作废（帧尾 flush 在此之前已把批落进本帧流）。
            __ocForgetSent() {
                sent = {};
                beginPending = false;
                ovalPending = null;
                batchDiscard();
            },

            // dom_api 在拿到录制面后按流布局位置设置（一次即可，之后每帧
            // 自动重录）。幂等：重复调用以首次为准，防二次 translate。
            __ocSetBaseOrigin(x, y) {
                if (baseOrigin) return;
                var nx = num(x), ny = num(y);
                if (!nx && !ny) return;
                baseOrigin = { x: nx, y: ny };
                flushPath();
                __canvas_translate(id, nx, ny);
            },

            // ── transforms ──
            // save/restore snapshot the FULL paint state, not just the
            // matrix: callers (g-canvas) read properties back through the
            // getters after restore and draw text with
            // `globalAlpha *= opacity` — a stale JS-side alpha multiplies
            // into every following draw.
            save() {
                flushPath();
                matrixStack.push(matrix);
                var snap = Object.assign(freshState(), state);
                snap.__dash = dash.slice();
                stateStack.push(snap);
                __canvas_save(id);
            },
            restore() {
                flushPath();
                matrix = matrixStack.pop() || matrix;
                var snap = stateStack.pop();
                if (snap) {
                    var savedDash = snap.__dash;
                    delete snap.__dash;
                    for (var k in snap) state[k] = snap[k];
                    dash.length = 0;
                    for (var i = 0; i < savedDash.length; i++) dash.push(savedDash[i]);
                }
                __canvas_restore(id);
            },
            translate(x, y) {
                flushPath();
                matrix = matMul(matrix, { a: 1, b: 0, c: 0, d: 1, e: num(x), f: num(y) });
                __canvas_translate(id, num(x), num(y));
            },
            rotate(radians) {
                flushPath();
                var cos = Math.cos(num(radians));
                var sin = Math.sin(num(radians));
                matrix = matMul(matrix, { a: cos, b: sin, c: -sin, d: cos, e: 0, f: 0 });
                __canvas_rotate(id, num(radians) * 180 / Math.PI);
            },
            scale(sx, sy) {
                flushPath();
                matrix = matMul(matrix, { a: num(sx), b: 0, c: 0, d: num(sy), e: 0, f: 0 });
                __canvas_scale(id, num(sx), num(sy));
            },
            // g-canvas touches these on every frame entry (its reset branch
            // probes resetTransform and falls back to setTransform), so both
            // must exist even though offline rendering keeps the CTM identity.
            setTransform(a, b, c, d, e, f) {
                setMatrix({ a: num(a), b: num(b), c: num(c), d: num(d), e: num(e), f: num(f) });
            },
            resetTransform() {
                setMatrix({ a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 });
            },
            getTransform() {
                return {
                    a: matrix.a, b: matrix.b, c: matrix.c,
                    d: matrix.d, e: matrix.e, f: matrix.f,
                    isIdentity: matrix.a === 1 && matrix.b === 0 && matrix.c === 0
                        && matrix.d === 1 && matrix.e === 0 && matrix.f === 0,
                };
            },

            // ── rects ──
            // Browsers *replace* the rect's pixels, and Skia's Clear is a
            // Src-blend paint under the current clip — so the regional form
            // is clip + clear, no bespoke op needed. Zero/negative area
            // clears nothing (HTML spec): TinySDF wipes its atlas with
            // `clearRect(3, 3, 0, 0)` before it knows the glyph box.
            clearRect(x, y, w, h) {
                if (!(num(w) > 0) || !(num(h) > 0)) return;
                flushPath();
                __canvas_save(id);
                __canvas_clip_rect(id, num(x), num(y), num(w), num(h), false);
                __canvas_clear(id, null);
                __canvas_restore(id);
            },
            fillRect(x, y, w, h) {
                flushPath();
                if (state.fillStyle && state.fillStyle.__ocPattern) {
                    // fillRect 不动调用方的当前路径：借用一条临时矩形路径
                    // 做裁剪，包围盒也临时替换，画完还原。
                    var savedBounds = pathBounds;
                    __canvas_begin_path(id);
                    __canvas_path_add_rect(id, num(x), num(y), num(w), num(h));
                    pathBounds = {
                        x0: num(x), y0: num(y),
                        x1: num(x) + num(w), y1: num(y) + num(h),
                    };
                    fillPattern(state.fillStyle);
                    pathBounds = savedBounds;
                    return;
                }
                if (state.fillStyle && state.fillStyle.__ocGradient) {
                    // Gradient fillRect lowers to the path surface: FillRect
                    // carries its own solid color, while FillPath consumes the
                    // current (gradient) paint.
                    applyFill();
                    __canvas_begin_path(id);
                    __canvas_path_add_rect(id, num(x), num(y), num(w), num(h));
                    __canvas_fill_path(id);
                    return;
                }
                applyFill();
                __canvas_fill_rect(id, num(x), num(y), num(w), num(h), cssColor(state.fillStyle));
            },
            strokeRect(x, y, w, h) {
                flushPath();
                if (state.strokeStyle && state.strokeStyle.__ocGradient) {
                    applyStroke();
                    __canvas_begin_path(id);
                    __canvas_path_add_rect(id, num(x), num(y), num(w), num(h));
                    __canvas_stroke_path(id);
                    return;
                }
                applyStroke();
                __canvas_stroke_rect(id, num(x), num(y), num(w), num(h), cssColor(state.strokeStyle), num(state.lineWidth));
            },

            // 位图绘制：3 参（默认自然尺寸）/ 5 参（缩放）/ 9 参（源矩形）。
            // 编码字节随 op 一起进记录器，由 execute_draw_op 内联进帧字节表。
            drawImage(image, ...args) {
                batchFlush();
                var bytes, nw, nh;
                if (image && image.__ocImageArray) {
                    bytes = image.__ocImageArray;
                    nw = num(image.naturalWidth || image.width) || 0;
                    nh = num(image.naturalHeight || image.height) || 0;
                } else if (image && image.__ocOffscreenCanvas) {
                    // 离屏 canvas 源：脚本期 Skia 光栅化 → PNG 字节 → 标准
                    // ImageRect（Encoded）。这正是用户允许的「分配 surface，
                    // 最后回读为 data」op。
                    var rasterized = rasterizeOffscreenCanvas(image);
                    bytes = rasterized.bytes;
                    nw = rasterized.width;
                    nh = rasterized.height;
                } else {
                    throw new Error('drawImage: only decoded Image or offscreen canvas sources are supported');
                }
                let values;
                if (args.length === 2) {
                    values = [num(args[0]), num(args[1]), nw, nh];
                } else if (args.length === 4) {
                    values = [num(args[0]), num(args[1]), num(args[2]), num(args[3])];
                } else if (args.length === 8) {
                    values = [
                        num(args[4]), num(args[5]), num(args[6]), num(args[7]),
                        num(args[0]), num(args[1]), num(args[2]), num(args[3]),
                    ];
                } else {
                    throw new Error('drawImage: expected 3, 5 or 9 arguments, got ' + (args.length + 1));
                }
                __canvas_draw_image_bytes(id, bytes, values, num(state.globalAlpha), true);
            },

            // ── paths ──
            beginPath() {
                // 批点横跨 beginPath 安全（每点几何+色在积攒时已冻结）：
                // 这里只落挂起路径头（语义同 flushPath，但不 flush 批——
                // halftone 每点 beginPath，flushPath 会把批冲成每点一次）。
                if (beginPending) {
                    beginPending = false;
                    __canvas_begin_path(id);
                }
                if (ovalPending) {
                    var o0 = ovalPending;
                    ovalPending = null;
                    __canvas_path_add_oval(id, o0.x, o0.y, o0.w, o0.h);
                }
                pathBounds = null;
                cur = { x: 0, y: 0, known: false };
                beginPending = true;
            },
            // HTML 2D dash surface over the same `dash` the stroke lowering
            // consumes (`lowerDash`); [] restores the solid line.
            setLineDash(segments) {
                dash = Array.isArray(segments) ? segments.map(num) : [];
                lowerDash();
            },
            getLineDash() { return dash.slice(); },
            // Linear gradient paint. Coordinates are captured in the current
            // transform space like the browser (the stream's CTM applies on
            // top); stops go out as (offset, css color) pairs and get parsed
            // on the Rust side.
            createLinearGradient(x0, y0, x1, y1) {
                var offsets = [];
                var colors = [];
                return {
                    __ocGradient: true,
                    x0: num(x0), y0: num(y0), x1: num(x1), y1: num(y1),
                    offsets: offsets,
                    colors: colors,
                    addColorStop(offset, color) {
                        var off = num(offset);
                        if (!(off >= 0 && off <= 1)) {
                            throw new Error('addColorStop: offset must be in [0, 1], got ' + offset);
                        }
                        offsets.push(off);
                        colors.push(String(color));
                    },
                };
            },
            // Radial gradient paint: two circles (x0,y0,r0)/(x1,y1,r1), same
            // stop lowering as the linear form.
            createRadialGradient(x0, y0, r0, x1, y1, r1) {
                var offsets = [];
                var colors = [];
                return {
                    __ocGradient: true,
                    __ocRadial: true,
                    x0: num(x0), y0: num(y0), r0: num(r0),
                    x1: num(x1), y1: num(y1), r1: num(r1),
                    offsets: offsets,
                    colors: colors,
                    addColorStop(offset, color) {
                        var off = num(offset);
                        if (!(off >= 0 && off <= 1)) {
                            throw new Error('addColorStop: offset must be in [0, 1], got ' + offset);
                        }
                        offsets.push(off);
                        colors.push(String(color));
                    },
                };
            },
            // 图案画刷：解码 Image 源或离屏 canvas 源（脚本期 Skia 光栅化 →
            // PNG 字节）。`setTransform` 记下图案矩阵，填充时由 fillPattern 用。
            createPattern(source, repetition) {
                var bytes, w, h;
                if (source && source.__ocImageArray) {
                    bytes = source.__ocImageArray;
                    w = num(source.naturalWidth || source.width) || 0;
                    h = num(source.naturalHeight || source.height) || 0;
                } else if (source && source.__ocOffscreenCanvas) {
                    var rasterized = rasterizeOffscreenCanvas(source);
                    bytes = rasterized.bytes;
                    w = rasterized.width;
                    h = rasterized.height;
                } else {
                    throw new Error('createPattern: only decoded Image or offscreen canvas sources are supported');
                }
                if (!(w > 0 && h > 0)) throw new Error('createPattern: empty source image');
                var tile = String(repetition === undefined ? 'repeat' : repetition);
                if (tile !== 'repeat') {
                    throw new Error(
                        "createPattern: only repetition 'repeat' is supported, got '" + tile + "'"
                    );
                }
                return {
                    __ocPattern: true,
                    bytes: bytes,
                    w: w,
                    h: h,
                    mat: { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 },
                    setTransform(m) {
                        if (!m) return;
                        this.mat = {
                            a: num(m.a), b: num(m.b), c: num(m.c),
                            d: num(m.d), e: num(m.e), f: num(m.f),
                        };
                    },
                };
            },
            closePath() { flushPath(); __canvas_close_path(id); },
            moveTo(x, y) {
                flushPath();
                expandPathBounds(num(x), num(y), num(x), num(y));
                cur = { x: num(x), y: num(y), known: true };
                __canvas_move_to(id, num(x), num(y));
            },
            lineTo(x, y) {
                flushPath();
                expandPathBounds(num(x), num(y), num(x), num(y));
                cur = { x: num(x), y: num(y), known: true };
                __canvas_line_to(id, num(x), num(y));
            },
            // 流里只有弧段没有 arcTo：按 canvas 规范求切点，降级成
            // lineTo(切点) + arc(圆心, 起止角)。圆角矩形全靠它。
            arcTo(x1, y1, x2, y2, radius) {
                var r = num(radius);
                var ax = num(x1), ay = num(y1), bx = num(x2), by = num(y2);
                if (!(r > 0) || !cur.known
                    || (cur.x === ax && cur.y === ay) || (ax === bx && ay === by)) {
                    this.lineTo(ax, ay);
                    return;
                }
                var d1x = cur.x - ax, d1y = cur.y - ay;
                var d2x = bx - ax, d2y = by - ay;
                var l1 = Math.hypot(d1x, d1y), l2 = Math.hypot(d2x, d2y);
                if (!(l1 > 0) || !(l2 > 0)) {
                    this.lineTo(ax, ay);
                    return;
                }
                d1x /= l1; d1y /= l1; d2x /= l2; d2y /= l2;
                var cross = d1x * d2y - d1y * d2x;
                var angle = Math.acos(Math.max(-1, Math.min(1, d1x * d2x + d1y * d2y)));
                if (!(Math.abs(cross) > 1e-9) || !(angle > 1e-9)) {
                    this.lineTo(ax, ay);
                    return;
                }
                var tanHalf = Math.tan(angle / 2);
                var dist = r / tanHalf;
                var t1x = ax + d1x * dist, t1y = ay + d1y * dist;
                var t2x = ax + d2x * dist, t2y = ay + d2y * dist;
                var bisx = d1x + d2x, bisy = d1y + d2y;
                var bl = Math.hypot(bisx, bisy);
                if (!(bl > 0)) {
                    this.lineTo(ax, ay);
                    return;
                }
                var h = r / Math.sin(angle / 2);
                var cx0 = ax + (bisx / bl) * h, cy0 = ay + (bisy / bl) * h;
                this.lineTo(t1x, t1y);
                // 切点间取短弧：y 向下坐标里 cross<0 的拐角，切点角递增
                //（顺时针）才是短弧，counterclockwise 取 cross>0。
                this.arc(
                    cx0, cy0, r,
                    Math.atan2(t1y - cy0, t1x - cx0),
                    Math.atan2(t2y - cy0, t2x - cx0),
                    cross > 0
                );
                cur = { x: t2x, y: t2y, known: true };
            },
            rect(x, y, w, h) {
                flushPath();
                expandPathBounds(num(x), num(y), num(x) + num(w), num(y) + num(h));
                cur = { x: num(x), y: num(y), known: true };
                __canvas_path_add_rect(id, num(x), num(y), num(w), num(h));
            },
            quadraticCurveTo(cpx, cpy, x, y) {
                flushPath();
                expandPathBounds(
                    Math.min(num(cpx), num(x)), Math.min(num(cpy), num(y)),
                    Math.max(num(cpx), num(x)), Math.max(num(cpy), num(y))
                );
                __canvas_quad_to(id, num(cpx), num(cpy), num(x), num(y));
            },
            bezierCurveTo(c1x, c1y, c2x, c2y, x, y) {
                flushPath();
                expandPathBounds(
                    Math.min(num(c1x), num(c2x), num(x)), Math.min(num(c1y), num(c2y), num(y)),
                    Math.max(num(c1x), num(c2x), num(x)), Math.max(num(c1y), num(c2y), num(y))
                );
                __canvas_cubic_to(id, num(c1x), num(c1y), num(c2x), num(c2y), num(x), num(y));
            },
            arc(x, y, radius, startAngle, endAngle, counterclockwise) {
                expandPathBounds(
                    num(x) - num(radius), num(y) - num(radius),
                    num(x) + num(radius), num(y) + num(radius)
                );
                var s = normalizeSweep(startAngle, endAngle, !!counterclockwise);
                lowerArc(num(x), num(y), num(radius), num(radius), s);
            },
            // Ellipses lower onto the stream's arc op with an oval rect;
            // x-axis rotation would need a save/rotate/restore dance that the
            // caller can express itself, so it fails loud.
            ellipse(x, y, radiusX, radiusY, xRotation, startAngle, endAngle, counterclockwise) {
                if (num(xRotation) !== 0) {
                    throw new Error('ellipse: rotated ellipses are not supported');
                }
                expandPathBounds(
                    num(x) - num(radiusX), num(y) - num(radiusY),
                    num(x) + num(radiusX), num(y) + num(radiusY)
                );
                var s = normalizeSweep(startAngle, endAngle, !!counterclockwise);
                lowerArc(num(x), num(y), num(radiusX), num(radiusY), s);
            },
            clip() {
                flushPath();
                __canvas_clip_path(id, true);
            },
            fill(path) {
                if (ovalPending && !(path && path._ops)
                    && !(state.fillStyle && state.fillStyle.__ocPattern)) {
                    // beginPath + 满圈 arc + fill：色可受限解析时进批（不跨
                    // 界，flush 时一次落同一条 op 序列）；解析失败退回逐点
                    // 快路（一次跨界录 BeginPath+AddOval+FillPath，与逐条
                    // 下发同参同序；fill 不消耗路径，执行端当前路径仍是
                    // 这个圆，后续 stroke() 照描）。
                    var o = ovalPending;
                    beginPending = false;
                    ovalPending = null;
                    var fillStr = cssColor(state.fillStyle);
                    var rgba = batchColor(fillStr);
                    if (rgba) {
                        batchPush(o, fillStr, rgba);
                        return;
                    }
                    batchFlush();
                    applyFill();
                    __canvas_fill_oval_rect(id, o.x, o.y, o.w, o.h);
                    return;
                }
                flushPath();
                if (path && path._ops && typeof __ocReplayPathOps === 'function') {
                    __ocReplayPathOps(id, path);
                }
                if (state.fillStyle && state.fillStyle.__ocPattern) {
                    fillPattern(state.fillStyle);
                    return;
                }
                applyFill();
                __canvas_fill_path(id);
            },
            stroke(path) {
                flushPath();
                if (path && path._ops && typeof __ocReplayPathOps === 'function') {
                    __ocReplayPathOps(id, path);
                }
                applyStroke();
                __canvas_stroke_path(id);
            },

            // ── line dash ──
            setLineDash(segments) {
                dash = [];
                if (Array.isArray(segments)) {
                    for (var i = 0; i < segments.length; i++) {
                        var v = num(segments[i]);
                        if (v < 0) throw new Error('setLineDash: negative dash interval');
                        dash.push(v);
                    }
                }
                lowerDash();
            },
            getLineDash() {
                return dash.slice();
            },

            // ── shapes (direct, no path needed) ──
            fillCircle(cx, cy, r) {
                flushPath();
                applyFill();
                __canvas_fill_circle(id, num(cx), num(cy), num(r));
            },

            // ── text ──
            // 批次 B 文本转路径：fillText/measureText 与 CanvasKit 面同源，
            // 都吃 __canvas_glyph_paths 的 shape 载荷（px 域 outline +
            // 相对基线的逐字位置）。字形烘进 2D path 一次 fill——DrawOp 流
            // 里只剩 path，段落位图通道（__canvas_draw_paragraph）退役出
            // 本面。emoji（color 载荷）不支持，报错指路 CanvasKit Paragraph。
            shapeGlyphs(text, font) {
                var key = text + '\u0000' + font.size + '\u0000' + font.family + '\u0000' + font.weight;
                var hit = shapeCache[key];
                if (hit) return hit;
                if (shapeCacheSize >= SHAPE_CACHE_CAP) {
                    shapeCache = Object.create(null);
                    shapeCacheSize = 0;
                }
                shapeCacheSize++;
                return (shapeCache[key] = this.shapeGlyphsUncached(text, font));
            },
            shapeGlyphsUncached(text, font) {
                var shaped = __canvas_glyph_paths(text, font.size, font.family, font.weight);
                if (shaped && shaped.color === true) {
                    // 彩色字形（emoji）：glyph outline 面拿不到彩图，改走段落
                    // 位图通道（`__canvas_draw_paragraph`，与 CanvasKit
                    // Paragraph 同源）。度量同取段落通道，行盒/基线与浏览器
                    // 一致；调用方见 `color` 标记改画位图。
                    if (typeof __canvas_measure_paragraph !== 'function'
                        || typeof __canvas_draw_paragraph !== 'function') {
                        throw new Error(
                            'fillText: color/emoji text needs the paragraph bindings (text: "'
                            + String(text).slice(0, 40) + '")'
                        );
                    }
                    var m = __canvas_measure_paragraph(
                        String(text), font.size, font.family, font.weight
                    ) || {};
                    return {
                        color: true,
                        glyphs: [],
                        positions: [],
                        width: Number(m.width) || 0,
                        ascent: Number(m.ascent) || 0,
                        descent: Number(m.descent) || 0,
                    };
                }
                return shaped;
            },
            // Shared path-baking for fillText/strokeText: shape once, then
            // replay every glyph outline at its position relative to the
            // (alignment-shifted) anchor. Returns false for empty text.
            bakeTextPath(text, x, y) {
                var t = String(text);
                if (t === '') return false;
                flushPath();
                // Horizontal alignment is composed here: the shape payload
                // measures the advance, and the baked path origin shifts to
                // the anchor the browser would use. `start`/`end` map to
                // `left`/`right` (the offline composition is LTR throughout).
                var align = state.textAlign;
                if (align === 'start') align = 'left';
                else if (align === 'end') align = 'right';
                if (align !== 'left' && align !== 'center' && align !== 'right') {
                    throw new Error(
                        "context2d.textAlign: only 'left'/'center'/'right'/'start'/'end' are supported, got '"
                        + state.textAlign + "'"
                    );
                }
                var f = parseFont(state.font);
                var shaped = this.shapeGlyphs(t, f);
                var bx = num(x);
                if (align === 'center') bx -= (Number(shaped.width) || 0) / 2;
                else if (align === 'right') bx -= Number(shaped.width) || 0;
                // Vertical baselines shift the alphabetic origin: the shape
                // payload measures from the alphabetic baseline (ascent up,
                // descent down), so middle puts the em-box midpoint at y,
                // top/bottom put the em edges at y.
                var by = num(y);
                var ascent = Number(shaped.ascent) || 0;
                var descent = Number(shaped.descent) || 0;
                // Baseline anchors come from the OS/2 typo pair (Chrome's
                // rule), not from the hhea em box above: for a CJK face they
                // differ by ~0.28em, i.e. a whole pixel at label sizes.
                var bAscent = shaped.baselineAscent === undefined ? ascent : Number(shaped.baselineAscent) || 0;
                var bDescent = shaped.baselineDescent === undefined ? descent : Number(shaped.baselineDescent) || 0;
                switch (state.textBaseline) {
                    case 'alphabetic': break;
                    case 'middle': by += (bAscent - bDescent) / 2; break;
                    case 'top': by += bAscent; break;
                    case 'bottom': case 'ideographic': by -= bDescent; break;
                    case 'hanging': by += ascent * 0.8; break;
                    default:
                        throw new Error(
                            "context2d.textBaseline: unsupported baseline '" + state.textBaseline + "'"
                        );
                }
                // Chrome snaps the device-space baseline to an integer y for
                // horizontal text (vertical glyph snapping; x stays subpixel).
                // The stream bakes in user space, so invert the CTM's y mapping
                // around the rounded device baseline; skewed text is not snapped.
                if (matrix.b === 0 && matrix.d !== 0) {
                    by = (Math.round(matrix.d * by + matrix.f) - matrix.f) / matrix.d;
                }
                if (shaped.color === true) {
                    // 彩色字形走位图（绑定约定 y 是段落顶，基线 = 顶 + ascent）。
                    // strokeText 同样落到位图：彩图字形没有 outline 可描，静默
                    // 描边比整帧失败更接近浏览器观察值。
                    var col = typeof state.fillStyle === 'string' ? state.fillStyle : '#000000';
                    __canvas_draw_paragraph(
                        id, t, f.size, f.family, Math.round(f.weight) || 400, col,
                        bx, by - ascent, Number(shaped.width) || 0, ascent + descent
                    );
                    return true;
                }
                __canvas_begin_path(id);
                pathFlat(id, textFlat(shaped), bx, by);
                return true;
            },
            fillText(text, x, y) {
                if (!this.bakeTextPath(text, x, y)) return;
                applyFill();
                __canvas_fill_path(id);
            },
            strokeText(text, x, y) {
                if (!this.bakeTextPath(text, x, y)) return;
                applyStroke();
                __canvas_stroke_path(id);
            },
            measureText(text) {
                var f = parseFont(state.font);
                var shaped = this.shapeGlyphs(String(text), f);
                // Browsers report the *ink* box, not the em box: TinySDF
                // (the g-webgl glyph atlas) sizes its SDF from
                // actualBoundingBox* and runs an EDT over exactly that
                // region. Curve control points overestimate the true ink by
                // a few tenths of a pixel — ceil() eats it, and solving the
                // extrema here would buy nothing visible.
                var byGlyph = {};
                var gs = shaped.glyphs || [];
                for (var i = 0; i < gs.length; i++) byGlyph[gs[i].k] = gs[i].ops;
                var positions = shaped.positions || [];
                var minX = 0, maxX = 0, minY = 0, maxY = 0, ink = false;
                for (var p = 0; p < positions.length; p++) {
                    var ops = byGlyph[positions[p].k];
                    if (!ops) continue;
                    var ox = Number(positions[p].x) || 0;
                    var oy = Number(positions[p].y) || 0;
                    for (var j = 0; j < ops.length; j++) {
                        var op = ops[j];
                        for (var c = 1; c + 1 < op.length; c += 2) {
                            var gx = ox + Number(op[c]);
                            var gy = oy + Number(op[c + 1]);
                            if (!ink) {
                                minX = maxX = gx; minY = maxY = gy; ink = true;
                            } else {
                                if (gx < minX) minX = gx;
                                if (gx > maxX) maxX = gx;
                                if (gy < minY) minY = gy;
                                if (gy > maxY) maxY = gy;
                            }
                        }
                    }
                }
                return {
                    width: Number(shaped.width) || 0,
                    actualBoundingBoxLeft: ink ? -minX : 0,
                    actualBoundingBoxRight: ink ? maxX : 0,
                    actualBoundingBoxAscent: ink ? -minY : 0,
                    actualBoundingBoxDescent: ink ? maxY : 0,
                };
            },
            // 光栅化回读（g-lite measureFont 逐像素扫 ascent/descent、heatmap
            // DE 全图 LUT 换色、three 拿 canvas 当贴图都走这里）：本帧已录的
            // 命令重放成 RGBA，再行拷贝切出请求窗口。离屏画布与 html 通道的
            // 树内 canvas 可读；尺寸各自从所属节点表取。
            getImageData(sx, sy, sw, sh) {
                var canvas = (typeof __ocOffscreenById === 'function' ? __ocOffscreenById(id) : null)
                    || (typeof __ocHtmlCanvasById === 'function' ? __ocHtmlCanvasById(id) : null);
                if (!canvas) throw new Error('getImageData: no readable canvas for ' + id);
                var cw = Math.max(1, Math.ceil(canvas.width > 0 ? canvas.width : 1));
                var ch = Math.max(1, Math.ceil(canvas.height > 0 ? canvas.height : 1));
                if (typeof __opencatRasterCanvas !== 'function') {
                    throw new Error('getImageData: engine native __opencatRasterCanvas unavailable');
                }
                var full = new Uint8Array(cw * ch * 4);
                __opencatRasterCanvas(id, cw, ch, full);
                // 规范化请求窗口（Web 语义：越界部分透明黑，负宽高翻转忽略——
                // g-lite/heatmap 都传规范值，这里只做夹取）。
                var x0 = Math.max(0, Math.min(cw, sx | 0));
                var y0 = Math.max(0, Math.min(ch, sy | 0));
                var x1 = Math.max(0, Math.min(cw, (sx + (sw | 0)) | 0));
                var y1 = Math.max(0, Math.min(ch, (sy + (sh | 0)) | 0));
                var w = Math.max(1, x1 - x0);
                var h = Math.max(1, y1 - y0);
                var out = new Uint8ClampedArray(w * h * 4);
                for (var row = 0; row < h; row++) {
                    var srcRow = ((y0 + row) * cw + x0) * 4;
                    out.set(full.subarray(srcRow, srcRow + w * 4), row * w * 4);
                }
                return { width: w, height: h, data: out };
            },
            // putImageData（浏览器语义：方法在 context 上，G2 heatmap 的
            // `ctx.putImageData(img, 0, 0)` 走这里；dom_api 元素级
            // node.putImageData 是同一 native 的旧挂法）。直写像素以
            // CanvasOp::PutImage 进离屏 op 流；key 帧（调用序）唯一——
            // 同 id 异像素共用键会被 native 判 hard error。
            putImageData(img, dx, dy) {
                if (!id.indexOf || id.indexOf('__oc_offscreen_') !== 0) {
                    throw new Error('putImageData: only offscreen canvases support pixel writes');
                }
                if (typeof __opencatPutImageData !== 'function') {
                    throw new Error('putImageData: engine native __opencatPutImageData unavailable');
                }
                var u8 = img.data instanceof Uint8Array
                    ? img.data
                    : new Uint8Array(img.data.buffer, img.data.byteOffset, img.data.byteLength);
                putImageSeq += 1;
                __opencatPutImageData(id, id + ':' + putImageSeq, u8, dx | 0, dy | 0, img.width, img.height);
            },
        };
    }

    // Property surface: the state record behind plain get/set accessors, so
    // author code reads/writes `ctx2d.fillStyle` like the browser. Writing a
    // value the stream cannot express throws at the assignment; unknown
    // properties return undefined instead of throwing (reading unknown props
    // is how feature probes fail on the web too) and writing one returns
    // false. The @antv/g-canvas state cache only writes a property when the
    // object's value differs from its default, so these writer checks never
    // fire for objects that don't use the feature.
    function paintObject(v, api) {
        if (v !== null && typeof v === 'object' && !v.__ocGradient && !v.__ocPattern) {
            // g-lite 的 applyAttributesToContext 会把 **raw** pattern 对象
            // （{image, repetition}）先赋给 fillStyle/strokeStyle；浏览器对
            // 无法识别的 paint 对象静默忽略（fillStyle 保持原值），真正的
            // CanvasPattern 在 styleRenderer.render 里经 getPattern →
            // createPattern 后重新赋值（那时才带 __ocPattern）。这里保持
            // 浏览器语义：记录赋值无效，不改状态。
            return false;
        }
        return true;
    }

    // canvas 2D composite/blend 名 → wire 编号（core `encode_blend_mode` 的
    // 规范序）。'lighter' 即 Skia 的 Plus；纯 'destination'、'plus-lighter'、
    // 'plus-darker' 不支持。
    var COMPOSITE_MODES = {
        'clear': 0,
        'copy': 1,
        'source-over': 3,
        'destination-over': 4,
        'source-in': 5,
        'destination-in': 6,
        'source-out': 7,
        'destination-out': 8,
        'source-atop': 9,
        'destination-atop': 10,
        'xor': 11,
        'lighter': 12,
        'multiply': 24,
        'screen': 14,
        'overlay': 15,
        'darken': 16,
        'lighten': 17,
        'color-dodge': 18,
        'color-burn': 19,
        'hard-light': 20,
        'soft-light': 21,
        'difference': 22,
        'exclusion': 23,
        'hue': 25,
        'saturation': 26,
        'color': 27,
        'luminosity': 28,
    };

    function compositeMode(state) {
        var m = COMPOSITE_MODES[state.globalCompositeOperation];
        return m === undefined ? 3 : m;
    }

    function checkComposite(v) {
        if (COMPOSITE_MODES[String(v)] === undefined) {
            throw new Error(
                "context2d.globalCompositeOperation: unsupported value '" + String(v) + "'"
            );
        }
    }

    // Canvas shadows have no Skia op: core expands this sticky state around
    // every fill/stroke as SaveLayer(DropShadow, keep_content:false) + the
    // real draw (helpers.rs `push_shadowed`). It is executor-side persistent
    // state, so it must ride along with each paint — same rule as globalAlpha
    // (sent against the context's `sent` mirror, same-value sends skipped).
    function lowerShadow(id, state, sent) {
        sendShadow(
            id,
            sent,
            cssColor(state.shadowColor),
            num(state.shadowBlur),
            num(state.shadowOffsetX),
            num(state.shadowOffsetY)
        );
    }

    function sendShadow(id, sent, color, blur, dx, dy) {
        if (sent.shadowColor === color && sent.shadowBlur === blur
            && sent.shadowDx === dx && sent.shadowDy === dy) return;
        sent.shadowColor = color;
        sent.shadowBlur = blur;
        sent.shadowDx = dx;
        sent.shadowDy = dy;
        __canvas_set_shadow(id, color, blur, dx, dy);
    }

    // globalCompositeOperation / ctx.filter likewise ride along with each
    // paint as executor-side persistent state: composite folds into the fill/
    // stroke paint, `blur` wraps each draw in a SaveLayer(Blur) (helpers.rs
    // `push_painted`). 下发按需：同值跳过（镜像 `sent` 见 makeContext2d）——
    // halftone 语料每帧 ~30 万次 fill，每 draw 两条默认值 op 会把流撑大 1/3。
    function lowerComposite(id, state, sent) {
        var blend = compositeMode(state);
        if (sent.blend !== blend) {
            __canvas_set_blend_mode(id, blend);
            sent.blend = blend;
        }
        var sigma = filterSigma(state);
        if (sent.filter !== sigma) {
            __canvas_set_ctx_filter(id, sigma);
            sent.filter = sigma;
        }
    }

    // 'blur(<N>px)' → sigma；其余（含 'none'）→ null。
    var FILTER_BLUR_RE = /^blur\(\s*([0-9]+(?:\.[0-9]+)?)px\s*\)$/;
    function parseCtxFilter(v) {
        var m = FILTER_BLUR_RE.exec(String(v).trim());
        return m ? parseFloat(m[1]) : null;
    }

    function filterSigma(state) {
        if (state.filter === 'none') return 0;
        var s = parseCtxFilter(state.filter);
        return s === null ? 0 : s;
    }

    function checkFilter(v) {
        if (String(v) !== 'none' && parseCtxFilter(v) === null) {
            throw new Error(
                "context2d.filter: only 'none' or 'blur(<N>px)' is supported, got '" + String(v) + "'"
            );
        }
    }

    var STATE_PROPS = [
        'fillStyle', 'strokeStyle', 'lineWidth', 'lineCap', 'lineJoin', 'miterLimit',
        'globalAlpha', 'globalCompositeOperation', 'shadowColor', 'shadowBlur',
        'shadowOffsetX', 'shadowOffsetY', 'filter', 'font', 'textAlign', 'textBaseline',
        'lineDashOffset',
    ];

    var WRITERS = {
        fillStyle: function (state, v) { if (paintObject(v, 'fillStyle')) state.fillStyle = v; },
        strokeStyle: function (state, v) { if (paintObject(v, 'strokeStyle')) state.strokeStyle = v; },
        globalCompositeOperation: function (state, v) { checkComposite(v); state.globalCompositeOperation = v; },
        shadowColor: function (state, v) { state.shadowColor = v; },
        shadowBlur: function (state, v) { state.shadowBlur = num(v); },
        shadowOffsetX: function (state, v) { state.shadowOffsetX = num(v); },
        shadowOffsetY: function (state, v) { state.shadowOffsetY = num(v); },
        filter: function (state, v) { checkFilter(v); state.filter = v; },
        miterLimit: function (state, v) {
            // 4 is what the stream's stroke paint carries (SkPaint default);
            // 10 is the CSS default (the stream clips spiky joins a bit
            // earlier — chart shapes never reach it). Anything else fails.
            if (num(v) !== 4 && num(v) !== 10) {
                throw new Error('context2d.miterLimit: only 4 (stream) or 10 (CSS default) is supported, got ' + num(v));
            }
            state.miterLimit = num(v);
        },
    };

    ctx.getContext = function (nodeId, type) {
        var t = String(type);
        if (t === 'webgl' || t === 'webgl2') {
            throw new Error(
                "getContext('" + t + "') is not supported; build 3D with three (import * as THREE from 'three') — WebGLRenderer + scene + camera"
            );
        }
        if (t !== '2d') {
            throw new Error("getContext: only '2d' is supported, got '" + t + "'");
        }
        var key = String(nodeId);
        if (!contexts[key]) {
            var raw = makeContext2d(key);
            var descriptors = {};
            for (var i = 0; i < STATE_PROPS.length; i++) {
                (function (prop) {
                    var write = WRITERS[prop];
                    descriptors[prop] = {
                        get() { return raw.state[prop]; },
                        set(v) {
                            if (write) write(raw.state, v);
                            else raw.state[prop] = v;
                        },
                    };
                })(STATE_PROPS[i]);
            }
            contexts[key] = Object.defineProperties(raw, descriptors);
        }
        return contexts[key];
    };

    // Frame-boundary reset for every cached context (called from
    // `__ocResetFrame`). Installed lazily-guarded so runtimes without this
    // module's globals (extract probes) stay unaffected.
    globalThis.__ocResetContext2d = function () {
        for (var key in contexts) contexts[key].__ocResetFrameState();
        // offscreenRasterCache 故意跨帧存活：pattern 源 canvas 建一次、每帧
        // fillPattern 重铺同一 PNG 字节。canvas 内容以 putImageData/draw
        // 追加时 id 不变，缓存失效由 rasterizeOffscreenCanvas 的调用点按
        // 「首像素化后只读」约定处理（2D pattern 源正是如此）。
    };
    // 宿主每帧 reset_for_frame 清空帧级 canvas 命令流后调用（realm 的
    // apply_frame_ctx，所有通道一个落点）：只作废下发镜像，上下文状态不动
    // ——html 通道不调 `__ocResetContext2d`，状态跨帧保持同浏览器画布。
    // 离屏画布的流跨帧持久，作废只多一次冗余下发（顺带兜住 LRU 驱逐）。
    globalThis.__ocContext2dNewFrame = function () {
        for (var key in contexts) contexts[key].__ocForgetSent();
    };
    // 帧尾批 flush（realm run_frame 尾、op 流 take 之前调用）：批点必须
    // 进本帧 op 流，否则 drawHalftone 这类「画完整帧不触发其他 canvas 调用」
    // 的循环把 30 万点全憋在 JS 数组里，帧界作废=整帧位图丢失。
    globalThis.__ocFlushCanvasBatches = function () {
        for (var key in contexts) contexts[key].__flushBatch();
    };
    // Offscreen canvases (`document.createElement('canvas')`) share this
    // recorder: dom_api hands us a synthetic id, we hand back the same
    // descriptor-wrapped face as `ctx.getContext`. Cached by id so repeated
    // getContext calls return the same object (Web identity semantics).
    globalThis.__ocMakeOffscreenContext2d = function (id) {
        var face = ctx.getContext(id, '2d');
        // 浏览器 2D 上下文都有 `.canvas` 反指（G2 heatmap 渲染器拿
        // ctx.canvas.toDataURL() 消费离屏产物）。face 由 makeContext2d
        // 缓存共享，defineProperty 一次即可。
        if (face && face.canvas === undefined) {
            Object.defineProperty(face, 'canvas', {
                get: function () { return globalThis.__ocOffscreenById(id); },
                configurable: true,
            });
        }
        return face;
    };
})();
