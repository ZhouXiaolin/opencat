//! PoC：wgpu (GL 后端) → Skia 的 GL 纹理共享。
//!
//! 验证三件事（全片 GPU 化的前提）：
//! 1. wgpu 的 GL 后端在 Windows 上确实走 WGL，`Device::as_hal::<Gles>()`
//!    能拿到 `HGLRC`；
//! 2. wgpu 的 `Texture` 能取到 GL texture id（`TextureInner::Texture{raw}`）；
//! 3. Skia 用**同一个** HGLRC 建 DirectContext 后，`borrow_texture_from`
//!    能正确采样该纹理——像素与写入值一致。
//!
//! 跑法（Windows，需 GL 后端）：
//! ```bash
//! WGPU_BACKEND=gl cargo run --release -p opencat-three --example gl_texture_share
//! ```

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("此 PoC 仅面向 Windows（WGL 路径）");
}

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    use skia_safe::gpu::SurfaceOrigin;
    use skia_safe::{AlphaType, ColorType, ImageInfo, gpu};
    use wgpu::hal::api::Gles;

    const W: u32 = 64;
    const H: u32 = 64;

    // ---- 1. wgpu：GL 后端建 device ----
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::GL;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| anyhow::anyhow!("no GL adapter: {e}"))?;
    println!("adapter: {:?}", adapter.get_info());

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("poc"),
        ..Default::default()
    }))?;

    // ---- 2. wgpu：建纹理并写入已知像素（纯红不透明）----
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("poc-shared"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let pixels: Vec<u8> = std::iter::repeat([255u8, 0, 0, 255])
        .take((W * H) as usize)
        .flatten()
        .collect();
    queue.write_texture(
        tex.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(W * 4),
            rows_per_image: Some(H),
        },
        wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([]);
    device.poll(wgpu::PollType::wait_indefinitely())?;

    // ---- 3. 取出 GL texture id 与 HGLRC ----
    let gl_tex_id = unsafe {
        let hal_tex = tex
            .as_hal::<Gles>()
            .ok_or_else(|| anyhow::anyhow!("texture has no Gles hal handle (backend isn't GL?)"))?;
        match &hal_tex.inner {
            wgpu::hal::gles::TextureInner::Texture { raw, .. } => {
                let id = raw.0.get();
                println!("GL texture id = {id}");
                id
            }
            other => anyhow::bail!("unexpected TextureInner: {other:?}"),
        }
    };

    let hglrc = unsafe {
        let hal_dev = device
            .as_hal::<Gles>()
            .ok_or_else(|| anyhow::anyhow!("device has no Gles hal handle"))?;
        hal_dev.context().raw_context()
    };
    println!("HGLRC = {hglrc:?}");
    if hglrc.is_null() {
        anyhow::bail!("raw_context() returned null");
    }

    // ---- 4. wgpu：再建一张纹理作为 Skia 的合成目标 ----
    // 注意：所有 wgpu 资源都必须在**取锁之前**建好。锁一旦持有，任何
    // wgpu 调用都会二次加锁 → 1 秒超时 panic（实测确认）。这条约束决定了
    // 真实管线的形态：wgpu 渲染 → 释放锁 → Skia 合成。
    let fbo_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("poc-skia-target"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let fbo_tex_id = unsafe {
        let hal_tex = fbo_tex.as_hal::<Gles>().unwrap();
        match &hal_tex.inner {
            wgpu::hal::gles::TextureInner::Texture { raw, .. } => raw.0.get(),
            other => anyhow::bail!("unexpected TextureInner: {other:?}"),
        }
    };
    println!("Skia target texture id = {fbo_tex_id}");

    // ---- 5. 取锁：当前线程 make current，之后**只做 Skia 操作** ----
    let hal_dev = unsafe {
        device
            .as_hal::<Gles>()
            .ok_or_else(|| anyhow::anyhow!("device has no Gles hal handle"))?
    };
    let _ctx_lock = hal_dev.context().lock();
    println!("wgpu GL context 已在当前线程 make current");

    let mut skia_ctx = {
        let interface = gpu::interfaces::make_win()
            .ok_or_else(|| anyhow::anyhow!("Skia make_win interface failed"))?;
        gpu::direct_contexts::make_gl(interface, None)
            .ok_or_else(|| anyhow::anyhow!("Skia make_gl failed"))?
    };

    // ---- 6. 零拷贝导入 wgpu 的 3D 输出纹理，画进 Skia 的 GPU surface ----
    let backend_tex = unsafe {
        let info = gpu::gl::TextureInfo {
            target: 0x0DE1,               // GL_TEXTURE_2D
            id: gl_tex_id,                // wgpu 的 GL texture id
            format: 0x8058,               // GL_RGBA8
            ..Default::default()
        };
        gpu::backend_textures::make_gl((W as i32, H as i32), gpu::Mipmapped::No, info, "poc")
    };
    let image = gpu::images::borrow_texture_from(
        &mut skia_ctx,
        &backend_tex,
        SurfaceOrigin::TopLeft,
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    )
    .ok_or_else(|| anyhow::anyhow!("borrow_texture_from failed"))?;

    // 自建 FBO 绑到 Skia 的合成目标纹理（GL 直接调用；wgpu 不提供 FBO 创建）。
    let fbo = unsafe { create_fbo(fbo_tex_id) };
    println!("FBO = {fbo}");
    if fbo == 0 {
        anyhow::bail!("glGenFramebuffers/glFramebufferTexture2D 失败");
    }

    let brt = gpu::backend_render_targets::make_gl(
        (W as i32, H as i32),
        0usize,
        8,
        gpu::gl::FramebufferInfo {
            fboid: fbo,
            format: gpu::gl::Format::RGBA8.into(),
            ..Default::default()
        },
    );
    let mut gpu_surface = gpu::surfaces::wrap_backend_render_target(
        &mut skia_ctx,
        &brt,
        SurfaceOrigin::TopLeft,
        ColorType::RGBA8888,
        None,
        None,
    )
    .ok_or_else(|| anyhow::anyhow!("wrap_backend_render_target failed"))?;

    {
        #[allow(invalid_reference_casting)]
        let canvas: &mut skia_safe::Canvas =
            unsafe { &mut *(gpu_surface.canvas() as *const _ as *mut skia_safe::Canvas) };
        canvas.clear(skia_safe::Color::BLUE);
        canvas.draw_image(&image, (0, 0), None);
    }
    skia_ctx.flush_and_submit_surface(&mut gpu_surface, Some(skia_safe::gpu::SyncCpu::Yes));

    let info = ImageInfo::new(
        (W as i32, H as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut out = vec![0u8; (W * H * 4) as usize];
    if !gpu_surface.read_pixels(&info, &mut out, W as usize * 4, (0, 0)) {
        anyhow::bail!("GPU surface read_pixels failed");
    }
    let c = ((H / 2) * W + W / 2) as usize * 4;
    println!("GPU surface center = {:?}", &out[c..c + 4]);
    println!("GPU surface corner = {:?}", &out[0..4]);
    if !(out[c] == 255 && out[c + 1] == 0 && out[c + 2] == 0) {
        anyhow::bail!("共享纹理未被正确采样: {:?}", &out[c..c + 4]);
    }
    // 纹理与画布同尺寸、draw_image 铺满，故角落也是红——这正是
    // "wgpu 纹理被完整采样"的证据（clear 的蓝已被覆盖）。
    if !(out[0] == 255 && out[1] == 0 && out[2] == 0) {
        anyhow::bail!("共享纹理未铺满合成目标: {:?}", &out[0..4]);
    }

    unsafe { delete_fbo(fbo) };
    println!("OK: wgpu GL texture 零拷贝导入 Skia GPU surface，像素正确");
    Ok(())
}

/// 建一个绑到指定 GL 纹理的 FBO，返回 FBO id（失败返回 0）。
#[cfg(target_os = "windows")]
unsafe fn create_fbo(texture_id: u32) -> u32 {
    use std::ffi::CString;
    // 经 opengl32.dll 解析 FBO 扩展入口（WGL 下 core 1.1 不含这些）。
    unsafe extern "system" {
        fn wglGetProcAddress(name: *const u8) -> *const std::ffi::c_void;
    }
    let get = |name: &str| -> *const std::ffi::c_void {
        let c = CString::new(name).unwrap();
        let p = unsafe { wglGetProcAddress(c.as_ptr() as *const u8) };
        let raw = p as usize;
        if matches!(raw, 0 | 1 | 2 | 3 | usize::MAX) {
            std::ptr::null()
        } else {
            p
        }
    };

    type GenFramebuffers = unsafe extern "system" fn(i32, *mut u32);
    type BindFramebuffer = unsafe extern "system" fn(u32, u32);
    type FramebufferTexture2D = unsafe extern "system" fn(u32, u32, u32, u32, i32);

    let gen_fn = get("glGenFramebuffers");
    let bind = get("glBindFramebuffer");
    let attach = get("glFramebufferTexture2D");
    if gen_fn.is_null() || bind.is_null() || attach.is_null() {
        return 0;
    }
    let gen_fn: GenFramebuffers = std::mem::transmute(gen_fn);
    let bind: BindFramebuffer = std::mem::transmute(bind);
    let attach: FramebufferTexture2D = std::mem::transmute(attach);

    let mut fbo: u32 = 0;
    unsafe { gen_fn(1, &mut fbo) };
    unsafe { bind(0x8D40, fbo) }; // GL_FRAMEBUFFER
    unsafe { attach(0x8D40, 0x8CE0, 0x0DE1, texture_id, 0) }; // COLOR_ATTACHMENT0, TEXTURE_2D
    unsafe { bind(0x8D40, 0) };
    fbo
}

#[cfg(target_os = "windows")]
unsafe fn delete_fbo(fbo: u32) {
    use std::ffi::CString;
    unsafe extern "system" {
        fn wglGetProcAddress(name: *const u8) -> *const std::ffi::c_void;
    }
    let c = CString::new("glDeleteFramebuffers").unwrap();
    let p = unsafe { wglGetProcAddress(c.as_ptr() as *const u8) };
    let raw = p as usize;
    if !matches!(raw, 0 | 1 | 2 | 3 | usize::MAX) {
        type Del = unsafe extern "system" fn(i32, *const u32);
        let del: Del = unsafe { std::mem::transmute(p) };
        unsafe { del(1, &fbo) };
    }
}
