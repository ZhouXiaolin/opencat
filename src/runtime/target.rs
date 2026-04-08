use std::ffi::c_void;

use anyhow::{Result, anyhow};

use crate::runtime::frame_view::RenderFrameView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderFrameViewKind {
    DrawContext2D,
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
pub type ResolveFrameViewFn =
    unsafe fn(user_data: *mut c_void, frame_surface: *mut c_void) -> Result<*mut c_void>;

pub struct RenderTargetHandle {
    frame_view_kind: RenderFrameViewKind,
    user_data: *mut c_void,
    begin_frame: BeginFrameFn,
    end_frame: EndFrameFn,
    resolve_frame_view: Option<ResolveFrameViewFn>,
    present_frame: Option<PresentFrameFn>,
    readback_rgba: Option<ReadbackRgbaFn>,
}

impl RenderTargetHandle {
    pub fn new(
        frame_view_kind: RenderFrameViewKind,
        user_data: *mut c_void,
        begin_frame: BeginFrameFn,
        end_frame: EndFrameFn,
    ) -> Self {
        Self {
            frame_view_kind,
            user_data,
            begin_frame,
            end_frame,
            resolve_frame_view: None,
            present_frame: None,
            readback_rgba: None,
        }
    }

    pub fn with_frame_view_resolver(mut self, resolve_frame_view: ResolveFrameViewFn) -> Self {
        self.resolve_frame_view = Some(resolve_frame_view);
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

    pub fn frame_view_kind(&self) -> RenderFrameViewKind {
        self.frame_view_kind
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

    pub(crate) fn resolve_frame_view(
        &mut self,
        frame_surface: FrameSurfaceHandle,
    ) -> Result<RenderFrameView> {
        let resolve = self
            .resolve_frame_view
            .ok_or_else(|| anyhow!("render target does not expose a frame view resolver"))?;
        // SAFETY: frame surface handle is created by this target's begin_frame callback.
        let view = unsafe { resolve(self.user_data, frame_surface.raw()) }?;
        RenderFrameView::new(self.frame_view_kind, view)
    }

    pub(crate) fn require_frame_view_kind(&self, expected: RenderFrameViewKind) -> Result<()> {
        if self.frame_view_kind == expected {
            return Ok(());
        }
        Err(anyhow!(
            "render target frame view {:?} is not compatible with renderer {:?}",
            self.frame_view_kind,
            expected
        ))
    }
}
