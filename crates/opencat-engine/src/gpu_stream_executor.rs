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
        if !outputs.is_empty() {
            restore_default_state(gl);
        }
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

        // 输出纹理 + FBO：流画到自己的附件上（合成按占位 z 序采样）。
        let out_tex = opencat_gpu_native::gl::create_texture_rgba8(gl, stream.width, stream.height)
            .ok_or_else(|| anyhow!("gpu stream: 输出纹理创建失败"))?;
        let out_fbo = opencat_gpu_native::gl::create_framebuffer(gl, out_tex)
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

        let entry = plan
            .generated_images
            .iter_mut()
            .find(|e| e.id == stream.image_id)
            .ok_or_else(|| {
                anyhow!(
                    "gpu stream '{}': 占位 id {} 不在本帧 plan（元素被裁剪？）",
                    stream.target_id,
                    stream.image_id.0
                )
            })?;

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
fn restore_default_state(gl: &Arc<glow::Context>) {
    unsafe {
        gl.use_program(None);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
        gl.bind_vertex_array(None);
        for unit in 0u32..8 {
            gl.active_texture(glow::TEXTURE0 + unit);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_sampler(glow::TEXTURE0 + unit, None);
        }
        gl.active_texture(glow::TEXTURE0);
        gl.bind_renderbuffer(glow::RENDERBUFFER, None);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
        gl.disable(glow::BLEND);
        gl.disable(glow::DEPTH_TEST);
        gl.disable(glow::SCISSOR_TEST);
        gl.disable(glow::STENCIL_TEST);
        gl.disable(glow::CULL_FACE);
        gl.disable(glow::POLYGON_OFFSET_FILL);
        gl.disable(glow::RASTERIZER_DISCARD);
        gl.depth_mask(true);
        gl.depth_func(glow::LESS);
        gl.stencil_mask(0xffff_ffff);
        gl.color_mask(true, true, true, true);
        gl.blend_equation(glow::FUNC_ADD);
        gl.blend_func(glow::ONE, glow::ZERO);
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
    last_bound_buffer: Option<glow::Buffer>,
    last_bound_texture: Option<glow::NativeTexture>,
    last_bound_texture_target: u32,
}

impl<'a> Replay<'a> {
    fn run(&mut self, ex: &mut GpuStreamExecutor) -> Result<ReplayState> {
        let gl = self.gl;
        let mut buffers: HashMap<u32, glow::Buffer> = HashMap::new();
        let mut vaos: HashMap<u32, glow::VertexArray> = HashMap::new();
        let mut textures: HashMap<u32, glow::NativeTexture> = HashMap::new();
        let mut fbos: HashMap<u32, glow::NativeFramebuffer> = HashMap::new();
        let mut shader_src: HashMap<u32, Vec<u8>> = HashMap::new();
        let mut shader_stage: HashMap<u32, u32> = HashMap::new();
        // pid → (vs handle, fs handle)
        let mut program_shaders: HashMap<u32, (Option<u32>, Option<u32>)> = HashMap::new();
        // 流内 pid → 内容 key（USE 时解析出 entry）。
        let mut program_key_by_pid: HashMap<u32, String> = HashMap::new();
        let mut state = ReplayState {
            program: None,
            program_key: None,
            last_bound_buffer: None,
            last_bound_texture: None,
            last_bound_texture_target: glow::TEXTURE_2D,
        };

        let mut p = 0usize;
        unsafe {
            while p < self.ops.len() {
                let opcode = self.ops[p];
                let start = p;
                p += 1;
                match opcode {
                    gl_opcode::SHADER_CREATE => {
                        let ty = rd_u16(self.ops, &mut p) as u32;
                        let id = rd_u32(self.ops, &mut p);
                        shader_stage.insert(id, ty);
                    }
                    gl_opcode::SHADER_SOURCE => {
                        let id = rd_u32(self.ops, &mut p);
                        let range = rd_u32(self.ops, &mut p);
                        let src = self.range_bytes(range)?.to_vec();
                        shader_src.insert(id, src);
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
                        let slot = program_shaders.entry(pid).or_default();
                        if shader_stage.get(&sid) == Some(&0x8B31) {
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
                            gl.use_program(None);
                        } else {
                            let key = ex.program_key_for(
                                gl,
                                pid,
                                &shader_src,
                                &program_shaders,
                                &mut program_key_by_pid,
                            )
                            ?;
                            let entry = ex
                                .program_cache
                                .get(&key)
                                .ok_or_else(|| anyhow!("program {pid} 缓存缺失"))?;
                            state.program = Some(entry.program);
                            state.program_key = Some(key);
                            // 状态机之外必须真绑定：漏了它 draw 落到固定
                            // 管线（compat profile 不报错、画默认色白三角）。
                            gl.use_program(Some(entry.program));
                        }
                    }
                    gl_opcode::SHADER_DELETE
                    | gl_opcode::PROGRAM_DELETE
                    | gl_opcode::BUFFER_DELETE
                    | gl_opcode::VAO_DELETE
                    | gl_opcode::TEXTURE_DELETE
                    | gl_opcode::FRAMEBUFFER_DELETE => {
                        // 生命周期命令：句柄级删除留给流内资源映射。
                        let _ = rd_u32(self.ops, &mut p);
                    }
                    gl_opcode::BUFFER_CREATE => {
                        let handle = rd_u32(self.ops, &mut p);
                        buffers
                            .entry(handle)
                            .or_insert_with(|| gl.create_buffer().expect("create_buffer"));
                    }
                    gl_opcode::BUFFER_BIND => {
                        let target = rd_u16(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let buf = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*buffers.entry(handle).or_insert_with(|| {
                                gl.create_buffer().expect("create_buffer")
                            }))
                        };
                        state.last_bound_buffer = buf;
                        gl.bind_buffer(target as u32, buf);
                    }
                    gl_opcode::BUFFER_DATA => {
                        let target = rd_u16(self.ops, &mut p);
                        let usage = rd_u16(self.ops, &mut p);
                        let range = rd_u32(self.ops, &mut p);
                        let data = self.range_bytes(range)?;
                        let buf = state
                            .last_bound_buffer
                            .ok_or_else(|| anyhow!("BUFFER_DATA 无绑定 buffer"))?;
                        gl.bind_buffer(target as u32, Some(buf));
                        gl.buffer_data_u8_slice(target as u32, data, usage as u32);
                    }
                    gl_opcode::BUFFER_DATA_NULL => {
                        let target = rd_u16(self.ops, &mut p);
                        let usage = rd_u16(self.ops, &mut p);
                        let size = rd_u32(self.ops, &mut p);
                        let buf = state
                            .last_bound_buffer
                            .ok_or_else(|| anyhow!("BUFFER_DATA_NULL 无绑定 buffer"))?;
                        gl.bind_buffer(target as u32, Some(buf));
                        gl.buffer_data_size(target as u32, size as i32, usage as u32);
                    }
                    gl_opcode::VAO_CREATE => {
                        let handle = rd_u32(self.ops, &mut p);
                        vaos.entry(handle).or_insert_with(|| {
                            gl.create_vertex_array().expect("create_vertex_array")
                        });
                    }
                    gl_opcode::VAO_BIND => {
                        let handle = rd_u32(self.ops, &mut p);
                        let vao = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*vaos.entry(handle).or_insert_with(|| {
                                gl.create_vertex_array().expect("create_vertex_array")
                            }))
                        };
                        gl.bind_vertex_array(vao);
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
                        textures.entry(handle).or_insert_with(|| {
                            gl.create_texture().expect("create_texture")
                        });
                    }
                    gl_opcode::TEXTURE_ACTIVE => {
                        let unit = rd_u8(self.ops, &mut p);
                        gl.active_texture(glow::TEXTURE0 + unit as u32);
                    }
                    gl_opcode::TEXTURE_BIND => {
                        let target = rd_u16(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let tex = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*textures.entry(handle).or_insert_with(|| {
                                gl.create_texture().expect("create_texture")
                            }))
                        };
                        if handle != NULL_HANDLE {
                            state.last_bound_texture = tex;
                            state.last_bound_texture_target = target as u32;
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
                        let data = if has_data != 0 {
                            Some(self.range_bytes(range)?.to_vec())
                        } else {
                            None
                        };
                        let tex = state
                            .last_bound_texture
                            .ok_or_else(|| anyhow!("TEX_IMAGE_2D 无绑定纹理"))?;
                        let content_key = format!(
                            "{}|{internal}|{w}x{h}|{format}|{ty}",
                            hash_bytes(data.as_deref().unwrap_or(&[]))
                        );
                        if !ex.texture_cache.contains(&content_key) {
                            gl.bind_texture(state.last_bound_texture_target, Some(tex));
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
                        gl.pixel_store_i32(pname as u32, value);
                    }
                    gl_opcode::FRAMEBUFFER_CREATE => {
                        let handle = rd_u32(self.ops, &mut p);
                        fbos.entry(handle).or_insert_with(|| {
                            gl.create_framebuffer().expect("create_framebuffer")
                        });
                    }
                    gl_opcode::BIND_FRAMEBUFFER => {
                        let target = rd_u16(self.ops, &mut p);
                        let handle = rd_u32(self.ops, &mut p);
                        let fbo = if handle == NULL_HANDLE {
                            None
                        } else {
                            Some(*fbos.entry(handle).or_insert_with(|| {
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
                            Some(*textures
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
                        gl.enable(rd_u16(self.ops, &mut p) as u32);
                    }
                    gl_opcode::DISABLE => {
                        gl.disable(rd_u16(self.ops, &mut p) as u32);
                    }
                    gl_opcode::DEPTH_FUNC => {
                        gl.depth_func(rd_u16(self.ops, &mut p) as u32);
                    }
                    gl_opcode::DEPTH_MASK => {
                        gl.depth_mask(rd_u8(self.ops, &mut p) != 0);
                    }
                    gl_opcode::BLEND_FUNC => {
                        let src = rd_u16(self.ops, &mut p);
                        let dst = rd_u16(self.ops, &mut p);
                        gl.blend_func(src as u32, dst as u32);
                    }
                    gl_opcode::BLEND_EQUATION => {
                        gl.blend_equation(rd_u16(self.ops, &mut p) as u32);
                    }
                    gl_opcode::BLEND_COLOR => {
                        let r = rd_f32(self.ops, &mut p);
                        let g = rd_f32(self.ops, &mut p);
                        let b = rd_f32(self.ops, &mut p);
                        let a = rd_f32(self.ops, &mut p);
                        gl.blend_color(r, g, b, a);
                    }
                    gl_opcode::CULL_FACE => {
                        gl.cull_face(rd_u16(self.ops, &mut p) as u32);
                    }
                    gl_opcode::FRONT_FACE => {
                        gl.front_face(rd_u16(self.ops, &mut p) as u32);
                    }
                    gl_opcode::CLEAR_DEPTH => {
                        gl.clear_depth_f32(rd_f32(self.ops, &mut p));
                    }
                    gl_opcode::CLEAR_STENCIL => {
                        gl.clear_stencil(rd_i32(self.ops, &mut p));
                    }
                    gl_opcode::POLYGON_OFFSET => {
                        let factor = rd_f32(self.ops, &mut p);
                        let units = rd_f32(self.ops, &mut p);
                        gl.polygon_offset(factor, units);
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
    fn program_key_for(
        &mut self,
        gl: &Arc<glow::Context>,
        pid: u32,
        shader_src: &HashMap<u32, Vec<u8>>,
        program_shaders: &HashMap<u32, (Option<u32>, Option<u32>)>,
        key_by_pid: &mut HashMap<u32, String>,
    ) -> Result<String> {
        if let Some(key) = key_by_pid.get(&pid) {
            return Ok(key.clone());
        }
        let (vid, fid) = program_shaders
            .get(&pid)
            .and_then(|s| Some((s.0?, s.1?)))
            .ok_or_else(|| anyhow!("gpu stream: program {pid} 缺 shader"))?;
        let vs = shader_src
            .get(&vid)
            .ok_or_else(|| anyhow!("gpu stream: program {pid} 缺 vs 源"))?;
        let fs = shader_src
            .get(&fid)
            .ok_or_else(|| anyhow!("gpu stream: program {pid} 缺 fs 源"))?;
        let key = format!("{}|{}", hash_bytes(vs), hash_bytes(fs));
        if !self.program_cache.contains_key(&key) {
            let vs_src = desktop_glsl(std::str::from_utf8(vs)?)?;
            let fs_src = desktop_glsl(std::str::from_utf8(fs)?)?;
            let entry = compile_program_entry(gl, &vs_src, &fs_src)?;
            self.program_cache.insert(key.clone(), entry);
        }
        key_by_pid.insert(pid, key.clone());
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
        Ok(ProgramEntry {
            program,
            attrib_names: scan_declarations(vs_src, "attribute"),
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

/// 桌面 GL（Windows WGL 的 compat profile）不认 GLSL ES 的 `#version 300 es`
/// 头——换成 `330 core`（3.3 core 与 300 es 在本命令面覆盖内的语法差异可
/// 忽略：in/out、texture()、precision 语句桌面 GLSL 全部接受）。Linux EGL
/// 上若是真 ES context 也兼容（驱动按版本行分派）。
/// TODO(batch-2): three 的完整 shader 面接 naga front::glsl → back::glsl 的
/// 自动转换，取代这一行改写。
fn desktop_glsl(src: &str) -> Result<std::borrow::Cow<'_, str>> {
    if let Some(rest) = src.strip_prefix('#') {
        let line_end = rest.find('\n').map(|i| i + 1).unwrap_or(rest.len());
        let version_line = &rest[..line_end];
        if version_line.contains("300 es") {
            let mut out = String::with_capacity(src.len() + 4);
            out.push('#');
            out.push_str(&version_line.replace("300 es", "330 core"));
            out.push_str(&rest[line_end..]);
            return Ok(std::borrow::Cow::Owned(out));
        }
    }
    Ok(std::borrow::Cow::Borrowed(src))
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
