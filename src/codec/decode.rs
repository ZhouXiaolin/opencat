use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Maximum number of distinct sws scaling contexts kept per decoder. Together
/// with `quantize_target_size` (16px buckets) this keeps memory bounded even
/// when target sizes drift over an animation.
const MAX_SCALERS_PER_DECODER: usize = 4;

use anyhow::{Context, Result, anyhow};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::threading::{Config, Type};
use ffmpeg_next::{
    ChannelLayout, codec, format, frame,
    software::{
        resampling::context::Context as ResamplingContext,
        scaling::{context::Context as ScalingContext, flag::Flags as ScalingFlags},
    },
    util::format::{
        pixel::Pixel,
        sample::{Sample, Type as SampleType},
    },
};

pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration_secs: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct AudioTrack {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl AudioTrack {
    pub fn new(sample_rate: u32, channels: u16, samples: Vec<f32>) -> Self {
        Self {
            sample_rate,
            channels,
            samples,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn sample_frames(&self) -> usize {
        self.samples.len() / self.channels as usize
    }
}

struct VideoDecoder {
    path: PathBuf,
    input: format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scalers: VecDeque<((u32, u32), ScalingContext)>,
    source_format: Pixel,
    stream_index: usize,
    time_base: ffmpeg::util::rational::Rational,
    width: u32,
    height: u32,
    duration_secs: Option<f64>,
    keyframe_pts_secs: Vec<f64>,
    current_pts_secs: f64,
    current_size: Option<(u32, u32)>,
    current_frame: Option<Arc<Vec<u8>>>,
    /// Last decoded source frame (pre-scale). Kept so that a same-time-but-
    /// different-size request can re-run sws without re-decoding.
    current_source: Option<ffmpeg::frame::Video>,
    eof: bool,
}

#[derive(Clone, Copy, Debug)]
struct DecoderCursor {
    has_frame: bool,
    current_pts_secs: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecoderLaneSelection {
    Reuse(usize),
    OpenNew,
}

impl VideoDecoder {
    fn open(path: &Path) -> Result<Self> {
        ffmpeg::init()?;

        let input = format::input(path)
            .with_context(|| format!("failed to open video: {}", path.display()))?;

        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or_else(|| anyhow!("no video stream in {}", path.display()))?;
        let stream_index = stream.index();
        let time_base = stream.time_base();

        let mut codec_ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
        let num_cpus = num_cpus::get().clamp(1, 16);
        if num_cpus > 1 {
            codec_ctx.set_threading(Config {
                kind: Type::Frame,
                count: num_cpus,
            });
        }
        let decoder = codec_ctx.decoder().video()?;

        let width = decoder.width();
        let height = decoder.height();
        let duration_secs = stream
            .duration()
            .checked_mul(time_base.numerator() as i64)
            .map(|duration_ticks| duration_ticks as f64 / time_base.denominator() as f64);
        let keyframe_pts_secs = collect_video_keyframe_pts_secs(path, stream_index, time_base)?;
        let source_format = decoder.format();
        let mut scalers = VecDeque::with_capacity(MAX_SCALERS_PER_DECODER);
        scalers.push_back((
            (width, height),
            ScalingContext::get(
                source_format,
                width,
                height,
                Pixel::RGBA,
                width,
                height,
                ScalingFlags::BILINEAR,
            )?,
        ));

        Ok(Self {
            path: path.to_path_buf(),
            input,
            decoder,
            scalers,
            source_format,
            stream_index,
            time_base,
            width,
            height,
            duration_secs,
            keyframe_pts_secs,
            current_pts_secs: -1.0,
            current_size: None,
            current_frame: None,
            current_source: None,
            eof: false,
        })
    }

    fn info(&self) -> VideoInfo {
        VideoInfo {
            width: self.width,
            height: self.height,
            duration_secs: self.duration_secs,
        }
    }

    fn get_frame_at_time(
        &mut self,
        target_secs: f64,
        quality: crate::resource::media::VideoPreviewQuality,
        target_size: Option<(u32, u32)>,
    ) -> Result<Arc<Vec<u8>>> {
        let resolved_size = target_size.unwrap_or((self.width, self.height));
        let same_time = self.current_frame.is_some()
            && (self.current_pts_secs - target_secs).abs() < 1e-6;
        let same_size = self.current_size == Some(resolved_size);
        if same_time && same_size {
            return Ok(self
                .current_frame
                .clone()
                .expect("current frame should exist"));
        }

        // Same time, different size: re-scale the cached source frame in place
        // (no decode work). This is the common path when a video is shown at
        // multiple sizes in the same composition frame, or when a preceding
        // path decoded the frame at source resolution.
        if same_time && !same_size {
            if let Some(source) = self.current_source.clone() {
                self.update_scaled_frame(&source, self.current_pts_secs, target_size)?;
                return Ok(self
                    .current_frame
                    .clone()
                    .expect("scaled frame just produced"));
            }
        }

        if self.should_seek_to_target(target_secs, quality) {
            self.seek_to_time(target_secs)?;
        }

        self.decode_forward(target_secs, target_size)?;
        self.current_frame
            .clone()
            .ok_or_else(|| anyhow!("failed to decode frame at {:.3}s", target_secs))
    }

    fn cursor(&self) -> DecoderCursor {
        DecoderCursor {
            has_frame: self.current_frame.is_some(),
            current_pts_secs: self.current_pts_secs,
        }
    }

    fn should_seek_to_target(
        &self,
        target_secs: f64,
        quality: crate::resource::media::VideoPreviewQuality,
    ) -> bool {
        if self.current_frame.is_none() {
            return true;
        }

        if target_secs + 1e-6 < self.current_pts_secs {
            return true;
        }

        let forward_delta = target_secs - self.current_pts_secs;
        let seek_threshold_secs = match quality {
            crate::resource::media::VideoPreviewQuality::Scrubbing => 0.12,
            crate::resource::media::VideoPreviewQuality::Realtime => 0.35,
            crate::resource::media::VideoPreviewQuality::Exact => 1.5,
        };
        forward_delta > seek_threshold_secs
    }

    fn reopen(&mut self) -> Result<()> {
        *self = Self::open(&self.path)?;
        Ok(())
    }

    fn seek_to_time(&mut self, target_secs: f64) -> Result<()> {
        let seek_secs = nearest_keyframe_before(&self.keyframe_pts_secs, target_secs.max(0.0));
        let target_pts = (seek_secs * 1_000_000.0).round() as i64;
        if self.input.seek(target_pts, ..target_pts).is_err() {
            self.reopen()?;
            return Ok(());
        }

        self.decoder.flush();
        self.current_pts_secs = -1.0;
        self.current_size = None;
        self.current_frame = None;
        self.current_source = None;
        self.eof = false;
        Ok(())
    }

    fn decode_forward(
        &mut self,
        target_secs: f64,
        target_size: Option<(u32, u32)>,
    ) -> Result<()> {
        if self.eof {
            return Ok(());
        }

        loop {
            let packet = self.read_next_packet();
            let Some(packet) = packet else { break };

            self.decoder.send_packet(&packet)?;
            if self.receive_until(target_secs, target_size)? {
                return Ok(());
            }
        }

        self.eof = true;
        self.decoder.send_eof()?;
        self.receive_until(target_secs, target_size)?;
        Ok(())
    }

    fn read_next_packet(&mut self) -> Option<ffmpeg::codec::packet::Packet> {
        for (stream, packet) in self.input.packets() {
            if stream.index() == self.stream_index {
                return Some(packet);
            }
        }
        None
    }

    fn receive_until(
        &mut self,
        target_secs: f64,
        target_size: Option<(u32, u32)>,
    ) -> Result<bool> {
        let mut frame = ffmpeg::frame::Video::empty();
        let mut selected_frame: Option<ffmpeg::frame::Video> = None;
        let mut selected_pts_secs = -1.0;
        let mut reached_target = false;

        while self.decoder.receive_frame(&mut frame).is_ok() {
            let pts = frame.pts().unwrap_or(0);
            let pts_secs = pts as f64 * self.time_base.numerator() as f64
                / self.time_base.denominator() as f64;
            selected_pts_secs = pts_secs;
            selected_frame = Some(frame.clone());

            if pts_secs >= target_secs {
                reached_target = true;
                break;
            }
        }

        if let Some(frame) = selected_frame.as_ref() {
            self.update_current_frame(frame, selected_pts_secs, target_size)?;
        }

        Ok(reached_target)
    }

    fn update_current_frame(
        &mut self,
        frame: &ffmpeg::frame::Video,
        pts_secs: f64,
        target_size: Option<(u32, u32)>,
    ) -> Result<()> {
        self.current_source = Some(frame.clone());
        self.update_scaled_frame(frame, pts_secs, target_size)
    }

    fn update_scaled_frame(
        &mut self,
        frame: &ffmpeg::frame::Video,
        pts_secs: f64,
        target_size: Option<(u32, u32)>,
    ) -> Result<()> {
        let (out_w, out_h) = target_size.unwrap_or((self.width, self.height));

        let scaler = self.scaler_for(out_w, out_h)?;
        let mut rgba = ffmpeg::frame::Video::new(Pixel::RGBA, out_w, out_h);
        scaler.run(frame, &mut rgba)?;
        self.current_pts_secs = pts_secs;
        self.current_size = Some((out_w, out_h));
        self.current_frame = Some(Arc::new(pack_rgba(&rgba, out_w, out_h)));
        Ok(())
    }

    fn scaler_for(&mut self, out_w: u32, out_h: u32) -> Result<&mut ScalingContext> {
        if let Some(idx) = self
            .scalers
            .iter()
            .position(|(size, _)| *size == (out_w, out_h))
        {
            // LRU touch: move to back.
            if idx + 1 != self.scalers.len() {
                let entry = self
                    .scalers
                    .remove(idx)
                    .expect("index validated above");
                self.scalers.push_back(entry);
            }
        } else {
            let scaler = ScalingContext::get(
                self.source_format,
                self.width,
                self.height,
                Pixel::RGBA,
                out_w,
                out_h,
                ScalingFlags::BILINEAR,
            )?;
            if self.scalers.len() >= MAX_SCALERS_PER_DECODER {
                self.scalers.pop_front();
            }
            self.scalers.push_back(((out_w, out_h), scaler));
        }
        Ok(&mut self
            .scalers
            .back_mut()
            .expect("scaler just pushed")
            .1)
    }
}

fn seek_threshold_secs(quality: crate::resource::media::VideoPreviewQuality) -> f64 {
    match quality {
        crate::resource::media::VideoPreviewQuality::Scrubbing => 0.12,
        crate::resource::media::VideoPreviewQuality::Realtime => 0.35,
        crate::resource::media::VideoPreviewQuality::Exact => 1.5,
    }
}

fn select_decoder_lane(
    cursors: &[DecoderCursor],
    target_secs: f64,
    quality: crate::resource::media::VideoPreviewQuality,
    max_lanes_per_asset: usize,
) -> DecoderLaneSelection {
    if cursors.is_empty() {
        return DecoderLaneSelection::OpenNew;
    }

    if let Some((index, _)) = cursors.iter().enumerate().find(|(_, cursor)| {
        cursor.has_frame && (cursor.current_pts_secs - target_secs).abs() < 1e-6
    }) {
        return DecoderLaneSelection::Reuse(index);
    }

    if let Some((index, _)) = cursors
        .iter()
        .enumerate()
        .find(|(_, cursor)| !cursor.has_frame)
    {
        return DecoderLaneSelection::Reuse(index);
    }

    let seek_threshold_secs = seek_threshold_secs(quality);
    if let Some((index, _)) = cursors
        .iter()
        .enumerate()
        .filter(|(_, cursor)| {
            cursor.has_frame
                && target_secs + 1e-6 >= cursor.current_pts_secs
                && target_secs - cursor.current_pts_secs <= seek_threshold_secs
        })
        .min_by(|(_, left), (_, right)| {
            (target_secs - left.current_pts_secs)
                .partial_cmp(&(target_secs - right.current_pts_secs))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    {
        return DecoderLaneSelection::Reuse(index);
    }

    if cursors.len() < max_lanes_per_asset.max(1) {
        return DecoderLaneSelection::OpenNew;
    }

    let (index, _) = cursors
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (left.current_pts_secs - target_secs)
                .abs()
                .partial_cmp(&(right.current_pts_secs - target_secs).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("non-empty cursor list should produce a nearest lane");
    DecoderLaneSelection::Reuse(index)
}

pub struct VideoDecodeCache {
    decoders: HashMap<PathBuf, VecDeque<VideoDecoder>>,
    max_lanes_per_asset: usize,
}

impl VideoDecodeCache {
    pub fn new() -> Self {
        Self::with_max_lanes_per_asset(2)
    }

    pub fn with_max_lanes_per_asset(max_lanes_per_asset: usize) -> Self {
        Self {
            decoders: HashMap::new(),
            max_lanes_per_asset: max_lanes_per_asset.max(1),
        }
    }

    pub fn get_frame(
        &mut self,
        path: &Path,
        target_time_secs: f64,
        quality: crate::resource::media::VideoPreviewQuality,
        target_size: Option<(u32, u32)>,
    ) -> Result<Arc<Vec<u8>>> {
        let path_buf = path.to_path_buf();
        let decoders = self.decoders.entry(path_buf).or_default();
        let cursors: Vec<_> = decoders.iter().map(VideoDecoder::cursor).collect();
        let selection = select_decoder_lane(
            &cursors,
            target_time_secs,
            quality,
            self.max_lanes_per_asset,
        );

        let mut decoder = match selection {
            DecoderLaneSelection::Reuse(index) => decoders
                .remove(index)
                .expect("selected decoder lane should exist"),
            DecoderLaneSelection::OpenNew => VideoDecoder::open(path)?,
        };

        let frame = decoder.get_frame_at_time(target_time_secs, quality, target_size)?;
        decoders.push_back(decoder);
        Ok(frame)
    }

    pub fn info(&mut self, path: &Path) -> Result<VideoInfo> {
        if !self.decoders.contains_key(path) {
            let decoder = VideoDecoder::open(path)?;
            let mut lanes = VecDeque::new();
            lanes.push_back(decoder);
            self.decoders.insert(path.to_path_buf(), lanes);
        }
        Ok(self
            .decoders
            .get(path)
            .expect("video decoder should exist")
            .front()
            .expect("video decoder lane should exist")
            .info())
    }
}

impl Default for VideoDecodeCache {
    fn default() -> Self {
        Self::new()
    }
}

fn collect_video_keyframe_pts_secs(
    path: &Path,
    stream_index: usize,
    time_base: ffmpeg::util::rational::Rational,
) -> Result<Vec<f64>> {
    let mut input = format::input(path)
        .with_context(|| format!("failed to open video for keyframe scan: {}", path.display()))?;
    let mut keyframes = Vec::new();

    for (stream, packet) in input.packets() {
        if stream.index() != stream_index || !packet.is_key() {
            continue;
        }

        let pts = packet.pts().or_else(|| packet.dts()).unwrap_or(0);
        let pts_secs = pts as f64 * time_base.numerator() as f64 / time_base.denominator() as f64;
        if keyframes
            .last()
            .is_none_or(|last| (pts_secs - last).abs() > 1e-6)
        {
            keyframes.push(pts_secs.max(0.0));
        }
    }

    if keyframes.is_empty() {
        keyframes.push(0.0);
    }

    Ok(keyframes)
}

fn nearest_keyframe_before(keyframes: &[f64], target_secs: f64) -> f64 {
    if keyframes.is_empty() {
        return target_secs.max(0.0);
    }

    let index = keyframes.partition_point(|&secs| secs <= target_secs + 1e-6);
    keyframes[index.saturating_sub(1)]
}

pub fn decode_audio_to_f32_stereo(path: &Path, target_rate: u32) -> Result<AudioTrack> {
    ffmpeg::init()?;

    let mut input = format::input(path)
        .with_context(|| format!("failed to open audio source: {}", path.display()))?;
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .ok_or_else(|| anyhow!("no audio stream in {}", path.display()))?;
    let stream_index = stream.index();

    let mut codec_ctx = codec::context::Context::from_parameters(stream.parameters())?;
    let num_cpus = num_cpus::get().clamp(1, 16);
    if num_cpus > 1 {
        codec_ctx.set_threading(Config {
            kind: Type::Frame,
            count: num_cpus,
        });
    }
    let mut decoder = codec_ctx.decoder().audio()?;

    let src_layout = if decoder.channel_layout().is_empty() {
        ChannelLayout::default(decoder.channels() as i32)
    } else {
        decoder.channel_layout()
    };
    let mut resampler = ResamplingContext::get(
        decoder.format(),
        src_layout,
        decoder.rate(),
        Sample::F32(SampleType::Packed),
        ChannelLayout::STEREO,
        target_rate,
    )?;

    let mut samples = Vec::new();
    for (packet_stream, packet) in input.packets() {
        if packet_stream.index() != stream_index {
            continue;
        }

        decoder.send_packet(&packet)?;
        drain_audio_frames(&mut decoder, &mut resampler, &mut samples)?;
    }

    decoder.send_eof()?;
    drain_audio_frames(&mut decoder, &mut resampler, &mut samples)?;

    loop {
        let mut converted = frame::Audio::empty();
        match resampler.flush(&mut converted) {
            Ok(Some(_)) => append_packed_stereo_samples(&converted, &mut samples),
            Ok(None) => break,
            Err(ffmpeg::Error::OutputChanged) => break,
            Err(err) => return Err(err.into()),
        }
    }

    Ok(AudioTrack::new(target_rate, 2, samples))
}

fn drain_audio_frames(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut ResamplingContext,
    output: &mut Vec<f32>,
) -> Result<()> {
    let mut decoded = frame::Audio::empty();
    while decoder.receive_frame(&mut decoded).is_ok() {
        let mut converted = frame::Audio::empty();
        resampler.run(&decoded, &mut converted)?;
        append_packed_stereo_samples(&converted, output);
    }
    Ok(())
}

fn append_packed_stereo_samples(frame: &frame::Audio, output: &mut Vec<f32>) {
    if frame.samples() == 0 {
        return;
    }

    match frame.format() {
        Sample::F32(SampleType::Packed) => {
            for &(left, right) in frame.plane::<(f32, f32)>(0) {
                output.push(left);
                output.push(right);
            }
        }
        other => {
            panic!("expected packed f32 stereo audio, got {:?}", other);
        }
    }
}

fn pack_rgba(frame: &ffmpeg::frame::Video, width: u32, height: u32) -> Vec<u8> {
    let stride = frame.stride(0) as usize;
    let row_bytes = width as usize * 4;
    let mut packed = Vec::with_capacity(row_bytes * height as usize);
    for y in 0..height as usize {
        let start = y * stride;
        packed.extend_from_slice(&frame.data(0)[start..start + row_bytes]);
    }
    packed
}

#[cfg(test)]
mod tests {
    use super::{
        DecoderCursor, DecoderLaneSelection, nearest_keyframe_before, seek_threshold_secs,
        select_decoder_lane,
    };
    use crate::resource::media::VideoPreviewQuality;

    #[test]
    fn nearest_keyframe_before_clamps_to_previous_anchor() {
        let keyframes = [0.0, 0.5, 1.2, 2.4];
        assert!((nearest_keyframe_before(&keyframes, 0.1) - 0.0).abs() < 1e-6);
        assert!((nearest_keyframe_before(&keyframes, 1.8) - 1.2).abs() < 1e-6);
        assert!((nearest_keyframe_before(&keyframes, 3.0) - 2.4).abs() < 1e-6);
    }

    #[test]
    fn select_decoder_lane_prefers_continuation_without_seek() {
        let lanes = [
            DecoderCursor {
                has_frame: true,
                current_pts_secs: 2.0,
            },
            DecoderCursor {
                has_frame: true,
                current_pts_secs: 4.8,
            },
        ];

        let selection = select_decoder_lane(&lanes, 2.3, VideoPreviewQuality::Realtime, 2);
        assert_eq!(selection, DecoderLaneSelection::Reuse(0));
    }

    #[test]
    fn select_decoder_lane_opens_new_lane_when_seek_would_thrash() {
        let lanes = [DecoderCursor {
            has_frame: true,
            current_pts_secs: 6.0,
        }];

        let selection = select_decoder_lane(&lanes, 1.0, VideoPreviewQuality::Exact, 2);
        assert_eq!(selection, DecoderLaneSelection::OpenNew);
    }

    #[test]
    fn select_decoder_lane_reuses_nearest_lane_when_lane_budget_is_full() {
        let lanes = [
            DecoderCursor {
                has_frame: true,
                current_pts_secs: 1.0,
            },
            DecoderCursor {
                has_frame: true,
                current_pts_secs: 8.0,
            },
        ];

        let selection = select_decoder_lane(&lanes, 6.9, VideoPreviewQuality::Exact, 2);
        assert_eq!(selection, DecoderLaneSelection::Reuse(1));
    }

    #[test]
    fn seek_threshold_matches_preview_quality() {
        assert!((seek_threshold_secs(VideoPreviewQuality::Scrubbing) - 0.12).abs() < 1e-6);
        assert!((seek_threshold_secs(VideoPreviewQuality::Realtime) - 0.35).abs() < 1e-6);
        assert!((seek_threshold_secs(VideoPreviewQuality::Exact) - 1.5).abs() < 1e-6);
    }
}
