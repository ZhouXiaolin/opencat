use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::{Result, anyhow};

use crate::{
    codec::decode::{AudioTrack, decode_audio_to_f32_stereo},
    frame_ctx::FrameCtx,
    resource::assets::AssetsMap,
    scene::{
        composition::{AudioAttachment, Composition, CompositionAudioSource},
        primitives::AudioSource,
        time::{FrameState, frame_state_for_root},
    },
};

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

#[derive(Clone)]
struct AudioInterval {
    source: AudioSource,
    start_sample_frame: usize,
    end_sample_frame: usize,
}

type AudioIntervalCacheKey = (usize, usize, u32, u32);

#[derive(Default)]
pub(crate) struct AudioIntervalCache {
    key: Option<AudioIntervalCacheKey>,
    intervals: Vec<AudioInterval>,
}

impl AudioIntervalCache {
    fn get_or_resolve<'a>(&'a mut self, composition: &Composition) -> &'a [AudioInterval] {
        let key = composition_audio_cache_key(composition);
        if self.key != Some(key) {
            self.intervals = resolve_audio_intervals(composition);
            self.key = Some(key);
        }
        self.intervals.as_slice()
    }
}

pub(crate) fn build_audio_track(
    composition: &Composition,
    assets: &mut AssetsMap,
    decoded: &mut DecodedAudioCache,
    interval_cache: &mut AudioIntervalCache,
) -> Result<Option<AudioTrack>> {
    let intervals = interval_cache.get_or_resolve(composition);
    if intervals.is_empty() {
        return Ok(None);
    }

    let total_sample_frames =
        frame_to_audio_sample_frames(composition.frames, composition.fps, AUDIO_SAMPLE_RATE);
    let mut mixed = Vec::with_capacity(total_sample_frames * AUDIO_CHANNELS as usize);

    let mut start_sample_frame = 0;
    while start_sample_frame < total_sample_frames {
        let chunk_sample_frames =
            (total_sample_frames - start_sample_frame).min(DEFAULT_AUDIO_CHUNK_FRAMES);
        let chunk = render_audio_chunk_from_intervals(
            assets,
            &intervals,
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

pub(crate) fn render_audio_chunk(
    composition: &Composition,
    assets: &mut AssetsMap,
    decoded: &mut DecodedAudioCache,
    interval_cache: &mut AudioIntervalCache,
    start_time_secs: f64,
    sample_frames: usize,
) -> Result<Option<AudioBuffer>> {
    let intervals = interval_cache.get_or_resolve(composition);
    if intervals.is_empty() {
        return Ok(None);
    }

    let total_sample_frames =
        frame_to_audio_sample_frames(composition.frames, composition.fps, AUDIO_SAMPLE_RATE);
    let start_sample_frame =
        time_to_audio_sample_frame(start_time_secs, AUDIO_SAMPLE_RATE).min(total_sample_frames);
    let sample_frames = sample_frames.min(total_sample_frames.saturating_sub(start_sample_frame));
    if sample_frames == 0 {
        return Ok(Some(AudioBuffer::silence(0)));
    }

    Ok(Some(render_audio_chunk_from_intervals(
        assets,
        &intervals,
        decoded,
        start_sample_frame,
        sample_frames,
    )?))
}

fn resolve_audio_intervals(composition: &Composition) -> Vec<AudioInterval> {
    let mut active_specs = HashMap::<CompositionAudioSource, u32>::new();
    let mut previous_specs = HashSet::<CompositionAudioSource>::new();
    let mut intervals = Vec::new();

    for spec in composition.audio_sources() {
        if matches!(spec.attach, AudioAttachment::Timeline) {
            previous_specs.insert(spec.clone());
            active_specs.insert(spec.clone(), 0);
        }
    }

    for frame in 0..composition.frames {
        let frame_ctx = FrameCtx {
            frame,
            fps: composition.fps,
            width: composition.width,
            height: composition.height,
            frames: composition.frames,
        };
        let root = composition.root_node(&frame_ctx);
        let active_scene_ids = active_scene_ids(&root, &frame_ctx);
        let mut current_specs = composition
            .audio_sources()
            .iter()
            .cloned()
            .filter(|spec| match &spec.attach {
                AudioAttachment::Timeline => true,
                AudioAttachment::Scene { scene_id } => active_scene_ids.contains(scene_id),
            })
            .collect::<HashSet<_>>();
        current_specs.retain(|spec| {
            let Some(start_frame) = active_specs.get(spec).copied() else {
                return true;
            };
            spec.duration
                .map(|duration| frame < start_frame.saturating_add(duration))
                .unwrap_or(true)
        });

        for spec in previous_specs.difference(&current_specs) {
            if let Some(start_frame) = active_specs.remove(spec) {
                let start_sample_frame =
                    frame_to_audio_sample_frames(start_frame, composition.fps, AUDIO_SAMPLE_RATE);
                let end_frame = spec
                    .duration
                    .map(|duration| start_frame.saturating_add(duration))
                    .unwrap_or(frame)
                    .min(frame);
                let end_sample_frame =
                    frame_to_audio_sample_frames(end_frame, composition.fps, AUDIO_SAMPLE_RATE);
                intervals.push(AudioInterval {
                    source: spec.source.clone(),
                    start_sample_frame,
                    end_sample_frame,
                });
            }
        }

        for spec in current_specs.difference(&previous_specs) {
            active_specs.insert(spec.clone(), frame);
        }
        previous_specs = current_specs;
    }

    for spec in previous_specs {
        if let Some(start_frame) = active_specs.remove(&spec) {
            let start_sample_frame =
                frame_to_audio_sample_frames(start_frame, composition.fps, AUDIO_SAMPLE_RATE);
            let end_frame = spec
                .duration
                .map(|duration| start_frame.saturating_add(duration))
                .unwrap_or(composition.frames)
                .min(composition.frames);
            let end_sample_frame =
                frame_to_audio_sample_frames(end_frame, composition.fps, AUDIO_SAMPLE_RATE);
            intervals.push(AudioInterval {
                source: spec.source,
                start_sample_frame,
                end_sample_frame,
            });
        }
    }

    intervals
}

fn composition_audio_cache_key(composition: &Composition) -> AudioIntervalCacheKey {
    let root_ptr = Arc::as_ptr(&composition.root) as *const () as usize;
    let audio_sources_ptr = Arc::as_ptr(&composition.audio_sources) as usize;
    (
        root_ptr,
        audio_sources_ptr,
        composition.frames,
        composition.fps,
    )
}

fn active_scene_ids(root: &crate::scene::node::Node, frame_ctx: &FrameCtx) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_active_scene_ids(root, frame_ctx, &mut out);
    out
}

fn collect_active_scene_ids(
    root: &crate::scene::node::Node,
    frame_ctx: &FrameCtx,
    out: &mut HashSet<String>,
) {
    let state = frame_state_for_root(root, frame_ctx);
    collect_active_scene_ids_from_state(&state, frame_ctx, out);
}

fn collect_active_scene_ids_from_state(
    frame_state: &FrameState,
    _frame_ctx: &FrameCtx,
    out: &mut HashSet<String>,
) {
    match frame_state {
        FrameState::Scene { scene, .. } => {
            out.insert(scene.style_ref().id.clone());
        }
        FrameState::Transition { from, to, .. } => {
            out.insert(from.style_ref().id.clone());
            out.insert(to.style_ref().id.clone());
        }
    }
}

#[derive(Default)]
pub(crate) struct DecodedAudioCache {
    decoded: std::collections::HashMap<AudioSource, AudioTrack>,
}

impl DecodedAudioCache {
    fn get_or_decode<'a>(
        &'a mut self,
        assets: &mut AssetsMap,
        source: &AudioSource,
    ) -> Result<&'a AudioTrack> {
        if !self.decoded.contains_key(source) {
            let asset_id = assets.register_audio_source(source)?;
            let path = assets
                .path(&asset_id)
                .ok_or_else(|| anyhow!("missing cached audio asset for {}", asset_id.0))?;
            let clip = decode_audio_to_f32_stereo(path, AUDIO_SAMPLE_RATE)?;
            self.decoded.insert(source.clone(), clip);
        }
        Ok(self
            .decoded
            .get(source)
            .expect("decoded audio clip should exist"))
    }
}

fn render_audio_chunk_from_intervals(
    assets: &mut AssetsMap,
    intervals: &[AudioInterval],
    decoded: &mut DecodedAudioCache,
    start_sample_frame: usize,
    sample_frames: usize,
) -> Result<AudioBuffer> {
    let mut mixed = AudioBuffer::silence(sample_frames);
    let end_sample_frame = start_sample_frame.saturating_add(sample_frames);

    for interval in intervals {
        if interval.end_sample_frame <= start_sample_frame
            || interval.start_sample_frame >= end_sample_frame
        {
            continue;
        }

        let clip = decoded.get_or_decode(assets, &interval.source)?;
        let overlap_start = interval.start_sample_frame.max(start_sample_frame);
        let overlap_end = interval.end_sample_frame.min(end_sample_frame);
        let chunk_offset_frames = overlap_start.saturating_sub(start_sample_frame);
        let interval_offset_frames = overlap_start.saturating_sub(interval.start_sample_frame);
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
    ((frame as u64 * sample_rate as u64) / fps as u64) as usize
}

fn time_to_audio_sample_frame(time_secs: f64, sample_rate: u32) -> usize {
    (time_secs.max(0.0) * sample_rate as f64).floor() as usize
}
