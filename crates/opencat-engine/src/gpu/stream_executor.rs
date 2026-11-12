//! GPU 命令流执行器：把 core 录制面（`webgl_api.js`）产出的 WebGL2 命令流
//! 一对一映射到 glow（真 GL 调用），结果回填 media plan 的占位条目。
//!
//! WebGL2 命令 ≈ OpenGL ES 调用序列——执行不是翻译，是直调。程序惰性编译
//! 于 `PROGRAM_USE`（three 在 JS 侧已展开 shader 模板，流里就是最终源码）。
//! uniform / attribute 按 (program, name) 寻址：真 location 由驱动 link 后
//! 才定。core 的 `getAttribLocation` 返回**扫描序号**，这里按同一扫描规则
//! （[`scan_declarations`]，必须与 webgl_api.js 的 collectDecls 同序）映射
//! 回真名再取真 location。
//!
//! 内容寻址缓存跨帧：不变纹理/程序不重传、不重编译（core 的 intern 去重
//! 传输，这里去重上载）。
//!
//! 输出：每条流渲进自己的 FBO（RGBA8 纹理）。GPU 合成模式纹理句柄走
//! [`crate::gpu::TextureRegistry`]（Skia 零拷贝采样），CPU 合成模式
//! readback 像素直填占位条目。

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use glow::HasContext;

use opencat_core::ir::gpu_stream::{gl_opcode, GpuSection, GpuStream};
use opencat_core::ir::media_plan::FrameMediaPlan;

const NULL_HANDLE: u32 = 0xFFFF_FFFF;

/// 一条流的命令帧字段（`GpuStream.ops` 的内层结构）。
use opencat_core::ir::GpuOpFrame;

#[derive(Default)]
pub struct GpuStreamExecutor {
    /// 内容 hash → 编译好的程序（跨帧/跨流复用）。
    program_cache: HashMap<String, ProgramEntry>,
    /// (纹理句柄, target, level) → 该槽当前装着的上传内容键：同槽同内容才
    /// 跳过重传。html 通道每帧句柄序归零重放，句柄可能逐帧换主（按时间惰性
    /// 建资源的合成会让后续句柄整体漂移）——"上传过的内容集合"会把旧主的
    /// 内容错当成现状。
    texture_cache: HashMap<(u32, u32, i32), String>,
    /// WebGL `UNPACK_FLIP_Y_WEBGL` 标志（影响下一次 texImage/SubImage2D）。
    /// 桌面 GL 没有这个 pname，翻转由本执行器对上传字节手动做。
    unpack_flip_y: bool,
    /// `GL_UNPACK_ALIGNMENT`（0 视作 GL 初值 4）。未定尺基础格式摊平时
    /// 要按它算源行距。
    unpack_alignment: i32,
    // ── WebGL 句柄 → GL 对象映射（跨流持久）─────────────────────────
    // JS 侧句柄 realm 级单调、GL 对象存活由脚本 delete 决定；持久 renderer
    // 的后续帧流里没有创建命令（three 只在首次构造时 create），映射必须
    // 跨流存活，否则 useProgram/bindBuffer 全部悬空。
    obj_buffers: HashMap<u32, glow::Buffer>,
    obj_vaos: HashMap<u32, glow::VertexArray>,
    obj_textures: HashMap<u32, glow::NativeTexture>,
    obj_fbos: HashMap<u32, glow::NativeFramebuffer>,
    obj_renderbuffers: HashMap<u32, glow::NativeRenderbuffer>,
    /// WebGL2 sampler 对象。采样状态（过滤/包裹/比较）在 WebGL2 里由
    /// sampler 对象承担并覆盖纹理自身参数——g-device-api 每次 drawcall 都
    /// `bindSampler`，糊弄会改像素，必须真建。
    obj_samplers: HashMap<u32, glow::NativeSampler>,
    shader_src: HashMap<u32, Vec<u8>>,
    shader_stage: HashMap<u32, u32>,
    /// pid → (vs handle, fs handle)
    program_shaders: HashMap<u32, (Option<u32>, Option<u32>)>,
    /// pid → link 前的 bindAttribLocation 请求表（按调用序）。真 program
    /// 在 USE 时才惰性编译，编译必须重放这张表，否则驱动自然分配的
    /// location 与调用方的固定常量（@antv 的 TEX=12/OFFSET=13）错位，
    /// vertexAttribPointer 全部绑到无效槽。
    program_attrib_binds: HashMap<u32, Vec<(u32, String)>>,
    /// 流内 pid → 内容 key（USE 时解析出 entry）。
    program_key_by_pid: HashMap<u32, String>,
    /// VAO 的 element 绑定由执行器自己记账、VAO_BIND 时主动重绑。共享
    /// GL 上下文上 VAO 状态会被其他人（Skia 合成、restore、驱动）踩掉，
    /// 而 three 的 bindingStates 缓存使后续帧不再重发绑定——依赖驱动
    /// 保留状态必然炸（drawElements 拿到 index=0 直接访问违例）。
    vao_element: HashMap<u32, u32>,
    /// 本帧流输出 FBO。WebGL `bindFramebuffer(target, null)` 的语义是
    /// 「绑回 canvas 绘制面」——离屏会话里没有 window system 默认帧缓冲，
    /// 必须显式映射到这里。
    stream_out_fbo: u32,
    /// 客户端可见的全局 GL 状态影子（three 的 JS 状态缓存视角）。
    client: ClientGlState,
    /// 按 canvas（流 target_id）分桶的客户端状态影子。浏览器里每个 canvas
    /// 是独立 GL 上下文，状态互不可见；执行器共用一份 GL 上下文，靠这里
    /// 在流边界换入换出。
    clients: HashMap<String, ClientGlState>,
}

/// GL 命令流客户端的"最后一次设置"状态。帧尾必须恢复到这个影子而不是
/// GL 默认——WebGL 客户端（three 的 WebGLState）按客户端视角缓存状态、
/// 只在变化时发命令：真 GL 被清回默认后，后续帧的增量命令全被缓存吞掉
/// （enable(CULL_FACE)/useProgram/bindTexture 只发一次），剔除、深度测试、
/// 程序绑定集体失效。
#[derive(Clone)]
struct ClientGlState {
    caps: Vec<(u32, bool)>,
    front_face: u32,
    cull_face: u32,
    depth_func: u32,
    depth_mask: bool,
    color_mask: [bool; 4],
    blend_equation: (u32, u32),
    blend_func: (u32, u32, u32, u32),
    blend_color: [f32; 4],
    stencil_func: (u32, i32, u32),
    stencil_op: (u32, u32, u32),
    stencil_mask: u32,
    polygon_offset: (f32, f32),
    clear_color: [f32; 4],
    clear_depth: f32,
    clear_stencil: i32,
    active_unit: u32,
    /// (纹理单元, target, 句柄) —— 跨帧被 three 缓存引用的绑定。
    tex_binds: Vec<(u32, u32, u32)>,
    /// (target, 句柄) —— renderbuffer 绑定。
    rb_binds: Vec<(u32, u32)>,
    /// 全局 ARRAY_BUFFER 句柄（element 归 VAO 账本管）。
    array_buffer: Option<u32>,
    /// 当前绑定的 VAO 句柄（None = 无）。
    cur_vao: Option<u32>,
    /// 当前绑定的 element buffer 句柄（None = 无）。
    cur_element: Option<u32>,
    /// 当前使用的程序（内容 key；None = 无）。
    cur_program_key: Option<String>,
    /// 视口 / 裁剪框（None = WebGL 初值，即画布尺寸；流首按流尺寸补）。
    viewport: Option<[i32; 4]>,
    scissor: Option<[i32; 4]>,
}

/// WebGL 的上下文初值 cap 表。影子必须**全量**记这些 cap：执行器把多个
/// canvas 的流跑在同一个 GL 上下文上，换 canvas 时要靠这张表把上一个
/// canvas 遗留的 cap 关掉——空表等于「什么都不关」，全屏 quad 会被别的
/// canvas 留下的 CULL_FACE 整片剔除（实测 vfx-canvas 全黑即此因）。
const DEFAULT_CAPS: [(u32, bool); 9] = [
    (glow::BLEND, false),
    (glow::CULL_FACE, false),
    (glow::DEPTH_TEST, false),
    (glow::DITHER, true),
    (glow::POLYGON_OFFSET_FILL, false),
    (glow::SCISSOR_TEST, false),
    (glow::STENCIL_TEST, false),
    (glow::SAMPLE_ALPHA_TO_COVERAGE, false),
    (glow::SAMPLE_COVERAGE, false),
];

impl Default for ClientGlState {
    fn default() -> Self {
        Self {
            caps: DEFAULT_CAPS.to_vec(),
            front_face: glow::CCW,
            cull_face: glow::BACK,
            depth_func: glow::LESS,
            depth_mask: true,
            color_mask: [true; 4],
            blend_equation: (glow::FUNC_ADD, glow::FUNC_ADD),
            blend_func: (glow::ONE, glow::ZERO, glow::ONE, glow::ZERO),
            blend_color: [0.0; 4],
            stencil_func: (glow::ALWAYS, 0, 0xFFFF_FFFF),
            stencil_op: (glow::KEEP, glow::KEEP, glow::KEEP),
            stencil_mask: 0xFFFF_FFFF,
            polygon_offset: (0.0, 0.0),
            clear_color: [0.0; 4],
            clear_depth: 1.0,
            clear_stencil: 0,
            viewport: None,
            scissor: None,
            active_unit: 0,
            tex_binds: Vec::new(),
            rb_binds: Vec::new(),
            array_buffer: None,
            cur_vao: None,
            cur_element: None,
            cur_program_key: None,
        }
    }
}

impl ClientGlState {
    fn upsert_cap(&mut self, cap: u32, on: bool) {
        if let Some(slot) = self.caps.iter_mut().find(|(c, _)| *c == cap) {
            slot.1 = on;
        } else {
            self.caps.push((cap, on));
        }
    }
}

impl GpuStreamExecutor {
    /// 客户端视角的当前纹理绑定（WebGL 语义，按 unit+target 持久）。
    /// three 的 state.bindTexture 带缓存、命中时不再发 bind——持久
    /// renderer 的后续帧流里可能没有 bind 命令，上传类命令必须从影子
    /// 解析目标纹理，否则数据落到别的绑定上。
    fn bound_texture(&self, target: u32) -> Option<u32> {
        let binding_target = texture_bind_target(target);
        self.client
            .tex_binds
            .iter()
            .rev()
            .find(|(u, t, _)| *u == self.client.active_unit && *t == binding_target)
            .map(|(_, _, h)| *h)
    }

    /// 客户端视角的当前 buffer 绑定（ARRAY 全局；ELEMENT 归 VAO 账本）。
    fn bound_buffer(&self, target: u32) -> Option<u32> {
        if target == glow::ARRAY_BUFFER {
            self.client.array_buffer
        } else {
            self.client.cur_element
        }
    }

    /// 客户端视角的当前 renderbuffer 绑定。
    fn bound_renderbuffer(&self, target: u32) -> Option<u32> {
        self.client
            .rb_binds
            .iter()
            .rev()
            .find(|(t, _)| *t == target)
            .map(|(_, h)| *h)
    }
}

/// 行反转一段像素块（flipY 的手动等价）。行宽按字节数均分——不解释
/// format/type，任何密度的行序都是同一个翻转。
fn flip_rows_vertical(bytes: &mut [u8], height: u32) {
    let h = height as usize;
    if h <= 1 || bytes.is_empty() {
        return;
    }
    let row = bytes.len() / h;
    if row == 0 {
        return;
    }
    let mut tmp = vec![0u8; row];
    for i in 0..h / 2 {
        let top = i * row;
        let bottom = (h - 1 - i) * row;
        tmp.copy_from_slice(&bytes[top..top + row]);
        bytes.copy_within(bottom..bottom + row, top);
        bytes[bottom..bottom + row].copy_from_slice(&tmp);
    }
}

/// 上传命令的 target → 纹理绑定点：cube 六个面是独立的上传 target，但
/// 绑定与参数都挂在 TEXTURE_CUBE_MAP 上（面 target 传给 bind/texParameter
/// 是 INVALID_ENUM）。
fn texture_bind_target(target: u32) -> u32 {
    if (glow::TEXTURE_CUBE_MAP_POSITIVE_X..=glow::TEXTURE_CUBE_MAP_NEGATIVE_Z).contains(&target) {
        glow::TEXTURE_CUBE_MAP
    } else {
        target
    }
}

struct ProgramEntry {
    program: glow::Program,
    /// 顶点源码的属性声明序（core 扫描序号的映射基准）。
    attrib_names: Vec<String>,
    attrib_loc: HashMap<String, Option<u32>>,
    uniform_loc: HashMap<String, Option<glow::UniformLocation>>,
    /// 活跃 uniform 的声明类型（数组同时登记裸名与 `name[0]`）。录制面
    /// 的 uniform{1,2,3,4}fv / iv 同走一条变长命令、不带分量宽度，执行时
    /// 按声明类型选 GL 入口。
    uniform_types: HashMap<String, u32>,
}

