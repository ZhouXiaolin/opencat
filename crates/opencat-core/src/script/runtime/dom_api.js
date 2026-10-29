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
            // 离线帧没有事件循环可订阅；g 系渲染器的鼠标监听路径在
            // globalThis 上加/删监听，销毁时必须可删。
            addEventListener: function () {},
            removeEventListener: function () {},
            dispatchEvent: function () { return true; },
        };
        globalThis.addEventListener = globalThis.window.addEventListener;
        globalThis.removeEventListener = globalThis.window.removeEventListener;
        globalThis.dispatchEvent = globalThis.window.dispatchEvent;
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

    // ── 影子元素：脚本自建的 DOM 子树（图表库的容器与画布）────────────
    // 存在、可查询、带尺寸；样式是私有 bag（不写 style store），结构不进
    // 合成树——图表只读尺寸不依赖布局。canvas 影子的 2D context 是只读
    // 桩（图表库拿它做字体探测 measureText；真正绘制全走 CanvasKit 面）。
    var shadowById = Object.create(null);

    function numberOr(value, fallback) {
        var n = Number(value);
        return Number.isFinite(n) ? n : fallback;
    }

    function shadowClassList() {
        return {
            add: function () {},
            remove: function () {},
            toggle: function () {},
            item: function () { return null; },
            contains: function () { return false; },
        };
    }

    function shadowContext2d(canvas) {
        // 图表库（@antv/g TextService）拿这个 2D 桩做布局测量——桩值直接
        // 进布局（G2 pie 的 legend 占位、outside 标签锚点、axis auto
        // padding），桩错布局就错。走真测量：解析 CSS font 简写 →
        // surface_measure_text（与 CanvasKit 面的 measureText 同源，
        // generic family 由 fontdb 解析）。
        function parseShadowFont(font) {
            var sizeM = /([0-9.]+)\s*px/.exec(font);
            var size = sizeM ? parseFloat(sizeM[1]) : 10;
            var weightM = /(^|\s)(bold|\d{2,3})(\s|$)/.exec(font);
            var weight = weightM ? (weightM[2] === 'bold' ? 700 : parseInt(weightM[2], 10)) : 400;
            var famM = /px(?:\s*\/\s*[0-9.]+)?\s+([^;]+)/.exec(font);
            var family = famM ? famM[1].split(',')[0].replace(/["']/g, '').trim() : 'sans-serif';
            return { family: family, weight: weight, size: size };
        }
        // g 系 TextService 的垂直度量不走 measureText 字段，靠像素扫描：
        // fillRect 涂红底、fillText 画黑字、getImageData 逐行找墨（pixi 同
        // 源算法）。桩不产真像素，但扫描的停止行必须与浏览器逐行等价，否则
        // ascent/descent 全错、文本节点高度错、轴 autoPadding 跟着错。墨带
        // 模型：记住最近一次 fillText 的墨行区间 [baseline-ink_ascent,
        // baseline+ink_descent)，getImageData 合成红底+墨行。
        var raster = { red: false, ink: null };
        function shadowMeasure(text) {
            var f = parseShadowFont(String(this.font || '10px sans-serif'));
            if (typeof __surface_measure_text === 'function') {
                return __surface_measure_text(String(text), f.family, f.weight, f.size, 0);
            }
            return { width: String(text).length * 8, ink_ascent: 0, ink_descent: 0 };
        }
        return {
            canvas: canvas,
            measureText: function (text) {
                var m = shadowMeasure.call(this, text);
                return {
                    width: m.width,
                    actualBoundingBoxLeft: m.ink_left,
                    actualBoundingBoxRight: m.ink_right,
                    actualBoundingBoxAscent: m.ink_ascent,
                    actualBoundingBoxDescent: m.ink_descent,
                    fontBoundingBoxAscent: m.font_ascent,
                    fontBoundingBoxDescent: m.font_descent
                };
            },
            fillText: function (text, x, y) {
                var m = shadowMeasure.call(this, text);
                raster.ink = [y - m.ink_ascent, y + m.ink_descent];
            },
            strokeText: function () {},
            clearRect: function () {
                raster.red = false;
                raster.ink = null;
            },
            fillRect: function () {
                raster.red = true;
                raster.ink = null;
            },
            getImageData: function (x, y, w, h) {
                var data = new Uint8ClampedArray(Math.max(0, w | 0) * Math.max(0, h | 0) * 4);
                for (var r = 0; r < h; r++) {
                    var inked = raster.ink && y + r >= raster.ink[0] && y + r < raster.ink[1];
                    var base = r * w * 4;
                    for (var c = 0; c < w; c++) {
                        if (inked) {
                            data[base + c * 4 + 3] = 255;
                        } else if (raster.red) {
                            data[base + c * 4] = 255;
                            data[base + c * 4 + 3] = 255;
                        }
                    }
                }
                return { width: w, height: h, data: data };
            },
            strokeRect: function () {},
            beginPath: function () {},
            closePath: function () {},
            moveTo: function () {},
            lineTo: function () {},
            save: function () {},
            restore: function () {},
            translate: function () {},
            scale: function () {},
            rotate: function () {},
            drawImage: function () {},
            putImageData: function () {},
            createImageData: function (w, h) {
                return { width: w, height: h, data: new Uint8ClampedArray(w * h * 4) };
            },
            setTransform: function () {},
            createLinearGradient: function () { return { addColorStop: function () {} }; },
            createRadialGradient: function () { return { addColorStop: function () {} }; },
            createPattern: function () { return null; },
            arc: function () {},
            rect: function () {},
            fill: function () {},
            stroke: function () {},
            clip: function () {},
            isPointInPath: function () { return false; },
        };
    }

    function shadowElement(tagName, opts) {
        var tag = String(tagName || 'div').toUpperCase();
        var node = {
            nodeType: 1,
            nodeName: tag,
            tagName: tag,
            style: rootStyle(),
            attributes: {},
            childNodes: [],
            parentNode: null,
            parentElement: null,
            classList: shadowClassList(),
            appendChild(child) {
                if (!child) return child;
                child.parentNode = node;
                child.parentElement = node;
                node.childNodes.push(child);
                return child;
            },
            insertBefore(child, reference) {
                if (!child) return child;
                child.parentNode = node;
                child.parentElement = node;
                var i = reference ? node.childNodes.indexOf(reference) : -1;
                if (i < 0) node.childNodes.push(child);
                else node.childNodes.splice(i, 0, child);
                return child;
            },
            removeChild(child) {
                var i = node.childNodes.indexOf(child);
                if (i >= 0) {
                    node.childNodes.splice(i, 1);
                    child.parentNode = null;
                    child.parentElement = null;
                }
                return child;
            },
            contains: function () { return false; },
            setAttribute: function (name, value) { node.attributes[name] = String(value); },
            getAttribute: function (name) {
                return Object.prototype.hasOwnProperty.call(node.attributes, name)
                    ? node.attributes[name]
                    : null;
            },
            removeAttribute: function (name) { delete node.attributes[name]; },
            hasAttribute: function (name) {
                return Object.prototype.hasOwnProperty.call(node.attributes, name);
            },
            addEventListener: function () {},
            removeEventListener: function () {},
            dispatchEvent: function () { return true; },
            focus: function () {},
            blur: function () {},
            // 布局不进合成树：盒子是零矩形，尺寸语义只走 clientWidth/Height。
            getBoundingClientRect: function () {
                return { x: 0, y: 0, left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0 };
            },
        };
        Object.defineProperty(node, 'children', { get: function () { return node.childNodes; } });
        Object.defineProperty(node, 'firstChild', {
            get: function () { return node.childNodes[0] || null; },
        });
        Object.defineProperty(node, 'lastChild', {
            get: function () {
                return node.childNodes.length
                    ? node.childNodes[node.childNodes.length - 1]
                    : null;
            },
        });
        Object.defineProperty(node, 'nextSibling', { get: function () { return null; } });
        Object.defineProperty(node, 'previousSibling', { get: function () { return null; } });
        Object.defineProperty(node, 'clientWidth', {
            get: function () { return numberOr(parseInt(node.style.width, 10), (opts && opts.width) || 0); },
        });
        Object.defineProperty(node, 'clientHeight', {
            get: function () { return numberOr(parseInt(node.style.height, 10), (opts && opts.height) || 0); },
        });
        if (tag === 'CANVAS') {
            node.width = (opts && opts.width) || 300;
            node.height = (opts && opts.height) || 150;
            node.getContext = function (kind) {
                if (kind !== '2d') {
                    throw new Error("canvas.getContext: only '2d' is supported on a shadow canvas, got '" + kind + "'");
                }
                return shadowContext2d(node);
            };
        }
        return node;
    }

    // 图表宿主入口：为 `nodeId`（<canvas> 声明节点）建/取影子容器。
    // 容器冒用节点 id——图表库 `new Chart({ container })` 期望一个可
    // appendChild 的元素，而声明节点句柄的样式写会进合成树；影子容器的
    // 私有 bag 隔离这一切。幂等：同一图表节点跨帧拿到同一容器。
    globalThis.__ocShadow = {
        ensureChart(nodeId, width, height) {
            var id = String(nodeId);
            var container = shadowById[id];
            if (!container) {
                container = shadowElement('div', { width: width, height: height });
                container.id = id;
                container.style.position = 'relative';
                container.style.width = width + 'px';
                container.style.height = height + 'px';
                // g-canvaskit 自建 `createElement('canvas')` 再挂进容器；引擎的
                // 录制 surface 按 canvas 的 id 找 DrawOp 流（makeRecordingSurface
                // → getCanvasById），挂载时把容器冒用的图表节点 id 传给它。
                var baseAppend = container.appendChild;
                container.appendChild = function (child) {
                    if (child && child.tagName === 'CANVAS' && !child.id) child.id = id;
                    return baseAppend.call(this, child);
                };
                shadowById[id] = container;
            }
            return container;
        },
        byId(id) {
            return shadowById[String(id)] || null;
        },
    };


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
        // 影子注册表优先：图表容器 id（`__ocShadow.ensureChart` 登记的）
        // 永远返回影子——图表库要往里 appendChild，声明节点句柄做不到。
        getElementById(id) {
            var shadow = shadowById[String(id)];
            if (shadow) return shadow;
            return nodeFor(String(id), 'document.getElementById');
        },
        querySelector(selector) {
            var shadow = shadowById[selectorToId('document.querySelector', selector)];
            if (shadow) return shadow;
            return elementFor(selectorToId('document.querySelector', selector), 'document.querySelector');
        },
        querySelectorAll(selector) {
            var shadow = shadowById[selectorToId('document.querySelectorAll', selector)];
            if (shadow) return [shadow];
            return [elementFor(selectorToId('document.querySelectorAll', selector), 'document.querySelectorAll')];
        },
        // canvas 元素进影子面：图表库的 `createElement('canvas')` 要的是
        // 一个带尺寸与字体探测 context 的私有元素，不进合成树。其余标签
        // 仍走树 overlay（gsap SplitText 等的可见包装节点）。
        createElement(tag) {
            if (String(tag).toLowerCase() === 'canvas') {
                return shadowElement('canvas', {});
            }
            return globalThis.__ocTree.createElement(tag);
        },
        // Plugins reach for `createElementNS` first (`_createElement` in
        // CSSPlugin) and only fall back to `createElement`. There is no
        // namespace in a declarative composition, so both make a scratch
        // element.
        createElementNS(_ns, tag) {
            if (String(tag).toLowerCase() === 'canvas') {
                return shadowElement('canvas', {});
            }
            return globalThis.__ocTree.createElement(tag);
        },
        createTextNode(text) {
            return globalThis.__ocTree.createTextNode(String(text));
        },
        // 离线帧没有事件循环：库（g 的 EventService）挂 document 级监听
        // 只是初始化仪式，no-op 即可。fail-loud 的 declarativeOnly 在这里
        // 会把 g 的 initAsync 打成 rejection——被它上游的空 catch 吞掉，
        // ready 永不落定（2026-10-28 corpus 首跑定位）。
        addEventListener: function () {},
        removeEventListener: function () {},
        dispatchEvent: function () { return true; },
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

    // 唯一的 document 安装点。图表库的 DOM 需求由影子面（上方
    // `__ocShadow` / canvas 分流）承接，不再有私有 document shim 抢先
    // 安装——那会把全 realm 的 document 换成元素桩（旧 g2 shim 的违宪
    // 形态，已退役）。
    if (typeof globalThis.document === 'undefined' || !globalThis.document) {
        globalThis.document = document;
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
    // 图表库（@antv/g）在初始化路径探测 MutationObserver；离线帧没有
    // mutation，桩只要求可构造、可断开。
    if (typeof globalThis.MutationObserver !== 'function') {
        globalThis.MutationObserver = function MutationObserver() {
            this.observe = function () {};
            this.unobserve = function () {};
            this.disconnect = function () {};
        };
    }
    // 图表库按 DPR 缩放画布；离线帧的坐标空间就是合成坐标空间，固定 1。
    if (typeof globalThis.devicePixelRatio === 'undefined') {
        globalThis.devicePixelRatio = 1;
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
