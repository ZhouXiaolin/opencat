// Node DOM facade — two surfaces over one node id.
//
// 1. `ctx.getNode(id)` — the author-facing chainable API
//    (`ctx.getNode('x').opacity(0.5)`). Unchanged.
// 2. `ctx.__domTarget(id)` — the element surface gsap plugins receive as
//    `target`. It mirrors a real DOM element: `nodeType`, `tagName`, `style`,
//    `getAttribute`, `getBBox`, `getTotalLength`, text channels — and, like a
//    real element, an unknown property reads back `undefined`.
//
// The two must stay separate. gsap probes `target[prop]` for every animated
// property (`_addPropTween(…, "get", …)`); on the chainable handle that read
// returns the mutation closure, which gsap then calls and multiplies — the
// resulting "Cannot convert object to primitive value" comes from exactly
// that. A real element answers `undefined` and gsap falls back to its
// attribute/style path.
(function() {
    function applyMutation(id, prop, ...args) {
        __write_style_value(id, prop, args.length > 1 ? args : args[0]);
    }

    function assertVisualTarget(id, apiName) {
        var key = String(id);
        var registry = ctx.__targetRegistry || {};
        if (registry.visual && registry.visual[key]) return key;
        if (registry.nonVisual && registry.nonVisual[key]) {
            throw new Error(apiName + ": non-visual id '" + key + "' cannot be targeted");
        }
        throw new Error(apiName + ": unknown id '" + key + "'");
    }

    // style-proxy property → store key. The store's keys are GSAP-style
    // camelCase (`backgroundColor`, `borderRadius`); CSS custom properties
    // pass through untouched.
    function styleKey(name) {
        name = String(name);
        if (name.indexOf('--') === 0) return name;
        if (name.indexOf('-') < 0) return name;
        return name.replace(/-([a-z])/g, function (_, c) { return c.toUpperCase(); });
    }

    function makeStyleProxy(id) {
        return new Proxy({}, {
            get(_target, prop) {
                if (typeof prop !== 'string') return undefined;
                // `style.setProperty('stroke-dasharray', v)` /
                // `getPropertyValue` — the kebab-case pair real plugins use.
                if (prop === 'setProperty') {
                    return function (name, value) { __write_style_value(id, styleKey(name), value); };
                }
                if (prop === 'getPropertyValue') {
                    return function (name) {
                        var v = __read_style_value(id, styleKey(name));
                        return v === null || v === undefined ? '' : String(v);
                    };
                }
                if (prop === 'removeProperty') return function () { return ''; };
                var v = __read_style_value(id, styleKey(prop));
                return v === null || v === undefined ? '' : v;
            },
            set(_target, prop, value) {
                if (typeof prop !== 'string') return false;
                __write_style_value(id, styleKey(prop), value);
                return true;
            },
            // A real CSSStyleDeclaration answers `'anything' in style` for
            // every property name the browser knows. CSSPlugin branches on
            // exactly that probe (`_addProps`: `p in style`) to decide whether
            // a plugin property is a CSS one; denying it sends every plain
            // property — `opacity` above all — down `_missingPlugin`, which
            // drops the tween without a word.
            has() { return true; },
        });
    }

    function box(id) {
        var raw = __read_box(id) || {};
        var left = Number(raw.left) || 0;
        var top = Number(raw.top) || 0;
        var width = Number(raw.width) || 0;
        var height = Number(raw.height) || 0;
        return {
            left: left, top: top, width: width, height: height,
            right: left + width, bottom: top + height,
            x: left, y: top,
        };
    }

    function facts(id) {
        var registry = ctx.__targetRegistry || {};
        return (registry.facts && registry.facts[id]) || null;
    }

    function tagFor(id) {
        var f = facts(id);
        return f && f.tag ? f.tag : 'div';
    }

    function pathDataFor(id) {
        var d = __read_style_value(id, 'd');
        if (d !== null && d !== undefined && d !== '') return String(d);
        var f = facts(id);
        return f && f.d ? String(f.d) : '';
    }

    // SVG shape attributes. The composition is declarative, so the only shape
    // data that exists is what JSX declared (`d` on a path — animated writes
    // land in the store) plus geometry the host derives from the node box.
    function readAttribute(id, name) {
        name = String(name);
        if (name === 'd' || name === 'morphSVG') {
            var d = pathDataFor(id);
            return d === '' ? null : d;
        }
        if (name === 'id') return id;
        if (name === 'width') return box(id).width;
        if (name === 'height') return box(id).height;
        if (name === 'x' || name === 'left') return box(id).left;
        if (name === 'y' || name === 'top') return box(id).top;
        // Style-backed presentation attributes (`fill`, `stroke`, `opacity`…).
        var v = __read_style_value(id, styleKey(name));
        return v === null || v === undefined ? '' : String(v);
    }

    function writeAttribute(id, name, value) {
        name = String(name);
        if (name === 'd') {
            __write_style_value(id, 'd', String(value));
            return;
        }
        if (name === 'class' || name === 'className') {
            __write_style_value(id, 'className', String(value));
            return;
        }
        __write_style_value(id, styleKey(name), value);
    }

    function childIds(id) {
        var registry = ctx.__targetRegistry || {};
        var out = [];
        for (var childId in registry.facts) {
            if (registry.facts[childId].parent === id) out.push(childId);
        }
        return out;
    }

    function writeText(id, value) {
        __write_style_value(id, 'text', String(value));
    }

    function readText(id) {
        var v = __read_style_value(id, 'text');
        if (v !== null && v !== undefined) return String(v);
        // Nothing has written this node's text yet, so fall back to the text
        // the composition declares for it. Reading `''` instead would report an
        // empty element: ScrambleText sizes its char budget from the length of
        // the text it finds, so an empty read makes it scramble a zero-length
        // string and reveal nothing.
        var initial = __text_source_get(id);
        return initial === null || initial === undefined ? '' : String(initial);
    }

    // Static tree mutation is the one DOM capability a declarative composition
    // cannot honour; fail loudly instead of letting the write vanish.
    function treeMutation(api) {
        return function () {
            throw new Error(
                api + ': the element tree is declarative and static; build the structure in JSX and animate with gsap'
            );
        };
    }

    // ── Element surface shared by both handles ───────────────────────
    // `own` collects the properties plugins stash on an element (gsap keeps
    // its per-target cache on `target._gsap`), so those writes survive.
    function elementMembers(id, own, getNode) {
        var style = makeStyleProxy(id);
        return {
            nodeType: 1,
            get tagName() { return tagFor(id).toUpperCase(); },
            get nodeName() { return tagFor(id).toUpperCase(); },
            get id() { return id; },
            get style() { return style; },

            getAttribute: function (name) { return readAttribute(id, name); },
            getAttributeNS: function (_ns, name) { return readAttribute(id, name); },
            setAttribute: function (name, value) { writeAttribute(id, name, value); },
            setAttributeNS: function (_ns, name, value) { writeAttribute(id, name, value); },
            hasAttribute: function (name) { return readAttribute(id, name) !== null; },

            // `getBBox` is SVG user space — the path's own bounds, not the
            // laid-out box. DrawSVG probes it, MotionPath transforms it.
            getBBox: function () {
                var d = pathDataFor(id);
                if (!d) return { x: 0, y: 0, width: 0, height: 0 };
                var b = __path_bbox(d);
                return { x: b[0], y: b[1], width: b[2], height: b[3] };
            },
            getTotalLength: function () {
                var d = pathDataFor(id);
                return d ? __path_total_length(d) : 0;
            },
            getComputedStyle: function () { return style; },
            getBoundingClientRect: function () { return box(id); },
            getContext: function (type) { return ctx.getContext(id, type); },

            get textContent() { return readText(id); },
            set textContent(value) { writeText(id, value); },
            get innerHTML() { return readText(id); },
            set innerHTML(value) { writeText(id, value); },
            get nodeValue() { return readText(id); },
            set nodeValue(value) { writeText(id, value); },

            get parentNode() {
                var f = facts(id);
                return f && f.parent ? getNode(f.parent) : null;
            },
            get parentElement() { return this.parentNode; },
            get children() { return childIds(id).map(getNode); },
            get childNodes() { return this.children; },
            get firstChild() { var kids = this.children; return kids.length ? kids[0] : null; },
            get firstElementChild() { return this.firstChild; },

            get offsetWidth() { return box(id).width; },
            get offsetHeight() { return box(id).height; },

            // Every node in a declarative composition is laid out — its box
            // exists without anything having to be attached to a document. A
            // falsy `offsetParent` would tell gsap the opposite, sending it
            // down its "re-parent into documentElement to force layout" hack,
            // which has no document to attach to here.
            get offsetParent() {
                var parent = this.parentNode;
                return parent || (globalThis.document && globalThis.document.body) || null;
            },

            get classList() {
                return {
                    contains: function (name) {
                        return (' ' + readAttribute(id, 'className') + ' ').indexOf(' ' + String(name) + ' ') >= 0;
                    },
                    add: treeMutation('classList.add'),
                    remove: treeMutation('classList.remove'),
                    toggle: treeMutation('classList.toggle'),
                };
            },

            appendChild: treeMutation('node.appendChild'),
            removeChild: treeMutation('node.removeChild'),
            insertBefore: treeMutation('node.insertBefore'),
            replaceChild: treeMutation('node.replaceChild'),
            remove: treeMutation('node.remove'),
            after: treeMutation('node.after'),
            before: treeMutation('node.before'),
        };
    }

    // ── 1. gsap target: a plain element, unknown properties undefined ──
    const targetCache = {};
    ctx.__domTarget = function(id, opts) {
        // gsap target resolution mirrors the real gsap semantics: a selector
        // that matches nothing yields an idle tween. Timelines here are
        // rebuilt every frame against that frame's tree, so a target outside
        // the current frame's tree (a later scene's stage, conditionally
        // rendered) passes `allowUnknown` and gets a placeholder handle —
        // writes land in the store harmlessly until the node exists.
        var key = String(id);
        var registry = ctx.__targetRegistry || {};
        if (!(registry.visual && registry.visual[key])) {
            if (registry.nonVisual && registry.nonVisual[key]) {
                throw new Error("__domTarget: non-visual id '" + key + "' cannot be targeted");
            }
            if (!opts || !opts.allowUnknown) {
                throw new Error("__domTarget: unknown id '" + key + "'");
            }
        }
        id = key;
        if (!targetCache[id]) {
            var own = Object.create(null);
            var members = elementMembers(id, own, ctx.__domTarget);
            targetCache[id] = new Proxy(members, {
                get(t, prop) {
                    if (typeof prop !== 'string') return t[prop];
                    if (prop in t) {
                        var v = t[prop];
                        return typeof v === 'function' ? v : v;
                    }
                    return own[prop];
                },
                set(t, prop, value) {
                    if (typeof prop === 'string' && prop in t) {
                        // Writing a real member (style.textContent…) goes
                        // through its setter.
                        t[prop] = value;
                        return true;
                    }
                    own[prop] = value;
                    return true;
                },
                has(t, prop) {
                    return typeof prop === 'string' && (prop in t || prop in own);
                },
            });
        }
        return targetCache[id];
    };

    // ── 2. Author handle: same element surface + chainable mutation API ──
    // A callable property (`getNode(id).opacity(0.5)`) is the historical
    // authoring form and stays. Anything that is not an element member falls
    // through to it.
    const nodeCache = {};
    ctx.getNode = function(id) {
        id = assertVisualTarget(id, 'ctx.getNode');
        if (!nodeCache[id]) {
            var own = Object.create(null);
            var members = elementMembers(id, own, ctx.getNode);
            var api = null;
            api = new Proxy(members, {
                get(t, prop) {
                    if (typeof prop !== 'string' || prop === 'then') return undefined;
                    if (prop in t) return t[prop];
                    if (prop in own) return own[prop];
                    return (...args) => {
                        applyMutation(id, prop, ...args);
                        return api;
                    };
                },
                set(t, prop, value) {
                    if (prop === 'style') {
                        throw new Error('node.style is read-only as a whole; assign individual properties: node.style.width = 100');
                    }
                    if (typeof prop === 'string' && prop in t) {
                        t[prop] = value;
                        return true;
                    }
                    own[prop] = value;
                    return true;
                },
            });
            nodeCache[id] = api;
        }
        return nodeCache[id];
    };
})();
