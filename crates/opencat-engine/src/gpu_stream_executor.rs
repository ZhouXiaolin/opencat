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
//! [`opencat_gpu_native::TextureRegistry`]（Skia 零拷贝采样），CPU 合成模式
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
    /// 内容 hash → 已上载纹理（跳过重传）。
    texture_cache: std::collections::HashSet<String>,
    /// WebGL `UNPACK_FLIP_Y_WEBGL` 标志（影响下一次 texImage/SubImage2D）。
    /// 桌面 GL 没有这个 pname，翻转由本执行器对上传字节手动做。
    unpack_flip_y: bool,
    // ── WebGL 句柄 → GL 对象映射（跨流持久）─────────────────────────
    // JS 侧句柄 realm 级单调、GL 对象存活由脚本 delete 决定；持久 renderer
    // 的后续帧流里没有创建命令（three 只在首次构造时 create），映射必须
    // 跨流存活，否则 useProgram/bindBuffer 全部悬空。
    obj_buffers: HashMap<u32, glow::Buffer>,
    obj_vaos: HashMap<u32, glow::VertexArray>,
    obj_textures: HashMap<u32, glow::NativeTexture>,
    obj_fbos: HashMap<u32, glow::NativeFramebuffer>,
    obj_renderbuffers: HashMap<u32, glow::NativeRenderbuffer>,
    shader_src: HashMap<u32, Vec<u8>>,
    shader_stage: HashMap<u32, u32>,
    /// pid → (vs handle, fs handle)
    program_shaders: HashMap<u32, (Option<u32>, Option<u32>)>,
    /// 流内 pid → 内容 key（USE 时解析出 entry）。
    program_key_by_pid: HashMap<u32, String>,
    /// VAO 的 element 绑定由执行器自己记账、VAO_BIND 时主动重绑。共享
    /// GL 上下文上 VAO 状态会被其他人（Skia 合成、restore、驱动）踩掉，
    /// 而 three 的 bindingStates 缓存使后续帧不再重发绑定——依赖驱动
    /// 保留状态必然炸（drawElements 拿到 index=0 直接访问违例）。
    vao_element: HashMap<u32, u32>,
    /// 当前绑定的 VAO 句柄（None = 无）。
    cur_vao: Option<u32>,
    /// 当前绑定的 element buffer 句柄（None = 无）。
    cur_element: Option<u32>,
    /// 当前使用的程序（内容 key；None = 无）。
    cur_program_key: Option<String>,
    /// 客户端可见的全局 GL 状态影子（three 的 JS 状态缓存视角）。
    client: ClientGlState,
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
}

