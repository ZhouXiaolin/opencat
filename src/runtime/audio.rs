use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow};

use crate::{
    codec::decode::{AudioTrack, decode_audio_to_f32_stereo},
    frame_ctx::FrameCtx,
    resource::assets::AssetsMap,
    runtime::preflight::collect_sources,
    scene::{
        composition::{AudioAttachment, Composition, CompositionAudioSource},
        primitives::AudioSource,
        time::{FrameState, frame_state_for_root},
    },
};

const AUDIO_SAMPLE_RATE: u32 = 48_000;

#[derive(Clone)]
struct AudioInterval {
    source: AudioSource,
    start_frame: u32,
    end_frame: u32,
}

pub(crate) fn build_audio_track(
    composition: &Composition,
    assets: &mut AssetsMap,
) -> Result<Option<AudioTrack>> {
    let intervals = collect_audio_intervals(composition);
    if intervals.is_empty() {
        return Ok(None);
    }

    let total_frames =
        frame_to_audio_sample(composition.frames, composition.fps, AUDIO_SAMPLE_RATE);
    let mut mixed = vec![0.0_f32; total_frames * 2];
    let mut decoded = HashMap::new();

    for interval in intervals {
        let clip = if let Some(clip) = decoded.get(&interval.source) {
            clip
        } else {
            let asset_id = assets.register_audio_source(&interval.source)?;
            let path = assets
                .path(&asset_id)
                .ok_or_else(|| anyhow!("missing cached audio asset for {}", asset_id.0))?;
            let clip = decode_audio_to_f32_stereo(path, AUDIO_SAMPLE_RATE)?;
            decoded.insert(interval.source.clone(), clip);
            decoded
                .get(&interval.source)
                .expect("decoded audio clip should exist")
        };

        let start_sample =
            frame_to_audio_sample(interval.start_frame, composition.fps, AUDIO_SAMPLE_RATE);
        let end_sample =
            frame_to_audio_sample(interval.end_frame, composition.fps, AUDIO_SAMPLE_RATE);
        let available_frames = clip
            .sample_frames()
            .min(end_sample.saturating_sub(start_sample));

        for frame_offset in 0..available_frames {
            let mix_index = (start_sample + frame_offset) * 2;
            let clip_index = frame_offset * 2;
            mixed[mix_index] += clip.samples[clip_index];
            mixed[mix_index + 1] += clip.samples[clip_index + 1];
        }
    }

    for sample in &mut mixed {
        *sample = sample.clamp(-1.0, 1.0);
    }

    Ok(Some(AudioTrack::new(AUDIO_SAMPLE_RATE, 2, mixed)))
}

fn collect_audio_intervals(composition: &Composition) -> Vec<AudioInterval> {
    let mut active_specs = HashMap::<CompositionAudioSource, u32>::new();
    let mut previous_specs = HashSet::<CompositionAudioSource>::new();
    let mut active = HashMap::<AudioSource, u32>::new();
    let mut previous = HashSet::<AudioSource>::new();
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
        let mut ignored_images = HashSet::new();
        let mut current = HashSet::<AudioSource>::new();

        match frame_state_for_root(&root, &frame_ctx) {
            FrameState::Scene { scene, .. } => {
                collect_sources(&scene, &frame_ctx, &mut ignored_images, &mut current);
            }
            FrameState::Transition { from, to, .. } => {
                collect_sources(&from, &frame_ctx, &mut ignored_images, &mut current);
                collect_sources(&to, &frame_ctx, &mut ignored_images, &mut current);
            }
        }

        for spec in previous_specs.difference(&current_specs) {
            if let Some(start_frame) = active_specs.remove(spec) {
                let end_frame = spec
                    .duration
                    .map(|duration| start_frame.saturating_add(duration))
                    .unwrap_or(frame)
                    .min(frame);
                intervals.push(AudioInterval {
                    source: spec.source.clone(),
                    start_frame,
                    end_frame,
                });
            }
        }

        for spec in current_specs.difference(&previous_specs) {
            active_specs.insert(spec.clone(), frame);
        }

        for source in current.difference(&previous) {
            active.insert(source.clone(), frame);
        }

        for source in previous.difference(&current) {
            if let Some(start_frame) = active.remove(source) {
                intervals.push(AudioInterval {
                    source: source.clone(),
                    start_frame,
                    end_frame: frame,
                });
            }
        }

        previous = current;
        previous_specs = current_specs;
    }

    for source in previous {
        if let Some(start_frame) = active.remove(&source) {
            intervals.push(AudioInterval {
                source,
                start_frame,
                end_frame: composition.frames,
            });
        }
    }

    for spec in previous_specs {
        if let Some(start_frame) = active_specs.remove(&spec) {
            let end_frame = spec
                .duration
                .map(|duration| start_frame.saturating_add(duration))
                .unwrap_or(composition.frames)
                .min(composition.frames);
            intervals.push(AudioInterval {
                source: spec.source,
                start_frame,
                end_frame,
            });
        }
    }

    intervals
}

fn active_scene_ids(root: &crate::scene::node::Node, frame_ctx: &FrameCtx) -> HashSet<String> {
    match frame_state_for_root(root, frame_ctx) {
        FrameState::Scene { scene, .. } => HashSet::from([scene.style_ref().id.clone()]),
        FrameState::Transition { from, to, .. } => {
            HashSet::from([from.style_ref().id.clone(), to.style_ref().id.clone()])
        }
    }
}

fn frame_to_audio_sample(frame: u32, fps: u32, sample_rate: u32) -> usize {
    ((frame as u64 * sample_rate as u64) / fps as u64) as usize
}
