//! GPU 命令流段：WebGL2 / WebGPU 录制面的跨边界 IR。
//!
//! core 的 HTML 协议面（`webgl_api.js`）把 `getContext('webgl2')` 返回的
//! 上下文实现成**录制器**：每次 GL 调用编码为 `[opcode: u8][payload]` 追加进
//! 目标 canvas 的一条流；同步查询（getUniformLocation / getShaderParameter /
//! getParameter…）由 core 侧的确定性客户端状态机回答，不进流。
//!
//! 执行契约与 Scene3 时代同构：占位 `DrawOp::Image{ImageRef::Generated}` 保
//! z 序，执行宿主（engine = glow 直调真 GL / web = 浏览器真 WebGL2）渲染流
//! 后把像素回填 `FrameMediaPlan::generated_images` 的同名占位条目。命令流对
//! 执行器不是中间表示——WebGL2 就是 OpenGL ES 的调用序列，engine 端
//! 一对一映射到 glow，零翻译。
//!
//! ## payload 布局
//!
//! 每条命令 `[opcode: u8][payload]`，payload 长度由 [`gl_opcode::PAYLOAD_LEN`]
//! 按命令静态给出；变长数据（bufferData / texImage2D 像素、shader 源码）不进
//! payload，走流内 intern 池（[`GpuOpFrame::byte_ranges`]），payload 里只带
//! `u32 range id`。跨帧不变的 intern（纹理、VBO、shader 源码）由执行端按
//! 内容 hash 缓存——同内容二次上载在执行端是 no-op。
//!
//! ## 状态基线
//!
//! 资源句柄（buffer/texture/program/…）是 realm 级持久 id，跨帧有效；绘制
//! 命令流每帧**自足**——帧首的绑定/enable 状态一律由帧内命令显式建立，
//! 执行端不假设任何跨帧 GL 状态。

use super::draw_types::TableRange;
use super::generated_image::GeneratedImageId;

/// 一帧的 GPU 命令流段（与 2D `draw` 并列的跨边界 IR）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuSection {
    pub streams: Vec<GpuStream>,
}

/// 一个 WebGL/WebGPU 上下文（目标 canvas）本帧的命令流。
#[derive(Clone, Debug, PartialEq)]
pub struct GpuStream {
    /// 占位生成图 id（内容寻址），draw 流里 `ImageRef::Generated` 的 z 序
    /// 引用它；执行宿主渲染本流后按它回填像素。
    pub image_id: GeneratedImageId,
    /// 目标 canvas 节点 id。
    pub target_id: String,
    pub width: u32,
    pub height: u32,
    pub backend: GpuBackend,
    /// `getContext(kind, attrs)` 的属性——执行端建真实上下文时遵守。
    pub attrs: ContextAttrs,
    /// 位编码命令流。
    pub ops: GpuOpFrame,
}

/// 后端协议。WebGL2 本批实现；WebGPU 命令流面（→ wgpu 一对一执行器）留位。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GpuBackend {
    #[default]
    WebGL2 = 0,
    WebGPU = 1,
}

/// `getContext` 属性（WebGL2 的布尔面；数值属性走 capability 表）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ContextAttrs {
    pub alpha: bool,
    pub antialias: bool,
    pub premultiplied_alpha: bool,
    pub preserve_drawing_buffer: bool,
    pub depth: bool,
    pub stencil: bool,
}

impl ContextAttrs {
    /// 打包 6 位：alpha|antialias|premultiplied|preserve|depth|stencil。
    pub fn pack(self) -> u8 {
        let mut bits = 0u8;
        if self.alpha {
            bits |= 1 << 0;
        }
        if self.antialias {
            bits |= 1 << 1;
        }
        if self.premultiplied_alpha {
            bits |= 1 << 2;
        }
        if self.preserve_drawing_buffer {
            bits |= 1 << 3;
        }
        if self.depth {
            bits |= 1 << 4;
        }
        if self.stencil {
            bits |= 1 << 5;
        }
        bits
    }

    pub fn unpack(bits: u8) -> Self {
        Self {
            alpha: bits & (1 << 0) != 0,
            antialias: bits & (1 << 1) != 0,
            premultiplied_alpha: bits & (1 << 2) != 0,
            preserve_drawing_buffer: bits & (1 << 3) != 0,
            depth: bits & (1 << 4) != 0,
            stencil: bits & (1 << 5) != 0,
        }
    }
}

/// 一条流的命令帧：命令字节流 + intern 池。
///
/// 与 2D [`super::draw_frame::DrawOpFrame`] 同构但更薄：GL 命令的小参数
/// （handle/uniform 标量/枚举）直接内联进 payload，f32 不 intern；只有
/// 变长大块（bufferData 像素、texImage2D 数据、shader 源码 UTF-8）intern
/// 到 `bytes`，payload 带 range id。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuOpFrame {
    /// `[opcode: u8][payload]` 的连续字节流。
    pub ops: Vec<u8>,
    /// intern 池（buffer 数据 / 纹理像素 / shader 源码 UTF-8）。
    pub bytes: Vec<u8>,
    /// `range id`（payload 里的 u32 索引）→ bytes 的区间。
    pub byte_ranges: Vec<TableRange>,
}

impl GpuOpFrame {
    /// 追加一条定长命令（opcode + payload）。
    pub fn push_cmd(&mut self, opcode: u8, payload: &[u8]) {
        self.ops.push(opcode);
        self.ops.extend_from_slice(payload);
    }

    /// intern 一段字节，返回 payload 用的 range id。
    pub fn intern_bytes(&mut self, data: &[u8]) -> u32 {
        // 内容寻址去重：同内容二次 intern 返回同 range（跨帧纹理/源码
        // 在执行端按内容 hash 缓存的先决——传输层面先去掉重复字节）。
        for (i, range) in self.byte_ranges.iter().enumerate() {
            let existing = &self.bytes[range.start as usize..(range.start + range.len) as usize];
            if existing.len() == data.len() && existing == data {
                return i as u32;
            }
        }
        let start = self.bytes.len() as u32;
        self.bytes.extend_from_slice(data);
        self.byte_ranges.push(TableRange {
            start,
            len: data.len() as u32,
        });
        (self.byte_ranges.len() - 1) as u32
    }
}

