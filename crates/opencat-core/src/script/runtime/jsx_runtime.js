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
 *   useCanvas(ref, draw)   — per-frame CanvasKit 2D drawing on a <canvas ref>
 *   useThree(ref, build)   — per-frame three.js-like 3D scene on a <canvas ref>
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
    // Compiled module factories, keyed by module key. `new Function(source)`
    // compiles the whole module body, and the module graph is immutable for a
    // pipeline's lifetime, so we compile once and reuse the factory across
    // frames. `factorySource` detects a changed source (defensive: a host that
    // re-pushes module sources) and forces a recompile.
    var factories = Object.create(null);
    var factorySource = Object.create(null);
    var hooks = [];
    // Scene clock stack. Inside a `<tl>`, each scene pushes its own clock
    // (`{ time, duration, frame }`, scene-local) while its subtree is walked,
    // so every `use*` registers against the scene it lives in. Outside a
    // timeline the stack is empty and hooks fall back to `ctx.global`.
    var sceneStack = [];
    var idCounter = 0;
    var textCounter = 0;
    var currentModule = null;

    // ── scene clocks ────────────────────────────────────────────────
    // Inside a `<tl>`, each scene owns a clock. It mirrors Rust's
    // `ScriptFrameCtx::for_segment`: scene-local frame counts are quantized by
    // duration and CLAMPED — 0 before the scene starts, frozen on its last
    // frame once the playhead passes it. So a scene's hook always reads its own
    // clock, whatever frame is being rendered.
    function durationToFrames(dur, fps) {
        if (!isFinite(dur) || dur <= 0) return 0;
        return Math.max(1, Math.ceil(dur * fps - 1e-6));
    }

    function metaFps() {
        var meta = globalThis.__ocMeta || {};
        var fps = Number(meta.fps);
        return isFinite(fps) && fps > 0 ? fps : 30;
    }

    function makeSceneClock(cell) {
        function localFrame() {
            var g = globalThis.ctx.global;
            var cur = g.frame - cell.startFrame;
            if (cur < 0) cur = 0;
            return cur;
        }
        return {
            // Where this scene starts on the composition clock. A `<tl>` nested
            // inside this scene places its own segments relative to here, so its
            // scenes run clocks from their own segment starts — `ctx.current.time`
            // is the component's own lifetime at any depth. Mirrors Rust's
            // `ScriptFrameCtx::scene_start_frame` / `scene_start_secs`.
            startFrame: cell.startFrame,
            startSecs: cell.startSecs,
            // Seconds come from the scene's exact start, not from the frame
            // index: a boundary is quantized to a frame while the author's
            // placement is not, and that error would otherwise accumulate down
            // the timeline. Mirrors Rust's `ScriptFrameCtx::for_window`.
            get time() { return Math.max(0, globalThis.ctx.global.time - cell.startSecs); },
            get duration() { return cell.sceneFrames / cell.fps; },
            get frame() { return localFrame(); },
            // A scene is on screen for its whole window — its own segment plus
            // the transitions it shares with its neighbours (a transition IS the
            // overlap between two scenes). Outside that window it is not
            // rendered, so its hooks must not run.
            get active() {
                if (cell.sceneFrames <= 0) return false;
                var cur = globalThis.ctx.global.frame - cell.startFrame;
                return cur >= 0 && cur < cell.sceneFrames;
            },
        };
    }

    // Tween progress is computed in Rust. Tell it which clock to use: the
    // scene-local one when a clock is given, the composition clock otherwise.
    // Every hook and every timeline flush brackets itself with this so a scene's
    // tweens advance on the scene's clock, not the composition's.
    //
    // Time is sent in SECONDS, not frames. Tween progress advances by elapsed
    // time, and a scene window can start between frames (a `<tl>` boundary is
    // quantized to a frame; the author's placement is not). Rounding the scene
    // start to a frame would run every tween in that scene that much early.
    // `__ocScriptFrame` still mirrors the whole-frame reading for scripts that
    // index by frame, and for the save/restore bracket around nested flushes.
    function setScriptFrame(clock) {
        var frame = clock ? clock.frame : -1;
        var secs = clock ? clock.time : -1;
        globalThis.__ocScriptFrame = frame;
        globalThis.__ocScriptTimeSecs = secs;
        if (globalThis.__set_script_frame) globalThis.__set_script_frame(frame);
        if (globalThis.__set_script_time_secs) globalThis.__set_script_time_secs(secs);
    }
    globalThis.__ocSetScriptFrame = setScriptFrame;

    // Call through component wrappers to the value a scene renders. Calling a
    // component is what registers its hooks, so this MUST run with the scene's
    // clock already pushed onto `sceneStack`.
    function resolveSceneValue(el) {
        var guard = 0;
        while (guard++ < 64) {
            if (el && el.__ocEl === true && typeof el.tag === 'function') {
                el = el.tag(el.props);
                continue;
            }
            break;
        }
        return el;
    }

    // The `duration` of the single element a `<tl>` child renders. A `<tl>`
    // child is one timeline segment, so it MUST render exactly one element with
    // a duration — a fragment or array wrapping several roots has no
    // well-defined segment length, and silently reading the first root's
    // duration would shift every later scene's start frame. Unwraps arrays and
    // fragments WITHOUT calling a component, so a component is never invoked
    // twice; components are already unwrapped by `resolveSceneValue`.
    function sceneDuration(el) {
        function multiRoot() {
            throw new Error(
                'use: a <tl> child must render exactly one element (a scene renders one root)'
            );
        }
        var guard = 0;
        while (guard++ < 64) {
            if (Array.isArray(el)) {
                if (el.length !== 1) multiRoot();
                el = el[0];
                continue;
            }
            if (el && el.__ocEl === true && el.tag === FRAGMENT) {
                if (el.children.length !== 1) multiRoot();
                el = el.children[0];
                continue;
            }
            break;
        }
        if (!el || el.__ocEl !== true) multiRoot();
        var duration = num(el.props && el.props.duration);
        if (duration === undefined) {
            throw new Error(
                'use: <tl> sequence `' + String(el.props && el.props.id) +
                '` is missing a duration'
            );
        }
        return duration;
    }

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

    function useRef(initial) {
        return { current: initial === undefined ? null : initial };
    }

    function h(tag, props) {
        var children = [];
        for (var i = 2; i < arguments.length; i++) flattenInto(arguments[i], children);
        props = props || {};
        if (props.id === undefined || props.id === null || props.id === '') {
            props.id = 'oc-' + (++idCounter);
        }
        var el = { __ocEl: true, tag: tag, props: props, children: children };
        if (props.ref && typeof props.ref === 'object' && 'current' in props.ref) {
            props.ref.current = el;
        }
        var name = String(tag);
        var isComponent = typeof tag === 'function' || /^[A-Z]/.test(name) || name.indexOf('.') >= 0;
        if (isComponent && children.length > 0) props.children = children;
        return el;
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
            var source = sources[key];
            var fn = factories[key];
            if (fn === undefined || factorySource[key] !== source) {
                fn = new Function('__ocRequire', 'module', 'exports', source);
                factories[key] = fn;
                factorySource[key] = source;
            }
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
        sceneStack.length = 0;
        idCounter = 0;
        textCounter = 0;
    };

    // Register a per-frame hook. The scene clock in effect right now (the
    // scene whose subtree is being walked) is captured with the hook, so the
    // hook can be replayed later against its own scene rather than whichever
    // scene the frame happens to render.
    globalThis.__ocOnFrame = function (fn) {
        hooks.push({
            fn: fn,
            clock: sceneStack.length ? sceneStack[sceneStack.length - 1] : null,
        });
    };

    globalThis.__ocRunFrameHooks = function () {
        var ctx = globalThis.ctx;
        for (var i = 0; i < hooks.length; i++) {
            var hook = hooks[i];
            // A hook bound to a scene only runs while that scene is on screen.
            // Off screen the scene is not rendered, so its work is pure waste —
            // this is what keeps a long timeline from re-running every scene's
            // driver on every frame.
            if (hook.clock && hook.clock.active === false) continue;
            var saved = ctx.current;
            var savedFrame = globalThis.__ocScriptFrame;
            var savedTime = globalThis.__ocScriptTimeSecs;
            try {
                ctx.current = hook.clock || ctx.global;
                setScriptFrame(hook.clock);
                hook.fn(ctx);
            } finally {
                ctx.current = saved;
                globalThis.__ocScriptFrame = savedFrame;
                globalThis.__ocScriptTimeSecs = savedTime;
                if (globalThis.__set_script_frame) {
                    globalThis.__set_script_frame(savedFrame != null ? savedFrame : -1);
                }
                if (globalThis.__set_script_time_secs) {
                    globalThis.__set_script_time_secs(savedTime != null ? savedTime : -1);
                }
            }
        }
    };

    // ── use* registry ───────────────────────────────────────────────
    function useCanvas(ref, draw) {
        if (!ref || typeof ref !== 'object' || !('current' in ref)) {
            throw new Error('useCanvas: first argument must be a ref from useRef()');
        }
        globalThis.__ocOnFrame(function () {
            var node = ref.current;
            if (!node || node.__ocEl !== true || node.tag !== 'canvas') {
                throw new Error(
                    'useCanvas: ref must point at a <canvas> element (got ' +
                    (node && node.tag ? '<' + node.tag + '>' : String(node)) + ')'
                );
            }
            var id = String(node.props.id);
            var canvas = globalThis.ctx.getCanvasById(id);
            augmentCanvas(canvas, id);
            draw(canvas, globalThis.ctx.current.time, globalThis.ctx);
        });
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

    // useThree(ref, build): the ref must point at a <canvas>; `build` receives
    // (three, node, t, ctx) and owns the whole scene graph + render call, the
    // same body a `<script>` block would run.
    function useThree(ref, build) {
        if (!ref || typeof ref !== 'object' || !('current' in ref)) {
            throw new Error('useThree: first argument must be a ref from useRef()');
        }
        globalThis.__ocOnFrame(function () {
            var node = ref.current;
            if (!node || node.__ocEl !== true || node.tag !== 'canvas') {
                throw new Error(
                    'useThree: ref must point at a <canvas> element (got ' +
                    (node && node.tag ? '<' + node.tag + '>' : String(node)) + ')'
                );
            }
            build(globalThis.ctx.three, node, globalThis.ctx.current.time, globalThis.ctx);
        });
    }

    // useGSAP(build): build(ctx, t) — author picks ctx.to/fromTo (facade) or
    // ctx.timeline(). Entries are re-recorded every frame and flushed after
    // the driver, exactly like the XML <script> block.
    function useGSAP(build) {
        globalThis.__ocOnFrame(function () {
            build(globalThis.ctx, globalThis.ctx.current.time);
        });
    }

    function useFrame(build) {
        globalThis.__ocOnFrame(function () {
            build(globalThis.ctx, globalThis.ctx.current.time);
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
        useRef: useRef,
        Fragment: FRAGMENT,
        useCanvas: useCanvas,
        useThree: useThree,
        useGSAP: useGSAP,
        useFrame: useFrame,
        useCanvasKit: useCanvasKit,
        useComposition: useComposition,
    };

    // ── extraction: element tree -> JSONL lines ──────────────────────
    // JSX string attributes ("3.6") must arrive at the Rust side as numbers.
    function num(value) {
        if (value === undefined || value === null || value === '') return undefined;
        var n = Number(value);
        if (!isFinite(n)) throw new Error('use: expected a number, got ' + JSON.stringify(value));
        return n;
    }

    function assignSource(line, props) {
        var src = props.src || props.path || props.url;
        if (src === undefined || src === null) return;
        src = String(src);
        if (/^(https?:)?\/\//.test(src) || src.charAt(0) === '/') line.url = src;
        else line.path = src;
    }

    // A `<tl>` owns the scene clocks. Walk its direct children in order,
    // alternating scenes and transitions (the timeline grammar). A scene runs
    // its component with the scene clock pushed, so its hooks bind to the scene;
    // its `duration` is read from the returned value afterwards. Transitions
    // advance the cursor without owning a clock.
    function emitTimeline(el, parentId, lines, audioSites) {
        var props = el.props || {};
        var id = String(props.id);
        var line = { type: 'tl', id: id };
        if (parentId) line.parentId = parentId;
        if (props.class || props.className) line.className = props.class || props.className;
        var tlDuration = num(props.duration);
        if (tlDuration !== undefined) line.duration = tlDuration;
        if (props.audioId && audioSites) audioSites.push({ nodeId: id, audioId: String(props.audioId) });
        if (lines) lines.push(line);

        var fps = metaFps();
        var extracting = globalThis.__ocMode === 'extract';
        var children = el.children || [];

        // A timeline places its segments on its OWN clock. Top level starts at
        // composition zero; nested inside a scene it starts where that scene
        // does, so the scenes it owns run their clocks from their own segment
        // starts — `ctx.current.time` is the component's own lifetime at any
        // depth. The enclosing scene's clock is exactly that origin, and it is
        // already on top of `sceneStack` while this subtree is walked. Mirrors
        // Rust's `TimelineOrigin`.
        var enclosing = sceneStack.length ? sceneStack[sceneStack.length - 1] : null;
        var originFrame = enclosing ? enclosing.startFrame : 0;
        var originSecs = enclosing ? enclosing.startSecs : 0;

        // Pass 1: lay out the segments. A scene's length is its `duration` prop
        // when declared on the `<tl>` child — no need to run the component to
        // measure it, which is what lets an off-screen scene be skipped later.
        // A scene that does NOT declare one must be run here to measure its
        // root's duration; such a scene is always rendered (it can never be
        // proven off screen), so it is resolved once and its value cached.
        // Transitions always declare their own duration.
        //
        // Boundaries come from a SECONDS cursor, and a segment's frame length is
        // the difference of two ceiled boundaries. Summing per-segment ceils
        // instead would drift one frame later at every fractional boundary, so a
        // long timeline would end up seconds off from where the author put it.
        // This mirrors Rust's `TimelineNode::segment_frame_bounds`.
        var cells = [];
        var cursorSecs = 0;
        for (var i = 0; i < children.length; i++) {
            var child = children[i];
            if (child && child.__ocEl === true && child.tag === 'transition') {
                var tStart = cursorSecs;
                var tDuration = num(child.props.duration) || 0;
                cursorSecs += tDuration;
                cells.push({
                    transition: true,
                    startFrame: originFrame + durationToFrames(tStart, fps),
                    durationSecs: tDuration,
                    frames: durationToFrames(cursorSecs, fps) - durationToFrames(tStart, fps),
                    el: child,
                });
                continue;
            }
            var declared = child && child.__ocEl === true ? num(child.props.duration) : undefined;
            var cell = {
                startFrame: originFrame + durationToFrames(cursorSecs, fps),
                startSecs: originSecs + cursorSecs,
                sceneFrames: 0,
                fps: fps,
            };
            var entry = { transition: false, cell: cell, el: child, declared: declared, value: null };
            if (declared !== undefined) {
                cursorSecs += declared;
            } else {
                sceneStack.push(makeSceneClock(cell));
                try {
                    entry.value = resolveSceneValue(child);
                    cursorSecs += sceneDuration(entry.value);
                } finally {
                    sceneStack.pop();
                }
            }
            // Length is the difference of two ceiled LOCAL boundaries, so the
            // origin cancels out.
            cell.sceneFrames = durationToFrames(cursorSecs, fps) + originFrame - cell.startFrame;
            if (entry.value !== null) stampSceneDuration(entry.value, declared);
            cells.push(entry);
        }

        // Pass 2: widen each scene's window by the transition segments that
        // touch it. A transition IS the overlap between its two scenes, so both
        // neighbours stay live for its whole span and each runs its clock from
        // its own window start. The clocks read `ctx.global.frame` lazily, so
        // filling their windows in after the walk is fine — no hook has run yet.
        // Widening stops at the timeline's own origin: a nested timeline's first
        // scene must not reach back before the enclosing scene began.
        for (var j = 0; j < cells.length; j++) {
            if (cells[j].transition) continue;
            var before = j > 0 && cells[j - 1].transition ? cells[j - 1].frames : 0;
            var after =
                j + 1 < cells.length && cells[j + 1].transition ? cells[j + 1].frames : 0;
            var beforeSecs = j > 0 && cells[j - 1].transition ? cells[j - 1].durationSecs : 0;
            var sceneCell = cells[j].cell;
            sceneCell.startFrame = Math.max(originFrame, sceneCell.startFrame - before);
            sceneCell.startSecs = Math.max(originSecs, sceneCell.startSecs - beforeSecs);
            sceneCell.sceneFrames = before + sceneCell.sceneFrames + after;
        }

        // Pass 3: walk the children in order. A scene is only resolved while it
        // is on screen — its component runs, which is what registers its hooks.
        // Off screen it is skipped outright (extract must emit the whole tree,
        // so it resolves everything). This is what keeps a long timeline from
        // paying for every scene on every frame.
        var frame = globalThis.ctx && globalThis.ctx.global ? globalThis.ctx.global.frame : 0;
        for (var k = 0; k < cells.length; k++) {
            var e = cells[k];
            if (e.transition) {
                if (lines) emitElement(e.el, id, lines, audioSites);
                continue;
            }
            if (e.value === null) {
                if (!extracting && e.cell.sceneFrames > 0) {
                    var local = frame - e.cell.startFrame;
                    if (local < 0 || local >= e.cell.sceneFrames) continue;
                }
            }
            // The scene clock stays pushed through the EMIT, not just the
            // resolve. Emitting walks the scene's subtree, which is where a
            // nested `<tl>` reads the origin it places its own segments on, and
            // where any component still inside the subtree registers its hooks.
            // Popping before the emit would bind both to the outer timeline.
            //
            // The emit runs even when `lines` is null: that is the per-frame
            // walk, and it is what registers the drivers of components nested
            // inside a scene. `emitElement` only pushes a line when `lines` is
            // non-null, so a null walk is side-effect-free apart from those
            // registrations.
            sceneStack.push(makeSceneClock(e.cell));
            try {
                if (e.value === null) {
                    e.value = resolveSceneValue(e.el);
                    stampSceneDuration(e.value, e.declared);
                }
                emitElement(e.value, id, lines, audioSites);
            } finally {
                sceneStack.pop();
            }
        }

        return id;
    }

    // Stamp the declared scene duration onto the root the scene renders, so the
    // JSONL line carries it (Rust reads the segment length off the line, not
    // off the `<tl>` child's props). Unwraps arrays and fragments without
    // re-running the component.
    function stampSceneDuration(value, duration) {
        var guard = 0;
        while (guard++ < 64) {
            if (Array.isArray(value)) {
                if (value.length !== 1) return;
                value = value[0];
                continue;
            }
            if (value && value.__ocEl === true && value.tag === FRAGMENT) {
                if (value.children.length !== 1) return;
                value = value.children[0];
                continue;
            }
            break;
        }
        if (!value || value.__ocEl !== true) return;
        if (value.props && value.props.duration === undefined) value.props.duration = duration;
    }

    function emitElement(el, parentId, lines, audioSites) {
        if (Array.isArray(el)) {
            for (var a = 0; a < el.length; a++) emitElement(el[a], parentId, lines, audioSites);
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
            return emitElement(el.tag(el.props), parentId, lines, audioSites);
        }
        if (el.tag === FRAGMENT) {
            for (var f = 0; f < el.children.length; f++) emitElement(el.children[f], parentId, lines, audioSites);
            return parentId;
        }
        if (el.tag === 'tl') {
            return emitTimeline(el, parentId, lines, audioSites);
        }
        var props = el.props || {};
        var id = String(props.id);
        var line = { type: el.tag, id: id };
        if (parentId) line.parentId = parentId;
        // `audioId` attaches a pooled `export const audio` source to wherever it
        // is written: a `<tl>` plays it across the whole timeline, a scene plays
        // it inside that scene. Recorded here, resolved on the Rust side.
        if (props.audioId && audioSites) audioSites.push({ nodeId: id, audioId: String(props.audioId) });
        var className = props.class || props.className;
        if (className) line.className = className;
        var duration = num(props.duration);
        if (duration !== undefined) line.duration = duration;
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
                // Shared sibling coordinate space (`view-box="minx,miny,w,h"`).
                // Without it each path is fitted by its own bounds — a small
                // accent shape would be stretched over the whole node box.
                var vb = props['view-box'] !== undefined ? props['view-box'] : props.viewBox;
                if (vb !== undefined && vb !== null && vb !== '') line.viewBox = String(vb);
                break;
            case 'icon':
                line.icon = String(props.icon || props.name);
                break;
            case 'transition':
                line.from = String(props.from);
                line.to = String(props.to);
                line.effect = String(props.effect);
                line.duration = num(props.duration);
                if (props.direction) line.direction = String(props.direction);
                if (props.timing) line.timing = String(props.timing);
                line.damping = num(props.damping);
                line.stiffness = num(props.stiffness);
                line.mass = num(props.mass);
                line.seed = num(props.seed);
                line.hueShift = num(props.hueShift);
                line.maskScale = num(props.maskScale);
                break;
            case 'image':
                assignSource(line, props);
                break;
            case 'lottie':
                assignSource(line, props);
                break;
            case 'video':
                assignSource(line, props);
                // Media window, timing and YUV→RGB matrix override, mirroring
                // the markup `data-*` attributes on <video>. Both the kebab-case
                // markup spelling and the camelCase JSX spelling are accepted on
                // input; emitted lines always carry the kebab-case keys (the
                // JSONL parser aliases the camelCase spellings). Numeric props
                // go through num() so string attributes arrive as numbers.
                var vStart = props['data-start'] !== undefined && props['data-start'] !== null ? props['data-start'] : props.dataStart;
                if (vStart !== undefined && vStart !== null) line['data-start'] = num(vStart);
                var vDur = props['data-duration'] !== undefined && props['data-duration'] !== null ? props['data-duration'] : props.dataDuration;
                if (vDur !== undefined && vDur !== null) line['data-duration'] = num(vDur);
                var vMedia = props['data-media-start'] !== undefined && props['data-media-start'] !== null ? props['data-media-start'] : props.dataMediaStart;
                if (vMedia !== undefined && vMedia !== null) line['data-media-start'] = num(vMedia);
                var vMatrix = props['data-color-matrix'] !== undefined && props['data-color-matrix'] !== null ? props['data-color-matrix'] : props.dataColorMatrix;
                if (vMatrix !== undefined && vMatrix !== null) line['data-color-matrix'] = String(vMatrix);
                var vHide = props['data-hide-after-end'] !== undefined && props['data-hide-after-end'] !== null ? props['data-hide-after-end'] : props.dataHideAfterEnd;
                if (vHide !== undefined && vHide !== null) line['data-hide-after-end'] = vHide === true || vHide === 'true';
                if (props.loop !== undefined && props.loop !== null) {
                    line.loop = props.loop === true || props.loop === 'true';
                }
                break;
            case 'caption':
                line.path = String(props.src || props.path);
                break;
            case 'audio':
                // jsonl Audio accepts path/url (parse/jsonl/mod.rs Audio) — the
                // JSX frontend must resolve the source the same way video/image
                // do, otherwise every JSX audio is silently sourceless.
                assignSource(line, props);
                line.attach = String(props.attach || parentId);
                if (props.duration !== undefined && props.duration !== null) {
                    line.duration = num(props.duration);
                }
                break;
            default:
                break;
        }
        if (lines) lines.push(line);
        for (var c = 0; c < el.children.length; c++) emitElement(el.children[c], id, lines, audioSites);
        return id;
    }

    // Walk a rendered tree. With `lines` null this is the per-frame walk: it
    // invokes every component function (so `use*` calls register their drivers)
    // without emitting anything.
    globalThis.__ocWalkTree = function (root, lines, audioSites) {
        emitElement(root, null, lines || null, audioSites || null);
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
        var audioSites = [];
        emitElement(root, null, lines, audioSites);
        return { meta: meta, fonts: entryExports.fonts, audio: entryExports.audio, audioSites: audioSites, lines: lines };
    };
})();