impl GpuStreamExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    /// 执行本帧全部 GPU 流并回填 plan。
    ///
    /// `gpu_mode`: true = 输出纹理登记到 `textures`（Skia 零拷贝采样），
    /// false = readback 像素直填 plan（GPU 句柄不产出）。
    pub fn execute(
        &mut self,
        gl: &Arc<glow::Context>,
        section: &GpuSection,
        plan: &mut FrameMediaPlan,
        textures: &crate::gpu::TextureRegistry,
        gpu_mode: bool,
    ) -> Result<Vec<(crate::gpu::NativeTexture, u32)>> {
        let mut outputs = Vec::new();
        if section.streams.is_empty() {
            // 没有任何流 = 本会话 GL 状态没被流碰过。restore 的一堆绑定
            // 调用反而会踩掉 Skia 的状态缓存，直接早退。
            return Ok(outputs);
        }
        for stream in &section.streams {
            // 每个 canvas 在浏览器里是独立 GL 上下文：客户端状态影子按
            // target 分桶。跑流前换入该 canvas 的影子（首次 = WebGL 初值），
            // 跑完换回——否则上一个 canvas 的 cap/绑定会灌进这个 canvas
            // 的流首，全屏 quad 被别人的 CULL_FACE 剔除、输出只剩 clear 色。
            let key = stream.target_id.clone();
            let mut saved = self.clients.remove(&key).unwrap_or_default();
            std::mem::swap(&mut self.client, &mut saved);
            let out = self.execute_stream(gl, stream, plan, textures, gpu_mode);
            let updated = std::mem::replace(&mut self.client, saved);
            self.clients.insert(key, updated);
            let out = out?;
            outputs.extend(out);
        }
        // 无条件恢复:被丢弃的流(占位不可见)同样把状态留在流尾,下一帧的
        // 执行不能从遗留状态开始。恢复到客户端影子(而非 GL 默认)——
        // three 的缓存视角与真 GL 保持一致,后续帧的增量命令不被吞。
        restore_default_state(self, gl);
        Ok(outputs)
    }

    fn execute_stream(
        &mut self,
        gl: &Arc<glow::Context>,
        stream: &GpuStream,
        plan: &mut FrameMediaPlan,
        textures: &crate::gpu::TextureRegistry,
        gpu_mode: bool,
    ) -> Result<Vec<(crate::gpu::NativeTexture, u32)>> {
        let ops = &stream.ops.ops;
        let bytes: &[u8] = &stream.ops.bytes;
        let ranges: &[opencat_core::ir::draw_types::TableRange] = &stream.ops.byte_ranges;

        // 流首状态对齐：GPU 管线下 Skia 合成与流执行共享上下文，上一帧
        // restore 之后的 Skia pass 会改真状态（blend/depth 等）。客户端
        // （three 的 WebGLState）只发增量命令——真状态若停在 Skia 遗留值，
        // 影子外的一切增量都会被打到错误状态上（实测：blend 残留把流输出
        // 的 alpha 通道整帧洗成 1、depth 残留毁掉机身的深度排序）。流首
        // 无条件把真状态拉回客户端影子，增量命令的起点重新可信。
        restore_default_state(self, gl);

        // 输出纹理 + FBO：流画到自己的附件上（合成按占位 z 序采样）。
        let out_tex = crate::gpu::gl::create_texture_rgba8(gl, stream.width, stream.height)
            .ok_or_else(|| anyhow!("gpu stream: 输出纹理创建失败"))?;
        let out_fbo =
            crate::gpu::gl::create_framebuffer(gl, out_tex, stream.width, stream.height)
                .ok_or_else(|| anyhow!("gpu stream: 输出 FBO 创建失败"))?;
        self.stream_out_fbo = out_fbo;

        // 常规帧的流不含 bindFramebuffer（three 的 rt 绑定走缓存不重发）；
        // 执行器把输出 FBO 设为绘制目标，clear/draw 才有落点。realm 构建
        // 帧（PMREM 等多 pass）流内的 bindFramebuffer 由 BIND_FRAMEBUFFER
        // 臂处理：null 映射回 out_fbo。
        unsafe {
            gl.bind_framebuffer(
                glow::FRAMEBUFFER,
                Some(glow::NativeFramebuffer(
                    std::num::NonZeroU32::new(out_fbo).unwrap(),
                )),
            );
        }

        // 视口/裁剪框按客户端影子恢复，缺省 = WebGL 初值（画布尺寸）。WebGL
        // 客户端只在变化时发 viewport（three 的 WebGLState 以
        // getParameter(VIEWPORT) 的初值为起点，裸 WebGL 页常常从不调用），
        // 离屏会话的真视口却停在上下文初值（1×1）或上一个使用者的值。
        let full = [0, 0, stream.width as i32, stream.height as i32];
        let vp = self.client.viewport.unwrap_or(full);
        let sc = self.client.scissor.unwrap_or(full);
        unsafe {
            gl.viewport(vp[0], vp[1], vp[2], vp[3]);
            gl.scissor(sc[0], sc[1], sc[2], sc[3]);
        }

        let trace = std::env::var("OC_TRACE_GL")
            .map(|v| v == stream.target_id)
            .unwrap_or(false);
        if std::env::var_os("OC_DEBUG_CLEAN_STATE").is_some() {
            unsafe {
                gl.disable(glow::CULL_FACE);
                gl.disable(glow::DEPTH_TEST);
                gl.disable(glow::BLEND);
                gl.disable(glow::STENCIL_TEST);
                gl.disable(glow::SCISSOR_TEST);
            }
        }
        let mut r = Replay { gl, ops, bytes, ranges, trace };
        let st = r.run(self)?;
        if std::env::var_os("OC_DETACH_FBO").is_some() {
            unsafe {
                gl.bind_framebuffer(
                    glow::FRAMEBUFFER,
                    Some(glow::NativeFramebuffer(std::num::NonZeroU32::new(out_fbo).unwrap())),
                );
                gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::TEXTURE_2D,
                    None,
                    0,
                );
                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            }
        }
        if std::env::var_os("OC_DEBUG_FILL_RED").is_some() {
            unsafe {
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(glow::NativeFramebuffer(std::num::NonZeroU32::new(out_fbo).unwrap())));
                gl.clear_color(1.0, 0.0, 0.0, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            }
        }

        // 占位不在本帧 plan = 该 canvas 子树本帧不可见（opacity 0 / 裁剪/
        // 移出屏）——流照常执行：three 等持久 renderer 的资源创建命令
        // （program/纹理/VBO）只在首帧录一次，跳过执行会让 GL 侧永远收
        // 不到它们。只丢弃输出，不回填 plan。
        let Some(entry) = plan
            .generated_images
            .iter_mut()
            .find(|e| e.id == stream.image_id)
        else {
            if trace {
                eprintln!(
                    "       [gl] 占位不在 plan: image_id={:?} 现存={:?}",
                    stream.image_id,
                    plan.generated_images
                        .iter()
                        .map(|e| format!("{:?}", e.id))
                        .collect::<Vec<_>>()
                );
            }
            crate::gpu::gl::delete_framebuffer(gl, out_fbo);
            crate::gpu::gl::delete_texture(gl, out_tex);
            let _ = st;
            return Ok(Vec::new());
        };

        let mut outputs = Vec::new();
        let gpu_mode = gpu_mode && std::env::var_os("OC_FORCE_CPU_STREAM").is_none();
        if gpu_mode {
            // 纹理登记到共享表；GL 资源的删除由调用方在合成完成后做
            // （outputs 携带 (纹理名, FBO)——调用点持有 current 上下文）。
            let native = crate::gpu::NativeTexture::from_raw(out_tex.raw());
            let token = textures.publish(native, Box::new(()));
            entry.rgba = Arc::from(Vec::new());
            if std::env::var_os("OC_TRACE_DRAW").is_some() {
                eprintln!(
                    "[stream] 回填 {:?} {}x{} token={}",
                    entry.id,
                    entry.width,
                    entry.height,
                    token.raw()
                );
            }
            entry.gpu = Some(opencat_core::ir::generated_image::GpuImageHandle {
                token: token.raw(),
            });
            outputs.push((native, out_fbo));
            // 调试：dump 流输出纹理字节（与 CPU readback 路径对比用）。
            if let Some(dir) = std::env::var_os("OC_DUMP_STREAM_OUTPUT") {
                let rgba = readback_sync(gl, out_fbo, stream.width, stream.height)?;
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::fs::write(
                    std::path::Path::new(&dir)
                        .join(format!("f{}-{}.rgba", stream.image_id.0, stream.target_id)),
                    &rgba,
                );
            }
        } else {
            let rgba = readback_sync(gl, out_fbo, stream.width, stream.height)?;
            // 调试：与 GPU 管线同格式 dump，供两端字节级对比。
            if let Some(dir) = std::env::var_os("OC_DUMP_STREAM_OUTPUT") {
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::fs::write(
                    std::path::Path::new(&dir)
                        .join(format!("f{}-{}.rgba", stream.image_id.0, stream.target_id)),
                    &rgba,
                );
            }
            entry.rgba = Arc::from(rgba);
            entry.gpu = None;
            crate::gpu::gl::delete_framebuffer(gl, out_fbo);
            crate::gpu::gl::delete_texture(gl, out_tex);
        }
        let _ = st;
        Ok(outputs)
    }
}

/// 流执行结束:把共享 GL 上下文恢复到近默认态。
///
/// replay 把绑定面(program/VAO/buffer/纹理/FBO)留在流的目标上;Skia 与
/// 我们共享同一 HGLRC,`resetContext(kAll)` 只失效 Skia 的缓存让它们重新
/// 查询,并不改这些真实状态——flush 时驱动在遗留绑定上执行 Skia 的命令,
/// 实测(Windows/NVIDIA)表现为访问违例。逐项还原到 GL 默认值。
/// WebGL2 的 buffer target 枚举 → 本上下文可用的桌面值。
/// g-device-api 的 setSubData 全走 WebGL 的 COPY_WRITE_BUFFER(0x8F37)：
/// 直传被驱动按无效枚举拒掉（实测 err=0x500），数据落空，SDF 文字因此
/// 错位；本执行器的 WGL legacy 上下文也不收桌面 4.4 的 0x94FA。退回
/// ARRAY_BUFFER 上传——驱动语义自洽（pointer 前总是重新 bind），客户端
/// 账本仍按原 target 记账，不会被上传踩掉。
fn desktop_buffer_target(target: u32) -> u32 {
    match target {
        0x8F36 | 0x8F37 => glow::ARRAY_BUFFER, // COPY_READ/WRITE_BUFFER
        v => v,
    }
}

impl GpuStreamExecutor {
    pub fn debug_counts(&self) -> String {
        format!(
            "prog={} texc={} buf={} vao={} tex={} fbo={} rb={} smp={} shsrc={}",
            self.program_cache.len(),
            self.texture_cache.len(),
            self.obj_buffers.len(),
            self.obj_vaos.len(),
            self.obj_textures.len(),
            self.obj_fbos.len(),
            self.obj_renderbuffers.len(),
            self.obj_samplers.len(),
            self.shader_src.len()
        )
    }
}