/// GL 命令 opcode 表 —— 权威定义，TS 侧由 schema 生成对齐。
///
/// payload 布局（小端）：
/// - `u32` 句柄：core 侧分配的不透明资源 id，realm 级持久；
/// - `u32` range id：[`GpuOpFrame::byte_ranges`] 的索引；
/// - 枚举/常量：原始 GL 数值（GLenum），执行端不翻译；
/// - `NULL_HANDLE = 0xFFFF_FFFF`：绑定解绑。
///
/// 变长命令的 payload 长度由首部 count 字段决定；其余命令
/// [`gl_opcode::PAYLOAD_LEN`] 给出定长。
pub mod gl_opcode {
    // ── shader / program ────────────────────────────────────────────
    /// {type: u16, handle: u32}
    pub const SHADER_CREATE: u8 = 0;
    /// {handle: u32, src_range: u32} — src 是 shader 源码 UTF-8
    pub const SHADER_SOURCE: u8 = 1;
    /// {handle: u32}
    pub const COMPILE_SHADER: u8 = 2;
    pub const SHADER_DELETE: u8 = 3;
    pub const PROGRAM_CREATE: u8 = 4;
    /// {program: u32, shader: u32}
    pub const PROGRAM_ATTACH: u8 = 5;
    pub const PROGRAM_LINK: u8 = 6;
    pub const PROGRAM_DELETE: u8 = 7;
    /// {handle: u32}
    pub const PROGRAM_USE: u8 = 8;

    // ── buffer ──────────────────────────────────────────────────────
    pub const BUFFER_CREATE: u8 = 9;
    /// {target: u16, handle: u32}
    pub const BUFFER_BIND: u8 = 10;
    /// {target: u16, usage: u8, range: u32} — range 为数据（空 range = null 数据）
    pub const BUFFER_DATA: u8 = 11;
    /// {target: u16, usage: u8, size: u32} — null 数据的占位分配
    pub const BUFFER_DATA_NULL: u8 = 12;
    pub const BUFFER_DELETE: u8 = 13;

    // ── vertex array / attributes ───────────────────────────────────
    pub const VAO_CREATE: u8 = 14;
    /// {handle: u32}
    pub const VAO_BIND: u8 = 15;
    pub const VAO_DELETE: u8 = 16;
    /// {name: u32(range)} — attribute 以 (program, name) 寻址：真 location
    /// 由驱动 link 后才定，core 合成的数字 id 必然漂移，执行器按 name 缓存
    /// getAttribLocation 的结果。
    pub const ATTRIB_ENABLE: u8 = 17;
    pub const ATTRIB_DISABLE: u8 = 18;
    /// {name: u32, size: u8, type: u16, normalized: u8, stride: u32, offset: u32}
    pub const ATTRIB_POINTER: u8 = 19;
    /// {name: u32, divisor: u32}
    pub const ATTRIB_DIVISOR: u8 = 20;

    // ── uniforms（同 attribute：(program 当前绑定, name_range) 寻址）──
    /// {name: u32(range), x: f32}
    pub const UNIFORM_1F: u8 = 21;
    /// {name: u32, x: f32, y: f32}
    pub const UNIFORM_2F: u8 = 22;
    /// {name: u32, x: f32, y: f32, z: f32}
    pub const UNIFORM_3F: u8 = 23;
    /// {name: u32, x: f32, y: f32, z: f32, w: f32}
    pub const UNIFORM_4F: u8 = 24;
    /// {name: u32, v: i32}
    pub const UNIFORM_1I: u8 = 25;
    /// {name: u32, flags: u32, data} — flags = count；data 是 count×f32。
    pub const UNIFORM_NF: u8 = 26;
    /// {name: u32, flags: u32, data} — flags = count；data 是 count×i32。
    pub const UNIFORM_NI: u8 = 27;
    /// {name: u32, count: u32, order: u8, data} — count = 矩阵数，
    /// order = 阶（2/3/4），data 是 count×order² f32。WebGL2 的 transpose
    /// 恒 false，流里不编码。
    pub const UNIFORM_MATRIX_4F: u8 = 28;

    // ── textures ────────────────────────────────────────────────────
    pub const TEXTURE_CREATE: u8 = 29;
    pub const TEXTURE_DELETE: u8 = 30;
    /// {unit: u8}
    pub const TEXTURE_ACTIVE: u8 = 31;
    /// {target: u16, handle: u32}
    pub const TEXTURE_BIND: u8 = 32;
    /// {target: u16, pname: u16, value: i32}
    pub const TEX_PARAM_I: u8 = 33;
    /// {target: u16, pname: u16, value: f32}
    pub const TEX_PARAM_F: u8 = 34;
    /// {target: u16, level: u8, internal: i32, w: u32, h: u32, format: u16,
    ///  type: u16, has_data: u8, range: u32}（has_data=0 时 range 恒 0）
    pub const TEX_IMAGE_2D: u8 = 35;
    /// {target: u16}
    pub const GENERATE_MIPMAP: u8 = 36;
    /// {pname: u16, value: i32}
    pub const PIXEL_STORE_I: u8 = 37;

    // ── framebuffers ────────────────────────────────────────────────
    pub const FRAMEBUFFER_CREATE: u8 = 38;
    pub const FRAMEBUFFER_DELETE: u8 = 39;
    /// {target: u16, handle: u32}
    pub const BIND_FRAMEBUFFER: u8 = 40;
    /// {target: u16, attachment: u16, tex_target: u16, handle: u32, level: i32}
    pub const FRAMEBUFFER_TEXTURE_2D: u8 = 41;

