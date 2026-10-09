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
    scalers: VecDeque<((u32, u32, ColorspaceSpec), YuvRgbScaler)>,
    source_format: Pixel,
    stream_index: usize,
    time_base: ffmpeg::util::rational::Rational,
    width: u32,
    height: u32,
    duration_secs: Option<f64>,
    keyframe_pts_us: Vec<u64>,
    /// Author-asserted YUV→RGB matrix (`data-color-matrix`). Stream metadata
    /// is unreliable for capture-tool encodes (BT.601 data tagged BT.709), so
    /// when present this wins over the frame/codec-declared colorspace.
    color_matrix_override: Option<opencat_core::media::VideoColorMatrix>,
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
        Self::open_with_matrix(path, None)
    }

    fn open_with_matrix(
        path: &Path,
        color_matrix_override: Option<opencat_core::media::VideoColorMatrix>,
    ) -> Result<Self> {
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
        // The initial context is built with the effective matrix (author
        // override when present, else the stream's declared one) so the very
        // first frame already converts correctly (per-frame specs that
        // disagree with this fall back to the key-mismatch rebuild in
        // `scaler_for`). AVCodecContext colorspace is populated from the
        // container metadata by the demuxer/parser before the first decode.
        let initial_spec = ColorspaceSpec::effective(
            ColorspaceSpec::from_codec(&decoder),
            color_matrix_override,
        );
        let mut scalers = VecDeque::with_capacity(MAX_SCALERS_PER_DECODER);
        scalers.push_back((
            (width, height, initial_spec),
            YuvRgbScaler::new(
                source_format,
                width,
                height,
                Pixel::RGBA,
                width,
                height,
                ScalingFlags::BILINEAR,
                initial_spec,
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
            color_matrix_override,
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

        let spec = ColorspaceSpec::effective(
            ColorspaceSpec::from_frame(frame),
            self.color_matrix_override,
        );
        let scaler = self.scaler_for(out_w, out_h, spec)?;
        let mut rgba = ffmpeg::frame::Video::new(Pixel::RGBA, out_w, out_h);
        scaler.run(frame, &mut rgba)?;
        self.current_pts_secs = pts_secs;
        self.current_size = Some((out_w, out_h));
        self.current_frame = Some(Arc::new(pack_rgba(&rgba, out_w, out_h)));
        Ok(())
    }

    fn scaler_for(&mut self, out_w: u32, out_h: u32, spec: ColorspaceSpec) -> Result<&mut YuvRgbScaler> {
        if let Some(idx) = self
            .scalers
            .iter()
            .position(|(key, _)| *key == (out_w, out_h, spec))
        {
            // LRU touch: move to back.
            if idx + 1 != self.scalers.len() {
                let entry = self.scalers.remove(idx).expect("index validated above");
                self.scalers.push_back(entry);
            }
        } else {
            let scaler = YuvRgbScaler::new(
                self.source_format,
                self.width,
                self.height,
                Pixel::RGBA,
                out_w,
                out_h,
                ScalingFlags::BILINEAR,
                spec,
            )?;
            if self.scalers.len() >= MAX_SCALERS_PER_DECODER {
                self.scalers.pop_front();
            }
            self.scalers
                .push_back(((out_w, out_h, spec), scaler));
        }
        Ok(&mut self.scalers.back_mut().expect("scaler just pushed").1)
    }
}

/// The YUV→RGB matrix a scaling context is built with. Streams decoded from
/// modern HD containers are BT.709; swscale's built-in default is the BT.601
/// family, which tints every converted pixel (~8/255 mean on a real HD frame
/// versus the matrix the container declares).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ColorspaceSpec {
    /// swscale built-in default (BT.601 coefficients, limited-range input) —
    /// the historical behavior for untagged streams.
    Default,
    /// BT.709 coefficients with the frame's declared range.
    BT709 { full_range: bool },
}

impl ColorspaceSpec {
    fn from_frame(frame: &frame::Video) -> Self {
        use ffmpeg::color::{Range, Space};
        match frame.color_space() {
            Space::BT709 => ColorspaceSpec::BT709 {
                full_range: frame.color_range() == Range::JPEG,
            },
            _ => ColorspaceSpec::Default,
        }
    }

    /// Stream-level fallback for the initial context: the codec context
    /// carries the container metadata before the first frame decodes.
    fn from_codec(decoder: &ffmpeg::decoder::Video) -> Self {
        use ffmpeg::color::{Range, Space};
        match decoder.color_space() {
            Space::BT709 => ColorspaceSpec::BT709 {
                full_range: decoder.color_range() == Range::JPEG,
            },
            _ => ColorspaceSpec::Default,
        }
    }

    /// The spec a decode actually uses: the author-asserted matrix when
    /// present (stream tags are unreliable for capture encodes), otherwise
    /// the stream's declared colorspace.
    fn effective(
        declared: ColorspaceSpec,
        override_matrix: Option<opencat_core::media::VideoColorMatrix>,
    ) -> Self {
        match override_matrix {
            Some(opencat_core::media::VideoColorMatrix::Bt601) => ColorspaceSpec::Default,
            Some(opencat_core::media::VideoColorMatrix::Bt709) => ColorspaceSpec::BT709 {
                full_range: matches!(declared, ColorspaceSpec::BT709 { full_range: true }),
            },
            None => declared,
        }
    }
}

/// A yuv→rgb scaling context built through the documented allocation
/// sequence — `sws_alloc_context` → AVOptions → `sws_setColorspaceDetails`
/// → `sws_init_context`.
///
/// `sws_getContext` cannot express a source matrix: it initializes the
/// context immediately with the BT.601 default coefficients, and calling
/// `sws_setColorspaceDetails` afterwards corrupts the pre-expanded conversion
/// tables on FFmpeg ≥7 (the conversion goes black), so the coefficients must
/// land before `sws_init_context`, exactly as the libswscale docs prescribe.
struct YuvRgbScaler {
    ptr: *mut ffmpeg::sys::SwsContext,
}

// The context is owned exclusively by the decoder that built it; swscale
// contexts are not thread-bound but the decode lanes never share one.
unsafe impl Send for YuvRgbScaler {}

impl YuvRgbScaler {
    #[allow(clippy::too_many_arguments)]
    fn new(
        src_format: Pixel,
        src_w: u32,
        src_h: u32,
        dst_format: Pixel,
        dst_w: u32,
        dst_h: u32,
        flags: ScalingFlags,
        spec: ColorspaceSpec,
    ) -> Result<Self> {
        use ffmpeg::sys;
        unsafe {
            let ptr = sys::sws_alloc_context();
            if ptr.is_null() {
                return Err(anyhow!("sws_alloc_context failed"));
            }
            // swscale AVOptions (`libswscale/options.c`): dimensions, formats,
            // rescale flags, and the source range. Names are NUL-terminated
            // via CStr literals.
            let set_opt = |ctx: *mut sys::SwsContext, name: &str, value: i64| {
                let cstr = std::ffi::CString::new(name).expect("option name");
                sys::av_opt_set_int(
                    ctx.cast(),
                    cstr.as_ptr(),
                    value,
                    0, // search flags: exact object only
                );
            };
            set_opt(ptr, "srcw", i64::from(src_w));
            set_opt(ptr, "srch", i64::from(src_h));
            let src_fmt: sys::AVPixelFormat = src_format.into();
            // bindgen's `AVPixelFormat` is a repr(C) int enum; the numeric id
            // feeds the "src_format" AVOption.
            set_opt(ptr, "src_format", i64::from(unsafe {
                std::mem::transmute::<sys::AVPixelFormat, std::os::raw::c_int>(src_fmt)
            }));
            set_opt(ptr, "dstw", i64::from(dst_w));
            set_opt(ptr, "dsth", i64::from(dst_h));
            let dst_fmt: sys::AVPixelFormat = dst_format.into();
            set_opt(ptr, "dst_format", i64::from(unsafe {
                std::mem::transmute::<sys::AVPixelFormat, std::os::raw::c_int>(dst_fmt)
            }));
            set_opt(ptr, "flags", i64::from(flags.bits()));
            // srcRange semantics (libswscale/swscale.h): 1 = jpeg/full, 0 =
            // mpeg/limited. Untagged sources keep the built-in default.
            if let ColorspaceSpec::BT709 { full_range } = spec {
                sys::sws_setColorspaceDetails(
                    ptr,
                    sys::sws_getCoefficients(sys::SWS_CS_ITU709),
                    i32::from(full_range),
                    sys::sws_getCoefficients(sys::SWS_CS_DEFAULT),
                    0, // RGBA destination is always full range
                    0,
                    1 << 16, // contrast/saturation are 16.16 fixed point; 1<<16 == 1.0
                    1 << 16,
                );
            }
            let rc = sys::sws_init_context(ptr, std::ptr::null_mut(), std::ptr::null_mut());
            if rc < 0 {
                sys::sws_freeContext(ptr);
                return Err(anyhow!("sws_init_context failed: {rc}"));
            }
            Ok(Self { ptr })
        }
    }

    fn run(&mut self, input: &frame::Video, output: &mut frame::Video) -> Result<()> {
        unsafe {
            let rc = ffmpeg::sys::sws_scale(
                self.ptr,
                (*input.as_ptr()).data.as_ptr() as *const *const _,
                (*input.as_ptr()).linesize.as_ptr() as *const _,
                0,
                input.height() as std::os::raw::c_int,
                (*output.as_mut_ptr()).data.as_ptr(),
                (*output.as_mut_ptr()).linesize.as_ptr() as *mut _,
            );
            if rc < 0 {
                return Err(anyhow!("sws_scale failed: {rc}"));
            }
        }
        Ok(())
    }
}

impl Drop for YuvRgbScaler {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { ffmpeg::sys::sws_freeContext(self.ptr) };
            self.ptr = std::ptr::null_mut();
        }
    }
}

