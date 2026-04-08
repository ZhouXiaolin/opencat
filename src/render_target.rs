use std::ffi::c_void;

use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackendKind {
    Skia,
}

pub type BeginFrameFn =
    unsafe fn(user_data: *mut c_void, width: i32, height: i32) -> Result<*mut c_void>;
pub type EndFrameFn = unsafe fn(user_data: *mut c_void) -> Result<()>;
pub type PresentFrameFn = unsafe fn(user_data: *mut c_void) -> Result<()>;
pub type ReadbackRgbaFn = unsafe fn(user_data: *mut c_void) -> Result<Vec<u8>>;

pub struct RenderTargetHandle {
    backend: RenderBackendKind,
    user_data: *mut c_void,
    begin_frame: BeginFrameFn,
    end_frame: EndFrameFn,
    present_frame: Option<PresentFrameFn>,
    readback_rgba: Option<ReadbackRgbaFn>,
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
            present_frame: None,
            readback_rgba: None,
        }
    }

    pub fn with_present_frame(mut self, present_frame: PresentFrameFn) -> Self {
        self.present_frame = Some(present_frame);
        self
    }

    pub fn with_readback_rgba(mut self, readback_rgba: ReadbackRgbaFn) -> Self {
        self.readback_rgba = Some(readback_rgba);
        self
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

    pub fn present_frame(&mut self) -> Result<()> {
        let present = self
            .present_frame
            .ok_or_else(|| anyhow!("render target does not support present"))?;
        // SAFETY: 回调函数与 user_data 由调用方配对提供并保证有效。
        unsafe { present(self.user_data) }
    }

    pub fn readback_rgba(&mut self) -> Result<Vec<u8>> {
        let readback = self
            .readback_rgba
            .ok_or_else(|| anyhow!("render target does not support RGBA readback"))?;
        // SAFETY: 回调函数与 user_data 由调用方配对提供并保证有效。
        unsafe { readback(self.user_data) }
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