    // ── state ───────────────────────────────────────────────────────
    /// {x: i32, y: i32, w: i32, h: i32}
    pub const VIEWPORT: u8 = 42;
    /// {r: f32, g: f32, b: f32, a: f32}
    pub const CLEAR_COLOR: u8 = 43;
    /// {mask: u8(bit0=color|bit1=depth|bit2=stencil)}
    pub const CLEAR: u8 = 44;
    /// {cap: u16}
    pub const ENABLE: u8 = 45;
    pub const DISABLE: u8 = 46;
    /// {func: u16}
    pub const DEPTH_FUNC: u8 = 47;
    /// {flag: u8}
    pub const DEPTH_MASK: u8 = 48;
    /// {src: u16, dst: u16}
    pub const BLEND_FUNC: u8 = 49;
    /// {mode: u16}
    pub const BLEND_EQUATION: u8 = 50;
    /// {r: f32, g: f32, b: f32, a: f32}
    pub const BLEND_COLOR: u8 = 51;
    /// {mode: u16}
    pub const CULL_FACE: u8 = 52;
    /// {mode: u16}
    pub const FRONT_FACE: u8 = 53;
    /// {d: f32}
    pub const CLEAR_DEPTH: u8 = 54;
    /// {s: i32}
    pub const CLEAR_STENCIL: u8 = 55;
    /// {factor: f32, units: f32}
    pub const POLYGON_OFFSET: u8 = 56;

    // ── draw ────────────────────────────────────────────────────────
    /// {mode: u16, first: i32, count: i32}
    pub const DRAW_ARRAYS: u8 = 57;
    /// {mode: u16, count: i32, type: u16, offset: i32}
    pub const DRAW_ELEMENTS: u8 = 58;

    // ── 批次②扩展（官方 three 需要面）────────────────────────────
    /// {src_rgb: u16, dst_rgb: u16, src_a: u16, dst_a: u16}
    pub const BLEND_FUNC_SEPARATE: u8 = 59;
    /// {rgb: u16, a: u16}
    pub const BLEND_EQUATION_SEPARATE: u8 = 60;
    /// {target: u16, level: u8, internal: i32, w: u32, h: u32, d: u32,
    ///  format: u16, type: u16, has_data: u8, range: u32}
    pub const TEX_IMAGE_3D: u8 = 61;
    /// {target: u16, level: u8, x: i32, y: i32, w: u32, h: u32, format: u16,
    ///  type: u16, has_data: u8, range: u32}
    pub const TEX_SUB_IMAGE_2D: u8 = 62;
    /// {handle: u32}
    pub const RENDERBUFFER_CREATE: u8 = 64;
    /// {target: u16, handle: u32}
    pub const BIND_RENDERBUFFER: u8 = 65;
    /// {handle: u32, internal: i32, w: u32, h: u32}
    pub const RENDERBUFFER_STORAGE: u8 = 66;
    /// {target: u16, attachment: u16, rb_handle: u32}
    pub const FRAMEBUFFER_RENDERBUFFER: u8 = 67;

    /// {r: u8, g: u8, b: u8, a: u8}
    pub const COLOR_MASK: u8 = 68;
    /// {name: u32, x: i32, y: i32}
    pub const UNIFORM_2I: u8 = 69;
    /// {name: u32, x: i32, y: i32, z: i32}
    pub const UNIFORM_3I: u8 = 70;
    /// {name: u32, x: i32, y: i32, z: i32, w: i32}
    pub const UNIFORM_4I: u8 = 71;
    /// {target: u16, levels: u8, internal: i32, w: u32, h: u32}
    pub const TEX_STORAGE_2D: u8 = 72;
    /// {target: u16, levels: u8, internal: i32, w: u32, h: u32, d: u32}
    pub const TEX_STORAGE_3D: u8 = 73;
    /// {target: u16, level: u8, x: i32, y: i32, z: i32, w: u32, h: u32,
    ///  d: u32, format: u16, type: u16, has_data: u8, range: u32}
    pub const TEX_SUB_IMAGE_3D: u8 = 74;
    /// {target: u16, index: u32, buffer: u32}
    pub const BIND_BUFFER_BASE: u8 = 75;
    /// {mode: u16, first: i32, count: i32, primcount: i32}
    pub const DRAW_ARRAYS_INSTANCED: u8 = 76;
    /// {mode: u16, count: i32, type: u16, offset: i32, primcount: i32}
    pub const DRAW_ELEMENTS_INSTANCED: u8 = 77;
    /// {index: u32, count: u32, data} — count = float 数（1..4）。
    /// 变长。
    pub const VERTEX_ATTRIB_NFV: u8 = 78;
    /// {program: u32, block_index: u32, binding: u32}
    pub const UNIFORM_BLOCK_BINDING: u8 = 79;
    /// {program: u32, index: u32, name_range: u32}
    pub const BIND_ATTRIB_LOCATION: u8 = 80;
    /// {x: i32, y: i32, w: i32, h: i32}
    pub const SCISSOR: u8 = 81;
    /// {func: u16, ref: i32, mask: u32}
    pub const STENCIL_FUNC: u8 = 82;
    /// {sfail: u16, dpfail: u16, dppass: u16}
    pub const STENCIL_OP: u8 = 83;
    /// {mask: u32}
    pub const STENCIL_MASK: u8 = 84;
    /// {count: u8, modes: [u32; count]} — WebGL2 MRT 绘制缓冲列表（PMREM 的
    /// setRenderTarget 路径）。变长（count 首部），不进 PAYLOAD_LEN。
    pub const DRAW_BUFFERS: u8 = 85;

