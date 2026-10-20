// Node DOM facade — the author-facing per-node surface that mirrors the
// browser element API. The existing chainable mutation API
// (`ctx.getNode('x').opacity(0.5)`) stays as-is; DOM-property reads/writes are
// added as Proxy branches on top, so both forms share one handle.
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
                var v = __read_style_value(id, styleKey(prop));
                return v === null || v === undefined ? '' : v;
            },
            set(_target, prop, value) {
                if (typeof prop !== 'string') return false;
                __write_style_value(id, styleKey(prop), value);
                return true;
            },
        });
    }

    function box(id) {
        var box = __read_box(id) || {};
        var left = Number(box.left) || 0;
        var top = Number(box.top) || 0;
        var width = Number(box.width) || 0;
        var height = Number(box.height) || 0;
        return {
            left: left, top: top, width: width, height: height,
            right: left + width, bottom: top + height,
            x: left, y: top,
        };
    }

    const nodeCache = {};
    ctx.getNode = function(id) {
        id = assertVisualTarget(id, 'ctx.getNode');
        if (!nodeCache[id]) {
            let api = null;
            api = new Proxy({}, {
                get(target, prop) {
                    if (typeof prop !== 'string' || prop === 'then') {
                        return undefined;
                    }
                    // ── DOM property branches (property form, not methods) ──
                    if (prop === 'id') return id;
                    if (prop === 'style') return makeStyleProxy(id);
                    if (prop === 'textContent') {
                        var v = __read_style_value(id, 'text');
                        return v === null || v === undefined ? '' : String(v);
                    }
                    if (prop === 'classList') {
                        return {
                            contains(name) {
                                var v = __read_style_value(id, 'className');
                                var classes = v === null || v === undefined ? '' : String(v);
                                return (' ' + classes + ' ').indexOf(' ' + String(name) + ' ') >= 0;
                            },
                            add() { throw new Error('classList.add: the class list is static in a declarative composition; animate with gsap instead'); },
                            remove() { throw new Error('classList.remove: the class list is static in a declarative composition; animate with gsap instead'); },
                            toggle() { throw new Error('classList.toggle: the class list is static in a declarative composition; animate with gsap instead'); },
                        };
                    }
                    if (prop === 'offsetWidth') return box(id).width;
                    if (prop === 'offsetHeight') return box(id).height;
                    if (prop === 'getBoundingClientRect') return function () { return box(id); };
                    if (prop === 'getContext') return function (type) { return ctx.getContext(id, type); };
                    // Tree mutation is the one DOM capability a declarative
                    // composition cannot honour; fail loudly instead of
                    // letting the unknown-key write vanish into the store.
                    if (prop === 'appendChild' || prop === 'removeChild'
                        || prop === 'insertBefore' || prop === 'replaceChild'
                        || prop === 'remove' || prop === 'after' || prop === 'before') {
                        throw new Error(
                            'node.' + prop + ': the element tree is declarative and static; build the structure in JSX and animate with gsap'
                        );
                    }
                    // ── Chainable mutation methods (existing form) ──
                    return (...args) => {
                        applyMutation(id, prop, ...args);
                        return api;
                    };
                },
                set(target, prop, value) {
                    if (prop === 'textContent') {
                        __write_style_value(id, 'text', String(value));
                        return true;
                    }
                    if (prop === 'style') {
                        throw new Error('node.style is read-only as a whole; assign individual properties: node.style.width = 100');
                    }
                    return false;
                },
            });
            nodeCache[id] = api;
        }
        return nodeCache[id];
    };
})();
