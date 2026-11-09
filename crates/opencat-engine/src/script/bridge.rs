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


/// 当前线程 GL 上下文的 RAII 现场：[`Self::save`] 记下 current，drop 时
/// 恢复。无线程绑定上下文时 save 成功、drop 不做任何事（与现状一致：
/// 本来就没有可恢复的东西）。
struct CurrentContextGuard {
    #[cfg(target_os = "windows")]
    saved: Option<crate::gpu::angle::CurrentBinding>,
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
                saved: crate::gpu::angle::CurrentBinding::save(),
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
        if let Some(binding) = self.saved.take()
            && let Err(e) = binding.restore()
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

thread_local! {
    /// 反射器常驻的独立离屏会话（反射只发生在脚本执行线程）。
    static REFLECT: std::cell::RefCell<
        Option<(crate::gpu::session::GlSession, Arc<glow::Context>)>,
    > = const { std::cell::RefCell::new(None) };
    /// 反射结果按 (vs, fs) 源码缓存：反射是源码的纯函数，html 页脚本每帧
    /// 重建 renderer、同一批 program 逐帧重新 link，真编译（ANGLE 走
    /// D3DCompile）每个几十毫秒。
    static REFLECT_CACHE: std::cell::RefCell<
        std::collections::HashMap<(String, String), serde_json::Value>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// 拆掉本线程的反射会话（进程退出前调用，见 `render::release_gl_sessions`）。
pub(crate) fn release_reflector_session() {
    let _ = REFLECT.try_with(|cell| cell.borrow_mut().take());
}

/// 把反射器装进脚本 realm 的 store。反射用本线程常驻的独立离屏 GL 会话
/// （thread_local——反射只发生在脚本执行线程）；每次反射 save → make
/// current → 反射 → RAII 恢复，主渲染路径的 current 不再被打断。
pub fn install_glsl_reflector(ctx: &crate::script::V8JsContext) {
    ctx.set_glsl_reflector(Box::new(|vs, fs| {
        let key = (vs.to_string(), fs.to_string());
        if let Some(hit) = REFLECT_CACHE.with(|c| c.borrow().get(&key).cloned()) {
            return Ok(hit);
        }
        let reflected = REFLECT.with(|cell| -> anyhow::Result<serde_json::Value> {
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
        })?;
        REFLECT_CACHE.with(|c| c.borrow_mut().insert(key, reflected.clone()));
        Ok(reflected)
    }));
}

/// 装资产读取器 + 图像解码器（three FileLoader 的 fetch 垫与 GLTFLoader 的
/// createImageBitmap 垫的后端）。路径语义与声明式 `<image path=>` 一致：
/// 逻辑相对路径对 base_dir join。解码走 image crate（default features 含
/// png/jpeg，另开 webp）。
pub fn install_asset_bridge(ctx: &crate::script::V8JsContext, base_dir: &std::path::Path) {
    let root = base_dir.to_path_buf();
    ctx.set_asset_reader(Box::new(move |path| {
        // 远程 URL（DRACOLoader 运行时拉解码器之类）：与外部脚本同一条
        // 抓取 + 磁盘缓存通道（`.oc-cache/net`），同输入逐字节可复现。
        if path.starts_with("https://") || path.starts_with("http://") {
            return crate::script::html::fetch_remote_bytes(&root, path).map(|(_, bytes)| bytes);
        }
        let full = root.join(path);
        std::fs::read(&full).map_err(|e| anyhow::anyhow!("asset `{path}`: {e}"))
    }));
    ctx.set_image_decoder(Box::new(|bytes| {
        // SVG 先走 resvg 光栅（`<img src=*.svg>` 位图语义，见 svg_raster
        // 模块注释）；image-rs 不认 svg 文本。预乘 RGBA 与 image-rs 的
        // to_rgba8 输出同底色系（透明区域全零）。
        if let Some((rgba, w, h)) = opencat_core::svg_raster::rasterize_svg(bytes, None) {
            return Ok((w, h, rgba));
        }
        let img = image::load_from_memory(bytes)
            .map_err(|e| anyhow::anyhow!("image decode: {e}"))?;
        let rgba = img.to_rgba8().into_raw();
        Ok((img.width(), img.height(), rgba))
    }));
    // 视频帧解码（three VideoTexture 的 face 像素源）：自持一个独立解码池，
    // 与渲染管线的 MediaContext 互不借用——管线池按渲染帧预取，JS 侧按
    // (asset, 媒体时刻) 惰性取帧；同一 mp4 两路解码在语料规模（数个短片段）
    // 下代价可忽略。帧缓存让逐帧重放的同时刻请求命中缓存。
    let pool_root = base_dir.to_path_buf();
    let pool = std::sync::Mutex::new(crate::media::MediaContext::new());
    ctx.set_video_frame_decoder(Box::new(move |path, time_secs| {
        let full = pool_root.join(path);
        let mut guard = pool.lock().expect("video frame pool poisoned");
        let (data, w, h, _cached) = guard.get_video_frame(
            &full,
            crate::media::VideoFrameRequest {
                composition_time_secs: time_secs,
                timing: Default::default(),
                quality: crate::media::VideoPreviewQuality::Exact,
                target_size: None,
            },
        )?;
        Ok((data.to_vec(), w, h))
    }));
}

/// 真 GL 反射：编译→link→按序枚举 active uniform/attribute/uniform 块。
/// GLSL ES 300 源直接喂 ANGLE（GLES3）上下文。
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
        // 反射会话是 ANGLE（GLES3）：GLSL ES 300 原文直接编译，不做桌面
        // 升级（WGL 时代的 naga 330 core 转换已随桌面 GL 退役）。
        let vs = compile(glow::VERTEX_SHADER, vs_es)?;
        let fs = compile(glow::FRAGMENT_SHADER, fs_es)?;
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
                // `l` 是驱动 location（浏览器 getAttribLocation 的返回值）。
                // 枚举序与之可以不等，core 侧必须拿真值——否则 three 的
                // vertexAttribPointer 会指到别的 attrib 上。
                let loc = gl.get_attrib_location(program, &a.name);
                attributes.push(serde_json::json!({
                    "n": a.name, "t": a.atype, "s": a.size.max(1), "l": loc,
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