    // ── sampler objects（WebGL2）────────────────────────────────────
    // WebGL2 把采样状态搬进 sampler 对象（纹理自身参数在绑定时被它覆盖），
    // g-webgl / g-device-api 每次 drawcall 都 `bindSampler`，所以采样语义
    // 只能靠这条通路表达。
    /// {handle: u32}
    pub const SAMPLER_CREATE: u8 = 86;
    /// {handle: u32}
    pub const SAMPLER_DELETE: u8 = 87;
    /// {unit: u32, sampler: u32} — sampler = NULL_HANDLE 解绑
    pub const SAMPLER_BIND: u8 = 88;
    /// {sampler: u32, pname: u16, value: i32}
    pub const SAMPLER_PARAM_I: u8 = 89;
    /// {sampler: u32, pname: u16, value: f32}
    pub const SAMPLER_PARAM_F: u8 = 90;

    /// {face: u16, func: u16, ref: i32, mask: u32} — 分离面模板测试
    pub const STENCIL_FUNC_SEPARATE: u8 = 91;
    /// {face: u16, sfail: u16, dpfail: u16, dppass: u16}
    pub const STENCIL_OP_SEPARATE: u8 = 92;
    /// {samples: u32, internal: i32, w: u32, h: u32} — 作用于客户端绑定的
    /// 渲染缓冲（与 RENDERBUFFER_STORAGE 同约定）
    pub const RENDERBUFFER_STORAGE_MS: u8 = 93;
    /// {target: u16, drawbuffer: i32, r: f32, g: f32, b: f32, a: f32}
    pub const CLEAR_BUFFER_FV: u8 = 94;
    /// {target: u16, drawbuffer: i32, x: i32, y: i32, z: i32, w: i32}
    pub const CLEAR_BUFFER_IV: u8 = 95;
    /// {target: u16, drawbuffer: i32, depth: f32, stencil: i32}
    pub const CLEAR_BUFFER_FI: u8 = 96;
    /// {target: u16, index: u32, buffer: u32, offset: i32, size: i32}
    pub const BIND_BUFFER_RANGE: u8 = 97;
    /// {sx0,sy0,sx1,sy1,dx0,dy0,dx1,dy1: i32 ×8, mask: u16（1=color 2=depth
    /// 4=stencil）, filter: u16} — 读/绘制 framebuffer 之间的块拷贝（MSAA 解析）
    pub const BLIT_FRAMEBUFFER: u8 = 98;

    pub const MAX_OPCODE: u8 = BLIT_FRAMEBUFFER;

    /// (名字, opcode) 全表 —— schema 生成面消费（TS 侧 GLOP 常量对象、
    /// realm 侧 gl-opcodes.generated.js 均由它迭代产出）。新增 opcode 时
    /// 在此同步加一行；缺项 = 生成物缺项，消费端 fail-loud。
    pub const NAMES: &[(&str, u8)] = &[
        ("SHADER_CREATE", SHADER_CREATE),
        ("SHADER_SOURCE", SHADER_SOURCE),
        ("COMPILE_SHADER", COMPILE_SHADER),
        ("SHADER_DELETE", SHADER_DELETE),
        ("PROGRAM_CREATE", PROGRAM_CREATE),
        ("PROGRAM_ATTACH", PROGRAM_ATTACH),
        ("PROGRAM_LINK", PROGRAM_LINK),
        ("PROGRAM_DELETE", PROGRAM_DELETE),
        ("PROGRAM_USE", PROGRAM_USE),
        ("BUFFER_CREATE", BUFFER_CREATE),
        ("BUFFER_BIND", BUFFER_BIND),
        ("BUFFER_DATA", BUFFER_DATA),
        ("BUFFER_DATA_NULL", BUFFER_DATA_NULL),
        ("BUFFER_DELETE", BUFFER_DELETE),
        ("VAO_CREATE", VAO_CREATE),
        ("VAO_BIND", VAO_BIND),
        ("VAO_DELETE", VAO_DELETE),
        ("ATTRIB_ENABLE", ATTRIB_ENABLE),
        ("ATTRIB_DISABLE", ATTRIB_DISABLE),
        ("ATTRIB_POINTER", ATTRIB_POINTER),
        ("ATTRIB_DIVISOR", ATTRIB_DIVISOR),
        ("UNIFORM_1F", UNIFORM_1F),
        ("UNIFORM_2F", UNIFORM_2F),
        ("UNIFORM_3F", UNIFORM_3F),
        ("UNIFORM_4F", UNIFORM_4F),
        ("UNIFORM_1I", UNIFORM_1I),
        ("UNIFORM_NF", UNIFORM_NF),
        ("UNIFORM_NI", UNIFORM_NI),
        ("UNIFORM_MATRIX_4F", UNIFORM_MATRIX_4F),
        ("TEXTURE_CREATE", TEXTURE_CREATE),
        ("TEXTURE_DELETE", TEXTURE_DELETE),
        ("TEXTURE_ACTIVE", TEXTURE_ACTIVE),
        ("TEXTURE_BIND", TEXTURE_BIND),
        ("TEX_PARAM_I", TEX_PARAM_I),
        ("TEX_PARAM_F", TEX_PARAM_F),
        ("TEX_IMAGE_2D", TEX_IMAGE_2D),
        ("GENERATE_MIPMAP", GENERATE_MIPMAP),
        ("PIXEL_STORE_I", PIXEL_STORE_I),
        ("FRAMEBUFFER_CREATE", FRAMEBUFFER_CREATE),
        ("FRAMEBUFFER_DELETE", FRAMEBUFFER_DELETE),
        ("BIND_FRAMEBUFFER", BIND_FRAMEBUFFER),
        ("FRAMEBUFFER_TEXTURE_2D", FRAMEBUFFER_TEXTURE_2D),
        ("VIEWPORT", VIEWPORT),
        ("CLEAR_COLOR", CLEAR_COLOR),
        ("CLEAR", CLEAR),
        ("ENABLE", ENABLE),
        ("DISABLE", DISABLE),
        ("DEPTH_FUNC", DEPTH_FUNC),
        ("DEPTH_MASK", DEPTH_MASK),
        ("BLEND_FUNC", BLEND_FUNC),
        ("BLEND_EQUATION", BLEND_EQUATION),
        ("BLEND_COLOR", BLEND_COLOR),
        ("CULL_FACE", CULL_FACE),
        ("FRONT_FACE", FRONT_FACE),
        ("CLEAR_DEPTH", CLEAR_DEPTH),
        ("CLEAR_STENCIL", CLEAR_STENCIL),
        ("POLYGON_OFFSET", POLYGON_OFFSET),
        ("DRAW_ARRAYS", DRAW_ARRAYS),
        ("DRAW_ELEMENTS", DRAW_ELEMENTS),
        ("BLEND_FUNC_SEPARATE", BLEND_FUNC_SEPARATE),
        ("BLEND_EQUATION_SEPARATE", BLEND_EQUATION_SEPARATE),
        ("TEX_IMAGE_3D", TEX_IMAGE_3D),
        ("TEX_SUB_IMAGE_2D", TEX_SUB_IMAGE_2D),
        ("RENDERBUFFER_CREATE", RENDERBUFFER_CREATE),
        ("BIND_RENDERBUFFER", BIND_RENDERBUFFER),
        ("RENDERBUFFER_STORAGE", RENDERBUFFER_STORAGE),
        ("FRAMEBUFFER_RENDERBUFFER", FRAMEBUFFER_RENDERBUFFER),
        ("COLOR_MASK", COLOR_MASK),
        ("UNIFORM_2I", UNIFORM_2I),
        ("UNIFORM_3I", UNIFORM_3I),
        ("UNIFORM_4I", UNIFORM_4I),
        ("TEX_STORAGE_2D", TEX_STORAGE_2D),
        ("TEX_STORAGE_3D", TEX_STORAGE_3D),
        ("TEX_SUB_IMAGE_3D", TEX_SUB_IMAGE_3D),
        ("BIND_BUFFER_BASE", BIND_BUFFER_BASE),
        ("DRAW_ARRAYS_INSTANCED", DRAW_ARRAYS_INSTANCED),
        ("DRAW_ELEMENTS_INSTANCED", DRAW_ELEMENTS_INSTANCED),
        ("VERTEX_ATTRIB_NFV", VERTEX_ATTRIB_NFV),
        ("UNIFORM_BLOCK_BINDING", UNIFORM_BLOCK_BINDING),
        ("BIND_ATTRIB_LOCATION", BIND_ATTRIB_LOCATION),
        ("SCISSOR", SCISSOR),
        ("STENCIL_FUNC", STENCIL_FUNC),
        ("STENCIL_OP", STENCIL_OP),
        ("STENCIL_MASK", STENCIL_MASK),
        ("DRAW_BUFFERS", DRAW_BUFFERS),
        ("SAMPLER_CREATE", SAMPLER_CREATE),
        ("SAMPLER_DELETE", SAMPLER_DELETE),
        ("SAMPLER_BIND", SAMPLER_BIND),
        ("SAMPLER_PARAM_I", SAMPLER_PARAM_I),
        ("SAMPLER_PARAM_F", SAMPLER_PARAM_F),
        ("STENCIL_FUNC_SEPARATE", STENCIL_FUNC_SEPARATE),
        ("STENCIL_OP_SEPARATE", STENCIL_OP_SEPARATE),
        ("RENDERBUFFER_STORAGE_MS", RENDERBUFFER_STORAGE_MS),
        ("CLEAR_BUFFER_FV", CLEAR_BUFFER_FV),
        ("CLEAR_BUFFER_IV", CLEAR_BUFFER_IV),
        ("CLEAR_BUFFER_FI", CLEAR_BUFFER_FI),
        ("BIND_BUFFER_RANGE", BIND_BUFFER_RANGE),
        ("BLIT_FRAMEBUFFER", BLIT_FRAMEBUFFER),
    ];