pub struct VideoDecodeCache {
    decoders: HashMap<PathBuf, VecDeque<VideoDecoder>>,
    max_lanes_per_asset: usize,
    /// Per-asset author-asserted YUV→RGB matrices (`data-color-matrix`).
    color_matrix_overrides: HashMap<PathBuf, opencat_core::media::VideoColorMatrix>,
}

impl VideoDecodeCache {
    pub fn new() -> Self {
        Self::with_max_lanes_per_asset(4)
    }

    pub fn with_max_lanes_per_asset(max_lanes_per_asset: usize) -> Self {
        Self {
            decoders: HashMap::new(),
            max_lanes_per_asset: max_lanes_per_asset.max(1),
            color_matrix_overrides: HashMap::new(),
        }
    }

    /// Author-asserted YUV→RGB matrix for an asset (`data-color-matrix`).
    /// Lanes already open with a different override are dropped so the next
    /// decode re-opens with the declared conversion.
    pub fn set_color_matrix_override(
        &mut self,
        path: &Path,
        matrix: opencat_core::media::VideoColorMatrix,
    ) {
        if self.color_matrix_overrides.get(path) == Some(&matrix) {
            return;
        }
        self.color_matrix_overrides.insert(path.to_path_buf(), matrix);
        self.decoders.remove(path);
    }