fn restore_default_state(ex: &mut GpuStreamExecutor, gl: &Arc<glow::Context>) {
    unsafe {
        // 绑定类恢复到客户端最后看到的对象(three 的缓存只在变化时重发);
        // 句柄已删的回落 None 并同步账本。VAO 先绑回——ELEMENT 绑定和
        // attrib 状态都属于 VAO,绑回后按账本重挂 element。
        let vao = ex.client.cur_vao.and_then(|h| ex.obj_vaos.get(&h).copied());
        gl.bind_vertex_array(vao);
        if vao.is_none() {
            ex.client.cur_vao = None;
            ex.client.cur_element = None;
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
        } else {
            let eh = ex.client.cur_vao.and_then(|v| ex.vao_element.get(&v).copied());
            ex.client.cur_element = eh;
            if let Some(h) = eh && let Some(buf) = ex.obj_buffers.get(&h) {
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(*buf));
            }
        }
        let program = ex
            .client
            .cur_program_key
            .as_ref()
            .and_then(|k| ex.program_cache.get(k));
        if program.is_none() {
            ex.client.cur_program_key = None;
        }
        gl.use_program(program.map(|e| e.program));
        let ab = ex
            .client
            .array_buffer
            .and_then(|h| ex.obj_buffers.get(&h).copied());
        if ab.is_none() {
            ex.client.array_buffer = None;
        }
        gl.bind_buffer(glow::ARRAY_BUFFER, ab);
        // 纹理:先全清,再按影子恢复跨帧引用的绑定。
        for unit in 0u32..8 {
            gl.active_texture(glow::TEXTURE0 + unit);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_sampler(glow::TEXTURE0 + unit, None);
        }
        ex.client.tex_binds.retain(|(_, _, h)| ex.obj_textures.contains_key(h));
        for (unit, target, h) in &ex.client.tex_binds {
            if *unit < 8 && let Some(tex) = ex.obj_textures.get(h) {
                gl.active_texture(glow::TEXTURE0 + unit);
                gl.bind_texture(*target, Some(*tex));
            }
        }
        gl.active_texture(glow::TEXTURE0 + ex.client.active_unit);
        // FBO/renderbuffer 不恢复绑定:协议规定每条流开画前由执行器自绑
        // 输出 FBO,客户端(three)对 render target 的绑定在流内自恢复。
        gl.bind_renderbuffer(glow::RENDERBUFFER, None);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
        // PMREM/WebGL2 的 MRT 会把 draw buffer 组合改掉;状态残留会让
        // Skia 的合成 pass 落到不存在的 attachment 上。
        gl.draw_buffers(&[glow::BACK]);
        // 全局值状态:恢复客户端影子。
        for (cap, on) in &ex.client.caps {
            if *on {
                gl.enable(*cap);
            } else {
                gl.disable(*cap);
            }
        }
        gl.front_face(ex.client.front_face);
        gl.cull_face(ex.client.cull_face);
        gl.depth_func(ex.client.depth_func);
        gl.depth_mask(ex.client.depth_mask);
        let cm = ex.client.color_mask;
        gl.color_mask(cm[0], cm[1], cm[2], cm[3]);
        let be = ex.client.blend_equation;
        gl.blend_equation_separate(be.0, be.1);
        let bf = ex.client.blend_func;
        gl.blend_func_separate(bf.0, bf.1, bf.2, bf.3);
        let bc = ex.client.blend_color;
        gl.blend_color(bc[0], bc[1], bc[2], bc[3]);
        let sf = ex.client.stencil_func;
        gl.stencil_func(sf.0, sf.1, sf.2);
        let so = ex.client.stencil_op;
        gl.stencil_op(so.0, so.1, so.2);
        gl.stencil_mask(ex.client.stencil_mask);
        let po = ex.client.polygon_offset;
        gl.polygon_offset(po.0, po.1);
        let cc = ex.client.clear_color;
        gl.clear_color(cc[0], cc[1], cc[2], cc[3]);
        gl.clear_depth_f32(ex.client.clear_depth);
        gl.clear_stencil(ex.client.clear_stencil);
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
        gl.pixel_store_i32(glow::PACK_ALIGNMENT, 4);
    }
}

fn readback_sync(gl: &Arc<glow::Context>, fbo: u32, width: u32, height: u32) -> Result<Vec<u8>> {
    use std::num::NonZeroU32;
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    unsafe {
        gl.bind_framebuffer(
            glow::READ_FRAMEBUFFER,
            Some(glow::NativeFramebuffer(NonZeroU32::new(fbo).unwrap())),
        );
        gl.read_pixels(
            0,
            0,
            width as i32,
            height as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(&mut rgba),
        );
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
    }
    // GL 底部原点 → 合成顶部原点：逐行翻转。
    let row = width as usize * 4;
    let mut out = vec![0u8; rgba.len()];
    for (y, dst) in out.chunks_mut(row).enumerate() {
        let src = (height as usize - 1 - y) * row;
        dst.copy_from_slice(&rgba[src..src + row]);
    }
    Ok(out)
}

// ── 命令重放 ──────────────────────────────────────────────────────────

struct Replay<'a> {
    gl: &'a Arc<glow::Context>,
    ops: &'a [u8],
    bytes: &'a [u8],
    ranges: &'a [opencat_core::ir::draw_types::TableRange],
    /// 调试：`OC_TRACE_GL=<target_id>` 时打印本流每条命令。
    trace: bool,
}

struct ReplayState {
    program: Option<glow::Program>,
    program_key: Option<String>,
}

