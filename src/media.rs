use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::{
    format,
    software::scaling::{context::Context as ScalingContext, flag::Flags as ScalingFlags},
    util::format::pixel::Pixel,
};
use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo, image::CachingHint};

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

        let codec_ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
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
            return Ok(self.current_frame.clone().unwrap());
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

            let packed = self.pack_rgba(&rgba);

            self.current_pts_secs = pts_secs;
            self.current_frame = Some(Arc::new(packed));

            if pts_secs >= target_secs {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn pack_rgba(&self, frame: &ffmpeg::frame::Video) -> Vec<u8> {
        let stride = frame.stride(0) as usize;
        let row_bytes = self.width as usize * 4;
        let mut packed = Vec::with_capacity(row_bytes * self.height as usize);
        for y in 0..self.height as usize {
            let start = y * stride;
            packed.extend_from_slice(&frame.data(0)[start..start + row_bytes]);
        }
        packed
    }
}

pub struct MediaContext {
    decoders: HashMap<PathBuf, VideoDecoder>,
    images: HashMap<PathBuf, (Arc<Vec<u8>>, u32, u32)>,
}

impl MediaContext {
    pub fn new() -> Self {
        Self {
            decoders: HashMap::new(),
            images: HashMap::new(),
        }
    }

    pub fn get_video_frame(&mut self, path: &Path, target_time_secs: f64) -> Result<Arc<Vec<u8>>> {
        if !self.decoders.contains_key(path) {
            let decoder = VideoDecoder::open(path)?;
            self.decoders.insert(path.to_path_buf(), decoder);
        }
        self.decoders
            .get_mut(path)
            .unwrap()
            .get_frame_at_time(target_time_secs)
    }

    pub fn video_info(&mut self, path: &Path) -> Result<VideoInfo> {
        if !self.decoders.contains_key(path) {
            let decoder = VideoDecoder::open(path)?;
            self.decoders.insert(path.to_path_buf(), decoder);
        }
        Ok(self.decoders.get(path).unwrap().info())
    }

    pub fn get_bitmap(
        &mut self,
        path: &Path,
        target_time_secs: f64,
    ) -> Result<(Arc<Vec<u8>>, u32, u32)> {
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("mp4" | "mov" | "m4v" | "webm" | "mkv" | "avi") => {
                let data = self.get_video_frame(path, target_time_secs)?;
                let info = self.video_info(path)?;
                Ok((data, info.width, info.height))
            }
            _ => {
                if !self.images.contains_key(path) {
                    let bitmap = load_image_bitmap(path)?;
                    self.images.insert(path.to_path_buf(), bitmap);
                }

                Ok(self
                    .images
                    .get(path)
                    .expect("cached image bitmap should exist")
                    .clone())
            }
        }
    }
}

fn load_image_bitmap(path: &Path) -> Result<(Arc<Vec<u8>>, u32, u32)> {
    let encoded = fs::read(path)
        .with_context(|| format!("failed to read image bytes: {}", path.display()))?;
    let image = Image::from_encoded(Data::new_copy(&encoded))
        .ok_or_else(|| anyhow!("failed to decode image: {}", path.display()))?;

    let width = image.width() as u32;
    let height = image.height() as u32;
    let row_bytes = width as usize * 4;
    let mut pixels = vec![0_u8; row_bytes * height as usize];
    let info = ImageInfo::new(
        (width as i32, height as i32),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );

    let ok = image.read_pixels(
        &info,
        pixels.as_mut_slice(),
        row_bytes,
        (0, 0),
        CachingHint::Allow,
    );
    if !ok {
        return Err(anyhow!(
            "failed to convert decoded image into RGBA pixels: {}",
            path.display()
        ));
    }

    Ok((Arc::new(pixels), width, height))
}
