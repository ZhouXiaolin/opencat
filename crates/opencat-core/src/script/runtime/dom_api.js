// DOM globals — `document` and `window`, plus clear errors for the browser
// APIs a deterministic renderer cannot support.
//
// The tree is declarative: `document.getElementById` returns the SAME handle
// `ctx.getNode` hands out (lazily — an id works before its element is
// created, matching how per-frame drivers replay hooks in order). Node
// creation and tree mutation go through the script-side overlay
// (`tree_api.js`), so official plugins can split text and move children.
// Timers and `requestAnimationFrame` stay hard errors that point at the
// supported alternative, so real-package code fails loudly instead of
// silently diverging.
(function () {
    'use strict';

    if (typeof globalThis.window === 'undefined' || !globalThis.window) {
        globalThis.window = {
            get innerWidth() {
                var g = globalThis.ctx && globalThis.ctx.global;
                return g && Number.isFinite(Number(g.width)) ? Number(g.width) : 0;
            },
            get innerHeight() {
                var g = globalThis.ctx && globalThis.ctx.global;
                return g && Number.isFinite(Number(g.height)) ? Number(g.height) : 0;
            },
            devicePixelRatio: 1,
        };
        // Bundles commonly alias these at eval time.
        globalThis.window.self = globalThis.window;
    }

    function unknownId(api, id) {
        throw new Error(api + ": unknown id '" + String(id) + "'");
    }

    function nodeFor(id, api) {
        var registry = (globalThis.ctx && globalThis.ctx.__targetRegistry) || {};
        if (registry.visual && registry.visual[id]) return globalThis.ctx.getNode(id);
        if (registry.nonVisual && registry.nonVisual[id]) {
            throw new Error(api + ": non-visual id '" + id + "' cannot be targeted");
        }
        return unknownId(api, id);
    }

    // `querySelector` supports the `#id` form only — a class or combinator
    // selector needs a live layout tree, which a declarative composition
    // never materializes as a mutable DOM.
    function selectorToId(api, selector) {
        var m = /^#\S+$/.exec(String(selector).trim());
        if (!m) {
            throw new Error(api + ": only '#id' selectors are supported, got '" + String(selector) + "'");
        }
        return selector.trim().slice(1);
    }

    // Queries return the element-face handle (`ctx.__domTarget`), not the
    // chainable author handle `ctx.getNode` hands out. Plugins compare the
    // element they queried against the tween target by identity — MorphSVG
    // reads `data-original` back only when `e === target` — and the two
    // handles must stay distinct (see node_style.js), so the query has to
    // answer with the same object gsap was handed as the target.
    function elementFor(id, api) {
        nodeFor(id, api);
        return globalThis.ctx.__domTarget(id, { allowUnknown: true });
    }

    function declarativeOnly(api) {
        return function () {
            throw new Error(
                api + ': the element tree is declarative and static; build the structure in JSX and animate with gsap'
            );
        };
    }

    // ── Document root ───────────────────────────────────────────────
    // Plugins walk UP from an element to the document root (`_setDoc` does
    // `doc.parentNode` until null, then calls `createElementNS` on whatever it
    // reached), so the root is an object that owns element creation. Node
    // creation itself goes to the script-side overlay (`tree_api.js`) — the
    // root only has to be a place to create FROM and a `body` to append to.
    // The root's own style declaration. Nothing is laid out through it, but
    // gsap feature-detects on `document.body` — `_setDoc` does
    // `'transform' in body.style` — so it answers for every name the browser
    // knows and stores whatever is assigned.
    function rootStyle() {
        var values = Object.create(null);
        return new Proxy(values, {
            get(target, prop) {
                if (typeof prop !== 'string') return target[prop];
                if (prop === 'cssText') return values.cssText || '';
                if (prop === 'setProperty') {
                    return function (name, value) { values[name] = String(value); };
                }
                if (prop === 'getPropertyValue') {
                    return function (name) {
                        var v = values[name];
                        return v === undefined ? '' : String(v);
                    };
                }
                if (prop === 'removeProperty') return function () { return ''; };
                var v = values[prop];
                return v === undefined ? '' : v;
            },
            set(target, prop, value) {
                if (typeof prop !== 'string') return false;
                values[prop] = value;
                return true;
            },
            has() { return true; },
        });
    }

    function rootElement() {
        var root = {
            nodeType: 9,
            nodeName: '#document',
            parentNode: null,
            style: rootStyle(),
            createElement(tag) { return globalThis.__ocTree.createElement(tag); },
            createElementNS(_ns, tag) { return globalThis.__ocTree.createElement(tag); },
            createTextNode(text) { return globalThis.__ocTree.createTextNode(String(text)); },
            appendChild(child) { return child; },
            removeChild(child) { return child; },
        };
        root.body = root;
        root.documentElement = root;
        return root;
    }

    // `getComputedStyle(el)` for a composition node is that node's style
    // handle — the same view `el.style` gives, which is the only style state a
    // declarative composition has. A scratch element gets its own plain bag.
    function elementIdFor(node) {
        if (!node) return null;
        if (typeof node === 'string') return node.charAt(0) === '#' ? node.slice(1) : node;
        var registry = (globalThis.ctx && globalThis.ctx.__targetRegistry) || {};
        if (registry.visual && registry.visual[node.id]) return node.id;
        return null;
    }

    // Computed-style defaults a declarative composition has no cascade for.
    // Plugins branch on these: SplitText reads `display` (inline elements get
    // `inline-block` so transforms apply) and `textAlign`/`whiteSpace`; MorphSVG
    // reads `fill`/`fillRule` to decide whether a fill-safe morph is needed.
    // `opacity` matters for `.from()`: gsap takes the tween's END value from the
    // computed style, and an empty string parses as 0 — which would make
    // `from({opacity: 0})` fade to nothing instead of to the element's own 1.
    var COMPUTED_DEFAULTS = {
        display: 'block',
        textAlign: 'left',
        whiteSpace: 'normal',
        position: 'static',
        opacity: '1',
        fill: 'rgb(0, 0, 0)',
        fillRule: 'nonzero',
        stroke: 'none',
        visibility: 'visible',
        overflow: 'visible',
        transformOrigin: '50% 50%',
    };

    function withComputedDefaults(style) {
        return new Proxy(style, {
            get(target, prop) {
                if (typeof prop !== 'string') return target[prop];
                var v = target[prop];
                if (v !== null && v !== undefined && v !== '') return v;
                if (Object.prototype.hasOwnProperty.call(COMPUTED_DEFAULTS, prop)) {
                    return COMPUTED_DEFAULTS[prop];
                }
                return v;
            },
        });
    }

    function getComputedStyle(node) {
        var id = elementIdFor(node);
        if (id) return withComputedDefaults(globalThis.ctx.getNode(id).style);
        if (node && node.style) return withComputedDefaults(node.style);
        return withComputedDefaults({});
    }

    var ROOT = rootElement();

    var document = {
        getElementById(id) {
            return nodeFor(String(id), 'document.getElementById');
        },
        querySelector(selector) {
            return elementFor(selectorToId('document.querySelector', selector), 'document.querySelector');
        },
        querySelectorAll(selector) {
            return [elementFor(selectorToId('document.querySelectorAll', selector), 'document.querySelectorAll')];
        },
        createElement(tag) {
            return globalThis.__ocTree.createElement(tag);
        },
        // Plugins reach for `createElementNS` first (`_createElement` in
        // CSSPlugin) and only fall back to `createElement`. There is no
        // namespace in a declarative composition, so both make a scratch
        // element.
        createElementNS(_ns, tag) {
            return globalThis.__ocTree.createElement(tag);
        },
        createTextNode(text) {
            return globalThis.__ocTree.createTextNode(String(text));
        },
        addEventListener: declarativeOnly('document.addEventListener'),
        // SplitText's `autoSplit` re-splits on `loadingdone`. The composition
        // ships its fonts with the bundle, so no load event ever fires; the
        // subscription just has to exist and be removable.
        fonts: {
            addEventListener: function () {},
            removeEventListener: function () {},
            ready: Promise.resolve(),
            check: function () { return true; },
            load: function () { return Promise.resolve([]); },
        },
        body: ROOT,
        documentElement: ROOT,
    };

    // The G2 shim installs a `document` before this runtime runs (it owns
    // canvas creation). Its `body`/`documentElement` are G2 chart elements
    // with no `createElementNS`, and plugins treat `document.body` as the
    // document root (`_setDoc` in Flip walks `ownerDocument || element`). So
    // the DOM-shaped members are overwritten, not merely filled in.
    if (typeof globalThis.document === 'undefined' || !globalThis.document) {
        globalThis.document = document;
    } else {
        var OVERWRITE = {
            body: true,
            documentElement: true,
            createElement: true,
            createElementNS: true,
            createTextNode: true,
            getElementById: true,
            querySelector: true,
            querySelectorAll: true,
            fonts: true,
        };
        for (var key in document) {
            if (!OVERWRITE[key] && globalThis.document[key] !== undefined && globalThis.document[key] !== null) {
                continue;
            }
            // A real browser `Document` exposes `body`/`documentElement` as
            // getter-only prototype accessors, so plain assignment throws.
            // An own property shadows the accessor, which is what plugins
            // need: `document.body` must be the composition root, not the
            // live page body.
            try {
                Object.defineProperty(globalThis.document, key, {
                    value: document[key],
                    configurable: true,
                    writable: true,
                    enumerable: true,
                });
            } catch (e) {
                // Some shim documents are proxies over a fixed member set;
                // leave whatever they refuse to accept.
            }
        }
    }

    // `window.getComputedStyle` is what the plugins actually call — they grab
    // it off `window`, not `document`. Assigned unconditionally: the G2 shim
    // installed its own earlier, and that one only knows canvas element bags.
    globalThis.window.getComputedStyle = getComputedStyle;
    globalThis.getComputedStyle = getComputedStyle;
    // `window.document` gates CSSPlugin's `_initCore` (`if (_windowExists() &&
    // window.document)`); without it the plugin never builds its scratch
    // element and every transform read throws. Also the DOM's own
    // createElementNS is absent here, so the plugin's `_createElement` falls
    // through to `createElement` — which is what the scratch element is for.
    // A browser window exposes `document`/`navigator` as getter-only
    // accessors, so the assignment throws there. It is also unnecessary: on
    // that host `globalThis.window.document` IS `globalThis.document`, the
    // object merged above.
    try {
        globalThis.window.document = document;
        globalThis.window.navigator = globalThis.window.navigator || { userAgent: 'opencat' };
    } catch (e) {
        // Read-only accessors on a real window: already what we need.
    }

    // SplitText's `autoSplit` observes its elements so a resize re-splits. A
    // composition's box is fixed by the composition, never by a viewport, so
    // the observation can never fire; the constructor has to exist and be
    // disconnectable.
    if (typeof globalThis.ResizeObserver !== 'function') {
        globalThis.ResizeObserver = function ResizeObserver() {
            this.observe = function () {};
            this.unobserve = function () {};
            this.disconnect = function () {};
        };
    }

    // ── Timers / frame callbacks: hard errors with a pointer to the model ──
    // g's bundle installs its own rAF queue polyfill BEFORE this runtime
    // (see JS_PLATFORM_SHIM), so only define the guard when the G2 polyfill
    // has not claimed the name.
    function timerError(api, alternative) {
        return function () {
            throw new Error(api + ' is not available in a deterministic renderer' + (alternative ? '; ' + alternative : ''));
        };
    }
    if (typeof globalThis.setTimeout !== 'function') {
        globalThis.setTimeout = timerError('setTimeout', 'advance time through the composition clock (ctx.global.time)');
        globalThis.clearTimeout = function () {};
    }
    if (typeof globalThis.setInterval !== 'function') {
        globalThis.setInterval = timerError('setInterval', 'use useEffect(fn) in your component for per-frame work');
        globalThis.clearInterval = function () {};
    }
    if (typeof globalThis.requestAnimationFrame !== 'function') {
        globalThis.requestAnimationFrame = timerError('requestAnimationFrame', 'use useEffect(fn) in your component to run per-frame work');
        globalThis.cancelAnimationFrame = function () {};
    }
})();
