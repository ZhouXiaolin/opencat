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

    function makeContext2d(id) {
        var state = freshState();
        var dash = [];
        var matrix = { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 };
        var matrixStack = [];
        var stateStack = [];
        // 当前路径的保守包围盒（用户空间）。只有图案填充要它：瓦片必须铺满
        // 整条路径，而流里没有 pattern 对象，core 只能按包围盒铺瓦片。曲线
        // 取控制点、圆弧取整圆——宁大勿小。
        var pathBounds = null;

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
            __canvas_set_anti_alias(id, true);
            if (state.strokeStyle && state.strokeStyle.__ocGradient) {
                lowerGradientPaint(true, state.strokeStyle);
            } else {
                __canvas_set_stroke_style(id, cssColor(state.strokeStyle));
            }
            __canvas_set_line_width(id, num(state.lineWidth));
            __canvas_set_line_cap(id, state.lineCap);
            __canvas_set_line_join(id, state.lineJoin);
            lowerDash();
            // 必须无条件下发：执行端的 alpha 是持久侧状态，漏发会把上一个
            // save/restore 组里的 globalAlpha 泄漏到后续所有绘制。
            __canvas_set_global_alpha(id, num(state.globalAlpha));
            lowerShadow(id, state);
        }

        function applyFill() {
            __canvas_set_anti_alias(id, true);
            if (state.fillStyle && state.fillStyle.__ocGradient) {
                lowerGradientPaint(false, state.fillStyle);
            } else {
                __canvas_set_fill_style(id, cssColor(state.fillStyle));
            }
            // 必须无条件下发：执行端的 alpha 是持久侧状态，漏发会把上一个
            // save/restore 组里的 globalAlpha 泄漏到后续所有绘制。
            __canvas_set_global_alpha(id, num(state.globalAlpha));
            lowerShadow(id, state);
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
            __canvas_set_global_alpha(id, alpha);
            __canvas_save(id);
            __canvas_clip_path(id, true);
            __canvas_concat(id, [m.a, m.b, 0, m.c, m.d, 0, m.e, m.f, 1]);
            for (var i = Math.floor(u0 / w); i <= Math.floor(u1 / w); i++) {
                for (var j = Math.floor(v0 / h); j <= Math.floor(v1 / h); j++) {
                    // 参数顺序与 canvas.drawImage(img, sx, sy, sw, sh, dx, dy, dw, dh)
                    // 不同：绑定层收 (dst, src)，即 [dx, dy, dw, dh, sx, sy, sw, sh]。
                    __canvas_draw_image_bytes(
                        id, paint.bytes, [i * w, j * h, w, h, 0, 0, w, h], alpha, true
                    );
                }
            }
            __canvas_restore(id);
        }

        function setMatrix(next) {
            // Stream ops compose on the right (`CTM · M`), so replacing the
            // matrix lowers to concatenating `inv(cur) · next`.
            var delta = matMul(matInverse(matrix), next);
            __canvas_concat(id, [delta.a, delta.b, 0, delta.c, delta.d, 0, delta.e, delta.f, 1]);
            matrix = next;
        }

        return {
            state: state,

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
            },

            // ── transforms ──
            // save/restore snapshot the FULL paint state, not just the
            // matrix: callers (g-canvas) read properties back through the
            // getters after restore and draw text with
            // `globalAlpha *= opacity` — a stale JS-side alpha multiplies
            // into every following draw.
            save() {
                matrixStack.push(matrix);
                var snap = Object.assign(freshState(), state);
                snap.__dash = dash.slice();
                stateStack.push(snap);
                __canvas_save(id);
            },
            restore() {
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
                matrix = matMul(matrix, { a: 1, b: 0, c: 0, d: 1, e: num(x), f: num(y) });
                __canvas_translate(id, num(x), num(y));
            },
            rotate(radians) {
                var cos = Math.cos(num(radians));
                var sin = Math.sin(num(radians));
                matrix = matMul(matrix, { a: cos, b: sin, c: -sin, d: cos, e: 0, f: 0 });
                __canvas_rotate(id, num(radians) * 180 / Math.PI);
            },
            scale(sx, sy) {
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
            // g-canvas clears from the origin on every frame entry; offline
            // frames have no prior content to erase, so an origin clear is
            // just a full-canvas clear. True regional clears would need a
            // transparent-fill op the stream doesn't have.
            clearRect(x, y, w, h) {
                if (num(x) <= 0 && num(y) <= 0 && num(w) > 0 && num(h) > 0) {
                    __canvas_clear(id, null);
                    return;
                }
                throw new Error(
                    'clearRect: only full-canvas clears from the origin are supported ('
                    + num(x) + ',' + num(y) + ',' + num(w) + ',' + num(h) + ')'
                );
            },
            fillRect(x, y, w, h) {
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
                const bytes = image && image.__ocImageArray;
                if (!bytes) throw new Error('drawImage: only decoded Image sources are supported');
                const nw = num(image.naturalWidth || image.width) || 0;
                const nh = num(image.naturalHeight || image.height) || 0;
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
            beginPath() { pathBounds = null; __canvas_begin_path(id); },
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
            // 图案画刷：只接解码后的 Image 源（离屏 canvas 源需要真像素面，
            // 未实现）。`setTransform` 记下图案矩阵，填充时由 fillPattern 用。
            createPattern(source, repetition) {
                var bytes = source && source.__ocImageArray;
                if (!bytes) {
                    throw new Error('createPattern: only decoded Image sources are supported');
                }
                var w = num(source.naturalWidth || source.width) || 0;
                var h = num(source.naturalHeight || source.height) || 0;
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
            closePath() { __canvas_close_path(id); },
            moveTo(x, y) {
                expandPathBounds(num(x), num(y), num(x), num(y));
                __canvas_move_to(id, num(x), num(y));
            },
            lineTo(x, y) {
                expandPathBounds(num(x), num(y), num(x), num(y));
                __canvas_line_to(id, num(x), num(y));
            },
            rect(x, y, w, h) {
                expandPathBounds(num(x), num(y), num(x) + num(w), num(y) + num(h));
                __canvas_path_add_rect(id, num(x), num(y), num(w), num(h));
            },
            quadraticCurveTo(cpx, cpy, x, y) {
                expandPathBounds(
                    Math.min(num(cpx), num(x)), Math.min(num(cpy), num(y)),
                    Math.max(num(cpx), num(x)), Math.max(num(cpy), num(y))
                );
                __canvas_quad_to(id, num(cpx), num(cpy), num(x), num(y));
            },
            bezierCurveTo(c1x, c1y, c2x, c2y, x, y) {
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
                __canvas_path_add_arc(
                    id,
                    num(x) - num(radius), num(y) - num(radius),
                    num(radius) * 2, num(radius) * 2,
                    s.start, s.sweep
                );
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
                __canvas_path_add_arc(
                    id,
                    num(x) - num(radiusX), num(y) - num(radiusY),
                    num(radiusX) * 2, num(radiusY) * 2,
                    s.start, s.sweep
                );
            },
            clip() {
                __canvas_clip_path(id, true);
            },
            fill() {
                if (state.fillStyle && state.fillStyle.__ocPattern) {
                    fillPattern(state.fillStyle);
                    return;
                }
                applyFill();
                __canvas_fill_path(id);
            },
            stroke() {
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
                var byGlyph = {};
                var gs = shaped.glyphs || [];
                for (var i = 0; i < gs.length; i++) byGlyph[gs[i].k] = gs[i].ops;
                var positions = shaped.positions || [];
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
                for (var p = 0; p < positions.length; p++) {
                    var ops = byGlyph[positions[p].k];
                    if (!ops) continue;
                    var gx = bx + positions[p].x;
                    var gy = by + positions[p].y;
                    for (var j = 0; j < ops.length; j++) {
                        var op = ops[j];
                        if (op[0] === 0) __canvas_move_to(id, gx + op[1], gy + op[2]);
                        else if (op[0] === 1) __canvas_line_to(id, gx + op[1], gy + op[2]);
                        else if (op[0] === 2) __canvas_quad_to(id, gx + op[1], gy + op[2], gx + op[3], gy + op[4]);
                        else if (op[0] === 3) __canvas_cubic_to(id, gx + op[1], gy + op[2], gx + op[3], gy + op[4], gx + op[5], gy + op[6]);
                        else if (op[0] === 4) __canvas_close_path(id);
                        else throw new Error('fillText: unknown glyph op ' + op[0]);
                    }
                }
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
                return { width: Number(shaped.width) || 0 };
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
            throw new Error(
                api + ': gradient/pattern paint objects are not supported; use a CSS color string'
            );
        }
    }

    function checkComposite(v) {
        if (String(v) !== 'source-over') {
            throw new Error(
                "context2d.globalCompositeOperation: only 'source-over' is supported, got '" + String(v) + "'"
            );
        }
    }

    // Canvas shadows have no Skia op: core expands this sticky state around
    // every fill/stroke as SaveLayer(DropShadow, keep_content:false) + the
    // real draw (helpers.rs `push_shadowed`). It is executor-side persistent
    // state, so it must ride along with each paint — same rule as globalAlpha.
    function lowerShadow(id, state) {
        __canvas_set_shadow(
            id,
            cssColor(state.shadowColor),
            num(state.shadowBlur),
            num(state.shadowOffsetX),
            num(state.shadowOffsetY)
        );
    }

    function checkFilter(v) {
        if (String(v) !== 'none') {
            throw new Error("context2d.filter: only 'none' is supported, got '" + String(v) + "'");
        }
    }

    var STATE_PROPS = [
        'fillStyle', 'strokeStyle', 'lineWidth', 'lineCap', 'lineJoin', 'miterLimit',
        'globalAlpha', 'globalCompositeOperation', 'shadowColor', 'shadowBlur',
        'shadowOffsetX', 'shadowOffsetY', 'filter', 'font', 'textAlign', 'textBaseline',
        'lineDashOffset',
    ];

    var WRITERS = {
        fillStyle: function (state, v) { paintObject(v, 'fillStyle'); state.fillStyle = v; },
        strokeStyle: function (state, v) { paintObject(v, 'strokeStyle'); state.strokeStyle = v; },
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
    };
})();
