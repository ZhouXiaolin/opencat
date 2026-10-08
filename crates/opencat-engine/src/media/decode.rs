use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Maximum number of distinct sws scaling contexts kept per decoder. Together
/// with `quantize_target_size` (16px buckets) this keeps memory bounded even
/// when target sizes drift over an animation.
const MAX_SCALERS_PER_DECODER: usize = 4;

use crate::media::seek::{
    DecoderCursor, DecoderLaneSelection, nearest_keyframe_before, seek_threshold_secs,
    select_decoder_lane,
};
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
    keyframe_pts_us: Vec<u64>,
    current_pts_secs: f64,
    /// Previous request target. Frame-display semantics show the last source
    /// frame whose pts is <= target, so a forward-moving target occasionally
    /// needs one extra decoded frame; see `receive_until`.
    last_target_secs: f64,
    /// The DecodedFrame one after `current_source` (the next source frame in
    /// presentation order). Held so the next display frame is already decoded
    /// and the switch is exact under floor semantics.
    ahead_source: Option<ffmpeg::frame::Video>,
    ahead_pts_secs: f64,
    current_size: Option<(u32, u32)>,
    current_frame: Option<Arc<Vec<u8>>>,
    /// Last decoded source frame (pre-scale). Kept so that a same-time-but-
    /// different-size request can re-run sws without re-decoding.
    current_source: Option<ffmpeg::frame::Video>,
    eof: bool,
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
        let keyframe_pts_us = collect_video_keyframe_pts_us(path, stream_index, time_base)?;
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
            keyframe_pts_us,
            current_pts_secs: -1.0,
            last_target_secs: f64::NEG_INFINITY,
            ahead_source: None,
            ahead_pts_secs: -1.0,
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
        quality: crate::media::VideoPreviewQuality,
        target_size: Option<(u32, u32)>,
    ) -> Result<Arc<Vec<u8>>> {
        let resolved_size = target_size.unwrap_or((self.width, self.height));
        let same_target =
            self.current_frame.is_some() && (self.last_target_secs - target_secs).abs() < 1e-9;
        let same_size = self.current_size == Some(resolved_size);
        self.last_target_secs = target_secs;

        if same_target && same_size {
            return Ok(self
                .current_frame
                .clone()
                .expect("current frame should exist"));
        }

        // Same request time, different size: re-scale the cached source frame
        // in place (no decode work). This is the common path when a video is
        // shown at multiple sizes in the same composition frame, or when a
        // preceding path decoded the frame at source resolution.
        if same_target
            && !same_size
            && let Some(source) = self.current_source.clone()
        {
            let pts = self.current_pts_secs;
            self.update_scaled_frame(&source, pts, target_size)?;
            return Ok(self
                .current_frame
                .clone()
                .expect("scaled frame just produced"));
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
        quality: crate::media::VideoPreviewQuality,
    ) -> bool {
        if self.current_frame.is_none() {
            return true;
        }

        // Target inside the currently displayed frame's interval
        // [current_pts, next_pts): the floor frame is the current one, so
        // advance by decoding (or not) rather than seeking.
        if target_secs + 1e-9 < self.current_pts_secs {
            return true;
        }

        let forward_delta = target_secs - self.current_pts_secs;
        forward_delta > seek_threshold_secs(quality)
    }

    fn reopen(&mut self) -> Result<()> {
        *self = Self::open(&self.path)?;
        Ok(())
    }

    fn seek_to_time(&mut self, target_secs: f64) -> Result<()> {
        let target_us = (target_secs.max(0.0) * 1_000_000.0).round() as u64;
        let target_pts = nearest_keyframe_before(&self.keyframe_pts_us, target_us) as i64;
        if self.input.seek(target_pts, ..target_pts).is_err() {
            self.reopen()?;
            return Ok(());
        }

        self.decoder.flush();
        self.current_pts_secs = -1.0;
        self.last_target_secs = f64::NEG_INFINITY;
        self.current_size = None;
        self.current_frame = None;
        self.current_source = None;
        self.ahead_source = None;
        self.ahead_pts_secs = -1.0;
        self.eof = false;
        Ok(())
    }

    /// Bring `current_source` to the display frame for `target_secs` under
    /// browser `<video>` semantics: the last source frame whose pts is
    /// <= target. `should_seek` has already been applied by the caller, so
    /// either `current_source` is `None` (fresh/after seek) or its pts is
    /// <= target.
    ///
    /// Core expresses media time in microseconds, so `target_secs` arrives
    /// microsecond-quantized (clip-local 1/3 s → 0.333333) while source pts are
    /// exact rationals (8/24 s → 0.333333333…). A raw f64 floor would then drop
    /// back to the *previous* source frame at a nominal boundary; comparing both
    /// in the same microsecond unit keeps the exact boundary on the frame the
    /// browser shows.
    ///
    /// Invariant on return: `current_pts_secs` = floor(target); when
    /// `ahead_source` is present it is the immediate successor frame
    /// (`ahead_pts_secs > current_pts_secs`).
    fn decode_forward(&mut self, target_secs: f64, target_size: Option<(u32, u32)>) -> Result<()> {
        let target_micros = (target_secs * 1_000_000.0).round() as i64;
        // Rebuild from scratch when there is no current frame.
        if self.current_source.is_none() {
            let Some(first) = self.decode_next_frame()? else {
                return Ok(()); // stream empty
            };
            let pts = self.frame_pts_secs(&first);
            // Target at/before the first frame clamps to it; otherwise this is
            // the starting point and the loop below advances to the floor.
            self.update_current_frame(&first, pts, target_size)?;
            self.last_target_secs = target_secs;
        }

        loop {
            let ahead_ok = self.ahead_source.is_some() && self.ahead_pts_secs > self.current_pts_secs;
            if !ahead_ok {
                // Drain the decoder even after packet exhaustion: a
                // frame-threaded decoder can still hold frames that were not
                // delivered when its packets ran out. `decode_next_frame`
                // returns `None` only once the stream is truly drained.
                match self.decode_next_frame()? {
                    Some(frame) => {
                        let pts = self.frame_pts_secs(&frame);
                        if pts > self.current_pts_secs + 1e-9 {
                            self.ahead_source = Some(frame);
                            self.ahead_pts_secs = pts;
                        }
                        // Otherwise a duplicate/reordered frame: ignore and
                        // read further.
                        continue;
                    }
                    None => {
                        self.eof = true;
                        return Ok(());
                    }
                }
            }

            if floor_advance(target_micros, self.ahead_pts_secs) {
                // The successor's interval still contains the target: advance.
                let frame = self.ahead_source.take().expect("ahead confirmed present");
                let pts = self.ahead_pts_secs;
                self.ahead_source = None;
                self.ahead_pts_secs = -1.0;
                self.update_current_frame(&frame, pts, target_size)?;
                self.last_target_secs = target_secs;
                continue;
            }

            // The successor starts after the target: the current frame floors
            // it. Refresh the scaled output in case only the size changed.
            if let Some(source) = self.current_source.clone() {
                self.update_scaled_frame(&source, self.current_pts_secs, target_size)?;
            }
            self.last_target_secs = target_secs;
            return Ok(());
        }
    }

    /// Decode the next source frame in presentation order, reading packets as
    /// needed. Returns `None` once the stream is fully drained.
    fn decode_next_frame(&mut self) -> Result<Option<ffmpeg::frame::Video>> {
        let mut frame = ffmpeg::frame::Video::empty();
        loop {
            if self.decoder.receive_frame(&mut frame).is_ok() {
                return Ok(Some(frame));
            }

            if self.eof {
                // Packets already drained and no more frames buffered.
                return Ok(None);
            }

            match self.read_next_packet() {
                Some(packet) => self.decoder.send_packet(&packet)?,
                None => {
                    self.decoder.send_eof()?;
                    self.eof = true;
                }
            }
        }
    }

    fn frame_pts_secs(&self, frame: &ffmpeg::frame::Video) -> f64 {
        let pts = frame.pts().unwrap_or(0);
        pts as f64 * self.time_base.numerator() as f64 / self.time_base.denominator() as f64
    }

    fn read_next_packet(&mut self) -> Option<ffmpeg::codec::packet::Packet> {
        for (stream, packet) in self.input.packets() {
            if stream.index() == self.stream_index {
                return Some(packet);
            }
        }
        None
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
                let entry = self.scalers.remove(idx).expect("index validated above");
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
        Ok(&mut self.scalers.back_mut().expect("scaler just pushed").1)
    }
}

