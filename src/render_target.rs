use std::ffi::c_void;

use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackendKind {
    Skia,
}

pub type BeginFrameFn =
    unsafe fn(user_data: *mut c_void, width: i32, height: i32) -> Result<*mut c_void>;
pub type EndFrameFn = unsafe fn(user_data: *mut c_void) -> Result<()>;

pub struct RenderTargetHandle {
    backend: RenderBackendKind,
    user_data: *mut c_void,
    begin_frame: BeginFrameFn,
    end_frame: EndFrameFn,
}

impl RenderTargetHandle {
    pub fn new(
        backend: RenderBackendKind,
        user_data: *mut c_void,
        begin_frame: BeginFrameFn,
        end_frame: EndFrameFn,
    ) -> Self {
        Self {
            backend,
            user_data,
            begin_frame,
            end_frame,
        }
    }

    pub fn backend(&self) -> RenderBackendKind {
        self.backend
    }

    pub(crate) fn begin_frame(&mut self, width: i32, height: i32) -> Result<*mut c_void> {
        // SAFETY: 回调函数与 user_data 由调用方配对提供并保证有效。
        unsafe { (self.begin_frame)(self.user_data, width, height) }
    }

    pub(crate) fn end_frame(&mut self) -> Result<()> {
        // SAFETY: 回调函数与 user_data 由调用方配对提供并保证有效。
        unsafe { (self.end_frame)(self.user_data) }
    }

    pub(crate) fn require_skia_backend(&self) -> Result<()> {
        if self.backend == RenderBackendKind::Skia {
            return Ok(());
        }
        Err(anyhow!(
            "render target backend is not compatible with current renderer"
        ))
    }
}
