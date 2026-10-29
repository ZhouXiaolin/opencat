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
// What this buys: real packages written against `getContext('2d')` (progress
// rings, sparklines, particles) run unmodified on the CK backend. Anything
// the CK stream cannot express (regional clearRect, counterclockwise arcs)
// errors with the supported alternative instead of silently diverging.
(function () {
    'use strict';

    var contexts = {};

    function cssColor(value) {
        if (value === undefined || value === null) return '#000000';
        var v = String(value).trim();
        return v === '' ? '#000000' : v;
    }

    // CSS font shorthand → { weight, size, family }. Only the canonical
    // `<weight> <size>px <family>` forms real packages write.
    function parseFont(font) {
        var m = /^\s*(\d{1,3})\s+([0-9.]+)px\s+(.+?)\s*$/.exec(String(font));
        if (!m) {
            throw new Error(
                "context2d.font: expected '<weight> <size>px <family>', got '" + String(font) + "'"
            );
        }
        return {
            weight: parseInt(m[1], 10),
            size: parseFloat(m[2]),
            family: m[3].split(',')[0].replace(/["']/g, '').trim(),
        };
    }

    function num(v) {
        var n = Number(v);
        return Number.isFinite(n) ? n : 0;
    }

    function makeContext2d(id) {
        var state = {
            fillStyle: '#000000',
            strokeStyle: '#000000',
            lineWidth: 1,
            lineCap: 'butt',
            lineJoin: 'miter',
            globalAlpha: 1,
            font: '10px sans-serif',
            textAlign: 'left',
            textBaseline: 'alphabetic',
        };

        function applyStroke() {
            __canvas_set_anti_alias(id, true);
            __canvas_set_stroke_style(id, cssColor(state.strokeStyle));
            __canvas_set_line_width(id, num(state.lineWidth));
            __canvas_set_line_cap(id, state.lineCap);
            __canvas_set_line_join(id, state.lineJoin);
            if (num(state.globalAlpha) < 1) __canvas_set_global_alpha(id, num(state.globalAlpha));
        }

        function applyFill() {
            __canvas_set_anti_alias(id, true);
            __canvas_set_fill_style(id, cssColor(state.fillStyle));
            if (num(state.globalAlpha) < 1) __canvas_set_global_alpha(id, num(state.globalAlpha));
        }

        return {
            state: state,

            // ── transforms ──
            save() { __canvas_save(id); },
            restore() { __canvas_restore(id); },
            translate(x, y) { __canvas_translate(id, num(x), num(y)); },
            rotate(deg) { __canvas_rotate(id, num(deg)); },
            scale(sx, sy) { __canvas_scale(id, num(sx), num(sy)); },

            // ── rects ──
            clearRect(x, y, w, h) {
                if (num(x) <= 0 && num(y) <= 0 && num(w) >= 1e9 && num(h) >= 1e9) {
                    __canvas_clear(id, null);
                    return;
                }
                throw new Error(
                    'clearRect: only full-canvas clears are supported; draw a filled rect in the background color instead'
                );
            },
            fillRect(x, y, w, h) {
                applyFill();
                __canvas_fill_rect(id, num(x), num(y), num(w), num(h), cssColor(state.fillStyle));
            },
            strokeRect(x, y, w, h) {
                applyStroke();
                __canvas_stroke_rect(id, num(x), num(y), num(w), num(h), cssColor(state.strokeStyle), num(state.lineWidth));
            },

            // ── paths ──
            beginPath() { __canvas_begin_path(id); },
            closePath() { __canvas_close_path(id); },
            moveTo(x, y) { __canvas_move_to(id, num(x), num(y)); },
            lineTo(x, y) { __canvas_line_to(id, num(x), num(y)); },
            quadraticCurveTo(cpx, cpy, x, y) {
                __canvas_quad_to(id, num(cpx), num(cpy), num(x), num(y));
            },
            bezierCurveTo(c1x, c1y, c2x, c2y, x, y) {
                __canvas_cubic_to(id, num(c1x), num(c1y), num(c2x), num(c2y), num(x), num(y));
            },
            arc(x, y, radius, startAngle, endAngle, counterclockwise) {
                if (counterclockwise) {
                    throw new Error('arc: counterclockwise arcs are not supported');
                }
                // HTML arcs go clockwise from startAngle to endAngle; a
                // negative sweep means the same circle the other way —
                // normalize into an equivalent positive sweep.
                var sweep = num(endAngle) - num(startAngle);
                if (sweep < 0) sweep += Math.PI * 2;
                applyFill();
                __canvas_path_add_arc(
                    id,
                    num(x) - num(radius), num(y) - num(radius),
                    num(radius) * 2, num(radius) * 2,
                    num(startAngle), sweep
                );
            },
            fill() {
                applyFill();
                __canvas_fill_path(id);
            },
            stroke() {
                applyStroke();
                __canvas_stroke_path(id);
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
                if (!shaped || shaped.color === true) {
                    throw new Error(
                        'fillText: color/emoji text is not supported on the 2D context; use the CanvasKit Paragraph API'
                    );
                }
                return shaped;
            },
            fillText(text, x, y) {
                var t = String(text);
                if (t === '') return;
                if (state.textAlign !== 'left') {
                    throw new Error(
                        "context2d.textAlign: only 'left' is supported, got '" + state.textAlign + "'"
                    );
                }
                if (state.textBaseline !== 'alphabetic') {
                    throw new Error(
                        "context2d.textBaseline: only 'alphabetic' is supported, got '" + state.textBaseline + "'"
                    );
                }
                var f = parseFont(state.font);
                var shaped = this.shapeGlyphs(t, f);
                var byGlyph = {};
                var gs = shaped.glyphs || [];
                for (var i = 0; i < gs.length; i++) byGlyph[gs[i].k] = gs[i].ops;
                var positions = shaped.positions || [];
                var bx = num(x);
                var by = num(y);
                applyFill();
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
                __canvas_fill_path(id);
            },
            measureText(text) {
                var f = parseFont(state.font);
                var shaped = this.shapeGlyphs(String(text), f);
                return { width: Number(shaped.width) || 0 };
            },
        };
    }

    // Property surface: the state record behind plain get/set accessors, so
    // author code reads/writes `ctx2d.fillStyle` like the browser. Unsupported
    // properties return undefined instead of throwing (reading unknown props
    // is how feature probes fail on the web too); writing one returns false
    // (strict-mode silently-ignored assignment) and the draw call itself will
    // surface anything real.
    var STATE_PROPS = [
        'fillStyle', 'strokeStyle', 'lineWidth', 'lineCap', 'lineJoin',
        'globalAlpha', 'font', 'textAlign', 'textBaseline',
    ];

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
                    descriptors[prop] = {
                        get() { return raw.state[prop]; },
                        set(v) { raw.state[prop] = v; },
                    };
                })(STATE_PROPS[i]);
            }
            contexts[key] = Object.defineProperties(raw, descriptors);
        }
        return contexts[key];
    };
})();
