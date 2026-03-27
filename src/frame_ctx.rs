#[derive(Debug, Clone, Copy)]
pub struct FrameCtx {
    pub frame: u32,
    pub fps: u32,
    pub width: i32,
    pub height: i32,
    pub frames: u32,
}
