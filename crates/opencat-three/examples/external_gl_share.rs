//! PoC：把 wgpu 建到**宿主自己的** GL 上下文上，验证纹理零拷贝进 Skia。
//!
//! 与 `gl_texture_share` 的区别：那个 PoC 用 wgpu 自建的隐藏窗口上下文，
//! 本 PoC 自建一个 HGLRC（模拟 `opencat-see` 的窗口上下文），让 wgpu 加入它。
//! 这是"预览器直接播 GPU 纹理"的前提：纹理名只在同一 HGLRC 里通用。
//!
//! 验证四件事：
//! 1. `gles::Adapter::new_external` 能用宿主的 HGLRC 建出 hal 适配器；
//! 2. 经 `Instance::from_hal` + `create_adapter_from_hal` 接进 wgpu 安全层；
//! 3. 该 wgpu 设备渲出的纹理，其 GL 名在宿主 Skia 的 DirectContext 里可采样；
//! 4. 像素正确（红），证明零拷贝链路成立。
//!
//! 跑法（Windows，需 GL 后端）：
//! ```bash
//! cargo run --release -p opencat-three --example external_gl_share
//! ```

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("此 PoC 仅面向 Windows（WGL 路径）");
}

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    use std::ffi::c_void;

    use skia_safe::gpu::SurfaceOrigin;
    use skia_safe::{AlphaType, ColorType, ImageInfo, gpu};
    use wgpu::hal::api::Gles;

    const W: u32 = 64;
    const H: u32 = 64;

    // ---- 1. 宿主自建隐藏窗口 + HGLRC（模拟 see 的窗口上下文）----
    let host = unsafe { create_host_gl_context()? };
    println!("宿主 HGLRC = {:?}", host.glrc);

    // ---- 2. 建 hal instance ----
    // 注意顺序：gles 的 `Instance::init` 会自建隐藏窗口并在结尾
    // `unmake_current`，把宿主的上下文解绑。所以必须在它**之后**重新
    // 绑定宿主上下文，`new_external` 才能查到扩展。
    let hal_instance = unsafe {
        let desc = wgpu::hal::InstanceDescriptor {
            name: "opencat-external-poc",
            flags: wgpu::InstanceFlags::empty(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            telemetry: None,
            display: None,
        };
        <wgpu::hal::gles::Instance as wgpu::hal::Instance>::init(&desc)
            .map_err(|e| anyhow::anyhow!("hal instance init 失败: {e}"))?
    };
    unsafe { host.make_current()? };

    // ---- 3. 用宿主的 GL 入口建 hal 适配器（context 字段为 None，
    //        意味着 current 状态此后由宿主维护）----
    let hal_adapter = unsafe {
        let fun = |name: &str| -> *const c_void { load_gl_proc(name) };
        wgpu::hal::gles::Adapter::new_external(fun, wgpu::GlBackendOptions::default())
    }
    .ok_or_else(|| anyhow::anyhow!("new_external 失败（宿主上下文不可用？）"))?;
    println!("hal adapter: {:?}", hal_adapter.info);

    let instance = unsafe { wgpu::Instance::from_hal::<Gles>(hal_instance) };
    let adapter = unsafe { instance.create_adapter_from_hal::<Gles>(hal_adapter) };
    println!("adapter: {:?}", adapter.get_info());

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("external-poc"),
        ..Default::default()
    }))?;

    // ---- 3. wgpu 建纹理并写入已知像素（纯红不透明）----
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

    let gl_tex_id = unsafe {
        let hal_tex = tex
            .as_hal::<Gles>()
            .ok_or_else(|| anyhow::anyhow!("纹理没有 Gles hal 句柄"))?;
        match &hal_tex.inner {
            wgpu::hal::gles::TextureInner::Texture { raw, .. } => {
                let id = raw.0.get();
                println!("wgpu 纹理 GL id = {id}");
                id
            }
            other => anyhow::bail!("意外的 TextureInner: {other:?}"),
        }
    };

    // ---- 4. 宿主上下文 current 下建 Skia DirectContext，零拷贝导入 ----
    // new_external 把内部 context 置 None：current 状态由宿主维护，
    // 所以这里不能依赖 wgpu 的锁来 make_current。
    let mut skia_ctx = {
        let interface = gpu::interfaces::make_win()
            .ok_or_else(|| anyhow::anyhow!("Skia make_win interface 失败"))?;
        gpu::direct_contexts::make_gl(interface, None)
            .ok_or_else(|| anyhow::anyhow!("Skia make_gl 失败"))?
    };

    let backend_tex = unsafe {
        let info = gpu::gl::TextureInfo {
            target: 0x0DE1,
            id: gl_tex_id,
            format: 0x8058,
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
    .ok_or_else(|| anyhow::anyhow!("borrow_texture_from 失败"))?;

    // ---- 5. 合成到自建 FBO（绑一张新纹理），读回验证 ----
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
            other => anyhow::bail!("意外的 TextureInner: {other:?}"),
        }
    };

    let fbo = opencat_gpu_native::gl::create_framebuffer(
        opencat_gpu_native::NativeTexture::from_raw(fbo_tex_id as u64),
    )
        .ok_or_else(|| anyhow::anyhow!("create_framebuffer 失败"))?;
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
    .ok_or_else(|| anyhow::anyhow!("wrap_backend_render_target 失败"))?;

    {
        #[allow(invalid_reference_casting)]
        let canvas: &mut skia_safe::Canvas =
            unsafe { &mut *(gpu_surface.canvas() as *const _ as *mut skia_safe::Canvas) };
        canvas.clear(skia_safe::Color::BLUE);
        canvas.draw_image(&image, (0, 0), None);
    }
    skia_ctx.flush_and_submit_surface(&mut gpu_surface, Some(gpu::SyncCpu::Yes));

    let info = ImageInfo::new(
        (W as i32, H as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut out = vec![0u8; (W * H * 4) as usize];
    if !gpu_surface.read_pixels(&info, &mut out, W as usize * 4, (0, 0)) {
        anyhow::bail!("GPU surface read_pixels 失败");
    }
    let c = ((H / 2) * W + W / 2) as usize * 4;
    println!("中心像素 = {:?}", &out[c..c + 4]);
    if !(out[c] == 255 && out[c + 1] == 0 && out[c + 2] == 0) {
        anyhow::bail!("共享纹理未被正确采样: {:?}", &out[c..c + 4]);
    }
    if !(out[0] == 255 && out[1] == 0 && out[2] == 0) {
        anyhow::bail!("共享纹理未铺满合成目标: {:?}", &out[0..4]);
    }

    opencat_gpu_native::gl::delete_framebuffer(fbo);
    println!("OK: 外部 GL 上下文下 wgpu 纹理零拷贝进 Skia，像素正确");
    // 诊断：先让 wgpu 侧全部析构（此时宿主上下文仍存活且 current），
    // 最后才销毁宿主窗口/上下文——否则 teardown 顺序会挂住进程。
    drop(gpu_surface);
    drop(skia_ctx);
    drop(fbo_tex);
    drop(tex);
    drop(queue);
    drop(device);
    drop(instance);
    println!("wgpu 侧已析构");
    drop(host);
    println!("宿主上下文已销毁");
    // 诊断：main 返回后进程仍不退出（疑似 wgpu-hal 的 WGL 实例线程）。
    // 用显式退出区分「退出路径挂住」与「析构挂住」。
    println!("调用 process::exit");
    std::process::exit(0);
}

/// 宿主自建的隐藏窗口 + GL 上下文（模拟 `opencat-see` 的窗口上下文）。
#[cfg(target_os = "windows")]
struct HostGl {
    hwnd: windows_sys::Win32::Foundation::HWND,
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    glrc: windows_sys::Win32::Graphics::OpenGL::HGLRC,
}

#[cfg(target_os = "windows")]
impl HostGl {
    /// 把宿主上下文绑到当前线程。
    unsafe fn make_current(&self) -> anyhow::Result<()> {
        use windows_sys::Win32::Graphics::OpenGL::wglMakeCurrent;
        if unsafe { wglMakeCurrent(self.hdc, self.glrc) } == 0 {
            anyhow::bail!("wglMakeCurrent 失败");
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl Drop for HostGl {
    fn drop(&mut self) {
        use windows_sys::Win32::Graphics::Gdi::ReleaseDC;
        use windows_sys::Win32::Graphics::OpenGL::{wglDeleteContext, wglMakeCurrent};
        use windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow;
        unsafe {
            let _ = wglMakeCurrent(self.hdc, std::ptr::null_mut());
            let _ = wglDeleteContext(self.glrc);
            let _ = ReleaseDC(self.hwnd, self.hdc);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// 建一个隐藏窗口 + HGLRC 并 make current。
#[cfg(target_os = "windows")]
unsafe fn create_host_gl_context() -> anyhow::Result<HostGl> {
    use windows_sys::Win32::Graphics::Gdi::GetDC;
    use windows_sys::Win32::Graphics::OpenGL::{
        ChoosePixelFormat, PFD_DRAW_TO_WINDOW, PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA,
        PIXELFORMATDESCRIPTOR, SetPixelFormat, wglCreateContext,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExA, RegisterClassA, WNDCLASSA, WS_OVERLAPPEDWINDOW,
    };

    let class_name = b"OpenCatExternalGlPoc\0";
    let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };
    let wc = WNDCLASSA {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        lpszClassName: class_name.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    if unsafe { RegisterClassA(&wc) } == 0 {
        anyhow::bail!("RegisterClassA 失败");
    }
    let hwnd = unsafe {
        CreateWindowExA(
            0,
            class_name.as_ptr(),
            b"poc\0".as_ptr(),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            64,
            64,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        anyhow::bail!("CreateWindowExA 失败");
    }

    let hdc = unsafe { GetDC(hwnd) };
    let pfd = PIXELFORMATDESCRIPTOR {
        nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
        nVersion: 1,
        dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL,
        iPixelType: PFD_TYPE_RGBA as u8,
        cColorBits: 32,
        ..unsafe { std::mem::zeroed() }
    };
    let fmt = unsafe { ChoosePixelFormat(hdc, &pfd) };
    if fmt == 0 || unsafe { SetPixelFormat(hdc, fmt, &pfd) } == 0 {
        anyhow::bail!("ChoosePixelFormat/SetPixelFormat 失败");
    }
    let glrc = unsafe { wglCreateContext(hdc) };
    if glrc.is_null() {
        anyhow::bail!("wglCreateContext 失败");
    }
    let host = HostGl { hwnd, hdc, glrc };
    unsafe { host.make_current()? };
    Ok(host)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn wnd_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::DefWindowProcA;
    unsafe { DefWindowProcA(hwnd, msg, wparam, lparam) }
}

/// 解析 GL 入口（先 wglGetProcAddress，再 opengl32.dll 导出表）。
#[cfg(target_os = "windows")]
fn load_gl_proc(name: &str) -> *const std::ffi::c_void {
    use std::ffi::CString;
    use windows_sys::Win32::Graphics::OpenGL::wglGetProcAddress;
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};

    let Ok(c) = CString::new(name) else {
        return std::ptr::null();
    };
    if let Some(proc) = unsafe { wglGetProcAddress(c.as_ptr() as *const u8) } {
        let raw = proc as *const () as usize;
        if !matches!(raw, 0 | 1 | 2 | 3 | usize::MAX) {
            return proc as *const () as *const std::ffi::c_void;
        }
    }
    let opengl32 = unsafe { GetModuleHandleA(b"opengl32.dll\0".as_ptr()) };
    if opengl32.is_null() {
        return std::ptr::null();
    }
    unsafe { GetProcAddress(opengl32, c.as_ptr() as *const u8) }
        .map(|p| p as *const () as *const std::ffi::c_void)
        .unwrap_or(std::ptr::null())
}
