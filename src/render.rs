use std::path::Path;

use anyhow::{Context, Result, anyhow};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::{
    codec,
    codec::packet::Packet,
    format,
    software::scaling::{context::Context as ScalingContext, flag::Flags as ScalingFlags},
    util::{format::pixel::Pixel, frame::video::Video, rational::Rational},
    Dictionary,
};
use skia_safe::{AlphaType, ColorType, ImageInfo, Rect, image::CachingHint, surfaces};

use crate::{Composition, FrameCtx};

pub struct EncodingConfig {
    pub crf: u8,
    pub preset: String,
}

impl Default for EncodingConfig {
    fn default() -> Self {
        Self {
            crf: 18,
            preset: "fast".to_string(),
        }
    }
}

impl Composition {
    pub fn render_to_mp4(
        &self,
        output_path: impl AsRef<Path>,
        config: &EncodingConfig,
    ) -> Result<()> {
        render_to_mp4_impl(self, output_path, config)
    }
}

pub fn render_to_mp4(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    config: &EncodingConfig,
) -> Result<()> {
    render_to_mp4_impl(composition, output_path, config)
}

fn render_to_mp4_impl(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    config: &EncodingConfig,
) -> Result<()> {
    ffmpeg::init()?;

    let output_path = output_path.as_ref();
    let mut output = format::output(output_path).with_context(|| {
        format!(
            "failed to create output context for {}",
            output_path.display()
        )
    })?;

    let codec = ffmpeg::encoder::find(codec::Id::H264)
        .ok_or_else(|| anyhow!("H264 encoder not found in local ffmpeg"))?;

    let nominal_time_base = Rational(1, composition.fps as i32);
    let stream_time_base = Rational(1, 90_000);

    let mut encoder_ctx = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()?;

    encoder_ctx.set_width(composition.width as u32);
    encoder_ctx.set_height(composition.height as u32);
    encoder_ctx.set_format(Pixel::YUV420P);
    encoder_ctx.set_time_base(nominal_time_base);
    encoder_ctx.set_frame_rate(Some(Rational(composition.fps as i32, 1)));

    if output
        .format()
        .flags()
        .contains(format::flag::Flags::GLOBAL_HEADER)
    {
        encoder_ctx.set_flags(codec::flag::Flags::GLOBAL_HEADER);
    }

    let mut encode_options = Dictionary::new();
    encode_options.set("crf", &config.crf.to_string());
    encode_options.set("preset", &config.preset);
    let mut encoder = encoder_ctx.open_as_with(codec, encode_options)?;
    let packet_time_base = nominal_time_base;
    let frame_duration = 1_i64;

    let stream_index = {
        let mut stream = output.add_stream(codec)?;
        stream.set_time_base(stream_time_base);
        stream.set_rate(Rational(composition.fps as i32, 1));
        stream.set_avg_frame_rate(Rational(composition.fps as i32, 1));
        stream.set_parameters(&encoder);
        stream.index()
    };

    output.write_header()?;

    let mut scaler = ScalingContext::get(
        Pixel::RGB24,
        composition.width as u32,
        composition.height as u32,
        Pixel::YUV420P,
        composition.width as u32,
        composition.height as u32,
        ScalingFlags::BILINEAR,
    )?;

    for frame_index in 0..composition.frames {
        let rgb = render_frame_rgb(composition, frame_index)?;

        let mut rgb_frame = Video::new(
            Pixel::RGB24,
            composition.width as u32,
            composition.height as u32,
        );
        copy_rgb_to_frame(
            &rgb,
            &mut rgb_frame,
            composition.width as usize,
            composition.height as usize,
        );

        let mut yuv_frame = Video::new(
            Pixel::YUV420P,
            composition.width as u32,
            composition.height as u32,
        );
        scaler.run(&rgb_frame, &mut yuv_frame)?;
        yuv_frame.set_pts(Some(frame_index as i64));

        encoder.send_frame(&yuv_frame)?;
        write_encoded_packets(
            &mut encoder,
            &mut output,
            stream_index,
            packet_time_base,
            stream_time_base,
            frame_duration,
        )?;
    }

    encoder.send_eof()?;
    write_encoded_packets(
        &mut encoder,
        &mut output,
        stream_index,
        packet_time_base,
        stream_time_base,
        frame_duration,
    )?;

    output.write_trailer()?;
    Ok(())
}

pub fn render_frame_rgb(composition: &Composition, frame_index: u32) -> Result<Vec<u8>> {
    let frame_ctx = FrameCtx {
        frame: frame_index,
        fps: composition.fps,
        width: composition.width,
        height: composition.height,
        frames: composition.frames,
    };

    let node = composition.root_node(&frame_ctx);

    let mut surface = surfaces::raster_n32_premul((composition.width, composition.height))
        .ok_or_else(|| anyhow!("failed to create skia raster surface"))?;
    let canvas = surface.canvas();
    let bounds = Rect::from_xywh(
        0.0,
        0.0,
        composition.width as f32,
        composition.height as f32,
    );
    node.draw(&frame_ctx, canvas, bounds);

    let image = surface.image_snapshot();
    let image_info = ImageInfo::new(
        (composition.width, composition.height),
        ColorType::BGRA8888,
        AlphaType::Premul,
        None,
    );

    let mut bgra = vec![0_u8; (composition.width as usize) * (composition.height as usize) * 4];
    let read_ok = image.read_pixels(
        &image_info,
        bgra.as_mut_slice(),
        (composition.width as usize) * 4,
        (0, 0),
        CachingHint::Allow,
    );

    if !read_ok {
        return Err(anyhow!("failed to read pixels from skia surface"));
    }

    let mut rgb = vec![0_u8; (composition.width as usize) * (composition.height as usize) * 3];
    for (src, dst) in bgra.chunks_exact(4).zip(rgb.chunks_exact_mut(3)) {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
    }

    Ok(rgb)
}

fn copy_rgb_to_frame(rgb: &[u8], frame: &mut Video, width: usize, height: usize) {
    let stride = frame.stride(0);
    let row_len = width * 3;
    let data = frame.data_mut(0);

    for y in 0..height {
        let src_start = y * row_len;
        let src_end = src_start + row_len;
        let dst_start = y * stride;
        let dst_end = dst_start + row_len;

        data[dst_start..dst_end].copy_from_slice(&rgb[src_start..src_end]);
    }
}

fn write_encoded_packets(
    encoder: &mut ffmpeg::codec::encoder::video::Encoder,
    output: &mut format::context::Output,
    stream_index: usize,
    packet_time_base: Rational,
    stream_time_base: Rational,
    frame_duration: i64,
) -> Result<()> {
    let mut packet = Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        if packet.duration() == 0 {
            packet.set_duration(frame_duration);
        }
        packet.rescale_ts(packet_time_base, stream_time_base);
        packet.set_stream(stream_index);
        packet.write_interleaved(output)?;
    }

    Ok(())
}
