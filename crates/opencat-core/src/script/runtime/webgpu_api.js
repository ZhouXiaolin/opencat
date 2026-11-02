// WebGPU recording face: `navigator.gpu` + `canvas.getContext('webgpu')`
// return deterministic RECORDERS, not live GPU objects.
//
// Same contract as webgl_api.js (WebGL2), one layer up: @antv/g-device-api's
// Device_WebGPU drives the real navigator.gpu surface — requestAdapter →
// requestDevice → canvas.getContext('webgpu') → configure → per-frame
// beginRenderPass/set*/draw → queue.submit. We stub exactly that surface; the
// engine executor (WgpuStreamExecutor) replays it one-to-one onto the wgpu
// crate. GLSL→WGSL compilation (glsl_wgsl_compiler_bg.wasm, wasm-bindgen)
// runs INSIDE g-device-api upstream of createShaderModule, so the stream
// carries final WGSL — same source the Edge/Dawn reference leg compiles.
//
// Descriptors (GPUBufferDescriptor & friends) intern as JSON (serde_json on
// the executor). Resource references inside a descriptor are wrapped as
// {"h": <id>} by wrapHandles; the derived bind-group layout reference is
// {"ph": <pipeline id>, "pi": <group index>} (WebGPU derives layouts from the
// shader — wgpu mirrors this with layout: None + get_bind_group_layout).
//
// Red line holds: no pixels flow back into core. mapAsync resolves zeros
// (with a stderr warning) — the ref-vs-engine PNG diff is the pixel oracle.