    /// 定长 payload 长度表；`None` = 变长（UNIFORM_NF/NI/MATRIX_4F）。
    pub const PAYLOAD_LEN: [Option<u16>; (MAX_OPCODE as usize) + 1] = build_payload_len();

    const fn build_payload_len() -> [Option<u16>; (MAX_OPCODE as usize) + 1] {
        let mut table = [None; (MAX_OPCODE as usize) + 1];
        // shader / program
        table[SHADER_CREATE as usize] = Some(6);
        table[SHADER_SOURCE as usize] = Some(8);
        table[COMPILE_SHADER as usize] = Some(4);
        table[SHADER_DELETE as usize] = Some(4);
        table[PROGRAM_CREATE as usize] = Some(4);
        table[PROGRAM_ATTACH as usize] = Some(8);
        table[PROGRAM_LINK as usize] = Some(4);
        table[PROGRAM_DELETE as usize] = Some(4);
        table[PROGRAM_USE as usize] = Some(4);
        // buffer
        table[BUFFER_CREATE as usize] = Some(4);
        table[BUFFER_BIND as usize] = Some(6);
        table[BUFFER_DATA as usize] = Some(8);
        table[BUFFER_DATA_NULL as usize] = Some(8);
        table[BUFFER_DELETE as usize] = Some(4);
        // vao / attributes
        table[VAO_CREATE as usize] = Some(4);
        table[VAO_BIND as usize] = Some(4);
        table[VAO_DELETE as usize] = Some(4);
        table[ATTRIB_ENABLE as usize] = Some(4);
        table[ATTRIB_DISABLE as usize] = Some(4);
        table[ATTRIB_POINTER as usize] = Some(16);
        table[ATTRIB_DIVISOR as usize] = Some(8);
        // uniforms
        table[UNIFORM_1F as usize] = Some(8);
        table[UNIFORM_2F as usize] = Some(12);
        table[UNIFORM_3F as usize] = Some(16);
        table[UNIFORM_4F as usize] = Some(20);
        table[UNIFORM_1I as usize] = Some(8);
        // UNIFORM_NF / UNIFORM_NI / UNIFORM_MATRIX_4F: 变长（count 首部）
        // textures
        table[TEXTURE_CREATE as usize] = Some(4);
        table[TEXTURE_DELETE as usize] = Some(4);
        table[TEXTURE_ACTIVE as usize] = Some(1);
        table[TEXTURE_BIND as usize] = Some(6);
        table[TEX_PARAM_I as usize] = Some(8);
        table[TEX_PARAM_F as usize] = Some(8);
        table[TEX_IMAGE_2D as usize] = Some(24);
        table[GENERATE_MIPMAP as usize] = Some(2);
        table[PIXEL_STORE_I as usize] = Some(6);
        // framebuffers
        table[FRAMEBUFFER_CREATE as usize] = Some(4);
        table[FRAMEBUFFER_DELETE as usize] = Some(4);
        table[BIND_FRAMEBUFFER as usize] = Some(6);
        table[FRAMEBUFFER_TEXTURE_2D as usize] = Some(14);
        // state
        table[VIEWPORT as usize] = Some(16);
        table[CLEAR_COLOR as usize] = Some(16);
        table[CLEAR as usize] = Some(1);
        table[ENABLE as usize] = Some(2);
        table[DISABLE as usize] = Some(2);
        table[DEPTH_FUNC as usize] = Some(2);
        table[DEPTH_MASK as usize] = Some(1);
        table[BLEND_FUNC as usize] = Some(4);
        table[BLEND_EQUATION as usize] = Some(2);
        table[BLEND_COLOR as usize] = Some(16);
        table[CULL_FACE as usize] = Some(2);
        table[FRONT_FACE as usize] = Some(2);
        table[CLEAR_DEPTH as usize] = Some(4);
        table[CLEAR_STENCIL as usize] = Some(4);
        table[POLYGON_OFFSET as usize] = Some(8);
        // draw
        table[DRAW_ARRAYS as usize] = Some(10);
        table[DRAW_ELEMENTS as usize] = Some(12);
        // 批次②扩展
        table[BLEND_FUNC_SEPARATE as usize] = Some(8);
        table[BLEND_EQUATION_SEPARATE as usize] = Some(4);
        table[TEX_IMAGE_3D as usize] = Some(28);
        table[TEX_SUB_IMAGE_2D as usize] = Some(28);
        table[RENDERBUFFER_CREATE as usize] = Some(4);
        table[BIND_RENDERBUFFER as usize] = Some(6);
        table[RENDERBUFFER_STORAGE as usize] = Some(16);
        table[FRAMEBUFFER_RENDERBUFFER as usize] = Some(8);
        table[COLOR_MASK as usize] = Some(4);
        table[UNIFORM_2I as usize] = Some(12);
        table[UNIFORM_3I as usize] = Some(16);
        table[UNIFORM_4I as usize] = Some(20);
        table[TEX_STORAGE_2D as usize] = Some(15);
        table[TEX_STORAGE_3D as usize] = Some(19);
        table[TEX_SUB_IMAGE_3D as usize] = Some(36);
        table[BIND_BUFFER_BASE as usize] = Some(10);
        table[DRAW_ARRAYS_INSTANCED as usize] = Some(14);
        table[DRAW_ELEMENTS_INSTANCED as usize] = Some(16);
        table[VERTEX_ATTRIB_NFV as usize] = Some(12);
        table[UNIFORM_BLOCK_BINDING as usize] = Some(12);
        table[BIND_ATTRIB_LOCATION as usize] = Some(12);
        table[SCISSOR as usize] = Some(16);
        table[STENCIL_FUNC as usize] = Some(10);
        table[STENCIL_OP as usize] = Some(6);
        table[STENCIL_MASK as usize] = Some(4);
        // sampler objects（WebGL2）
        table[SAMPLER_CREATE as usize] = Some(4);
        table[SAMPLER_DELETE as usize] = Some(4);
        table[SAMPLER_BIND as usize] = Some(8);
        table[SAMPLER_PARAM_I as usize] = Some(10);
        table[SAMPLER_PARAM_F as usize] = Some(10);
        // WebGL2 状态/绘制补充
        table[STENCIL_FUNC_SEPARATE as usize] = Some(12);
        table[STENCIL_OP_SEPARATE as usize] = Some(8);
        table[RENDERBUFFER_STORAGE_MS as usize] = Some(16);
        table[CLEAR_BUFFER_FV as usize] = Some(22);
        table[CLEAR_BUFFER_IV as usize] = Some(22);
        table[CLEAR_BUFFER_FI as usize] = Some(14);
        table[BIND_BUFFER_RANGE as usize] = Some(18);
        table[BLIT_FRAMEBUFFER as usize] = Some(36);

        table
    }
}

