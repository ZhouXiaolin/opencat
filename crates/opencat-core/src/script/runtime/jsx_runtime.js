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
 *      materializes the returned element tree into a DOM tree (the host hands
 *      it to parse::dom). Runs BEFORE any per-frame driver.
 *
 * `use*` family (v1):
 *   useCanvas(ref, draw)   — per-frame CanvasKit 2D drawing on a <canvas ref>
 *   useThree(ref, build)   — per-frame three.js-like 3D scene on a <canvas ref>
 *   useChart(ref, build)   — per-frame AntV G2 chart on a <canvas ref>
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
            throw new Error('use: unknown module "' + spec + '" (relative paths plus the builtins "opencat/runtime", "three", "canvaskit-wasm" resolve in-engine, and "gsap" (+ gsap/<Plugin> subpaths) is injected by the host from the project\'s dist/gsap.bundle.mjs; any other npm package must be bundled by the vite plugin)');
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
        // `gsap/<Plugin>` subpaths are views over the one project-supplied
        // `gsap` module, not separate modules: requiring the main module
        // first installs the subpath builtins, then the view is served from
        // there. This keeps a single module evaluation per realm.
        if (spec !== 'gsap' && spec.indexOf('gsap/') === 0) {
            requireModule('gsap');
            var subpath = builtins[spec];
            if (!subpath) throw new Error('use: gsap has no subpath "' + spec + '" (not in the project gsap.bundle.mjs export list)');
            return subpath;
        }
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
            // The project-supplied package needs one install pass (target
            // normalization, paused protocol, hook gating, subpath views)
            // before any of its exports are handed to user code. The gated
            // record replaces the raw module exports as the require result —
            // the first caller must not get an ungated surface.
            if (key === 'gsap') {
                installGsapBuiltins(mod.exports);
                return builtins['gsap'];
            }
        } finally {
            currentModule = previous;
        }
        return mod.exports;
    }

    globalThis.__ocRequire = requireModule;

    globalThis.__ocResetFrame = function () {
        // The gsap package module survives frame resets: it is a library, not
        // per-frame state, and re-evaluating ~370KB of package code per frame
        // would only leak one rAF tick into the queue each frame. Tweens and
        // timelines are still created fresh every frame by the hooks.
        var gsapModule = modules['gsap'];
        modules = Object.create(null);
        if (gsapModule) modules['gsap'] = gsapModule;
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
            // The hook gate for the real-package builtins (`gsap` etc.):
            // tween calls are only meaningful inside a per-frame hook.
            var savedHookCtx = globalThis.__ocHookCtx;
            globalThis.__ocHookCtx = ctx;
            try {
                ctx.current = hook.clock || ctx.global;
                setScriptFrame(hook.clock);
                hook.fn(ctx);
                // animate = g(t): tweens/timelines created during the hook
                // body are declarative descriptions. Evaluate them at this
                // hook's clock time — the write-out IS the seek. Idempotent
                // if the author also sought manually.
                var pendingTweens = ctx.__ocGsapPending;
                if (pendingTweens) {
                    ctx.__ocGsapPending = null;
                    var seekTime = Number((ctx.current && ctx.current.time) || 0);
                    for (var s = 0; s < pendingTweens.length; s++) {
                        pendingTweens[s].time(seekTime);
                    }
                }
            } finally {
                globalThis.__ocHookCtx = savedHookCtx;
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

    // useChart(ref, build): the ref must point at a <canvas>; `build(Chart,
    // node, t, ctx)` constructs the chart for THIS frame. Like useCanvas and
    // useThree, the chart is rebuilt every frame — G2's animation timeline runs
    // on the virtual clock the G2 host advances to `t` (`frame = f(t)`), and
    // every draw lands on the node's DrawOp stream.
    function useChart(ref, build) {
        if (!ref || typeof ref !== 'object' || !('current' in ref)) {
            throw new Error('useChart: first argument must be a ref from useRef()');
        }
        globalThis.__ocOnFrame(function () {
            var node = ref.current;
            if (!node || node.__ocEl !== true || node.tag !== 'canvas') {
                throw new Error(
                    'useChart: ref must point at a <canvas> element (got ' +
                    (node && node.tag ? '<' + node.tag + '>' : String(node)) + ')'
                );
            }
            var id = String(node.props.id);
            if (typeof globalThis.__ocG2Frame !== 'function') {
                throw new Error('useChart: the G2 runtime is not installed');
            }
            globalThis.__ocG2Frame(id, globalThis.ctx.current.time, function (Chart, chartNode) {
                build(Chart, chartNode, globalThis.ctx.current.time, globalThis.ctx);
            });
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
        useChart: useChart,
        useGSAP: useGSAP,
        useFrame: useFrame,
        useCanvasKit: useCanvasKit,
        useComposition: useComposition,
    };

    // ── package builtins ────────────────────────────────────────────
    // `import { gsap } from 'gsap'` (oxc lowers to __ocRequire("gsap").gsap)
    // resolves to the REAL npm package, bundled by the example project into
    // dist/gsap.bundle.mjs and injected into __ocModuleSources by the host.
    // three / canvaskit forward to the ctx namespaces, which really are their
    // whole surface. gsap is hook-gated: tween calls only mean something
    // inside a per-frame hook, where the scene clock is bound. Object
    // constructors (three / canvaskit) forward ungated — they are pure local
    // state.
    function hookGated(target, apiName) {
        var wrapped = new WeakMap();
        function wrapValue(v) {
            if (typeof v === 'function') {
                return function () {
                    if (!globalThis.__ocHookCtx) {
                        throw new Error(
                            apiName + ': call it inside useGSAP() / useFrame() — the per-frame hook is where the scene clock is bound'
                        );
                    }
                    return v.apply(this, arguments);
                };
            }
            if (v && typeof v === 'object') {
                if (!wrapped.has(v)) wrapped.set(v, hookGated(v, apiName));
                return wrapped.get(v);
            }
            return v;
        }
        return new Proxy(target, {
            get(t, prop) {
                if (typeof prop !== 'string') return t[prop];
                return wrapValue(t[prop]);
            },
        });
    }

    // Module shape: `import { gsap } from 'gsap'` lowers to
    // __ocRequire("gsap").gsap and `import gsap from 'gsap'` to .default —
    // so the builtin is the package's module record, not the namespace.
    //
    // `gsap` resolves to the REAL npm package (the project's
    // dist/gsap.bundle.mjs, injected by the host). The model stays
    // frame=f(t): the author writes declarative animation —
    // `gsap.timeline().to(...)`, `gsap.from(...)` — and NEVER drives it.
    // Creation registers the tween on the current hook; the hook runner seeks
    // every registered tween to ctx.current.time after the hook body returns
    // (animate = g(t)). The paused flag is forced here so nothing depends on
    // the package's ticker.
    //
    // The package is project-supplied, so it is NOT present at realm
    // bootstrap: requireModule('gsap') evaluates the sidecar module and then
    // calls installGsapBuiltins once. A missing sidecar is a project error
    // (the host reports it when it cannot read the file); there is no
    // fallback surface.
    function installGsapBuiltins(gsapPackage) {
        if (builtins['gsap']) return;
        // animate = g(t) plumbing: register every created tween on the
        // running hook and force `paused` so the package's ticker (wall
        // clock) is never in play. Mutating the real gsap object is safe —
        // its internals reach these entry points through closures, not
        // through the object.
        var realGsap = gsapPackage.gsap || gsapPackage.default;
        var rawTimeline = realGsap.timeline;
        var rawTo = realGsap.to;
        var rawFrom = realGsap.from;
        var rawFromTo = realGsap.fromTo;
        var rawSet = realGsap.set;
        // Targets are node ids — the one id namespace of the composition, same
        // as the ctx.* facade (`gsap.from('box', ...)`). Not CSS selectors:
        // the tree is declarative and there is no live DOM to query.
        //
        // A target resolves to the node's DOM handle, not the numeric
        // ocTarget proxy: the plugin suite decides what to do from the target
        // itself (`CSSPlugin.targetTest` is `target.style && target.nodeType`,
        // MorphSVG reads `target.nodeName`, DrawSVG reads `tagName`), so a
        // plain object would silently route every plugin down its
        // "not an element" branch. The handle writes through to the same
        // mutation store the numeric channel uses.
        function nodeHandle(id) {
            return globalThis.ctx.__domTarget(String(id));
        }
        function resolveTargets(t) {
            if (typeof t === 'string') return nodeHandle(t.charAt(0) === '#' ? t.slice(1) : t);
            if (Array.isArray(t)) {
                for (var i = 0; i < t.length; i++) {
                    if (typeof t[i] === 'string') {
                        return t.map(function (x) {
                            return typeof x === 'string'
                                ? nodeHandle(x.charAt(0) === '#' ? x.slice(1) : x)
                                : x;
                        });
                    }
                }
            }
            return t;
        }
        function registerTween(result) {
            var hook = globalThis.__ocHookCtx;
            if (hook) {
                if (!hook.__ocGsapPending) hook.__ocGsapPending = [];
                hook.__ocGsapPending.push(result);
            }
            return result;
        }
        function withPaused(vars) {
            var merged = {};
            if (vars) {
                for (var k in vars) merged[k] = vars[k];
            }
            merged.paused = true;
            return merged;
        }
        realGsap.timeline = function (vars) {
            return registerTween(rawTimeline.call(this, withPaused(vars)));
        };
        // Only the top-level timeline factory is detached from the package's
        // ticker (paused). Child tweens created via tl.from(...) or gsap.to(...)
        // must NOT be paused themselves — a paused child is skipped by its
        // parent timeline in gsap, which would flatten every duration to 0.
        // A child of a paused timeline never renders on its own, so nothing
        // wall-clock-driven is in play.
        realGsap.to = function (target, vars, position) {
            return registerTween(rawTo.call(this, resolveTargets(target), vars, position));
        };
        realGsap.from = function (target, vars, position) {
            return registerTween(rawFrom.call(this, resolveTargets(target), vars, position));
        };
        realGsap.fromTo = function (target, fromVars, toVars, position) {
            return registerTween(rawFromTo.call(this, resolveTargets(target), fromVars, toVars, position));
        };
        realGsap.set = function (target, vars, position) {
            return registerTween(rawSet.call(this, resolveTargets(target), vars, position));
        };
        // Timeline instance methods are the main authoring surface
        // (`gsap.timeline().from('box', ...)`) — same normalization there.
        // The package exports the class as TimelineLite/TimelineMax only.
        var TimelineCtor = gsapPackage.TimelineLite || gsapPackage.TimelineMax || gsapPackage.Timeline;
        if (TimelineCtor && TimelineCtor.prototype) {
            var proto = TimelineCtor.prototype;
            var wrapVarsMethod = function (name, raw) {
                proto[name] = function (target, vars, position) {
                    return raw.call(this, resolveTargets(target), vars, position);
                };
            };
            wrapVarsMethod('to', proto.to);
            wrapVarsMethod('from', proto.from);
            wrapVarsMethod('set', proto.set);
            var rawFromToProto = proto.fromTo;
            proto.fromTo = function (target, fromVars, toVars, position) {
                return rawFromToProto.call(this, resolveTargets(target), fromVars, toVars, position);
            };
        }
        var gatedGsap = hookGated(gsapPackage.gsap || gsapPackage.default, 'gsap');
        builtins['gsap'] = Object.assign(Object.create(null), gsapPackage, {
            default: gatedGsap,
            gsap: gatedGsap,
        });
        // `import { CustomEase } from 'gsap/CustomEase'` — the npm package's own
        // per-plugin subpaths. Each record carries the plugin as both the named
        // and the default export, so the documented import line works verbatim;
        // the sidecar entry already registered every plugin on the core.
        var gsapSubpaths = [
            'CSSPlugin',
            'CustomEase',
            'CustomWiggle',
            'CustomBounce',
            'SplitText',
            'MorphSVGPlugin',
            'DrawSVGPlugin',
            'MotionPathPlugin',
            'ScrambleTextPlugin',
            'TextPlugin',
            'Flip',
            'Physics2DPlugin',
            'PhysicsPropsPlugin',
        ];
        for (var gi = 0; gi < gsapSubpaths.length; gi++) {
            var gname = gsapSubpaths[gi];
            var gvalue = gsapPackage[gname];
            if (gvalue === undefined) continue;
            var grecord = Object.create(null);
            grecord['default'] = gvalue;
            grecord[gname] = gvalue;
            builtins['gsap/' + gname] = grecord;
        }
        // EasePack exports three eases as one module, not a single plugin.
        var easeBag = Object.create(null);
        easeBag['default'] = { SlowMo: gsapPackage.SlowMo, ExpoScaleEase: gsapPackage.ExpoScaleEase, RoughEase: gsapPackage.RoughEase };
        easeBag['SlowMo'] = gsapPackage.SlowMo;
        easeBag['ExpoScaleEase'] = gsapPackage.ExpoScaleEase;
        easeBag['RoughEase'] = gsapPackage.RoughEase;
        easeBag['EasePack'] = easeBag['default'];
        builtins['gsap/EasePack'] = easeBag;
    }
    builtins['three'] = globalThis.ctx.threejs;
    builtins['canvaskit-wasm'] = {
        default: globalThis.ctx.canvas,
        CanvasKit: globalThis.ctx.canvas,
    };

    // ── ctx.ocTarget: real-gsap capture target ──────────────────────
    // Real gsap on a plain object takes the numeric channel: it writes
    // tweened properties as bare numbers (no CSSPlugin, no transform
    // strings). ocTarget(id) returns a Proxy whose resolvable numeric writes
    // forward into node mutations via the same descriptor.apply surface the
    // facade drives; unresolved properties just sit on the record. Reads fall
    // through to the node's current value (descriptor default), so `.to`
    // start-value semantics hold. Usage inside a hook:
    //   var el = ctx.ocTarget('title');
    //   var tl = gsap.timeline({ paused: true });
    //   tl.from(el, { opacity: 0, y: 48, duration: 0.8 }, 0.2);
    //   tl.time(ctx.global.time);
    function ocTarget(nodeId) {
        var runtime = globalThis.__opencatAnimation;
        var core = runtime.core;
        var target = core.createTarget(String(nodeId));
        var tracks = {};
        var values = {};
        function trackFor(prop) {
            if (tracks[prop] !== undefined) return tracks[prop];
            var descriptor = runtime.animation.resolveProperty(prop);
            if (!descriptor) {
                tracks[prop] = null;
                return null;
            }
            tracks[prop] = {
                descriptor: descriptor,
                name: runtime.animation.canonicalName(prop),
            };
            return tracks[prop];
        }
        return new Proxy(values, {
            get(t, prop) {
                if (typeof prop !== 'string') return t[prop];
                if (prop in t) return t[prop];
                var track = trackFor(prop);
                if (!track) return undefined;
                return core.getDescriptorDefault(track.descriptor, target, track.name, 0);
            },
            set(t, prop, value) {
                if (typeof prop !== 'string') return false;
                t[prop] = value;
                if (typeof value !== 'number' || !isFinite(value)) return true;
                var track = trackFor(prop);
                if (!track) return true;
                track.descriptor.apply(target, value);
                return true;
            },
        });
    }
    ctx.ocTarget = ocTarget;

    // ── extraction: element tree -> DOM tree ─────────────────────────
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

    // `style` attribute → declaration text, the same shape as the HTML
    // attribute. Both forms the React contract allows are accepted: a string
    // (`style="color: red"`) is emitted verbatim, an object
    // (`style={{ color: 'red', '--gap': 4 }}`) is serialized with
    // camelCase→kebab-case and unitless-number rules. The Rust side hands the
    // text to lightningcss, so inline declarations get the full CSS grammar
    // (shorthands, !important, custom properties).
    function styleDeclarations(value) {
        if (value === undefined || value === null || value === '') return undefined;
        if (typeof value === 'string') return value;
        if (typeof value !== 'object') {
            throw new Error('style: expected a string or an object, got ' + typeof value);
        }
        var out = [];
        for (var key in value) {
            if (!Object.prototype.hasOwnProperty.call(value, key)) continue;
            var v = value[key];
            if (v === undefined || v === null || v === false) continue;
            var name = String(key);
            if (name.indexOf('--') !== 0) {
                name = name.replace(/[A-Z]/g, function (c) { return '-' + c.toLowerCase(); });
            }
            var text;
            if (typeof v === 'number') {
                // React appends `px` to a unitless length. These properties are
                // unitless by definition, so the number stays as-is.
                var unitless = /^(opacity|z-?index|flex|flex-grow|flex-shrink|order|font-weight|line-height|aspect-ratio|scale|zoom|tab-size|column-count|orphans|widows|fill-opacity|stroke-opacity|stroke-width|stroke-dashoffset|stroke-dasharray|animation-iteration-count)$/;
                text = unitless.test(name) || v === 0 ? String(v) : v + 'px';
            } else {
                text = String(v);
            }
            out.push(name + ': ' + text);
        }
        return out.length ? out.join('; ') : undefined;
    }

    // A `<tl>` owns the scene clocks. Walk its direct children in order,
    // alternating scenes and transitions (the timeline grammar). A scene runs
    // its component with the scene clock pushed, so its hooks bind to the scene;
    // its `duration` is read from the returned value afterwards. Transitions
    // advance the cursor without owning a clock.
    function emitTimeline(el, audioSites) {
        var props = el.props || {};
        var id = String(props.id);
        var node = { type: 'tl', id: id };
        if (props.class || props.className) node.className = props.class || props.className;
        var tlStyle = styleDeclarations(props.style);
        if (tlStyle) node.style = tlStyle;
        var tlDuration = num(props.duration);
        if (tlDuration !== undefined) node.duration = tlDuration;
        if (props.audioId && audioSites) audioSites.push({ nodeId: id, audioId: String(props.audioId) });

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
        var kids = [];
        for (var k = 0; k < cells.length; k++) {
            var e = cells[k];
            if (e.transition) {
                if (extracting) pushNodes(kids, emitElement(e.el, audioSites));
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
            // The emit runs on the per-frame walk too (nodes then discarded):
            // that walk is what registers the drivers of components nested
            // inside a scene.
            sceneStack.push(makeSceneClock(e.cell));
            try {
                if (e.value === null) {
                    e.value = resolveSceneValue(e.el);
                    stampSceneDuration(e.value, e.declared);
                }
                pushNodes(kids, emitElement(e.value, audioSites));
            } finally {
                sceneStack.pop();
            }
        }
        if (kids.length) node.children = kids;

        return node;
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

    // Element tags the Rust side understands (mirrors `DomElement`). Anything
    // else is an author mistake, reported here instead of as a serde
    // "unknown variant" later.

    // Splice an emitElement result into a parent's children: single node,
    // array (fragment / multi-root component), or nothing.
    function pushNodes(out, value) {
        if (value === null || value === undefined) return;
        if (Array.isArray(value)) {
            for (var i = 0; i < value.length; i++) pushNodes(out, value[i]);
            return;
        }
        out.push(value);
    }
    var SUPPORTED_TAGS = {
        div: true, text: true, canvas: true, image: true, audio: true,
        video: true, transition: true, tl: true,
        path: true, lottie: true, svg: true,
    };

    // 容器标签：孩子的发射结果挂进 node.children；其余标签的孩子仅走一遍
    // （组件函数里的 use* 要登记驱动），节点丢弃 —— 与 Rust 装配一致：只有
    // div/canvas/video/tl 消费子树。
    var CONTAINER_TAGS = { div: true, canvas: true, video: true, tl: true };

    function emitElement(el, audioSites) {
        if (Array.isArray(el)) {
            var out = [];
            for (var a = 0; a < el.length; a++) pushNodes(out, emitElement(el[a], audioSites));
            return out;
        }
        if (typeof el === 'string' || typeof el === 'number') {
            var textId = 'oc-text-' + (++textCounter);
            return { type: 'text', id: textId, text: String(el) };
        }
        if (!el || el.__ocEl !== true) {
            throw new Error('use: JSX tree contains a non-element value (' + typeof el + ')');
        }
        if (typeof el.tag === 'function') {
            // Component element: invoke with its props and splice the result in.
            return emitElement(el.tag(el.props), audioSites);
        }
        if (el.tag === FRAGMENT) {
            var out = [];
            for (var f = 0; f < el.children.length; f++) pushNodes(out, emitElement(el.children[f], audioSites));
            return out;
        }
        if (el.tag === 'tl') {
            return emitTimeline(el, audioSites);
        }
        var props = el.props || {};
        var id = String(props.id);
        // HTML spelling `<img>` reaches the same element as `<image>`.
        var tag = el.tag === 'img' ? 'image' : el.tag;
        var node = { type: tag, id: id };
        // `audioId` attaches a pooled `export const audio` source to wherever it
        // is written: a `<tl>` plays it across the whole timeline, a scene plays
        // it inside that scene. Recorded here, resolved on the Rust side.
        if (props.audioId && audioSites) audioSites.push({ nodeId: id, audioId: String(props.audioId) });
        var className = props.class || props.className;
        if (className) node.className = className;
        var inlineStyle = styleDeclarations(props.style);
        if (inlineStyle) node.style = inlineStyle;
        var duration = num(props.duration);
        if (duration !== undefined) node.duration = duration;
        switch (tag) {
            case 'text': {
                var text = '';
                for (var t = 0; t < el.children.length; t++) {
                    var child = el.children[t];
                    if (typeof child === 'string' || typeof child === 'number') text += String(child);
                }
                node.text = text;
                break;
            }
            case 'path':
                node.d = String(props.d);
                // Shared sibling coordinate space (`view-box="minx,miny,w,h"`).
                // Without it each path is fitted by its own bounds — a small
                // accent shape would be stretched over the whole node box.
                var vb = props['view-box'] !== undefined ? props['view-box'] : props.viewBox;
                if (vb !== undefined && vb !== null && vb !== '') node.viewBox = String(vb);
                break;
            case 'transition':
                node.from = String(props.from);
                node.to = String(props.to);
                node.effect = String(props.effect);
                node.duration = num(props.duration);
                if (props.direction) node.direction = String(props.direction);
                if (props.timing) node.timing = String(props.timing);
                node.damping = num(props.damping);
                node.stiffness = num(props.stiffness);
                node.mass = num(props.mass);
                node.seed = num(props.seed);
                node.hueShift = num(props.hueShift);
                node.maskScale = num(props.maskScale);
                break;
            case 'image':
                assignSource(node, props);
                // Unsplash-style procedural source: `query`/`queryCount`
                // replace a file path, `aspectRatio` pins the crop.
                if (props.query !== undefined && props.query !== null) node.query = String(props.query);
                if (props.queryCount !== undefined && props.queryCount !== null) {
                    node.queryCount = num(props.queryCount);
                }
                if (props.aspectRatio !== undefined && props.aspectRatio !== null) {
                    node.aspectRatio = String(props.aspectRatio);
                }
                break;
            case 'svg':
                // External SVG file (PLAN §4). `src` stays verbatim — the
                // Rust side resolves it against the jsx base_dir itself, so
                // absolute corpus paths are not rewritten to `url`.
                // `inline` carries ready-made svg markup (bundled react icon
                // components serialize their subtree into it); it wins over
                // `src` and skips file resolution on the Rust side.
                if (props.src !== undefined && props.src !== null) node.src = String(props.src);
                if (props.inline !== undefined && props.inline !== null) node.inline = String(props.inline);
                if (props.width !== undefined && props.width !== null) node.width = num(props.width);
                if (props.height !== undefined && props.height !== null) node.height = num(props.height);
                var par = props['preserveAspectRatio'];
                if (par !== undefined && par !== null && par !== '') node.preserveAspectRatio = String(par);
                break;
            case 'video':
                assignSource(node, props);
                // Media window, timing and YUV→RGB matrix override, mirroring
                // the markup `data-*` attributes on <video>. Both the kebab-case
                // markup spelling and the camelCase JSX spelling are accepted on
                // input; emitted nodes always carry the kebab-case keys. Numeric
                // props go through num() so string attributes arrive as numbers.
                var vStart = props['data-start'] !== undefined && props['data-start'] !== null ? props['data-start'] : props.dataStart;
                if (vStart !== undefined && vStart !== null) node['data-start'] = num(vStart);
                var vDur = props['data-duration'] !== undefined && props['data-duration'] !== null ? props['data-duration'] : props.dataDuration;
                if (vDur !== undefined && vDur !== null) node['data-duration'] = num(vDur);
                var vMedia = props['data-media-start'] !== undefined && props['data-media-start'] !== null ? props['data-media-start'] : props.dataMediaStart;
                if (vMedia !== undefined && vMedia !== null) node['data-media-start'] = num(vMedia);
                var vMatrix = props['data-color-matrix'] !== undefined && props['data-color-matrix'] !== null ? props['data-color-matrix'] : props.dataColorMatrix;
                if (vMatrix !== undefined && vMatrix !== null) node['data-color-matrix'] = String(vMatrix);
                var vHide = props['data-hide-after-end'] !== undefined && props['data-hide-after-end'] !== null ? props['data-hide-after-end'] : props.dataHideAfterEnd;
                if (vHide !== undefined && vHide !== null) node['data-hide-after-end'] = vHide === true || vHide === 'true';
                if (props.loop !== undefined && props.loop !== null) {
                    node.loop = props.loop === true || props.loop === 'true';
                }
                break;
            case 'lottie':
                assignSource(node, props);
                break;
            case 'audio':
                // 树内 <audio> 不占视觉位：作为节点挂进容器，Rust 装配期收进
                // 音频池（attach 缺省指向挂载父元素）。
                assignSource(node, props);
                if (props.attach !== undefined && props.attach !== null && props.attach !== '') {
                    node.attach = String(props.attach);
                }
                break;
            default:
                if (SUPPORTED_TAGS[tag] !== true) {
                    throw new Error(
                        'use: unsupported element <' + tag + '>; supported: ' +
                        Object.keys(SUPPORTED_TAGS).join(', ')
                    );
                }
                break;
        }
        var kids = [];
        for (var c = 0; c < el.children.length; c++) {
            pushNodes(kids, emitElement(el.children[c], audioSites));
        }
        if (kids.length && CONTAINER_TAGS[tag] === true) node.children = kids;
        return node;
    }

    // Walk a rendered tree for side effects only: it invokes every component
    // function (so `use*` calls register their drivers); the produced nodes
    // are discarded. Per-frame replay walks this way.
    globalThis.__ocWalkTree = function (root) {
        emitElement(root, null);
    };

    // Entry convention: `export function main()` (the multi-file layout puts
    // the composition root in main) takes precedence; `export default` stays
    // the single-file spelling. Both may name a component OR an element.
    function resolveEntryRoot(entryExports) {
        var root = typeof entryExports.main === 'function'
            ? entryExports.main(entryExports.defaultProps || {})
            : entryExports.default;
        if (typeof root === 'function') root = root(entryExports.defaultProps || {});
        if (root === undefined || root === null) {
            throw new Error('use: entry module has no main() or default export');
        }
        return root;
    }

    globalThis.__ocExtract = function (entrySpec) {
        var entryExports = requireModule(entrySpec || ENTRY);
        var meta = entryExports.meta || globalThis.__ocMeta || {};
        var root = resolveEntryRoot(entryExports);
        var audioSites = [];
        var tree = [];
        pushNodes(tree, emitElement(root, audioSites));
        // `export const css`：打包器把编译好的样式表嵌进 bundle，是样式的
        // 唯一来源；没有则 class 不产生样式。
        return { meta: meta, fonts: entryExports.fonts, audio: entryExports.audio, audioSites: audioSites, tree: tree, css: entryExports.css };
    };
})();