    fn override_for(&self, path: &Path) -> Option<opencat_core::media::VideoColorMatrix> {
        self.color_matrix_overrides.get(path).copied()
    }

    pub fn get_frame(
        &mut self,
        path: &Path,
        target_time_secs: f64,
        quality: crate::media::VideoPreviewQuality,
        target_size: Option<(u32, u32)>,
    ) -> Result<Arc<Vec<u8>>> {
        let path_buf = path.to_path_buf();
        let override_matrix = self.override_for(path);
        let decoders = self.decoders.entry(path_buf).or_default();
        let cursors: Vec<_> = decoders.iter().map(VideoDecoder::cursor).collect();
        let selection = select_decoder_lane(
            &cursors,
            target_time_secs,
            quality,
            self.max_lanes_per_asset,
        );

        let mut decoder = match selection {
            DecoderLaneSelection::Reuse(index) => {
                let lane = decoders
                    .remove(index)
                    .expect("selected decoder lane should exist");
                if lane.color_matrix_override == override_matrix {
                    lane
                } else {
                    // The lane predates the asset's `data-color-matrix`
                    // declaration; its scalers bake the wrong matrix, so
                    // re-open with the declared conversion.
                    VideoDecoder::open_with_matrix(path, override_matrix)?
                }
            }
            DecoderLaneSelection::OpenNew => {
                VideoDecoder::open_with_matrix(path, override_matrix)?
            }
        };

        let frame = decoder.get_frame_at_time(target_time_secs, quality, target_size)?;
        decoders.push_back(decoder);
        Ok(frame)
    }

