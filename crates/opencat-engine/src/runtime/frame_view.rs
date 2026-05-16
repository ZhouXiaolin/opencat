use std::ffi::c_void;

use anyhow::{Result, anyhow};

#[derive(Clone, Copy)]
pub(crate) struct RenderFrameView {
    raw: *mut c_void,
}

impl RenderFrameView {
    pub(crate) fn new(raw: *mut c_void) -> Result<Self> {
        if raw.is_null() {
            return Err(anyhow!("render frame view resolver returned null view"));
        }
        Ok(Self { raw })
    }

    pub(crate) fn raw(self) -> *mut c_void {
        self.raw
    }
}
