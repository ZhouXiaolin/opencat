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

    pub const MAX_OPCODE: u8 = DRAW_BUFFERS;

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
        table[DRAW_ELEMENTS_INSTANCED as usize] = Some(14);
        table[VERTEX_ATTRIB_NFV as usize] = Some(12);
        table[UNIFORM_BLOCK_BINDING as usize] = Some(12);
        table[BIND_ATTRIB_LOCATION as usize] = Some(12);
        table[SCISSOR as usize] = Some(16);
        table[STENCIL_FUNC as usize] = Some(10);
        table[STENCIL_OP as usize] = Some(6);
        table[STENCIL_MASK as usize] = Some(4);

        table
    }
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