    pub fn info(&mut self, path: &Path) -> Result<VideoInfo> {
        if !self.decoders.contains_key(path) {
            let decoder = VideoDecoder::open_with_matrix(path, self.override_for(path))?;
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
    use super::ColorspaceSpec;
    use super::ScalingFlags;
    use super::Pixel;
    use super::YuvRgbScaler;
    use super::floor_advance;
    use super::VideoDecodeCache;
    use ffmpeg_next as ffmpeg;

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

    /// `data-color-matrix` overrides the stream's declared colorspace.
    /// Capture tools routinely encode BT.601 while tagging BT.709, so the
    /// author assertion must win: `bt601` forces the swscale-default spec,
    /// `bt709` keeps the 709 coefficients, and no override keeps `declared`.
    #[test]
    fn color_matrix_override_beats_declared_metadata() {
        use opencat_core::media::VideoColorMatrix;

        let declared = ColorspaceSpec::BT709 { full_range: false };
        assert_eq!(
            ColorspaceSpec::effective(declared, Some(VideoColorMatrix::Bt601)),
            ColorspaceSpec::Default,
            "bt601 override must decode with the swscale-default (601) matrix"
        );
        assert_eq!(
            ColorspaceSpec::effective(declared, Some(VideoColorMatrix::Bt709)),
            declared,
            "bt709 override keeps the 709 coefficients"
        );
        assert_eq!(
            ColorspaceSpec::effective(declared, None),
            declared,
            "no override keeps the declared spec"
        );
    }

    /// The decode cache must drop lanes whose override differs, so a newly
    /// declared matrix is honored instead of reusing scalers built with the
    /// old one.
    #[test]
    fn decode_cache_override_drops_stale_lanes() {
        use opencat_core::media::VideoColorMatrix;
        use std::path::Path;

        let mut cache = VideoDecodeCache::new();
        // No decoder open yet: recording an override must not panic and must
        // be observable through the override table.
        cache.set_color_matrix_override(Path::new("clip.mp4"), VideoColorMatrix::Bt601);
        assert_eq!(cache.override_for(Path::new("clip.mp4")), Some(VideoColorMatrix::Bt601));
        assert!(
            !cache.decoders.contains_key(Path::new("clip.mp4")),
            "override bookkeeping must not open lanes"
        );
        // Re-setting the same value is a no-op for the lane table.
        cache.set_color_matrix_override(Path::new("clip.mp4"), VideoColorMatrix::Bt601);
    }

    /// Within a frame interval the floor frame wins; exactly on a boundary the
    /// frame starting there wins (its pts micros equals the target micros).
    #[test]
    fn boundary_pts_equal_to_target_selects_the_frame() {
        let target_micros = (1.0_f64 * 1_000_000.0).round() as i64;
        assert!(floor_advance(target_micros, 1.0));
    }

    /// A BT.709-labelled stream must be converted with the BT.709 matrix —
    /// the matrix a browser `<video>` decodes with — not swscale's BT.601
    /// default (getting it wrong tints every video texture ~8/255 mean on a
    /// real HD frame). Verified at the pixel level: BT.709/limited green
    /// (Y 173, U 42, V 26) must decode to green with R≈0, while the 601
    /// default gives R≈20; and 709 white (Y 235, U=V=128) must come out
    /// full-range (≈255), never RGB-limited (≈235).
    #[test]
    fn bt709_frame_metadata_selects_bt709_sws_coefficients() {
        use ffmpeg::{color::Range, color::Space};

        // BT.709 green: R must land at ≈0, not the 601 default's ≈20.
        let mut scaler = YuvRgbScaler::new(
            Pixel::YUV420P,
            4,
            4,
            Pixel::RGBA,
            4,
            4,
            ScalingFlags::BILINEAR,
            ColorspaceSpec::BT709 { full_range: false },
        )
        .expect("scaler context");
        let frame = solid_yuv_frame(173, 42, 26);
        let rgba = convert(&mut scaler, &frame);
        assert!(rgba[0] <= 3, "R={} (601 residue would be ~20)", rgba[0]);
        assert!(rgba[1] >= 250, "G={}", rgba[1]);

        // BT.709 white must land full-range: ≈255, not ≈235 (limited RGB).
        let mut scaler = YuvRgbScaler::new(
            Pixel::YUV420P,
            4,
            4,
            Pixel::RGBA,
            4,
            4,
            ScalingFlags::BILINEAR,
            ColorspaceSpec::BT709 { full_range: false },
        )
        .expect("scaler context");
        let white = solid_yuv_frame(235, 128, 128);
        let rgba = convert(&mut scaler, &white);
        assert!(rgba[0] >= 253, "R={} (RGB-limited would be ~235)", rgba[0]);

        // Full-range label must widen the luma expansion (JPEG white Y=255).
        let mut scaler = YuvRgbScaler::new(
            Pixel::YUV420P,
            4,
            4,
            Pixel::RGBA,
            4,
            4,
            ScalingFlags::BILINEAR,
            ColorspaceSpec::BT709 { full_range: true },
        )
        .expect("scaler context");
        let jpeg_white = solid_yuv_frame(255, 128, 128);
        let rgba = convert(&mut scaler, &jpeg_white);
        assert!(rgba[0] >= 253, "R={} (limited expansion would be ~235)", rgba[0]);

        // Untagged spec keeps the historical 601 default: 709 green decoded
        // through it shows the 601 residue (this is the pre-fix behavior,
        // pinned so the default-path semantics cannot silently drift).
        let mut scaler = YuvRgbScaler::new(
            Pixel::YUV420P,
            4,
            4,
            Pixel::RGBA,
            4,
            4,
            ScalingFlags::BILINEAR,
            ColorspaceSpec::Default,
        )
        .expect("scaler context");
        let frame = solid_yuv_frame(173, 42, 26);
        let rgba = convert(&mut scaler, &frame);
        assert!(
            (11..=28).contains(&rgba[0]),
            "R={} (untagged default should show 601 residue ~19)",
            rgba[0]
        );

        // Spec equality keys the per-decoder scaler cache.
        let mut mpeg = solid_yuv_frame(0, 128, 128);
        mpeg.set_color_space(Space::BT709);
        mpeg.set_color_range(Range::MPEG);
        assert_eq!(
            ColorspaceSpec::from_frame(&mpeg),
            ColorspaceSpec::BT709 { full_range: false }
        );
        let mut untagged = solid_yuv_frame(0, 128, 128);
        untagged.set_color_space(Space::Unspecified);
        assert_eq!(ColorspaceSpec::from_frame(&untagged), ColorspaceSpec::Default);
    }

    /// Real-asset end-to-end probe: decode an actual BT.709 mp4 through the
    /// production decoder and confirm the returned RGBA is not black. This
    /// closes the gap the synthetic-frame test above cannot see (decoder
    /// plane layout / initial-context reuse). Ignored by default; run with
    /// `--ignored` and LBR_FACET_MP4 pointing at a bt709 yuv420p asset.
    #[test]
    #[ignore = "needs a real mp4 asset; set LBR_FACET_MP4"]
    fn real_bt709_asset_decodes_nonblack() {
        let path = std::env::var("LBR_FACET_MP4")
            .expect("set LBR_FACET_MP4 to a bt709 yuv420p mp4");
        let mut cache = VideoDecodeCache::new();
        let rgba = cache
            .get_frame(
                std::path::Path::new(&path),
                31.0 / 30.0,
                crate::media::VideoPreviewQuality::Exact,
                None,
            )
            .expect("decode frame");
        let nonblack = rgba
            .iter()
            .step_by(4)
            .filter(|&&r| r > 8)
            .count();
        assert!(
            nonblack > rgba.len() / 8,
            "decoded frame looks black: {nonblack} nonblack R of {} px",
            rgba.len() / 4
        );
    }

    fn solid_yuv_frame(y: u8, u: u8, v: u8) -> ffmpeg::frame::Video {
        let mut frame = ffmpeg::frame::Video::new(Pixel::YUV420P, 4, 4);
        frame.data_mut(0).fill(y);
        frame.data_mut(1).fill(u);
        frame.data_mut(2).fill(v);
        frame
    }

    fn convert(scaler: &mut YuvRgbScaler, frame: &ffmpeg::frame::Video) -> Vec<u8> {
        let mut rgba = ffmpeg::frame::Video::new(Pixel::RGBA, 4, 4);
        scaler
            .run(frame, &mut rgba)
            .expect("yuv→rgb conversion");
        rgba.data(0)[..4].to_vec()
    }
}

#[cfg(test)]
mod override_integration_tests {
    use super::*;

    /// End-to-end: a BT.709-tagged asset decoded with the bt601 override must
    /// produce different pixels than the automatic (709) decode — proving the
    /// override reaches the scaler, not just the spec math.
    #[test]
    fn real_asset_decode_honors_override() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/website-to-hyperframes/assets/w2h-a3/a3-01-stripe.mp4"
        ));
        if !path.exists() {
            return; // asset not present in this checkout
        }
        let auto = VideoDecoder::open(path).unwrap();
        let forced = VideoDecoder::open_with_matrix(
            path,
            Some(opencat_core::media::VideoColorMatrix::Bt601),
        )
        .unwrap();
        assert!(auto.color_matrix_override.is_none());
        assert_eq!(
            forced.color_matrix_override,
            Some(opencat_core::media::VideoColorMatrix::Bt601)
        );

        let mut auto = auto;
        let mut forced = forced;
        let a = auto
            .get_frame_at_time(0.5, crate::media::VideoPreviewQuality::Exact, None)
            .unwrap();
        let b = forced
            .get_frame_at_time(0.5, crate::media::VideoPreviewQuality::Exact, None)
            .unwrap();
        let diff: u64 = a
            .iter()
            .zip(b.iter())
            .map(|(x, y)| (i64::from(*x) - i64::from(*y)).unsigned_abs())
            .sum();
        assert!(
            diff > 1_000_000,
            "bt601 override must change decoded pixels on a 709-tagged asset (diff={diff})"
        );
    }
}