/// WebGPU 命令 opcode 表 —— `webgpu_api.js` 录制面与 wgpu 执行器的权威契约。
///
/// 与 [`gl_opcode`] 同构，但映射方向不同：GL 流是「本来就是调用序列」，这里
/// 是把浏览器真 WebGPU（`navigator.gpu`）的调用序列一对一映射到 wgpu crate。
/// 录制面只桩掉 `GPUAdapter/GPUDevice/GPUCanvasContext` 对象面；g-device-api
/// 的 GLSL→WGSL 编译（realm 内 wasm 真跑）发生在 `createShaderModule` 上游，
/// 所以流里 [`wgpu_opcode::SHADER_MODULE_CREATE`] 携带的已是 WGSL——与 ref 腿
/// （Edge/Dawn 编译的同一份 WGSL）同源。
///
/// ## 描述符约定
///
/// 创建类命令的 descriptor（`GPUBufferDescriptor` 等）以 JSON intern 进
/// `byte_ranges`（serde_json 解析，`glsl_reflect` 先例）。JSON 里的资源句柄
/// 用 `{"h": <id>}` 包装标记（bindGroup 的 buffer/sampler/textureView、
/// pipeline 的 layout/module 等），执行端解析后替换成映射表里的真实资源；
/// 其余字段名与 WebGPU 规格一致（`loadOp: "clear"`、`format: "bgra8unorm"`）。
///
/// ## payload 布局（小端）
///
/// `u32` 句柄 / range id 同 GL；新增 `u64`（BufferAddress）用于 buffer
/// offset/size；`u8` 枚举仅用于可枚举穷尽的短枚举（index format、alpha
/// mode），其余枚举走 JSON 字符串原值。
pub mod wgpu_opcode {
    // ── 资源创建（desc JSON intern）────────────────────────────────
    /// {handle: u32, desc_range: u32} — GPUBufferDescriptor
    pub const BUFFER_CREATE: u8 = 0;
    /// {handle: u32, desc_range: u32} — GPUTextureDescriptor
    pub const TEXTURE_CREATE: u8 = 1;
    /// {handle: u32, texture: u32, desc_range: u32} — GPUTextureViewDescriptor
    pub const TEXTURE_VIEW_CREATE: u8 = 2;
    /// {handle: u32, desc_range: u32} — GPUSamplerDescriptor
    pub const SAMPLER_CREATE: u8 = 3;
    /// {handle: u32, desc_range: u32} — GPUBindGroupLayoutDescriptor
    pub const BIND_GROUP_LAYOUT_CREATE: u8 = 4;
    /// {handle: u32, layout: u32, desc_range: u32} — GPUBindGroupDescriptor
    pub const BIND_GROUP_CREATE: u8 = 5;
    /// {handle: u32, desc_range: u32} — GPUPipelineLayoutDescriptor
    pub const PIPELINE_LAYOUT_CREATE: u8 = 6;
    /// {handle: u32, code_range: u32} — code 是 WGSL UTF-8
    pub const SHADER_MODULE_CREATE: u8 = 7;
    /// {handle: u32, desc_range: u32} — GPURenderPipelineDescriptor
    pub const RENDER_PIPELINE_CREATE: u8 = 8;
    /// {handle: u32, desc_range: u32} — GPUComputePipelineDescriptor
    pub const COMPUTE_PIPELINE_CREATE: u8 = 9;
    /// {handle: u32, desc_range: u32} — GPUQuerySetDescriptor
    pub const QUERY_SET_CREATE: u8 = 10;
    /// {handle: u32, desc_range: u32} — GPURenderBundleEncoderDescriptor
    pub const RENDER_BUNDLE_ENCODER_CREATE: u8 = 11;
    /// {handle: u32} — WebGPU 统一 destroy；类型由执行端映射表得知
    pub const RESOURCE_DESTROY: u8 = 12;

