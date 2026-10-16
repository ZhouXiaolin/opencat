//! WGL 上下文引导（Windows）。
//!
//! 宿主拿到窗口后在这里建 GL 上下文。这段代码原先在预览器二进制里，
//! 约 230 行裸 win32；收进来之后宿主只剩「窗口 → 上下文」一次调用。
//!
//! 这里用 `windows-sys` 而不是手写 `extern`：`PIXELFORMATDESCRIPTOR` 有
//! 40 字节、十几个字段，手写布局容易错。`gl` 模块仍用手写 `extern`——那里
//! 只有几个函数指针，且已经过实测。

use std::ffi::{CString, c_void};

use anyhow::{Result, anyhow};
use windows_sys::Win32::{
    Foundation::{FreeLibrary, HMODULE, HWND},
    Graphics::{
        Gdi::{GetDC, HDC, ReleaseDC},
        OpenGL::{
            ChoosePixelFormat, HGLRC, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE,
            PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR, SetPixelFormat, SwapBuffers,
            wglCreateContext, wglDeleteContext, wglGetProcAddress, wglMakeCurrent,
        },
    },
    System::LibraryLoader::{GetProcAddress, LoadLibraryA},
};

/// 一个窗口上的 WGL 上下文。
///
/// [`Self::from_window`] 建好即 make current。drop 时解绑、删上下文、
/// 还 DC、卸 opengl32。
///
/// 调用线程必须自己保证 make current——wgpu 的外部上下文模式不再代劳
/// （见 `wgpu::hal::gles::Adapter::new_external`）。hal 的 GL 实例初始化
/// 会在结尾 unmake，所以初始化之后要再调一次 [`Self::make_current`]。
pub struct WglContext {
    hwnd: HWND,
    hdc: HDC,
    glrc: HGLRC,
    opengl32: HMODULE,
}

impl WglContext {
    /// 在 `hwnd` 上建 GL 上下文并 make current。
    ///
    /// `hwnd` 是裸窗口句柄（`raw-window-handle` 的 `Win32WindowHandle::hwnd`）。
    pub fn from_window(hwnd: *mut c_void) -> Result<Self> {
        if hwnd.is_null() {
            return Err(anyhow!("Win32 window handle is null"));
        }

        let hdc = unsafe { GetDC(hwnd) };
        if hdc.is_null() {
            return Err(anyhow!("GetDC returned null for Win32 window"));
        }

        let pixel_format_descriptor = PIXELFORMATDESCRIPTOR {
            nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
            nVersion: 1,
            dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
            iPixelType: PFD_TYPE_RGBA,
            cColorBits: 32,
            cAlphaBits: 8,
            cDepthBits: 24,
            cStencilBits: 8,
            iLayerType: PFD_MAIN_PLANE as u8,
            ..Default::default()
        };

        let pixel_format = unsafe { ChoosePixelFormat(hdc, &pixel_format_descriptor) };
        if pixel_format == 0 {
            unsafe { ReleaseDC(hwnd, hdc) };
            return Err(anyhow!("ChoosePixelFormat failed for Win32 OpenGL window"));
        }

        if unsafe { SetPixelFormat(hdc, pixel_format, &pixel_format_descriptor) } == 0 {
            unsafe { ReleaseDC(hwnd, hdc) };
            return Err(anyhow!("SetPixelFormat failed for Win32 OpenGL window"));
        }

        let glrc = unsafe { wglCreateContext(hdc) };
        if glrc.is_null() {
            unsafe { ReleaseDC(hwnd, hdc) };
            return Err(anyhow!("wglCreateContext failed for Win32 window"));
        }

        if unsafe { wglMakeCurrent(hdc, glrc) } == 0 {
            unsafe {
                wglDeleteContext(glrc);
                ReleaseDC(hwnd, hdc);
            }
            return Err(anyhow!(
                "wglMakeCurrent failed while creating Win32 context"
            ));
        }

        let opengl32 = unsafe { LoadLibraryA(c"opengl32.dll".as_ptr() as *const u8) };
        if opengl32.is_null() {
            unsafe {
                wglMakeCurrent(hdc, std::ptr::null_mut());
                wglDeleteContext(glrc);
                ReleaseDC(hwnd, hdc);
            }
            return Err(anyhow!("LoadLibraryA(opengl32.dll) failed"));
        }

        Ok(Self {
            hwnd,
            hdc,
            glrc,
            opengl32,
        })
    }

    /// 把本上下文绑到当前线程。
    pub fn make_current(&self) -> Result<()> {
        if unsafe { wglMakeCurrent(self.hdc, self.glrc) } == 0 {
            return Err(anyhow!("wglMakeCurrent failed for Win32 context"));
        }
        Ok(())
    }

    /// 解析一个 GL 入口点。
    ///
    /// 先问 `wglGetProcAddress`（扩展与新版本），再问 opengl32.dll 的导出表
    /// （GL 1.1 的老函数不在前者里）。前者的哨兵返回值必须过滤——规范允许
    /// 它返回 1/2/3/`usize::MAX`。
    pub fn load_proc(&self, name: &str) -> *const c_void {
        let Ok(name) = CString::new(name) else {
            return std::ptr::null();
        };

        if let Some(proc) = unsafe { wglGetProcAddress(name.as_ptr() as *const u8) } {
            let raw = proc as *const () as usize;
            if !matches!(raw, 1 | 2 | 3 | usize::MAX) {
                return proc as *const () as *const c_void;
            }
        }

        unsafe { GetProcAddress(self.opengl32, name.as_ptr() as *const u8) }
            .map(|proc| proc as *const () as *const c_void)
            .unwrap_or(std::ptr::null())
    }

    /// 交换前后缓冲（双缓冲窗口）。
    pub fn swap_buffers(&self) -> Result<()> {
        if unsafe { SwapBuffers(self.hdc) } == 0 {
            return Err(anyhow!("SwapBuffers failed for Win32 window"));
        }
        Ok(())
    }
}

impl Drop for WglContext {
    fn drop(&mut self) {
        unsafe {
            wglMakeCurrent(self.hdc, std::ptr::null_mut());
            if !self.glrc.is_null() {
                wglDeleteContext(self.glrc);
            }
            if !self.hdc.is_null() {
                ReleaseDC(self.hwnd, self.hdc);
            }
            if !self.opengl32.is_null() {
                FreeLibrary(self.opengl32);
            }
        }
    }
}