impl<'a> Replay<'a> {
    fn run(&mut self, ex: &mut GpuStreamExecutor) -> Result<ReplayState> {
        let gl = self.gl;
        // 句柄→GL 对象与 shader/program 元数据表都在 executor 上跨流持久
        // （持久 renderer 的后续帧流没有创建命令）；state 只留帧内状态机。
        //
        // 但 program 是 GL 全局状态，跨帧保持——浏览器里 uniform 落在
        // 当前 program 上，three 的流是增量的（程序没变就不重发
        // useProgram）。帧内状态机必须继承客户端影子的当前 program，
        // 否则流首的 uniform 找不到程序（"uniform 命令出现在
        // USE_PROGRAM 之前"）。restore_default_state 已把真 GL 绑回同一
        // 程序，继承与真状态一致。
        let mut state = ReplayState {
            program: ex
                .client
                .cur_program_key
                .as_ref()
                .and_then(|k| ex.program_cache.get(k))
                .map(|e| e.program),
            program_key: ex.client.cur_program_key.clone(),
        };

        let trace = self.trace;
        let mut p = 0usize;
        unsafe {
            while p < self.ops.len() {
                let opcode = self.ops[p];
                let start = p;
                p += 1;
                if trace {
                    let name = gl_opcode::NAMES
                        .iter()
                        .find(|(_, v)| *v == opcode)
                        .map(|(n, _)| *n)
                        .unwrap_or("UNKNOWN");
                    eprintln!("[gl] {start:>5} {name}");
                }
                match opcode {
                    gl_opcode::SHADER_CREATE => {
                        let ty = rd_u16(self.ops, &mut p) as u32;
                        let id = rd_u32(self.ops, &mut p);
                        ex.shader_stage.insert(id, ty);
                    }
                    gl_opcode::SHADER_SOURCE => {
                        let id = rd_u32(self.ops, &mut p);
                        let range = rd_u32(self.ops, &mut p);
                        let src = self.range_bytes(range)?.to_vec();
                        ex.shader_src.insert(id, src);
                    }
                    gl_opcode::COMPILE_SHADER => {
                        rd_u32(self.ops, &mut p);
                    }
                    gl_opcode::PROGRAM_CREATE => {
                        // html 通道每帧句柄序归零重放：同一 pid 下一帧可能是
                        // 另一份程序（惰性建 program 的合成会让后续句柄整体
                        // 漂移）。新建即清旧映射；编译产物仍按源码键复用。
                        let pid = rd_u32(self.ops, &mut p);
                        ex.program_shaders.remove(&pid);
                        ex.program_key_by_pid.remove(&pid);
                        ex.program_attrib_binds.remove(&pid);
                    }
                    gl_opcode::PROGRAM_ATTACH => {
                        let pid = rd_u32(self.ops, &mut p);
                        let sid = rd_u32(self.ops, &mut p);
                        let slot = ex.program_shaders.entry(pid).or_default();
                        if ex.shader_stage.get(&sid) == Some(&0x8B31) {
                            slot.0 = Some(sid);
                        } else {
                            slot.1 = Some(sid);
                        }
                    }
                    gl_opcode::PROGRAM_LINK => {
                        rd_u32(self.ops, &mut p);
                    }
                    gl_opcode::PROGRAM_USE => {
                        let pid = rd_u32(self.ops, &mut p);
                        if pid == NULL_HANDLE {
                            state.program = None;
                            state.program_key = None;
                            ex.client.cur_program_key = None;
                            gl.use_program(None);
                        } else {
                            let key = ex.program_key_for(gl, pid)?;
                            let entry = ex
                                .program_cache
                                .get(&key)
                                .ok_or_else(|| anyhow!("program {pid} 缓存缺失"))?;
                            state.program = Some(entry.program);
                            state.program_key = Some(key.clone());
                            ex.client.cur_program_key = Some(key);
                            // 状态机之外必须真绑定：漏了它 draw 落到固定
                            // 管线（compat profile 不报错、画默认色白三角）。
                            gl.use_program(Some(entry.program));
                        }
                    }
                    gl_opcode::SHADER_DELETE => {
                        // WebGL 语义:attach 后 delete 只标记删除,program
                        // 仍持引用(three 构造完就删)。源码/阶段表是惰性
                        // 编译的元数据,跟随 program 生命周期,不随删。
                        let _ = rd_u32(self.ops, &mut p);
                    }
                    gl_opcode::PROGRAM_DELETE => {
                        let pid = rd_u32(self.ops, &mut p);
                        ex.program_shaders.remove(&pid);
                        ex.program_key_by_pid.remove(&pid);
                        ex.program_attrib_binds.remove(&pid);
                    }
                    // 删除即真删 GL 对象：html 通道每帧重放会删了再建（PMREM 之类
                    // 用完即 dispose），只摘映射不删对象会逐帧泄漏整份存储。
                    gl_opcode::BUFFER_DELETE => {
                        let h = rd_u32(self.ops, &mut p);
                        if let Some(buf) = ex.obj_buffers.remove(&h) {
                            gl.delete_buffer(buf);
                        }
                    }
                    gl_opcode::VAO_DELETE => {
                        let h = rd_u32(self.ops, &mut p);
                        if let Some(vao) = ex.obj_vaos.remove(&h) {
                            gl.delete_vertex_array(vao);
                        }
                        if ex.client.cur_vao == Some(h) {
                            ex.client.cur_vao = None;
                            ex.client.cur_element = None;
                        }
                        ex.vao_element.remove(&h);
                    }
                    gl_opcode::TEXTURE_DELETE => {
                        let h = rd_u32(self.ops, &mut p);
                        if let Some(tex) = ex.obj_textures.remove(&h) {
                            gl.delete_texture(tex);
                        }
                        ex.texture_cache.retain(|(th, _, _), _| *th != h);
                        ex.client.tex_binds.retain(|(_, _, th)| *th != h);
                    }
                    gl_opcode::FRAMEBUFFER_DELETE => {
                        let h = rd_u32(self.ops, &mut p);
                        if let Some(fbo) = ex.obj_fbos.remove(&h) {
                            gl.delete_framebuffer(fbo);
                        }
                    }
                    gl_opcode::BUFFER_CREATE => {
                        let handle = rd_u32(self.ops, &mut p);
                        ex.obj_buffers
                            .entry(handle)
                            .or_insert_with(|| gl.create_buffer().expect("create_buffer"));
                    }
                    gl_opcode::BUFFER_BIND => {
                        let target = rd_u16(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let dt = desktop_buffer_target(target as u32);
                        let buf = if handle == NULL_HANDLE {
                            ex.client.cur_element = None;
                            None
                        } else {
                            ex.client.cur_element = Some(handle);
                            if target as u32 == glow::ELEMENT_ARRAY_BUFFER
                                && let Some(v) = ex.client.cur_vao
                            {
                                ex.vao_element.insert(v, handle);
                            }
                            Some(*ex.obj_buffers.entry(handle).or_insert_with(|| {
                                gl.create_buffer().expect("create_buffer")
                            }))
                        };
                        if target as u32 == glow::ARRAY_BUFFER {
                            ex.client.array_buffer =
                                (handle != NULL_HANDLE).then_some(handle);
                        }
                        gl.bind_buffer(dt, buf);
                    }
                    gl_opcode::BUFFER_DATA => {
                        let target = rd_u16(self.ops, &mut p);
                        let usage = rd_u16(self.ops, &mut p);
                        let range = rd_u32(self.ops, &mut p);
                        let data = self.range_bytes(range)?;
                        if trace {
                            let head: Vec<f32> = data
                                .chunks_exact(4)
                                .take(6)
                                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                                .collect();
                            eprintln!("       bufferData target={target} len={} head={head:?}", data.len());
                        }
                        let buf = ex
                            .bound_buffer(target as u32)
                            .and_then(|h| ex.obj_buffers.get(&h).copied())
                            .ok_or_else(|| anyhow!("BUFFER_DATA 无客户端绑定 buffer"))?;
                        // g-device-api 全部 setSubData 走 WebGL 的
                        // COPY_WRITE_BUFFER(0x8F37)：枚举直传桌面 GL 是
                        // INVALID_ENUM，buffer 全空——SDF 文字因此错位。
                        gl.bind_buffer(desktop_buffer_target(target as u32), Some(buf));
                        gl.buffer_data_u8_slice(desktop_buffer_target(target as u32), data, usage as u32);
                    }
                    gl_opcode::BUFFER_DATA_NULL => {
                        let target = rd_u16(self.ops, &mut p);
                        let usage = rd_u16(self.ops, &mut p);
                        let size = rd_u32(self.ops, &mut p);
                        let buf = ex
                            .bound_buffer(target as u32)
                            .and_then(|h| ex.obj_buffers.get(&h).copied())
                            .ok_or_else(|| anyhow!("BUFFER_DATA_NULL 无客户端绑定 buffer"))?;
                        gl.bind_buffer(desktop_buffer_target(target as u32), Some(buf));
                        gl.buffer_data_size(desktop_buffer_target(target as u32), size as i32, usage as u32);
                    }
                    gl_opcode::VAO_CREATE => {
                        let handle = rd_u32(self.ops, &mut p);
                        ex.obj_vaos.entry(handle).or_insert_with(|| {
                            gl.create_vertex_array().expect("create_vertex_array")
                        });
                    }
                    gl_opcode::VAO_BIND => {
                        let handle = rd_u32(self.ops, &mut p);
                        let vao = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*ex.obj_vaos.entry(handle).or_insert_with(|| {
                                gl.create_vertex_array().expect("create_vertex_array")
                            }))
                        };
                        ex.client.cur_vao = if handle == NULL_HANDLE { None } else { Some(handle) };
                        gl.bind_vertex_array(vao);
                        // 绑定后按账本重挂 element：不信任驱动保留的 VAO
                        // 状态（会被共享上下文的其他使用者踩掉）。
                        if let Some(eh) = ex.vao_element.get(&handle).copied() {
                            if let Some(buf) = ex.obj_buffers.get(&eh) {
                                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(*buf));
                            }
                        }
                    }
                    gl_opcode::ATTRIB_ENABLE => {
                        let name_range = rd_u32(self.ops, &mut p);
                        let loc = self.attrib_loc(ex, &state, name_range)?;
                        if trace {
                            let name = self.range_str(name_range).unwrap_or("?");
                            eprintln!("       attribEnable {name} loc={loc:?}");
                        }
                        if let Some(loc) = loc {
                            gl.enable_vertex_attrib_array(loc);
                        }
                    }
                    gl_opcode::ATTRIB_DISABLE => {
                        let name_range = rd_u32(self.ops, &mut p);
                        if let Some(loc) = self.attrib_loc(ex, &state, name_range)? {
                            gl.disable_vertex_attrib_array(loc);
                        }
                    }
                    gl_opcode::ATTRIB_POINTER => {
                        let name_range = rd_u32(self.ops, &mut p);
                        let size = rd_u8(self.ops, &mut p);
                        let ty = rd_u16(self.ops, &mut p);
                        let normalized = rd_u8(self.ops, &mut p);
                        let stride = rd_u32(self.ops, &mut p);
                        let offset = rd_u32(self.ops, &mut p);
                        let loc = self.attrib_loc(ex, &state, name_range)?;
                        if trace {
                            let name = self.range_str(name_range).unwrap_or("?");
                            eprintln!(
                                "       attribPointer {name} loc={loc:?} size={size} ty={ty} stride={stride} off={offset}"
                            );
                        }
                        if let Some(loc) = loc {
                            gl.vertex_attrib_pointer_f32(
                                loc,
                                size as i32,
                                ty as u32,
                                normalized != 0,
                                stride as i32,
                                offset as i32,
                            );
                        }
                    }
                    gl_opcode::ATTRIB_DIVISOR => {
                        let name_range = rd_u32(self.ops, &mut p);
                        let divisor = rd_u32(self.ops, &mut p);
                        if let Some(loc) = self.attrib_loc(ex, &state, name_range)? {
                            gl.vertex_attrib_divisor(loc, divisor);
                        }
                    }
                    gl_opcode::UNIFORM_1F => {
                        let range = rd_u32(self.ops, &mut p);
                        let x = rd_f32(self.ops, &mut p);
                        if trace {
                            let name = self.range_str(range).unwrap_or("?");
                            eprintln!("       uniform1f {name} = {x}");
                        }
                        gl.uniform_1_f32(self.uniform_loc(ex, &state, range)?.as_ref(), x);
                    }
                    gl_opcode::UNIFORM_2F => {
                        let range = rd_u32(self.ops, &mut p);
                        let x = rd_f32(self.ops, &mut p);
                        let y = rd_f32(self.ops, &mut p);
                        gl.uniform_2_f32(self.uniform_loc(ex, &state, range)?.as_ref(), x, y);
                    }
                    gl_opcode::UNIFORM_3F => {
                        let range = rd_u32(self.ops, &mut p);
                        let x = rd_f32(self.ops, &mut p);
                        let y = rd_f32(self.ops, &mut p);
                        let z = rd_f32(self.ops, &mut p);
                        gl.uniform_3_f32(self.uniform_loc(ex, &state, range)?.as_ref(), x, y, z);
                    }
                    gl_opcode::UNIFORM_4F => {
                        let range = rd_u32(self.ops, &mut p);
                        let x = rd_f32(self.ops, &mut p);
                        let y = rd_f32(self.ops, &mut p);
                        let z = rd_f32(self.ops, &mut p);
                        let w = rd_f32(self.ops, &mut p);
                        gl.uniform_4_f32(self.uniform_loc(ex, &state, range)?.as_ref(), x, y, z, w);
                    }
                    gl_opcode::UNIFORM_1I => {
                        let range = rd_u32(self.ops, &mut p);
                        let v = rd_i32(self.ops, &mut p);
                        gl.uniform_1_i32(self.uniform_loc(ex, &state, range)?.as_ref(), v);
                    }
                    gl_opcode::UNIFORM_NF => {
                        let range = rd_u32(self.ops, &mut p);
                        let count = rd_u32(self.ops, &mut p);
                        let data = self.tail_f32(&mut p, count)?;
                        let loc = self.uniform_loc(ex, &state, range)?;
                        match self.uniform_type(ex, &state, range)? {
                            Some(glow::FLOAT_VEC2) => gl.uniform_2_f32_slice(loc.as_ref(), &data),
                            Some(glow::FLOAT_VEC3) => gl.uniform_3_f32_slice(loc.as_ref(), &data),
                            Some(glow::FLOAT_VEC4) => gl.uniform_4_f32_slice(loc.as_ref(), &data),
                            _ => gl.uniform_1_f32_slice(loc.as_ref(), &data),
                        }
                    }
                    gl_opcode::UNIFORM_NI => {
                        let range = rd_u32(self.ops, &mut p);
                        let count = rd_u32(self.ops, &mut p);
                        let data = self.tail_i32(&mut p, count)?;
                        let loc = self.uniform_loc(ex, &state, range)?;
                        match self.uniform_type(ex, &state, range)? {
                            Some(glow::INT_VEC2 | glow::BOOL_VEC2) => gl.uniform_2_i32_slice(loc.as_ref(), &data),
                            Some(glow::INT_VEC3 | glow::BOOL_VEC3) => gl.uniform_3_i32_slice(loc.as_ref(), &data),
                            Some(glow::INT_VEC4 | glow::BOOL_VEC4) => gl.uniform_4_i32_slice(loc.as_ref(), &data),
                            _ => gl.uniform_1_i32_slice(loc.as_ref(), &data),
                        }
                    }
                    gl_opcode::UNIFORM_MATRIX_4F => {
                        let range = rd_u32(self.ops, &mut p);
                        let count = rd_u32(self.ops, &mut p);
                        let order = rd_u8(self.ops, &mut p);
                        let n = (count * order as u32 * order as u32) as usize;
                        let data = self.tail_f32(&mut p, n as u32)?;
                        let loc = self.uniform_loc(ex, &state, range)?;
                        match order {
                            2 => gl.uniform_matrix_2_f32_slice(loc.as_ref(), false, &data),
                            3 => gl.uniform_matrix_3_f32_slice(loc.as_ref(), false, &data),
                            _ => gl.uniform_matrix_4_f32_slice(loc.as_ref(), false, &data),
                        }
                    }
                    gl_opcode::TEXTURE_CREATE => {
                        let handle = rd_u32(self.ops, &mut p);
                        ex.obj_textures.entry(handle).or_insert_with(|| {
                            gl.create_texture().expect("create_texture")
                        });
                    }
                    gl_opcode::TEXTURE_ACTIVE => {
                        let unit = rd_u8(self.ops, &mut p);
                        ex.client.active_unit = unit as u32;
                        gl.active_texture(glow::TEXTURE0 + unit as u32);
                    }
                    gl_opcode::TEXTURE_BIND => {
                        let target = rd_u16(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let tex = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*ex.obj_textures.entry(handle).or_insert_with(|| {
                                gl.create_texture().expect("create_texture")
                            }))
                        };
                        let bind = (ex.client.active_unit, target as u32, handle);
                        ex.client.tex_binds.retain(|b| {
                            b.0 != bind.0 || b.1 != bind.1
                        });
                        if handle != NULL_HANDLE {
                            ex.client.tex_binds.push(bind);
                        }
                        gl.bind_texture(target as u32, tex);
                    }
                    gl_opcode::TEX_PARAM_I => {
                        let target = rd_u16(self.ops, &mut p);
                        let pname = rd_u16(self.ops, &mut p);
                        let value = rd_i32(self.ops, &mut p);
                        gl.tex_parameter_i32(target as u32, pname as u32, value);
                    }
                    gl_opcode::TEX_PARAM_F => {
                        let target = rd_u16(self.ops, &mut p);
                        let pname = rd_u16(self.ops, &mut p);
                        let value = rd_f32(self.ops, &mut p);
                        gl.tex_parameter_f32(target as u32, pname as u32, value);
                    }
                    gl_opcode::TEX_IMAGE_2D => {
                        let target = rd_u16(self.ops, &mut p);
                        let level = rd_u8(self.ops, &mut p) as i32;
                        let internal = rd_i32(self.ops, &mut p);
                        let w = rd_u32(self.ops, &mut p);
                        let h = rd_u32(self.ops, &mut p);
                        let format = rd_u16(self.ops, &mut p);
                        let ty = rd_u16(self.ops, &mut p);
                        let has_data = rd_u8(self.ops, &mut p);
                        let range = rd_u32(self.ops, &mut p);
                        if trace {
                            eprintln!("       texImage2D {w}x{h} has_data={has_data} range={range}");
                        }
                        let mut data = if has_data != 0 {
                            Some(self.range_bytes(range)?.to_vec())
                        } else {
                            None
                        };
                        let tex = ex
                            .bound_texture(target as u32)
                            .and_then(|th| ex.obj_textures.get(&th).copied())
                            .ok_or_else(|| anyhow!("TEX_IMAGE_2D 无客户端绑定纹理"))?;
                        if ex.unpack_flip_y {
                            if let Some(bytes) = data.as_mut() {
                                flip_rows_vertical(bytes, h);
                            }
                        }
                        // WebGL2 允许 ALPHA / LUMINANCE / LUMINANCE_ALPHA /
                        // RGB 这类未定尺基础格式当 internalformat，桌面 GL
                        // 3.3 core 已移除——摊平成 RGBA8 并改写像素，语义等价。
                        let mut internal = internal;
                        let mut format = format;
                        let mut expanded = expand_unsized_upload(
                            internal,
                            format as u32,
                            ty as u32,
                            data.as_deref(),
                            w,
                            h,
                            ex.unpack_alignment.max(0) as usize,
                        );
                        if let Some((nint, nfmt, bytes)) = expanded.take() {
                            internal = nint;
                            format = nfmt as u16;
                            data = Some(bytes);
                        }
                        // 上传去重必须按纹理对象——同内容的不同纹理对象
                        // （如多 canvas 各自的 SDF 图集）各自需要真实上传，
                        // 只按内容 hash 去重会把后续纹理留成空纹理。
                        let tex_handle = ex.bound_texture(target as u32).unwrap_or(0);
                        let slot = (tex_handle, target as u32, level);
                        let content_key = format!(
                            "{internal}|{w}x{h}|{format}|{ty}|{}",
                            hash_bytes(data.as_deref().unwrap_or(&[])),
                        );
                        if ex.texture_cache.get(&slot) != Some(&content_key) {
                            gl.bind_texture(texture_bind_target(target as u32), Some(tex));
                            gl.tex_image_2d(
                                target as u32, level, internal, w as i32, h as i32, 0,
                                format as u32, ty as u32, data.as_deref(),
                            );
                            // ANGLE（Chrome）把单 level 纹理的 mip 选择钳回
                            // base level（D3D SRV 的 mip 范围语义）；桌面 GL
                            // 无此钳制——mip 模式 sampler 对缺失 level 实测
                            // 采出 (0,0,0,1)，SDF 文字因此成实心块。
                            if level == 0 {
                                gl.tex_parameter_i32(texture_bind_target(target as u32), glow::TEXTURE_MAX_LEVEL, 0);
                            }
                            ex.texture_cache.insert(slot, content_key);
                        }
                    }
                    gl_opcode::GENERATE_MIPMAP => {
                        let target = rd_u16(self.ops, &mut p);
                        gl.generate_mipmap(target as u32);
                    }
                    gl_opcode::PIXEL_STORE_I => {
                        let pname = rd_u16(self.ops, &mut p);
                        let value = rd_i32(self.ops, &mut p);
                        // WebGL 专属 pixelStore 状态没有桌面 GL 平价:
                        // FLIP_Y 由本执行器在下一次纹理上传时手动行反转,
                        // premultiply/colorspace 无对应语义且 three 每次
                        // 上传前都会重设,吞掉;UNPACK_ALIGNMENT 桌面合法,
                        // 直通。
                        match pname as u32 {
                            0x9240 => ex.unpack_flip_y = value != 0,
                            0x9241 | 0x9243 => {}
                            0x0CF5 => {
                                // UNPACK_ALIGNMENT：直通给桌面 GL，同时记下
                                // 供未定尺格式摊平算源行距。
                                ex.unpack_alignment = value;
                                gl.pixel_store_i32(pname as u32, value);
                            }
                            _ => gl.pixel_store_i32(pname as u32, value),
                        }
                    }
                    gl_opcode::DRAW_BUFFERS => {
                        let n = rd_u8(self.ops, &mut p) as usize;
                        let mut modes = [0u32; 8];
                        // WebGL 默认帧缓冲的 BACK 在这里是流输出 FBO 的
                        // COLOR_ATTACHMENT0（FBO 上传 BACK 是 INVALID_OPERATION）。
                        for m in modes.iter_mut().take(n) {
                            *m = match rd_u32(self.ops, &mut p) {
                                glow::BACK => glow::COLOR_ATTACHMENT0,
                                v => v,
                            };
                        }
                        gl.draw_buffers(&modes[..n]);
                    }
                    gl_opcode::FRAMEBUFFER_CREATE => {
                        let handle = rd_u32(self.ops, &mut p);
                        ex.obj_fbos.entry(handle).or_insert_with(|| {
                            gl.create_framebuffer().expect("create_framebuffer")
                        });
                    }
                    gl_opcode::BIND_FRAMEBUFFER => {
                        let target = rd_u16(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let fbo = if handle == NULL_HANDLE {
                            // WebGL null = canvas 绘制面 = 本帧流输出 FBO。
                            // 绑 None 是桌面 GL 的 window system 帧缓冲，
                            // 离屏会话里不存在——主 pass 的绘制会整体丢失。
                            Some(glow::NativeFramebuffer(
                                std::num::NonZeroU32::new(ex.stream_out_fbo).unwrap(),
                            ))
                        } else {
                            Some(*ex.obj_fbos.entry(handle).or_insert_with(|| {
                                gl.create_framebuffer().expect("create_framebuffer")
                            }))
                        };
                        gl.bind_framebuffer(target as u32, fbo);
                    }
                    gl_opcode::FRAMEBUFFER_TEXTURE_2D => {
                        let target = rd_u16(self.ops, &mut p);
                        let attachment = rd_u16(self.ops, &mut p);
                        let textarget = rd_u16(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let level = rd_i32(self.ops, &mut p);
                        let tex = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*ex.obj_textures
                                .get(&handle)
                                .ok_or_else(|| anyhow!("FBO 附着未知纹理 {handle}"))?)
                        };
                        gl.framebuffer_texture_2d(
                            target as u32, attachment as u32, textarget as u32, tex, level,
                        );
                    }
                    gl_opcode::VIEWPORT => {
                        let x = rd_i32(self.ops, &mut p);
                        let y = rd_i32(self.ops, &mut p);
                        let w = rd_i32(self.ops, &mut p);
                        let h = rd_i32(self.ops, &mut p);
                        ex.client.viewport = Some([x, y, w, h]);
                        gl.viewport(x, y, w, h);
                    }
                    gl_opcode::CLEAR_COLOR => {
                        let r = rd_f32(self.ops, &mut p);
                        let g = rd_f32(self.ops, &mut p);
                        let b = rd_f32(self.ops, &mut p);
                        let a = rd_f32(self.ops, &mut p);
                        ex.client.clear_color = [r, g, b, a];
                        gl.clear_color(r, g, b, a);
                    }
                    gl_opcode::CLEAR => {
                        let bits = rd_u8(self.ops, &mut p);
                        let mask = (if bits & 1 != 0 { glow::COLOR_BUFFER_BIT } else { 0 })
                            | (if bits & 2 != 0 { glow::DEPTH_BUFFER_BIT } else { 0 })
                            | (if bits & 4 != 0 { glow::STENCIL_BUFFER_BIT } else { 0 });
                        gl.clear(mask);
                    }
                    gl_opcode::ENABLE => {
                        let cap = rd_u16(self.ops, &mut p) as u32;
                        ex.client.upsert_cap(cap, true);
                        gl.enable(cap);
                    }
                    gl_opcode::DISABLE => {
                        let cap = rd_u16(self.ops, &mut p) as u32;
                        ex.client.upsert_cap(cap, false);
                        gl.disable(cap);
                    }
                    gl_opcode::DEPTH_FUNC => {
                        let f = rd_u16(self.ops, &mut p) as u32;
                        ex.client.depth_func = f;
                        gl.depth_func(f);
                    }
                    gl_opcode::DEPTH_MASK => {
                        let on = rd_u8(self.ops, &mut p) != 0;
                        ex.client.depth_mask = on;
                        gl.depth_mask(on);
                    }
                    gl_opcode::BLEND_FUNC => {
                        let src = rd_u16(self.ops, &mut p);
                        let dst = rd_u16(self.ops, &mut p);
                        ex.client.blend_func = (src as u32, dst as u32, src as u32, dst as u32);
                        gl.blend_func(src as u32, dst as u32);
                    }
                    gl_opcode::BLEND_FUNC_SEPARATE => {
                        let src_rgb = rd_u16(self.ops, &mut p);
                        let dst_rgb = rd_u16(self.ops, &mut p);
                        let src_a = rd_u16(self.ops, &mut p);
                        let dst_a = rd_u16(self.ops, &mut p);
                        ex.client.blend_func = (
                            src_rgb as u32, dst_rgb as u32, src_a as u32, dst_a as u32,
                        );
                        gl.blend_func_separate(
                            src_rgb as u32, dst_rgb as u32, src_a as u32, dst_a as u32,
                        );
                    }
                    gl_opcode::BLEND_EQUATION_SEPARATE => {
                        let rgb = rd_u16(self.ops, &mut p);
                        let a = rd_u16(self.ops, &mut p);
                        ex.client.blend_equation = (rgb as u32, a as u32);
                        gl.blend_equation_separate(rgb as u32, a as u32);
                    }
                    gl_opcode::TEX_IMAGE_3D => {
                        let target = rd_u16(self.ops, &mut p);
                        let level = rd_u8(self.ops, &mut p) as i32;
                        let internal = rd_i32(self.ops, &mut p);
                        let w = rd_u32(self.ops, &mut p);
                        let h = rd_u32(self.ops, &mut p);
                        let d = rd_u32(self.ops, &mut p);
                        let format = rd_u16(self.ops, &mut p);
                        let ty = rd_u16(self.ops, &mut p);
                        let has_data = rd_u8(self.ops, &mut p);
                        let range = rd_u32(self.ops, &mut p);
                        let data = if has_data != 0 {
                            Some(self.range_bytes(range)?.to_vec())
                        } else {
                            None
                        };
                        let tex = ex
                            .bound_texture(target as u32)
                            .and_then(|th| ex.obj_textures.get(&th).copied())
                            .ok_or_else(|| anyhow!("TEX_IMAGE_3D 无客户端绑定纹理"))?;
                        gl.bind_texture(target as u32, Some(tex));
                        gl.tex_image_3d(
                            target as u32, level, internal, w as i32, h as i32, d as i32, 0,
                            format as u32, ty as u32, data.as_deref(),
                        );
                    }
                    gl_opcode::TEX_SUB_IMAGE_2D => {
                        let target = rd_u16(self.ops, &mut p);
                        let level = rd_u8(self.ops, &mut p) as i32;
                        let x = rd_i32(self.ops, &mut p);
                        let y = rd_i32(self.ops, &mut p);
                        let w = rd_u32(self.ops, &mut p);
                        let h = rd_u32(self.ops, &mut p);
                        let format = rd_u16(self.ops, &mut p);
                        let ty = rd_u16(self.ops, &mut p);
                        let has_data = rd_u8(self.ops, &mut p);
                        let range = rd_u32(self.ops, &mut p);
                        let mut data = if has_data != 0 {
                            Some(self.range_bytes(range)?.to_vec())
                        } else {
                            None
                        };
                        let tex = ex
                            .bound_texture(target as u32)
                            .and_then(|th| ex.obj_textures.get(&th).copied())
                            .ok_or_else(|| anyhow!("TEX_SUB_IMAGE_2D 无客户端绑定纹理"))?;
                        // 子区写入改了槽内容：下一次整图上传不能再按旧键跳过。
                        if let Some(th) = ex.bound_texture(target as u32) {
                            ex.texture_cache.remove(&(th, target as u32, level));
                        }
                        if ex.unpack_flip_y {
                            if let Some(bytes) = data.as_mut() {
                                flip_rows_vertical(bytes, h);
                            }
                        }
                        let mut format = format;
                        if let Some((_nint, nfmt, bytes)) = expand_unsized_upload(
                            format as i32,
                            format as u32,
                            ty as u32,
                            data.as_deref(),
                            w,
                            h,
                            ex.unpack_alignment.max(0) as usize,
                        ) {
                            format = nfmt as u16;
                            data = Some(bytes);
                        }
                        gl.bind_texture(texture_bind_target(target as u32), Some(tex));
                        gl.tex_sub_image_2d(
                            target as u32, level, x, y, w as i32, h as i32,
                            format as u32, ty as u32,
                            if has_data != 0 {
                                glow::PixelUnpackData::Slice(
                                    data.as_deref().unwrap_or(&[]),
                                )
                            } else {
                                glow::PixelUnpackData::BufferOffset(0)
                            },
                        );
                    }
                    gl_opcode::RENDERBUFFER_CREATE => {
                        let handle = rd_u32(self.ops, &mut p);
                        ex.obj_renderbuffers.entry(handle).or_insert_with(|| {
                            gl.create_renderbuffer().expect("create_renderbuffer")
                        });
                    }
                    gl_opcode::BIND_RENDERBUFFER => {
                        let target = rd_u16(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let rb = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*ex.obj_renderbuffers.entry(handle).or_insert_with(|| {
                                gl.create_renderbuffer().expect("create_renderbuffer")
                            }))
                        };
                        if handle != NULL_HANDLE {
                            ex.client.rb_binds.retain(|(t, _)| *t != target as u32);
                            ex.client.rb_binds.push((target as u32, handle));
                        } else {
                            ex.client.rb_binds.retain(|(t, _)| *t != target as u32);
                        }
                        gl.bind_renderbuffer(target as u32, rb);
                    }
                    gl_opcode::RENDERBUFFER_STORAGE => {
                        let _handle = rd_u32(self.ops, &mut p);
                        let internal = rd_i32(self.ops, &mut p);
                        let w = rd_u32(self.ops, &mut p);
                        let h = rd_u32(self.ops, &mut p);
                        let rb = ex
                            .bound_renderbuffer(glow::RENDERBUFFER)
                            .and_then(|rh| ex.obj_renderbuffers.get(&rh).copied())
                            .ok_or_else(|| anyhow!("RENDERBUFFER_STORAGE 无客户端绑定渲染缓冲"))?;
                        gl.renderbuffer_storage(glow::RENDERBUFFER, internal as u32, w as i32, h as i32);
                    }
                    gl_opcode::COLOR_MASK => {
                        let r = rd_u8(self.ops, &mut p);
                        let g = rd_u8(self.ops, &mut p);
                        let b = rd_u8(self.ops, &mut p);
                        let a = rd_u8(self.ops, &mut p);
                        ex.client.color_mask = [r != 0, g != 0, b != 0, a != 0];
                        gl.color_mask(r != 0, g != 0, b != 0, a != 0);
                    }
                    gl_opcode::UNIFORM_2I => {
                        let range = rd_u32(self.ops, &mut p);
                        let x = rd_i32(self.ops, &mut p);
                        let y = rd_i32(self.ops, &mut p);
                        let loc = self.uniform_loc(ex, &state, range)?;
                        gl.uniform_2_i32(loc.as_ref(), x, y);
                    }
                    gl_opcode::UNIFORM_3I => {
                        let range = rd_u32(self.ops, &mut p);
                        let x = rd_i32(self.ops, &mut p);
                        let y = rd_i32(self.ops, &mut p);
                        let z = rd_i32(self.ops, &mut p);
                        let loc = self.uniform_loc(ex, &state, range)?;
                        gl.uniform_3_i32(loc.as_ref(), x, y, z);
                    }
                    gl_opcode::UNIFORM_4I => {
                        let range = rd_u32(self.ops, &mut p);
                        let x = rd_i32(self.ops, &mut p);
                        let y = rd_i32(self.ops, &mut p);
                        let z = rd_i32(self.ops, &mut p);
                        let w = rd_i32(self.ops, &mut p);
                        let loc = self.uniform_loc(ex, &state, range)?;
                        gl.uniform_4_i32(loc.as_ref(), x, y, z, w);
                    }
                    gl_opcode::TEX_STORAGE_2D => {
                        let target = rd_u16(self.ops, &mut p);
                        let levels = rd_u8(self.ops, &mut p) as i32;
                        let internal = rd_i32(self.ops, &mut p);
                        let w = rd_u32(self.ops, &mut p);
                        let h = rd_u32(self.ops, &mut p);
                        gl.tex_storage_2d(target as u32, levels, internal as u32, w as i32, h as i32);
                    }
                    gl_opcode::TEX_STORAGE_3D => {
                        let target = rd_u16(self.ops, &mut p);
                        let levels = rd_u8(self.ops, &mut p) as i32;
                        let internal = rd_i32(self.ops, &mut p);
                        let w = rd_u32(self.ops, &mut p);
                        let h = rd_u32(self.ops, &mut p);
                        let d = rd_u32(self.ops, &mut p);
                        gl.tex_storage_3d(target as u32, levels, internal as u32, w as i32, h as i32, d as i32);
                    }
                    gl_opcode::TEX_SUB_IMAGE_3D => {
                        let target = rd_u16(self.ops, &mut p);
                        let level = rd_u8(self.ops, &mut p) as i32;
                        let x = rd_i32(self.ops, &mut p);
                        let y = rd_i32(self.ops, &mut p);
                        let z = rd_i32(self.ops, &mut p);
                        let w = rd_u32(self.ops, &mut p);
                        let h = rd_u32(self.ops, &mut p);
                        let d = rd_u32(self.ops, &mut p);
                        let format = rd_u16(self.ops, &mut p);
                        let ty = rd_u16(self.ops, &mut p);
                        let has_data = rd_u8(self.ops, &mut p);
                        let range = rd_u32(self.ops, &mut p);
                        let data = if has_data != 0 {
                            Some(self.range_bytes(range)?.to_vec())
                        } else {
                            None
                        };
                        let tex = ex
                            .bound_texture(target as u32)
                            .and_then(|th| ex.obj_textures.get(&th).copied())
                            .ok_or_else(|| anyhow!("TEX_SUB_IMAGE_3D 无客户端绑定纹理"))?;
                        gl.bind_texture(target as u32, Some(tex));
                        gl.tex_sub_image_3d(
                            target as u32, level, x, y, z, w as i32, h as i32, d as i32,
                            format as u32, ty as u32,
                            if has_data != 0 {
                                glow::PixelUnpackData::Slice(data.as_deref().unwrap_or(&[]))
                            } else {
                                glow::PixelUnpackData::BufferOffset(0)
                            },
                        );
                    }
                    gl_opcode::BIND_BUFFER_BASE => {
                        let target = rd_u16(self.ops, &mut p);
                        let index = rd_u32(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let buf = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*ex.obj_buffers.entry(handle).or_insert_with(|| {
                                gl.create_buffer().expect("create_buffer")
                            }))
                        };
                        gl.bind_buffer_base(target as u32, index, buf);
                    }
                    gl_opcode::DRAW_ARRAYS_INSTANCED => {
                        let mode = rd_u16(self.ops, &mut p);
                        let first = rd_i32(self.ops, &mut p);
                        let count = rd_i32(self.ops, &mut p);
                        let primcount = rd_i32(self.ops, &mut p);
                        gl.draw_arrays_instanced(mode as u32, first, count, primcount);
                    }
                    gl_opcode::DRAW_ELEMENTS_INSTANCED => {
                        let mode = rd_u16(self.ops, &mut p);
                        let count = rd_i32(self.ops, &mut p);
                        let ty = rd_u16(self.ops, &mut p);
                        let offset = rd_i32(self.ops, &mut p);
                        let primcount = rd_i32(self.ops, &mut p);
                        gl.draw_elements_instanced(
                            mode as u32, count, ty as u32, offset, primcount,
                        );
                    }
                    gl_opcode::VERTEX_ATTRIB_NFV => {
                        let index = rd_u32(self.ops, &mut p);
                        let nfloats = rd_u32(self.ops, &mut p);
                        let range = rd_u32(self.ops, &mut p);
                        let data = self.range_bytes(range)?;
                        let mut f = [0f32; 4];
                        for (i, chunk) in data.chunks(4).take(4).enumerate() {
                            if chunk.len() == 4 {
                                f[i] = f32::from_le_bytes(chunk.try_into().unwrap());
                            }
                        }
                        match nfloats {
                            1 => gl.vertex_attrib_1_f32(index, f[0]),
                            2 => gl.vertex_attrib_2_f32(index, f[0], f[1]),
                            3 => gl.vertex_attrib_3_f32(index, f[0], f[1], f[2]),
                            _ => gl.vertex_attrib_4_f32(index, f[0], f[1], f[2], f[3]),
                        }
                    }
                    gl_opcode::UNIFORM_BLOCK_BINDING => {
                        let program = rd_u32(self.ops, &mut p);
                        let block_index = rd_u32(self.ops, &mut p);
                        let binding = rd_u32(self.ops, &mut p);
                        // program pid 在 link/use 时解析（同 USE 路径的缓存）：
                        // 块绑定是 program 对象状态，必须在真 program 上执行。
                        let key = ex.program_key_for(gl, program)?;
                        match ex.program_cache.get(&key) {
                            Some(entry) => {
                                gl.uniform_block_binding(entry.program, block_index, binding);
                            }
                            None => bail!(
                                "gpu stream: program {program} 未编译，块绑定无处落"
                            ),
                        }
                    }
                    gl_opcode::BIND_ATTRIB_LOCATION => {
                        let program = rd_u32(self.ops, &mut p);
                        let index = rd_u32(self.ops, &mut p);
                        let name_range = rd_u32(self.ops, &mut p);
                        let name = String::from_utf8_lossy(self.range_bytes(name_range)?).to_string();
                        // link 前生效：真 program 在 USE 时惰性编译，这里
                        // 只记账，编译时统一重放（program_key_for）。
                        ex.program_attrib_binds
                            .entry(program)
                            .or_default()
                            .push((index, name));
                    }
                    gl_opcode::BLEND_EQUATION => {
                        let eq = rd_u16(self.ops, &mut p) as u32;
                        ex.client.blend_equation = (eq, eq);
                        gl.blend_equation(eq);
                    }
                    gl_opcode::BLEND_COLOR => {
                        let r = rd_f32(self.ops, &mut p);
                        let g = rd_f32(self.ops, &mut p);
                        let b = rd_f32(self.ops, &mut p);
                        let a = rd_f32(self.ops, &mut p);
                        ex.client.blend_color = [r, g, b, a];
                        gl.blend_color(r, g, b, a);
                    }
                    gl_opcode::CULL_FACE => {
                        let mode = rd_u16(self.ops, &mut p) as u32;
                        ex.client.cull_face = mode;
                        gl.cull_face(mode);
                    }
                    gl_opcode::FRONT_FACE => {
                        let mode = rd_u16(self.ops, &mut p) as u32;
                        ex.client.front_face = mode;
                        gl.front_face(mode);
                    }
                    gl_opcode::CLEAR_DEPTH => {
                        let d = rd_f32(self.ops, &mut p);
                        ex.client.clear_depth = d;
                        gl.clear_depth_f32(d);
                    }
                    gl_opcode::CLEAR_STENCIL => {
                        let s = rd_i32(self.ops, &mut p);
                        ex.client.clear_stencil = s;
                        gl.clear_stencil(s);
                    }
                    gl_opcode::POLYGON_OFFSET => {
                        let factor = rd_f32(self.ops, &mut p);
                        let units = rd_f32(self.ops, &mut p);
                        ex.client.polygon_offset = (factor, units);
                        gl.polygon_offset(factor, units);
                    }
                    gl_opcode::SCISSOR => {
                        let x = rd_i32(self.ops, &mut p);
                        let y = rd_i32(self.ops, &mut p);
                        let w = rd_i32(self.ops, &mut p);
                        let h = rd_i32(self.ops, &mut p);
                        ex.client.scissor = Some([x, y, w, h]);
                        gl.scissor(x, y, w, h);
                    }
                    gl_opcode::STENCIL_FUNC => {
                        let func = rd_u16(self.ops, &mut p);
                        let reference = rd_i32(self.ops, &mut p);
                        let mask = rd_u32(self.ops, &mut p);
                        ex.client.stencil_func = (func as u32, reference, mask);
                        gl.stencil_func(func as u32, reference, mask);
                    }
                    gl_opcode::STENCIL_OP => {
                        let sfail = rd_u16(self.ops, &mut p);
                        let dpfail = rd_u16(self.ops, &mut p);
                        let dppass = rd_u16(self.ops, &mut p);
                        ex.client.stencil_op =
                            (sfail as u32, dpfail as u32, dppass as u32);
                        gl.stencil_op(sfail as u32, dpfail as u32, dppass as u32);
                    }
                    gl_opcode::STENCIL_MASK => {
                        let m = rd_u32(self.ops, &mut p);
                        ex.client.stencil_mask = m;
                        gl.stencil_mask(m);
                    }
                    gl_opcode::DRAW_ARRAYS => {
                        let mode = rd_u16(self.ops, &mut p);
                        let first = rd_i32(self.ops, &mut p);
                        let count = rd_i32(self.ops, &mut p);
                        if trace {
                            eprintln!(
                                "       drawArrays mode={mode} first={first} count={count} scissor={} depth={} blend={} cull={}",
                                gl.is_enabled(glow::SCISSOR_TEST),
                                gl.is_enabled(glow::DEPTH_TEST),
                                gl.is_enabled(glow::BLEND),
                                gl.is_enabled(glow::CULL_FACE),
                            );
                            gl.draw_arrays(mode as u32, first, count);
                            let mut px = [0u8; 4];
                            gl.read_pixels(
                                960,
                                540,
                                1,
                                1,
                                glow::RGBA,
                                glow::UNSIGNED_BYTE,
                                glow::PixelPackData::Slice(&mut px),
                            );
                            eprintln!(
                                "       glError after draw = {:?} center={px:?}",
                                gl.get_error()
                            );
                            continue;
                        }
                        gl.draw_arrays(mode as u32, first, count);
                    }
                    gl_opcode::FRAMEBUFFER_RENDERBUFFER => {
                        let target = rd_u16(self.ops, &mut p);
                        let attachment = rd_u16(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let rb = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*ex.obj_renderbuffers.entry(handle).or_insert_with(|| {
                                gl.create_renderbuffer().expect("create_renderbuffer")
                            }))
                        };
                        gl.framebuffer_renderbuffer(
                            target as u32,
                            attachment as u32,
                            glow::RENDERBUFFER,
                            rb,
                        );
                    }
                    gl_opcode::SAMPLER_CREATE => {
                        let handle = rd_u32(self.ops, &mut p);
                        ex.obj_samplers
                            .entry(handle)
                            .or_insert_with(|| gl.create_sampler().expect("create_sampler"));
                    }
                    gl_opcode::SAMPLER_DELETE => {
                        let handle = rd_u32(self.ops, &mut p);
                        if let Some(smp) = ex.obj_samplers.remove(&handle) {
                            gl.delete_sampler(smp);
                        }
                    }
                    gl_opcode::SAMPLER_BIND => {
                        let unit = rd_u32(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let smp = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*ex.obj_samplers.entry(handle).or_insert_with(|| {
                                gl.create_sampler().expect("create_sampler")
                            }))
                        };
                        gl.bind_sampler(unit, smp);
                    }
                    gl_opcode::SAMPLER_PARAM_I => {
                        let handle = rd_u32(self.ops, &mut p);
                        let pname = rd_u16(self.ops, &mut p);
                        let value = rd_i32(self.ops, &mut p);
                        if let Some(smp) = ex.obj_samplers.get(&handle).copied() {
                            gl.sampler_parameter_i32(smp, pname as u32, value);
                        }
                    }
                    gl_opcode::SAMPLER_PARAM_F => {
                        let handle = rd_u32(self.ops, &mut p);
                        let pname = rd_u16(self.ops, &mut p);
                        let value = rd_f32(self.ops, &mut p);
                        if let Some(smp) = ex.obj_samplers.get(&handle).copied() {
                            gl.sampler_parameter_f32(smp, pname as u32, value);
                        }
                    }
                    gl_opcode::STENCIL_FUNC_SEPARATE => {
                        let face = rd_u16(self.ops, &mut p);
                        let func = rd_u16(self.ops, &mut p);
                        let reference = rd_i32(self.ops, &mut p);
                        let mask = rd_u32(self.ops, &mut p);
                        gl.stencil_func_separate(face as u32, func as u32, reference, mask);
                    }
                    gl_opcode::STENCIL_OP_SEPARATE => {
                        let face = rd_u16(self.ops, &mut p);
                        let sfail = rd_u16(self.ops, &mut p);
                        let dpfail = rd_u16(self.ops, &mut p);
                        let dppass = rd_u16(self.ops, &mut p);
                        gl.stencil_op_separate(
                            face as u32,
                            sfail as u32,
                            dpfail as u32,
                            dppass as u32,
                        );
                    }
                    gl_opcode::RENDERBUFFER_STORAGE_MS => {
                        let samples = rd_u32(self.ops, &mut p);
                        let internal = rd_i32(self.ops, &mut p);
                        let w = rd_u32(self.ops, &mut p);
                        let h = rd_u32(self.ops, &mut p);
                        let _rb = ex
                            .bound_renderbuffer(glow::RENDERBUFFER)
                            .and_then(|rh| ex.obj_renderbuffers.get(&rh).copied())
                            .ok_or_else(|| {
                                anyhow!("RENDERBUFFER_STORAGE_MS 无客户端绑定渲染缓冲")
                            })?;
                        gl.renderbuffer_storage_multisample(
                            glow::RENDERBUFFER,
                            samples as i32,
                            internal as u32,
                            w as i32,
                            h as i32,
                        );
                    }
                    gl_opcode::CLEAR_BUFFER_FV => {
                        let target = rd_u16(self.ops, &mut p);
                        let drawbuffer = rd_i32(self.ops, &mut p);
                        let r = rd_f32(self.ops, &mut p);
                        let g = rd_f32(self.ops, &mut p);
                        let b = rd_f32(self.ops, &mut p);
                        let a = rd_f32(self.ops, &mut p);
                        gl.clear_buffer_f32_slice(target as u32, drawbuffer as u32, &[r, g, b, a]);
                    }
                    gl_opcode::CLEAR_BUFFER_IV => {
                        let target = rd_u16(self.ops, &mut p);
                        let drawbuffer = rd_i32(self.ops, &mut p);
                        let x = rd_i32(self.ops, &mut p);
                        let y = rd_i32(self.ops, &mut p);
                        let z = rd_i32(self.ops, &mut p);
                        let w = rd_i32(self.ops, &mut p);
                        gl.clear_buffer_i32_slice(target as u32, drawbuffer as u32, &[x, y, z, w]);
                    }
                    gl_opcode::CLEAR_BUFFER_FI => {
                        let target = rd_u16(self.ops, &mut p);
                        let drawbuffer = rd_i32(self.ops, &mut p);
                        let depth = rd_f32(self.ops, &mut p);
                        let stencil = rd_i32(self.ops, &mut p);
                        gl.clear_buffer_depth_stencil(
                            target as u32,
                            drawbuffer as u32,
                            depth,
                            stencil,
                        );
                    }
                    gl_opcode::BIND_BUFFER_RANGE => {
                        let target = rd_u16(self.ops, &mut p);
                        let index = rd_u32(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let offset = rd_i32(self.ops, &mut p);
                        let size = rd_i32(self.ops, &mut p);
                        let buf = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*ex.obj_buffers.entry(handle).or_insert_with(|| {
                                gl.create_buffer().expect("create_buffer")
                            }))
                        };
                        gl.bind_buffer_range(target as u32, index, buf, offset, size);
                    }
                    gl_opcode::BLIT_FRAMEBUFFER => {
                        let sx0 = rd_i32(self.ops, &mut p);
                        let sy0 = rd_i32(self.ops, &mut p);
                        let sx1 = rd_i32(self.ops, &mut p);
                        let sy1 = rd_i32(self.ops, &mut p);
                        let dx0 = rd_i32(self.ops, &mut p);
                        let dy0 = rd_i32(self.ops, &mut p);
                        let dx1 = rd_i32(self.ops, &mut p);
                        let dy1 = rd_i32(self.ops, &mut p);
                        let mask = rd_u16(self.ops, &mut p);
                        let filter = rd_u16(self.ops, &mut p);
                        let mut gl_mask = 0u32;
                        if mask & 1 != 0 {
                            gl_mask |= glow::COLOR_BUFFER_BIT;
                        }
                        if mask & 2 != 0 {
                            gl_mask |= glow::DEPTH_BUFFER_BIT;
                        }
                        if mask & 4 != 0 {
                            gl_mask |= glow::STENCIL_BUFFER_BIT;
                        }
                        gl.blit_framebuffer(
                            sx0, sy0, sx1, sy1, dx0, dy0, dx1, dy1, gl_mask, filter as u32,
                        );
                    }
                    gl_opcode::DRAW_ELEMENTS => {
                        let mode = rd_u16(self.ops, &mut p);
                        let count = rd_i32(self.ops, &mut p);
                        let ty = rd_u16(self.ops, &mut p);
                        let offset = rd_i32(self.ops, &mut p);
                        gl.draw_elements(mode as u32, count, ty as u32, offset);
                    }
                    other => bail!("gpu stream: 未知 opcode {other} @ {start}"),
                }
                // 定长命令的步进校验（变长命令自行消费）。
                if let Some(len) = gl_opcode::PAYLOAD_LEN[opcode as usize] {
                    let consumed = p - start - 1;
                    if consumed != len as usize {
                        bail!(
                            "gpu stream: opcode {opcode} payload 漂移（表 {len}，实消费 {consumed}）"
                        );
                    }
                }
            }
        }
        Ok(state)
    }

    fn range_bytes(&self, range: u32) -> Result<&'a [u8]> {
        let r = self
            .ranges
            .get(range as usize)
            .ok_or_else(|| anyhow!("gpu stream: 坏 range {range}"))?;
        Ok(&self.bytes[r.start as usize..(r.start + r.len) as usize])
    }

    /// 调试：range 内的 UTF-8 文本（uniform/attrib 名）。
    fn range_str(&self, range: u32) -> Result<&'a str> {
        Ok(std::str::from_utf8(self.range_bytes(range)?).unwrap_or("?"))
    }

    fn tail_f32(&self, p: &mut usize, count: u32) -> Result<Vec<f32>> {
        if *p + count as usize * 4 > self.ops.len() {
            bail!("gpu stream: UNIFORM_NF 尾部越界");
        }
        let mut out = Vec::with_capacity(count as usize);
        for _ in 0..count {
            out.push(rd_f32(self.ops, p));
        }
        Ok(out)
    }

    fn tail_i32(&self, p: &mut usize, count: u32) -> Result<Vec<i32>> {
        if *p + count as usize * 4 > self.ops.len() {
            bail!("gpu stream: UNIFORM_NI 尾部越界");
        }
        let mut out = Vec::with_capacity(count as usize);
        for _ in 0..count {
            out.push(rd_i32(self.ops, p));
        }
        Ok(out)
    }

    /// 扫描序号（core `getAttribLocation` 的返回值，以 `index:` 编码进
    /// name 槽）→ 真名 → 真 location；直接名字直取。
    fn attrib_loc(
        &self,
        ex: &mut GpuStreamExecutor,
        state: &ReplayState,
        name_range: u32,
    ) -> Result<Option<u32>> {
        let name = std::str::from_utf8(self.range_bytes(name_range)?)?;
        // `index:<n>` 是录制面 getAttribLocation 的返回值——即反射会话里
        // 该 attrib 的驱动 location（core 取反射的 `l` 字段）。vertexAttrib
        // Pointer 是 VAO 状态、与程序无关，GL 语义上此时可能还没
        // useProgram，所以这里不查 attrib_names，直接用 location。
        if let Some(idx) = name.strip_prefix("index:") {
            let loc: u32 = idx.parse()?;
            return Ok(Some(loc));
        }
        let key = state
            .program_key
            .as_ref()
            .ok_or_else(|| anyhow!("attrib 命令出现在 USE_PROGRAM 之前"))?;
        GpuStreamExecutor::resolve_attrib_on(ex, self.gl, key, name)
    }

    fn uniform_loc(
        &self,
        ex: &mut GpuStreamExecutor,
        state: &ReplayState,
        name_range: u32,
    ) -> Result<Option<glow::UniformLocation>> {
        let name = std::str::from_utf8(self.range_bytes(name_range)?)?;
        let key = state
            .program_key
            .as_ref()
            .ok_or_else(|| anyhow!("uniform 命令出现在 USE_PROGRAM 之前（{name}）"))?;
        GpuStreamExecutor::resolve_uniform_on(ex, self.gl, key, name)
    }

    /// uniform 的声明类型；`arr[3]` 这类非首元素名按裸名查。
    fn uniform_type(
        &self,
        ex: &mut GpuStreamExecutor,
        state: &ReplayState,
        name_range: u32,
    ) -> Result<Option<u32>> {
        let name = std::str::from_utf8(self.range_bytes(name_range)?)?;
        let key = state
            .program_key
            .as_ref()
            .ok_or_else(|| anyhow!("uniform 命令出现在 USE_PROGRAM 之前"))?;
        let types = &ex.entry(key)?.uniform_types;
        Ok(types.get(name).copied().or_else(|| {
            let base = name.strip_suffix(']').and_then(|n| n.rsplit_once('[')).map(|(b, _)| b)?;
            types.get(base).copied()
        }))
    }
}

