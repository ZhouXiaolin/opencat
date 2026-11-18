// HTML 标准 DOM 面（core 标准层）。
//
// 这个文件解释 W3C / 浏览器 DOM 的输入语义：元素、属性、样式、选择器、
// 文本节点、媒体元素、canvas/GL 查询面、序列化。core 是确定性视频渲染器，
// 不是浏览器内核；帧由 `frame = f(t)` 重建，动画由 `animate = g(t)` 写入。
// 共有组合协议在 `shared_api.js`，HF 私有扩展在 `hyperframes_api.js`，
// 标准层只经窄接口调用扩展钩子。
(function () {
    'use strict';
    if (globalThis.__ocHtmlStd) return;
    // html 通道标记：webgl_api 的 tagAsWebGL2 以此限定作用域。
    globalThis.__ocHtmlChannel = true;

    // 扩展层钩子：标准层不认识 hyperframes 私有词汇，只调用这些函数。
    // manifest 保证 shared/hyperframes 先行装配；缺失即装配错误，fail-loud。
    if (!globalThis.__ocHyperframes) {
        throw new Error('hyperframes_api.js must load before html_std_api.js');
    }
    var extension = globalThis.__ocHyperframes;
    globalThis.__ocHtmlStd = {
        extension: extension,
        nodes: nodes,
        setSeed: function (value) { seed = value; },
        getSeed: function () { return seed; },
        setRootFace: function (value) { rootFace = value; },
        getRootFace: function () { return rootFace; },
        hasStore: function () { return hasStore; },
        isSeekStarted: function () { return seekStarted; },
        beginSeek: function () { seekStarted = true; },
        nextSynthId: function () { return synthSeq++; },
        offscreenById: htmlOffscreenById,
    };

    var nodes = Object.create(null); // id → element face
    var htmlOffscreenById = Object.create(null);
    var rootFace = null;
    // 本帧登记的 gsap 产物（timeline 或顶层补间）。必须挂在 globalThis 上：
    // 本文件每帧重 eval，闭包里的数组会随之重建，而 gsap 的包装器只装一次
    // （旧闭包仍持有它），新帧写进旧数组 → 产物永不回收，整片渲染顶到 V8
    // 堆上限（~110 帧即 OOM）。
    var synthSeq = 0;
    var seed = null;
    var hasStore = typeof __write_style_value === 'function';
    // 持久文档（init-once）：页面脚本只跑一次，之后每帧由虚拟 rAF 驱动
    // （引擎 setClock(t) + 每帧一次 tick，注册表 timeline 由播放器语义
    // play——见 driver 逐帧段）。DOM 状态（内联样式/文本、canvas 位图、
    // svg 属性）因此跨帧留在 store 里——与浏览器一致：gsap 只重写在动的
    // 补间，已结束补间的终值靠元素自己保持，不以基准值回退。构造期脚本
    // 写的自定义属性并进提取的内联样式（静态树的基准值）；tick 驱动的
    // 写入（gsap 补间 `{"--x": v}`、tl.call 回调）每帧经 store 重放，
    // 不进静态树——提取期 computed 读不到样式表变量，补间起点是空串，
    // 烤进去会把基准值写坏。
    var seekStarted = false;
    if (hasStore && typeof __persist_frame_state === 'function') {
        __persist_frame_state();
    }

    // ── style 通道 ────────────────────────────────────────────────
    // store 的键 = CSS 标准词表（camelCase）。本层只做 CSSOM 语义的归一：
    // kebab→camel 与厂商前缀别名（浏览器对 `-webkit-clip-path` 与
    // `clip-path` 同义；语料页面成对双写两者）。
    var VENDOR_KEY_ALIAS = { webkitClipPath: 'clipPath' };
    function styleKey(name) {
        name = String(name);
        if (name.indexOf('--') === 0) return name;
        if (name.indexOf('-') >= 0) {
            name = name.replace(/-([a-z])/g, function (_, c) { return c.toUpperCase(); });
        }
        return VENDOR_KEY_ALIAS[name] || name;
    }

    // 纯色判定：`border` 简写拆出来的颜色段要先验证是颜色本身。
    var FLAT_COLOR = /^#([0-9a-fA-F]{3,8})$|^rgba?\(|^hsla?\(|^[a-zA-Z]+$/;
    function writeStyle(id, key, value) {
        if (!hasStore || !id) return;
        var k = styleKey(key);
        var v = value;
        if (!seekStarted && k === 'opacity' && globalThis.__ocOpSnap && globalThis.__ocOpSnap[id] !== undefined) {
            (globalThis.__ocOpStale || (globalThis.__ocOpStale = {}))[id] = true;
        }
        if (k === 'border') {
            // `border: 1px solid rgba(...)` 拆成 store 的宽度 + 颜色两字段
            var str = String(v);
            var w = /(\d+(?:\.\d+)?)px/.exec(str);
            var c = str.replace(/\d+(?:\.\d+)?px/g, '').replace(/solid|dashed|dotted|none/g, '').trim();
            if (w) __write_style_value(id, 'borderWidth', w[1]);
            if (c && FLAT_COLOR.test(c)) __write_style_value(id, 'borderColor', c);
            return;
        }
        __write_style_value(id, k, v);
    }

    function readStyle(id, key) {
        if (!hasStore || !id) return '';
        var v = __read_style_value(id, styleKey(key));
        return v === null || v === undefined ? '' : v;
    }

    function makeStyleProxy(el) {
        // 取 id 必须延迟：`this.style = makeStyleProxy(this)` 在 id 存取器
        // 定义之前执行，构造期读 `el.id` 恒为 undefined（写全被静默丢弃）。
        function id() { return el.__payload.id; }
        // 自定义属性（`--x`）记进 __styleBag 供读回；构造期的写入另记进
        // __customBase，提取期并进内联样式（静态树基准，见 seekStarted）。
        // texture 的 feel/mask 场景在构造期按帧数据写 `--feel-bg` 等。
        function custom(name) { return String(name).indexOf('--') === 0; }
        function bakeCustom(name, value) {
            if (seekStarted) return;
            if (value === undefined) delete el.__customBase[name];
            else el.__customBase[name] = value;
        }
        return new Proxy({}, {
            get(_t, prop) {
                if (typeof prop !== 'string') return undefined;
                if (prop === 'setProperty') {
                    return function (name, value) {
                        if (custom(name)) {
                            el.__styleBag[String(name)] = String(value);
                            bakeCustom(String(name), String(value));
                        }
                        svgStyleWrite(el, name, value);
                        writeStyle(id(), name, value);
                    };
                }
                if (prop === 'getPropertyValue') {
                    return function (name) {
                        if (custom(name) && el.__styleBag[String(name)] !== undefined) {
                            return el.__styleBag[String(name)];
                        }
                        return String(readStyle(id(), name));
                    };
                }
                if (prop === 'removeProperty') {
                    return function (name) {
                        if (custom(name)) {
                            delete el.__styleBag[String(name)];
                            bakeCustom(String(name), undefined);
                        }
                        return '';
                    };
                }
                if (prop === 'cssText') {
                    return function () {
                        var inline = el.__payload.style || '';
                        var dyn = Object.keys(el.__styleBag).map(function (k) {
                            return k + ': ' + el.__styleBag[k];
                        });
                        return dyn.length ? inline + '; ' + dyn.join('; ') : inline;
                    };
                }
                var v = readStyle(id(), prop);
                if (v === '' && el.__styleBag[prop] !== undefined) v = el.__styleBag[prop];
                if (v === '' && el.__svgNode) v = svgComputedValue(el, prop);
                return v;
            },
            set(_t, prop, value) {
                if (typeof prop !== 'string') return false;
                if (prop === 'cssText') {
                    el.__payload.style = String(value);
                    el.__styleBag = Object.create(null);
                    el.__customBase = Object.create(null);
                    String(value).split(';').forEach(function (decl) {
                        var i = decl.indexOf(':');
                        if (i < 0) return;
                        var name = decl.slice(0, i).trim();
                        if (!name) return;
                        if (custom(name)) {
                            el.__styleBag[name] = decl.slice(i + 1).trim();
                            bakeCustom(name, decl.slice(i + 1).trim());
                        }
                        writeStyle(id(), name, decl.slice(i + 1).trim());
                    });
                    return true;
                }
                el.__styleBag[prop] = value;
                svgStyleWrite(el, prop, value);
                writeStyle(id(), prop, value);
                return true;
            },
            // gsap CSSPlugin 用 `'transform' in style` 探测属性支持
            // （_checkPropPrefix）：无 has trap 时探测落空 → gr=null →
            // `_parseTransform` 里 `e.replace` 炸。声明全支持（tree_api 的
            // detachedStyle 同款约定）。
            has() { return true; },
        });
    }

    // ── 选择器引擎 ────────────────────────────────────────────────
    // compound：tag / #id / .class / [attr=v]；组合器：后代 / > / + / ~；
    // 逗号分隔多选择器。伪类伪元素不支持（永不匹配）。
    function parseSelector(sel) {
        var s = String(sel == null ? '' : sel).trim();
        if (!s) return null;
        var seqs = s.split(',').map(function (part) { return part.trim(); }).filter(Boolean);
        var out = [];
        for (var g = 0; g < seqs.length; g++) {
            var seq = parseCompoundSeq(seqs[g]);
            if (!seq) return null;
            out.push(seq);
        }
        return out.length ? out : null;
    }

    // 一个 sequence：[{parts, comb}]，comb 是它与左侧相邻 compound 的组合器；
    // parts[0] 是 subject。右到左匹配从 sequence 末尾开始。
    function parseCompoundSeq(input) {
        var s = input;
        var i = 0;
        var seq = [];
        var pendingComb = null;
        function readCompound() {
            var parts = [];
            var m = /^[a-zA-Z][\w-]*/.exec(s.slice(i));
            if (m) { parts.push({ kind: 'tag', name: m[0].toLowerCase() }); i += m[0].length; }
            for (;;) {
                var ch = s[i];
                if (ch === '#') {
                    m = /^([\w-]+)/.exec(s.slice(i + 1));
                    if (!m) return null;
                    parts.push({ kind: 'id', name: m[1] }); i += 1 + m[0].length;
                } else if (ch === '.') {
                    m = /^([\w-]+)/.exec(s.slice(i + 1));
                    if (!m) return null;
                    parts.push({ kind: 'class', name: m[1] }); i += 1 + m[0].length;
                } else if (ch === '[') {
                    var close = s.indexOf(']', i);
                    if (close < 0) return null;
                    var am = /^([\w:.-]+)\s*(?:([~|^$*]?=)\s*([\s\S]*?))?$/
                        .exec(s.slice(i + 1, close).trim());
                    if (!am) return null;
                    var raw = am[3] === undefined ? '' : am[3];
                    var ci = false;
                    var fm = /\s+([iIsS])$/.exec(raw);
                    if (fm) { ci = fm[1].toLowerCase() === 'i'; raw = raw.slice(0, fm.index); }
                    var attrVal = raw.trim().replace(/^["']|["']$/g, '');
                    parts.push(am[2]
                        ? { kind: 'attr', name: am[1].toLowerCase(), op: am[2], value: attrVal, ci: ci }
                        : { kind: 'attr', name: am[1].toLowerCase(), op: null, value: null });
                    i = close + 1;
                } else if (ch === ':') {
                    return null;
                } else break;
            }
            return parts.length ? parts : null;
        }
        for (;;) {
            while (i < s.length && /\s/.test(s[i])) i++;
            if (i >= s.length) break;
            var parts = readCompound();
            if (!parts) return null;
            var comb = pendingComb || ' ';
            pendingComb = null;
            seq.push({ parts: parts, comb: comb });
            var j = i;
            while (j < s.length && /\s/.test(s[j])) j++;
            if (j >= s.length) break;
            if (s[j] === '>') { pendingComb = '>'; i = j + 1; }
            else if (s[j] === '+') { pendingComb = '+'; i = j + 1; }
            else if (s[j] === '~') { pendingComb = '~'; i = j + 1; }
            // j > i 说明两个 compound 之间隔着空白 = 后代组合器（s[j] 已经是
            // 下一个 compound 的首字符，不能再拿它判空白，否则 `.pin .stem`
            // 这类后代选择器整条被丢弃）。
            else if (j > i) { pendingComb = ' '; i = j; }
            else return null;
        }
        return seq.length ? seq : null;
    }

    function elTag(el) {
        // 真实 HTML 标签在 `uaTag`（`<i>`/`<span>` 的引擎节点类都是 div）。
        // 只读 `type` 时 `querySelectorAll('.trk i')` 一条都命中不了——
        // gsap 静默跳过整组进度条（cloud-render 的青色填充永不出现）。
        var p = el.__payload || {};
        var t = String(p.uaTag || p.type || 'div').toLowerCase();
        // svg 内部面（`<text>`/`<image>`/`<g>`）的 type 就是真实标签名，不能
        // 走下面的 HTML 语义归一——`#wm-c-0 text` 必须命中 svg 的 text 元素，
        // 归一成 span 后选择器永远 0 命中（gsap 的 fill 补间静默跳过）。
        if (el.__svgNode) return t;
        // 引擎节点名 → HTML 语义（CSS tag 选择器输入）
        if (p.uaTag) return t;
        if (t === 'text') return 'span';
        if (t === 'image') return 'img';
        // 引擎归一成 div 但原文标签不同的节点（游离 head face 等）按原文
        // 标签匹配选择器——浏览器里 <head> 的 tag 就是 head。
        if (t === 'div' && el.__payload.uaTag) return String(el.__payload.uaTag).toLowerCase();
        return t;
    }

    function elClasses(el) {
        var cls = el.__payload.className || '';
        return cls ? cls.split(/\s+/) : [];
    }

    // 属性选择器求值（与 crates/opencat-core/src/css/cascade.rs 的
    // attr_value_matches 同语义：`=`/`~=`/`|=`/`^=`/`$=`/`*=` + i 标志）。
    function attrMatches(actual, op, value, ci) {
        var a = ci ? String(actual).toLowerCase() : String(actual);
        var b = ci ? String(value).toLowerCase() : String(value);
        if (op === '=') return a === b;
        if (op === '~=') return a.split(/\s+/).indexOf(b) >= 0;
        if (op === '|=') return a === b || a.indexOf(b + '-') === 0;
        if (op === '^=') return a.indexOf(b) === 0;
        if (op === '$=') return b.length <= a.length && a.slice(a.length - b.length) === b;
        if (op === '*=') return a.indexOf(b) >= 0;
        return false;
    }

    function compoundMatches(el, parts) {
        for (var i = 0; i < parts.length; i++) {
            var p = parts[i];
            if (p.kind === 'tag') {
                if (elTag(el) !== p.name) {
                    // html/body 别名只给根
                    if (!((p.name === 'html' || p.name === 'body') && el === rootFace)) return false;
                }
            } else if (p.kind === 'id') {
                if (el.id !== p.name) return false;
            } else if (p.kind === 'class') {
                if (elClasses(el).indexOf(p.name) < 0) return false;
            } else if (p.kind === 'attr') {
                var v = el.getAttribute(p.name);
                if (p.op === null || p.op === undefined) { if (v === null) return false; }
                else if (v === null || !attrMatches(v, p.op, p.value, p.ci)) return false;
            }
        }
        return true;
    }

    function seqMatches(el, seq) {
        // seq 右到左：seq[len-1] 是 subject。
        var idx = seq.length - 1;
        if (!compoundMatches(el, seq[idx].parts)) return false;
        var cur = el;
        while (idx > 0) {
            idx--;
            var comb = seq[idx + 1].comb; // subject 与左侧的连接
            if (comb === '>') {
                cur = cur.parentNode;
                if (!cur || cur.nodeType !== 1 || !compoundMatches(cur, seq[idx].parts)) return false;
            } else if (comb === ' ') {
                var found = false;
                cur = cur.parentNode;
                while (cur && cur.nodeType === 1) {
                    if (compoundMatches(cur, seq[idx].parts)) { found = true; break; }
                    cur = cur.parentNode;
                }
                if (!found) return false;
            } else if (comb === '+' || comb === '~') {
                var sibs = elementSiblings(cur);
                var at = sibs.indexOf(cur);
                var hit = false;
                for (var k = at - 1; k >= 0; k--) {
                    if (compoundMatches(sibs[k], seq[idx].parts)) { hit = true; break; }
                    if (comb === '+') break; // 相邻只看紧邻前兄
                }
                if (!hit) return false;
                cur = sibs[k];
            } else return false;
        }
        return true;
    }

    function matchesSelector(el, parsed) {
        for (var i = 0; i < parsed.length; i++) {
            if (seqMatches(el, parsed[i])) return true;
        }
        return false;
    }

    function walkAll(el, out) {
        var kids = el.__kids || [];
        for (var i = 0; i < kids.length; i++) {
            if (kids[i].nodeType !== 1) continue; // 文本节点不进选择器树
            out.push(kids[i]);
            walkAll(kids[i], out);
        }
        // svg 子树的查询面（__svgKids）：种子/fragment 里 svg 是整体 inline
        // XML（serializeNode 遇 element 子节点会把 svg 降级 div），所以子面
        // 不进 __kids、只进这里——选择器遍历看得见，提取序列化看不见。
        var sk = el.__svgKids;
        if (sk) {
            for (i = 0; i < sk.length; i++) {
                if (sk[i].nodeType !== 1) continue;
                out.push(sk[i]);
                walkAll(sk[i], out);
            }
        }
        return out;
    }

    // 选择器兄弟语义只看元素（+ / ~ 跳过文本节点，DOM 标准行为）。
    function elementSiblings(el) {
        var parent = el.parentNode;
        if (!parent) return [];
        return (parent.__kids || []).filter(function (k) { return k.nodeType === 1; });
    }

    function queryAll(roots, sel) {
        var parsed = parseSelector(sel);
        if (!parsed) return [];
        var all = [];
        for (var i = 0; i < roots.length; i++) walkAll(roots[i], all);
        return all.filter(function (el) { return matchesSelector(el, parsed); });
    }

    // ── innerHTML fragment 解析 ───────────────────────────────────
    // 页面脚本注入的标记是模板字符串拼的（k3 语料）：div/span/br/img/
    // svg 子树/文本/实体。 tokenizer 级解析，不追求完整 HTML5 语法。
    var ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: '\u00a0' };

    function decodeEntities(text) {
        return String(text).replace(/&(#x?[0-9a-fA-F]+|[a-zA-Z]+);/g, function (_, body) {
            if (body[0] === '#') {
                var code = body[1] === 'x' || body[1] === 'X'
                    ? parseInt(body.slice(2), 16) : parseInt(body.slice(1), 10);
                return isNaN(code) ? _ : String.fromCharCode(code);
            }
            return ENTITIES[body] !== undefined ? ENTITIES[body] : _;
        });
    }

    var VOID_TAGS = { br: 1, img: 1, hr: 1, input: 1, meta: 1, link: 1 };

    // fragment/createElement 的 tag → 引擎 DomElement 归一（serde 枚举只认
    // 固定变体，unknown 直接炸——Rust 解析端的 Div 兜底到不了这条路）。
    // 语义装饰标签（i/b/u/…）降 div：Div 承载文本时 Rust 端合成 P，不丢内容。
    var FRAG_TAG_MAP = {
        img: 'image', image: 'image', video: 'video', audio: 'audio',
        canvas: 'canvas', svg: 'svg',
        // opencat 特色节点（serde 变体名，与 dom.rs 对齐；缺失会让脚本
        // 建元降级成 div、时间轴/转场语义整条丢失）。
        timeline: 'timeline', tl: 'timeline', transition: 'transition',
        p: 'p', span: 'span', text: 'span',
        h1: 'p', h2: 'p', h3: 'p', h4: 'p', h5: 'p', h6: 'p',
        i: 'div', b: 'div', strong: 'div', em: 'div', u: 'div', small: 'div',
        label: 'div', section: 'div', header: 'div', footer: 'div', nav: 'div',
        main: 'div', article: 'div', aside: 'div', hr: 'div', br: 'div',
        ul: 'div', ol: 'div', li: 'div', dl: 'div', dt: 'div', dd: 'div',
        a: 'div', button: 'div', input: 'div', form: 'div', table: 'div',
    };
    function normalizeEngineTag(tag) {
        return FRAG_TAG_MAP[tag] || 'div';
    }

    function xmlEscape(s) {
        return String(s)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    // svg 子树整体序列化成近似 XML（DomElement::Svg.inline 通道，与 Rust
    // 解析端 serialize_subtree 同语义）；子树不再展开成 JS 元素。
    function xmlSerialize(node) {
        var s = '<' + node.tag;
        for (var k in node.attrs) s += ' ' + k + '="' + xmlEscape(node.attrs[k] || '') + '"';
        if (!node.kids.length) return s + '/>';
        s += '>';
        node.kids.forEach(function (kid) {
            if (kid.text !== undefined) s += xmlEscape(kid.text);
            else s += xmlSerialize(kid);
        });
        return s + '</' + node.tag + '>';
    }

    // ── svg 子树的查询面 ─────────────────────────────────────────
    // intro 语料在页面脚本里查 svg 内部元素（`path[d]` 读 d/fill、渐变 stop
    // 读 offset/stop-color）再光栅进 GL 纹理。seed 里 svg 是整体 inline XML
    // 字符串（Rust serialize_subtree 产物：双引号属性、&amp;/&lt;/&gt;/&quot;
    // 实体、`/>` 自闭合），这里 tokenizer 级解析回轻量 face 挂 __svgKids——
    // walkAll 让选择器看得见，serializeNode 看不见（不进提取树）。
    function parseInlineXml(text) {
        var root = { tag: '#root', attrs: {}, kids: [] };
        var stack = [root];
        var s = String(text == null ? '' : text);
        var i = 0;
        while (i < s.length) {
            var lt = s.indexOf('<', i);
            if (lt < 0) lt = s.length;
            if (lt > i) {
                var txt = decodeEntities(s.slice(i, lt));
                if (txt.trim()) {
                    var top = stack[stack.length - 1];
                    top.kids.push({ tag: '#text', attrs: {}, kids: [], text: txt });
                }
            }
            if (lt >= s.length) break;
            i = lt;
            if (s.slice(i, i + 2) === '</') {
                var close = s.indexOf('>', i);
                if (close < 0) break;
                if (stack.length > 1) stack.pop();
                i = close + 1;
            } else {
                var close2 = s.indexOf('>', i);
                if (close2 < 0) break;
                var selfClose = s[close2 - 1] === '/';
                var inner = s.slice(i + 1, selfClose ? close2 - 1 : close2);
                var m = /^([^\s/>]+)[\s\S]*$/.exec(inner);
                if (!m) { i = close2 + 1; continue; }
                var attrs = {};
                var are = /([^\s=/>]+)\s*=\s*"([^"]*)"/g, am;
                while ((am = are.exec(inner))) attrs[am[1]] = decodeEntities(am[2]);
                // 裸属性（`vector-effect="non-scaling-stroke"` 之外的布尔式写
                // 法）只看标签名之后的部分——整段参与会把标签名本身收成
                // `path=""`，重序列化后污染每个元素的标记。
                var bareRest = inner.slice(m[1].length).replace(/([^\s=/>]+)\s*=\s*"[^"]*"/g, '');
                var bare = bareRest.split(/\s+/);
                for (var b = 0; b < bare.length; b++) {
                    if (bare[b] && !(bare[b] in attrs)) attrs[bare[b]] = '';
                }
                var node = { tag: m[1], attrs: attrs, kids: [] };
                stack[stack.length - 1].kids.push(node);
                if (!selfClose) stack.push(node);
                i = close2 + 1;
            }
        }
        return root;
    }

    // 面的 payload.type 直接用真实小写标签名（elTag 的匹配输入）；引擎标签
    // 归一会把未知标签映成 div，tag 选择器（`path`）就断了。这些面不进提取。
    function buildSvgQueryFace(node, parent) {
        var tag = String(node.tag).toLowerCase();
        var payload = {
            type: tag,
            uaTag: tag,
            id: node.attrs.id || nextSynthId(tag),
            className: node.attrs['class'] || '',
            style: node.attrs['style'] || ''
        };
        var face = createElementFace(payload);
        // 写穿锚点：脚本 setAttribute 改的是这个 XML 节点（svgWriteAttr）。
        face.__svgNode = node;
        face.__svgRoot = parent.__svgRoot || parent;
        // 带 id 的 svg 内部元素（如 `<defs id="fxdefs">`）按浏览器语义可被
        // getElementById 命中——nodes 表是 getElementById 的唯一查找面。每帧
        // __ocHtmlResetFrame 重建 nodes 后重新走物化，无泄漏。
        if (node.attrs.id) nodes[payload.id] = face;
        for (var k in node.attrs) {
            if (k === 'class' || k === 'style' || k === 'id') continue;
            face.__attrs[k] = node.attrs[k];
        }
        face.parentNode = parent;
        face.__svgKids = [];
        for (var i = 0; i < node.kids.length; i++) {
            if (node.kids[i].tag === '#text') continue;
            face.__svgKids.push(buildSvgQueryFace(node.kids[i], face));
        }
        return face;
    }

    function attachSvgQueryFaces(el, inline) {
        if (!inline || typeof inline !== 'string' || el.__svgKids) return;
        var tree = parseInlineXml(inline);
        el.__svgXml = tree;
        el.__svgKids = [];
        var kids = tree.kids;
        // inline 是 `<svg>…</svg>` 整体（serialize_subtree / xmlSerialize
        // 产物）：顶层元素是 svg 根自身——el 这个 face 已代表根，查询子面
        // 取根的直接子元素，不再包一层 svg face。多包一层会让 `#mark > svg
        // path` 的直父（`>`）语义断在中间 face 上（后代 ` ` 碰巧仍通）。
        if (kids.length === 1 && String(kids[0].tag).toLowerCase() === 'svg') {
            kids = kids[0].kids;
        }
        for (var i = 0; i < kids.length; i++) {
            if (kids[i].tag === '#text') continue;
            el.__svgKids.push(buildSvgQueryFace(kids[i], el));
        }
    }

    // svg 内部元素的属性写（setAttribute/removeAttribute、gsap attr:）写穿到
    // inline XML 树：渲染树消费的是提取期快照，整棵 svg 重序列化后经
    // `__svg_set_markup` 进 store，resolve 按 svg 节点 id 覆盖本帧 markup；
    // 提取期由 serializeNode 认 __svgDirty。属性名大小写按 XML 原文对齐
    // （viewBox 之类），新属性保留调用方写法。
    function svgWriteAttr(face, name, value) {
        var attrs = face.__svgNode.attrs;
        var lower = name.toLowerCase();
        var key = name;
        for (var k in attrs) {
            if (k.toLowerCase() === lower) { key = k; break; }
        }
        if (value === null) delete attrs[key];
        else attrs[key] = value;
        var root = face.__svgRoot;
        root.__svgDirty = true;
        if (typeof __svg_set_markup === 'function' && root.__payload && root.__payload.id) {
            __svg_set_markup(root.__payload.id, svgInlineFromXml(root.__svgXml));
        }
    }

    function svgInlineFromXml(tree) {
        var s = '';
        tree.kids.forEach(function (kid) {
            s += kid.text !== undefined ? xmlEscape(kid.text) : xmlSerialize(kid);
        });
        return s;
    }

    // svg 内部 face 的 style 写（gsap CSSPlugin 的 `style.strokeDashoffset = v`）
    // 写穿到 inline XML：这些元素不是渲染树节点，store 里的样式突变无人
    // 消费——只有重序列化 markup（svgWriteAttr → __svg_set_markup）才能
    // 到达 resolve 的本帧覆盖。属性名 camelCase → dash-case（usvg 按 CSS
    // 属性名解析 style）。
    function svgStyleDash(prop) {
        return String(prop).replace(/[A-Z]/g, function (c) { return '-' + c.toLowerCase(); });
    }
    function svgStyleWrite(el, name, value) {
        if (!el || !el.__svgNode || !el.__svgNode.attrs) return;
        var dash = svgStyleDash(name).toLowerCase();
        var decls = [];
        var seen = false;
        (el.__svgNode.attrs['style'] || '').split(';').forEach(function (d) {
            var i = d.indexOf(':');
            if (i <= 0) return;
            var k = d.slice(0, i).trim();
            if (!k || k.toLowerCase() === dash) {
                if (k.toLowerCase() === dash && !seen) {
                    seen = true;
                    if (value !== null && value !== '') decls.push(k + ': ' + value);
                }
                return;
            }
            decls.push(k + ':' + d.slice(i + 1).trim());
        });
        if (!seen && value !== null && value !== '') decls.push(svgStyleDash(name) + ': ' + value);
        svgWriteAttr(el, 'style', decls.join('; '));
    }
    function svgStyleRead(el, prop) {
        if (!el || !el.__svgNode || !el.__svgNode.attrs) return '';
        var dash = svgStyleDash(prop).toLowerCase();
        var style = el.__svgNode.attrs['style'] || '';
        var parts = style.split(';');
        for (var i = parts.length - 1; i >= 0; i--) {
            var at = parts[i].indexOf(':');
            if (at <= 0) continue;
            if (parts[i].slice(0, at).trim().toLowerCase() === dash) {
                return parts[i].slice(at + 1).trim();
            }
        }
        return '';
    }

    // svg 内部元素的 `transform` 呈现属性折成 2×3 矩阵串。浏览器把属性
    // transform 计入 computed transform，gsap 的 `_parseTransform` 从那里
    // 读元素当前 x/y；只回 `none` 时它认为元素在原点上，随后写下的
    // `translate(0,0) scale(…)` 覆盖属性 transform，元素整个跳到画布外
    // （codex `#send` 的脉冲动画从 translate(6597 904.5) 跳到原点）。
    function svgAttrTransformMatrix(el) {
        var t = el && el.__svgNode ? el.__svgNode.attrs['transform'] : undefined;
        if (t === undefined || t === null || String(t).trim() === '') return 'none';
        var m = [1, 0, 0, 1, 0, 0];
        function mul(a, b) {
            return [
                a[0] * b[0] + a[2] * b[1], a[1] * b[0] + a[3] * b[1],
                a[0] * b[2] + a[2] * b[3], a[1] * b[2] + a[3] * b[3],
                a[0] * b[4] + a[2] * b[5] + a[4], a[1] * b[4] + a[3] * b[5] + a[5],
            ];
        }
        var re = /([a-zA-Z]+)\s*\(([^)]*)\)/g, mm;
        while ((mm = re.exec(String(t)))) {
            var name = mm[1].toLowerCase();
            var args = mm[2].split(/[\s,]+/).filter(function (s) { return s !== ''; })
                .map(function (s) { return parseFloat(s); });
            var n = null;
            if (name === 'translate') n = [1, 0, 0, 1, args[0] || 0, args[1] || 0];
            else if (name === 'scale') {
                var sx = args.length ? args[0] : 1, sy = args.length > 1 ? args[1] : sx;
                n = [sx, 0, 0, sy, 0, 0];
            } else if (name === 'rotate') {
                var a = (args[0] || 0) * Math.PI / 180, c = Math.cos(a), s = Math.sin(a);
                n = [c, s, -s, c, 0, 0];
                if (args.length > 2) {
                    n = mul(mul([1, 0, 0, 1, args[1] || 0, args[2] || 0], n),
                        [1, 0, 0, 1, -(args[1] || 0), -(args[2] || 0)]);
                }
            } else if (name === 'matrix') {
                n = [args[0] || 0, args[1] || 0, args[2] || 0, args[3] || 0, args[4] || 0, args[5] || 0];
            } else if (name === 'skewx') n = [1, 0, Math.tan((args[0] || 0) * Math.PI / 180), 1, 0, 0];
            else if (name === 'skewy') n = [1, Math.tan((args[0] || 0) * Math.PI / 180), 0, 1, 0, 0];
            if (n) m = mul(m, n);
        }
        return 'matrix(' + m.join(', ') + ')';
    }

    // svg 内部 face 的 computed 值：style 属性优先，其次呈现属性，
    // transform 属性折成矩阵串。浏览器把这三层都计入 computed style，
    // gsap 的补间起点从这里读（`tl.to('#wm-c-0 text', {fill:'rgb(27,25,29)'})`
    // 读不到起点就整条跳过，字标停在源里的紫）。
    function svgComputedValue(el, name) {
        if (!el || !el.__svgNode) return '';
        var sv = svgStyleRead(el, name);
        if (sv !== '') return sv;
        if (name === 'transform') return svgAttrTransformMatrix(el);
        var attrs = el.__svgNode.attrs || {};
        var dash = String(name).replace(/[A-Z]/g, function (c) {
            return '-' + c.toLowerCase();
        }).toLowerCase();
        var av = attrs[dash];
        if (av === undefined) av = attrs[name];
        return av === undefined || av === null ? '' : String(av);
    }

    function parseFragment(html) {
        var out = []; // [{tag, attrs, text}]
        var s = String(html == null ? '' : html);
        var i = 0;
        var stack = []; // 未闭合标签栈；text 收进当前栈顶（或根）
        function emit(node) {
            if (stack.length) stack[stack.length - 1].kids.push(node);
            else out.push(node);
        }
        while (i < s.length) {
            var lt = s.indexOf('<', i);
            if (lt < 0) {
                var rest = s.slice(i);
                if (rest) emit({ text: decodeEntities(rest) });
                break;
            }
            if (lt > i) emit({ text: decodeEntities(s.slice(i, lt)) });
            if (s.slice(lt, lt + 4) === '<!--') {
                var end = s.indexOf('-->', lt);
                i = end < 0 ? s.length : end + 3;
                continue;
            }
            var close = /^<\/\s*([a-zA-Z][\w-]*)\s*>/.exec(s.slice(lt));
            if (close) {
                // 弹到同名标签为止（不校验嵌套，语料形态简单）
                var name = close[1].toLowerCase();
                for (var d = stack.length - 1; d >= 0; d--) {
                    if (stack[d].tagLower === name) { stack.length = d; break; }
                }
                i = lt + close[0].length;
                continue;
            }
            // svg 子树里的名字是大小写敏感的 XML（`viewBox`/`linearGradient`），
            // 而 HTML tokenizer 会把名字整个小写——小写后的 markup 进 usvg 不认
            // viewBox，整张图不画（innerHTML 注入的字标/图标全消失）。子树内保留
            // 作者写法，只另存小写键给闭合匹配用；子树外照旧归一。
            var inSvg = stack.some(function (n) { return n.tagLower === 'svg'; });
            // HTML5 tokenizer 接受相邻属性（`class="a"data-start=1`）：引号
            // 结束后下一个属性名可直接开始，不必有空白。parse5 实测如此。
            // 旧模式要求每个属性名前有 `\s`，整串不匹配。对齐标准：放宽
            // 属性名边界，引号值仍由后面 attrRe 单独处理。
            var open = /^<\s*([a-zA-Z][\w-]*)((?:[\s]+[^\s>]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+))?)*)\s*(\/?)\s*>/.exec(s.slice(lt));
            if (!open) {
                // 不是标签（比如 `< ` 字面量）→ 文本
                emit({ text: '<' });
                i = lt + 1;
                continue;
            }
            var tagLower = open[1].toLowerCase();
            // svg 元素自身的属性（`viewBox`）与子树内的一样大小写敏感。
            var keepCase = inSvg || tagLower === 'svg';
            var node = { tag: inSvg ? open[1] : tagLower, tagLower: tagLower, attrs: {}, kids: [] };
            var attrRe = /([\w-]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
            var am;
            while ((am = attrRe.exec(open[2]))) {
                var v = am[2] !== undefined ? am[2] : am[3] !== undefined ? am[3] : am[4] !== undefined ? am[4] : '';
                node.attrs[keepCase ? am[1] : am[1].toLowerCase()] = decodeEntities(v);
            }
            emit(node);
            if (!open[3] && !VOID_TAGS[tagLower]) stack.push(node);
            i = lt + open[0].length;
        }
        return out;
    }

    // ── 序列化（extract）─────────────────────────────────────────
    // JS 树 → DomElement JSON。裸文本不能挂 Div（DomElement::Div 无 text
    // 字段）：统一合成 P 子节点，与 html.rs 解析期 assemble_children 同语义。
    function serializeNode(el) {
        var p = el.__payload;
        var out = { type: p.type, id: p.id };
        if (p.className) out.className = p.className;
        var inline = p.style || '';
        // 构造期脚本写的自定义属性并进内联样式（见 makeStyleProxy）。
        var customs = Object.keys(el.__customBase);
        if (customs.length) {
            inline = (inline ? inline.replace(/;?\s*$/, '; ') : '') + customs.map(function (k) {
                return k + ': ' + el.__customBase[k];
            }).join('; ');
        }
        if (inline) out.style = inline;
        if (p.uaTag) out.uaTag = p.uaTag;
        // 属性表回传（选择器匹配与页内 getAttribute 的输入）。class/style/id
        // 等已单列字段，这里只带其余原文属性与脚本写过的。
        var attrPairs = [];
        for (var an in el.__attrs) attrPairs.push([an, el.__attrs[an]]);
        if (attrPairs.length) out.attrs = attrPairs;
        // svg 整树字符串通道：静态树（Rust 解析端产）与 fragment 路径同字段；
        // 脚本改过内部元素属性的 svg 按 XML 树重序列化（svgWriteAttr）。
        if (el.__svgDirty) out.inline = svgInlineFromXml(el.__svgXml);
        else if (p.inline) out.inline = p.inline;
        else if (p.__svgInline) out.inline = p.__svgInline;
        if (p.width !== undefined && p.width !== null) out.width = p.width;
        if (p.height !== undefined && p.height !== null) out.height = p.height;
        if (p.preserveAspectRatio) out.preserveAspectRatio = p.preserveAspectRatio;
        if (extension) {
            extension.writeSerializeAttrs(p, out);
        }
        // src 按 http(s) 前缀分流 path/url（与 parse/html.rs 的 img/video
        // 解析同语义，serde 端 DomElement 只认这两个字段）；静态 seed 的
        // path/url 原样带回。
        if (p.src) {
            if (/^https?:\/\//i.test(p.src)) out.url = p.src;
            else out.path = p.src;
        } else {
            if (p.path) out.path = p.path;
            if (p.url) out.url = p.url;
        }
        if (p.duration !== undefined && p.duration !== null) out.duration = p.duration;
        // 扩展字段由扩展层序列化：video（可见时间窗、媒体相位、着色矩阵、
        // 终帧可见性）与 audio（data-volume）走 serializeVideo；
        // transition（from/to/effect 协议，serde 必填字段）走 serializeTransition。
        // 标准层不写协议属性名字面量。
        if (p.type === 'video' || p.type === 'audio') extension.serializeVideo(el, p, out);
        if (p.type === 'transition') extension.serializeTransition(el, p, out);

        var sc = serializeChildren(el);
        if (sc.kids.length) {
            out.children = sc.kids;
            // p/span 承载文本却有了元素子节点 → 降级 div + 文本 P 首子
            if (sc.hasElement && p.type !== 'div') out.type = 'div';
        } else if (!sc.hasElement && p.type !== 'div') {
            // p/span/text 承载文本
            if (sc.allText || p.type === 'p' || p.type === 'span' || p.type === 'text') {
                out.text = sc.allText;
            }
        }
        return out;
    }

    // el 的 children 表示（serializeNode 与 recordTreeChange 增量上报共用，
    // 两路必须同语义——否则提取期快照与运行期 child_override 对子树的理解
    // 分叉：remotion 语料动态文本行消失的根因就是增量路只收元素子，文本
    // 节点整类蒸发成 `[]`）。
    function serializeChildren(el) {
        var p = el.__payload;
        var hasElement = false;
        var textKids = '';
        el.__kids.forEach(function (k) {
            if (k.nodeType === 1) hasElement = true;
            else if (k.nodeType === 3) textKids += k.data;
        });

        // 前后空白裁剪与 html.rs 一致：只裁带换行的缩进空白；单个边界空格是
        // 行内兄弟之间的词间距（`figma <span>motion</span> works`），保留。
        // 中间空白保留（CSS 折叠在渲染端）。
        // 注意 JS 的 `\s` 含 U+00A0（&nbsp;）——它是不可折叠空格，必须保留，
        // 否则 k3 逐字 span 用 innerHTML="&nbsp;" 占位的空格会消失。
        var allText = ((p.text || '') + textKids)
            .replace(/^[ \t\n\r\f]*\n[ \t\n\r\f]*/, '')
            .replace(/[ \t\n\r\f]*\n[ \t\n\r\f]*$/, '');

        var kids = [];
        if (!hasElement) {
            if (p.type === 'div' && (allText || el.__textDynamic)) {
                kids.push({ type: 'p', id: p.id + ':t', text: allText });
            }
            return { kids: kids, allText: allText, hasElement: false };
        }

        // 有 element 子节点：文本 run 按原位交错合成 P（匿名行内盒），元素
        // 子节点原序保留——Chrome 把容器内联文本按 DOM 位置做成匿名 flex
        // item / 行内盒，并成单 P 首子会把 `<span>C</span> Export to Canva`
        // 的文字排到图标前面（claude-design s3 菜单行图标/文案换位）。与
        // 静态种子路径（parse/html.rs 按位交错 + 独立 id）同语义。首个 run
        // 维持 `:t` 惯例，后续 `:t1`、`:t2`…。行内兄弟间的单空格是词间距，
        // 保留；带换行的缩进空白（flex/block 子项之间）裁掉。
        var run = p.text || '';
        var runIndex = 0;
        var flush = function () {
            var text = run
                .replace(/^[ \t\n\r\f]*\n[ \t\n\r\f]*/, '')
                .replace(/[ \t\n\r\f]*\n[ \t\n\r\f]*$/, '');
            if (text) {
                kids.push({ type: 'p', id: p.id + (runIndex === 0 ? ':t' : ':t' + runIndex), text: text });
            }
            runIndex += 1;
            run = '';
        };
        el.__kids.forEach(function (k) {
            if (k.nodeType === 3) { run += k.data; return; }
            flush();
            kids.push(serializeNode(k));
        });
        flush();
        return { kids: kids, allText: allText, hasElement: true };
    }

    // ── 元素 face ─────────────────────────────────────────────────
    // react-dom 19 的 commit 路径会触碰：nodeValue（commitTextUpdate）、
    // ownerDocument（getRootNode/contains 语义链）。nodeValue 写必须上报
    // 宿主（文本池化在 serializeNode 里并进宿主的 text 槽）——G8 打字机
    // 类动态文本就落这里。
    function HtmlTextNodeFace(type, name, data) {
        this.nodeType = type;
        this.nodeName = name;
        this.data = String(data);
        this.parentNode = null;
    }
    HtmlTextNodeFace.prototype.toString = function () { return this.data; };
    // configurable: true——本文件每帧重 eval，defineProperty 二次执行不可炸。
    Object.defineProperty(HtmlTextNodeFace.prototype, 'nodeValue', {
        configurable: true,
        get: function () { return this.data; },
        set: function (v) {
            this.data = String(v == null ? '' : v);
            var host = this.parentNode;
            if (host && host.nodeType === 1) recordTreeChange(host);
        },
    });
    Object.defineProperty(HtmlTextNodeFace.prototype, 'textContent', {
        configurable: true,
        get: function () { return this.data; },
        set: function (v) { this.nodeValue = v; },
    });
    Object.defineProperty(HtmlTextNodeFace.prototype, 'ownerDocument', {
        configurable: true,
        get: function () { return typeof documentFace === 'object' && documentFace ? documentFace : null; },
    });
    function HtmlText(data) {
        HtmlTextNodeFace.call(this, 3, '#text', data);
    }
    HtmlText.prototype = Object.create(HtmlTextNodeFace.prototype);
    HtmlText.prototype.constructor = HtmlText;
    // react-dom 的 commitPlacement 走 insertBefore/appendChild，注释节点
    // 只占位（React Suspense/Fragment 边界用），渲染语义无像素。
    function CommentFace(data) {
        HtmlTextNodeFace.call(this, 8, '#comment', data);
    }
    CommentFace.prototype = Object.create(HtmlTextNodeFace.prototype);
    CommentFace.prototype.constructor = CommentFace;

    function FragmentFace() {
        this.nodeType = 11;
        this.nodeName = '#document-fragment';
        this.__kids = [];
        this.parentNode = null;
    }
    FragmentFace.prototype.appendChild = function (child) {
        if (!child) return child;
        if (child.parentNode) child.parentNode.removeChild(child);
        child.parentNode = this;
        this.__kids.push(child);
        return child;
    };

    // 外部库初始化期间（__ocExtPhase）造的探测元素另立命名空间：它们只在
    // 首帧存在，混进主序号会让后续帧的合成 id 整体漂移，mutation 就落空。
    var extSeq = 0;
    function nextSynthId(hint) {
        var base = String(hint || 'el').replace(/[^\w-]/g, '') || 'el';
        if (globalThis.__ocExtPhase) {
            return 'ext:' + base + ':' + (extSeq++);
        }
        var id;
        do { id = base + ':' + (synthSeq++); } while (nodes[id]);
        return id;
    }

    function createElementFace(payload) {
        return new HtmlElement(payload);
    }

    // 媒体元素（video/audio）的最小面：取帧时刻由引擎按媒体时间驱动
    // （frame = f(t)），页面的 play/pause/currentTime 不改变取帧。play 返回
    // 已兑现的 Promise——与浏览器自动播放成功同形（页面普遍 `.catch`）。
    function attachMediaFace(el) {
        el.paused = true;
        el.currentTime = 0;
        el.play = function () { this.paused = false; return Promise.resolve(); };
        el.pause = function () { this.paused = true; };
        el.load = function () {};
        if (el.__payload && el.__payload.type === 'video') {
            // three VideoTexture 的供帧协议：readyState ≥ HAVE_CURRENT_DATA
            // 让语料的 needsUpdate 判定通过；texImage2D 消费时经
            // __video_frame_pixels 按 (src, 当前媒体时刻) 取帧——浏览器由
            // 视频管线供帧，face 只承载状态与最近一帧的像素。
            el.readyState = 2;
            el.__ocVideoFace = true;
            el.__ocVideoEnsureFrame = function (timeSecs) {
                // seed media 节点的资产在 path 字段（parse/html.rs 的
                // img/video 解析专字段）；页面脚本赋值走 src。
                var src = this.__payload.src || this.__payload.path || '';
                if (!src || typeof __video_frame_pixels !== 'function') return;
                // 同一时刻不重取（逐帧重放会对同一时刻多次渲染）
                if (this.__ocVideoFrameT === timeSecs && this.__ocPixels) return;
                var frame = __video_frame_pixels(src, timeSecs);
                if (!frame || !frame.rgba) return;
                this.__ocPixels = {
                    width: frame.width,
                    height: frame.height,
                    rgba: new Uint8Array(frame.rgba),
                };
                this.__ocVideoFrameT = timeSecs;
                this.width = frame.width;
                this.height = frame.height;
                this.videoWidth = frame.width;
                this.videoHeight = frame.height;
            };
        }
    }

    function HtmlElement(payload) {
        this.nodeType = 1;
        this.__payload = payload;
        this.__styleBag = Object.create(null);
        this.__customBase = Object.create(null);
        this.__kids = []; // element/text faces；种子 children 在物化期填
        this.parentNode = null;
        this.__attrs = Object.create(null); // data-* 等动态属性袋
        this.__evs = Object.create(null); // addEventListener 事件表（img 加载等）
        // 种子 payload 的原文属性（Rust 解析端产）：属性选择器与
        // getAttribute 的匹配输入。
        if (payload && payload.attrs) {
            for (var ai = 0; ai < payload.attrs.length; ai++) {
                var an = String(payload.attrs[ai][0]).toLowerCase();
                // 有专字段的属性不复刻（class/style/id/src）。扩展协议属性
                // 也留一份原文进 __attrs：dataset
                // 与属性选择器都从这读。
                if (an === 'class' || an === 'style' || an === 'id' || an === 'src') continue;
                this.__attrs[an] = String(payload.attrs[ai][1]);
            }
        }
        this.style = makeStyleProxy(this);
        this.classList = attachClassList(this);
        if (payload && (payload.type === 'video' || payload.type === 'audio')) attachMediaFace(this);

        var self = this;
        Object.defineProperty(this, 'id', {
            get: function () { return self.__payload.id; },
            set: function (v) {
                var old = self.__payload.id;
                if (old && nodes[old] === self) delete nodes[old];
                self.__payload.id = String(v);
                nodes[self.__payload.id] = self;
            },
            enumerable: true,
        });
        Object.defineProperty(this, 'className', {
            get: function () { return self.__payload.className || ''; },
            set: function (v) { self.__payload.className = String(v || ''); reportClassChange(self); },
            enumerable: true,
        });
        Object.defineProperty(this, 'tagName', { get: function () { return elTag(self).toUpperCase(); } });
        this.nodeName = this.tagName;
        Object.defineProperty(this, 'innerHTML', {
            get: function () { return serializeHtml(self); },
            set: function (v) {
                self.__kids = [];
                // 浏览器语义：innerHTML 整体替换子节点。宿主自带的文本字段
                // 也归零——否则 serializeNode 的文本 run 会把它当首段拼进去
                // （上一句字幕与新子树的文字叠印）。
                self.__payload.text = '';
                self.__textDynamic = false;
                var frag = parseFragment(v);
                frag.forEach(function (n) { self.__kids.push(materializeFragmentNode(n, self)); });
                // 提取期之后（帧内 gsap `tl.add(()=>{el.innerHTML=…})` 置词）
                // 的子树替换必须进 store：渲染树消费的是提取期种子，不报结构
                // 变更就整段不出现（spacex 的 VO 字幕 pill 只剩空底）。纯文本
                // 片段没有 element 子节点，走 textContent 同一条文本通道。
                var hasElement = false;
                for (var i = 0; i < self.__kids.length; i++) {
                    if (self.__kids[i].nodeType === 1) { hasElement = true; break; }
                }
                if (hasElement) {
                    var sent = recordTreeChange(self);
                    // 文本通道残留：上一句纯文本 innerHTML / textContent 写进
                    // `<id>:t` 的内容比覆盖列表优先（resolve_text 先查 store），
                    // 不跟着改就与新的首段 run 叠印（字幕前一句压在新句上）。
                    // 只在覆盖真正上报后补写：提取期不报结构，写 `:t` 反而会
                    // 让 resolve 清空种子子节点（任务列表整片消失）。
                    if (sent && hasStore && self.__payload.id && self.__payload.type === 'div') {
                        var slot = self.__payload.id + ':t';
                        var firstKid = (serializeNode(self).children || [])[0];
                        var runText = firstKid && firstKid.id === slot ? (firstKid.text || '') : '';
                        __write_style_value(slot, 'text', runText);
                    }
                    return;
                }
                var text = '';
                for (var j = 0; j < self.__kids.length; j++) text += self.__kids[j].data;
                self.textContent = text;
            },
            enumerable: true,
        });
        Object.defineProperty(this, 'textContent', {
            get: function () {
                var t = self.__payload.text || '';
                self.__kids.forEach(function (k) {
                    if (k.nodeType === 3) t += k.data;
                    else if (k.nodeType === 1) t += k.textContent;
                });
                return t;
            },
            set: function (v) {
                self.__kids = [];
                var text = String(v == null ? '' : v);
                self.__payload.text = text;
                // 文本突变必须进 store：渲染树消费的是提取期种子快照，
                // payload 改动不经 extract 回传（__ocHtmlExtract 只在提取期
                // 调一次）。gsap 的 tl.call 打字/置词都走这条 setter。
                // 目标 id 与 serializeNode 同规则：div 型元素的文本落在合成
                // 子节点 `<id>:t`；提取期写过文本的元素即使为空也保留该子
                // 节点（__textDynamic），渲染期的突变才有落点。
                self.__textDynamic = true;
                if (globalThis.__ocTraceJs && typeof __ocStderr === 'function') {
                    __ocStderr('[tc-set] id=' + self.__payload.id + ' type=' + self.__payload.type + ' hasStore=' + hasStore + ' len=' + text.length);
                }
                if (hasStore && self.__payload.id) {
                    var target = self.__payload.type === 'div'
                        ? self.__payload.id + ':t'
                        : self.__payload.id;
                    __write_style_value(target, 'text', text);
                    if (typeof __register_measured_text === 'function') {
                        var measuredId = self.__payload.id;
                        var measuredTextId = self.__payload.type === 'div'
                            ? measuredId + ':t'
                            : measuredId;
                        __register_measured_text(measuredId, measuredTextId, text);
                    }
                }
            },
            enumerable: true,
        });
        // innerText 与 textContent 同义（渲染感知的差异——隐藏元素返回空、
        // <br> 转换行——对打字动画/置词场景无观测差异）。缺这个 setter 时
        // `el.innerText = v` 落成普通实例属性，DOM 不变且读回是假象。
        Object.defineProperty(this, 'innerText', {
            get: function () { return self.textContent; },
            set: function (v) { self.textContent = v; },
            enumerable: true,
        });
        Object.defineProperty(this, 'children', {
            get: function () { return self.__kids.filter(function (k) { return k.nodeType === 1; }); },
        });
        Object.defineProperty(this, 'childNodes', {
            get: function () { return self.__kids.slice(); },
        });
        Object.defineProperty(this, 'firstChild', {
            get: function () { return self.__kids.length ? self.__kids[0] : null; },
        });
        Object.defineProperty(this, 'lastChild', {
            get: function () { return self.__kids.length ? self.__kids[self.__kids.length - 1] : null; },
        });
        Object.defineProperty(this, 'nextSibling', {
            get: function () {
                var p = self.parentNode;
                if (!p) return null;
                var i = p.__kids.indexOf(self);
                return i >= 0 && i + 1 < p.__kids.length ? p.__kids[i + 1] : null;
            },
        });
        Object.defineProperty(this, 'previousSibling', {
            get: function () {
                var p = self.parentNode;
                if (!p) return null;
                var i = p.__kids.indexOf(self);
                return i > 0 ? p.__kids[i - 1] : null;
            },
        });
        // gsap 量 transform 后按 nextElementSibling 把元素插回原位；缺这个
        // 属性时它退回 appendChild，元素被挪到父节点末尾。
        Object.defineProperty(this, 'nextElementSibling', {
            get: function () {
                var p = self.parentNode;
                if (!p) return null;
                for (var i = p.__kids.indexOf(self) + 1; i > 0 && i < p.__kids.length; i++) {
                    if (p.__kids[i].nodeType === 1) return p.__kids[i];
                }
                return null;
            },
        });
        Object.defineProperty(this, 'previousElementSibling', {
            get: function () {
                var p = self.parentNode;
                if (!p) return null;
                for (var i = p.__kids.indexOf(self) - 1; i >= 0; i--) {
                    if (p.__kids[i].nodeType === 1) return p.__kids[i];
                }
                return null;
            },
        });
        // first/lastElementChild：插入过滤语义与 next/previousElementSibling
        // 一致（heygen mk() 的 `fxdefs.lastElementChild.firstElementChild` 取
        // insertAdjacentHTML 落进 defs 的 filter）。
        Object.defineProperty(this, 'firstElementChild', {
            get: function () {
                for (var i = 0; i < self.__kids.length; i++) {
                    if (self.__kids[i].nodeType === 1) return self.__kids[i];
                }
                return null;
            },
        });
        Object.defineProperty(this, 'lastElementChild', {
            get: function () {
                for (var i = self.__kids.length - 1; i >= 0; i--) {
                    if (self.__kids[i].nodeType === 1) return self.__kids[i];
                }
                return null;
            },
        });
    }

    function detach(child) {
        if (child.parentNode) {
            var sibs = child.parentNode.__kids;
            var i = sibs.indexOf(child);
            if (i >= 0) sibs.splice(i, 1);
        }
    }

    // ── 运行期结构上报（appendChild 通道）─────────────────────────
    // 提取期快照定型的渲染树看不见逐帧长出的子树（tl.call 里 appendChild
    // 挂上的气泡等）。提取完成后任何结构写都把宿主的当前 element 子树
    // 整体序列化上报：resolve 期按它重建宿主的 children（宿主祖先链随行，
    // CSS 后代选择器如 `组合根下的 `.user-row .bubble`` 才能命中；
    // 子树走完整 build_node 链路，store 里的样式突变按 face id 命中）。
    // 挂载关系是节点状态：persistent 文档跨帧留在 store，非持久每帧随
    // 重放的脚本自然重建。
    var extractDone = false;
    function mountedToRoot(el) {
        var cur = el;
        while (cur) {
            if (cur === rootFace) return true;
            cur = cur.parentNode;
        }
        return false;
    }
    function subtreeHasCompositionSrc(el) {
        var attrs = el.__attrs;
        if (attrs && extension &&
            attrs[extension.attr.COMPOSITION_SRC] !== undefined) return true;
        var kids = el.__kids;
        for (var i = 0; kids && i < kids.length; i++) {
            if (kids[i].nodeType === 1 && subtreeHasCompositionSrc(kids[i])) return true;
        }
        return false;
    }
    function recordTreeChange(host) {
        if ((!extractDone && !globalThis.__ocHtmlExtractDone) || !host
            || !host.__payload || !host.__payload.id) return false;
        if (!mountedToRoot(host)) return false;
        if (subtreeHasCompositionSrc(host)) return false;
        if (typeof __html_set_children !== 'function') return false;
        var chain = [];
        var cur = host;
        while (cur && cur.nodeType === 1 && cur.__payload) {
            var attrs = [];
            for (var an in cur.__attrs) attrs.push([an, cur.__attrs[an]]);
            chain.push({
                tag: cur.__payload.uaTag || cur.__payload.type || 'div',
                id: cur.__payload.id,
                class: cur.__payload.className || '',
                attrs: attrs,
            });
            cur = cur.parentNode;
        }
        // 与提取期同形：整宿主序列化后取 children——文本 run 按原位交错成
        // 合成 P（`<span class="clay">$1 trillion</span>` 两边的文案就靠它）。
        // 只发 element 子节点会把行内文本整段丢掉（VO 字幕只剩高亮词）。
        var kids = serializeChildren(host).kids;
        try {
            __html_set_children(host.__payload.id, chain, kids);
        } catch (e) {
            if (typeof __ocLogs !== 'undefined') {
                __ocLogs.push('set_children threw: ' + (e && e.message || e));
            }
            return false;
        }
        return true;
    }

    HtmlElement.prototype.appendChild = function (child) {
        if (!child) return child;
        if (child instanceof FragmentFace) {
            var kids = child.__kids.slice();
            kids.forEach(function (k) { this.appendChild(k); }, this);
            child.__kids = [];
            return child;
        }
        detach(child);
        child.parentNode = this;
        this.__kids.push(child);
        recordTreeChange(this);
        return child;
    };
    HtmlElement.prototype.insertBefore = function (child, ref) {
        if (!child) return child;
        if (!ref) return this.appendChild(child);
        if (child instanceof FragmentFace) {
            var kids = child.__kids.slice();
            var self = this;
            kids.forEach(function (k) { self.insertBefore(k, ref); });
            return child;
        }
        detach(child);
        var i = this.__kids.indexOf(ref);
        if (i < 0) { child.parentNode = this; this.__kids.push(child); }
        else { child.parentNode = this; this.__kids.splice(i, 0, child); }
        recordTreeChange(this);
        return child;
    };
    HtmlElement.prototype.removeChild = function (child) {
        var i = this.__kids.indexOf(child);
        if (i >= 0) {
            this.__kids.splice(i, 1);
            child.parentNode = null;
            recordTreeChange(this);
        }
        return child;
    };
    HtmlElement.prototype.replaceChild = function (nu, old) {
        var i = this.__kids.indexOf(old);
        if (i < 0) return this.appendChild(nu);
        this.removeChild(old);
        this.insertBefore(nu, this.__kids[i] || null);
        return old;
    };
    HtmlElement.prototype.contains = function (other) {
        var cur = other;
        while (cur) { if (cur === this) return true; cur = cur.parentNode; }
        return false;
    };
    // react-dom 的 DOM 面探测（getRootForHostInstance、commit 卸载路径）。
    // ownerDocument 延迟读 documentFace（构造期它还没赋值）。configurable:
    // true——本文件每帧重 eval，重复 defineProperty 不可再炸（dataset 已有
    // 既有实现，见下方 datasetKey 区）。
    Object.defineProperty(HtmlElement.prototype, 'ownerDocument', {
        configurable: true,
        get: function () { return typeof documentFace === 'object' && documentFace ? documentFace : null; },
    });
    Object.defineProperty(HtmlElement.prototype, 'isConnected', {
        configurable: true,
        get: function () {
            var cur = this;
            while (cur && cur.parentNode) cur = cur.parentNode;
            return cur === rootFace;
        },
    });
    HtmlElement.prototype.remove = function () {
        if (this.parentNode) this.parentNode.removeChild(this);
    };
    HtmlElement.prototype.replaceChildren = function () {
        while (this.__kids.length) this.removeChild(this.__kids[this.__kids.length - 1]);
        for (var i = 0; i < arguments.length; i++) this.appendChild(arguments[i]);
    };
    HtmlElement.prototype.closest = function (sel) {
        var parsed = parseSelector(sel);
        if (!parsed) return null;
        var cur = this;
        while (cur) {
            if (cur.nodeType === 1 && matchesSelector(cur, parsed)) return cur;
            cur = cur.parentNode;
        }
        return null;
    };
    HtmlElement.prototype.getRootNode = function () {
        var cur = this;
        while (cur && cur.parentNode) cur = cur.parentNode;
        return cur;
    };
    HtmlElement.prototype.cloneNode = function () {
        var payload = {};
        for (var k in this.__payload) {
            if (Object.prototype.hasOwnProperty.call(this.__payload, k)) payload[k] = this.__payload[k];
        }
        // 标准 DOM 的 cloneNode(true) 保留作者 id：`template.content
        // .cloneNode(true)` 复制出的节点 id 与原节点相同。引擎旧实现给每
        // 个克隆体合成 `:N` 后缀，破坏 getElementById / gsap 选择器。
        // 对齐标准：保留原 id，只有合成节点（无作者 id）才重新生成。
        var authoredId = this.__payload.id && !/:\d+$/.test(this.__payload.id);
        payload.id = authoredId ? this.__payload.id : nextSynthId(this.__payload.id);
        var copy = createElementFace(payload);
        var self = this;
        this.__kids.forEach(function (k) {
            if (k.nodeType === 3) copy.appendChild(new HtmlText(k.data));
            else if (k.nodeType === 1) copy.appendChild(k.cloneNode(true));
        });
        return copy;
    };
    HtmlElement.prototype.setAttribute = function (name, value) {
        var n = String(name).toLowerCase();
        var v = String(value);
        if (this.__svgNode) svgWriteAttr(this, String(name), v);
        if (n === 'class') { this.__payload.className = v; reportClassChange(this); }
        else if (n === 'style') this.style.cssText = v;
        else if (n === 'id') this.id = v;
        else if (n === 'src') this.__payload.src = v;
        else if (extension && extension.isProtocolAttr(n)) {
            var proto = extension.readElementAttr(n, v);
            if (proto) Object.assign(this.__payload, proto);
            else this.__attrs[n] = v;
        }
        else this.__attrs[n] = v;
    };
    HtmlElement.prototype.getAttribute = function (name) {
        var n = String(name).toLowerCase();
        if (n === 'class') return this.__payload.className || null;
        if (n === 'style') return this.__payload.style || null;
        if (n === 'id') return this.__payload.id;
        if (n === 'src') return this.__payload.src || null;
        if (extension) {
            var protoValue = extension.getElementAttr(this.__payload, n);
            if (protoValue !== undefined) return protoValue;
        }
        return Object.prototype.hasOwnProperty.call(this.__attrs, n) ? this.__attrs[n] : null;
    };
    HtmlElement.prototype.hasAttribute = function (name) {
        return this.getAttribute(name) !== null;
    };
    HtmlElement.prototype.removeAttribute = function (name) {
        var n = String(name).toLowerCase();
        if (this.__svgNode) svgWriteAttr(this, String(name), null);
        if (n === 'class') { this.__payload.className = ''; reportClassChange(this); }
        else if (n === 'style') this.__payload.style = '';
        else if (n === 'src') delete this.__payload.src;
        else delete this.__attrs[n];
    };
    // 事件面：从空操作升级为真记录 + 派发。img.src 的异步 load/error
    // （three ImageLoader）依赖它；无人派发时行为与空操作一致。
    HtmlElement.prototype.addEventListener = function (type, fn) {
        if (typeof fn !== 'function') return;
        (this.__evs[String(type)] = this.__evs[String(type)] || []).push(fn);
    };
    HtmlElement.prototype.removeEventListener = function (type, fn) {
        var a = this.__evs[String(type)];
        if (a) {
            var i = a.indexOf(fn);
            if (i >= 0) a.splice(i, 1);
        }
    };
    HtmlElement.prototype.dispatchEvent = function (ev) {
        var a = ev && this.__evs[ev.type];
        if (a) {
            a.slice().forEach(function (fn) {
                try { fn.call(this, ev); } catch (e) { /* 单监听失败不拖垮 */ }
            }, this);
        }
        return true;
    };
    HtmlElement.prototype.focus = function () {};
    HtmlElement.prototype.blur = function () {};

    // 浏览器语义:position ∈ {beforebegin, afterbegin, beforeend, afterend}。
    // 解析与物化复用 innerHTML 的同一链路(parseFragment → materialize
    // FragmentNode,自注册 nodes);<svg> 字符串走整体 inline 通道。
    HtmlElement.prototype.insertAdjacentHTML = function (position, html) {
        var pos = String(position == null ? 'beforeend' : position).toLowerCase();
        var frag = parseFragment(html);
        var self = this;
        var make = function (node) { return materializeFragmentNode(node, null); };
        if (pos === 'beforeend') {
            frag.forEach(function (n) { self.appendChild(make(n)); });
        } else if (pos === 'afterbegin') {
            var first = self.__kids[0] || null;
            frag.forEach(function (n) { self.insertBefore(make(n), first); });
        } else if (pos === 'beforebegin' || pos === 'afterend') {
            var p = self.parentNode;
            if (!p) return;
            if (pos === 'beforebegin') {
                frag.forEach(function (n) { p.insertBefore(make(n), self); });
            } else {
                var anchor = self.nextSibling;
                frag.forEach(function (n) { p.insertBefore(make(n), anchor); });
            }
        }
    };

    // dataset:__attrs 里 data-* 的 camelCase 视图;写走 setAttribute 保证
    // 与选择器/序列化同源（扩展协议属性构造期已留原文进 __attrs）。
    function datasetKey(name) {
        return 'data-' + String(name).replace(/[A-Z]/g, function (c) { return '-' + c.toLowerCase(); });
    }
    Object.defineProperty(HtmlElement.prototype, 'dataset', {
        get: function () {
            var self = this;
            return new Proxy(Object.create(null), {
                get: function (_t, prop) {
                    if (typeof prop !== 'string') return undefined;
                    var k = datasetKey(prop);
                    return Object.prototype.hasOwnProperty.call(self.__attrs, k) ? self.__attrs[k] : undefined;
                },
                set: function (_t, prop, value) {
                    if (typeof prop !== 'string') return false;
                    self.setAttribute(datasetKey(prop), value);
                    return true;
                },
                deleteProperty: function (_t, prop) {
                    if (typeof prop !== 'string') return false;
                    self.removeAttribute(datasetKey(prop));
                    return true;
                },
                has: function (_t, prop) {
                    return typeof prop === 'string'
                        && Object.prototype.hasOwnProperty.call(self.__attrs, datasetKey(prop));
                },
            });
        },
        enumerable: true,
    });

    // append/prepend：多节点变参插入（浏览器 ChildNode API）。字符串参数
    // 按文本节点处理（texture 语料 `look.append(photo, cardA, …)`）。
    HtmlElement.prototype.append = function () {
        for (var i = 0; i < arguments.length; i++) {
            var v = arguments[i];
            this.appendChild(typeof v === 'string' ? new HtmlText(v) : v);
        }
    };
    HtmlElement.prototype.prepend = function () {
        for (var i = arguments.length - 1; i >= 0; i--) {
            var v = arguments[i];
            this.insertBefore(typeof v === 'string' ? new HtmlText(v) : v, this.__kids[0] || null);
        }
    };

    // SVG 几何面（标准 SVGGeometryElement）：html 通道的 svg 查询 face 手上
    // 有原文属性（__attrs.d）。绑定 __path_total_length/__path_bbox 两个
    // realm 都可用（提取期 binding_shim_js、渲染期 ensure_runtime）。非 path
    // 形状（circle/rect 等）语料未命中，回零值，按需后续补。
    HtmlElement.prototype.getTotalLength = function () {
        if (String(this.__payload.type || '').toLowerCase() !== 'path') return 0;
        var d = this.getAttribute('d');
        return d && typeof __path_total_length === 'function' ? __path_total_length(d) : 0;
    };
    HtmlElement.prototype.getBBox = function () {
        if (String(this.__payload.type || '').toLowerCase() === 'path') {
            var d = this.getAttribute('d');
            if (d && typeof __path_bbox === 'function') {
                var b = __path_bbox(d);
                return { x: b[0], y: b[1], width: b[2], height: b[3] };
            }
        }
        return { x: 0, y: 0, width: 0, height: 0 };
    };

    // 类名变更上报：渲染树吃的是提取期种子，类名只存在 JS 侧时
    // `.arg.pill2` 这类「同一节点换类名」的样式永远不出现（stripe 终端
    // 里 `tl.call(()=>arg1.classList.add('pill2'))` 的绿药丸底色）。
    // 上报父宿主的 children 让 resolve 重建兄弟序列并按新类名重新层叠。
    function reportClassChange(el) {
        var p = el && el.parentNode;
        if (p && p.nodeType === 1) { recordTreeChange(p); }
        else if (el) { recordTreeChange(el); }
    }

    function attachClassList(el) {
        var face = {
            add: function () {
                var cls = elClasses(el);
                for (var i = 0; i < arguments.length; i++) {
                    var c = String(arguments[i]);
                    if (c && cls.indexOf(c) < 0) cls.push(c);
                }
                el.__payload.className = cls.join(' ');
                reportClassChange(el);
            },
            remove: function () {
                var cls = elClasses(el);
                for (var i = 0; i < arguments.length; i++) {
                    var at = cls.indexOf(String(arguments[i]));
                    if (at >= 0) cls.splice(at, 1);
                }
                el.__payload.className = cls.join(' ');
                reportClassChange(el);
            },
            toggle: function (c, force) {
                var cls = elClasses(el);
                var at = cls.indexOf(String(c));
                var want = force === undefined ? at < 0 : !!force;
                if (want && at < 0) { cls.push(String(c)); }
                else if (!want && at >= 0) cls.splice(at, 1);
                el.__payload.className = cls.join(' ');
                reportClassChange(el);
                return want;
            },
            contains: function (c) { return elClasses(el).indexOf(String(c)) >= 0; },
        };
        Object.defineProperty(face, 'value', { get: function () { return el.__payload.className || ''; } });
        return face;
    }

    // 布局盒：先读本帧样式盒，声明盒为空（内容自适应尺寸）时退回上一帧布局
    // 探针的视口坐标盒——`offsetWidth` 是浏览器布局的事实，页面脚本（k3 的
    // pillDims）据它算位置，恒回 0 会让所有这类定位偏掉。
    function boxOf(el) {
        var id = el.__payload.id;
        var declaredText = el.__payload.text || '';
        var raw = (typeof __read_box === 'function' && __read_box(id)) || {};
        var box = {
            left: Number(raw.left) || 0,
            top: Number(raw.top) || 0,
            width: Number(raw.width) || 0,
            height: Number(raw.height) || 0,
        };
        if ((box.width === 0 || box.height === 0) && typeof __read_layout_box === 'function') {
            var laid = __read_layout_box(id);
            if (laid) {
                box.left = Number(laid.left) || 0;
                box.top = Number(laid.top) || 0;
                box.width = Number(laid.width) || 0;
                box.height = Number(laid.height) || 0;
            }
        }
        if (box.width === 0 && typeof __text_element_boxes === 'function') {
            var ownInk = (__text_element_boxes(id, []) || [])[0];
            if (ownInk && Number(ownInk.width) > 0) {
                box.width = Number(ownInk.width);
                box.height = Math.max(box.height, Number(ownInk.height) || 0);
            }
        }
        if (box.width === 0 && typeof __measure_text_px === 'function'
            && declaredText) {
            box.width = Number(__measure_text_px(id, declaredText)) || 0;
        }
        // `div` 的文本 run 序列化成 `<id>:t` 子节点；同步布局探针没有它
        // 的布局盒时，用该 run 的字形墨迹盒作为行内元素的 CSSOM 矩形。
        var slot = id + ':t';
        if ((box.width === 0 || box.height === 0) && typeof __read_layout_box === 'function') {
            var slotLaid = __read_layout_box(slot);
            if (slotLaid) {
                box.left = Number(slotLaid.left) || 0;
                box.top = Number(slotLaid.top) || 0;
                box.width = Number(slotLaid.width) || 0;
                box.height = Number(slotLaid.height) || 0;
            }
        }
        if ((box.width === 0 || box.height === 0) && typeof __text_element_boxes === 'function') {
            var slotInk = (__text_element_boxes(slot, []) || [])[0];
            if (slotInk) {
                box.left = Number(slotInk.left) || 0;
                box.top = Number(slotInk.top) || 0;
                box.width = Number(slotInk.width) || 0;
                box.height = Number(slotInk.height) || 0;
            }
        }
        return box;
    }

    // <img> 的加载：字节 → 宿主解码（__image_decode_pixels，SVG 分支走
    // resvg 光栅）→ 尺寸 + RGBA 像素一把抓，缓存在 face 的 `__ocPixels`
    // （three 的 texImage2D/texSubImage2D 对 img face 的像素源）。下游
    // getDimensions 读 `naturalWidth || width` 分配纹理存储——face 缺尺寸
    // 会发出 texStorage2D(NaN, undefined×undefined)，GL 分配失败后 three
    // 每帧重试上传，命令流逐帧卡死在 TEX_STORAGE_2D（liquid wordmark 青屏
    // 根因）；缺像素则 texSubImage2D 静默记空流，纹理全零透明。
    function decodeImgFace(self, url) {
        var len = __opencatAssetStat(url);
        if (len <= 0) return false;
        var bytes = new Uint8Array(len);
        __opencatAssetReadInto(url, bytes);
        var plain = Array.from(bytes);
        if (typeof __image_decode_pixels !== 'function') return false;
        var decoded = __image_decode_pixels(plain);
        if (!decoded || !decoded.rgba) return false;
        var rgba = new Uint8Array(decoded.rgba);
        self.__ocPixels = { width: decoded.width, height: decoded.height, rgba: rgba };
        self.naturalWidth = self.width = decoded.width;
        self.naturalHeight = self.height = decoded.height;
        return true;
    }
    // img.src 赋值 → 异步解码 + load/error 事件（three ImageLoader 的
    // 浏览器语义：监听挂在 src 赋值之前，微任务后派发）。uaTag 判 img
    // （createElementNS('img') 的 face type 走 FRAG_TAG_MAP 归一成 div，
    // uaTag 保留原文）。
    Object.defineProperty(HtmlElement.prototype, 'src', {
        get: function () { return this.__payload.src || ''; },
        set: function (v) {
            this.setAttribute('src', v);
            var self = this;
            if (String(this.__payload.uaTag || '') !== 'img') return;
            Promise.resolve().then(function () {
                var url = self.__payload.src || '';
                var ok = typeof __opencatAssetStat === 'function' && __opencatAssetStat(url) >= 0;
                if (ok && typeof __opencatAssetReadInto === 'function') {
                    ok = decodeImgFace(self, url);
                }
                self.complete = true;
                self.dispatchEvent({ type: ok ? 'load' : 'error', target: self });
            });
        },
    });

    HtmlElement.prototype.getBoundingClientRect = function () {
        var b = boxOf(this);
        return {
            x: b.left, y: b.top, left: b.left, top: b.top,
            right: b.left + b.width, bottom: b.top + b.height,
            width: b.width, height: b.height,
        };
    };
    Object.defineProperty(HtmlElement.prototype, 'offsetWidth', { get: function () { return boxOf(this).width; } });
    Object.defineProperty(HtmlElement.prototype, 'offsetHeight', { get: function () { return boxOf(this).height; } });
    Object.defineProperty(HtmlElement.prototype, 'offsetLeft', { get: function () { return boxOf(this).left; } });
    Object.defineProperty(HtmlElement.prototype, 'offsetTop', { get: function () { return boxOf(this).top; } });
    Object.defineProperty(HtmlElement.prototype, 'clientWidth', { get: function () { return boxOf(this).width; } });
    Object.defineProperty(HtmlElement.prototype, 'clientHeight', {
        get: function () {
            var b = boxOf(this);
            if (globalThis.__ocTraceJs && typeof __ocStderr === 'function') {
                __ocStderr('[clientH] id=' + this.__payload.id + ' h=' + b.height.toFixed(1) + ' top=' + b.top.toFixed(1));
            }
            return b.height;
        },
    });
    // scrollHeight/Width：内容包围盒（含 overflow:hidden 裁掉的部分）。
    // 被裁剪的子树在 taffy 布局里仍有位置——递归取后代盒的视口
    // bottom/right 对元素自身 top/left 的偏移，与自身盒取 max。刚 append
    // 尚未布局的子树盒为空，量到的是上一次布局的内容高——push 类测量晚
    // 一步由下一次 append 补上，与帧级批布局语义一致。
    function contentExtent(el) {
        var box = boxOf(el);
        var bottom = box.top + box.height;
        var right = box.left + box.width;
        var kids = el.__kids;
        for (var i = 0; kids && i < kids.length; i++) {
            if (kids[i].nodeType !== 1) continue;
            var sub = contentExtent(kids[i]);
            if (sub.bottom > bottom) bottom = sub.bottom;
            if (sub.right > right) right = sub.right;
        }
        return { bottom: bottom, right: right };
    }
    Object.defineProperty(HtmlElement.prototype, 'scrollHeight', {
        get: function () {
            var box = boxOf(this);
            var ext = contentExtent(this);
            if (globalThis.__ocTraceJs && typeof __ocStderr === 'function') {
                __ocStderr('[scrollH] id=' + this.__payload.id + ' h=' + box.height.toFixed(1) + ' top=' + box.top.toFixed(1) + ' extBottom=' + ext.bottom.toFixed(1));
            }
            return Math.max(box.height, ext.bottom - box.top);
        },
    });
    Object.defineProperty(HtmlElement.prototype, 'scrollWidth', {
        get: function () {
            var box = boxOf(this);
            return Math.max(box.width, contentExtent(this).right - box.left);
        },
    });
    // offsetLeft/Top 是视口坐标，等价于以 body 为偏移父：挂在文档树上的元素
    // 统一回 body，游离元素回 null（浏览器语义）。gsap 据它判断元素在不在
    // 文档里，null 会走「临时挂到根上量 transform 再插回」的分支。
    Object.defineProperty(HtmlElement.prototype, 'offsetParent', {
        get: function () {
            for (var cur = this.parentNode; cur; cur = cur.parentNode) {
                if (cur === rootFace) return documentFace.body;
            }
            return null;
        },
    });
    HtmlElement.prototype.querySelector = function (sel) {
        var list = queryAll([this], sel);
        return list.length ? list[0] : null;
    };
    HtmlElement.prototype.querySelectorAll = function (sel) { return queryAll([this], sel); };
    HtmlElement.prototype.getElementsByTagName = function (tag) { return byTagNames([this], tag); };
    HtmlElement.prototype.matches = function (sel) {
        var parsed = parseSelector(sel);
        return !!parsed && matchesSelector(this, parsed);
    };

    // innerHTML 读回（调试用途为主）。匿名文本 run（引擎合成 P，uaTag 缺席）
    // 序列化成裸文本：浏览器 DOM 里裸文本就是 text node，包成 `<p>` 会让
    // innerHTML 往返（gsap clone 卡片等）把行内 run 变成块级真 p——布局
    // 随之从行内流退化成块级堆叠（figma act2 高亮短语折行断裂的根因）。
    function isSyntheticTextRun(el) {
        return el.__payload.type === 'p' && !el.__payload.uaTag;
    }
    function serializeHtml(el) {
        var s = String(el.__payload.text || '');
        el.__kids.forEach(function (k) {
            if (k.nodeType === 3) s += k.data.replace(/&/g, '&amp;').replace(/</g, '&lt;');
            else if (k.nodeType === 1) {
                // svg 子树：face 不展开 kids，整体走 __svgInline（inner 往返
                // 后 path 才不丢——figma act2 echo 克隆的 X logo）。
                if (k.__payload.__svgInline !== undefined) {
                    s += k.__payload.__svgInline;
                    return;
                }
                if (isSyntheticTextRun(k)) {
                    s += serializeHtml(k);
                    return;
                }
                s += '<' + elTag(k);
                if (k.__payload.className) s += ' class="' + k.__payload.className + '"';
                // inline style 参与渲染（echo 克隆的 av 渐变头像），往返必须带回。
                if (k.__payload.style) s += ' style="' + String(k.__payload.style).replace(/"/g, '&quot;') + '"';
                s += '>';
                s += serializeHtml(k);
                s += '</' + elTag(k) + '>';
            }
        });
        return s;
    }

    function materializeFragmentNode(node, parent) {
        if (node.text !== undefined) return new HtmlText(node.text);
        // svg 子树：整体字符串化进 inline，不展开 JS 子树
        if (node.tag === 'svg') {
            // `width`/`height` 是 svg 的内在尺寸（`<svg width="48">`）：只带
            // markup 不带属性，布局拿不到尺寸，svg 退成父盒 100%——父容器
            // 是 shrink-to-fit 的绝对定位盒子时直接量出 0 宽（heygen 头像
            // 整块不出现）。百分比留给 CSS 走，不在这里折。
            var svgLen = function (name) {
                var m = /^\s*(\d+(?:\.\d+)?)(?:px)?\s*$/.exec(String(node.attrs[name] || ''));
                return m ? parseFloat(m[1]) : undefined;
            };
            var svgPayload = {
                type: 'svg',
                id: node.attrs['id'] || nextSynthId('svg'),
                className: node.attrs['class'] || '',
                style: node.attrs['style'] || '',
                width: svgLen('width'),
                height: svgLen('height'),
                preserveAspectRatio: node.attrs['preserveAspectRatio'],
                __svgInline: xmlSerialize(node),
            };
            delete node.attrs['id'];
            var svgEl = createElementFace(svgPayload);
            svgEl.parentNode = parent;
            nodes[svgPayload.id] = svgEl;
            attachSvgQueryFaces(svgEl, svgPayload.__svgInline);
            return svgEl;
        }
        var payload = {
            type: normalizeEngineTag(node.tag),
            uaTag: node.tag,
            id: nextSynthId(node.tag),
            className: node.attrs['class'] || '',
            style: node.attrs['style'] || '',
        };
        delete node.attrs['class'];
        delete node.attrs['style'];
        var bag = Object.create(null);
        for (var a in node.attrs) {
            if (a === 'src') payload.src = node.attrs[a];
            else if (a === 'id') payload.id = node.attrs[a];
            else if (extension && extension.isProtocolAttr(a)) {
                var proto = extension.readElementAttr(a, node.attrs[a]);
                if (proto) Object.assign(payload, proto);
                else bag[a] = node.attrs[a];
            }
            else bag[a] = node.attrs[a];
        }
        var el = createElementFace(payload);
        el.parentNode = parent;
        el.__attrs = bag;
        node.kids.forEach(function (kid) { el.__kids.push(materializeFragmentNode(kid, el)); });
        nodes[payload.id] = el;
        return el;
    }

    // canvas 元素：2D 绘制统一录进 CONTEXT2D_RUNTIME 的 DrawOp 流。
    // fillText 录成 glyph path，getImageData 在消费点回放 path 成像素。
    // 未挂树 canvas 用 `__oc_offscreen_` 前缀进持久 op 流。
    var putSeq = 0;
    // 树内的 canvas 有父节点；`createElement("canvas")` 出来、只当离屏栅格用
    // 的那张从不 append，parentNode 恒为 null —— 只有前者需要合成。
    function isAttached(el) {
        return !!(el && el.parentNode);
    }
    HtmlElement.prototype.getContext = function (kind) {
        var self = this;
        var k = String(kind).toLowerCase();
        // WebGL2：走 core 的录制面（webgl_api.js 覆盖的 ctx.getContext），
        // three 的 WebGLRenderer 因此能拿到命令流上下文。
        if (k === 'webgl2' || k === 'webgl' || k === 'experimental-webgl') {
            if (self.__glCtx) return self.__glCtx;
            if (typeof ctx === 'undefined' || !ctx || typeof ctx.getContext !== 'function') return null;
            var gw = Math.max(1, Number(self.__payload.width) || 300);
            var gh = Math.max(1, Number(self.__payload.height) || 150);
            self.__glCtx = ctx.getContext(
                self.__payload.id,
                k === 'webgl2' ? 'webgl2' : 'webgl',
                {},
                gw,
                gh,
                0,
                0
            );
            return self.__glCtx;
        }
        if (k !== '2d') return null;
        if (self.__ctx2d) return self.__ctx2d;
        if (typeof ctx === 'undefined' || !ctx || typeof ctx.getContext !== 'function') return null;
        var streamId = isAttached(self)
            ? String(self.__payload.id)
            : '__oc_offscreen_html_' + String(self.__payload.id).replace(/[^A-Za-z0-9_:-]/g, '_');
        if (!isAttached(self)) {
            self.__offscreenId = streamId;
            htmlOffscreenById[streamId] = self;
        }
        var ckFace = null;
        function ensureCk() {
            if (ckFace) return ckFace;
            try { ckFace = ctx.getContext(streamId, '2d'); } catch (e) { ckFace = null; }
            if (ckFace && !isAttached(self)) {
                Object.defineProperty(ckFace, 'canvas', {
                    get: function () { return self; },
                    configurable: true,
                });
            }
            self.__ckFace = ckFace;
            return ckFace;
        }
        self.__ctx2d = new Proxy({}, {
            get: function (_t, prop) {
                if (prop === 'canvas') return self;
                if (prop === 'createImageData') {
                    return function (w, h) {
                        var ww = Math.max(1, Math.round(Number(w) || 1));
                        var hh = Math.max(1, Math.round(Number(h) || 1));
                        return { width: ww, height: hh, data: new Uint8ClampedArray(ww * hh * 4) };
                    };
                }
                if (prop === 'putImageData') {
                    return function (img, dx, dy) {
                        if (!img || !img.data || typeof __opencatPutImageData !== 'function') return;
                        var src = img.data;
                        var u8 = src instanceof Uint8Array
                            ? src
                            : new Uint8Array(src.buffer, src.byteOffset, src.byteLength);
                        putSeq += 1;
                        // 内容寻址键包含帧时刻与像素序号；同帧重复写入或
                        // 跨帧同序号写入不同像素都不会撞 id。
                        var t = Number(globalThis.__ocSeekT);
                        __opencatPutImageData(
                            streamId,
                            'html-put:' + streamId + ':' + t + ':' + putSeq,
                            u8,
                            Number(dx) || 0,
                            Number(dy) || 0,
                            img.width,
                            img.height
                        );
                    };
                }
                if (prop === 'getImageData') {
                    var reader = ensureCk();
                    if (!reader) throw new Error('getImageData: 2D recording face unavailable');
                    return reader.getImageData.bind(reader);
                }
                var c = ensureCk();
                if (c && c[prop] !== undefined) {
                    var cv = c[prop];
                    return typeof cv === 'function' ? cv.bind(c) : cv;
                }
                return undefined;
            },
            set: function (_t, prop, value) {
                var c = ensureCk();
                if (c && c[prop] !== undefined) {
                    c[prop] = value;
                    return true;
                }
                return true;
            },
        });
        return self.__ctx2d;
    };
    // HTML 规范：canvas 的 width/height 赋值（同值也算）清空位图、重置 2D
    // 上下文状态。页面整帧重画前先设尺寸（半调背景等）；不丢弃已录命令的
    // 话，同帧前几遍（脚本重放 + seek 回调）的笔迹全进光栅化。
    function resetCanvasBitmap(el) {
        var face = el.__ckFace;
        if (face && typeof face.__ocResetBitmap === 'function') face.__ocResetBitmap();
        htmlOffscreenReset(el);
    }
    Object.defineProperty(HtmlElement.prototype, 'width', {
        get: function () { return Number(this.__payload.width) || (this.__payload.type === 'canvas' ? 300 : undefined); },
        set: function (v) { this.__payload.width = Number(v); resetCanvasBitmap(this); },
    });
    Object.defineProperty(HtmlElement.prototype, 'height', {
        get: function () { return Number(this.__payload.height) || (this.__payload.type === 'canvas' ? 150 : undefined); },
        set: function (v) { this.__payload.height = Number(v); resetCanvasBitmap(this); },
    });

    // ── 种子物化 ─────────────────────────────────────────────────
    // 协议字段归一由 HyperFrames 扩展层提供；标准层不写属性名。
    function normalizeSeedPayload(p) {
        return extension
            ? extension.readSeedProtocol(p)
            : p;
    }

    function materializeSeed(payload, parent) {
        normalizeSeedPayload(payload);
        var el = createElementFace(payload);
        el.parentNode = parent;
        nodes[payload.id] = el;
        if (payload.type === 'svg') attachSvgQueryFaces(el, payload.inline || payload.__svgInline);
        var kids = payload.children || [];
        for (var i = 0; i < kids.length; i++) {
            el.__kids.push(materializeSeed(kids[i], el));
        }
        // seed payload 的文本（text 字段）留在 payload —— 序列化取回。
        return el;
    }

    // ── 扩展层安装钩子 ────────────────────────────────────────────
    // UMD gsap 挂在 shim window 上，由 hoistWindowGlobals 提升到 globalThis。
    // 动画驱动由虚拟 rAF 与页面脚本完成。框架私有注册表由扩展层安装。
    if (extension) {
        extension.installTimelinesRegistry(globalThis.window);
    }

    globalThis.__ocHtmlAfterScripts = function () {
        hoistWindowGlobals();
    };

    // 浏览器里 window 就是全局对象：UMD 插件（CustomEase 等）挂到
    // `window.X` 后，页面按裸标识符 `X` 引用（spacex/claude-paper 的
    // `if(window.CustomEase){ gsap.registerPlugin(CustomEase) }`、inspector 的
    // `typeof CustomEase`）。shim window 是独立对象——裸标识符解析不到，
    // 抛 ReferenceError 让整个 composition 的 timeline 建不起来。把脚本挂上
    // 的数据属性提升到 globalThis（不覆盖已有全局；访问器是 shim 自带的
    // 视口等事实，不搬）。
    function hoistWindowGlobals() {
        if (typeof window !== 'object' || !window || window === globalThis) return;
        Object.keys(window).forEach(function (key) {
            if (key in globalThis) return;
            var desc = Object.getOwnPropertyDescriptor(window, key);
            if (desc && 'value' in desc) globalThis[key] = desc.value;
        });
    }

    // ── document face ─────────────────────────────────────────────
    // 宿主判别：导航宿主（浏览器）的 `globalThis.location` 是真导航面——
    // 写 href 就是真实页面导航，G13 的文档 URL 只能进合成面。shim 宿主
    // （V8/quickjs 的 JS_PLATFORM_SHIM location 是普通对象）可直写。
    // 能力位由 web wasm 宿主在 dispatcher 安装时声明；shim 宿主不声明。
    var __ocLocationIsNav = globalThis.__ocHtmlLocationIsNav === true;
    var __ocHostLocation = (typeof globalThis.location === 'object' && globalThis.location) || null;
    var __ocDocLocation = __ocLocationIsNav
        ? { href: '', pathname: '/', search: '', hash: '', origin: '' }
        : __ocHostLocation;
    // G13 文档 URL 面的填充入口：driver 在文档脚本执行前调用。
    // 导航宿主：只进 documentFace.location（作者脚本从 `document.location`
    // 读到合成文档 URL；裸 `location` 保持宿主真面，与浏览器同形）。
    // shim 宿主：直写 shim location（现行为不变）。
    globalThis.__ocHtmlSetLocationFace = function (href, pathname) {
        if (__ocLocationIsNav) {
            __ocDocLocation.href = href;
            __ocDocLocation.pathname = pathname;
            return;
        }
        if (__ocHostLocation) {
            __ocHostLocation.href = href;
            __ocHostLocation.pathname = pathname;
        }
    };
    // getElementsByTagName 共用体:候选含根自身与全部后裔(含 svg 查询子
    // 面,walkAll 已覆盖)。tag 匹配与选择器引擎同语义(elTag 归一),`*`
    // 通配。返回静态数组——HTML 的 live list 本通道无消费者。
    function byTagNames(rootList, tag) {
        var t = String(tag == null ? '' : tag).toLowerCase();
        var out = [];
        for (var i = 0; i < rootList.length; i++) {
            var root = rootList[i];
            if (!root) continue;
            var all = [root].concat(walkAll(root, []));
            for (var j = 0; j < all.length; j++) {
                if (t === '*' || elTag(all[j]) === t) out.push(all[j]);
            }
        }
        return out;
    }
    var documentFace = {
        nodeType: 9,
        documentElement: null, // 物化后指 rootFace
        body: null,
        head: null,
        title: '',
        readyState: 'complete',
        // react-dom 的 commitBeforeMutationEffects 读 activeElement（焦点
        // 恢复链）；无焦点语义给 null（浏览器 body 无焦点时同值）。
        activeElement: null,
        // 导航宿主给合成 location 小面（`document.location.href` 读 G13
        // 文档 URL）；shim 宿主即 JS_PLATFORM_SHIM 的 location。
        location: __ocDocLocation,
        getElementById: function (id) {
            var el = nodes[String(id)];
            return el === undefined ? null : el;
        },
        // 浏览器语义：document.querySelector 能匹配文档根元素（html/body）。
        // 种子树里 rootFace 代理 html/body，候选集要含根自身，否则
        // 组合根页面 `document.querySelector(...)`
        // 拿到 null（根就是宿主）。游离 head face 也在候选里
        // （style-loader 的 `document.querySelector("head")` 注入点）。
        docCandidates: function () {
            var all = [rootFace].concat(walkAll(rootFace, []));
            if (this.head && this.head !== rootFace) all.push(this.head);
            return all;
        },
        querySelector: function (sel) {
            var parsed = parseSelector(sel);
            if (!parsed) return null;
            var list = this.docCandidates().filter(function (el) { return matchesSelector(el, parsed); });
            return list.length ? list[0] : null;
        },
        querySelectorAll: function (sel) {
            var parsed = parseSelector(sel);
            if (!parsed) return [];
            return this.docCandidates().filter(function (el) { return matchesSelector(el, parsed); });
        },
        getElementsByTagName: function (tag) { return byTagNames([rootFace], tag); },
        createElement: function (tag) {
            var t = String(tag || 'div').toLowerCase();
            var payload = {
                type: normalizeEngineTag(t),
                uaTag: t,
                id: nextSynthId(t),
                className: '',
                style: '',
            };
            var el = createElementFace(payload);
            nodes[payload.id] = el;
            // 标准 DOM 的 HTMLTemplateElement.content 是 DocumentFragment，
            // firstChild / cloneNode 走片段的子节点。引擎 innerHTML setter
            // 已把解析结果放进 __kids，这里给 template 一个自身视图，
            // 让 content.firstChild 返回模板内容根。
            if (t === 'template') {
                Object.defineProperty(el, 'content', { get: function () { return el; } });
            }
            return el;
        },
        // SVG 命名空间建元（three 的 ImageLoader 内部 `createElementNS(xhtml,
        // 'img')` 走这里）：namespace 忽略，与 createElement 同语义。
        createElementNS: function (ns, tag) {
            return this.createElement(tag);
        },
        createTextNode: function (text) { return new HtmlText(text); },
        createComment: function (text) { return new CommentFace(text); },
        createDocumentFragment: function () { return new FragmentFace(); },
        addEventListener: function () {},
        removeEventListener: function () {},
        // react-dom / 页面库的探测面。fonts.ready 每帧被 seekToFrame await
        // （`document.fonts.ready.then(() => true)`）——永不 reject 的已决
        // Promise 即浏览器终态（无墙钟加载语义）。
        fonts: {
            ready: Promise.resolve(),
            check: function () { return true; },
            load: function () { return Promise.resolve([]); },
            addEventListener: function () {},
            removeEventListener: function () {},
            has: function () { return true; },
        },
        dispatchEvent: function () { return true; },
        getElementsByClassName: function (cls) {
            var wanted = String(cls || '').split(/\s+/).filter(Boolean);
            if (!wanted.length) return [];
            var all = [rootFace].concat(walkAll(rootFace, []));
            return all.filter(function (el) {
                var have = String((el.__payload && el.__payload.className) || '').split(/\s+/).filter(Boolean);
                return wanted.every(function (w) { return have.indexOf(w) >= 0; });
            });
        },
        // webpack publicPath 的读点（document.currentScript.src）。默认 null；
        // driver 对每个 src 型脚本前置同形对象（html.rs build_html_driver）。
        currentScript: null,
    };
    // 文档身份跨帧稳定：gsap 首次使用时缓存 `window.document`（`_doc`），之后
    // 每帧按它查选择器。本文件每帧重 eval，新文档面的闭包指向本帧物化树；
    // 旧对象留在首帧闭包里，查到的是首帧那批元素面（带上一帧的 `_gsap`
    // 缓存）——无显式起点的 `to` 补间从上一帧终值起步、逐帧复利（heygen-04
    // 卡片拖拽跑飞）。本帧的方法与字段灌进同一个对象。
    if (globalThis.__ocHtmlDocumentFace) {
        documentFace = Object.assign(globalThis.__ocHtmlDocumentFace, documentFace);
    } else {
        globalThis.__ocHtmlDocumentFace = documentFace;
    }

    // ── 帧边界 ────────────────────────────────────────────────────
    // CK 面像素读回按 id 查树内 canvas 的位图尺寸（离屏画布见 dom_api）。
    globalThis.__ocHtmlOffscreenById = function (id) {
        return htmlOffscreenById[String(id)] || null;
    };
    globalThis.__ocHtmlCanvasById = function (id) {
        var el = nodes[String(id)];
        if (!el || !el.__payload) return null;
        return {
            width: Number(el.__payload.width) || 300,
            height: Number(el.__payload.height) || 150,
        };
    };
    function htmlOffscreenReset(el) {
        var id = el.__offscreenId;
        if (!id) return;
        if (typeof __opencatOffscreenReset === 'function') __opencatOffscreenReset(id);
        if (typeof globalThis.__ocInvalidateOffscreenRaster === 'function') {
            globalThis.__ocInvalidateOffscreenRaster(id);
        }
    }
    globalThis.__ocHtmlResetFrame = function (seedJson) {
        if (seedJson) seed = seedJson;
        // lottie-web（bodymovin）把每个 `loadAnimation` 注册进 animationManager
        // 的闭包注册表且永不清理；嵌套子页的内联脚本每帧重放都调一次
        // loadAnimation，注册表逐帧持有整个 SVG renderer（timeline-launch /
        // cloud-render-launch 全片渲染 V8 OOM 的根因——external 库脚本只在
        // 首帧估一次，注册表却跨帧存活）。`AnimationItem.destroy()` 释放
        // renderer 并经 destroy 事件把自身从注册表 splice 掉，一次调用两清。
        try {
            var lt = globalThis.lottie || (typeof window !== 'undefined' && window ? window.lottie : null);
            if (lt && typeof lt.getRegisteredAnimations === 'function') {
                var anims = lt.getRegisteredAnimations();
                for (var li = 0; li < anims.length; li++) {
                    var anim = anims[li];
                    if (anim && typeof anim.destroy === 'function') {
                        try { anim.destroy(); } catch (e2) { /* 已失效的动画忽略 */ }
                    }
                }
            }
        } catch (e) { /* lottie 未加载时跳过 */ }
        nodes = Object.create(null);
        rootFace = null;
        synthSeq = 0;
        // 页脚本每帧重放、按同序重建 GL 资源：句柄序归零让句柄逐帧稳定，
        // 执行器按句柄复用 GL 对象（单调句柄会逐帧新建对象，全片 OOM）。
        if (typeof __gl_reset_handles === 'function') __gl_reset_handles();
        // 持久 renderer 的帧首重开流：module 只 eval 一次，帧 1+ 的
        // getContext 缓存命中不再发生，reset_for_frame 清流后没人重开。
        if (typeof __ocGlBeginFrameStreams === 'function') __ocGlBeginFrameStreams();
        if (!seed) return;
        rootFace = materializeSeed(seed.tree, null);
        documentFace.documentElement = rootFace;
        documentFace.body = rootFace;
        // head：html 通道没有解析态 <head>（样式在提取期并进 css_text），
        // 但页面库（style-loader 的 injectCSS、字体注入）要 document.head
        // 可 appendChild。给游离 face（不挂 rootFace——画面树零污染；
        // mountedToRoot 判定让它的结构写不上报）。
        var headFace = createElementFace({
            type: 'div',
            uaTag: 'head',
            id: 'oc-doc-head',
            className: '',
            style: 'display:none',
        });
        nodes[headFace.__payload.id] = headFace;
        documentFace.head = headFace;
    };

    // 持久文档的帧首：脚本不重跑，store 里的 DOM 状态原样留用，这里只
    // 复位帧级计数并重开 GL 流。
    globalThis.__ocHtmlBeginFrame = function () {
        seekStarted = true;
        putSeq = 0;
        if (typeof __ocGlBeginFrameStreams === 'function') __ocGlBeginFrameStreams();
    };

    // 返回对象（宿主统一 JSON.stringify；这里若先 stringify 会被双重
    // 包裹成 String，from_extract 读不到 tree）。
    globalThis.__ocHtmlExtract = function () {
        // 提取完成后的结构写（运行期 appendChild）才走上报通道；提取期
        // 建树阶段的 appendChild 由随后的 serializeNode(rootFace) 自然
        // 覆盖，上报是多余的。渲染进程的 realm 不跑 extract——driver 的
        // init 段尾（页面脚本执行完、timeline 注册完）置同一标志。
        extractDone = true;
        globalThis.__ocHtmlExtractDone = true;
        // CSS-in-JS 收集：页面库运行时建 `<style>` 元素（style-loader 的
        // CSS import、styled-components 等）append 到 document.head/根子树
        // ——html 通道的级联在提取期只有静态 css_text，运行时注入若无
        // 通道整类样式丢失（remotion 语料 .root 背景/字号全黑屏的根因）。
        // 按级联顺序并进 css 尾层（head 内 append 顺序=源序）。
        var runtimeCss = '';
        function collectStyle(el, depth) {
            if (!el || depth > 24) return;
            var kids = el.__kids || [];
            for (var i = 0; i < kids.length; i++) {
                var k = kids[i];
                if (k.nodeType !== 1) continue;
                var tag = String((k.__payload && (k.__payload.uaTag || k.__payload.type)) || '').toLowerCase();
                if (tag === 'style') runtimeCss += (k.textContent || '') + '\n';
                collectStyle(k, depth + 1);
            }
        }
        collectStyle(documentFace.head, 0);
        if (documentFace.head !== rootFace) collectStyle(rootFace, 0);
        var baseCss = seed ? seed.css : '';
        return {
            html: true,
            meta: seed ? seed.meta : {},
            css: runtimeCss ? (baseCss ? baseCss + '\n' + runtimeCss : runtimeCss) : baseCss,
            // from_extract 的 tree 是 `Vec<DomElement>`（多根约定）：单根包数组。
            tree: [serializeNode(rootFace)],
        };
    };

    // getComputedStyle 换皮：gsap CSSPlugin `_parseTransform` 从这里读
    // 当前 transform/opacity（`null.replace` 的 null 就来自 dom_api 版对
    // html 元素面返回的空值）。读 store，缺省给 CSS 初始值。
    function computedStyleFor(el) {
        var id = el && el.__payload ? el.__payload.id : null;
        var defaults = {
            transform: 'none',
            opacity: '1',
            visibility: 'visible',
            display: 'block',
            color: 'rgb(0, 0, 0)',
        };
        // 自定义属性按 CSS 继承：自身无值时沿祖先链取（脚本写入在 store
        // 的 mutation 层，样式表定义在初始层）。gsap 补间 `{"--x": v}` 的
        // 起点就从这里读。
        function customValue(name) {
            for (var cur = el; cur; cur = cur.parentNode) {
                var cid = cur.__payload ? cur.__payload.id : null;
                if (!cid) continue;
                var v = readStyle(cid, name);
                if (v !== '') return String(v);
            }
            return '';
        }
        // svg 内部 face：浏览器把属性（stroke/fill/transform/opacity）也计入
        // computed style。
        function svgValue(name) {
            return svgComputedValue(el, name);
        }
        return new Proxy({}, {
            get(_t, prop) {
                if (typeof prop !== 'string') return undefined;
                if (prop === 'getPropertyValue') {
                    return function (name) {
                        var n = String(name);
                        if (n.indexOf('--') === 0) return customValue(n);
                        if (id) {
                            var v = readStyle(id, n);
                            if (v !== '') return String(v);
                        }
                        var sv = svgValue(n);
                        if (sv !== '') return sv;
                        return Object.prototype.hasOwnProperty.call(defaults, n)
                            ? defaults[n]
                            : '';
                    };
                }
                if (prop.indexOf('--') === 0) return customValue(prop);
                // `from` 补间的终点值靠"读当前值"拿：gsap 先建 startAt（写
                // 起点）再读终点，两次读之间 store 已被起点污染 → 终点读成
                // 起点（opacity from 恒 0）。浏览器里起点经合成层延迟落地，
                // 读到的仍是补间前的值。构造期对 opacity 留一次读值快照：
                // 写入后紧接的那一次读返回快照，之后恢复实时值。
                if (prop === 'opacity' && id) {
                    var stale = globalThis.__ocOpStale || (globalThis.__ocOpStale = {});
                    if (stale[id]) {
                        delete stale[id];
                        var snap = (globalThis.__ocOpSnap || {})[id];
                        delete (globalThis.__ocOpSnap || {})[id];
                        if (snap !== undefined) return snap;
                    }
                }
                var __ret;
                if (id) {
                    var v = readStyle(id, prop);
                    if (v !== '') __ret = String(v);
                }
                if (__ret === undefined) {
                    var sv = svgValue(prop);
                    if (sv !== '') __ret = sv;
                }
                if (__ret === undefined) {
                    __ret = Object.prototype.hasOwnProperty.call(defaults, prop) ? defaults[prop] : '';
                }
                if (prop === 'opacity' && id && !seekStarted) {
                    (globalThis.__ocOpSnap || (globalThis.__ocOpSnap = {}))[id] = __ret;
                }
                return __ret;
            },
        });
    }
    globalThis.getComputedStyle = computedStyleFor;
    if (ocWinFor()) {
        ocWinFor().getComputedStyle = computedStyleFor;
    }

    // document 覆盖（HTML 文档输入面换皮；core 仍是视频渲染器，不是浏览器）。
    // gsap 从 `window.document` 缓存 doc（`Ie=window, Le=Ie.document`），
    // 所以 shimWindow 这层必须一起换——它藏在 `globalThis.__ocWindow`
    // （dom_api 的 shimWindow 是其 IIFE 局部变量，外面摸不到）。
    // 浏览器宿主里 `document` 是只读访问器（getter-only）：直接赋值会抛
    // TypeError。这里只重定义自身属性描述符；不可配置时保留宿主 document，
    // 合成面仍通过 `__ocDocument` 与 shim window 供作者脚本使用。
    function defineMutable(target, name, value) {
        try {
            Object.defineProperty(target, name, {
                configurable: true,
                writable: true,
                value: value,
            });
        } catch (e) {
            if (!Object.prototype.hasOwnProperty.call(target, name)) {
                throw e;
            }
        }
    }
    try {
        Object.defineProperty(globalThis, 'document', {
            configurable: true,
            writable: true,
            value: documentFace,
        });
    } catch (e) {
        if (!Object.prototype.hasOwnProperty.call(globalThis, 'document')) {
            throw e;
        }
    }
    // dist 模块工厂的 `new Function` 把 document 形参绑到
    // `__ocDocument || document`——dom_api 的 chart 面对未知 id 会抛
    // "unknown id"（liquid 语料 module 里 getElementById 中止的根因）；
    // html 通道这里必须换皮成 documentFace。
    globalThis.__ocDocument = documentFace;
    function ocWinFor() {
        return typeof globalThis.__ocWindow === 'object' ? globalThis.__ocWindow : null;
    }
    var ocWin = ocWinFor();
    if (ocWin) {
        try {
            Object.defineProperty(ocWin, 'document', {
                configurable: true,
                writable: true,
                value: documentFace,
            });
        } catch (e) {
            if (!Object.prototype.hasOwnProperty.call(ocWin, 'document')) {
                throw e;
            }
        }
    }
    // 浏览器原生 document 的 `currentScript` 是原型上的只读访问器；页面
    // driver 每个脚本段都会写它。把合成 document 本身的可写同名属性装到
    // 宿主 document 上，供那些仍从裸 `document` 读的库使用。
    var hostDocument = globalThis.document;
    if (hostDocument && hostDocument !== documentFace) {
        defineMutable(hostDocument, 'currentScript', documentFace.currentScript);
    }
    globalThis.__ocHtmlSetCurrentScript = function (value) {
        documentFace.currentScript = value;
        if (hostDocument && hostDocument !== documentFace) {
            defineMutable(hostDocument, 'currentScript', value);
        }
    };

    // ── react-dom / scheduler 全局垫 ─────────────────────────────
    // React 19 scheduler 用 MessageChannel 派发 work loop（无它则 setState
    // 永不被调度）。垫片把 port2.postMessage 排进 rAF 队列：注册当帧、
    // 帧宿主下一批 flush 执行——确定性异步，无墙钟。
    if (typeof globalThis.MessageChannel !== 'function') {
        globalThis.MessageChannel = function () {
            var port1 = {
                onmessage: null,
                postMessage: function () {},
            };
            var port2 = {
                onmessage: null,
                postMessage: function (msg) {
                    globalThis.requestAnimationFrame(function () {
                        if (typeof port1.onmessage === 'function') port1.onmessage({ data: msg });
                    });
                },
            };
            this.port1 = port1;
            this.port2 = port2;
        };
    }
    if (typeof globalThis.queueMicrotask !== 'function') {
        globalThis.queueMicrotask = function (fn) { return Promise.resolve().then(fn); };
    }
    if (typeof globalThis.matchMedia !== 'function') {
        globalThis.matchMedia = function (query) {
            return {
                matches: false, media: String(query), onchange: null,
                addEventListener: function () {}, removeEventListener: function () {},
                addListener: function () {}, removeListener: function () {},
                dispatchEvent: function () { return false; },
            };
        };
    }
    if (typeof globalThis.IntersectionObserver !== 'function') {
        globalThis.IntersectionObserver = function (cb) {
            this.observe = function () {};
            this.unobserve = function () {};
            this.disconnect = function () {};
            this.takeRecords = function () { return []; };
            void cb;
        };
    }
    if (typeof globalThis.BroadcastChannel !== 'function') {
        // OffthreadVideo 的抽帧频道（渲染器注入 window.remotion_broadcastChannel
        // 同语义）。离线引擎无跨 tab 面：postMessage 无人收听即 no-op。
        globalThis.BroadcastChannel = function (name) {
            this.name = String(name);
            this.onmessage = null;
            this.postMessage = function () {};
            this.close = function () {};
            this.addEventListener = function () {};
            this.removeEventListener = function () {};
        };
    }
    // Web storage（memory 版）：remotion 的 getInitialFrameState、页面库的
    // 状态持久化都直接读。离线渲染无跨帧持久语义，进程内 Map 即浏览器
    // 「会话内」行为的真子集。
    function makeStorage() {
        var m = {};
        return {
            getItem: function (k) { return Object.prototype.hasOwnProperty.call(m, String(k)) ? m[String(k)] : null; },
            setItem: function (k, v) { m[String(k)] = String(v); },
            removeItem: function (k) { delete m[String(k)]; },
            clear: function () { m = {}; },
            key: function (i) { var ks = Object.keys(m); return i >= 0 && i < ks.length ? ks[i] : null; },
            get length() { return Object.keys(m).length; },
        };
    }
    if (!globalThis.localStorage) globalThis.localStorage = makeStorage();
    if (!globalThis.sessionStorage) globalThis.sessionStorage = makeStorage();

    // shimWindow 的调度面镜像：remotion/页面库混用裸调用与 window.* 形态。
    if (ocWin) {
        if (!ocWin.requestAnimationFrame) ocWin.requestAnimationFrame = globalThis.requestAnimationFrame;
        if (!ocWin.cancelAnimationFrame) ocWin.cancelAnimationFrame = globalThis.cancelAnimationFrame;
        if (!ocWin.setTimeout) ocWin.setTimeout = function (fn, ms) { return globalThis.setTimeout(fn, ms); };
        if (!ocWin.clearTimeout) ocWin.clearTimeout = function (id) { return globalThis.clearTimeout(id); };
        if (!ocWin.setInterval) ocWin.setInterval = function (fn, ms) { return globalThis.setInterval(fn, ms); };
        if (!ocWin.clearInterval) ocWin.clearInterval = function (id) { return globalThis.clearInterval(id); };
        if (!ocWin.queueMicrotask) ocWin.queueMicrotask = globalThis.queueMicrotask;
        if (!ocWin.MessageChannel) ocWin.MessageChannel = globalThis.MessageChannel;
        if (!ocWin.matchMedia) ocWin.matchMedia = globalThis.matchMedia;
        if (!ocWin.BroadcastChannel) ocWin.BroadcastChannel = globalThis.BroadcastChannel;
        if (!ocWin.self) ocWin.self = ocWin;
        if (!ocWin.localStorage) ocWin.localStorage = globalThis.localStorage;
        if (!ocWin.sessionStorage) ocWin.sessionStorage = globalThis.sessionStorage;
        // React 19 的 root 默认 onUncaughtError/onCaughtError 走
        // window.reportError——不装的话错误在 React 内部二次抛出并被吞，
        // commit 阶段的根因无从可见。
        if (!ocWin.reportError) {
            ocWin.reportError = function (e) {
                var m = e && e.stack || String(e);
                (globalThis.__ocLogs = globalThis.__ocLogs || []).push('reportError: ' + m);
                if (typeof __ocStderr === 'function') __ocStderr('[reportError] ' + m);
            };
        }
        if (!ocWin.getSelection) ocWin.getSelection = function () { return null; };
    }
    // react-dom 19 的 instance 判定走 nodeType 数值，但页面/第三方库常见
    // instanceof——给 pass-through 构造（face 对象 instanceof 恒 false，
    // 至少 `value instanceof Object`/try-catch 探测不炸）。
    ['Node', 'Element', 'HTMLElement', 'Text', 'Comment', 'DocumentFragment', 'SVGElement', 'ShadowRoot', 'HTMLIFrameElement'].forEach(function (n) {
        if (typeof globalThis[n] !== 'function') {
            globalThis[n] = new Function('return function ' + n + '() { throw new TypeError("Illegal constructor: ' + n + ' is not supported in this renderer"); }')();
        }
        // react-dom 的探测形态是 `elem instanceof window.HTMLIFrameElement`
        // （getActiveElementDeep）——window 面必须同步有。
        if (ocWin && !ocWin[n]) ocWin[n] = globalThis[n];
    });

    // ── 真延迟 setTimeout（web 平台语义补全）────────────────────
    // dom_api 的降级版丢 delay 参数（每 pass 一步）：页面库的
    // `await new Promise(r => setTimeout(r, 300))` 变成下一帧就跑——
    // delayRender 类超时保护、节流、防抖的语义全错。这里按虚拟钟排程：
    // due 未到不执行，flush 前先跑到期表。0ms 也越批（浏览器定时器最小
    // 一步）。__ocVirtTimerAdvance 是提取期水位（html.rs 的 drain 循环
    // 推进——提取期钟恒 0，轮询类续体靠水位落定）；渲染 realm 全新
    // 重建，水位不跨进程，逐帧判定回到真钟。
    //
    // 导航宿主（真浏览器）不装：wasm realm 与页面共一个 global，覆盖
    // `globalThis.setTimeout` 会劫持页面/harness 自己的定时器，而虚拟钟
    // 的到期表只在 realm flush（帧内）推进——帧外的任何
    // `await new Promise(r => setTimeout(r, 0))`（渲染半环的 open 后泵、
    // warmup 泵）从此永不落定，主线程闲等死锁（omp-reduction web 腿
    // drawFrame 挂死根因）。浏览器里原生定时器本来就是正确语义，逐帧
    // 确定性由 build_frame_ir 的种子与帧驱动保证，不靠拦截时钟。
    if (!globalThis.__ocVirtTimersInstalled && globalThis.__ocHtmlLocationIsNav !== true) {
        globalThis.__ocVirtTimersInstalled = true;
        var __ocTimerSeq = 0;
        var __ocTimers = [];
        var __ocTimerFloor = 0;
        globalThis.__ocVirtTimerAdvance = function (ms) {
            if (ms > __ocTimerFloor) __ocTimerFloor = ms;
        };
        globalThis.setTimeout = function (fn, ms) {
            var id = ++__ocTimerSeq;
            var delay = Number(ms);
            __ocTimers.push({
                id: id,
                due: Math.max(globalThis.__ocClockMs ? globalThis.__ocClockMs() : 0, __ocTimerFloor)
                    + (delay > 0 ? delay : 1),
                fn: fn,
            });
            return id;
        };
        globalThis.clearTimeout = function (id) {
            for (var i = 0; i < __ocTimers.length; i++) {
                if (__ocTimers[i].id === id) { __ocTimers.splice(i, 1); return; }
            }
        };
        globalThis.__ocRunDueTimers = function () {
            var now = Math.max(globalThis.__ocClockMs ? globalThis.__ocClockMs() : 0, __ocTimerFloor);
            for (var i = 0; i < __ocTimers.length;) {
                if (__ocTimers[i].due <= now) {
                    var t = __ocTimers.splice(i, 1)[0];
                    try { t.fn(); } catch (e) {
                        (globalThis.__ocLogs = globalThis.__ocLogs || []).push('timer threw: ' + (e && e.stack || e));
                    }
                } else i++;
            }
        };
        var __ocRawFlushVirt = globalThis.__ocFlushRaf;
        globalThis.__ocFlushRaf = function () { globalThis.__ocRunDueTimers(); __ocRawFlushVirt(); };
        if (ocWin && !ocWin.setTimeout) {
            ocWin.setTimeout = function (fn, ms) { return globalThis.setTimeout(fn, ms); };
            ocWin.clearTimeout = function (id) { return globalThis.clearTimeout(id); };
        }
    }
})();
