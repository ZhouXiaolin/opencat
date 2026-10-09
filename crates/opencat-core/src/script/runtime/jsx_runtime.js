/* JSX runtime for the JSX frontend (`parse::jsx`).
 *
 * Responsibilities:
 *   1. `h()` element factory — the classic runtime every JSX tag lowers to.
 *   2. A tiny CommonJS-style module system (`__ocRequire`) so author-facing
 *      `import` works inside quickjs / wasm hosts. Module sources are injected
 *      by the host as `globalThis.__ocModuleSources` (specifier -> code).
 *   3. The `use*` capability registry: every `use*` call registers a per-frame
 *      driver closure. Frame model is `frame = f(t)` — the driver resets module
 *      state each frame, re-evaluates the module graph (fresh globals), then
 *      replays the hooks. Hooks must be pure in `t`.
 *   4. `__ocExtract()` — one-shot pass that evaluates the entry module and
 *      walks the returned element tree into JSONL lines (the host turns those
 *      into a ParsedComposition). Runs BEFORE any per-frame driver.
 *
 * `use*` family (v1):
 *   useCanvas(node, draw)  — per-frame CanvasKit 2D drawing on a canvas node
 *   useGSAP(build)         — per-frame timeline block (GSAP-like)
 *   useFrame(build)        — escape hatch: arbitrary per-frame `(ctx, t)` work
 *   useCanvasKit()         — the CanvasKit namespace, for module-level paints
 *   useComposition()       — `{ width, height, fps, duration }` from `meta`
 */