impl GpuStreamExecutor {
    /// 解析（或惰性编译）`pid` 引用的程序，返回内容缓存 key。
    fn program_key_for(&mut self, gl: &Arc<glow::Context>, pid: u32) -> Result<String> {
        if let Some(key) = self.program_key_by_pid.get(&pid) {
            return Ok(key.clone());
        }
        let (vid, fid) = self
            .program_shaders
            .get(&pid)
            .and_then(|s| Some((s.0?, s.1?)))
            .ok_or_else(|| anyhow!("gpu stream: program {pid} 缺 shader"))?;
        let vs = self
            .shader_src
            .get(&vid)
            .ok_or_else(|| anyhow!("gpu stream: program {pid} 缺 vs 源"))?;
        let fs = self
            .shader_src
            .get(&fid)
            .ok_or_else(|| anyhow!("gpu stream: program {pid} 缺 fs 源"))?;
        let base = format!("{}|{}", hash_bytes(vs), hash_bytes(fs));
        // 重放 bindAttribLocation：绑定表进 key——同源码不同表必须各编
        // 各的，否则先编译者的 location 布局会被后者复用。
        let binds = self
            .program_attrib_binds
            .get(&pid)
            .cloned()
            .unwrap_or_default();
        let key = if binds.is_empty() {
            base
        } else {
            let mut material = String::new();
            for (i, n) in &binds {
                material.push_str(&i.to_string());
                material.push('\u{1}');
                material.push_str(n);
                material.push('\u{2}');
            }
            format!("{base}|ab{}", hash_bytes(material.as_bytes()))
        };
        if !self.program_cache.contains_key(&key) {
            // 执行上下文是 ANGLE（GLES3）：GLSL ES 300 原文直接编译，不做
            // 桌面升级（WGL 时代的 naga 330 core 转换已随桌面 GL 退役）。
            let vs_src = std::str::from_utf8(vs)?;
            let fs_src = std::str::from_utf8(fs)?;
            let entry = compile_program_entry(gl, vs_src, fs_src, &binds)?;
            self.program_cache.insert(key.clone(), entry);
        }
        self.program_key_by_pid.insert(pid, key.clone());
        Ok(key)
    }

