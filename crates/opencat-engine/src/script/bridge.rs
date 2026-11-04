//! 脚本 realm 的宿主桥装配：GLSL 反射器 + 资产读取 / 图像解码。
//!
//! 反射器**借用现场而非窃取**：进入 reflect 前存下当前线程绑定的 GL
//! 上下文，反射结束 RAII 恢复。装配发生在主渲染会话建立之前
//! （`render_from_source` 装配桥 → `FrameRenderer::new` 才建会话），所以
//! 反射器拿不到主会话句柄；存/恢复 current 上下文是唯一与装配顺序无关
//! 的等价方案——主 GPU 会话与 CPU 合成路径的线程本地会话（`CPU_GL`）
//! 都被覆盖。此前反射器 make_current 后从不归还，消费端只能各自防御性
//! 重抢（原 `render_frame_inner` 的防御性 make_current 与 NVIDIA 段错误
//! 事故注释即为此而生，桥借还后删除）。
//!
//! 资产桥是纯闭包装配：core 不持有文件系统根，路径解析在这里 join
//! base_dir；图像解码走 image crate。

use std::sync::Arc;

use anyhow::{Result, anyhow};
use glow::HasContext;

use crate::gpu::stream_executor::desktop_glsl;

/// 当前线程 GL 上下文的 RAII 现场：[`Self::save`] 记下 current，drop 时
/// 恢复。无线程绑定上下文时 save 成功、drop 不做任何事（与现状一致：
/// 本来就没有可恢复的东西）。
struct CurrentContextGuard {
    #[cfg(target_os = "windows")]
    saved: Option<(
        windows_sys::Win32::Graphics::Gdi::HDC,
        windows_sys::Win32::Graphics::OpenGL::HGLRC,
    )>,
    #[cfg(target_os = "linux")]
    saved: Option<(
        khronos_egl::Display,
        khronos_egl::Surface,
        khronos_egl::Surface,
        khronos_egl::Context,
    )>,
}

impl CurrentContextGuard {
    fn save() -> Result<Self> {
        #[cfg(target_os = "windows")]
        {
            Ok(Self {
                saved: crate::gpu::win32::WglContext::get_current_context(),
            })
        }
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                saved: crate::gpu::egl::EglContext::get_current_context(),
            })
        }
    }
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        if let Some((hdc, glrc)) = self.saved.take()
            && let Err(e) = unsafe { crate::gpu::win32::WglContext::make_current_raw(hdc, glrc) }
        {
            tracing::warn!("GL 现场恢复失败: {e}");
        }
        #[cfg(target_os = "linux")]
        if let Some((display, draw, read, context)) = self.saved.take()
            && let Err(e) = crate::gpu::egl::EglContext::make_current_raw(display, draw, read, context)
        {
            tracing::warn!("GL 现场恢复失败: {e}");
        }
    }
}

/// 把反射器装进脚本 realm 的 store。反射用本线程常驻的独立离屏 GL 会话
/// （thread_local——反射只发生在脚本执行线程）；每次反射 save → make
/// current → 反射 → RAII 恢复，主渲染路径的 current 不再被打断。
pub fn install_glsl_reflector(ctx: &crate::script::V8JsContext) {
    ctx.set_glsl_reflector(Box::new(|vs, fs| {
        thread_local! {
            static REFLECT: std::cell::RefCell<
                Option<(crate::gpu::session::GlSession, Arc<glow::Context>)>,
            > = const { std::cell::RefCell::new(None) };
        }
        REFLECT.with(|cell| -> anyhow::Result<serde_json::Value> {
            let mut slot = cell.borrow_mut();
            // guard 活到闭包尾：首建分支（GlSession::offscreen 构造即
            // make current）与后续反射都被覆盖，离开时现场归位。
            let _guard = CurrentContextGuard::save()?;
            if slot.is_none() {
                let session = crate::gpu::session::GlSession::offscreen()?;
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
pub fn install_asset_bridge(ctx: &crate::script::V8JsContext, base_dir: &std::path::Path) {
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

/// 真 GL 反射：编译→link→按序枚举 active uniform/attribute/uniform 块。
/// GLSL（ES 方言）经 [`desktop_glsl`] 升到桌面 3.3。
fn reflect_program_gl(
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
                anyhow::bail!("shader 编译失败: {log}");
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
            anyhow::bail!("反射 link 失败: {log}");
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
        // uniform 块（WebGL2 的 sampler 之外，矩阵/屏尺寸等都走 std140 块）。
        // glow 没有块计数 API，只能按序号探名；越界探名会置 GL_INVALID_VALUE，
        // 探完清错。
        let mut blocks = Vec::new();
        for i in 0..16u32 {
            let name = gl.get_active_uniform_block_name(program, i);
            if name.is_empty() {
                break;
            }
            blocks.push(serde_json::json!({ "n": name, "i": i }));
        }
        gl.delete_program(program);
        while gl.get_error() != glow::NO_ERROR {}
        Ok(serde_json::json!({
            "uniforms": uniforms,
            "attributes": attributes,
            "blocks": blocks,
        }))
    }
}
