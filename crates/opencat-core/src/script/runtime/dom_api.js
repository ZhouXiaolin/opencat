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

    // ── Detached scratch elements ───────────────────────────────────
    // `document.createElement('div')` is how plugins parse the HTML strings
    // they are handed: TextPlugin sets `_tempDiv.innerHTML = value.value` and
    // then splits the result. Nothing here joins the composition — a scratch
    // element is a plain object holding text — so the HTML never has to be
    // modelled; only the round trip has to work.
    function scratchTextNode(text) {
        return {
            nodeType: 3,
            nodeName: '#text',
            nodeValue: text,
            textContent: text,
            data: text,
        };
    }

    // A CSSStyleDeclaration stand-in. The plugins probe it by name —
    // `'transform' in style`, `style.WebkitTransform` — to feature-detect
    // supported properties, and a real declaration answers for every name the
    // browser knows. So this bag answers `true` for any identifier and stores
    // whatever is assigned; nothing reads it back for layout.
    function scratchStyle() {
        var values = Object.create(null);
        return new Proxy(values, {
            get(target, prop) {
                if (typeof prop !== 'string') return target[prop];
                if (prop === 'cssText') return target.cssText || '';
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

    function scratchElement(tag) {
        var name = String(tag || 'div').toUpperCase();
        var node = {
            nodeType: 1,
            nodeName: name,
            tagName: name,
            style: scratchStyle(),
            attributes: Object.create(null),
            childNodes: [],
            // Detached: nothing is attached to anything, so the walk up to a
            // document root terminates immediately. `ownerDocument` is absent
            // on purpose — plugins fall back to treating the element itself as
            // the doc, and `rootElement()` supplies the members they need.
            parentNode: null,
            appendChild(child) {
                child.parentNode = node;
                node.childNodes.push(child);
                return child;
            },
            removeChild(child) {
                var i = node.childNodes.indexOf(child);
                if (i >= 0) node.childNodes.splice(i, 1);
                child.parentNode = null;
                return child;
            },
            setAttribute(attr, value) {
                node.attributes[String(attr)] = String(value);
            },
            getAttribute(attr) {
                var v = node.attributes[String(attr)];
                return v === undefined ? null : v;
            },
            getBoundingClientRect() {
                return { left: 0, top: 0, width: 0, height: 0, right: 0, bottom: 0, x: 0, y: 0 };
            },
            getBBox() {
                return { x: 0, y: 0, width: 0, height: 0 };
            },
        };
        Object.defineProperty(node, 'textContent', {
            get: function () {
                var out = '';
                for (var i = 0; i < node.childNodes.length; i++) out += node.childNodes[i].textContent;
                return out;
            },
            set: function (value) {
                node.childNodes = [scratchTextNode(String(value))];
            },
        });
        // Markup is stored as one text node: the plugins that use a scratch
        // element read it straight back as text, never as a tree.
        Object.defineProperty(node, 'innerHTML', {
            get: function () { return node.textContent; },
            set: function (value) { node.childNodes = [scratchTextNode(String(value))]; },
        });
        Object.defineProperty(node, 'firstChild', {
            get: function () { return node.childNodes.length ? node.childNodes[0] : null; },
        });
        Object.defineProperty(node, 'nextSibling', { get: function () { return null; } });
        return node;
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

    function getComputedStyle(node) {
        var id = elementIdFor(node);
        if (id) return globalThis.ctx.getNode(id).style;
        if (node && node.style) return node.style;
        return scratchStyle();
    }

    // One scratch root for the whole realm: plugins walk UP from an element to
    // the document root (`_setDoc` does `doc.parentNode` until null, then calls
    // `createElementNS` on whatever it reached), so the root has to be an
    // object that owns element creation.
    function rootElement() {
        var root = scratchElement('html');
        root.createElement = function (tag) { return scratchElement(tag); };
        root.createElementNS = function (_ns, tag) { return scratchElement(tag); };
        root.createTextNode = function (text) { return scratchTextNode(String(text)); };
        root.body = root;
        root.documentElement = root;
        return root;
    }

    var ROOT = rootElement();

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
        createElement(tag) {
            return scratchElement(tag);
        },
        // Plugins reach for `createElementNS` first (`_createElement` in
        // CSSPlugin) and only fall back to `createElement`. There is no
        // namespace in a declarative composition, so both make a scratch
        // element.
        createElementNS(_ns, tag) {
            return scratchElement(tag);
        },
        createTextNode(text) {
            return scratchTextNode(String(text));
        },
        addEventListener: declarativeOnly('document.addEventListener'),
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
        };
        for (var key in document) {
            if (!OVERWRITE[key] && globalThis.document[key] !== undefined && globalThis.document[key] !== null) {
                continue;
            }
            try {
                globalThis.document[key] = document[key];
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
    globalThis.window.document = document;
    globalThis.window.navigator = globalThis.window.navigator || { userAgent: 'opencat' };

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