impl Default for ClientGlState {
    fn default() -> Self {
        Self {
            caps: Vec::new(),
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
            active_unit: 0,
            tex_binds: Vec::new(),
            rb_binds: Vec::new(),
            array_buffer: None,
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
        // cube 六个面是独立 target,但绑定挂在 TEXTURE_CUBE_MAP 上。
        let binding_target = if (glow::TEXTURE_CUBE_MAP_POSITIVE_X..=glow::TEXTURE_CUBE_MAP_NEGATIVE_Z)
            .contains(&target)
        {
            glow::TEXTURE_CUBE_MAP
        } else {
            target
        };
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
            self.cur_element
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

struct ProgramEntry {
    program: glow::Program,
    /// 顶点源码的属性声明序（core 扫描序号的映射基准）。
    attrib_names: Vec<String>,
    attrib_loc: HashMap<String, Option<u32>>,
    uniform_loc: HashMap<String, Option<glow::UniformLocation>>,
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
        textures: &opencat_gpu_native::TextureRegistry,
        gpu_mode: bool,
    ) -> Result<Vec<(opencat_gpu_native::NativeTexture, u32)>> {
        let mut outputs = Vec::new();
        for stream in &section.streams {
            let out = self.execute_stream(gl, stream, plan, textures, gpu_mode)?;
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
        textures: &opencat_gpu_native::TextureRegistry,
        gpu_mode: bool,
    ) -> Result<Vec<(opencat_gpu_native::NativeTexture, u32)>> {
        let ops = &stream.ops.ops;
        let bytes: &[u8] = &stream.ops.bytes;
        let ranges: &[opencat_core::ir::draw_types::TableRange] = &stream.ops.byte_ranges;
        if std::env::var_os("OC_DBG_GPU").is_some() {
            eprintln!(
                "OCGPU stream '{}' {}x{} ops={} bytes={}",
                stream.target_id,
                stream.width,
                stream.height,
                ops.len(),
                bytes.len()
            );
        }

        // 输出纹理 + FBO：流画到自己的附件上（合成按占位 z 序采样）。
        let out_tex = opencat_gpu_native::gl::create_texture_rgba8(gl, stream.width, stream.height)
            .ok_or_else(|| anyhow!("gpu stream: 输出纹理创建失败"))?;
        let out_fbo =
            opencat_gpu_native::gl::create_framebuffer(gl, out_tex, stream.width, stream.height)
                .ok_or_else(|| anyhow!("gpu stream: 输出 FBO 创建失败"))?;

        // 流内不录 bindFramebuffer（作者代码不调它）；执行器把输出 FBO 设为
        // 绘制目标，否则 clear/draw 落到默认帧缓冲，输出纹理保持黑。
        unsafe {
            gl.bind_framebuffer(
                glow::FRAMEBUFFER,
                Some(glow::NativeFramebuffer(
                    std::num::NonZeroU32::new(out_fbo).unwrap(),
                )),
            );
        }

        let mut r = Replay { gl, ops, bytes, ranges };
        let st = r.run(self)?;

        // 占位不在本帧 plan = 该 canvas 子树本帧不可见（opacity 0 / 裁剪/
        // 移出屏）——流照常执行：three 等持久 renderer 的资源创建命令
        // （program/纹理/VBO）只在首帧录一次，跳过执行会让 GL 侧永远收
        // 不到它们。只丢弃输出，不回填 plan。
        let Some(entry) = plan
            .generated_images
            .iter_mut()
            .find(|e| e.id == stream.image_id)
        else {
            if std::env::var_os("OC_DBG_GPU").is_some() {
                eprintln!(
                    "OCGPU stream '{}' 占位不在本帧 plan（不可见）——已执行、输出丢弃",
                    stream.target_id
                );
            }
            opencat_gpu_native::gl::delete_framebuffer(gl, out_fbo);
            opencat_gpu_native::gl::delete_texture(gl, out_tex);
            let _ = st;
            return Ok(Vec::new());
        };

        let mut outputs = Vec::new();
        if gpu_mode {
            // 纹理登记到共享表；GL 资源的删除由调用方在合成完成后做
            // （outputs 携带 (纹理名, FBO)——调用点持有 current 上下文）。
            let native = opencat_gpu_native::NativeTexture::from_raw(out_tex.raw());
            let token = textures.publish(native, Box::new(()));
            entry.rgba = Arc::from(Vec::new());
            entry.gpu = Some(opencat_core::ir::generated_image::GpuImageHandle {
                token: token.raw(),
            });
            outputs.push((native, out_fbo));
        } else {
            let rgba = readback_sync(gl, out_fbo, stream.width, stream.height)?;
            entry.rgba = Arc::from(rgba);
            entry.gpu = None;
            opencat_gpu_native::gl::delete_framebuffer(gl, out_fbo);
            opencat_gpu_native::gl::delete_texture(gl, out_tex);
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
fn restore_default_state(ex: &mut GpuStreamExecutor, gl: &Arc<glow::Context>) {
    unsafe {
        // 绑定类恢复到客户端最后看到的对象(three 的缓存只在变化时重发);
        // 句柄已删的回落 None 并同步账本。VAO 先绑回——ELEMENT 绑定和
        // attrib 状态都属于 VAO,绑回后按账本重挂 element。
        let vao = ex.cur_vao.and_then(|h| ex.obj_vaos.get(&h).copied());
        gl.bind_vertex_array(vao);
        if vao.is_none() {
            ex.cur_vao = None;
            ex.cur_element = None;
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
        } else {
            let eh = ex.cur_vao.and_then(|v| ex.vao_element.get(&v).copied());
            ex.cur_element = eh;
            if let Some(h) = eh && let Some(buf) = ex.obj_buffers.get(&h) {
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(*buf));
            }
        }
        let program = ex
            .cur_program_key
            .as_ref()
            .and_then(|k| ex.program_cache.get(k));
        if program.is_none() {
            ex.cur_program_key = None;
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
        let mut state = ReplayState {
            program: None,
            program_key: None,
        };

        let mut p = 0usize;
        unsafe {
            while p < self.ops.len() {
                let opcode = self.ops[p];
                let start = p;
                p += 1;
                if std::env::var_os("OC_DBG_OPS").is_some() {
                    eprintln!("OCOP @{start:06} op={opcode}");
                }
                match opcode {
                    gl_opcode::SHADER_CREATE => {
                        let ty = rd_u16(self.ops, &mut p) as u32;
                        let id = rd_u32(self.ops, &mut p);
                        if std::env::var_os("OC_DBG_SHADER").is_some() {
                            eprintln!("OCGL SHADER_CREATE ty={ty:#x} id={id}");
                        }
                        ex.shader_stage.insert(id, ty);
                    }
                    gl_opcode::SHADER_SOURCE => {
                        let id = rd_u32(self.ops, &mut p);
                        let range = rd_u32(self.ops, &mut p);
                        let src = self.range_bytes(range)?.to_vec();
                        if std::env::var_os("OC_DBG_SHADER").is_some() {
                            eprintln!("OCGL SHADER_SOURCE id={id} len={}", src.len());
                        }
                        ex.shader_src.insert(id, src);
                    }
                    gl_opcode::COMPILE_SHADER => {
                        rd_u32(self.ops, &mut p);
                    }
                    gl_opcode::PROGRAM_CREATE => {
                        rd_u32(self.ops, &mut p);
                    }
                    gl_opcode::PROGRAM_ATTACH => {
                        let pid = rd_u32(self.ops, &mut p);
                        let sid = rd_u32(self.ops, &mut p);
                        if std::env::var_os("OC_DBG_SHADER").is_some() {
                            eprintln!(
                                "OCGL ATTACH pid={pid} sid={sid} stage={:?}",
                                ex.shader_stage.get(&sid)
                            );
                        }
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
                            ex.cur_program_key = None;
                            gl.use_program(None);
                        } else {
                            if std::env::var_os("OC_DBG_SHADER").is_some() {
                                eprintln!("OCGL USE pid={pid}");
                            }
                            let key = ex.program_key_for(gl, pid)?;
                            let entry = ex
                                .program_cache
                                .get(&key)
                                .ok_or_else(|| anyhow!("program {pid} 缓存缺失"))?;
                            state.program = Some(entry.program);
                            state.program_key = Some(key.clone());
                            ex.cur_program_key = Some(key);
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
                    }
                    gl_opcode::BUFFER_DELETE => {
                        let h = rd_u32(self.ops, &mut p);
                        ex.obj_buffers.remove(&h);
                    }
                    gl_opcode::VAO_DELETE => {
                        let h = rd_u32(self.ops, &mut p);
                        if std::env::var_os("OC_DBG_OPS2").is_some() {
                            eprintln!("OCVAO DELETE handle={h}");
                        }
                        ex.obj_vaos.remove(&h);
                        if ex.cur_vao == Some(h) {
                            ex.cur_vao = None;
                            ex.cur_element = None;
                        }
                        ex.vao_element.remove(&h);
                    }
                    gl_opcode::TEXTURE_DELETE => {
                        let h = rd_u32(self.ops, &mut p);
                        ex.obj_textures.remove(&h);
                        ex.client.tex_binds.retain(|(_, _, th)| *th != h);
                    }
                    gl_opcode::FRAMEBUFFER_DELETE => {
                        let h = rd_u32(self.ops, &mut p);
                        ex.obj_fbos.remove(&h);
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
                        let buf = if handle == NULL_HANDLE {
                            ex.cur_element = None;
                            None
                        } else {
                            ex.cur_element = Some(handle);
                            if target as u32 == glow::ELEMENT_ARRAY_BUFFER
                                && let Some(v) = ex.cur_vao
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
                        gl.bind_buffer(target as u32, buf);
                    }
                    gl_opcode::BUFFER_DATA => {
                        let target = rd_u16(self.ops, &mut p);
                        let usage = rd_u16(self.ops, &mut p);
                        let range = rd_u32(self.ops, &mut p);
                        let data = self.range_bytes(range)?;
                        let buf = ex
                            .bound_buffer(target as u32)
                            .and_then(|h| ex.obj_buffers.get(&h).copied())
                            .ok_or_else(|| anyhow!("BUFFER_DATA 无客户端绑定 buffer"))?;
                        gl.bind_buffer(target as u32, Some(buf));
                        gl.buffer_data_u8_slice(target as u32, data, usage as u32);
                    }
                    gl_opcode::BUFFER_DATA_NULL => {
                        let target = rd_u16(self.ops, &mut p);
                        let usage = rd_u16(self.ops, &mut p);
                        let size = rd_u32(self.ops, &mut p);
                        let buf = ex
                            .bound_buffer(target as u32)
                            .and_then(|h| ex.obj_buffers.get(&h).copied())
                            .ok_or_else(|| anyhow!("BUFFER_DATA_NULL 无客户端绑定 buffer"))?;
                        gl.bind_buffer(target as u32, Some(buf));
                        gl.buffer_data_size(target as u32, size as i32, usage as u32);
                    }
                    gl_opcode::VAO_CREATE => {
                        let handle = rd_u32(self.ops, &mut p);
                        if std::env::var_os("OC_DBG_OPS2").is_some() {
                            eprintln!("OCVAO CREATE handle={handle} new={}", !ex.obj_vaos.contains_key(&handle));
                        }
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
                        ex.cur_vao = if handle == NULL_HANDLE { None } else { Some(handle) };
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
                        if let Some(loc) = self.attrib_loc(ex, &state, name_range)? {
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
                        if let Some(loc) = self.attrib_loc(ex, &state, name_range)? {
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
                        gl.uniform_1_f32_slice(self.uniform_loc(ex, &state, range)?.as_ref(), &data);
                    }
                    gl_opcode::UNIFORM_NI => {
                        let range = rd_u32(self.ops, &mut p);
                        let count = rd_u32(self.ops, &mut p);
                        let data = self.tail_i32(&mut p, count)?;
                        gl.uniform_1_i32_slice(self.uniform_loc(ex, &state, range)?.as_ref(), &data);
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
                        let mut data = if has_data != 0 {
                            Some(self.range_bytes(range)?.to_vec())
                        } else {
                            None
                        };
                        let tex = ex
                            .bound_texture(target as u32)
                            .and_then(|th| ex.obj_textures.get(&th).copied())
                            .ok_or_else(|| anyhow!("TEX_IMAGE_2D 无客户端绑定纹理"))?;
                        let content_key = format!(
                            "{}|{internal}|{w}x{h}|{format}|{ty}|fy={}",
                            hash_bytes(data.as_deref().unwrap_or(&[])),
                            ex.unpack_flip_y
                        );
                        if !ex.texture_cache.contains(&content_key) {
                            if ex.unpack_flip_y {
                                if let Some(bytes) = data.as_mut() {
                                    flip_rows_vertical(bytes, h);
                                }
                            }
                            gl.bind_texture(target as u32, Some(tex));
                            gl.tex_image_2d(
                                target as u32, level, internal, w as i32, h as i32, 0,
                                format as u32, ty as u32, data.as_deref(),
                            );
                            ex.texture_cache.insert(content_key);
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
                            _ => gl.pixel_store_i32(pname as u32, value),
                        }
                    }
                    gl_opcode::DRAW_BUFFERS => {
                        let n = rd_u8(self.ops, &mut p) as usize;
                        let mut modes = [0u32; 8];
                        for m in modes.iter_mut().take(n) {
                            *m = rd_u32(self.ops, &mut p);
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
                            None
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
                        if ex.unpack_flip_y {
                            if let Some(bytes) = data.as_mut() {
                                flip_rows_vertical(bytes, h);
                            }
                        }
                        gl.bind_texture(target as u32, Some(tex));
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
                        // program pid 在 link/use 时解析（同 USE 路径的缓存）
                        let _ = (program, block_index, binding);
                    }
                    gl_opcode::BIND_ATTRIB_LOCATION => {
                        let program = rd_u32(self.ops, &mut p);
                        let index = rd_u32(self.ops, &mut p);
                        let name_range = rd_u32(self.ops, &mut p);
                        let name = String::from_utf8_lossy(self.range_bytes(name_range)?).to_string();
                        // link 前生效：three 在 link() 前调用。流内 program
                        // 的真 program 在 USE 才编译，所以这里先记下，USE
                        // 前的 link 时统一重放。
                        let _ = (program, index, name);
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
                        gl.draw_arrays(mode as u32, first, count);
                    }
                    gl_opcode::DRAW_ELEMENTS => {
                        let mode = rd_u16(self.ops, &mut p);
                        let count = rd_i32(self.ops, &mut p);
                        let ty = rd_u16(self.ops, &mut p);
                        let offset = rd_i32(self.ops, &mut p);
                        if std::env::var_os("OC_DBG_OPS2").is_some() {
                            let ele =
                                gl.get_parameter_i32(glow::ELEMENT_ARRAY_BUFFER_BINDING);
                            eprintln!(
                                "OCDE mode={mode:#x} count={count} ty={ty:#x} off={offset} prog={:?} ele_buf={ele}",
                                state.program.map(|p| p.0.get())
                            );
                        }
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
            .ok_or_else(|| anyhow!("uniform 命令出现在 USE_PROGRAM 之前"))?;
        GpuStreamExecutor::resolve_uniform_on(ex, self.gl, key, name)
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
        let vs = self.shader_src.get(&vid).ok_or_else(|| {
            if std::env::var_os("OC_DBG_SHADER").is_some() {
                let mut ks: Vec<u32> = self.shader_src.keys().copied().collect();
                ks.sort_unstable();
                let mut lens: Vec<(u32, usize)> =
                    self.shader_src.iter().map(|(k, v)| (*k, v.len())).collect();
                lens.sort_unstable();
                eprintln!(
                    "OCGL MISS pid={pid} vid={vid} keys={ks:?} lens={lens:?}"
                );
            }
            anyhow!("gpu stream: program {pid} 缺 vs 源")
        })?;
        let fs = self
            .shader_src
            .get(&fid)
            .ok_or_else(|| anyhow!("gpu stream: program {pid} 缺 fs 源"))?;
        let key = format!("{}|{}", hash_bytes(vs), hash_bytes(fs));
        if !self.program_cache.contains_key(&key) {
            let vs_src = desktop_glsl(std::str::from_utf8(vs)?, naga::ShaderStage::Vertex)?;
            let fs_src = desktop_glsl(std::str::from_utf8(fs)?, naga::ShaderStage::Fragment)?;
            let entry = compile_program_entry(gl, &vs_src, &fs_src)?;
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
) -> Result<ProgramEntry> {
    unsafe {
        let compile = |ty: u32, src: &str| -> Result<glow::Shader> {
            let sh = gl.create_shader(ty).map_err(|e| anyhow!("create_shader: {e}"))?;
            gl.shader_source(sh, src);
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
        Ok(ProgramEntry {
            program,
            attrib_names,
            attrib_loc: HashMap::new(),
            uniform_loc: HashMap::new(),
        })
    }
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

/// GLSL ES 300 → 桌面 GLSL 330 core。主路径走 naga fork 分支
/// feat/glsl-es300-to-desktop330 的官方链（front::glsl → IR → back::glsl）：
/// 预处理器宏、精度限定、重载解析全部由 IR 化吸收，输出驱动编译器直接吃
/// 的结构化源码——取代批次①"只换版本行"的词法降级。shader 源码在流里
/// 保持 ES 原文，web 端不受影响。
///
/// writer_flags 必须为空：默认的 ADJUST_COORDINATE_SPACE 是 Vulkan/Metal
/// NDC 翻转垫片，GL→GL 不需要，开了画面会上下镜像。
/// naga 吃不下的构造（front/validate/back 报错）回退到只换版本行的旧降级
/// 并记日志：缺口由日志驱动逐个消除，不出片不炸。
fn desktop_glsl(src: &str, stage: naga::ShaderStage) -> Result<String> {
    match naga_es300_to_desktop330(src, stage) {
        Ok(out) => Ok(out),
        Err(e) => {
            tracing::warn!("naga GLSL 转换失败，回退版本行降级: {e}");
            desktop_glsl_version_line_only(src)
        }
    }
}

fn naga_es300_to_desktop330(src: &str, stage: naga::ShaderStage) -> Result<String> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut parser = naga::front::glsl::Frontend::default();
    let module = parser
        .parse(
            &naga::front::glsl::Options {
                stage,
                defines: Default::default(),
            },
            src,
        )
        .map_err(|errs| {
            anyhow::anyhow!(
                "glsl-in: {}",
                errs.first().map(|e| e.to_string()).unwrap_or_default()
            )
        })?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|e| anyhow::anyhow!("validate: {e}"))?;
    let options = naga::back::glsl::Options {
        version: naga::back::glsl::Version::Desktop(330),
        writer_flags: naga::back::glsl::WriterFlags::empty(),
        binding_map: Default::default(),
        zero_initialize_workgroup_memory: false,
    };
    let pipeline = naga::back::glsl::PipelineOptions {
        shader_stage: stage,
        entry_point: "main".to_string(),
        multiview: None,
    };
    let mut out = String::new();
    let mut writer = naga::back::glsl::Writer::new(
        &mut out,
        &module,
        &info,
        &options,
        &pipeline,
        naga::proc::BoundsCheckPolicies::default(),
    )
    .map_err(|e| anyhow::anyhow!("glsl-out init: {e}"))?;
    writer.write().map_err(|e| anyhow::anyhow!("glsl-out: {e}"))?;
    Ok(out)
}

/// 桌面 GL（Windows WGL 的 compat profile）不认 GLSL ES 的 `#version 300 es`
/// 头——换成 `330 core`。WGL 上下文是 compatibility
/// profile（win32.rs：实测 NVIDIA 4.6 compat），拒绝 es profile 标识，但
/// 330 core 与 ES 3.00 语法同形（in/out、texture() 一致）——批次①的 vfx
/// tracer 即此路径过闸。
fn desktop_glsl_version_line_only(src: &str) -> Result<String> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    if let Some(pos) = src.find("#version") {
        let line_end = src[pos..]
            .find('\n')
            .map(|i| pos + i + 1)
            .unwrap_or(src.len());
        let version_line = &src[pos..line_end];
        if version_line.contains("es") {
            let mut out = String::with_capacity(src.len() + 4);
            out.push_str(&src[..pos]);
            out.push_str("#version 330 core\n");
            out.push_str(&src[line_end..]);
            return Ok(out);
        }
    }
    Ok(src.to_string())
}

/// 真 GL 反射：编译（版本行降级后）→ link → 问驱动 active uniform/attribute
/// 表 → 即毁。core 的 linkProgram 在 three 查询前同步走这里，结果与执行器
/// 的驱动语义一致（struct/数组展开、类型枚举全自动正确）。
///
/// 把反射器装进脚本 realm 的 store。反射用本线程常驻的独立离屏 GL 会话
/// （thread_local——反射只发生在脚本执行线程；主渲染路径每帧 make_current
/// 幂等重绑，临时切换安全）。
pub fn install_glsl_reflector(ctx: &crate::js_context::V8JsContext) {
    ctx.set_glsl_reflector(Box::new(|vs, fs| {
        thread_local! {
            static REFLECT: std::cell::RefCell<
                Option<(crate::gl_session::GlSession, Arc<glow::Context>)>,
            > = const { std::cell::RefCell::new(None) };
        }
        REFLECT.with(|cell| -> anyhow::Result<serde_json::Value> {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                let session = crate::gl_session::GlSession::offscreen()?;
                let gl = session.gl();
                *slot = Some((session, gl));
            }
            let (session, gl) = slot.as_ref().expect("reflect session");
            session.make_current()?;
            reflect_program_gl(gl, vs, fs)
        })
    }));
}

/// 装资产读取器 + 图像解码器（three FileLoader 的 fetch 垫与 GLTFLoader 的
/// createImageBitmap 垫的后端）。路径语义与声明式 `<image path=>` 一致：
/// 逻辑相对路径对 base_dir join。解码走 image crate（default features 含
/// png/jpeg，另开 webp）。
pub fn install_asset_bridge(ctx: &crate::js_context::V8JsContext, base_dir: &std::path::Path) {
    let root = base_dir.to_path_buf();
    ctx.set_asset_reader(Box::new(move |path| {
        let full = root.join(path);
        std::fs::read(&full).map_err(|e| anyhow::anyhow!("asset `{path}`: {e}"))
    }));
    ctx.set_image_decoder(Box::new(|bytes| {
        let img = image::load_from_memory(bytes)
            .map_err(|e| anyhow::anyhow!("image decode: {e}"))?;
        let rgba = img.to_rgba8().into_raw();
        Ok((img.width(), img.height(), rgba))
    }));
}

pub fn reflect_program_gl(
    gl: &Arc<glow::Context>,
    vs_es: &str,
    fs_es: &str,
) -> anyhow::Result<serde_json::Value> {
    unsafe {
        let compile = |ty: u32, src: &str| -> Result<glow::Shader> {
            let sh = gl.create_shader(ty).map_err(|e| anyhow!("create_shader: {e}"))?;
            gl.shader_source(sh, src);
            gl.compile_shader(sh);
            if !gl.get_shader_compile_status(sh) {
                let log = gl.get_shader_info_log(sh);
                bail!("shader 编译失败: {log}");
            }
            Ok(sh)
        };
        let vs = compile(
            glow::VERTEX_SHADER,
            &desktop_glsl(vs_es, naga::ShaderStage::Vertex)?,
        )?;
        let fs = compile(
            glow::FRAGMENT_SHADER,
            &desktop_glsl(fs_es, naga::ShaderStage::Fragment)?,
        )?;
        let program = gl
            .create_program()
            .map_err(|e| anyhow!("create_program: {e}"))?;
        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            bail!("反射 link 失败: {log}");
        }
        let mut uniforms = Vec::new();
        for i in 0..gl.get_active_uniforms(program) {
            if let Some(u) = gl.get_active_uniform(program, i) {
                uniforms.push(serde_json::json!({
                    "n": u.name, "t": u.utype, "s": u.size.max(1),
                }));
            }
        }
        let mut attributes = Vec::new();
        for i in 0..gl.get_active_attributes(program) {
            if let Some(a) = gl.get_active_attribute(program, i) {
                attributes.push(serde_json::json!({
                    "n": a.name, "t": a.atype, "s": a.size.max(1),
                }));
            }
        }
        gl.delete_program(program);
        Ok(serde_json::json!({ "uniforms": uniforms, "attributes": attributes }))
    }
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
