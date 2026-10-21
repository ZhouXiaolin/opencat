// Script-side DOM tree: node handles that read and write the overlay.
//
// The composition tree is declarative — JSX decides structure. Plugins still
// build nodes (SplitText's char/word/line wrappers) and move children, so the
// host keeps an overlay: node identity is realm-scoped (a plugin's module-level
// scratch element outlives one frame), structure is frame-scoped (scripts rerun
// every frame and rebuild what they need).
//
// Three node shapes, all carrying their overlay id on a private symbol:
//   - text node (nodeType 3) — created by `createTextNode` or synthesized for a
//     declared `<text>`; text is its only state.
//   - dynamic element — created by `createElement`; style writes go to the same
//     store channel as declared nodes, so gsap tweens it normally.
//   - declared element — its own style/attributes stay declarative (the
//     `ctx.__domTarget` handle); only its CHILDREN come from the overlay.
(function () {
    'use strict';

    var NODE_ID = typeof Symbol === 'function' ? Symbol('ocNodeId') : '__ocNodeId';
    var handleCache = Object.create(null);

    // ── No-store fallback ───────────────────────────────────────────
    // The JSX extract realm evaluates the module graph once to read the tree;
    // it has no binding table. Module top level still registers plugins, and
    // CSSPlugin builds scratch elements there (`_tempDiv` for unit probes).
    // Those elements never join the composition, so a detached bag with a
    // style proxy is enough — the same shape the overlay's created nodes have.
    if (typeof __dom_create_element !== 'function') {
        function detachedStyle() {
            var values = Object.create(null);
            return new Proxy(values, {
                get(target, prop) {
                    if (typeof prop !== 'string') return target[prop];
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
                    if (prop === 'cssText') return target.cssText || '';
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

        function detachedNode(nodeType, tag, text) {
            var node = {
                nodeType: nodeType,
                nodeName: nodeType === 3 ? '#text' : String(tag).toUpperCase(),
                tagName: nodeType === 3 ? undefined : String(tag).toUpperCase(),
                style: detachedStyle(),
                childNodes: [],
                parentNode: null,
                appendChild(child) {
                    child.parentNode = node;
                    node.childNodes.push(child);
                    return child;
                },
                insertBefore(child, reference) {
                    var i = reference ? node.childNodes.indexOf(reference) : -1;
                    child.parentNode = node;
                    if (i < 0) node.childNodes.push(child);
                    else node.childNodes.splice(i, 0, child);
                    return child;
                },
                removeChild(child) {
                    var i = node.childNodes.indexOf(child);
                    if (i >= 0) node.childNodes.splice(i, 1);
                    child.parentNode = null;
                    return child;
                },
                replaceChild(newChild, oldChild) {
                    var i = node.childNodes.indexOf(oldChild);
                    if (i >= 0) node.childNodes[i] = newChild;
                    newChild.parentNode = node;
                    oldChild.parentNode = null;
                    return oldChild;
                },
                replaceWith() {},
                remove() {},
                cloneNode(deep) {
                    var copy = detachedNode(nodeType, tag, text);
                    if (deep) {
                        for (var i = 0; i < node.childNodes.length; i++) {
                            copy.appendChild(node.childNodes[i].cloneNode(true));
                        }
                    }
                    return copy;
                },
                normalize() {},
                contains(other) { return other === node; },
                setAttribute() {},
                getAttribute() { return null; },
                removeAttribute() {},
                hasAttribute() { return false; },
                getBoundingClientRect() {
                    return { left: 0, top: 0, width: 0, height: 0, right: 0, bottom: 0, x: 0, y: 0 };
                },
                getBBox() { return { x: 0, y: 0, width: 0, height: 0 }; },
            };
            Object.defineProperty(node, 'textContent', {
                get: function () {
                    if (nodeType === 3) return text;
                    var out = '';
                    for (var i = 0; i < node.childNodes.length; i++) out += node.childNodes[i].textContent;
                    return out;
                },
                set: function (value) {
                    if (nodeType === 3) { text = String(value); return; }
                    node.childNodes = [];
                    if (value !== '') node.appendChild(detachedNode(3, '#text', String(value)));
                },
            });
            Object.defineProperty(node, 'nodeValue', {
                get: function () { return nodeType === 3 ? text : null; },
                set: function (value) { if (nodeType === 3) text = String(value); },
            });
            Object.defineProperty(node, 'data', {
                get: function () { return nodeType === 3 ? text : ''; },
                set: function (value) { if (nodeType === 3) text = String(value); },
            });
            Object.defineProperty(node, 'innerHTML', {
                get: function () { return node.textContent; },
                set: function (value) { node.textContent = value; },
            });
            Object.defineProperty(node, 'outerHTML', {
                get: function () { return node.textContent; },
            });
            Object.defineProperty(node, 'className', {
                get: function () { return node.getAttribute('class') || ''; },
                set: function (value) { node.setAttribute('class', value); },
            });
            Object.defineProperty(node, 'firstChild', {
                get: function () { return node.childNodes.length ? node.childNodes[0] : null; },
            });
            Object.defineProperty(node, 'lastChild', {
                get: function () {
                    return node.childNodes.length ? node.childNodes[node.childNodes.length - 1] : null;
                },
            });
            Object.defineProperty(node, 'nextSibling', {
                get: function () {
                    var p = node.parentNode;
                    if (!p) return null;
                    var i = p.childNodes.indexOf(node);
                    return i >= 0 && i + 1 < p.childNodes.length ? p.childNodes[i + 1] : null;
                },
            });
            Object.defineProperty(node, 'previousSibling', {
                get: function () {
                    var p = node.parentNode;
                    if (!p) return null;
                    var i = p.childNodes.indexOf(node);
                    return i > 0 ? p.childNodes[i - 1] : null;
                },
            });
            return node;
        }

        globalThis.__ocTree = {
            createElement: function (tag) { return detachedNode(1, tag || 'div', ''); },
            createTextNode: function (text) { return detachedNode(3, '#text', String(text)); },
            handleFor: function (id) { return id; },
            nodeId: function (node) { return node; },
        };
        return;
    }

    function styleKey(name) {
        name = String(name);
        if (name.indexOf('--') === 0) return name;
        if (name.indexOf('-') < 0) return name;
        return name.replace(/-([a-z])/g, function (_, c) { return c.toUpperCase(); });
    }

    function applyCssText(id, text) {
        var parts = String(text).split(';');
        for (var i = 0; i < parts.length; i++) {
            var colon = parts[i].indexOf(':');
            if (colon < 0) continue;
            var name = parts[i].slice(0, colon).trim();
            var value = parts[i].slice(colon + 1).trim();
            if (name) __write_style_value(id, styleKey(name), value);
        }
    }

    function styleProxy(id) {
        return new Proxy({}, {
            get(_t, prop) {
                if (typeof prop !== 'string') return undefined;
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
                if (prop === 'cssText') return '';
                var v = __read_style_value(id, styleKey(prop));
                return v === null || v === undefined ? '' : String(v);
            },
            set(_t, prop, value) {
                if (typeof prop !== 'string') return false;
                if (prop === 'cssText') {
                    applyCssText(id, value);
                    return true;
                }
                __write_style_value(id, styleKey(prop), value);
                return true;
            },
            has() { return true; },
        });
    }

    function textOf(id) {
        var t = __dom_text(id);
        return t === null || t === undefined ? '' : String(t);
    }

    function parentHandle(id) {
        var p = __dom_parent(id);
        return p === null || p === undefined ? null : handleFor(String(p));
    }

    function siblingHandles(id) {
        var s = __dom_siblings(id) || [];
        return [
            s[0] === null || s[0] === undefined ? null : handleFor(String(s[0])),
            s[1] === null || s[1] === undefined ? null : handleFor(String(s[1])),
        ];
    }

    function childHandles(id) {
        var pairs = __dom_children(id) || [];
        var out = [];
        for (var i = 0; i < pairs.length; i++) out.push(handleFor(String(pairs[i][0])));
        return out;
    }

    function tagUpper(id) {
        var t = __dom_tag(id);
        return t === null || t === undefined ? 'DIV' : String(t).toUpperCase();
    }

    // A script-built wrapper (SplitText's char/word/line element) has no
    // declared position or size — its box comes from the text it holds. The
    // range is resolved against the nearest declared text ancestor, so the box
    // is the same shaping run the renderer uses, aggregated per byte range.
    //
    // This must answer truthfully. gsap's `_getMatrix` treats an element with
    // an identity matrix, no `offsetParent` and a zero-width rect as
    // not-laid-out, and re-parents it to `documentElement` to force layout —
    // which reorders the split children.
    function boxOf(id) {
        var range = __dom_text_range(id);
        var b = range
            ? (__text_element_boxes(String(range[2]), [[range[0], range[1]]]) || [])[0]
            : null;
        if (!b) return { left: 0, top: 0, width: 0, height: 0 };
        var left = Number(b.left) || 0;
        var top = Number(b.top) || 0;
        var width = Number(b.width) || 0;
        var height = Number(b.height) || 0;
        return { left: left, top: top, width: width, height: height };
    }

    function rectOf(id) {
        var b = boxOf(id);
        return {
            left: b.left, top: b.top, width: b.width, height: b.height,
            right: b.left + b.width, bottom: b.top + b.height,
            x: b.left, y: b.top,
        };
    }

    /// Children/tree members every overlay-backed node shares.
    function treeMembers(id, target) {
        function define(name, getter, setter) {
            Object.defineProperty(target, name, {
                get: getter,
                set: setter,
                configurable: true,
                enumerable: true,
            });
        }
        define('nodeType', function () { return __dom_node_type(id); });
        define('childNodes', function () { return childHandles(id); });
        define('children', function () { return childHandles(id); });
        define('firstChild', function () {
            var kids = childHandles(id);
            return kids.length ? kids[0] : null;
        });
        define('firstElementChild', function () {
            var kids = childHandles(id);
            return kids.length ? kids[0] : null;
        });
        define('lastChild', function () {
            var kids = childHandles(id);
            return kids.length ? kids[kids.length - 1] : null;
        });
        define('parentNode', function () { return parentHandle(id); });
        define('parentElement', function () { return parentHandle(id); });
        define('previousSibling', function () { return siblingHandles(id)[0]; });
        define('nextSibling', function () { return siblingHandles(id)[1]; });
        define('textContent', function () { return textOf(id); },
            function (value) { __dom_set_text(id, String(value)); });
        define('nodeValue', function () { return textOf(id); },
            function (value) { __dom_set_text(id, String(value)); });
        define('className', function () {
            var v = __dom_class_name(id);
            return v === null || v === undefined ? '' : String(v);
        }, function (value) { __dom_set_class_name(id, String(value)); });
        return target;
    }

    /// Children/tree methods every overlay-backed node shares.
    function treeMethods(id, target) {
        target.appendChild = function (child) { return moveChild(id, child, 'append'); };
        target.insertBefore = function (child, reference) { return moveChild(id, child, 'before', reference); };
        target.removeChild = function (child) { __dom_remove_child(id, nodeId(child)); return child; };
        target.replaceChild = function (newChild, oldChild) {
            __dom_replace_child(id, nodeId(newChild), nodeId(oldChild));
            return oldChild;
        };
        target.replaceWith = function () {
            __dom_replace_with(id, Array.prototype.slice.call(arguments).map(nodeId));
        };
        target.remove = function () { __dom_remove(id); };
        target.cloneNode = function (deep) {
            var cloned = __dom_clone_node(id, deep !== false);
            return cloned === null || cloned === undefined ? null : handleFor(String(cloned));
        };
        target.normalize = function () { __dom_normalize(id); };
        target.contains = function (other) {
            var cursor = other;
            while (cursor) {
                if (nodeId(cursor) === id) return true;
                cursor = cursor.parentNode;
            }
            return false;
        };
        target.getBoundingClientRect = function () { return rectOf(id); };
        target.getBBox = function () {
            var b = boxOf(id);
            return { x: b.left, y: b.top, width: b.width, height: b.height };
        };
        // Every node in a declarative composition is laid out — its box exists
        // without anything being attached to a document. A falsy `offsetParent`
        // tells gsap the opposite and sends it down its re-parent-to-force-
        // layout path (see `boxOf`).
        Object.defineProperty(target, 'offsetWidth', { get: function () { return boxOf(id).width; }, configurable: true });
        Object.defineProperty(target, 'offsetHeight', { get: function () { return boxOf(id).height; }, configurable: true });
        Object.defineProperty(target, 'offsetParent', {
            get: function () { return parentHandle(id) || (globalThis.document && globalThis.document.body) || null; },
            configurable: true,
        });
        return target;
    }

    function textHandle(id) {
        var handle = { nodeName: '#text' };
        treeMembers(id, handle);
        treeMethods(id, handle);
        handle.tagName = undefined;
        handle.data = textOf(id);
        handle.splitText = function (offset) {
            var text = textOf(id);
            __dom_set_text(id, text.slice(0, offset));
            var tail = String(__dom_create_text_node(text.slice(offset)));
            var parent = __dom_parent(id);
            if (parent !== null && parent !== undefined) {
                var siblings = __dom_siblings(id) || [];
                var next = siblings[1];
                __dom_insert_before(String(parent), tail, next === null || next === undefined ? null : String(next));
            }
            return handleFor(tail);
        };
        return handle;
    }

    function dynamicElement(id) {
        var handle = {
            get tagName() { return tagUpper(id); },
            get nodeName() { return tagUpper(id); },
            get id() { return ''; },
            get data() { return textOf(id); },
            set data(value) { __dom_set_text(id, String(value)); },
            // A created element holds text only, so markup round trips as text.
            get innerHTML() { return textOf(id); },
            set innerHTML(value) { __dom_set_text(id, String(value)); },
            get outerHTML() { return textOf(id); },
            get offsetWidth() { return 0; },
            get offsetHeight() { return 0; },
            get offsetParent() { return null; },
            get style() { return styleProxy(id); },
            setAttribute(name, value) { __write_style_value(id, styleKey(name), String(value)); },
            getAttribute(name) {
                var v = __read_style_value(id, styleKey(name));
                return v === null || v === undefined ? null : String(v);
            },
            getAttributeNS(_ns, name) { return this.getAttribute(name); },
            setAttributeNS(_ns, name, value) { this.setAttribute(name, value); },
            removeAttribute(name) { __write_style_value(id, styleKey(name), ''); },
            hasAttribute(name) {
                var v = __read_style_value(id, styleKey(name));
                return v !== null && v !== undefined && v !== '';
            },
        };
        treeMembers(id, handle);
        treeMethods(id, handle);
        return handle;
    }

    function moveChild(parentId, child, mode, reference) {
        var childId = nodeId(child);
        if (!childId) return child;
        if (mode === 'append') {
            __dom_append_child(parentId, childId);
        } else {
            var refId = reference === null || reference === undefined ? null : nodeId(reference);
            __dom_insert_before(parentId, childId, refId || null);
        }
        return child;
    }

    function nodeId(node) {
        if (node === null || node === undefined) return '';
        if (typeof node === 'string') return node;
        // Declared handles (node_style.js) and overlay handles both stamp the id.
        return node.__ocNodeId || node[NODE_ID] || '';
    }

    /// Handle for an overlay node id, cached per id for the realm lifetime
    /// (identity is realm-scoped, so a handle stays valid across frames).
    function handleFor(id) {
        if (handleCache[id]) return handleCache[id];
        var nodeType = __dom_node_type(id);
        var handle;
        if (nodeType === 3) {
            handle = textHandle(id);
        } else if (__dom_is_dynamic(id)) {
            handle = dynamicElement(id);
        } else {
            // A declared node: node_style.js owns its style/attribute surface and
            // already reads its children from the overlay, so reuse that handle
            // rather than shadowing it with a second one.
            handle = globalThis.ctx.__domTarget(id, { allowUnknown: true });
            handleCache[id] = handle;
            return handle;
        }
        Object.defineProperty(handle, NODE_ID, { value: id, enumerable: false });
        handleCache[id] = handle;
        return handle;
    }

    function createElement(tag) {
        return handleFor(String(__dom_create_element(String(tag || 'div'))));
    }

    function createTextNode(text) {
        return handleFor(String(__dom_create_text_node(String(text))));
    }

    globalThis.__ocTree = {
        createElement: createElement,
        createTextNode: createTextNode,
        handleFor: handleFor,
        nodeId: nodeId,
    };
})();