(function () {
    'use strict';

    var FRAGMENT = '__ocFragment';
    var ENTRY = '<entry>';

    var builtins = Object.create(null);
    var modules = Object.create(null);
    var hooks = [];
    var idCounter = 0;
    var textCounter = 0;
    var currentModule = null;

    // ── element factory ──────────────────────────────────────────────
    function flattenInto(value, out) {
        if (value === null || value === undefined || value === false || value === true || value === '') {
            return;
        }
        if (Array.isArray(value)) {
            for (var i = 0; i < value.length; i++) flattenInto(value[i], out);
            return;
        }
        out.push(value);
    }

    function h(tag, props) {
        var children = [];
        for (var i = 2; i < arguments.length; i++) flattenInto(arguments[i], children);
        props = props || {};
        if (props.id === undefined || props.id === null || props.id === '') {
            props.id = 'oc-' + (++idCounter);
        }
        return { __ocEl: true, tag: tag, props: props, children: children };
    }

    globalThis.h = h;
    globalThis[FRAGMENT] = FRAGMENT;

    // ── module system ────────────────────────────────────────────────
    function resolveSpec(spec) {
        if (Object.prototype.hasOwnProperty.call(builtins, spec)) return spec;
        var sources = globalThis.__ocModuleSources || {};
        if (Object.prototype.hasOwnProperty.call(sources, spec)) return spec;
        if (spec.charAt(0) !== '.') {
            throw new Error('use: unknown module "' + spec + '" (only relative paths and "opencat/runtime" are built in)');
        }
        var base = currentModule || '';
        var slash = base.lastIndexOf('/');
        var parts = slash >= 0 ? base.slice(0, slash).split('/') : [];
        var segments = spec.split('/');
        for (var i = 0; i < segments.length; i++) {
            var seg = segments[i];
            if (seg === '' || seg === '.') continue;
            if (seg === '..') parts.pop();
            else parts.push(seg);
        }
        var joined = parts.join('/');
        if (Object.prototype.hasOwnProperty.call(sources, joined)) return joined;
        var suffixes = ['.js', '.jsx', '/index.js', '/index.jsx'];
        for (var s = 0; s < suffixes.length; s++) {
            if (Object.prototype.hasOwnProperty.call(sources, joined + suffixes[s])) {
                return joined + suffixes[s];
            }
        }
        throw new Error('use: cannot resolve "' + spec + '" from "' + (base || ENTRY) + '"');
    }

    function requireModule(spec) {
        var key = resolveSpec(spec);
        if (Object.prototype.hasOwnProperty.call(builtins, key)) return builtins[key];
        if (Object.prototype.hasOwnProperty.call(modules, key)) return modules[key].exports;
        var sources = globalThis.__ocModuleSources || {};
        if (!Object.prototype.hasOwnProperty.call(sources, key)) {
            throw new Error('use: module "' + key + '" was not provided by the host');
        }
        var mod = { exports: {} };
        modules[key] = mod;
        var previous = currentModule;
        currentModule = key;
        try {
            var fn = new Function('__ocRequire', 'module', 'exports', sources[key]);
            fn(requireModule, mod, mod.exports);
        } finally {
            currentModule = previous;
        }
        return mod.exports;
    }

    globalThis.__ocRequire = requireModule;

    globalThis.__ocResetFrame = function () {
        modules = Object.create(null);
        hooks.length = 0;
        idCounter = 0;
        textCounter = 0;
    };

    globalThis.__ocOnFrame = function (fn) { hooks.push(fn); };

    globalThis.__ocRunFrameHooks = function () {
        for (var i = 0; i < hooks.length; i++) hooks[i](globalThis.ctx);
    };

    // ── use* registry ───────────────────────────────────────────────
    function useCanvas(node, draw) {
        if (!node || node.__ocEl !== true) {
            throw new Error('useCanvas: first argument must be a JSX element');
        }
        if (node.tag !== 'canvas') {
            throw new Error('useCanvas: expected a <canvas> element, got <' + node.tag + '>');
        }
        var id = String(node.props.id);
        globalThis.__ocOnFrame(function () {
            var canvas = globalThis.ctx.getCanvasById(id);
            augmentCanvas(canvas, id);
            draw(canvas, globalThis.ctx.currentTime, globalThis.ctx);
        });
        return node;
    }

    // Full handle surface: the raw CanvasKit facade (clear / drawCircle /
    // drawPath / save / restore / …) plus the shorthands below.
    var augmented = new WeakMap();
    function augmentCanvas(canvas, id) {
        if (augmented.get(canvas)) return canvas;
        augmented.set(canvas, true);
        canvas.nodeId = id;
        canvas.CK = globalThis.ctx.CanvasKit;
        // paint({ color, alpha, style, strokeWidth, cap, join, dash })
        // `color` accepts '#rrggbb', [r,g,b,a] in 0..1, or a CK color array.
        canvas.paint = function (opts) {
            opts = opts || {};
            var paint = new globalThis.ctx.CanvasKit.Paint();
            if (opts.style) paint.setStyle(opts.style);
            if (opts.antiAlias !== undefined) paint.setAntiAlias(opts.antiAlias);
            if (opts.color !== undefined) {
                if (Array.isArray(opts.color)) {
                    paint.setColorComponents(
                        opts.color[0], opts.color[1], opts.color[2],
                        opts.color.length > 3 ? opts.color[3] : 1
                    );
                } else {
                    paint.setColor(opts.color);
                }
            }
            if (opts.alpha !== undefined) paint.setAlphaf(opts.alpha);
            if (opts.strokeWidth !== undefined) paint.setStrokeWidth(opts.strokeWidth);
            if (opts.cap) paint.setStrokeCap(opts.cap);
            if (opts.join) paint.setStrokeJoin(opts.join);
            if (opts.dash) paint.setStrokeDash(opts.dash, opts.dashPhase || 0);
            return paint;
        };
        return canvas;
    }

    function useGSAP(build) {
        globalThis.__ocOnFrame(function () {
            var tl = globalThis.ctx.timeline();
            build(tl, globalThis.ctx.currentTime);
        });
    }

    function useFrame(build) {
        globalThis.__ocOnFrame(function () {
            build(globalThis.ctx, globalThis.ctx.currentTime);
        });
    }

    function useCanvasKit() {
        return globalThis.ctx.CanvasKit;
    }

    function useComposition() {
        return globalThis.__ocMeta || {};
    }

    builtins['opencat/runtime'] = {
        h: h,
        Fragment: FRAGMENT,
        useCanvas: useCanvas,
        useGSAP: useGSAP,
        useFrame: useFrame,
        useCanvasKit: useCanvasKit,
        useComposition: useComposition,
    };

    // ── extraction: element tree -> JSONL lines ──────────────────────
    function assignSource(line, props) {
        var src = props.src || props.path || props.url;
        if (src === undefined || src === null) return;
        src = String(src);
        if (/^(https?:)?\/\//.test(src) || src.charAt(0) === '/') line.url = src;
        else line.path = src;
    }

    function emitElement(el, parentId, lines) {
        if (Array.isArray(el)) {
            for (var a = 0; a < el.length; a++) emitElement(el[a], parentId, lines);
            return parentId;
        }
        if (typeof el === 'string' || typeof el === 'number') {
            var textId = 'oc-text-' + (++textCounter);
            if (lines) lines.push({ type: 'text', id: textId, parentId: parentId, text: String(el) });
            return textId;
        }
        if (!el || el.__ocEl !== true) {
            throw new Error('use: JSX tree contains a non-element value (' + typeof el + ')');
        }
        if (typeof el.tag === 'function') {
            // Component element: invoke with its props and splice the result in.
            return emitElement(el.tag(el.props), parentId, lines);
        }
        if (el.tag === FRAGMENT) {
            for (var f = 0; f < el.children.length; f++) emitElement(el.children[f], parentId, lines);
            return parentId;
        }
        var props = el.props || {};
        var id = String(props.id);
        var line = { type: el.tag, id: id };
        if (parentId) line.parentId = parentId;
        var className = props.class || props.className;
        if (className) line.className = className;
        if (props.duration !== undefined && props.duration !== null) line.duration = props.duration;
        switch (el.tag) {
            case 'text': {
                var text = '';
                for (var t = 0; t < el.children.length; t++) {
                    var child = el.children[t];
                    if (typeof child === 'string' || typeof child === 'number') text += String(child);
                }
                line.text = text;
                break;
            }
            case 'path':
                line.d = String(props.d);
                break;
            case 'icon':
                line.icon = String(props.name);
                break;
            case 'image':
            case 'video':
                assignSource(line, props);
                break;
            case 'caption':
                line.path = String(props.src || props.path);
                break;
            case 'audio':
                line.attach = String(props.attach || parentId);
                break;
            default:
                break;
        }
        if (lines) lines.push(line);
        for (var c = 0; c < el.children.length; c++) emitElement(el.children[c], id, lines);
        return id;
    }

    // Walk a rendered tree. With `lines` null this is the per-frame walk: it
    // invokes every component function (so `use*` calls register their drivers)
    // without emitting anything.
    globalThis.__ocWalkTree = function (root, lines) {
        emitElement(root, null, lines || null);
    };

    globalThis.__ocExtract = function (entrySpec) {
        var entryExports = requireModule(entrySpec || ENTRY);
        var meta = entryExports.meta || globalThis.__ocMeta || {};
        var root = entryExports.default;
        if (typeof root === 'function') root = root(entryExports.defaultProps || {});
        if (root === undefined || root === null) {
            throw new Error('use: entry module has no default export');
        }
        var lines = [];
        emitElement(root, null, lines);
        return { meta: meta, lines: lines };
    };
})();
