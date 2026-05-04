#[derive(Debug, Clone, Copy)]
pub struct FrameCtx {
    pub frame: u32,
    pub fps: u32,
    pub width: i32,
    pub height: i32,
    pub frames: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScriptFrameCtx {
    pub(crate) frame: u32,
    pub(crate) total_frames: u32,
    pub(crate) current_frame: u32,
    pub(crate) scene_frames: u32,
}

impl ScriptFrameCtx {
    pub(crate) fn global(frame_ctx: &FrameCtx) -> Self {
        Self {
            frame: frame_ctx.frame,
            total_frames: frame_ctx.frames,
            current_frame: frame_ctx.frame,
            scene_frames: frame_ctx.frames,
        }
    }

    pub(crate) fn for_segment(frame_ctx: &FrameCtx, start_frame: u32, scene_frames: u32) -> Self {
        let max_local_frame = scene_frames.saturating_sub(1);
        let current_frame = frame_ctx
            .frame
            .saturating_sub(start_frame)
            .min(max_local_frame);
        Self {
            frame: frame_ctx.frame,
            total_frames: frame_ctx.frames,
            current_frame,
            scene_frames,
        }
    }
}