    fn entry(&self, key: &str) -> Result<&ProgramEntry> {
        self.program_cache
            .get(key)
            .ok_or_else(|| anyhow!("program 缓存缺失（{key}）"))
    }

    pub(crate) fn resolve_attrib_on(
        &mut self,
        gl: &Arc<glow::Context>,
        key: &str,
        name: &str,
    ) -> Result<Option<u32>> {
        let real_name = match name.strip_prefix("index:") {
            Some(idx) => {
                let index: usize = idx.parse()?;
                let entry = self.entry(key)?;
                entry
                    .attrib_names
                    .get(index)
                    .cloned()
                    .ok_or_else(|| anyhow!("attrib 序号 {index} 超出扫描范围 {}", entry.attrib_names.len()))?
            }
            None => name.to_string(),
        };
        if let Some(hit) = self.entry(key)?.attrib_loc.get(&real_name) {
            return Ok(*hit);
        }
        let program = self.entry(key)?.program;
        let loc = unsafe { gl.get_attrib_location(program, &real_name) };
        self.program_cache
            .get_mut(key)
            .unwrap()
            .attrib_loc
            .insert(real_name, loc);
        Ok(loc)
    }

    pub(crate) fn resolve_uniform_on(
        &mut self,
        gl: &Arc<glow::Context>,
        key: &str,
        name: &str,
    ) -> Result<Option<glow::UniformLocation>> {
        if let Some(hit) = self.entry(key)?.uniform_loc.get(name) {
            return Ok(hit.clone());
        }
        let program = self.entry(key)?.program;
        let loc = unsafe { gl.get_uniform_location(program, name) };
        self.program_cache
            .get_mut(key)
            .unwrap()
            .uniform_loc
            .insert(name.to_string(), loc.clone());
        Ok(loc)
    }
}

