use std::ffi::c_void;

use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSurfaceKind {
    Canvas,
}

#[derive(Clone, Copy)]
pub(crate) struct FrameSurfaceHandle {
    raw: *mut c_void,
}

impl FrameSurfaceHandle {
    pub(crate) fn new(raw: *mut c_void) -> Result<Self> {
        if raw.is_null() {
            return Err(anyhow!(
                "render target begin_frame returned null frame surface"
            ));
        }
        Ok(Self { raw })
    }

    pub(crate) fn raw(self) -> *mut c_void {
        self.raw
    }
}

pub type BeginFrameFn =
    unsafe fn(user_data: *mut c_void, width: i32, height: i32) -> Result<*mut c_void>;
pub type EndFrameFn = unsafe fn(user_data: *mut c_void) -> Result<()>;
pub type PresentFrameFn = unsafe fn(user_data: *mut c_void) -> Result<()>;
pub type ReadbackRgbaFn = unsafe fn(user_data: *mut c_void) -> Result<Vec<u8>>;
pub type ResolveSurfaceViewFn =
    unsafe fn(user_data: *mut c_void, frame_surface: *mut c_void) -> Result<*mut c_void>;

pub struct RenderTargetHandle {
    surface_kind: RenderSurfaceKind,
    user_data: *mut c_void,
    begin_frame: BeginFrameFn,
    end_frame: EndFrameFn,
    resolve_surface_view: Option<ResolveSurfaceViewFn>,
    present_frame: Option<PresentFrameFn>,
    readback_rgba: Option<ReadbackRgbaFn>,
}

impl RenderTargetHandle {
    pub fn new(
        surface_kind: RenderSurfaceKind,
        user_data: *mut c_void,
        begin_frame: BeginFrameFn,
        end_frame: EndFrameFn,
    ) -> Self {
        Self {
            surface_kind,
            user_data,
            begin_frame,
            end_frame,
            resolve_surface_view: None,
            present_frame: None,
            readback_rgba: None,
        }
    }

    pub fn with_surface_view_resolver(
        mut self,
        resolve_surface_view: ResolveSurfaceViewFn,
    ) -> Self {
        self.resolve_surface_view = Some(resolve_surface_view);
        self
    }

    pub fn with_present_frame(mut self, present_frame: PresentFrameFn) -> Self {
        self.present_frame = Some(present_frame);
        self
    }

    pub fn with_readback_rgba(mut self, readback_rgba: ReadbackRgbaFn) -> Self {
        self.readback_rgba = Some(readback_rgba);
        self
    }

    pub fn surface_kind(&self) -> RenderSurfaceKind {
        self.surface_kind
    }

    pub(crate) fn begin_frame_surface(
        &mut self,
        width: i32,
        height: i32,
    ) -> Result<FrameSurfaceHandle> {
        // SAFETY: 回调函数与 user_data 由调用方配对提供并保证有效。
        let raw = unsafe { (self.begin_frame)(self.user_data, width, height) }?;
        FrameSurfaceHandle::new(raw)
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

    pub(crate) fn resolve_surface_view(
        &mut self,
        frame_surface: FrameSurfaceHandle,
    ) -> Result<*mut c_void> {
        let resolve = self
            .resolve_surface_view
            .ok_or_else(|| anyhow!("render target does not expose a frame surface resolver"))?;
        // SAFETY: frame surface handle is created by this target's begin_frame callback.
        let view = unsafe { resolve(self.user_data, frame_surface.raw()) }?;
        if view.is_null() {
            return Err(anyhow!(
                "render target frame surface resolver returned null view"
            ));
        }
        Ok(view)
    }

    pub(crate) fn require_surface_kind(&self, expected: RenderSurfaceKind) -> Result<()> {
        if self.surface_kind == expected {
            return Ok(());
        }
        Err(anyhow!(
            "render target surface {:?} is not compatible with renderer {:?}",
            self.surface_kind,
            expected
        ))
    }
}
