//! Engine-side audio decode / mix.
//!
//! Segment timing comes exclusively from core [`opencat_core::AudioPlan`].
//! This module never walks the composition tree to invent scene/timeline offsets.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use opencat_core::ir::asset_id::AssetId;
use opencat_core::media::AudioPlan;
use opencat_core::time::timestamp_micros_to_secs;

use crate::media::decode::{AudioTrack, decode_audio_to_f32_stereo};

pub(crate) const AUDIO_SAMPLE_RATE: u32 = 48_000;
pub(crate) const AUDIO_CHANNELS: u16 = 2;
const DEFAULT_AUDIO_CHUNK_FRAMES: usize = 2048;

#[derive(Clone, Debug)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl AudioBuffer {
    fn silence(sample_frames: usize) -> Self {
        Self {
            sample_rate: AUDIO_SAMPLE_RATE,
            channels: AUDIO_CHANNELS,
            samples: vec![0.0; sample_frames * AUDIO_CHANNELS as usize],
        }
    }
}

/// Cached decode results keyed by canonical audio [`AssetId`].
#[derive(Default)]
pub struct DecodedAudioCache {
    decoded: HashMap<AssetId, AudioTrack>,
}

impl DecodedAudioCache {
    fn get_or_decode<'a>(
        &'a mut self,
        path_store: &crate::resource::AssetPathStore,
        asset: &AssetId,
    ) -> Result<&'a AudioTrack> {
        if !self.decoded.contains_key(asset) {
            let path = path_store
                .path(asset)
                .ok_or_else(|| anyhow!("missing cached audio asset for {}", asset.key))?;
            let clip = decode_audio_to_f32_stereo(path, AUDIO_SAMPLE_RATE)?;
            self.decoded.insert(asset.clone(), clip);
        }
        Ok(self
            .decoded
            .get(asset)
            .expect("decoded audio clip should exist"))
    }
}

/// Optional host-side stash of the composition audio plan. Core recomputes the
/// plan when the pipeline opens; this cache is only for engine playback helpers.
#[derive(Default)]
pub struct AudioIntervalCache {
    pub plan: Option<AudioPlan>,
}

impl AudioIntervalCache {
    pub fn set_plan(&mut self, plan: AudioPlan) {
        self.plan = Some(plan);
    }

    pub fn plan(&self) -> Option<&AudioPlan> {
        self.plan.as_ref()
    }
}

/// Premix the whole composition from a core [`AudioPlan`].
#[allow(dead_code)] // available for chunked/offline paths; export uses build_audio_track_from_pipeline
pub(crate) fn build_audio_track(
    plan: &AudioPlan,
    composition_frames: u32,
    composition_fps: u32,
    path_store: &crate::resource::AssetPathStore,
    decoded: &mut DecodedAudioCache,
) -> Result<Option<AudioTrack>> {
    if plan.segments.is_empty() {
        return Ok(None);
    }

    let total_sample_frames =
        frame_to_audio_sample_frames(composition_frames, composition_fps, AUDIO_SAMPLE_RATE);
    let mut mixed = Vec::with_capacity(total_sample_frames * AUDIO_CHANNELS as usize);

    let mut start_sample_frame = 0;
    while start_sample_frame < total_sample_frames {
        let chunk_sample_frames =
            (total_sample_frames - start_sample_frame).min(DEFAULT_AUDIO_CHUNK_FRAMES);
        let chunk = render_audio_chunk_from_plan(
            path_store,
            plan,
            decoded,
            start_sample_frame,
            chunk_sample_frames,
        )?;
        mixed.extend_from_slice(&chunk.samples);
        start_sample_frame += chunk_sample_frames;
    }

    Ok(Some(AudioTrack::new(
        AUDIO_SAMPLE_RATE,
        AUDIO_CHANNELS,
        mixed,
    )))
}