fn compile_program_entry(
    gl: &Arc<glow::Context>,
    vs_src: &str,
    fs_src: &str,
    attrib_binds: &[(u32, String)],
) -> Result<ProgramEntry> {
    unsafe {
        // 调试：`OC_DEBUG_FS=1` 时把片元着色器换成纯红，用来区分「几何/绘制
        // 通路坏了」与「shader 输出黑」。
        let fs_src = if std::env::var_os("OC_DEBUG_FS").is_some() {
            if fs_src.starts_with("#version 300") {
                "#version 300 es\nprecision mediump float;\nout vec4 ocDebugColor;\nvoid main(){ ocDebugColor = vec4(1.0, 0.0, 0.0, 1.0); }"
            } else {
                "void main(){ gl_FragColor = vec4(1.0, 0.0, 0.0, 1.0); }"
            }
        } else {
            fs_src
        };
        let compile = |ty: u32, src: &str| -> Result<glow::Shader> {
            let sh = gl.create_shader(ty).map_err(|e| anyhow!("create_shader: {e}"))?;
            gl.shader_source(sh, &swiftshader_math(src));
            gl.compile_shader(sh);
            if !gl.get_shader_compile_status(sh) {
                let log = gl.get_shader_info_log(sh);
                bail!("shader 编译失败: {log}");
            }
            Ok(sh)
        };
        let vs = compile(glow::VERTEX_SHADER, vs_src)?;
        let fs = compile(glow::FRAGMENT_SHADER, fs_src)?;
        let program = gl
            .create_program()
            .map_err(|e| anyhow!("create_program: {e}"))?;
        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        // 重放 link 前的 bindAttribLocation：调用方（@antv 等固定布局的
        // renderer）按请求值发 vertexAttribPointer，驱动自然分配的
        // location 与之错位会让指针全落空。
        for (index, name) in attrib_binds {
            gl.bind_attrib_location(program, *index, name);
        }
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            bail!("program 链接失败: {log}");
        }
        // attrib 序号表与 core 侧 getAttribLocation 的返回值同源同序：
        // 都是驱动 glGetActiveAttrib 的真实序。core 的录制面把
        // `getAttribLocation` 的返回值编成 `index:N`，这里按同一张驱动
        // 序列表还原——词法声明扫描（scan_declarations）与此序不一致
        // （#ifdef 内声明、去重、驱动重排），曾经把 three 的 position
        // 指针错位到别的 attrib 上导致整个 draw 落空。
        let mut attrib_names = Vec::new();
        for i in 0..gl.get_active_attributes(program) {
            if let Some(a) = gl.get_active_attribute(program, i) {
                attrib_names.push(a.name);
            }
        }
        let mut uniform_types = HashMap::new();
        for i in 0..gl.get_active_uniforms(program) {
            if let Some(u) = gl.get_active_uniform(program, i) {
                if let Some(base) = u.name.strip_suffix("[0]") {
                    uniform_types.insert(base.to_string(), u.utype);
                }
                uniform_types.insert(u.name, u.utype);
            }
        }
        Ok(ProgramEntry {
            program,
            attrib_names,
            attrib_loc: HashMap::new(),
            uniform_loc: HashMap::new(),
            uniform_types,
        })
    }
}

