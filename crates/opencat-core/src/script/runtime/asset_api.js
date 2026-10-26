// Web loading shims — fetch / Blob / URL.createObjectURL / createImageBitmap.
// three 的 FileLoader → GLTFLoader → ImageBitmapLoader 加载链全部走这几个
// 浏览器 API;字节后端是引擎注入的 AssetStat / AssetReadInto /
// ImageDecodeInto natives(见 engine js_context.rs),资产按文档相对路径
// 解析,图像解码直写会话 surface。没有 native 的宿主(web 回放)整个不
// 装,加载 API 保持缺省缺省态,上游走自己的错误分支。
(function () {
    if (typeof __opencatAssetStat !== 'function') return;

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
                return Promise.reject(new Error('fetch: asset not found: ' + url));
            }
            bytes = new Uint8Array(len);
            __opencatAssetReadInto(url, bytes);
        }
        return Promise.resolve(new ResponseShim(bytes));
    };

    if (!globalThis.URL) globalThis.URL = {};
    globalThis.URL.createObjectURL = function (blob) {
        var key = 'oc-blob:' + (blobSeq++);
        blobRegistry[key] = bytesOf(blob) || new Uint8Array(0);
        return key;
    };
    globalThis.URL.revokeObjectURL = function (key) {
        delete blobRegistry[key];
    };

    // 解码进会话 surface，返回 surface 形态对象（width/height/getImageData/
    // __opencatSurface）——与 CanvasTexture 的 image 输入同构，texImage2D 的
    // 上传链共用一条桥。
    globalThis.createImageBitmap = function (source) {
        var bytes = bytesOf(source);
        if (!bytes) {
            return Promise.reject(new Error('createImageBitmap: unsupported source'));
        }
        var sid = 'ibm-' + (bitmapSeq++);
        var info = __opencatImageDecodeInto(bytes, sid);
        if (!info) {
            return Promise.reject(new Error('createImageBitmap: decode failed'));
        }
        var surface = globalThis.ctx.__surfaceById(sid, info.width, info.height);
        return Promise.resolve(surface);
    };
})();
