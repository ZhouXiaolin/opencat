//! 按核心 [`opencat_core::AudioPlan`] 预混整条音轨。
//!
//! AudioPlan 是**唯一的**合成级权威输出——段时序只来自
//! `CompositionInfo::audio_plan`（core 的 `collect_audio_plan` 推导）；
//! engine 只解码、混音，绝不重走合成树推导自己的段偏移。plan 已锚定微秒
//! 精度，engine 把 `start/end_micros` 直接换算成采样帧写入混音缓冲——
//! 不存在第二次时间轴几何推导。

use anyhow::{Result, anyhow};

use opencat_core::frame_ctx::duration_secs_to_frames;

use super::decode::{AudioTrack, decode_audio_to_f32_stereo};
use crate::EnginePipeline;

/// Premix the whole composition audio track.
/// Shared by `render_from_source` and `opencat view`.
pub fn build_audio_track_from_pipeline(pipeline: &EnginePipeline) -> Result<Option<AudioTrack>> {
    let info = pipeline.info();
    let plan = &info.audio_plan;
    if plan.segments.is_empty() {
        return Ok(None);
    }

    let mut mixed_samples = Vec::new();
    let sample_rate: u32 = 48_000;
    let channels: u16 = 2;
    let frame_count = duration_secs_to_frames(info.duration, info.fps);
    let total_sample_frames = ((frame_count as u64 * sample_rate as u64) / info.fps as u64) as usize;
    mixed_samples.resize(total_sample_frames * channels as usize, 0.0f32);

    for seg in &plan.segments {
        let handle = pipeline
            .loader()
            .handle(&seg.asset)
            .ok_or_else(|| anyhow!("audio asset {:?} not found in loader", seg.asset))?;
        let path = handle
            .local_path()
            .ok_or_else(|| anyhow!("audio {:?}: local_path required", seg.asset))?;
        let clip = decode_audio_to_f32_stereo(path, sample_rate)?;
        let start_us = seg.start_micros().0;
        let end_us = seg.end_micros().0;
        let start_sample = ((start_us * sample_rate as u64) / 1_000_000) as usize;
        let end_sample = ((end_us * sample_rate as u64) / 1_000_000) as usize;
        let copy_frames = end_sample
            .saturating_sub(start_sample)
            .min(clip.sample_frames());
        for i in 0..copy_frames {
            let dst = (start_sample + i) * channels as usize;
            let src = i * channels as usize;
            if dst + 1 < mixed_samples.len() && src + 1 < clip.samples.len() {
                mixed_samples[dst] += clip.samples[src];
                mixed_samples[dst + 1] += clip.samples[src + 1];
            }
        }
    }

    for s in &mut mixed_samples {
        *s = s.clamp(-1.0, 1.0);
    }

    Ok(Some(AudioTrack::new(sample_rate, channels, mixed_samples)))
}
