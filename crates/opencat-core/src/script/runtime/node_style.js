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
    var SPACE_RE = /\s+/;

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
                // `style.cssText = 'opacity:0; left:10px'` — the standard bulk
                // write. Split on `;`, then on the first `:` so `filter` values
                // keep their own commas and parentheses.
                if (prop === 'cssText') {
                    return function () { return ''; };
                }
                var v = __read_style_value(id, styleKey(prop));
                return v === null || v === undefined ? '' : v;
            },
            set(_target, prop, value) {
                if (typeof prop !== 'string') return false;
                if (prop === 'cssText') {
                    String(value).split(';').forEach(function (decl) {
                        var i = decl.indexOf(':');
                        if (i < 0) return;
                        var name = decl.slice(0, i).trim();
                        if (!name) return;
                        __write_style_value(id, styleKey(name), decl.slice(i + 1).trim());
                    });
                    return true;
                }
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
        // An inline text element has no declared box: its rect is the layout
        // box, which is viewport-relative and needs a layout pass to exist.
        if (width === 0 || height === 0) {
            var laid = (typeof __read_layout_box === 'function' && __read_layout_box(id)) || null;
            if (laid) {
                left = Number(laid.left) || 0;
                top = Number(laid.top) || 0;
                width = Number(laid.width) || 0;
                height = Number(laid.height) || 0;
            } else {
                var ink = (__text_element_boxes(id, []) || [])[0];
                if (ink) {
                    left = Number(ink.left) || 0;
                    top = Number(ink.top) || 0;
                    width = Number(ink.width) || 0;
                    height = Number(ink.height) || 0;
                }
            }
        }
        return {
            left: left, top: top, width: width, height: height,
            right: left + width, bottom: top + height,
            x: left, y: top,
        };
    }

    // A script-built wrapper (SplitText's char/word/line element) has no
    // declared position or size — its box comes from the text it holds. The
    // range is resolved against the nearest declared text ancestor, so the
    // shape is the SAME shaping run the renderer does, aggregated per byte
    // range. Declared nodes keep their declared box.
    function wrapperBox(id) {
        var range = __dom_text_range(id);
        if (!range) return box(id);
        var b = (__text_element_boxes(String(range[2]), [[range[0], range[1]]]) || [])[0];
        if (!b) return box(id);
        var left = Number(b.left) || 0;
        var top = Number(b.top) || 0;
        var width = Number(b.width) || 0;
        var height = Number(b.height) || 0;
        return {
            left: left, top: top, width: width, height: height,
            right: left + width, bottom: top + height,
            x: left, y: top,
        };
    }

    function elementBox(id) {
        return __dom_is_dynamic(id) ? wrapperBox(id) : box(id);
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

    // Attributes a script wrote (`setAttribute`). These are node facts, not
    // animatable style — MorphSVG stores the pre-morph `d` in `data-original`
    // so a later `morphSVG` back to the same element can revert it.
    function scriptAttribute(id, name) {
        var v = __dom_get_attribute(id, name);
        return v === null || v === undefined ? null : String(v);
    }

    // SVG shape attributes. The composition is declarative, so shape data is
    // what JSX declared (`d` on a path — animated writes land in the store)
    // plus geometry the host derives from the node box.
    function readAttribute(id, name) {
        name = String(name);
        // `d` reads the style channel first: a tween writes there, and that
        // value is the one being rendered. `data-original` and other script
        // attributes only ever live in the attribute bag.
        if (name === 'd' || name === 'morphSVG') {
            var d = pathDataFor(id);
            return d === '' ? null : d;
        }
        if (name === 'id') return id;
        if (name === 'class' || name === 'className') return readClassName(id);
        var written = scriptAttribute(id, name);
        if (written !== null) return written;
        if (name === 'width') return box(id).width;
        if (name === 'height') return box(id).height;
        if (name === 'x' || name === 'left') return box(id).left;
        if (name === 'y' || name === 'top') return box(id).top;
        // Style-backed presentation attributes (`fill`, `stroke`, `opacity`…).
        var v = __read_style_value(id, styleKey(name));
        return v === null || v === undefined ? '' : String(v);
    }

    // Writes land in both channels. The attribute bag is what read-back sees
    // (a browser reports `getAttribute('fill')` after `setAttribute('fill')`),
    // the style channel is what the renderer paints — presentation attributes
    // ARE styles in a browser. Unknown names are ignored by the style channel,
    // so `data-*` costs nothing there.
    function writeAttribute(id, name, value) {
        name = String(name);
        var text = String(value);
        if (name === 'class' || name === 'className') {
            writeClassName(id, text);
            __dom_set_attribute(id, 'class', text);
            return;
        }
        __dom_set_attribute(id, name, text);
        __write_style_value(id, name === 'd' ? 'd' : styleKey(name), text);
    }

    // Children come from the script-side overlay: a declared node's declared
    // children plus anything a script inserted (SplitText wrappers). Reading it
    // is what makes `childNodes` reflect this frame's structure.
    function childNodesOf(id) {
        var pairs = __dom_children(id) || [];
        var out = [];
        for (var i = 0; i < pairs.length; i++) {
            out.push(globalThis.__ocTree.handleFor(String(pairs[i][0])));
        }
        return out;
    }

    // `className` is a node fact, not an animatable style: a declared node's
    // Tailwind class comes from the seeded initial styles, a write lands in the
    // overlay so it survives the frame but never reaches the renderer's style
    // channel.
    function readClassName(id) {
        var v = __dom_class_name(id);
        if (v !== null && v !== undefined && v !== '') return String(v);
        var seeded = __read_style_value(id, 'className');
        return seeded === null || seeded === undefined ? '' : String(seeded);
    }

    function writeClassName(id, value) {
        __dom_set_class_name(id, String(value));
    }

    // Markup is not modelled — only text is. `innerHTML` therefore serializes
    // the subtree's text with the five entity escapes a browser applies, and
    // the write side unescapes them back. `_revertOriginal` in SplitText
    // depends on exactly this round trip.
    var HTML_ESCAPES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' };

    function escapeHtml(text) {
        return String(text).replace(/[&<>"]/g, function (c) { return HTML_ESCAPES[c]; });
    }

    function unescapeHtml(text) {
        return String(text)
            .replace(/&lt;/g, '<')
            .replace(/&gt;/g, '>')
            .replace(/&quot;/g, '"')
            .replace(/&#39;/g, "'")
            .replace(/&nbsp;/g, '\u00a0')
            .replace(/&amp;/g, '&');
    }

    function writeText(id, value) {
        __write_style_value(id, 'text', String(value));
    }

    function readText(id) {
        var v = __read_style_value(id, 'text');
        if (v !== null && v !== undefined) return String(v);
        // Nothing has written this node's text yet, so fall back to the tree:
        // `textContent` of a declared node is its composed text, and of a split
        // element it is the concatenation of the wrappers. Reading `''` instead
        // would report an empty element — ScrambleText sizes its char budget
        // from the length of the text it finds, so an empty read makes it
        // scramble a zero-length string and reveal nothing.
        var initial = __dom_text(id);
        return initial === null || initial === undefined ? '' : String(initial);
    }

    // ── Element surface shared by both handles ───────────────────────
    // `own` collects the properties plugins stash on an element (gsap keeps
    // its per-target cache on `target._gsap`), so those writes survive.
    function elementMembers(id, own, getNode) {
        var style = makeStyleProxy(id);
        // `<canvas>` 内容尺寸的写入缓冲（three 的 setSize 写 `canvas.width`/
        // `height`）。初值是布局盒——浏览器里 canvas.width 初始就是声明的
        // width 属性，而这里的布局盒就是那声明本身。惰性取：h() 建树时
        // 布局还不存在（host 绑定未注入），首次读/写才落盒。
        var canvasSize = null;
        function canvasBox() {
            if (!canvasSize) {
                var b = box(id);
                canvasSize = { w: b.width | 0, h: b.height | 0 };
            }
            return canvasSize;
        }
        var members = {
            nodeType: 1,
            get tagName() { return tagFor(id).toUpperCase(); },
            get nodeName() { return tagFor(id).toUpperCase(); },
            get id() { return id; },
            get style() { return style; },
            get className() { return readClassName(id); },
            set className(value) { writeClassName(id, value); },

            getAttribute: function (name) { return readAttribute(id, name); },
            getAttributeNS: function (_ns, name) { return readAttribute(id, name); },
            setAttribute: function (name, value) { writeAttribute(id, name, value); },
            setAttributeNS: function (_ns, name, value) { writeAttribute(id, name, value); },
            removeAttribute: function (name) { writeAttribute(id, name, ''); },
            removeAttributeNS: function (_ns, name) { writeAttribute(id, name, ''); },
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
            getBoundingClientRect: function () { return elementBox(id); },
            // DrawSVG scales a `vector-effect: non-scaling-stroke` path's length
            // by this matrix. The composition's user space IS the screen, so the
            // identity matrix is the truthful answer (scale 1, no rotation).
            getScreenCTM: function () {
                return { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 };
            },
            getContext: function (type, attrs) { return ctx.getContext(id, type, attrs); },

            // three 的 WebGLRenderer 构造向 canvas 挂 `webglcontextlost`/
            // `webglcontextrestored` 监听。确定性录制面没有上下文丢失事件，
            // 订阅只须存在且可移除（同 document.fonts 的先例）。
            addEventListener: function () {},
            removeEventListener: function () {},

            // `<canvas>` 的内容尺寸属性。three 的 setSize 写 `canvas.width`/
            // `height = floor(css * pixelRatio)` 再回读；布局是声明式的，写
            // 入不改布局盒，回读必须与写入 round-trip（three 的视口计算用
            // 它），所以缓冲在句柄上而不映射回样式。
            get width() { return canvasBox().w; },
            set width(v) { canvasBox().w = Math.max(0, Math.floor(Number(v) || 0)); },
            get height() { return canvasBox().h; },
            set height(v) { canvasBox().h = Math.max(0, Math.floor(Number(v) || 0)); },

            // Text reads prefer the style store's `text` channel, then the
            // composition's declared text.
            get textContent() { return readText(id); },
            set textContent(value) {
                // An element with children replaces them with one text node
                // (DOM semantics); a leaf keeps using the style `text` channel so
                // the renderer's existing text path stays authoritative.
                var kids = __dom_children(id) || [];
                if (kids.length) {
                    __dom_set_text(id, String(value));
                } else {
                    writeText(id, value);
                }
            },
            // SplitText snapshots `element.innerHTML` before splitting and
            // writes it back in `revert()`. Only the TEXT is modelled here, so
            // the snapshot is the escaped text content and the restore is its
            // unescaping — which round trips exactly for a text-only element
            // (`<text>`, or a div holding one string). Markup structure is not
            // parsed: nested tags come back as their text.
            get innerHTML() { return escapeHtml(__dom_text(id) || ''); },
            set innerHTML(value) {
                var kids = __dom_children(id) || [];
                if (kids.length) {
                    __dom_set_text(id, unescapeHtml(String(value)));
                } else {
                    writeText(id, unescapeHtml(String(value)));
                }
            },
            get nodeValue() { return readText(id); },
            set nodeValue(value) { writeText(id, value); },

            get parentNode() {
                var parent = __dom_parent(id);
                if (parent === null || parent === undefined) return null;
                return globalThis.__ocTree.handleFor(String(parent));
            },
            get parentElement() { return this.parentNode; },
            get children() { return childNodesOf(id); },
            get childNodes() { return childNodesOf(id); },
            get firstChild() { var kids = childNodesOf(id); return kids.length ? kids[0] : null; },
            get firstElementChild() { return this.firstChild; },
            get lastChild() { var kids = childNodesOf(id); return kids.length ? kids[kids.length - 1] : null; },
            get previousSibling() {
                var s = __dom_siblings(id) || [];
                return s[0] === null || s[0] === undefined ? null : globalThis.__ocTree.handleFor(String(s[0]));
            },
            get nextSibling() {
                var s = __dom_siblings(id) || [];
                return s[1] === null || s[1] === undefined ? null : globalThis.__ocTree.handleFor(String(s[1]));
            },

            get offsetWidth() { return box(id).width; },
            get offsetHeight() { return box(id).height; },
            // Offsets are border-box edges relative to the offset parent; with
            // no offset parent those are the viewport edges (CSSOM View §7.3).
            get offsetLeft() { return box(id).left; },
            get offsetTop() { return box(id).top; },

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
                function read() { return readClassName(id); }
                function write(list) { writeClassName(id, list.filter(Boolean).join(' ')); }
                return {
                    contains: function (name) {
                        return (' ' + read() + ' ').indexOf(' ' + String(name) + ' ') >= 0;
                    },
                    add: function (name) {
                        var list = read().split(SPACE_RE).filter(Boolean);
                        if (list.indexOf(String(name)) < 0) list.push(String(name));
                        write(list);
                    },
                    remove: function (name) {
                        write(read().split(SPACE_RE).filter(function (c) { return c && c !== String(name); }));
                    },
                    toggle: function (name) {
                        var list = read().split(SPACE_RE).filter(Boolean);
                        var i = list.indexOf(String(name));
                        if (i < 0) list.push(String(name)); else list.splice(i, 1);
                        write(list);
                    },
                };
            },

            // Tree mutation goes to the overlay: a declared node's own style
            // stays declarative, but its child list is a view scripts may change.
            appendChild: function (child) {
                __dom_append_child(id, globalThis.__ocTree.nodeId(child));
                return child;
            },
            insertBefore: function (child, reference) {
                var refId = reference === null || reference === undefined
                    ? null : globalThis.__ocTree.nodeId(reference);
                __dom_insert_before(id, globalThis.__ocTree.nodeId(child), refId || null);
                return child;
            },
            removeChild: function (child) {
                __dom_remove_child(id, globalThis.__ocTree.nodeId(child));
                return child;
            },
            replaceChild: function (newChild, oldChild) {
                __dom_replace_child(id, globalThis.__ocTree.nodeId(newChild), globalThis.__ocTree.nodeId(oldChild));
                return oldChild;
            },
            replaceWith: function () {
                var args = Array.prototype.slice.call(arguments).map(globalThis.__ocTree.nodeId);
                __dom_replace_with(id, args);
            },
            remove: function () { __dom_remove(id); },
            normalize: function () { __dom_normalize(id); },
            after: function () {
                // Insert the argument(s) right after this node in its parent.
                var parent = __dom_parent(id);
                if (parent === null || parent === undefined) return;
                var siblings = __dom_siblings(id) || [];
                var next = siblings[1];
                var args = Array.prototype.slice.call(arguments);
                for (var i = 0; i < args.length; i++) {
                    __dom_insert_before(String(parent), globalThis.__ocTree.nodeId(args[i]),
                        next === null || next === undefined ? null : String(next));
                }
            },
            before: function () {
                var parent = __dom_parent(id);
                if (parent === null || parent === undefined) return;
                var args = Array.prototype.slice.call(arguments);
                for (var i = 0; i < args.length; i++) {
                    __dom_insert_before(String(parent), globalThis.__ocTree.nodeId(args[i]), id);
                }
            },
        };
        // The overlay id travels with the handle so tree bindings accept a
        // declared handle as a child argument (SplitText moves child nodes).
        Object.defineProperty(members, '__ocNodeId', { value: id, enumerable: false });
        return members;
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

    // jsx_runtime 的 `h()`：宿主标签的 el 以这个面为原型——ref.current 与
    // `ctx.__domTarget(id)` 是同一对象（同一缓存），gsap/three 消费的
    // HTMLElement 语义与 gsap 目标解析共享一份元素身份。与 __domTarget 的
    // 差别只有验证：h 发生在树物化之前，unknown 是常态而非错误。
    ctx.__ocElementFace = function (id) {
        var key = String(id);
        if (!targetCache[key]) {
            var own = Object.create(null);
            var members = elementMembers(key, own, ctx.__domTarget);
            targetCache[key] = new Proxy(members, {
                get(t, prop) {
                    if (typeof prop !== 'string') return t[prop];
                    if (prop in t) return t[prop];
                    return own[prop];
                },
                set(t, prop, value) {
                    if (typeof prop === 'string' && prop in t) {
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
        return targetCache[key];
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
                    // `width` / `height` 是 `<canvas>` 的内容尺寸数据属性，
                    // 会遮住作者句柄的链式写入 `ctx.getNode(id).width(v)`。
                    // 非 canvas 节点没有数据属性，落回 applyMutation。
                    if (prop === 'width' || prop === 'height') {
                        if (tagFor(id) !== 'canvas') {
                            return (...args) => {
                                applyMutation(id, prop, ...args);
                                return api;
                            };
                        }
                    }
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
