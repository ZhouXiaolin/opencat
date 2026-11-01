// Web loading shims — fetch / Blob / URL.createObjectURL / createImageBitmap.
// three 的 FileLoader → GLTFLoader → ImageBitmapLoader 加载链全部走这几个
// 浏览器 API;字节后端是引擎注入的 AssetStat / AssetReadInto /
// ImageDecodeInto natives(见 engine js_context.rs),资产按文档相对路径
// 解析,图像解码直写会话 surface。没有 native 的宿主(web 回放)整个不
// 装,加载 API 保持缺省缺省态,上游走自己的错误分支。
(function () {
    if (typeof __opencatAssetStat !== 'function') return;

    // base64 编解码（Web 标准 atob/btoa）：离屏 canvas `toDataURL` 需要
    // btoa，`Image.src` 的 data URL 分支需要 atob。realm 没有宿主内置，
    // 这里手写实现（字节序 String.fromCharCode，块处理防栈溢出）。
    var B64_CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    var B64_LOOKUP = null;
    function makeLookup() {
        var table = new Uint8Array(128);
        for (var i = 0; i < B64_CHARS.length; i++) table[B64_CHARS.charCodeAt(i)] = i;
        return table;
    }
    if (typeof globalThis.btoa !== 'function') {
        globalThis.btoa = function (text) {
            var bytes = new Uint8Array(text.length);
            for (var i = 0; i < text.length; i++) {
                var code = text.charCodeAt(i);
                if (code > 0xff) throw new Error('btoa: source contains chars above U+00FF');
                bytes[i] = code;
            }
            var out = '';
            for (var i = 0; i < bytes.length; i += 3) {
                var b0 = bytes[i];
                var b1 = i + 1 < bytes.length ? bytes[i + 1] : 0;
                var b2 = i + 2 < bytes.length ? bytes[i + 2] : 0;
                out += B64_CHARS[b0 >> 2];
                out += B64_CHARS[((b0 & 3) << 4) | (b1 >> 4)];
                out += i + 1 < bytes.length ? B64_CHARS[((b1 & 15) << 2) | (b2 >> 6)] : '=';
                out += i + 2 < bytes.length ? B64_CHARS[b2 & 63] : '=';
            }
            return out;
        };
    }
    if (typeof globalThis.atob !== 'function') {
        globalThis.atob = function (text) {
            var clean = String(text).replace(/[^A-Za-z0-9+/]/g, '');
            if (clean.length % 4 === 1) throw new Error('atob: invalid base64 length');
            while (clean.length % 4 !== 0) clean += '=';
            var table = B64_LOOKUP || (B64_LOOKUP = makeLookup());
            var pad = 0;
            if (clean.length >= 2 && clean[clean.length - 1] === '=') pad++;
            if (clean.length >= 2 && clean[clean.length - 2] === '=') pad++;
            var outLen = Math.floor(clean.length / 4) * 3 - pad;
            var out = '';
            var chunk = [];
            for (var i = 0; i < clean.length; i += 4) {
                var a = table[clean.charCodeAt(i)];
                var b = table[clean.charCodeAt(i + 1)];
                var c = clean.charCodeAt(i + 2) === 61 ? 0 : table[clean.charCodeAt(i + 2)];
                var d = clean.charCodeAt(i + 3) === 61 ? 0 : table[clean.charCodeAt(i + 3)];
                var n = (a << 18) | (b << 12) | (c << 6) | d;
                chunk.push((n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff);
            }
            chunk.length = outLen;
            // 逐块 fromCharCode（大输入一次性 apply 会栈溢出）。
            for (var i = 0; i < chunk.length; i += 0x8000) {
                out += String.fromCharCode.apply(null, chunk.slice(i, i + 0x8000));
            }
            return out;
        };
    }


    var blobRegistry = {};
    var blobSeq = 1;
    var bitmapSeq = 1;

    function bytesOf(source) {
        if (source && source.__ocBytes) return source.__ocBytes;
        if (source instanceof ArrayBuffer) return new Uint8Array(source);
        if (source && source.buffer instanceof ArrayBuffer) {
            return new Uint8Array(source.buffer, source.byteOffset || 0, source.byteLength);
        }
        return null;
    }

    // 手动 UTF-8 解码（realm 没有 TextDecoder；只服务 FileLoader 的 text()
    // 分支，GLTFLoader 主文件走 arrayBuffer）。
    function utf8Decode(bytes) {
        var out = '';
        var i = 0;
        while (i < bytes.length) {
            var cp = bytes[i];
            if (cp < 0x80) { i += 1; }
            else if (cp < 0xe0) { cp = ((cp & 0x1f) << 6) | (bytes[i + 1] & 0x3f); i += 2; }
            else if (cp < 0xf0) {
                cp = ((cp & 0x0f) << 12) | ((bytes[i + 1] & 0x3f) << 6) | (bytes[i + 2] & 0x3f);
                i += 3;
            } else {
                cp = ((cp & 0x07) << 18) | ((bytes[i + 1] & 0x3f) << 12) |
                     ((bytes[i + 2] & 0x3f) << 6) | (bytes[i + 3] & 0x3f);
                i += 4;
            }
            out += String.fromCodePoint(cp);
        }
        return out;
    }

    function Blob(parts, options) {
        var chunks = [];
        var total = 0;
        for (var i = 0; i < parts.length; i++) {
            var part = parts[i];
            var b = bytesOf(part);
            if (!b && typeof part === 'string') {
                // 极简 UTF-8 编码：加载链只传字节，字符串分支保底。
                var utf8 = [];
                for (var k = 0; k < part.length; k++) utf8.push(part.charCodeAt(k) & 0xff);
                b = new Uint8Array(utf8);
            }
            if (b) { chunks.push(b); total += b.length; }
        }
        var out = new Uint8Array(total);
        var off = 0;
        for (var j = 0; j < chunks.length; j++) { out.set(chunks[j], off); off += chunks[j].length; }
        this.__ocBytes = out;
        this.size = total;
        this.type = (options && options.type) || '';
    }

    function ResponseShim(bytes) {
        this.__ocBytes = bytes;
        this.ok = true;
        this.status = 200;
        this.statusText = 'OK';
        // wasm-bindgen glue 在 instantiateStreaming 失败回退时读
        // `response.headers.get('Content-Type')`；本地资产无真实头，恒给
        // wasm MIME（bytes 长得像 wasm 时语义也对）。
        this.headers = new Headers({ 'Content-Type': 'application/wasm' });
    }
    ResponseShim.prototype.arrayBuffer = function () {
        return Promise.resolve(this.__ocBytes.slice().buffer);
    };
    ResponseShim.prototype.blob = function () {
        return Promise.resolve(new Blob([this.__ocBytes]));
    };
    ResponseShim.prototype.text = function () {
        return Promise.resolve(utf8Decode(this.__ocBytes));
    };
    ResponseShim.prototype.json = function () {
        return Promise.resolve(JSON.parse(utf8Decode(this.__ocBytes)));
    };
    // wasm-bindgen glue 的 `module instanceof Response` 分支（glsl_wgsl_compiler
    // 等 __wbg_load）需要全局 Response 构造器；不注册时 fetch 返回值被当
    // ArrayBuffer 处理直接 TypeError。与 Request/Headers/Blob 同批注册。
    globalThis.Response = ResponseShim;

    globalThis.Blob = Blob;

    // three FileLoader 的 fetch 中断语义:我们的 fetch 同步 resolve,中断
    // 无处发生,最小形态即可(有 signal 字段 + abort() 存在性)。
    globalThis.AbortController = function AbortController() {
        this.signal = { aborted: false, onabort: null, reason: undefined };
        var signal = this.signal;
        this.abort = function () {
            signal.aborted = true;
        };
    };

    // Loader 基类做 `typeof AbortSignal.any === 'function'` 特性检测后合并
    // signal——同步加载下中断无处发生,返回一个静态 signal 即可。
    globalThis.AbortSignal = function AbortSignal() {
        this.aborted = false;
        this.onabort = null;
        this.reason = undefined;
    };
    globalThis.AbortSignal.any = function () {
        return new globalThis.AbortSignal();
    };
    globalThis.AbortSignal.timeout = function () {
        return new globalThis.AbortSignal();
    };
    globalThis.AbortSignal.abort = function () {
        var s = new globalThis.AbortSignal();
        s.aborted = true;
        return s;
    };

    // FileLoader r181 走 new Request(url, init) → fetch(request);headers
    // 用 new Headers(init.headers) 构造。
    globalThis.Headers = function Headers(init) {
        this.map = {};
        if (init) {
            for (var k in init) this.map[String(k).toLowerCase()] = String(init[k]);
        }
    };
    Headers.prototype.get = function (name) {
        return this.map[String(name).toLowerCase()] ?? null;
    };
    Headers.prototype.set = function (name, value) {
        this.map[String(name).toLowerCase()] = String(value);
    };
    Headers.prototype.has = function (name) {
        return String(name).toLowerCase() in this.map;
    };

    globalThis.Request = function Request(url, init) {
        this.url = String(url);
        this.method = (init && init.method) || 'GET';
        this.headers = (init && init.headers) || {};
        this.signal = (init && init.signal) || null;
    };

    globalThis.fetch = function (input) {
        var url = String(input && input.url !== undefined ? input.url : input);
        url = String(url);
        var bytes = blobRegistry[url];
        if (bytes === undefined) {
            var len = __opencatAssetStat(url);
            if (len < 0) {
                var err = new Error('fetch: asset not found: ' + url);
                if (typeof console !== 'undefined' && console.error) console.error(err.message);
                return Promise.reject(err);
            }
            bytes = new Uint8Array(len);
            __opencatAssetReadInto(url, bytes);
        }
        return Promise.resolve(new ResponseShim(bytes));
    };

    if (!globalThis.URL) globalThis.URL = {};
    // 返回值必须带真 `blob:` 前缀：three 的 LoaderUtils.resolveURL 对
    // /^blob:/ 白名单放行，自定义前缀会被当相对路径拼上 resourcePath
    //（`assets/oc-blob:1`），GLB 全部内嵌贴图 404。
    globalThis.URL.createObjectURL = function (blob) {
        var key = 'blob:oc/' + (blobSeq++);
        blobRegistry[key] = bytesOf(blob) || new Uint8Array(0);
        return key;
    };
    globalThis.URL.revokeObjectURL = function (key) {
        delete blobRegistry[key];
    };

    // `<img>` / `new Image()`：只支持引擎镜像的本地资产（批量对齐面把远程
    // 图片镜像到 cwd 的 `assets/`，URL → 文件名表由入口注入 `__ocAssetMap`）。
    // 加载是异步的（微任务）——g 系渲染器先赋 `src` 再挂 `onload`，同步派发
    // 会丢回调。解码尺寸走 `__image_probe`，字节留给 `ctx.drawImage`。
    var IMAGE_OK = typeof __opencatAssetStat === 'function' && typeof __image_probe === 'function';
    function imageAssetPath(src) {
        var url = String(src);
        var map = globalThis.__ocAssetMap;
        var name = map && map[url];
        if (name) return 'assets/' + name;
        return url.replace(/^\.?\//, '');
    }
    function ImageElement(width, height) {
        this.width = Number(width) || 0;
        this.height = Number(height) || 0;
        this.naturalWidth = 0;
        this.naturalHeight = 0;
        this.complete = false;
        this.crossOrigin = null;
        this.alt = '';
        this.onload = null;
        this.onerror = null;
        this.__ocImageBytes = null;
        this.__ocListeners = { load: [], error: [] };
    }
    ImageElement.prototype.addEventListener = function (type, fn) {
        var list = this.__ocListeners[type];
        if (list && typeof fn === 'function') list.push(fn);
    };
    ImageElement.prototype.removeEventListener = function (type, fn) {
        var list = this.__ocListeners[type];
        if (!list) return;
        var i = list.indexOf(fn);
        if (i >= 0) list.splice(i, 1);
    };
    ImageElement.prototype.decode = function () {
        var self = this;
        return new Promise(function (resolve, reject) {
            if (self.complete && self.__ocImageBytes) resolve();
            else reject(new Error('decode: image is not loaded: ' + self.src));
        });
    };
    ImageElement.prototype.__ocSettle = function (err) {
        var self = this;
        Promise.resolve().then(function () {
            var type = err ? 'error' : 'load';
            var handler = self['on' + type];
            if (typeof handler === 'function') handler.call(self, { type: type, target: self });
            var list = self.__ocListeners[type];
            for (var i = 0; i < list.length; i++) list[i].call(self, { type: type, target: self });
        });
    };
    Object.defineProperty(ImageElement.prototype, 'src', {
        get: function () { return this.__ocSrc || ''; },
        set: function (value) {
            this.__ocSrc = String(value);
            // data URL（heatmap 离屏 canvas 的 toDataURL 产物）：base64
            // 解码后与文件资产同一条 __image_probe 通道，无需 asset store。
            var dataMatch = /^data:image\/[a-z+.-]+;base64,(.*)$/s.exec(this.__ocSrc);
            if (dataMatch) {
                if (typeof atob !== 'function') {
                    this.__ocSettle(new Error('Image.src: atob unavailable for data URL'));
                    return;
                }
                var bin = atob(dataMatch[1]);
                var bytes = new Uint8Array(bin.length);
                for (var i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
                var plain = Array.from(bytes);
                var size = __image_probe(plain);
                if (!size || size.length < 2) {
                    this.__ocSettle(new Error('Image.src: data URL decode failed'));
                    return;
                }
                this.__ocImageBytes = bytes;
                this.__ocImageArray = plain;
                this.naturalWidth = this.width = size[0];
                this.naturalHeight = this.height = size[1];
                this.complete = true;
                this.__ocSettle(null);
                return;
            }
            if (!IMAGE_OK) {
                this.__ocSettle(new Error('Image.src: asset store is unavailable: ' + this.__ocSrc));
                return;
            }
            var path = imageAssetPath(this.__ocSrc);
            var len = __opencatAssetStat(path);
            if (len < 0) {
                this.__ocSettle(new Error('Image.src: asset not found: ' + path));
                return;
            }
            var bytes = new Uint8Array(len);
            __opencatAssetReadInto(path, bytes);
            // 绑定层只吃普通数组（Uint8Array 会被当成 map 解码），转换一次
            // 后缓存，避免每帧 drawImage 重复展开。
            var plain = Array.from(bytes);
            var size = __image_probe(plain);
            if (!size || size.length < 2) {
                this.__ocSettle(new Error('Image.src: decode failed: ' + path));
                return;
            }
            this.__ocImageBytes = bytes;
            this.__ocImageArray = plain;
            this.naturalWidth = this.width = size[0];
            this.naturalHeight = this.height = size[1];
            this.complete = true;
            this.__ocSettle(null);
        },
    });
    globalThis.Image = ImageElement;
    // `img instanceof HTMLImageElement` 是图库的常规判断（canvas 出身的
    // 图源要区分开）；接口面与 Image 同源即可。
    if (!globalThis.HTMLImageElement) globalThis.HTMLImageElement = ImageElement;
    if (globalThis.window) {
        globalThis.window.Image = ImageElement;
        if (!globalThis.window.HTMLImageElement) globalThis.window.HTMLImageElement = ImageElement;
    }


    // 解码进会话 surface，返回 surface 形态对象（width/height/getImageData/
    // __opencatSurface）——与 CanvasTexture 的 image 输入同构，texImage2D 的
    // 上传链共用一条桥。
    globalThis.createImageBitmap = function (source) {
        var bytes = bytesOf(source);
        if (!bytes) {
            var e0 = new Error('createImageBitmap: unsupported source');
            if (typeof console !== 'undefined' && console.error) console.error(e0.message);
            return Promise.reject(e0);
        }
        var sid = 'ibm-' + (bitmapSeq++);
        var info = __opencatImageDecodeInto(bytes, sid);
        if (!info) {
            var e1 = new Error('createImageBitmap: decode failed (' + bytes.length + ' bytes)');
            if (typeof console !== 'undefined' && console.error) console.error(e1.message);
            return Promise.reject(e1);
        }
        var surface = globalThis.ctx.__surfaceById(sid, info.width, info.height);
        return Promise.resolve(surface);
    };
})();
