//! 最小复现：`new_external` 建出的 wgpu 设备析构后，进程是否退出。
//!
//! 跑法：`cargo run --release -p opencat-three --example external_gl_exit`

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("仅 Windows");
}

#[cfg(target_os = "windows")]
fn main() {
    if std::env::args().any(|a| a == "--with-skia") {
        probe_with_skia();
        return;
    }
    use wgpu::hal::api::Gles;

    let host = unsafe { create_host_gl_context() };
    println!("宿主 HGLRC = {:?}", host.glrc);

    let hal_instance = unsafe {
        let desc = wgpu::hal::InstanceDescriptor {
            name: "exit-probe",
            flags: wgpu::InstanceFlags::empty(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            telemetry: None,
            display: None,
        };
        <wgpu::hal::gles::Instance as wgpu::hal::Instance>::init(&desc).unwrap()
    };
    unsafe { host.make_current().unwrap() };

    let hal_adapter = unsafe {
        wgpu::hal::gles::Adapter::new_external(
            |name| load_gl_proc(name),
            wgpu::GlBackendOptions::default(),
        )
    }
    .unwrap();

    let instance = unsafe { wgpu::Instance::from_hal::<Gles>(hal_instance) };
    let adapter = unsafe { instance.create_adapter_from_hal::<Gles>(hal_adapter) };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("exit-probe"),
        ..Default::default()
    }))
    .unwrap();

    drop(queue);
    drop(device);
    drop(instance);
    println!("wgpu 侧已析构");
    drop(host);
    println!("宿主已析构，main 即将返回");
}

/// 诊断变体：是否 Skia 的 DirectContext 导致进程不退出。
#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn probe_with_skia() {
    use wgpu::hal::api::Gles;

    let host = unsafe { create_host_gl_context() };
    let hal_instance = unsafe {
        let desc = wgpu::hal::InstanceDescriptor {
            name: "exit-probe-skia",
            flags: wgpu::InstanceFlags::empty(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            telemetry: None,
            display: None,
        };
        <wgpu::hal::gles::Instance as wgpu::hal::Instance>::init(&desc).unwrap()
    };
    unsafe { host.make_current().unwrap() };
    let hal_adapter = unsafe {
        wgpu::hal::gles::Adapter::new_external(
            |name| load_gl_proc(name),
            wgpu::GlBackendOptions::default(),
        )
    }
    .unwrap();
    let instance = unsafe { wgpu::Instance::from_hal::<Gles>(hal_instance) };
    let adapter = unsafe { instance.create_adapter_from_hal::<Gles>(hal_adapter) };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("exit-probe-skia"),
        ..Default::default()
    }))
    .unwrap();

    let mut skia = skia_safe::gpu::direct_contexts::make_gl(
        skia_safe::gpu::interfaces::make_win().unwrap(),
        None,
    )
    .unwrap();

    // 合成目标纹理 + FBO + Skia surface（完整 PoC 的剩余部分）。
    let fbo_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("probe-target"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
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
            other => panic!("unexpected TextureInner: {other:?}"),
        }
    };
    let fbo = opencat_gpu_native::gl::create_framebuffer(
        opencat_gpu_native::NativeTexture::from_raw(fbo_tex_id as u64),
    ).unwrap();
    let brt = skia_safe::gpu::backend_render_targets::make_gl(
        (64, 64),
        0usize,
        8,
        skia_safe::gpu::gl::FramebufferInfo {
            fboid: fbo,
            format: skia_safe::gpu::gl::Format::RGBA8.into(),
            ..Default::default()
        },
    );
    let surface = skia_safe::gpu::surfaces::wrap_backend_render_target(
        &mut skia,
        &brt,
        skia_safe::gpu::SurfaceOrigin::TopLeft,
        skia_safe::ColorType::RGBA8888,
        None,
        None,
    )
    .unwrap();

    drop(surface);
    opencat_gpu_native::gl::delete_framebuffer(fbo);
    drop(fbo_tex);
    drop(skia);
    drop(queue);
    drop(device);
    drop(instance);
    println!("[skia] wgpu 侧已析构");
    drop(host);
    println!("[skia] 宿主已析构，main 即将返回");
}

#[cfg(target_os = "windows")]
struct HostGl {
    hwnd: windows_sys::Win32::Foundation::HWND,
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    glrc: windows_sys::Win32::Graphics::OpenGL::HGLRC,
}

#[cfg(target_os = "windows")]
impl HostGl {
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

#[cfg(target_os = "windows")]
unsafe fn create_host_gl_context() -> HostGl {
    use windows_sys::Win32::Graphics::Gdi::GetDC;
    use windows_sys::Win32::Graphics::OpenGL::{
        ChoosePixelFormat, PFD_DRAW_TO_WINDOW, PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA,
        PIXELFORMATDESCRIPTOR, SetPixelFormat, wglCreateContext,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExA, RegisterClassA, WNDCLASSA, WS_OVERLAPPEDWINDOW,
    };

    let class_name = b"OpenCatExitProbe\0";
    let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };
    let wc = WNDCLASSA {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        lpszClassName: class_name.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    unsafe { RegisterClassA(&wc) };
    let hwnd = unsafe {
        CreateWindowExA(
            0,
            class_name.as_ptr(),
            b"probe\0".as_ptr(),
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
    unsafe { SetPixelFormat(hdc, fmt, &pfd) };
    let glrc = unsafe { wglCreateContext(hdc) };
    let host = HostGl { hwnd, hdc, glrc };
    unsafe { host.make_current().unwrap() };
    host
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
