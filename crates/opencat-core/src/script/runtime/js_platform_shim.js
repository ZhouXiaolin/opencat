(function () {
    if (typeof globalThis.TextEncoder === 'undefined') {
        globalThis.TextEncoder = function TextEncoder() {};
        globalThis.TextEncoder.prototype.encode = function (input) {
            var out = [];
            var text = String(input == null ? '' : input);
            for (var i = 0; i < text.length; i++) {
                var code = text.charCodeAt(i);
                if (code < 0x80) out.push(code);
                else if (code < 0x800) out.push(0xc0 | (code >> 6), 0x80 | (code & 0x3f));
                else out.push(0xe0 | (code >> 12), 0x80 | ((code >> 6) & 0x3f), 0x80 | (code & 0x3f));
            }
            return new Uint8Array(out);
        };
    }
    if (typeof globalThis.TextDecoder === 'undefined') {
        globalThis.TextDecoder = function TextDecoder() {};
        globalThis.TextDecoder.prototype.decode = function (bytes) {
            if (!bytes) return '';
            var out = '';
            for (var i = 0; i < bytes.length;) {
                var b = bytes[i++];
                if (b < 0x80) out += String.fromCharCode(b);
                else if (b < 0xe0) out += String.fromCharCode(((b & 0x1f) << 6) | (bytes[i++] & 0x3f));
                else out += String.fromCharCode(((b & 0x0f) << 12) | ((bytes[i++] & 0x3f) << 6) | (bytes[i++] & 0x3f));
            }
            return out;
        };
    }
    if (typeof globalThis.Blob === 'undefined') {
        globalThis.Blob = function Blob(parts) {
            this.parts = parts || [];
            this.size = this.parts.reduce(function (sum, part) {
                return sum + (part && part.length ? part.length : 0);
            }, 0);
            this.type = '';
        };
    }
    // ── the realm clock ──────────────────────────────────────────────
    // One virtual clock for the whole realm: the frame host sets it
    // (`__ocSetClock`) before each replay, and every library clock read —
    // g2's tween engine, gsap's ticker math, rAF timestamps — sees one
    // coherent `frame = f(t)` time. `performance.now` and `Date.now` are
    // rewired to it; a library reading the host's real clock inside the
    // realm would be nondeterministic by construction.
    var clockMs = 0;
    globalThis.__ocSetClock = function (secs) {
        clockMs = Math.max(0, Number(secs) || 0) * 1000;
    };
    globalThis.__ocClockMs = function () { return clockMs; };
    globalThis.performance = {
        now: function () { return clockMs; },
        mark: function () {},
        measure: function () {},
        getEntriesByName: function () { return []; },
        getEntriesByType: function () { return []; },
        clearMarks: function () {},
        clearMeasures: function () {},
    };
    // The Date shape mirrors the clock: `Date.now()` returns the virtual
    // millisecond, and a constructor call binds to the clock instead of the
    // host wall clock. The native Date stays as the prototype carrier, so
    // every calendar method (`getHours`, `toISOString`, …) keeps working on
    // the instance's own epoch value — which is the virtual one.
    var NativeDate = Date;
    globalThis.Date = function () {
        if (!(this instanceof globalThis.Date)) return new NativeDate(clockMs).toString();
        if (arguments.length === 0) return new NativeDate(clockMs);
        if (arguments.length === 1 && typeof arguments[0] === 'string') {
            return new NativeDate(globalThis.Date.parse(arguments[0]));
        }
        return new (Function.prototype.bind.apply(NativeDate, [null].concat(Array.prototype.slice.call(arguments))))();
    };
    globalThis.Date.prototype = NativeDate.prototype;
    globalThis.Date.now = function () { return clockMs; };
    // The host V8 build has no ICU: `NativeDate.UTC` is absent too, so compute
    // the UTC timestamp with the days-from-civil algorithm (proleptic
    // Gregorian), which needs no runtime calendar support at all.
    globalThis.Date.UTC = function (y, mo, da, hh, mi, ss, ms) {
        y = Math.floor(+y || 0);
        mo = Math.floor(+mo || 0);
        y += Math.floor(mo / 12);
        mo = ((mo % 12) + 12) % 12;
        var day = Math.floor(+da || 1);
        var hhv = Math.floor(+hh || 0);
        var miv = Math.floor(+mi || 0);
        var ssv = Math.floor(+ss || 0);
        var msv = Math.floor(+ms || 0);
        var m1 = mo + 1; // Hinnant's days-from-civil counts months from 1
        if (m1 <= 2) y -= 1; // Jan/Feb belong to the previous era-year
        var era = Math.floor((y >= 0 ? y : y - 399) / 400);
        var yoe = y - era * 400;
        var doy = Math.floor((153 * (m1 + (m1 > 2 ? -3 : 9)) + 2) / 5) + day - 1;
        var doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
        var days = era * 146097 + doe - 719468;
        return days * 86400000 + hhv * 3600000 + miv * 60000 + ssv * 1000 + msv;
    };
    globalThis.Date.parse = function (str) {
        // ES-conforming calendar parser: date-only forms are UTC per spec,
        // a date-time without an offset is local wall time. Falls back to
        // the native parser (when present) for non-ISO shapes.
        var s = String(str).trim();
        var m = /^(\d{4})(?:-(\d{2})(?:-(\d{2}))?)?(?:[Tt ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d+))?)?)?([Zz]|[+-]\d{2}:?\d{2})?$/.exec(s);
        if (!m) {
            return typeof NativeDate.parse === 'function' ? NativeDate.parse(s) : NaN;
        }
        var ms = globalThis.Date.UTC(
            +m[1],
            (+(m[2] || 1)) - 1,
            +(m[3] || 1),
            +(m[4] || 0),
            +(m[5] || 0),
            +(m[6] || 0),
            +(((m[7] || '') + '000').slice(0, 3))
        );
        if (isNaN(ms)) return NaN;
        if (m[8] && m[8] !== 'Z' && m[8] !== 'z') {
            var sign = m[8].charAt(0) === '-' ? 1 : -1;
            var oh = +m[8].slice(1, 3), om = +m[8].slice(-2);
            ms += sign * (oh * 60 + om) * 60000;
        } else if (m[4] !== undefined && !m[8]) {
            var probe = new NativeDate(ms);
            if (typeof probe.getTimezoneOffset === 'function') {
                ms -= probe.getTimezoneOffset() * 60000;
            }
        }
        return ms;
    };
    // A synchronous frame queue. g's `Canvas` takes `requestAnimationFrame`
    // from `raf.bind(root)` at bundle-eval time (see its raf-polyfill), so the
    // implementation has to be on the global *before* any library bundle is
    // required: the bundle caches the reference and re-assigns it to itself.
    // The queue is drained by the frame host (`__ocFlushRaf`) — a queued
    // callback runs at the next frame boundary and never on a host timer.
    // The bundle's raf-polyfill resolves its `root` through `self.self === self`
    // (then `global`) at bundle-eval time and CACHES the reference.
    if (typeof globalThis.self !== 'object' || globalThis.self === null) globalThis.self = globalThis;
    if (typeof globalThis.global !== 'object' || globalThis.global === null) globalThis.global = globalThis;
    if (typeof globalThis.__ocRafQueue !== 'object' || globalThis.__ocRafQueue === null) {
        globalThis.__ocRafQueue = [];
    }
    if (typeof globalThis.__ocRafNextId !== 'number') {
        globalThis.__ocRafNextId = 1;
    }
    globalThis.requestAnimationFrame = function (callback) {
        globalThis.__ocRafQueue.push(callback);
        return globalThis.__ocRafNextId++;
    };
    globalThis.cancelAnimationFrame = function () {};
    /* Run the queued frame callbacks exactly once (one batch, one pass).
       Callbacks registered *while* running (gsap's ticker re-arms itself
       every tick, g's canvas re-registers its draw loop) stay in the queue
       for the *next* flush — the host calls `__ocFlushRaf` once per frame,
       so a self-re-arming tick advances exactly one dt per frame, matching
       the browser's one-rAF-per-frame contract. Extraction-time loops that
       need multiple passes (promise chains, setTimeout polling) call it
       repeatedly; the frame host's drain loop already does (realm frame
       + html extract). An in-JS cap would silently advance gsap's clock
       multiple frames per host frame — that bug is why the cap is gone. */
    globalThis.__ocFlushRaf = function () {
        var batch = globalThis.__ocRafQueue;
        globalThis.__ocRafQueue = [];
        for (var i = 0; i < batch.length; i++) {
            try {
                batch[i](globalThis.__ocClockMs());
            } catch (e) {
                if (typeof __ocStderr === 'function') __ocStderr('raf callback threw: ' + (e && e.stack || e));
            }
        }
    };
    if (typeof globalThis.navigator !== 'object' || globalThis.navigator === null) {
        globalThis.navigator = {};
    }
    // g-plugin-dragndrop branches on this at init; the offline frame has no
    // pointer input, so the mouse path is the one that fits.
    if (typeof globalThis.navigator.msPointerEnabled !== 'boolean') {
        globalThis.navigator.msPointerEnabled = false;
    globalThis.console = globalThis.console || {
        log: function () { var k = globalThis.__ocLogSink; if (k) { k('log ' + Array.prototype.join.call(arguments, ' ')); } else { (globalThis.__ocLogs = globalThis.__ocLogs || []).push('console.log ' + Array.prototype.join.call(arguments, ' ')); } },
        warn: function () { var k = globalThis.__ocLogSink; if (k) { k('warn ' + Array.prototype.join.call(arguments, ' ')); } else { (globalThis.__ocLogs = globalThis.__ocLogs || []).push('console.warn ' + Array.prototype.join.call(arguments, ' ')); } },
        error: function () { var m = Array.prototype.join.call(arguments, ' '); var k = globalThis.__ocLogSink; if (k) { k('error ' + m); } if (typeof __ocStderr === 'function') { __ocStderr(m); } (globalThis.__ocLogs = globalThis.__ocLogs || []).push('console.error ' + m); },
    };
    globalThis.__ocInstallLogSink = function (fn) { globalThis.__ocLogSink = fn; };
    }
    if (typeof globalThis.navigator.userAgent !== 'string') {
        globalThis.navigator.userAgent = 'opencat';
    }
    // 浏览器 `URL` 是构造函数；页面脚本（module 通道）用它做「相对
    // import.meta.url 定位资产」的路径合并。最小语义：相对 base 目录 join +
    // `./`/`../` 归一，href/pathname/protocol 字段；绝对 scheme 直通。完整
    // URL 规范（查询/锚点/编码）语料未命中。后面 asset_api 会在同一对象上
    // 补真 blob registry 的 createObjectURL/revokeObjectURL。
    if (typeof globalThis.URL !== 'function' || globalThis.URL === null) {
        globalThis.URL = function URL(url, base) {
            var s = String(url == null ? '' : url);
            var m = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.exec(s);
            if (m) {
                this.protocol = m[0];
                this.href = s;
            } else {
                var b = '';
                if (base !== undefined && base !== null) b = String(base);
                else if (globalThis.location && globalThis.location.href) b = String(globalThis.location.href);
                var dir = b;
                var q = dir.search(/[?#]/);
                if (q >= 0) dir = dir.slice(0, q);
                dir = dir.slice(0, dir.lastIndexOf('/') + 1);
                var href = dir + s;
                var parts = href.split('/');
                var out = [];
                for (var i = 0; i < parts.length; i++) {
                    if (parts[i] === '.') continue;
                    if (parts[i] === '..' && out.length && out[out.length - 1] !== '..') { out.pop(); continue; }
                    out.push(parts[i]);
                }
                href = out.join('/');
                m = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.exec(href);
                this.protocol = m ? m[0] : '';
                this.href = href;
            }
            this.pathname = this.href.slice(this.protocol.length);
        };
        globalThis.URL.prototype.toString = function () { return this.href; };
        globalThis.URL.createObjectURL = function () { return 'blob:opencat'; };
        globalThis.URL.revokeObjectURL = function () {};
    }
    // `location`：页面脚本用它读 query（配 URLSearchParams）或拼绝对地址。
    // 离线渲染没有文档 URL，给空 query 的静态形态；reload/assign 空操作。
    if (typeof globalThis.location !== 'object' || globalThis.location === null) {
        globalThis.location = {
            href: '', protocol: '', host: '', hostname: '', port: '',
            pathname: '', search: '', hash: '', origin: '',
            reload: function () {}, replace: function () {}, assign: function () {},
            toString: function () { return this.href; },
        };
    }
    // CustomEvent：window 事件面（dom_api 记录派发）需要的事件载体。
    if (typeof globalThis.CustomEvent !== 'function') {
        globalThis.CustomEvent = function CustomEvent(type, params) {
            this.type = String(type);
            this.detail = params && params.detail !== undefined ? params.detail : null;
        };
    }
    if (typeof globalThis.Event !== 'function') {
        globalThis.Event = function Event(type) {
            this.type = String(type);
        };
    }
    if (typeof globalThis.URLSearchParams !== 'function') {
        globalThis.URLSearchParams = function URLSearchParams(init) {
            this._p = [];
            var str = '';
            if (typeof init === 'string') {
                str = init;
            } else if (init && typeof init === 'object') {
                // [ [k,v], ... ] 或 { k: v }
                if (typeof init.forEach === 'function') {
                    init.forEach(function (v, k) { this.append(k, v); }, this);
                } else {
                    for (var kk in init) {
                        if (Object.prototype.hasOwnProperty.call(init, kk)) this.append(kk, init[kk]);
                    }
                }
            } else if (init !== undefined && init !== null) {
                str = String(init);
            }
            if (str) {
                var parts = str.charAt(0) === '?' ? str.slice(1).split('&') : str.split('&');
                for (var i = 0; i < parts.length; i++) {
                    if (!parts[i]) continue;
                    var eq = parts[i].indexOf('=');
                    var name = eq < 0 ? parts[i] : parts[i].slice(0, eq);
                    var value = eq < 0 ? '' : parts[i].slice(eq + 1);
                    var swap = function (s) {
                        try { return decodeURIComponent(String(s).replace(/\+/g, ' ')); } catch (e) { return String(s); }
                    };
                    this._p.push([swap(name), swap(value)]);
                }
            }
        };
        globalThis.URLSearchParams.prototype.append = function (name, value) {
            this._p.push([String(name), String(value)]);
        };
        globalThis.URLSearchParams.prototype.get = function (name) {
            for (var i = 0; i < this._p.length; i++) {
                if (this._p[i][0] === String(name)) return this._p[i][1];
            }
            return null;
        };
        globalThis.URLSearchParams.prototype.getAll = function (name) {
            var out = [];
            for (var i = 0; i < this._p.length; i++) {
                if (this._p[i][0] === String(name)) out.push(this._p[i][1]);
            }
            return out;
        };
        globalThis.URLSearchParams.prototype.has = function (name) {
            return this.get(name) !== null;
        };
        globalThis.URLSearchParams.prototype.set = function (name, value) {
            var hit = false;
            for (var i = 0; i < this._p.length; i++) {
                if (this._p[i][0] === String(name)) {
                    if (!hit) { this._p[i][1] = String(value); hit = true; }
                    else this._p.splice(i--, 1);
                }
            }
            if (!hit) this._p.push([String(name), String(value)]);
        };
        globalThis.URLSearchParams.prototype.delete = function (name) {
            for (var i = this._p.length - 1; i >= 0; i--) {
                if (this._p[i][0] === String(name)) this._p.splice(i, 1);
            }
        };
        globalThis.URLSearchParams.prototype.forEach = function (fn, thisArg) {
            for (var i = 0; i < this._p.length; i++) fn.call(thisArg, this._p[i][1], this._p[i][0], this);
        };
        globalThis.URLSearchParams.prototype.entries = function () {
            var p = this._p.slice();
            var i = 0;
            return { next: function () { return i < p.length ? { done: false, value: p[i++].slice() } : { done: true, value: undefined }; } };
        };
        globalThis.URLSearchParams.prototype.keys = function () {
            var p = this._p.slice();
            var i = 0;
            return { next: function () { return i < p.length ? { done: false, value: p[i++][0] } : { done: true, value: undefined }; } };
        };
        globalThis.URLSearchParams.prototype.values = function () {
            var p = this._p.slice();
            var i = 0;
            return { next: function () { return i < p.length ? { done: false, value: p[i++][1] } : { done: true, value: undefined }; } };
        };
        globalThis.URLSearchParams.prototype.toString = function () {
            var esc = function (s) { return encodeURIComponent(String(s)).replace(/%20/g, '+'); };
            var out = [];
            for (var i = 0; i < this._p.length; i++) out.push(esc(this._p[i][0]) + '=' + esc(this._p[i][1]));
            return out.join('&');
        };
    }
})();