    // ── 画布面（swapchain 语义）────────────────────────────────────
    /// {format_range: u32, alpha_mode: u8, usage: u32} — alphaMode:
    /// 0=opaque 1=premultiplied。执行端把 getCurrentTexture 映射到本流的
    /// 输出纹理（同 GL 的 bindFramebuffer(null)→out_fbo 规则）。
    pub const CANVAS_CONFIGURE: u8 = 13;
    /// {handle: u32} — 返回的 GPUTexture 桩句柄，attachment 引用它
    pub const CANVAS_GET_CURRENT_TEXTURE: u8 = 14;

    // ── queue ──────────────────────────────────────────────────────
    /// {buffer: u32, offset: u64, data_range: u32}
    pub const QUEUE_WRITE_BUFFER: u8 = 15;
    /// {dest_range: u32, data_range: u32, size_range: u32} — dest 是
    /// GPUTextureCopyView JSON（texture: {"h":id}），size 是 GPUExtent3D JSON
    pub const QUEUE_WRITE_TEXTURE: u8 = 16;
    /// {count: u32, encoders: [u32 × count]} — 变长（count 首部）
    pub const QUEUE_SUBMIT: u8 = 17;

    // ── command encoder ────────────────────────────────────────────
    /// {enc: u32, pass: u32, desc_range: u32} — GPURenderPassDescriptor
    pub const ENC_BEGIN_RENDER_PASS: u8 = 18;
    /// {enc: u32, src: u32, src_offset: u64, dst: u32, dst_offset: u64, size: u64}
    pub const ENC_COPY_BUFFER_TO_BUFFER: u8 = 19;
    /// {enc: u32, desc_range: u32} — copyTextureToTexture 三段 JSON
    pub const ENC_COPY_TEXTURE_TO_TEXTURE: u8 = 20;
    /// {enc: u32, desc_range: u32}
    pub const ENC_COPY_TEXTURE_TO_BUFFER: u8 = 21;
    /// {enc: u32, desc_range: u32}
    pub const ENC_COPY_BUFFER_TO_TEXTURE: u8 = 22;
    /// {enc: u32, cmd_buf: u32}
    pub const ENC_FINISH: u8 = 23;

    // ── render pass 热路径（每帧逐 drawcall）────────────────────────
    /// {pass: u32, pipeline: u32}
    pub const PASS_SET_PIPELINE: u8 = 24;
    /// {pass: u32, index: u32, bind_group: u32} — dynamicOffsets 不支持
    /// （g-device-api setBindings 不传；录制面遇到即 throw，fail-loud）
    pub const PASS_SET_BIND_GROUP: u8 = 25;
    /// {pass: u32, slot: u32, buffer: u32, offset: u64}
    pub const PASS_SET_VERTEX_BUFFER: u8 = 26;
    /// {pass: u32, buffer: u32, index_format: u8, offset: u64} — 0=u16 1=u32
    pub const PASS_SET_INDEX_BUFFER: u8 = 27;
    /// {pass: u32, x,y,w,h, min_depth, max_depth: f32 ×6}
    pub const PASS_SET_VIEWPORT: u8 = 28;
    /// {pass: u32, x,y,w,h: u32 ×4}
    pub const PASS_SET_SCISSOR_RECT: u8 = 29;
    /// {pass: u32, r,g,b,a: f32 ×4}
    pub const PASS_SET_BLEND_CONSTANT: u8 = 30;
    /// {pass: u32, reference: u32}
    pub const PASS_SET_STENCIL_REFERENCE: u8 = 31;
    /// {pass: u32, vertices: u32, instances: u32, first_vertex: u32, first_instance: u32}
    pub const PASS_DRAW: u8 = 32;
    /// {pass: u32, indices: u32, instances: u32, first_index: u32, base_vertex: i32, first_instance: u32}
    pub const PASS_DRAW_INDEXED: u8 = 33;
    /// {pass: u32, buffer: u32, offset: u64}
    pub const PASS_DRAW_INDIRECT: u8 = 34;
    /// {pass: u32, buffer: u32, offset: u64}
    pub const PASS_DRAW_INDEXED_INDIRECT: u8 = 35;
    /// {pass: u32}
    pub const PASS_END: u8 = 36;