(function () {
    'use strict';

    var ctx = globalThis.ctx;

    // ── platform globals for g-device-api instanceof probes ─────────
    // Texture_WebGPU.setImageData 判型第一行就是 `data instanceof
    // ImageBitmap`——realm 缺构造器时是 ReferenceError 而不是 false，
    // SDF 图集的字节上传（writeTexture else 分支）根本走不到。类型面
    // stub 只让 instanceof 正常返回 false，不改任何行为。
    if (typeof globalThis.ImageBitmap === 'undefined') {
        globalThis.ImageBitmap = function ImageBitmap() {};
    }
    if (typeof globalThis.HTMLCanvasElement === 'undefined') {
        globalThis.HTMLCanvasElement = function HTMLCanvasElement() {};
    }
    if (typeof globalThis.HTMLVideoElement === 'undefined') {
        globalThis.HTMLVideoElement = function HTMLVideoElement() {};
    }

    // ── wasm platform shim ─────────────────────────────────────────
    // The engine boots V8 with a 0-worker platform
    // (v8::new_default_platform(0, false)): asynchronous
    // WebAssembly.instantiate never settles because background compilation
    // has no thread to run on. Sync compile + immediate resolve is
    // semantically equivalent and deterministic — the wasm-bindgen glue
    // (glsl_wgsl_compiler.js) only awaits the promise.
    if (typeof WebAssembly !== 'undefined' && !WebAssembly.__ocSyncShim) {
        var syncInstantiate = function (bufSource, imports) {
            var mod =
                bufSource instanceof WebAssembly.Module
                    ? bufSource
                    : new WebAssembly.Module(bufSource);
            var instance = new WebAssembly.Instance(mod, imports || {});
            return Promise.resolve({ module: mod, instance: instance });
        };
        syncInstantiate.prototype = WebAssembly.instantiate; // keep identity probe honest
        WebAssembly.instantiate = syncInstantiate;
        WebAssembly.instantiateStreaming = function (respPromise, imports) {
            return Promise.resolve(respPromise)
                .then(function (r) { return r.arrayBuffer(); })
                .then(function (buf) { return WebAssembly.instantiate(buf, imports); });
        };
        WebAssembly.__ocSyncShim = true;
    }

    // ── opcodes — authority: ir/gpu_stream.rs wgpu_opcode ──────────
    var W = {
        BUFFER_CREATE: 0,
        TEXTURE_CREATE: 1,
        TEXTURE_VIEW_CREATE: 2,
        SAMPLER_CREATE: 3,
        BIND_GROUP_LAYOUT_CREATE: 4,
        BIND_GROUP_CREATE: 5,
        PIPELINE_LAYOUT_CREATE: 6,
        SHADER_MODULE_CREATE: 7,
        RENDER_PIPELINE_CREATE: 8,
        COMPUTE_PIPELINE_CREATE: 9,
        QUERY_SET_CREATE: 10,
        RENDER_BUNDLE_ENCODER_CREATE: 11,
        RESOURCE_DESTROY: 12,
        CANVAS_CONFIGURE: 13,
        CANVAS_GET_CURRENT_TEXTURE: 14,
        QUEUE_WRITE_BUFFER: 15,
        QUEUE_WRITE_TEXTURE: 16,
        QUEUE_SUBMIT: 17,
        ENC_BEGIN_RENDER_PASS: 18,
        ENC_COPY_BUFFER_TO_BUFFER: 19,
        ENC_COPY_TEXTURE_TO_TEXTURE: 20,
        ENC_COPY_TEXTURE_TO_BUFFER: 21,
        ENC_COPY_BUFFER_TO_TEXTURE: 22,
        ENC_FINISH: 23,
        PASS_SET_PIPELINE: 24,
        PASS_SET_BIND_GROUP: 25,
        PASS_SET_VERTEX_BUFFER: 26,
        PASS_SET_INDEX_BUFFER: 27,
        PASS_SET_VIEWPORT: 28,
        PASS_SET_SCISSOR_RECT: 29,
        PASS_SET_BLEND_CONSTANT: 30,
        PASS_SET_STENCIL_REFERENCE: 31,
        PASS_DRAW: 32,
        PASS_DRAW_INDEXED: 33,
        PASS_DRAW_INDIRECT: 34,
        PASS_DRAW_INDEXED_INDIRECT: 35,
        PASS_END: 36,
        ENC_BEGIN_COMPUTE_PASS: 37,
        PASS_DISPATCH_WORKGROUPS: 38,
        PASS_DISPATCH_WORKGROUPS_INDIRECT: 39,
    };

    var ALPHA_MODE = { opaque: 0, premultiplied: 1 };

    // ── command encoding (same wire format as webgl_api.js) ────────

    function Encoder(len) {
        this.bytes = new Uint8Array(len + 1);
        this.dv = new DataView(this.bytes.buffer);
        this.pos = 1; // byte 0 is the opcode, set by done()
    }
    Encoder.prototype.u8 = function (v) {
        this.dv.setUint8(this.pos, v);
        this.pos += 1;
        return this;
    };
    Encoder.prototype.u32 = function (v) {
        this.dv.setUint32(this.pos, v >>> 0, true);
        this.pos += 4;
        return this;
    };
    Encoder.prototype.i32 = function (v) {
        this.dv.setInt32(this.pos, v, true);
        this.pos += 4;
        return this;
    };
    Encoder.prototype.f32 = function (v) {
        this.dv.setFloat32(this.pos, v, true);
        this.pos += 4;
        return this;
    };
    // GPUBufferAddress: little-endian lo+hi pair (offsets < 4 GiB in practice).
    Encoder.prototype.u64 = function (v) {
        this.dv.setUint32(this.pos, v >>> 0, true);
        this.dv.setUint32(this.pos + 4, Math.floor(v / 4294967296) >>> 0, true);
        this.pos += 8;
        return this;
    };
    Encoder.prototype.done = function (opcode) {
        this.bytes[0] = opcode;
        return this.bytes.subarray(0, this.pos);
    };

    // g-device-api 的 Device 构造在 configure 之前就创建 fallback 纹理
    // （desc intern）——此时帧流还没开（流头在 configure/getCurrentTexture
    // 才建立）。emit/intern 先确保流按画布头开（gl_stream_begin 同 target
    // 同帧幂等，重复调用只刷新头）。
    function ensureOpenFor(targetId) {
        var ctx = registeredCanvasCtx[targetId];
        if (ctx && typeof ctx.__ocEnsureOpen === 'function') ctx.__ocEnsureOpen();
    }

    function emit(targetId, opcode, payload) {
        ensureOpenFor(targetId);
        payload[0] = opcode;
        __gl_stream_emit(targetId, Array.prototype.slice.call(payload));
    }

    function intern(targetId, bytes) {
        ensureOpenFor(targetId);
        if (typeof __opencatGLIntern === 'function') {
            if (!(bytes instanceof Uint8Array)) {
                throw new Error('intern: non-U8 ' + (bytes && bytes.constructor ? bytes.constructor.name : typeof bytes));
            }
            return __opencatGLIntern(targetId, bytes);
        }
        return __gl_stream_intern(targetId, Array.prototype.slice.call(bytes));
    }

    function utf8(str) {
        var out = [];
        for (var i = 0; i < str.length; i++) {
            var cp = str.codePointAt(i);
            if (cp > 0xffff) i++;
            if (cp < 0x80) {
                out.push(cp);
            } else if (cp < 0x800) {
                out.push(0xc0 | (cp >> 6), 0x80 | (cp & 0x3f));
            } else if (cp < 0x10000) {
                out.push(0xe0 | (cp >> 12), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f));
            } else {
                out.push(
                    0xf0 | (cp >> 18),
                    0x80 | ((cp >> 12) & 0x3f),
                    0x80 | ((cp >> 6) & 0x3f),
                    0x80 | (cp & 0x3f)
                );
            }
        }
        return new Uint8Array(out);
    }

    function internString(targetId, str) {
        return intern(targetId, utf8(str));
    }

    function internDesc(targetId, desc) {
        return internString(targetId, JSON.stringify(wrapHandles(desc)));
    }

    // typed arrays / ArrayBuffers pass through as raw bytes.
    function typedBytes(data) {
        if (data && data.buffer instanceof ArrayBuffer) {
            return new Uint8Array(data.buffer, data.byteOffset || 0, data.byteLength);
        }
        if (data instanceof ArrayBuffer) return new Uint8Array(data);
        return null;
    }

    // ── resource stubs & handle wrapping ───────────────────────────

    function makeStub(ctor, kind, id) {
        var s = new ctor();
        s.__gpuKind = kind;
        s.__gpuId = id;
        return s;
    }

    function stubId(v, api) {
        if (v === null || v === undefined) return 0xffffffff;
        if (typeof v === 'object' && typeof v.__gpuId === 'number') return v.__gpuId;
        throw new Error(api + ': not a GPU resource handle');
    }

    // Recursively replace stubs with {"h": id}; derived-layout markers with
    // {"ph": pipeline, "pi": index}.
    function wrapHandles(v) {
        if (Array.isArray(v)) {
            var arr = new Array(v.length);
            for (var i = 0; i < v.length; i++) arr[i] = wrapHandles(v[i]);
            return arr;
        }
        if (v && typeof v === 'object') {
            if (typeof v.__gpuId === 'number') return { h: v.__gpuId };
            if (typeof v.__ocPipeline === 'number') return { ph: v.__ocPipeline, pi: v.__ocLayoutIndex };
            var out = {};
            for (var k in v) out[k] = wrapHandles(v[k]);
            return out;
        }
        return v;
    }

    // ── global constructors (instanceof probes in libraries) ───────
    function GPUAdapter() {}
    function GPUDevice() {}
    function GPUCanvasContext() {}
    function GPUBuffer() {}
    function GPUTexture() {}
    function GPUTextureView() {}
    function GPUSampler() {}
    function GPUBindGroupLayout() {}
    function GPUBindGroup() {}
    function GPUPipelineLayout() {}
    function GPUShaderModule() {}
    function GPURenderPipeline() {}
    function GPUComputePipeline() {}
    function GPUCommandEncoder() {}
    function GPURenderPassEncoder() {}
    function GPUComputePassEncoder() {}
    function GPUCommandBuffer() {}
    function GPUQuerySet() {}
    function GPURenderBundleEncoder() {}
    function GPURenderBundle() {}
    globalThis.GPUAdapter = GPUAdapter;
    globalThis.GPUDevice = GPUDevice;
    globalThis.GPUCanvasContext = GPUCanvasContext;
    globalThis.GPUBuffer = GPUBuffer;
    globalThis.GPUTexture = GPUTexture;
    globalThis.GPUTextureView = GPUTextureView;
    globalThis.GPUSampler = GPUSampler;
    globalThis.GPUBindGroupLayout = GPUBindGroupLayout;
    globalThis.GPUBindGroup = GPUBindGroup;
    globalThis.GPUPipelineLayout = GPUPipelineLayout;
    globalThis.GPUShaderModule = GPUShaderModule;
    globalThis.GPURenderPipeline = GPURenderPipeline;
    globalThis.GPUComputePipeline = GPUComputePipeline;
    globalThis.GPUCommandEncoder = GPUCommandEncoder;
    globalThis.GPURenderPassEncoder = GPURenderPassEncoder;
    globalThis.GPUComputePassEncoder = GPUComputePassEncoder;
    globalThis.GPUCommandBuffer = GPUCommandBuffer;
    globalThis.GPUQuerySet = GPUQuerySet;
    globalThis.GPURenderBundleEncoder = GPURenderBundleEncoder;
    globalThis.GPURenderBundle = GPURenderBundle;

    // ── spec constant namespaces ───────────────────────────────────
    globalThis.GPUTextureUsage = {
        COPY_SRC: 0x01,
        COPY_DST: 0x02,
        TEXTURE_BINDING: 0x04,
        STORAGE_BINDING: 0x08,
        RENDER_ATTACHMENT: 0x10,
    };
    globalThis.GPUBufferUsage = {
        MAP_READ: 0x0001,
        MAP_WRITE: 0x0002,
        COPY_SRC: 0x0004,
        COPY_DST: 0x0008,
        INDEX: 0x0010,
        VERTEX: 0x0020,
        UNIFORM: 0x0040,
        STORAGE: 0x0080,
        INDIRECT: 0x0100,
        QUERY_RESOLVE: 0x0200,
    };
    globalThis.GPUShaderStage = { VERTEX: 0x1, FRAGMENT: 0x2, COMPUTE: 0x4 };
    globalThis.GPUMapMode = { READ: 0x1, WRITE: 0x2 };
    globalThis.GPUColorWriteFlags = { RED: 1, GREEN: 2, BLUE: 4, ALPHA: 8 };

    // Spec-default limits (WebGPU §GPUSupportedLimits).
    var DEFAULT_LIMITS = {
        maxTextureDimension1D: 8192,
        maxTextureDimension2D: 8192,
        maxTextureDimension3D: 2048,
        maxTextureArrayLayers: 256,
        maxBindGroups: 4,
        maxBindGroupsPlusVertexBuffers: 24,
        maxBindingsPerBindGroup: 1000,
        maxDynamicUniformBuffersPerPipelineLayout: 8,
        maxDynamicStorageBuffersPerPipelineLayout: 4,
        maxSampledTexturesPerShaderStage: 16,
        maxSamplersPerShaderStage: 16,
        maxStorageBuffersPerShaderStage: 10,
        maxStorageTexturesPerShaderStage: 8,
        maxUniformBuffersPerShaderStage: 12,
        maxUniformBufferBindingSize: 65536,
        maxStorageBufferBindingSize: 134217728,
        minUniformBufferOffsetAlignment: 256,
        minStorageBufferOffsetAlignment: 256,
        maxVertexBuffers: 8,
        maxBufferSize: 268435456,
        maxVertexAttributes: 16,
        maxVertexBufferArrayStride: 2048,
        maxInterStageShaderVariables: 16,
        maxColorAttachments: 8,
        maxColorAttachmentBytesPerSample: 32,
        maxComputeWorkgroupSizeX: 64,
        maxComputeWorkgroupSizeY: 64,
        maxComputeWorkgroupSizeZ: 64,
        maxComputeInvocationsPerWorkgroup: 64,
        maxComputePerDimensionDispatchSize: 65535,
    };

    function makeSet(items) {
        var s = new Set(items || []);
        return s;
    }

    // ── canvas contexts (one webgpu canvas per renderer, corpus-wide) ──
    // Device commands route to the canvas that configured the device; the
    // g-device-api constructor creates fallback textures BEFORE configure,
    // so pre-configure commands fall back to the single registered canvas.
    var registeredCanvasCtx = {}; // canvas id → GPUCanvasContext stub
    var registeredCount = 0;

    function resolveDeviceTarget(device) {
        var ids = device.__ocTargets;
        if (ids.length === 1) return ids[0];
        if (ids.length > 1) {
            throw new Error('webgpu recording: device configured by ' + ids.length + ' canvases — one device per canvas in the corpus face');
        }
        if (registeredCount === 1) {
            for (var k in registeredCanvasCtx) return k;
        }
        throw new Error('webgpu recording: no canvas for device command (no configure yet)');
    }

    function makeIndexFormat(v) {
        if (v === 'uint16') return 0;
        if (v === 'uint32') return 1;
        throw new Error('webgpu recording: unsupported indexFormat ' + v);
    }

    // ── recording device ───────────────────────────────────────────
    // One per adapter.requestDevice(). `__ocTargets` is filled by canvas
    // configure; `__ocMapped` holds ArrayBuffers for mappedAtCreation.
    function makeRecordingDevice() {
        var device = makeStub(GPUDevice, 'device', 0);
        var handles = 0;
        var mapped = {}; // handle → ArrayBuffer (mappedAtCreation window)

        function alloc(kind) {
            var id = __gl_alloc_handle();
            handles++;
            return id;
        }

        function target() {
            return resolveDeviceTarget(device);
        }

        function emitTo(api, opcode, enc) {
            emit(target(), opcode, enc.done(opcode));
            void api;
        }

        function destroyStub(kind) {
            return function () {
                var id = this.__gpuId;
                if (typeof id === 'number' && id !== 0xffffffff) {
                    emitTo(kind, W.RESOURCE_DESTROY, new Encoder(4).u32(id));
                }
            };
        }

        device.__ocTargets = [];
        device.__ocMapped = mapped;

        // spec surface g-device-api reads
        device.limits = DEFAULT_LIMITS;
        device.features = makeSet([]);
        device.lost = new Promise(function () {}); // never resolves (no loss in recording)
        device.onuncapturederror = null;
        device.ongpucontextlost = null;
        device.ongpucontextrestored = null;
        device.pushErrorScope = function () {};
        device.popErrorScope = function () {
            return Promise.resolve(null);
        };
        device.destroy = function () {};
        device.isLost = false;
        device.adapterInfo = { vendor: '', architecture: '', device: '', description: '' };
        device.internalError = null;

        device.queue = {
            __gpuKind: 'queue',
            __gpuId: 0,
            writeBuffer: function (buffer, bufferOffset, data, dataOffset, size) {
                var tid = target();
                var bytes = typedBytes(data);
                var lo = 0;
                var len = 0;
                if (bytes) {
                    lo = (dataOffset || 0) | 0;
                    len = size !== undefined ? (size | 0) : (bytes.byteLength - lo) | 0;
                } else if (data instanceof ArrayBuffer) {
                    lo = (dataOffset || 0) | 0;
                    len = size !== undefined ? (size | 0) : (data.byteLength - lo) | 0;
                    bytes = new Uint8Array(data, lo, len);
                    lo = 0;
                } else {
                    throw new Error('queue.writeBuffer: unsupported data ' + (data && data.constructor ? data.constructor.name : typeof data));
                }
                if (len < 0) len = 0;
                var view = new Uint8Array(bytes.buffer, bytes.byteOffset + lo, len);
                var range = intern(tid, view);
                emit(tid, W.QUEUE_WRITE_BUFFER, new Encoder(16).u32(stubId(buffer, 'writeBuffer')).u64(bufferOffset || 0).u32(range).done(W.QUEUE_WRITE_BUFFER));
            },
            writeTexture: function (destination, data, dataLayout, size) {
                var tid = target();
                var bytes = typedBytes(data);
                if (!bytes) {
                    throw new Error('queue.writeTexture: unsupported data ' + (data && data.constructor ? data.constructor.name : typeof data));
                }
                var dataRange = intern(tid, bytes);
                var destRange = internDesc(tid, destination);
                var layoutRange = internDesc(tid, dataLayout || {});
                var sizeRange = internDesc(tid, size);
                emit(tid, W.QUEUE_WRITE_TEXTURE, new Encoder(16).u32(destRange).u32(dataRange).u32(sizeRange).u32(layoutRange).done(W.QUEUE_WRITE_TEXTURE));
            },
            copyExternalImageToTexture: function () {
                throw new Error('webgpu recording: queue.copyExternalImageToTexture unsupported — ImageBitmap sources have no pixels in the recording realm');
            },
            submit: function (commandBuffers) {
                var tid = target();
                var n = commandBuffers.length;
                var enc = new Encoder(4 + 4 * n).u32(n);
                for (var i = 0; i < n; i++) enc.u32(stubId(commandBuffers[i], 'queue.submit'));
                emit(tid, W.QUEUE_SUBMIT, enc.done(W.QUEUE_SUBMIT));
            },
            onSubmittedWorkDone: function () {
                return Promise.resolve(undefined);
            },
            get label() { return 'queue'; },
            set label(v) { void v; },
        };

        device.createBuffer = function (descriptor) {
            var tid = target();
            var id = alloc('buffer');
            // mappedAtCreation is recording-internal: the executor sees a
            // plain buffer + an explicit QUEUE_WRITE_BUFFER at unmap().
            emit(tid, W.BUFFER_CREATE, new Encoder(8).u32(id).u32(internDesc(tid, {
                size: descriptor.size,
                usage: descriptor.usage,
                mappedAtCreation: false,
            })).done(W.BUFFER_CREATE));
            var stub = makeStub(GPUBuffer, 'buffer', id);
            stub.size = descriptor.size;
            stub.usage = descriptor.usage;
            stub.mapState = 'unmapped';
            if (descriptor.mappedAtCreation) {
                mapped[id] = new ArrayBuffer(descriptor.size);
                stub.mapState = 'mapped';
            }
            stub.getMappedRange = function () {
                var buf = mapped[id];
                if (!buf) throw new Error('GPUBuffer.getMappedRange: not mapped');
                return buf;
            };
            stub.mapAsync = function (mode) {
                // Red line: no pixel data flows back into core. Readback
                // buffers resolve zeros; the PNG diff is the pixel oracle.
                if (typeof console !== 'undefined' && console.error) {
                    console.error('[webgpu recording] mapAsync resolves zeros (handle ' + id + ')');
                }
                return Promise.resolve().then(function () {
                    stub.mapState = 'mapped';
                    if (!mapped[id]) mapped[id] = new ArrayBuffer(stub.size || 0);
                    return stub;
                });
            };
            stub.unmap = function () {
                var buf = mapped[id];
                if (buf) {
                    var tid2 = target();
                    var range = intern(tid2, new Uint8Array(buf));
                    emit(tid2, W.QUEUE_WRITE_BUFFER, new Encoder(16).u32(id).u64(0).u32(range).done(W.QUEUE_WRITE_BUFFER));
                    delete mapped[id];
                }
                stub.mapState = 'unmapped';
            };
            stub.destroy = destroyStub('buffer.destroy');
            stub.onunmap = null;
            return stub;
        };

        device.createTexture = function (descriptor) {
            var tid = target();
            var id = alloc('texture');
            emit(tid, W.TEXTURE_CREATE, new Encoder(8).u32(id).u32(internDesc(tid, descriptor)).done(W.TEXTURE_CREATE));
            var stub = makeStub(GPUTexture, 'texture', id);
            stub.width = descriptor.size ? descriptor.size.width : descriptor.width;
            stub.height = descriptor.size ? descriptor.size.height : descriptor.height;
            stub.depthOrArrayLayers = descriptor.size ? (descriptor.size.depthOrArrayLayers || 1) : 1;
            stub.mipLevelCount = descriptor.mipLevelCount || 1;
            stub.sampleCount = descriptor.sampleCount || 1;
            stub.dimension = descriptor.dimension || '2d';
            stub.format = descriptor.format;
            stub.usage = descriptor.usage;
            stub.createView = function (viewDescriptor) {
                var vid = alloc('textureView');
                emit(target(), W.TEXTURE_VIEW_CREATE, new Encoder(12).u32(vid).u32(id).u32(internDesc(target(), viewDescriptor || {})).done(W.TEXTURE_VIEW_CREATE));
                var vs = makeStub(GPUTextureView, 'textureView', vid);
                vs.destroy = function () {};
                vs.__ocTexture = id;
                return vs;
            };
            stub.destroy = destroyStub('texture.destroy');
            return stub;
        };

        device.createSampler = function (descriptor) {
            var tid = target();
            var id = alloc('sampler');
            emit(tid, W.SAMPLER_CREATE, new Encoder(8).u32(id).u32(internDesc(tid, descriptor || {})).done(W.SAMPLER_CREATE));
            var stub = makeStub(GPUSampler, 'sampler', id);
            stub.destroy = function () {};
            return stub;
        };

        device.importExternalTexture = function () {
            throw new Error('webgpu recording: importExternalTexture unsupported');
        };
        device.createQuerySet = function (descriptor) {
            var tid = target();
            var id = alloc('querySet');
            emit(tid, W.QUERY_SET_CREATE, new Encoder(8).u32(id).u32(internDesc(tid, descriptor)).done(W.QUERY_SET_CREATE));
            var stub = makeStub(GPUQuerySet, 'querySet', id);
            stub.destroy = destroyStub('querySet.destroy');
            return stub;
        };

        device.createShaderModule = function (descriptor) {
            var tid = target();
            var id = alloc('shaderModule');
            var code = descriptor.code;
            if (typeof code !== 'string') {
                throw new Error('createShaderModule: non-string code ' + (code && code.constructor ? code.constructor.name : typeof code));
            }
            var range = intern(tid, utf8(code));
            emit(tid, W.SHADER_MODULE_CREATE, new Encoder(8).u32(id).u32(range).done(W.SHADER_MODULE_CREATE));
            var stub = makeStub(GPUShaderModule, 'shaderModule', id);
            stub.getCompilationInfo = function () {
                return Promise.resolve({ messages: [] });
            };
            stub.compilationInfo = stub.getCompilationInfo;
            stub.destroy = function () {};
            return stub;
        };

        device.createBindGroupLayout = function (descriptor) {
            var tid = target();
            var id = alloc('bindGroupLayout');
            emit(tid, W.BIND_GROUP_LAYOUT_CREATE, new Encoder(8).u32(id).u32(internDesc(tid, descriptor)).done(W.BIND_GROUP_LAYOUT_CREATE));
            return makeStub(GPUBindGroupLayout, 'bindGroupLayout', id);
        };

        device.createPipelineLayout = function (descriptor) {
            var tid = target();
            var id = alloc('pipelineLayout');
            emit(tid, W.PIPELINE_LAYOUT_CREATE, new Encoder(8).u32(id).u32(internDesc(tid, descriptor)).done(W.PIPELINE_LAYOUT_CREATE));
            return makeStub(GPUPipelineLayout, 'pipelineLayout', id);
        };

        function makePipeline(ctor, kind, opcode, descriptor) {
            var tid = target();
            var id = alloc(kind);
            emit(tid, opcode, new Encoder(8).u32(id).u32(internDesc(tid, descriptor)).done(opcode));
            var stub = makeStub(ctor, kind, id);
            // Derived layout (g-device-api createBindings): layout comes from
            // the pipeline (shader auto-derive), wrapped as {"ph","pi"}.
            //
            // 同一 (pipeline, index) 必须返回同一对象——Dawn 就是这么做的
            // （BGL 建 pipeline 时缓存，getBindGroupLayout 只取缓存）。这层
            // 同一性直接决定 G2 的行为：g-device-api 按 BGL 对象身份缓存
            // bind group，每次返回新对象会让缓存永不命中，于是它每帧重建
            // bind group（派生源=当时 pipeline，恰好合法），Dawn 下则命中
            // 旧 bind group 并跨 pipeline 复用（非法）。不能忠实复刻这点，
            // 引擎就会比 Dawn 多画出一帧内容，失败不对称。
            var derivedLayouts = [];
            stub.getBindGroupLayout = function (index) {
                var i = index | 0;
                return derivedLayouts[i] || (derivedLayouts[i] = {
                    __gpuKind: 'derivedLayout',
                    __ocPipeline: id,
                    __ocLayoutIndex: i,
                });
            };
            return stub;
        }

        device.createRenderPipeline = function (descriptor) {
            return makePipeline(GPURenderPipeline, 'renderPipeline', W.RENDER_PIPELINE_CREATE, descriptor);
        };
        device.createComputePipeline = function (descriptor) {
            return makePipeline(GPUComputePipeline, 'computePipeline', W.COMPUTE_PIPELINE_CREATE, descriptor);
        };
        device.createRenderPipelineAsync = function (descriptor) {
            return Promise.resolve(device.createRenderPipeline(descriptor));
        };
        device.createComputePipelineAsync = function (descriptor) {
            return Promise.resolve(device.createComputePipeline(descriptor));
        };

        device.createBindGroup = function (descriptor) {
            var tid = target();
            var id = alloc('bindGroup');
            emit(tid, W.BIND_GROUP_CREATE, new Encoder(8).u32(id).u32(internDesc(tid, descriptor)).done(W.BIND_GROUP_CREATE));
            return makeStub(GPUBindGroup, 'bindGroup', id);
        };

        device.createCommandEncoder = function (descriptor) {
            void descriptor;
            var id = alloc('commandEncoder');
            var enc = makeStub(GPUCommandEncoder, 'commandEncoder', id);

            enc.beginRenderPass = function (desc) {
                var tid2 = target();
                var pid = alloc('renderPass');
                if (desc.timestampWrites) throw new Error('render pass timestampWrites unsupported');
                emit(tid2, W.ENC_BEGIN_RENDER_PASS, new Encoder(12).u32(id).u32(pid).u32(internDesc(tid2, desc)).done(W.ENC_BEGIN_RENDER_PASS));
                return makeRenderPassEncoder(pid);
            };
            enc.beginComputePass = function (desc) {
                var tid2 = target();
                var pid = alloc('computePass');
                if (desc && desc.timestampWrites) throw new Error('compute pass timestampWrites unsupported');
                emit(tid2, W.ENC_BEGIN_COMPUTE_PASS, new Encoder(12).u32(id).u32(pid).u32(internDesc(tid2, desc || {})).done(W.ENC_BEGIN_COMPUTE_PASS));
                return makeComputePassEncoder(pid);
            };
            enc.copyBufferToBuffer = function (src, srcOffset, dst, dstOffset, size) {
                emitTo('copyBufferToBuffer', W.ENC_COPY_BUFFER_TO_BUFFER, new Encoder(36)
                    .u32(id).u32(stubId(src, 'copyBufferToBuffer')).u64(srcOffset || 0)
                    .u32(stubId(dst, 'copyBufferToBuffer')).u64(dstOffset || 0).u64(size));
            };
            enc.copyTextureToTexture = function (src, dst, size) {
                var tid2 = target();
                var r = internString(tid2, JSON.stringify({ source: wrapHandles(src), destination: wrapHandles(dst), size: size }));
                emit(tid2, W.ENC_COPY_TEXTURE_TO_TEXTURE, new Encoder(8).u32(id).u32(r).done(W.ENC_COPY_TEXTURE_TO_TEXTURE));
            };
            enc.copyTextureToBuffer = function (src, dst, size) {
                var tid2 = target();
                var r = internString(tid2, JSON.stringify({ source: wrapHandles(src), destination: wrapHandles(dst), size: size }));
                emit(tid2, W.ENC_COPY_TEXTURE_TO_BUFFER, new Encoder(8).u32(id).u32(r).done(W.ENC_COPY_TEXTURE_TO_BUFFER));
            };
            enc.copyBufferToTexture = function (src, dst, size) {
                var tid2 = target();
                var r = internString(tid2, JSON.stringify({ source: wrapHandles(src), destination: wrapHandles(dst), size: size }));
                emit(tid2, W.ENC_COPY_BUFFER_TO_TEXTURE, new Encoder(8).u32(id).u32(r).done(W.ENC_COPY_BUFFER_TO_TEXTURE));
            };
            enc.finish = function (descriptor) {
                void descriptor;
                var tid2 = target();
                var bid = alloc('commandBuffer');
                emit(tid2, W.ENC_FINISH, new Encoder(8).u32(id).u32(bid).done(W.ENC_FINISH));
                var buf = makeStub(GPUCommandBuffer, 'commandBuffer', bid);
                buf.destroy = function () {};
                return buf;
            };
            enc.pushDebugGroup = function () {};
            enc.popDebugGroup = function () {};
            enc.insertDebugMarker = function () {};
            enc.writeTimestamp = function () {
                throw new Error('webgpu recording: writeTimestamp unsupported');
            };
            enc.resolveQuerySet = function () {
                throw new Error('webgpu recording: resolveQuerySet unsupported');
            };
            return enc;
        };

        function makeComputePassEncoder(pid) {
            var pass = makeStub(GPUComputePassEncoder, 'computePass', pid);
            var t = function () { return target(); };
            // setPipeline/setBindGroup 与 render pass 共 opcode（pass 句柄
            // 全局唯一，执行端按段类别分派到 ComputePipeline/BindGroup）。
            pass.setPipeline = function (pipeline) {
                emit(t(), W.PASS_SET_PIPELINE, new Encoder(8).u32(pid).u32(stubId(pipeline, 'setPipeline')).done(W.PASS_SET_PIPELINE));
            };
            pass.setBindGroup = function (index, bindGroup, dynamicOffsets) {
                if (dynamicOffsets && dynamicOffsets.length) {
                    throw new Error('webgpu recording: dynamicOffsets unsupported');
                }
                emit(t(), W.PASS_SET_BIND_GROUP, new Encoder(12).u32(pid).u32(index | 0).u32(stubId(bindGroup, 'setBindGroup')).done(W.PASS_SET_BIND_GROUP));
            };
            pass.dispatchWorkgroups = function (x, y, z) {
                emit(t(), W.PASS_DISPATCH_WORKGROUPS, new Encoder(16).u32(pid)
                    .u32(x | 0).u32(y === undefined ? 1 : y | 0).u32(z === undefined ? 1 : z | 0).done(W.PASS_DISPATCH_WORKGROUPS));
            };
            pass.dispatchWorkgroupsIndirect = function (buffer, offset) {
                emit(t(), W.PASS_DISPATCH_WORKGROUPS_INDIRECT, new Encoder(16).u32(pid).u32(stubId(buffer, 'dispatchWorkgroupsIndirect')).u64(offset || 0).done(W.PASS_DISPATCH_WORKGROUPS_INDIRECT));
            };
            pass.end = function () {
                emit(t(), W.PASS_END, new Encoder(4).u32(pid).done(W.PASS_END));
            };
            pass.pushDebugGroup = function () {};
            pass.popDebugGroup = function () {};
            pass.insertDebugMarker = function () {};
            return pass;
        }

        function makeRenderPassEncoder(pid) {
            var pass = makeStub(GPURenderPassEncoder, 'renderPass', pid);
            var t = function () { return target(); };
            var p4 = new Encoder(8); // reusable? no — fresh encoders below
            void p4;

            pass.setPipeline = function (pipeline) {
                emit(t(), W.PASS_SET_PIPELINE, new Encoder(8).u32(pid).u32(stubId(pipeline, 'setPipeline')).done(W.PASS_SET_PIPELINE));
            };
            pass.setBindGroup = function (index, bindGroup, dynamicOffsets) {
                if (dynamicOffsets && dynamicOffsets.length) {
                    throw new Error('webgpu recording: dynamicOffsets unsupported');
                }
                emit(t(), W.PASS_SET_BIND_GROUP, new Encoder(12).u32(pid).u32(index | 0).u32(stubId(bindGroup, 'setBindGroup')).done(W.PASS_SET_BIND_GROUP));
            };
            pass.setVertexBuffer = function (slot, buffer, offset) {
                emit(t(), W.PASS_SET_VERTEX_BUFFER, new Encoder(20).u32(pid).u32(slot | 0).u32(stubId(buffer, 'setVertexBuffer')).u64(offset || 0).done(W.PASS_SET_VERTEX_BUFFER));
            };
            pass.setIndexBuffer = function (buffer, indexFormat, offset) {
                emit(t(), W.PASS_SET_INDEX_BUFFER, new Encoder(17).u32(pid).u32(stubId(buffer, 'setIndexBuffer')).u8(makeIndexFormat(indexFormat)).u64(offset || 0).done(W.PASS_SET_INDEX_BUFFER));
            };
            pass.setViewport = function (x, y, w, h, minDepth, maxDepth) {
                emit(t(), W.PASS_SET_VIEWPORT, new Encoder(28).u32(pid)
                    .f32(x).f32(y).f32(w).f32(h).f32(minDepth).f32(maxDepth).done(W.PASS_SET_VIEWPORT));
            };
            pass.setScissorRect = function (x, y, w, h) {
                emit(t(), W.PASS_SET_SCISSOR_RECT, new Encoder(20).u32(pid).u32(x | 0).u32(y | 0).u32(w | 0).u32(h | 0).done(W.PASS_SET_SCISSOR_RECT));
            };
            pass.setBlendConstant = function (color) {
                var c = Array.isArray(color) ? color : [color.r, color.g, color.b, color.a];
                emit(t(), W.PASS_SET_BLEND_CONSTANT, new Encoder(20).u32(pid)
                    .f32(Number(c[0]) || 0).f32(Number(c[1]) || 0).f32(Number(c[2]) || 0).f32(Number(c[3]) || 0).done(W.PASS_SET_BLEND_CONSTANT));
            };
            pass.setStencilReference = function (reference) {
                emit(t(), W.PASS_SET_STENCIL_REFERENCE, new Encoder(8).u32(pid).u32(reference >>> 0).done(W.PASS_SET_STENCIL_REFERENCE));
            };
            pass.draw = function (vertexCount, instanceCount, firstVertex, firstInstance) {
                emit(t(), W.PASS_DRAW, new Encoder(20).u32(pid)
                    .u32(vertexCount | 0).u32(instanceCount === undefined ? 1 : instanceCount | 0)
                    .u32(firstVertex || 0).u32(firstInstance || 0).done(W.PASS_DRAW));
            };
            pass.drawIndexed = function (indexCount, instanceCount, firstIndex, baseVertex, firstInstance) {
                emit(t(), W.PASS_DRAW_INDEXED, new Encoder(24).u32(pid)
                    .u32(indexCount | 0).u32(instanceCount === undefined ? 1 : instanceCount | 0)
                    .u32(firstIndex || 0).i32(baseVertex || 0).u32(firstInstance || 0).done(W.PASS_DRAW_INDEXED));
            };
            pass.drawIndirect = function (indirectBuffer, indirectOffset) {
                emit(t(), W.PASS_DRAW_INDIRECT, new Encoder(16).u32(pid).u32(stubId(indirectBuffer, 'drawIndirect')).u64(indirectOffset || 0).done(W.PASS_DRAW_INDIRECT));
            };
            pass.drawIndexedIndirect = function (indirectBuffer, indirectOffset) {
                emit(t(), W.PASS_DRAW_INDEXED_INDIRECT, new Encoder(16).u32(pid).u32(stubId(indirectBuffer, 'drawIndexedIndirect')).u64(indirectOffset || 0).done(W.PASS_DRAW_INDEXED_INDIRECT));
            };
            pass.end = function () {
                emit(t(), W.PASS_END, new Encoder(4).u32(pid).done(W.PASS_END));
            };
            pass.executeBundles = function () {
                throw new Error('webgpu recording: executeBundles unsupported');
            };
            pass.setVertexBuffer_done = null;
            pass.beginOcclusionQuery = function () {
                throw new Error('webgpu recording: occlusion queries unsupported');
            };
            pass.endOcclusionQuery = pass.beginOcclusionQuery;
            pass.pushDebugGroup = function () {};
            pass.popDebugGroup = function () {};
            pass.insertDebugMarker = function () {};
            return pass;
        }

        device.__ocAlloc = alloc;
        return device;
    }

    // ── canvas context ─────────────────────────────────────────────

    function makeCanvasContext(canvasId, width, height, originX, originY) {
        void originX; void originY; // 合成位置已进占位 Image op 流头（gl_stream_begin 同源）
        var self = new GPUCanvasContext();
        var device = null;
        var configured = false;
        // Stable swapchain identity: same texture & view handles every frame
        // (executor keeps the mapping; view re-created per frame because
        // reset_for_frame clears the stream but not JS objects).
        var swapTexture = null;
        var swapView = null;

        function ensureOpen() {
            // 帧流重开：reset_for_frame 清流后，getCurrentTexture/configure
            // 命中的第一笔命令把流按缓存头重开（store 自动重开也兜底）。
            __gl_stream_begin(canvasId, width, height, 1, 0, 0, 0);
        }
        // emit/intern 层的 ensureOpenFor 用（configure 前的 fallback 资源
        // 创建命令也要求流已开，见 ensureOpenFor 注释）。
        self.__ocEnsureOpen = ensureOpen;

        self.canvas = { id: canvasId };
        self.configure = function (config) {
            if (!config || !config.device) throw new Error('canvas configure: device required');
            device = config.device;
            var fmt = String(config.format || 'bgra8unorm');
            var alphaMode = ALPHA_MODE[config.alphaMode || 'opaque'];
            if (alphaMode === undefined) throw new Error('canvas configure: bad alphaMode ' + config.alphaMode);
            if (device.__ocTargets.indexOf(canvasId) < 0) device.__ocTargets.push(canvasId);
            ensureOpen();
            var range = internString(canvasId, fmt);
            emit(canvasId, W.CANVAS_CONFIGURE, new Encoder(9).u32(range).u8(alphaMode).u32(config.usage >>> 0).done(W.CANVAS_CONFIGURE));
            configured = true;
        };
        self.unconfigure = function () {
            configured = false;
        };
        self.getCurrentTexture = function () {
            if (!configured) throw new Error('getCurrentTexture: canvas not configured');
            ensureOpen();
            if (!swapTexture) {
                var tid = __gl_alloc_handle();
                swapTexture = makeStub(GPUTexture, 'texture', tid);
                swapTexture.width = width;
                swapTexture.height = height;
                swapTexture.__ocSwapchain = true;
                swapTexture.createView = function (viewDescriptor) {
                    if (!swapView) {
                        var vid = __gl_alloc_handle();
                        swapView = makeStub(GPUTextureView, 'textureView', vid);
                        swapView.__ocTexture = tid;
                    }
                    // 每帧重发：帧流自足（view 创建命令随帧重建，句柄稳定，
                    // 执行端按句柄覆盖映射）。
                    emit(canvasId, W.TEXTURE_VIEW_CREATE, new Encoder(12).u32(swapView.__gpuId).u32(tid).u32(internDesc(canvasId, viewDescriptor || {})).done(W.TEXTURE_VIEW_CREATE));
                    return swapView;
                };
                swapTexture.destroy = function () {};
            }
            emit(canvasId, W.CANVAS_GET_CURRENT_TEXTURE, new Encoder(4).u32(swapTexture.__gpuId).done(W.CANVAS_GET_CURRENT_TEXTURE));
            return swapTexture;
        };
        self.getConfiguration = function () {
            return null;
        };
        return self;
    }

    // ── navigator.gpu ──────────────────────────────────────────────
    // g-device-api probes `globalThis.navigator.gpu === undefined` first; a
    // recording adapter resolves immediately (deterministic by construction).

    function installNavigatorGPU() {
        if (typeof navigator === 'undefined') return;
        if (navigator.gpu) return; // existing surface wins (never in this realm)
        var adapter = makeStub(GPUAdapter, 'adapter', 0);
        adapter.features = makeSet([]);
        adapter.limits = DEFAULT_LIMITS;
        adapter.info = { vendor: 'opencat', architecture: 'recording', device: 'recording', description: 'OpenCat WebGPU recorder' };
        adapter.isFallbackAdapter = false;
        adapter.requestAdapterInfo = function () {
            return Promise.resolve(adapter.info);
        };
        adapter.requestDevice = function (descriptor) {
            void descriptor; // requiredFeatures/limits recorded against spec defaults — corpus needs none
            return Promise.resolve(makeRecordingDevice());
        };

        var gpu = {
            __gpuKind: 'gpu',
            requestAdapter: function () {
                return Promise.resolve(adapter);
            },
            getPreferredCanvasFormat: function () {
                // Windows/Dawn reference leg reports bgra8unorm; wgpu surface
                // on the engine side follows the stream's configured format.
                return 'bgra8unorm';
            },
            wgslLanguageFeatures: makeSet(['wgsl']),
        };
        try {
            Object.defineProperty(navigator, 'gpu', { value: gpu, configurable: true });
        } catch (e) {
            navigator.gpu = gpu;
        }
    }

    // ── sizing & getContext routing (mirrors webgl_api.js) ─────────

    function getGPUCanvasContext(nodeId, sizeW, sizeH, originX, originY) {
        var key = String(nodeId);
        var existing = registeredCanvasCtx[key];
        if (existing) return existing;
        // drawingBuffer 尺寸 = canvas 的 width/height 内容属性（HTML 语义，
        // 与布局盒无关）；取不到再落布局盒，最后 1×1 兜底。
        var width = Number(sizeW) > 0 ? Math.floor(Number(sizeW)) : 0;
        var height = Number(sizeH) > 0 ? Math.floor(Number(sizeH)) : 0;
        if (!width || !height) {
            var box = (typeof __read_box === 'function' && __read_box(key)) || {};
            if (!width) width = Math.round(Number(box.width) || 0);
            if (!height) height = Math.round(Number(box.height) || 0);
        }
        var cc = makeCanvasContext(key, Math.max(1, width), Math.max(1, height), originX, originY);
        registeredCanvasCtx[key] = cc;
        registeredCount++;
        return cc;
    }

    var baseGetContext = ctx.getContext;
    ctx.getContext = function (nodeId, type, attrs, sizeW, sizeH, originX, originY) {
        var t = String(type);
        if (t === 'webgpu') {
            return getGPUCanvasContext(nodeId, sizeW, sizeH, originX, originY);
        }
        return baseGetContext(nodeId, type, attrs, sizeW, sizeH, originX, originY);
    };

    installNavigatorGPU();
})();