pub struct VideoDecodeCache {
    decoders: HashMap<PathBuf, VecDeque<VideoDecoder>>,
    max_lanes_per_asset: usize,
}

impl VideoDecodeCache {
    pub fn new() -> Self {
        Self::with_max_lanes_per_asset(4)
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
        quality: crate::media::VideoPreviewQuality,
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

fn collect_video_keyframe_pts_us(
    path: &Path,
    stream_index: usize,
    time_base: ffmpeg::util::rational::Rational,
) -> Result<Vec<u64>> {
    let mut input = format::input(path)
        .with_context(|| format!("failed to open video for keyframe scan: {}", path.display()))?;
    let mut keyframes = Vec::new();

    for (stream, packet) in input.packets() {
        if stream.index() != stream_index || !packet.is_key() {
            continue;
        }

        let pts = packet.pts().or_else(|| packet.dts()).unwrap_or(0);
        let pts_secs = pts as f64 * time_base.numerator() as f64 / time_base.denominator() as f64;
        let pts_us = (pts_secs.max(0.0) * 1_000_000.0).round() as u64;
        if keyframes
            .last()
            .is_none_or(|last| pts_us.saturating_sub(*last) > 1)
        {
            keyframes.push(pts_us);
        }
    }

    if keyframes.is_empty() {
        keyframes.push(0);
    }

    Ok(keyframes)
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

/// Floor-selection predicate for [`VideoDecoder::decode_forward`]. The successor
/// frame replaces the current frame when its pts, rounded to the same whole
/// microsecond grid core uses for targets, is still at or before the target.
///
/// Both sides are microsecond-quantized so an exact rational source boundary
/// (e.g. 8/24 s = 0.333333333…) is not dropped below a microsecond-rounded
/// target (0.333333) by a raw f64 comparison.
fn floor_advance(target_micros: i64, ahead_pts_secs: f64) -> bool {
    let ahead_micros = (ahead_pts_secs * 1_000_000.0).round() as i64;
    ahead_micros <= target_micros
}

fn pack_rgba(frame: &ffmpeg::frame::Video, width: u32, height: u32) -> Vec<u8> {
    let stride = frame.stride(0);
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
    use super::floor_advance;

    /// Core hands the host a microsecond-quantized target (clip-local 1/3 s →
    /// 0.333333). A source frame at the exact rational boundary (8/24 s =
    /// 0.333333333…) must still floor-select that frame rather than dropping to
    /// its predecessor. This is the site that regressed act 3's clip-boundary
    /// frames before the microsecond-unit comparison.
    #[test]
    fn exact_frame_boundary_is_not_dropped_by_microsecond_target() {
        let target_micros = (0.333333_f64 * 1_000_000.0).round() as i64;
        // Successor frame at 1/3 s: its interval contains the target, so the
        // decoder must advance to it.
        assert!(floor_advance(target_micros, 8.0 / 24.0));
    }

    /// The predicate is sugar for "successor pts (micros) <= target (micros)":
    /// a successor strictly after the target must not replace the current one.
    #[test]
    fn successor_after_target_is_not_selected() {
        let target_micros = (0.333333_f64 * 1_000_000.0).round() as i64;
        assert!(!floor_advance(target_micros, 9.0 / 24.0));
    }

    /// Within a frame interval the floor frame wins; exactly on a boundary the
    /// frame starting there wins (its pts micros equals the target micros).
    #[test]
    fn boundary_pts_equal_to_target_selects_the_frame() {
        let target_micros = (1.0_f64 * 1_000_000.0).round() as i64;
        assert!(floor_advance(target_micros, 1.0));
    }
}