    // ── compute pass 热路径（与 render pass 共用 SET_PIPELINE /
    // SET_BIND_GROUP / PASS_END；pass 句柄全局唯一，执行端按段类别分派）──
    /// {enc: u32, pass: u32, desc_range: u32} — GPUComputePassDescriptor
    /// （语料只传 label；timestampWrites 录制面即 throw）
    pub const ENC_BEGIN_COMPUTE_PASS: u8 = 37;
    /// {pass: u32, x: u32, y: u32, z: u32}
    pub const PASS_DISPATCH_WORKGROUPS: u8 = 38;
    /// {pass: u32, buffer: u32, offset: u64}
    pub const PASS_DISPATCH_WORKGROUPS_INDIRECT: u8 = 39;

    pub const MAX_OPCODE: u8 = PASS_DISPATCH_WORKGROUPS_INDIRECT;

    /// 定长 payload 长度表；`None` = 变长（QUEUE_SUBMIT）。
    pub const PAYLOAD_LEN: [Option<u16>; (MAX_OPCODE as usize) + 1] = build_payload_len();

    const fn build_payload_len() -> [Option<u16>; (MAX_OPCODE as usize) + 1] {
        let mut table = [None; (MAX_OPCODE as usize) + 1];
        // 资源创建
        table[BUFFER_CREATE as usize] = Some(8);
        table[TEXTURE_CREATE as usize] = Some(8);
        table[TEXTURE_VIEW_CREATE as usize] = Some(12);
        table[SAMPLER_CREATE as usize] = Some(8);
        table[BIND_GROUP_LAYOUT_CREATE as usize] = Some(8);
        // {handle: u32, desc_range: u32} —— 与 SET_BIND_GROUP(12, 含 index) 不同
        table[BIND_GROUP_CREATE as usize] = Some(8);
        table[PIPELINE_LAYOUT_CREATE as usize] = Some(8);
        table[SHADER_MODULE_CREATE as usize] = Some(8);
        table[RENDER_PIPELINE_CREATE as usize] = Some(8);
        table[COMPUTE_PIPELINE_CREATE as usize] = Some(8);
        table[QUERY_SET_CREATE as usize] = Some(8);
        table[RENDER_BUNDLE_ENCODER_CREATE as usize] = Some(8);
        table[RESOURCE_DESTROY as usize] = Some(4);
        // 画布
        table[CANVAS_CONFIGURE as usize] = Some(9);
        table[CANVAS_GET_CURRENT_TEXTURE as usize] = Some(4);
        // queue
        table[QUEUE_WRITE_BUFFER as usize] = Some(16);
        table[QUEUE_WRITE_TEXTURE as usize] = Some(16); // dest+data+size+layout ranges
        // QUEUE_SUBMIT: 变长（count 首部）
        // encoder
        table[ENC_BEGIN_RENDER_PASS as usize] = Some(12);
        table[ENC_COPY_BUFFER_TO_BUFFER as usize] = Some(36);
        table[ENC_COPY_TEXTURE_TO_TEXTURE as usize] = Some(8);
        table[ENC_COPY_TEXTURE_TO_BUFFER as usize] = Some(8);
        table[ENC_COPY_BUFFER_TO_TEXTURE as usize] = Some(8);
        table[ENC_FINISH as usize] = Some(8);
        // render pass
        table[PASS_SET_PIPELINE as usize] = Some(8);
        table[PASS_SET_BIND_GROUP as usize] = Some(12);
        table[PASS_SET_VERTEX_BUFFER as usize] = Some(20); // pass+slot+buffer+u64 offset
        table[PASS_SET_INDEX_BUFFER as usize] = Some(17);
        table[PASS_SET_VIEWPORT as usize] = Some(28);
        table[PASS_SET_SCISSOR_RECT as usize] = Some(20);
        table[PASS_SET_BLEND_CONSTANT as usize] = Some(20);
        table[PASS_SET_STENCIL_REFERENCE as usize] = Some(8);
        table[PASS_DRAW as usize] = Some(20);
        table[PASS_DRAW_INDEXED as usize] = Some(24);
        table[PASS_DRAW_INDIRECT as usize] = Some(16);
        table[PASS_DRAW_INDEXED_INDIRECT as usize] = Some(16);
        table[PASS_END as usize] = Some(4);
        // compute pass
        table[ENC_BEGIN_COMPUTE_PASS as usize] = Some(12);
        table[PASS_DISPATCH_WORKGROUPS as usize] = Some(16);
        table[PASS_DISPATCH_WORKGROUPS_INDIRECT as usize] = Some(16);
        table
    }
}

/// WebGPU 变长命令：QUEUE_SUBMIT（count 首部，之后每元素 u32 encoder 句柄）。
pub fn wgpu_variable_payload_submit(opcode: u8) -> bool {
    matches!(opcode, wgpu_opcode::QUEUE_SUBMIT)
}

/// 变长命令首部（loc/count 等）之后的每元素字节数。
pub fn variable_payload_f32(opcode: u8) -> bool {
    matches!(opcode, gl_opcode::UNIFORM_NF)
}

pub fn variable_payload_i32(opcode: u8) -> bool {
    matches!(opcode, gl_opcode::UNIFORM_NI)
}

pub fn variable_payload_matrix4(opcode: u8) -> bool {
    matches!(opcode, gl_opcode::UNIFORM_MATRIX_4F)
}
