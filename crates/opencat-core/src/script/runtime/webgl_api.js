// WebGL2 recording face: `canvas.getContext('webgl2', attrs)` returns a
// deterministic command RECORDER, not a live GL context.
//
// Every GL call appends one `[opcode: u8][payload]` command to the target
// canvas's stream (format authority: ir/gpu_stream.rs gl_opcode — the TS side
// imports the generated table, the engine executor mirrors it). Synchronous
// queries never enter the stream: they are answered from a fixed capability
// table plus a GLSL source scan (ANGLE-style client state machine). The
// executing host (engine → glow / web → real WebGL2) replays the stream
// against a real context and backfills the frame's generated-image slot.
//
// Queries are deterministic by construction:
//   - getParameter/getExtension      → pinned capability table
//   - getShaderParameter(COMPILE/LINK) → true (errors surface at the executor)
//   - getUniformLocation/Attrib      → (program, name) handles from a source
//     scan; "declared ⇒ present" (over-reporting is harmless: the executor
//     matches by name and ignores unknowns; under-reporting would break)
//   - getError                        → recorded errors (NO_ERROR in practice)
//
// Pixel data never flows back into core (readPixels is not even recorded —
// the red line against reading back composited surfaces holds).

(function () {
    'use strict';

    var ctx = globalThis.ctx;

    // ── command encoding ────────────────────────────────────────────
    // One scratch buffer per command; the binding copies it into the stream.

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
    Encoder.prototype.u16 = function (v) {
        this.dv.setUint16(this.pos, v, true);
        this.pos += 2;
        return this;
    };
    Encoder.prototype.i32 = function (v) {
        this.dv.setInt32(this.pos, v, true);
        this.pos += 4;
        return this;
    };
    Encoder.prototype.u32 = function (v) {
        this.dv.setUint32(this.pos, v >>> 0, true);
        this.pos += 4;
        return this;
    };
    Encoder.prototype.f32 = function (v) {
        this.dv.setFloat32(this.pos, v, true);
        this.pos += 4;
        return this;
    };
    Encoder.prototype.done = function (opcode) {
        this.bytes[0] = opcode;
        return this.bytes.subarray(0, this.pos);
    };

    // Bindings marshal through JSON.stringify: a TypedArray would become an
    // object map, so commands travel as plain arrays (u8s).
    function emit(targetId, opcode, payload) {
        payload[0] = opcode;
        __gl_stream_emit(targetId, Array.prototype.slice.call(payload));
    }

    function intern(targetId, bytes) {
        return __gl_stream_intern(targetId, Array.prototype.slice.call(bytes));
    }

    function utf8(str) {
        // Manual UTF-8 encode: the realm has no TextEncoder. Shader sources
        // and names are small.
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
        return out;
    }

    function internString(targetId, str) {
        return intern(targetId, utf8(str));
    }

    // typed arrays pass through as raw bytes (bufferData / texImage2D).
    function typedBytes(data) {
        if (data && data.buffer instanceof ArrayBuffer) {
            return new Uint8Array(data.buffer, data.byteOffset || 0, data.byteLength);
        }
        if (data instanceof ArrayBuffer) return new Uint8Array(data);
        return null;
    }

    // ── GL constants (WebGL2 GLenum; the working set) ───────────────

    var GL = {
        VERTEX_SHADER: 0x8b31,
        FRAGMENT_SHADER: 0x8b30,
        COMPILE_STATUS: 0x8b81,
        LINK_STATUS: 0x8b82,
        ACTIVE_UNIFORMS: 0x8b86,
        ACTIVE_ATTRIBUTES: 0x8b89,
        ACTIVE_UNIFORM_BLOCKS: 0x8a36,
        SHADER_TYPE: 0x8b4f,
        DELETE_STATUS: 0x8b80,
        CURRENT_PROGRAM: 0x8b8d,
        ARRAY_BUFFER: 0x8892,
        ELEMENT_ARRAY_BUFFER: 0x8893,
        STATIC_DRAW: 0x88e4,
        DYNAMIC_DRAW: 0x88e8,
        STREAM_DRAW: 0x88e0,
        TEXTURE_2D: 0x0de1,
        TEXTURE_CUBE_MAP: 0x8513,
        RGBA8: 0x8058,
        RGBA: 0x1908,
        RGB: 0x1907,
        RGBA16F: 0x881a,
        RGB16F: 0x881b,
        RGBA32F: 0x8814,
        DEPTH_COMPONENT16: 0x81a5,
        DEPTH_COMPONENT24: 0x81a6,
        DEPTH24_STENCIL8: 0x88f0,
        UNSIGNED_BYTE: 0x1401,
        UNSIGNED_SHORT: 0x1403,
        UNSIGNED_INT: 0x1405,
        FLOAT: 0x1406,
        HALF_FLOAT: 0x140b,
        TEXTURE_MAG_FILTER: 0x2800,
        TEXTURE_MIN_FILTER: 0x2801,
        TEXTURE_WRAP_S: 0x2802,
        TEXTURE_WRAP_T: 0x2803,
        NEAREST: 0x2600,
        LINEAR: 0x2601,
        NEAREST_MIPMAP_NEAREST: 0x2700,
        LINEAR_MIPMAP_NEAREST: 0x2701,
        NEAREST_MIPMAP_LINEAR: 0x2702,
        LINEAR_MIPMAP_LINEAR: 0x2703,
        CLAMP_TO_EDGE: 0x812f,
        MIRRORED_REPEAT: 0x8370,
        REPEAT: 0x2901,
        UNPACK_FLIP_Y_WEBGL: 0x9240,
        UNPACK_PREMULTIPLY_ALPHA_WEBGL: 0x9241,
        UNPACK_COLORSPACE_CONVERSION_WEBGL: 0x9243,
        UNPACK_ALIGNMENT: 0x0cf5,
        PACK_ALIGNMENT: 0x0d05,
        FRAMEBUFFER: 0x8d40,
        COLOR_ATTACHMENT0: 0x8ce0,
        COLOR_ATTACHMENT1: 0x8ce1,
        COLOR_ATTACHMENT2: 0x8ce2,
        COLOR_ATTACHMENT3: 0x8ce3,
        DEPTH_ATTACHMENT: 0x8d00,
        STENCIL_ATTACHMENT: 0x8d20,
        DEPTH_STENCIL_ATTACHMENT: 0x821a,
        FRAMEBUFFER_COMPLETE: 0x8cd5,
        COLOR_BUFFER_BIT: 0x4000,
        DEPTH_BUFFER_BIT: 0x0100,
        STENCIL_BUFFER_BIT: 0x0400,
        BLEND: 0x0be2,
        CULL_FACE: 0x0b44,
        DEPTH_TEST: 0x0b71,
        DITHER: 0x0bd0,
        POLYGON_OFFSET_FILL: 0x8037,
        SAMPLE_ALPHA_TO_COVERAGE: 0x809e,
        SCISSOR_TEST: 0x0c11,
        STENCIL_TEST: 0x0b90,
        ZERO: 0,
        ONE: 1,
        SRC_COLOR: 0x0300,
        ONE_MINUS_SRC_COLOR: 0x0301,
        SRC_ALPHA: 0x0302,
        ONE_MINUS_SRC_ALPHA: 0x0303,
        DST_ALPHA: 0x0304,
        ONE_MINUS_DST_ALPHA: 0x0305,
        DST_COLOR: 0x0306,
        ONE_MINUS_DST_COLOR: 0x0307,
        CONSTANT_COLOR: 0x8001,
        ONE_MINUS_CONSTANT_COLOR: 0x8002,
        CONSTANT_ALPHA: 0x8003,
        ONE_MINUS_CONSTANT_ALPHA: 0x8004,
        FUNC_ADD: 0x8006,
        FUNC_SUBTRACT: 0x800a,
        FUNC_REVERSE_SUBTRACT: 0x800b,
        NEVER: 0x0200,
        ALWAYS: 0x0207,
        LESS: 0x0201,
        LEQUAL: 0x0203,
        GREATER: 0x0204,
        GEQUAL: 0x0202,
        EQUAL: 0x0202,
        NOTEQUAL: 0x0205,
        CW: 0x0900,
        CCW: 0x0901,
        FRONT: 0x0404,
        BACK: 0x0405,
        FRONT_AND_BACK: 0x0408,
        POINTS: 0x0000,
        LINES: 0x0001,
        LINE_LOOP: 0x0002,
        LINE_STRIP: 0x0003,
        TRIANGLES: 0x0004,
        TRIANGLE_STRIP: 0x0005,
        TRIANGLE_FAN: 0x0006,
        NO_ERROR: 0,
        INVALID_ENUM: 0x0500,
        INVALID_VALUE: 0x0501,
        INVALID_OPERATION: 0x0502,
        OUT_OF_MEMORY: 0x0505,
        FLOAT_VEC2: 0x8b50,
        FLOAT_VEC3: 0x8b51,
        FLOAT_VEC4: 0x8b52,
        INT_VEC2: 0x8b53,
        INT_VEC3: 0x8b54,
        INT_VEC4: 0x8b55,
        BOOL: 0x8b56,
        FLOAT_MAT2: 0x8b5a,
        FLOAT_MAT3: 0x8b5b,
        FLOAT_MAT4: 0x8b5c,
        SAMPLER_2D: 0x8b5e,
        SAMPLER_CUBE: 0x8b60,
        INT: 0x1404,
        SAMPLER_2D_SHADOW: 0x8b62,
        SAMPLER_2D_ARRAY: 0x8dc1,
        LOW_FLOAT: 0x8df0,
        MEDIUM_FLOAT: 0x8df1,
        HIGH_FLOAT: 0x8df2,
        LOW_INT: 0x8df3,
        MEDIUM_INT: 0x8df4,
        HIGH_INT: 0x8df5,
        MAX_TEXTURE_MAX_ANISOTROPY_EXT: 0x84ff,
        TEXTURE_MAX_ANISOTROPY_EXT: 0x84fe,
    };

    // ── capability table (pinned; both executors must honor these) ──

    var CAPS = {
        VERSION: 'WebGL 2.0 (OpenGL ES 3.0 OpenCat)',
        SHADING_LANGUAGE_VERSION: 'WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.00 OpenCat)',
        VENDOR: 'OpenCat',
        RENDERER: 'OpenCat WebGL Recorder',
        UNMASKED_VENDOR_WEBGL: 'OpenCat',
        UNMASKED_RENDERER_WEBGL: 'OpenCat WebGL (deterministic)',

        MAX_TEXTURE_SIZE: 16384,
        MAX_CUBE_MAP_TEXTURE_SIZE: 16384,
        MAX_3D_TEXTURE_SIZE: 2048,
        MAX_ARRAY_TEXTURE_LAYERS: 2048,
        MAX_RENDERBUFFER_SIZE: 16384,
        MAX_VIEWPORT_DIMS: [32767, 32767],
        MAX_VERTEX_ATTRIBS: 16,
        MAX_VERTEX_UNIFORM_VECTORS: 4096,
        MAX_VARYING_VECTORS: 30,
        MAX_FRAGMENT_UNIFORM_VECTORS: 1024,
        MAX_VERTEX_TEXTURE_IMAGE_UNITS: 16,
        MAX_TEXTURE_IMAGE_UNITS: 16,
        MAX_COMBINED_TEXTURE_IMAGE_UNITS: 32,
        MAX_ELEMENTS_INDICES: 16777216,
        MAX_ELEMENTS_VERTICES: 16777216,
        MAX_SAMPLES: 4,
        MAX_COLOR_ATTACHMENTS: 4,
        MAX_DRAW_BUFFERS: 4,
        MAX_VERTEX_UNIFORM_COMPONENTS: 16384,
        MAX_FRAGMENT_UNIFORM_COMPONENTS: 4096,
        MAX_VERTEX_OUTPUT_COMPONENTS: 64,
        MAX_FRAGMENT_INPUT_COMPONENTS: 60,
        MAX_COMBINED_VERTEX_UNIFORM_COMPONENTS: 65536,
        MAX_COMBINED_FRAGMENT_UNIFORM_COMPONENTS: 65536,
        MAX_UNIFORM_BLOCK_SIZE: 65536,
        MAX_VERTEX_UNIFORM_BLOCKS: 16,
        MAX_FRAGMENT_UNIFORM_BLOCKS: 16,
        MAX_COMBINED_UNIFORM_BLOCKS: 32,
        MAX_UNIFORM_BUFFER_BINDINGS: 32,
        UNIFORM_BUFFER_OFFSET_ALIGNMENT: 256,
        ALIASED_LINE_WIDTH_RANGE: [1, 1],
        ALIASED_POINT_SIZE_RANGE: [1, 1024],
        SAMPLES: 0,
        SAMPLE_BUFFERS: 0,
        RED_BITS: 8,
        GREEN_BITS: 8,
        BLUE_BITS: 8,
        ALPHA_BITS: 8,
        DEPTH_BITS: 24,
        STENCIL_BITS: 8,
        COMPRESSED_TEXTURE_FORMATS: [],
    };

    var SUPPORTED_EXTENSIONS = [
        'EXT_color_buffer_float',
        'EXT_color_buffer_half_float',
        'EXT_texture_filter_anisotropic',
        'OES_texture_float_linear',
        'WEBGL_debug_renderer_info',
    ];

    function makeExtension(name) {
        if (name === 'EXT_texture_filter_anisotropic') {
            return {
                MAX_TEXTURE_MAX_ANISOTROPY_EXT: GL.MAX_TEXTURE_MAX_ANISOTROPY_EXT,
                TEXTURE_MAX_ANISOTROPY_EXT: GL.TEXTURE_MAX_ANISOTROPY_EXT,
            };
        }
        if (name === 'WEBGL_debug_renderer_info') {
            return { UNMASKED_VENDOR_WEBGL: 0x9245, UNMASKED_RENDERER_WEBGL: 0x9246 };
        }
        // Presence is the whole contract (the capability table already
        // assumes float render targets etc.).
        return {};
    }

    // ── GLSL source scan (declared uniforms / attributes) ───────────
    // "Declared ⇒ present": over-reporting is safe (the executor matches by
    // name and ignores unknowns); under-reporting silently drops writes.

    function stripComments(src) {
        return String(src)
            .replace(/\/\*[\s\S]*?\*\//g, ' ')
            .replace(/\/\/[^\n]*/g, ' ');
    }

    function expandDefines(src) {
        // one pass: `#define NAME 42` → substitute NAME in later lines.
        var defines = {};
        var lines = src.split('\n');
        var out = [];
        for (var i = 0; i < lines.length; i++) {
            var m = /^\s*#\s*define\s+([A-Za-z_]\w*)\s+(\d+)\s*$/.exec(lines[i]);
            if (m) {
                defines[m[1]] = m[2];
                out.push('');
                continue;
            }
            var line = lines[i];
            for (var name in defines) {
                line = line.replace(new RegExp('\\b' + name + '\\b', 'g'), defines[name]);
            }
            out.push(line);
        }
        return out.join('\n');
    }

    // kind: 'uniform' | 'attributes'. Vertex-stage `in` counts as an
    // attribute; fragment `in` is a varying input and is not queryable.
    function collectDecls(src, kind) {
        var text = expandDefines(stripComments(src));
        var out = [];
        var seen = {};
        var re = /\b(uniform|attribute|in)\s+(?:highp\s+|mediump\s+|lowp\s+)?[A-Za-z_]\w*\s+([A-Za-z_]\w*(?:\s*\[[^\]]*\])?(?:\s*,\s*[A-Za-z_]\w*(?:\s*\[[^\]]*\])?)*)\s*;/g;
        var m;
        while ((m = re.exec(text)) !== null) {
            var kw = m[1];
            var names = m[2].split(',');
            for (var i = 0; i < names.length; i++) {
                var decl = names[i].trim();
                var am = /^([A-Za-z_]\w*)\s*\[\s*(\d+)\s*\]$/.exec(decl);
                var base = am ? am[1] : decl;
                var want = kw === kind || (kind === 'attributes' && kw === 'in');
                if (!want) continue;
                if (am) {
                    // arrays appear under their `[0]` name (WebGL active-name
                    // convention); the bare name maps to element 0.
                    if (!seen[base + '[0]']) {
                        seen[base + '[0]'] = true;
                        out.push(base + '[0]');
                    }
                    if (!seen[base]) {
                        seen[base] = true;
                        out.push(base);
                    }
                } else if (!seen[base]) {
                    seen[base] = true;
                    out.push(base);
                }
            }
        }
        return out;
    }

    function scanProgramSource(vertexSrc, fragmentSrc) {
        var uniforms = collectDecls(vertexSrc, 'uniform');
        var seen = {};
        var i;
        for (i = 0; i < uniforms.length; i++) seen[uniforms[i]] = true;
        var fragUniforms = collectDecls(fragmentSrc, 'uniform');
        for (i = 0; i < fragUniforms.length; i++) {
            if (!seen[fragUniforms[i]]) {
                seen[fragUniforms[i]] = true;
                uniforms.push(fragUniforms[i]);
            }
        }
        return {
            uniforms: uniforms,
            attributes: collectDecls(vertexSrc, 'attributes'),
        };
    }

    // ── resource handles ────────────────────────────────────────────

    function glRes(kind, id) {
        return { __glKind: kind, __glId: id };
    }

    function resId(v, api) {
        if (v === null || v === undefined) return 0xffffffff;
        if (typeof v === 'object' && typeof v.__glId === 'number') return v.__glId;
        throw new Error(api + ': not a GL resource handle');
    }

    function NULL_HANDLE() {
        return 0xffffffff;
    }

    // ── recording context ───────────────────────────────────────────

    function makeRecordingContext(canvasId, width, height, attrsBits) {
        var streamId = canvasId;
        var self = {};

        // link-time scan cache: program handle → { uniforms, attributes }
        var programInfo = {};
        var shaderSource = {}; // shader handle → source text
        var shaderKind = {}; // shader handle → 'v' | 'f'
        var programShaders = {}; // 'pid:v' / 'pid:f' → shader handle
        var errors = [];

        // 每帧重开:reset_for_frame 清掉上一帧的流,而上下文对象跨帧缓存,
        // 所以缓存命中路径也要重开(帧内幂等——同 target 复用同一条流)。
        self.__beginFrameStream = function () {
            __gl_stream_begin(streamId, width, height, 0, attrsBits);
        };
        self.__beginFrameStream();

        // GLenum surface: author code reads gl.VERTEX_SHADER off the context
        // itself (the browser exposes constants on the WebGLRenderingContext).
        for (var k in GL) self[k] = GL[k];

        self.drawingBufferWidth = width;
        self.drawingBufferHeight = height;
        self.canvas = { id: canvasId };

        function badToken(api, what) {
            throw new Error(api + ': ' + what);
        }

        self.getContextAttributes = function () {
            return {
                alpha: (attrsBits & 1) !== 0,
                antialias: (attrsBits & 2) !== 0,
                premultipliedAlpha: (attrsBits & 4) !== 0,
                preserveDrawingBuffer: (attrsBits & 8) !== 0,
                depth: (attrsBits & 16) !== 0,
                stencil: (attrsBits & 32) !== 0,
                failIfMajorPerformanceCaveat: false,
                powerPreference: 'default',
                desynchronized: false,
            };
        };

        self.isContextLost = function () {
            return false;
        };

        self.getExtension = function (name) {
            var n = String(name);
            if (SUPPORTED_EXTENSIONS.indexOf(n) >= 0) return makeExtension(n);
            return null;
        };

        self.getSupportedExtensions = function () {
            return SUPPORTED_EXTENSIONS.slice();
        };

        self.getShaderPrecisionFormat = function (shaderType, precisionType) {
            void shaderType;
            // Pinned precision: the executors' drivers match or exceed; the
            // table is the floor.
            if (
                precisionType === GL.LOW_FLOAT ||
                precisionType === GL.MEDIUM_FLOAT ||
                precisionType === GL.HIGH_FLOAT
            ) {
                return { rangeMin: 127, rangeMax: 127, precision: 23 };
            }
            return { rangeMin: 31, rangeMax: 30, precision: 0 };
        };

        self.getError = function () {
            return errors.length ? errors.shift() : GL.NO_ERROR;
        };

        self.getParameter = function (pname) {
            var p = pname >>> 0;
            if (p in CAPS) {
                var v = CAPS[p];
                return typeof v === 'object' ? v.slice() : v;
            }
            // Binding queries live at the executor; the browser returns the
            // bound object, three only probes existence in debug paths.
            if (
                p === GL.CURRENT_PROGRAM ||
                p === GL.TEXTURE_BINDING_2D ||
                p === GL.ARRAY_BUFFER_BINDING ||
                p === GL.ELEMENT_ARRAY_BUFFER_BINDING ||
                p === GL.FRAMEBUFFER_BINDING
            ) {
                return null;
            }
            return null;
        };

        self.getIntegerv = self.getParameter;

        self.getString = function (name) {
            var v = CAPS[name === 0x1f00 ? 'VENDOR' : name === 0x1f01 ? 'RENDERER' : name === 0x1f02 ? 'VERSION' : name === 0x8b8c ? 'SHADING_LANGUAGE_VERSION' : ''];
            return v === undefined ? '' : v;
        };

        // ── shader / program ────────────────────────────────────────

        self.createShader = function (type) {
            var id = __gl_alloc_handle();
            var e = new Encoder(6);
            e.u16(type).u32(id);
            emit(streamId, 0 /* SHADER_CREATE */, e.done(0));
            shaderKind[id] = type === GL.VERTEX_SHADER ? 'v' : 'f';
            return glRes('shader', id);
        };

        self.shaderSource = function (shader, source) {
            var id = resId(shader, 'shaderSource');
            shaderSource[id] = String(source);
            var range = internString(streamId, shaderSource[id]);
            var e = new Encoder(8);
            e.u32(id).u32(range);
            emit(streamId, 1 /* SHADER_SOURCE */, e.done(1));
        };

        self.getShaderSource = function () {
            badToken('getShaderSource', 'sources live in the stream, not in core');
        };

        self.compileShader = function (shader) {
            var e = new Encoder(4);
            e.u32(resId(shader, 'compileShader'));
            emit(streamId, 2 /* COMPILE_SHADER */, e.done(2));
        };

        self.deleteShader = function (shader) {
            if (!shader) return;
            var e = new Encoder(4);
            e.u32(resId(shader, 'deleteShader'));
            emit(streamId, 3, e.done(3));
        };

        self.createProgram = function () {
            var id = __gl_alloc_handle();
            var e = new Encoder(4);
            e.u32(id);
            emit(streamId, 4 /* PROGRAM_CREATE */, e.done(4));
            return glRes('program', id);
        };

        self.attachShader = function (program, shader) {
            var pid = resId(program, 'attachShader');
            var sid = resId(shader, 'attachShader');
            programShaders[pid + ':' + shaderKind[sid]] = sid;
            var e = new Encoder(8);
            e.u32(pid).u32(sid);
            emit(streamId, 5, e.done(5));
        };

        self.detachShader = function () {
            badToken('detachShader', 'unsupported in the recording face');
        };

        self.linkProgram = function (program) {
            var pid = resId(program, 'linkProgram');
            var e = new Encoder(4);
            e.u32(pid);
            emit(streamId, 6 /* PROGRAM_LINK */, e.done(6));
            // Link is the scan point: three queries the active lists right
            // after.
            programInfo[pid] = scanProgramSource(
                shaderSource[programShaders[pid + ':v']] || '',
                shaderSource[programShaders[pid + ':f']] || ''
            );
        };

        self.deleteProgram = function (program) {
            if (!program) return;
            var id = resId(program, 'deleteProgram');
            var e = new Encoder(4);
            e.u32(id);
            emit(streamId, 7, e.done(7));
            delete programInfo[id];
        };

        self.useProgram = function (program) {
            var e = new Encoder(4);
            e.u32(resId(program, 'useProgram'));
            emit(streamId, 8 /* PROGRAM_USE */, e.done(8));
        };

        self.getShaderParameter = function (shader, pname) {
            if (pname === GL.COMPILE_STATUS) return true;
            if (pname === GL.DELETE_STATUS) return false;
            if (pname === GL.SHADER_TYPE) {
                return shaderKind[resId(shader, 'getShaderParameter')] === 'v'
                    ? GL.VERTEX_SHADER
                    : GL.FRAGMENT_SHADER;
            }
            return null;
        };

        self.getShaderInfoLog = function () {
            return '';
        };

        self.getProgramParameter = function (program, pname) {
            var info = programInfo[resId(program, 'getProgramParameter')] || {
                uniforms: [],
                attributes: [],
            };
            switch (pname) {
                case GL.LINK_STATUS:
                    return true;
                case GL.DELETE_STATUS:
                    return false;
                case GL.VALIDATE_STATUS:
                    return true;
                case GL.ACTIVE_UNIFORMS:
                    return info.uniforms.length;
                case GL.ACTIVE_ATTRIBUTES:
                    return info.attributes.length;
                case GL.ACTIVE_UNIFORM_BLOCKS:
                    return 0;
                default:
                    return null;
            }
        };

        self.getProgramInfoLog = function () {
            return '';
        };

        self.getAttachedShaders = function (program) {
            var pid = resId(program, 'getAttachedShaders');
            var out = [];
            if (programShaders[pid + ':v'] !== undefined) out.push(glRes('shader', programShaders[pid + ':v']));
            if (programShaders[pid + ':f'] !== undefined) out.push(glRes('shader', programShaders[pid + ':f']));
            return out;
        };

        // ── buffer ──────────────────────────────────────────────────

        self.createBuffer = function () {
            var id = __gl_alloc_handle();
            var e = new Encoder(4);
            e.u32(id);
            emit(streamId, 9 /* BUFFER_CREATE */, e.done(9));
            return glRes('buffer', id);
        };

        self.bindBuffer = function (target, buffer) {
            var e = new Encoder(6);
            e.u16(target).u32(buffer ? resId(buffer, 'bindBuffer') : NULL_HANDLE());
            emit(streamId, 10, e.done(10));
        };

        self.bufferData = function (target, srcData, usage) {
            if (srcData === null || srcData === undefined || typeof srcData === 'number') {
                // size form: bufferData(target, size, usage)
                var e2 = new Encoder(8);
                e2.u16(target).u16(usage).u32(srcData >>> 0);
                emit(streamId, 12 /* BUFFER_DATA_NULL */, e2.done(12));
                return;
            }
            var bytes = typedBytes(srcData);
            if (!bytes) badToken('bufferData', 'expected TypedArray/ArrayBuffer data');
            var range = intern(streamId, bytes);
            var e = new Encoder(8);
            e.u16(target).u16(usage).u32(range);
            emit(streamId, 11 /* BUFFER_DATA */, e.done(11));
        };

        self.bufferSubData = function () {
            badToken('bufferSubData', 're-upload with bufferData (streams cannot patch)');
        };

        self.deleteBuffer = function (buffer) {
            if (!buffer) return;
            var e = new Encoder(4);
            e.u32(resId(buffer, 'deleteBuffer'));
            emit(streamId, 13, e.done(13));
        };

        // ── vao ─────────────────────────────────────────────────────

        self.createVertexArray = function () {
            var id = __gl_alloc_handle();
            var e = new Encoder(4);
            e.u32(id);
            emit(streamId, 14 /* VAO_CREATE */, e.done(14));
            return glRes('vao', id);
        };

        self.bindVertexArray = function (vao) {
            var e = new Encoder(4);
            e.u32(vao ? resId(vao, 'bindVertexArray') : NULL_HANDLE());
            emit(streamId, 15, e.done(15));
        };

        self.deleteVertexArray = function (vao) {
            if (!vao) return;
            var e = new Encoder(4);
            e.u32(resId(vao, 'deleteVertexArray'));
            emit(streamId, 16, e.done(16));
        };

        // ── attributes ──────────────────────────────────────────────
        // three passes the NAME (string) it got from getAttribLocation-style
        // lookups through vertexAttribPointer — actually the browser takes
        // the numeric location; three stores the name and resolves once.
        // We accept BOTH: a string interns as the name; a number maps to the
        // link-scan's attribute list order (deterministic both sides).

        self.enableVertexAttribArray = function (nameOrIndex) {
            var e = new Encoder(4);
            e.u32(attrNameRange(nameOrIndex, 'enableVertexAttribArray'));
            emit(streamId, 17, e.done(17));
        };

        self.disableVertexAttribArray = function (nameOrIndex) {
            var e = new Encoder(4);
            e.u32(attrNameRange(nameOrIndex, 'disableVertexAttribArray'));
            emit(streamId, 18, e.done(18));
        };

        self.vertexAttribPointer = function (nameOrIndex, size, type, normalized, stride, offset) {
            var e = new Encoder(16);
            e.u32(attrNameRange(nameOrIndex, 'vertexAttribPointer')).u8(size).u16(type).u8(normalized ? 1 : 0).u32(stride).u32(offset);
            emit(streamId, 19, e.done(19));
        };

        self.vertexAttribDivisor = function (nameOrIndex, divisor) {
            var e = new Encoder(8);
            e.u32(attrNameRange(nameOrIndex, 'vertexAttribDivisor')).u32(divisor);
            emit(streamId, 20, e.done(20));
        };

        function attrNameRange(nameOrIndex, api) {
            if (typeof nameOrIndex === 'string') {
                return internString(streamId, nameOrIndex);
            }
            // Numeric location: the executor resolves numeric indices against
            // the real driver directly (they are already driver locations in
            // the browser path three uses bindAttribLocation for). Numeric
            // indices pass through as-is in the name slot via a sentinel
            // encoding: "index:<n>".
            return internString(streamId, 'index:' + (nameOrIndex >>> 0));
            void api;
        }

        // ── uniforms ────────────────────────────────────────────────
        // Locations are (program, name) handles; the executor resolves them
        // after link and caches by name. `null` locations skip exactly like
        // the browser (three relies on this for conditional uniforms).

        function uniName(location, api) {
            if (location === null || location === undefined) return null;
            if (typeof location === 'object' && typeof location.__name === 'string') return location.__name;
            badToken(api, 'not a uniform location');
        }

        self.uniform1f = function (loc, x) {
            var name = uniName(loc, 'uniform1f');
            if (name === null) return;
            var range = internString(streamId, name);
            var e = new Encoder(8);
            e.u32(range).f32(x);
            emit(streamId, 21, e.done(21));
        };
        self.uniform2f = function (loc, x, y) {
            var name = uniName(loc, 'uniform2f');
            if (name === null) return;
            var range = internString(streamId, name);
            var e = new Encoder(12);
            e.u32(range).f32(x).f32(y);
            emit(streamId, 22, e.done(22));
        };
        self.uniform3f = function (loc, x, y, z) {
            var name = uniName(loc, 'uniform3f');
            if (name === null) return;
            var range = internString(streamId, name);
            var e = new Encoder(16);
            e.u32(range).f32(x).f32(y).f32(z);
            emit(streamId, 23, e.done(23));
        };
        self.uniform4f = function (loc, x, y, z, w) {
            var name = uniName(loc, 'uniform4f');
            if (name === null) return;
            var range = internString(streamId, name);
            var e = new Encoder(20);
            e.u32(range).f32(x).f32(y).f32(z).f32(w);
            emit(streamId, 24, e.done(24));
        };
        self.uniform1i = function (loc, v) {
            var name = uniName(loc, 'uniform1i');
            if (name === null) return;
            var range = internString(streamId, name);
            var e = new Encoder(8);
            e.u32(range).i32(v);
            emit(streamId, 25, e.done(25));
        };

        // Variable-length uniforms: [opcode][name u32][count u32][tail]
        // (matrix commands carry one more order byte after count).
        function emitVariableUniform(opcode, location, count, tailBytes, order) {
            var name = uniName(location, 'uniform');
            if (name === null) return;
            var nameRange = internString(streamId, name);
            var head = order ? 10 : 9;
            var out = new Uint8Array(head + tailBytes.length);
            var dv = new DataView(out.buffer);
            dv.setUint8(0, opcode);
            dv.setUint32(1, nameRange, true);
            dv.setUint32(5, count, true);
            if (order) dv.setUint8(9, order);
            out.set(tailBytes, head);
            __gl_stream_emit(streamId, Array.prototype.slice.call(out));
        }

        self.uniform1fv = function (loc, data) {
            var bytes = typedBytes(data);
            if (!bytes) return;
            emitVariableUniform(26, loc, bytes.byteLength / 4, bytes);
        };
        self.uniform2fv = self.uniform1fv;
        self.uniform3fv = self.uniform1fv;
        self.uniform4fv = self.uniform1fv;
        self.uniform1iv = function (loc, data) {
            var bytes = typedBytes(data);
            if (!bytes) return;
            emitVariableUniform(27, loc, bytes.byteLength / 4, bytes);
        };
        self.uniform2iv = self.uniform1iv;
        self.uniform3iv = self.uniform1iv;
        self.uniform4iv = self.uniform1iv;
        self.uniformMatrix2fv = function (loc, transpose, data) {
            void transpose; // WebGL2 requires false; the stream drops it
            var bytes = typedBytes(data);
            if (!bytes) return;
            emitVariableUniform(28, loc, bytes.byteLength / 16, bytes, 2);
        };
        self.uniformMatrix3fv = function (loc, transpose, data) {
            void transpose;
            var bytes = typedBytes(data);
            if (!bytes) return;
            emitVariableUniform(28, loc, bytes.byteLength / 36, bytes, 3);
        };
        self.uniformMatrix4fv = function (loc, transpose, data) {
            void transpose;
            var bytes = typedBytes(data);
            if (!bytes) return;
            emitVariableUniform(28, loc, bytes.byteLength / 64, bytes, 4);
        };

        self.getUniformLocation = function (program, name) {
            var info = programInfo[resId(program, 'getUniformLocation')];
            if (!info) return null;
            var n = String(name);
            if (info.uniforms.indexOf(n) >= 0) return { __name: n };
            return null;
        };

        self.getAttribLocation = function (program, name) {
            var info = programInfo[resId(program, 'getAttribLocation')];
            if (!info) return -1;
            return info.attributes.indexOf(String(name));
        };

        self.getActiveUniform = function (program, index) {
            var info = programInfo[resId(program, 'getActiveUniform')];
            if (!info || index >= info.uniforms.length) {
                badToken('getActiveUniform', 'index out of range');
            }
            return { name: info.uniforms[index], type: GL.FLOAT_VEC4, size: 1 };
        };

        self.getActiveAttrib = function (program, index) {
            var info = programInfo[resId(program, 'getActiveAttrib')];
            if (!info || index >= info.attributes.length) {
                badToken('getActiveAttrib', 'index out of range');
            }
            return { name: info.attributes[index], type: GL.FLOAT_VEC3, size: 1 };
        };

        self.getUniformBlockIndex = function () {
            return 0xffffffff;
        };
        self.getActiveUniformBlockName = function () {
            return '';
        };
        self.getActiveUniformBlockParameter = function () {
            return null;
        };

        // ── textures ────────────────────────────────────────────────

        self.createTexture = function () {
            var id = __gl_alloc_handle();
            var e = new Encoder(4);
            e.u32(id);
            emit(streamId, 29, e.done(29));
            return glRes('texture', id);
        };

        self.deleteTexture = function (texture) {
            if (!texture) return;
            var e = new Encoder(4);
            e.u32(resId(texture, 'deleteTexture'));
            emit(streamId, 30, e.done(30));
        };

        self.activeTexture = function (unit) {
            // WebGL 约定传 TEXTURE0+i 的 GLenum;归一化成单元序号(u8)。
            var idx = unit - GL.TEXTURE0;
            var e = new Encoder(1);
            e.u8(idx >= 0 && idx < 32 ? idx : (unit >>> 0));
            emit(streamId, 31, e.done(31));
        };

        self.bindTexture = function (target, texture) {
            var e = new Encoder(6);
            e.u16(target).u32(texture ? resId(texture, 'bindTexture') : NULL_HANDLE());
            emit(streamId, 32, e.done(32));
        };

        self.texParameteri = function (target, pname, value) {
            var e = new Encoder(8);
            e.u16(target).u16(pname).i32(value);
            emit(streamId, 33, e.done(33));
        };

        self.texParameterf = function (target, pname, value) {
            var e = new Encoder(8);
            e.u16(target).u16(pname).f32(value);
            emit(streamId, 34, e.done(34));
        };

        self.texImage2D = function (target, level, internalformat, width, height, border, format, type, pixels) {
            // The bytes-form (TypedArray) streams the payload directly. DOM
            // sources (img/canvas/video) arrive in batch ② via the media
            // asset bridge; here they error with a pointer.
            var bytes = null;
            if (pixels !== undefined && pixels !== null) {
                bytes = typedBytes(pixels);
                if (!bytes) {
                    badToken(
                        'texImage2D',
                        'DOM-image sources need the batch-2 asset bridge; pass TypedArray pixels or use declared asset textures'
                    );
                }
            }
            var e = new Encoder(23);
            e.u16(target).u8(level).i32(internalformat).u32(width).u32(height).u16(format).u16(type);
            if (bytes) {
                var range = intern(streamId, bytes);
                e.u8(1).u32(range);
            } else {
                e.u8(0).u32(0);
            }
            emit(streamId, 35, e.done(35));
        };

        self.texSubImage2D = function () {
            badToken('texSubImage2D', 'batch 2 — upload whole textures');
        };
        self.copyTexImage2D = self.texSubImage2D;
        self.copyTexSubImage2D = self.texSubImage2D;
        self.compressedTexImage2D = function () {
            badToken('compressedTexImage2D', 'batch 2');
        };
        self.compressedTexSubImage2D = self.compressedTexImage2D;

        self.generateMipmap = function (target) {
            var e = new Encoder(2);
            e.u16(target);
            emit(streamId, 36, e.done(36));
        };

        self.pixelStorei = function (pname, value) {
            var e = new Encoder(6);
            e.u16(pname).i32(value);
            emit(streamId, 37, e.done(37));
        };

        // ── framebuffers ────────────────────────────────────────────

        self.createFramebuffer = function () {
            var id = __gl_alloc_handle();
            var e = new Encoder(4);
            e.u32(id);
            emit(streamId, 38, e.done(38));
            return glRes('fbo', id);
        };

        self.deleteFramebuffer = function (fbo) {
            if (!fbo) return;
            var e = new Encoder(4);
            e.u32(resId(fbo, 'deleteFramebuffer'));
            emit(streamId, 39, e.done(39));
        };

        self.bindFramebuffer = function (target, framebuffer) {
            var e = new Encoder(6);
            e.u16(target).u32(framebuffer ? resId(framebuffer, 'bindFramebuffer') : NULL_HANDLE());
            emit(streamId, 40, e.done(40));
        };

        self.framebufferTexture2D = function (target, attachment, textarget, texture, level) {
            var e = new Encoder(14);
            e.u16(target).u16(attachment).u16(textarget).u32(texture ? resId(texture, 'framebufferTexture2D') : NULL_HANDLE()).i32(level);
            emit(streamId, 41, e.done(41));
        };

        self.checkFramebufferStatus = function () {
            return GL.FRAMEBUFFER_COMPLETE;
        };

        self.framebufferRenderbuffer = function () {
            badToken('framebufferRenderbuffer', 'batch 2 (depth renderbuffers)');
        };
        self.createRenderbuffer = self.framebufferRenderbuffer;
        self.renderbufferStorage = self.framebufferRenderbuffer;
        self.blitFramebuffer = function () {
            badToken('blitFramebuffer', 'batch 2 (three RT resolves)');
        };

        self.readPixels = function () {
            // Core never reads pixels back (red line). Nothing consumes a
            // read whose result must reach the script, so it is not recorded.
            errors.push(GL.INVALID_OPERATION);
        };
        self.invalidateFramebuffer = function () {};

        // ── state ───────────────────────────────────────────────────

        self.viewport = function (x, y, w, h) {
            var e = new Encoder(16);
            e.i32(x).i32(y).i32(w).i32(h);
            emit(streamId, 42, e.done(42));
        };

        self.clearColor = function (r, g, b, a) {
            var e = new Encoder(16);
            e.f32(r).f32(g).f32(b).f32(a);
            emit(streamId, 43, e.done(43));
        };

        self.clear = function (mask) {
            var bits =
                (mask & GL.COLOR_BUFFER_BIT ? 1 : 0) |
                (mask & GL.DEPTH_BUFFER_BIT ? 2 : 0) |
                (mask & GL.STENCIL_BUFFER_BIT ? 4 : 0);
            var e = new Encoder(1);
            e.u8(bits);
            emit(streamId, 44, e.done(44));
        };

        self.enable = function (cap) {
            var e = new Encoder(2);
            e.u16(cap);
            emit(streamId, 45, e.done(45));
        };

        self.disable = function (cap) {
            var e = new Encoder(2);
            e.u16(cap);
            emit(streamId, 46, e.done(46));
        };

        self.depthFunc = function (func) {
            var e = new Encoder(2);
            e.u16(func);
            emit(streamId, 47, e.done(47));
        };

        self.depthMask = function (flag) {
            var e = new Encoder(1);
            e.u8(flag ? 1 : 0);
            emit(streamId, 48, e.done(48));
        };

        self.blendFunc = function (src, dst) {
            var e = new Encoder(4);
            e.u16(src).u16(dst);
            emit(streamId, 49, e.done(49));
        };

        self.blendFuncSeparate = function (srcRGB, dstRGB, srcA, dstA) {
            if (srcRGB === srcA && dstRGB === dstA) {
                self.blendFunc(srcRGB, dstRGB);
                return;
            }
            badToken('blendFuncSeparate', 'separate blend factors ride batch 2');
        };

        self.blendEquation = function (mode) {
            var e = new Encoder(2);
            e.u16(mode);
            emit(streamId, 50, e.done(50));
        };

        self.blendColor = function (r, g, b, a) {
            var e = new Encoder(16);
            e.f32(r).f32(g).f32(b).f32(a);
            emit(streamId, 51, e.done(51));
        };

        self.cullFace = function (mode) {
            var e = new Encoder(2);
            e.u16(mode);
            emit(streamId, 52, e.done(52));
        };

        self.frontFace = function (mode) {
            var e = new Encoder(2);
            e.u16(mode);
            emit(streamId, 53, e.done(53));
        };

        self.clearDepth = function (d) {
            var e = new Encoder(4);
            e.f32(d);
            emit(streamId, 54, e.done(54));
        };

        self.clearStencil = function (s) {
            var e = new Encoder(4);
            e.i32(s);
            emit(streamId, 55, e.done(55));
        };

        self.polygonOffset = function (factor, units) {
            var e = new Encoder(8);
            e.f32(factor).f32(units);
            emit(streamId, 56, e.done(56));
        };

        self.scissor = function () {
            badToken('scissor', 'batch 2');
        };
        self.stencilFunc = self.scissor;
        self.stencilOp = self.scissor;
        self.stencilMask = self.scissor;
        self.hint = function () {};
        self.lineWidth = function () {};

        // ── draw ────────────────────────────────────────────────────

        self.drawArrays = function (mode, first, count) {
            var e = new Encoder(12);
            e.u16(mode).i32(first).i32(count);
            emit(streamId, 57, e.done(57));
        };

        self.drawElements = function (mode, count, type, offset) {
            var e = new Encoder(12);
            e.u16(mode).i32(count).u16(type).i32(offset);
            emit(streamId, 58, e.done(58));
        };

        self.drawBuffers = function () {
            badToken('drawBuffers', 'MRT rides batch 2');
        };

        self.flush = function () {};
        self.finish = function () {};

        return self;
    }

    // ── getContext integration ──────────────────────────────────────

    var contexts = {};

    function attrsFromOptions(options) {
        options = options || {};
        function b(v, dflt) {
            return v === undefined || v === null ? dflt : !!v;
        }
        return {
            alpha: b(options.alpha, true),
            antialias: b(options.antialias, true),
            premultiplied_alpha: b(options.premultipliedAlpha, true),
            preserve_drawing_buffer: b(options.preserveDrawingBuffer, false),
            depth: b(options.depth, true),
            stencil: b(options.stencil, false),
        };
    }

    // ContextAttrs::unpack in core mirrors this byte order.
    function packAttrsBits(attrs) {
        var bits = 0;
        if (attrs.alpha) bits |= 1 << 0;
        if (attrs.antialias) bits |= 1 << 1;
        if (attrs.premultiplied_alpha) bits |= 1 << 2;
        if (attrs.preserve_drawing_buffer) bits |= 1 << 3;
        if (attrs.depth) bits |= 1 << 4;
        if (attrs.stencil) bits |= 1 << 5;
        return bits;
    }

    function getWebGLContext(nodeId, type, options) {
        var key = String(nodeId);
        var want = type === 'webgl2' ? 'webgl2' : 'webgl';
        var existing = contexts[key];
        if (existing) {
            if (existing.__kind !== want) {
                throw new Error(
                    "getContext: canvas '" + key + "' already has a '" + existing.__kind + "' context — the browser forbids a second kind too"
                );
            }
            existing.__beginFrameStream();
            return existing;
        }
        if (want !== 'webgl2') {
            throw new Error(
                "getContext('webgl') (WebGL1) is not supported — three r152+ and every OpenCat executor target WebGL2"
            );
        }

        var attrs = attrsFromOptions(options);
        var box = (typeof __read_box === 'function' && __read_box(key)) || {};
        var width = Math.max(1, Math.round(Number(box.width) || 0));
        var height = Math.max(1, Math.round(Number(box.height) || 0));
        if (!box.width || !box.height) {
            throw new Error(
                "getContext('webgl2'): canvas '" + key + "' has no layout box — declare width/height (class or style) before getContext"
            );
        }

        var rec = makeRecordingContext(key, width, height, packAttrsBits(attrs));
        rec.__kind = want;
        contexts[key] = rec;
        return rec;
    }

    // Route `ctx.getContext(id, 'webgl2'|'webgl', attrs)` (and the element
    // face, which forwards through here) into the recorder.
    var baseGetContext = ctx.getContext;
    ctx.getContext = function (nodeId, type, attrs) {
        var t = String(type);
        if (t === 'webgl2' || t === 'webgl') {
            return getWebGLContext(nodeId, t, attrs);
        }
        return baseGetContext(nodeId, type, attrs);
    };
})();