/// 参考渲染器（Chrome `--use-angle=swiftshader`）的 sin/cos/dot 语义：sin/cos
/// 是 SwiftShader 的 5 阶多项式近似（ShaderCore.cpp `Sin5`），点积与乘加按
/// Subzero 后端不融合、逐步舍入。`fract(sin(dot(p, k)) * 43758.5453)` 一类
/// 散列对最低位超敏感：驱动原生 sin 或融合的点积指令会让整幅噪声图案换一张。
/// 乘积后加一个从不赋值的 uniform（GL 规定初值 0）：驱动折叠不掉它，乘法
/// 因此单独舍入。
const SW_MATH_PRELUDE: &str = "
uniform highp float oc_sw_z;
highp float oc_sw_m(highp float a, highp float b) { return a * b + oc_sw_z; }
highp float oc_sw_rne(highp float x) {
  highp float r = floor(x + 0.5);
  return (r - x == 0.5 && mod(r, 2.0) != 0.0) ? r - 1.0 : r;
}
highp float oc_sw_sin5(highp float z) {
  highp float z2 = oc_sw_m(z, z);
  return oc_sw_m(oc_sw_m(oc_sw_m(74.43888854980469, z2) + -41.169368743896484, z2) + 6.282308578491211, z);
}
highp float oc_sw_sin(highp float x) {
  highp float t = oc_sw_m(x, -0.15915493667125702) + 0.25;
  return oc_sw_sin5(0.25 - abs(t - oc_sw_rne(t)));
}
highp float oc_sw_cos(highp float x) {
  highp float t = oc_sw_m(x, 0.15915493667125702);
  return oc_sw_sin5(0.25 - abs(t - oc_sw_rne(t)));
}
highp vec2 oc_sw_sin(highp vec2 v) { return vec2(oc_sw_sin(v.x), oc_sw_sin(v.y)); }
highp vec3 oc_sw_sin(highp vec3 v) { return vec3(oc_sw_sin(v.x), oc_sw_sin(v.y), oc_sw_sin(v.z)); }
highp vec4 oc_sw_sin(highp vec4 v) { return vec4(oc_sw_sin(v.x), oc_sw_sin(v.y), oc_sw_sin(v.z), oc_sw_sin(v.w)); }
highp vec2 oc_sw_cos(highp vec2 v) { return vec2(oc_sw_cos(v.x), oc_sw_cos(v.y)); }
highp vec3 oc_sw_cos(highp vec3 v) { return vec3(oc_sw_cos(v.x), oc_sw_cos(v.y), oc_sw_cos(v.z)); }
highp vec4 oc_sw_cos(highp vec4 v) { return vec4(oc_sw_cos(v.x), oc_sw_cos(v.y), oc_sw_cos(v.z), oc_sw_cos(v.w)); }
highp float oc_sw_dot(highp float a, highp float b) { return oc_sw_m(a, b); }
highp float oc_sw_dot(highp vec2 a, highp vec2 b) { return oc_sw_m(a.y, b.y) + oc_sw_m(a.x, b.x); }
highp float oc_sw_dot(highp vec3 a, highp vec3 b) {
  return oc_sw_m(a.z, b.z) + (oc_sw_m(a.y, b.y) + oc_sw_m(a.x, b.x));
}
highp float oc_sw_dot(highp vec4 a, highp vec4 b) {
  return oc_sw_m(a.w, b.w) + (oc_sw_m(a.z, b.z) + (oc_sw_m(a.y, b.y) + oc_sw_m(a.x, b.x)));
}
";

/// 把 `sin(` / `cos(` / `dot(` 调用改写到 [`SW_MATH_PRELUDE`] 的等价实现，
/// 前言插在 `#version` / `#extension` 指令之后。
fn swiftshader_math(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len() + SW_MATH_PRELUDE.len());
    let (mut last, mut i) = (0, 0);
    while i < bytes.len() {
        let start = i;
        if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &src[start..i];
            if matches!(word, "sin" | "cos" | "dot") && src[i..].trim_start().starts_with('(') {
                out.push_str(&src[last..start]);
                out.push_str("oc_sw_");
                last = start;
            }
        } else {
            i += 1;
        }
    }
    if out.is_empty() {
        return src.to_string();
    }
    out.push_str(&src[last..]);
    let mut at = 0;
    let mut offset = 0;
    for line in out.split_inclusive('\n') {
        offset += line.len();
        let head = line.trim_start();
        if head.starts_with("#version") || head.starts_with("#extension") {
            at = offset;
        }
    }
    out.insert_str(at, SW_MATH_PRELUDE);
    out
}

/// GLSL 声明扫描（剥注释 → 按 `;` 分段 → 关键字匹配 → 名字提取）。
///
/// 必须与 core webgl_api.js 的 collectDecls 同序同规则：声明序、去重、
/// 数组名剥 `[..]`（数组与裸名各出现一次——attributes 场景 three 只用
/// 裸名，这里与 JS 相同地保留首次出现顺序）。
fn scan_declarations(src: &str, kind: &str) -> Vec<String> {
    let text = strip_glsl_comments(src);
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for statement in text.split(';') {
        let stmt = statement.trim();
        if stmt.is_empty() {
            continue;
        }
        // 词界匹配关键字（uniform / attribute / in）。
        let first_word = stmt.split_whitespace().next().unwrap_or("");
        let (want, rest) = if first_word == kind {
            (true, stmt[kind.len()..].trim_start())
        } else if kind == "attribute" && first_word == "in" {
            (true, stmt[2..].trim_start())
        } else {
            (false, stmt)
        };
        if !want {
            continue;
        }
        // 跳过精度限定符。
        let mut tokens = rest.split_whitespace().peekable();
        if let Some(&t) = tokens.peek() {
            if t == "highp" || t == "mediump" || t == "lowp" {
                tokens.next();
            }
        }
        // 类型 token。
        let _type = tokens.next();
        // 名字列表（逗号分隔，数组剥 [..]）。
        let tail = tokens.collect::<Vec<_>>().join(" ");
        for decl in tail.split(',') {
            let base = decl.trim().split('[').next().unwrap_or("").trim();
            if base.is_empty() || base.contains('(') {
                continue;
            }
            if kind == "uniform" {
                // 数组：`[0]` 名与裸名都进列表（与 JS 相同）。
                let raw = decl.trim();
                if raw.contains('[') {
                    if seen.insert(format!("{base}[0]")) {
                        out.push(format!("{base}[0]"));
                    }
                    if seen.insert(base.to_string()) {
                        out.push(base.to_string());
                    }
                } else if seen.insert(base.to_string()) {
                    out.push(base.to_string());
                }
            } else if seen.insert(base.to_string()) {
                out.push(base.to_string());
            }
        }
    }
    out
}

fn strip_glsl_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            if let Some(end) = src[i + 2..].find("*/") {
                i += 2 + end + 2;
                out.push(' ');
                continue;
            }
            break;
        }
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            if let Some(end) = src[i..].find('\n') {
                i += end;
                continue;
            }
            break;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}


/// 把 WebGL2 允许、桌面 GL 3.3 core 已移除的"未定尺基础格式"
/// （ALPHA / LUMINANCE / LUMINANCE_ALPHA / RGB）摊平成 RGBA8。
///
/// WebGL2 里 `texImage2D(TEXTURE_2D, 0, ALPHA, w, h, 0, ALPHA, UNSIGNED_BYTE, px)`
/// 是合法上传；桌面 GL 3.3 core 只接受定尺 internalformat，同形调用直接
/// GL_INVALID_ENUM。语义等价改写：ALPHA→RGBA8 且 `a = src`，LUMINANCE→
/// `(l,l,l,255)`，LUMINANCE_ALPHA→`(l,l,l,a)`，RGB→`(r,g,b,255)`。
///
/// 返回 `None` 表示无需改写（格式已定尺或数据缺失）。
/// `alignment` 是同批 `GL_UNPACK_ALIGNMENT`，用于算源行距（浏览器与
/// TinySDF 的图集常是宽度不被 4 整除的 ALPHA 图）。
fn expand_unsized_upload(
    internal: i32,
    format: u32,
    ty: u32,
    data: Option<&[u8]>,
    w: u32,
    h: u32,
    alignment: usize,
) -> Option<(i32, u32, Vec<u8>)> {
    const ALPHA: u32 = 0x1906;
    const RGB: u32 = 0x1907;
    const RGBA: u32 = 0x1908;
    const LUMINANCE: u32 = 0x1909;
    const LUMINANCE_ALPHA: u32 = 0x190A;
    const UNSIGNED_BYTE: u32 = 0x1401;
    if ty != UNSIGNED_BYTE {
        return None;
    }
    let channels = match format {
        ALPHA => 1usize,
        LUMINANCE => 1,
        LUMINANCE_ALPHA => 2,
        RGB => 3,
        _ => return None,
    };
    let src = data?;
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 {
        return None;
    }
    let align = if alignment == 0 { 4 } else { alignment };
    let row_raw = w * channels;
    let row_padded = (row_raw + align - 1) / align * align;
    // 数据长度既可能按 alignment 补齐，也可能被调用方按紧排送来。
    let stride = if src.len() >= row_padded * h {
        row_padded
    } else if src.len() >= row_raw * h {
        row_raw
    } else {
        return None;
    };
    let mut out = vec![0u8; w * h * 4];
    for row in 0..h {
        let base = row * stride;
        for col in 0..w {
            let si = base + col * channels;
            let di = (row * w + col) * 4;
            match format {
                ALPHA => {
                    out[di] = 0;
                    out[di + 1] = 0;
                    out[di + 2] = 0;
                    out[di + 3] = src[si];
                }
                LUMINANCE => {
                    let l = src[si];
                    out[di] = l;
                    out[di + 1] = l;
                    out[di + 2] = l;
                    out[di + 3] = 255;
                }
                LUMINANCE_ALPHA => {
                    let l = src[si];
                    out[di] = l;
                    out[di + 1] = l;
                    out[di + 2] = l;
                    out[di + 3] = src[si + 1];
                }
                _ => {
                    out[di] = src[si];
                    out[di + 1] = src[si + 1];
                    out[di + 2] = src[si + 2];
                    out[di + 3] = 255;
                }
            }
        }
    }
    let _ = internal;
    Some((RGBA as i32, RGBA, out))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{}:{h:016x}", bytes.len())
}

fn rd_u8(buf: &[u8], p: &mut usize) -> u8 {
    let v = buf[*p];
    *p += 1;
    v
}
fn rd_u16(buf: &[u8], p: &mut usize) -> u16 {
    let v = u16::from_le_bytes([buf[*p], buf[*p + 1]]);
    *p += 2;
    v
}
fn rd_u32(buf: &[u8], p: &mut usize) -> u32 {
    let v = u32::from_le_bytes([buf[*p], buf[*p + 1], buf[*p + 2], buf[*p + 3]]);
    *p += 4;
    v
}
fn rd_i32(buf: &[u8], p: &mut usize) -> i32 {
    rd_u32(buf, p) as i32
}
fn rd_f32(buf: &[u8], p: &mut usize) -> f32 {
    let v = f32::from_le_bytes([buf[*p], buf[*p + 1], buf[*p + 2], buf[*p + 3]]);
    *p += 4;
    v
}
