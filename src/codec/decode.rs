use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::threading::{Config, Type};
use ffmpeg_next::{
    format,
    software::scaling::{context::Context as ScalingContext, flag::Flags as ScalingFlags},
    util::format::pixel::Pixel,
};

pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
}

struct VideoDecoder {
    input: format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: ScalingContext,
    stream_index: usize,
    time_base: ffmpeg::util::rational::Rational,
    width: u32,
    height: u32,
    current_pts_secs: f64,
    current_frame: Option<Arc<Vec<u8>>>,
    eof: bool,
}

impl VideoDecoder {
    fn open(path: &Path) -> Result<Self> {
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
        let scaler = ScalingContext::get(
            decoder.format(),
            width,
            height,
            Pixel::RGBA,
            width,
            height,
            ScalingFlags::BILINEAR,
        )?;

        Ok(Self {
            input,
            decoder,
            scaler,
            stream_index,
            time_base,
            width,
            height,
            current_pts_secs: -1.0,
            current_frame: None,
            eof: false,
        })
    }

    fn info(&self) -> VideoInfo {
        VideoInfo {
            width: self.width,
            height: self.height,
        }
    }

    fn get_frame_at_time(&mut self, target_secs: f64) -> Result<Arc<Vec<u8>>> {
        if self.current_frame.is_some() && (self.current_pts_secs - target_secs).abs() < 1e-6 {
            return Ok(self
                .current_frame
                .clone()
                .expect("current frame should exist"));
        }

        if target_secs <= self.current_pts_secs {
            return self
                .current_frame
                .clone()
                .ok_or_else(|| anyhow!("no frame available"));
        }

        self.decode_forward(target_secs)?;
        self.current_frame
            .clone()
            .ok_or_else(|| anyhow!("failed to decode frame at {:.3}s", target_secs))
    }

    fn decode_forward(&mut self, target_secs: f64) -> Result<()> {
        if self.eof {
            return Ok(());
        }

        loop {
            let packet = self.read_next_packet();
            let Some(packet) = packet else { break };

            self.decoder.send_packet(&packet)?;
            if self.receive_until(target_secs)? {
                return Ok(());
            }
        }

        self.eof = true;
        self.decoder.send_eof()?;
        self.receive_until(target_secs)?;
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

    fn receive_until(&mut self, target_secs: f64) -> Result<bool> {
        let mut frame = ffmpeg::frame::Video::empty();
        while self.decoder.receive_frame(&mut frame).is_ok() {
            let pts = frame.pts().unwrap_or(0);
            let pts_secs = pts as f64 * self.time_base.numerator() as f64
                / self.time_base.denominator() as f64;

            let mut rgba = ffmpeg::frame::Video::new(Pixel::RGBA, self.width, self.height);
            self.scaler.run(&frame, &mut rgba)?;

            self.current_pts_secs = pts_secs;
            self.current_frame = Some(Arc::new(pack_rgba(&rgba, self.width, self.height)));

            if pts_secs >= target_secs {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

pub struct VideoDecodeCache {
    decoders: HashMap<PathBuf, VideoDecoder>,
}

impl VideoDecodeCache {
    pub fn new() -> Self {
        Self {
            decoders: HashMap::new(),
        }
    }

    pub fn get_frame(&mut self, path: &Path, target_time_secs: f64) -> Result<Arc<Vec<u8>>> {
        if !self.decoders.contains_key(path) {
            let decoder = VideoDecoder::open(path)?;
            self.decoders.insert(path.to_path_buf(), decoder);
        }
        self.decoders
            .get_mut(path)
            .expect("video decoder should exist")
            .get_frame_at_time(target_time_secs)
    }

    pub fn info(&mut self, path: &Path) -> Result<VideoInfo> {
        if !self.decoders.contains_key(path) {
            let decoder = VideoDecoder::open(path)?;
            self.decoders.insert(path.to_path_buf(), decoder);
        }
        Ok(self
            .decoders
            .get(path)
            .expect("video decoder should exist")
            .info())
    }
}

impl Default for VideoDecodeCache {
    fn default() -> Self {
        Self::new()
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
