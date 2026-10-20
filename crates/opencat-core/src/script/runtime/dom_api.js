// DOM globals — `document` and `window`, plus clear errors for the browser
// APIs a deterministic renderer cannot support.
//
// The tree is declarative: `document.getElementById` returns the SAME handle
// `ctx.getNode` hands out (lazily — an id works before its element is
// created, matching how per-frame drivers replay hooks in order). Tree
// mutation (`appendChild` & friends), timers, and `requestAnimationFrame` are
// hard errors that point at the supported alternative, so real-package code
// fails loudly here instead of silently diverging.
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

    function declarativeOnly(api) {
        return function () {
            throw new Error(
                api + ': the element tree is declarative and static; build the structure in JSX and animate with gsap'
            );
        };
    }

    var document = {
        getElementById(id) {
            return nodeFor(String(id), 'document.getElementById');
        },
        querySelector(selector) {
            return nodeFor(selectorToId('document.querySelector', selector), 'document.querySelector');
        },
        querySelectorAll(selector) {
            return [nodeFor(selectorToId('document.querySelectorAll', selector), 'document.querySelectorAll')];
        },
        createElement: declarativeOnly('document.createElement'),
        createTextNode: declarativeOnly('document.createTextNode'),
        addEventListener: declarativeOnly('document.addEventListener'),
        body: null,
        documentElement: null,
    };

    if (typeof globalThis.document === 'undefined' || !globalThis.document) {
        globalThis.document = document;
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
        globalThis.setInterval = timerError('setInterval', 'use useFrame(build) for per-frame work');
        globalThis.clearInterval = function () {};
    }
    if (typeof globalThis.requestAnimationFrame !== 'function') {
        globalThis.requestAnimationFrame = timerError('requestAnimationFrame', 'use useFrame(build) to run per-frame work');
        globalThis.cancelAnimationFrame = function () {};
    }
})();