/// Mix a time slice starting at `start_time_secs` for `sample_frames` frames.
#[allow(dead_code)]
pub(crate) fn render_audio_chunk(
    plan: &AudioPlan,
    composition_frames: u32,
    composition_fps: u32,
    path_store: &crate::resource::AssetPathStore,
    decoded: &mut DecodedAudioCache,
    start_time_secs: f64,
    sample_frames: usize,
) -> Result<Option<AudioBuffer>> {
    if plan.segments.is_empty() {
        return Ok(None);
    }

    let total_sample_frames =
        frame_to_audio_sample_frames(composition_frames, composition_fps, AUDIO_SAMPLE_RATE);
    let start_sample_frame =
        time_to_audio_sample_frame(start_time_secs, AUDIO_SAMPLE_RATE).min(total_sample_frames);
    let sample_frames = sample_frames.min(total_sample_frames.saturating_sub(start_sample_frame));
    if sample_frames == 0 {
        return Ok(Some(AudioBuffer::silence(0)));
    }

    Ok(Some(render_audio_chunk_from_plan(
        path_store,
        plan,
        decoded,
        start_sample_frame,
        sample_frames,
    )?))
}

fn render_audio_chunk_from_plan(
    path_store: &crate::resource::AssetPathStore,
    plan: &AudioPlan,
    decoded: &mut DecodedAudioCache,
    start_sample_frame: usize,
    sample_frames: usize,
) -> Result<AudioBuffer> {
    let mut mixed = AudioBuffer::silence(sample_frames);
    let end_sample_frame = start_sample_frame.saturating_add(sample_frames);

    for seg in &plan.segments {
        let seg_start = micros_to_sample_frame(seg.start_micros().0, AUDIO_SAMPLE_RATE);
        let seg_end = micros_to_sample_frame(seg.end_micros().0, AUDIO_SAMPLE_RATE);
        if seg_end <= start_sample_frame || seg_start >= end_sample_frame {
            continue;
        }

        let clip = decoded.get_or_decode(path_store, &seg.asset)?;
        let overlap_start = seg_start.max(start_sample_frame);
        let overlap_end = seg_end.min(end_sample_frame);
        let chunk_offset_frames = overlap_start.saturating_sub(start_sample_frame);
        // Source offset inside the clip: clip plays from t=0 at segment start.
        let interval_offset_frames = overlap_start.saturating_sub(seg_start);
        let copy_frames = overlap_end.saturating_sub(overlap_start);
        let available_frames = clip.sample_frames().saturating_sub(interval_offset_frames);
        let mix_frames = copy_frames.min(available_frames);

        for frame_offset in 0..mix_frames {
            let mix_index = (chunk_offset_frames + frame_offset) * AUDIO_CHANNELS as usize;
            let clip_index = (interval_offset_frames + frame_offset) * AUDIO_CHANNELS as usize;
            mixed.samples[mix_index] += clip.samples[clip_index];
            mixed.samples[mix_index + 1] += clip.samples[clip_index + 1];
        }
    }

    for sample in &mut mixed.samples {
        *sample = sample.clamp(-1.0, 1.0);
    }

    Ok(mixed)
}

fn frame_to_audio_sample_frames(frame: u32, fps: u32, sample_rate: u32) -> usize {
    let fps = fps.max(1) as u64;
    ((frame as u64 * sample_rate as u64) / fps) as usize
}

fn time_to_audio_sample_frame(time_secs: f64, sample_rate: u32) -> usize {
    (time_secs.max(0.0) * sample_rate as f64).floor() as usize
}

fn micros_to_sample_frame(micros: u64, sample_rate: u32) -> usize {
    // Prefer integer path: micros * rate / 1_000_000
    ((micros * sample_rate as u64) / 1_000_000) as usize
}

#[allow(dead_code)]
fn segment_start_secs(plan: &AudioPlan, index: usize) -> f64 {
    plan.segments
        .get(index)
        .map(|s| timestamp_micros_to_secs(s.start_micros().0))
        .unwrap_or(0.0)
}
