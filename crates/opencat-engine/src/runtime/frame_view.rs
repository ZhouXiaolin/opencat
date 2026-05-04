use std::ffi::c_void;

use anyhow::{Result, anyhow};

use crate::host::runtime::target::RenderFrameViewKind;

#[derive(Clone, Copy)]
pub(crate) struct RenderFrameView {
    kind: RenderFrameViewKind,
    raw: *mut c_void,
}

impl RenderFrameView {
    pub(crate) fn new(kind: RenderFrameViewKind, raw: *mut c_void) -> Result<Self> {
        if raw.is_null() {
            return Err(anyhow!("render frame view resolver returned null view"));
        }
        Ok(Self { kind, raw })
    }

    pub(crate) fn kind(self) -> RenderFrameViewKind {
        self.kind
    }

    pub(crate) fn raw(self) -> *mut c_void {
        self.raw
    }
}
