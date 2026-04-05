use std::{path::Path, ptr};

use anyhow::{Context, Result, anyhow};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::{
    Dictionary, codec,
    codec::packet::Packet,
    format,
    software::scaling::{context::Context as ScalingContext, flag::Flags as ScalingFlags},
    util::{format::pixel::Pixel, frame::video::Video, rational::Rational},
};

pub struct Mp4Config {
    pub crf: u8,
    pub preset: String,
}

impl Default for Mp4Config {
    fn default() -> Self {
        Self {
            crf: 18,
            preset: "fast".to_string(),
        }
    }
}

pub fn encode_rgba_frames(
    output_path: impl AsRef<Path>,
    width: u32,
    height: u32,
    fps: u32,
    frame_count: u32,
    config: &Mp4Config,
    mut frame_provider: impl FnMut(u32) -> Result<Vec<u8>>,
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

    let nominal_time_base = Rational(1, fps as i32);
    let stream_time_base = Rational(1, 90_000);

    let mut encoder_ctx = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()?;
    encoder_ctx.set_width(width);
    encoder_ctx.set_height(height);
    encoder_ctx.set_format(Pixel::YUV420P);
    encoder_ctx.set_time_base(nominal_time_base);
    encoder_ctx.set_frame_rate(Some(Rational(fps as i32, 1)));

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
        stream.set_rate(Rational(fps as i32, 1));
        stream.set_avg_frame_rate(Rational(fps as i32, 1));
        stream.set_parameters(&encoder);
        stream.index()
    };

    output.write_header()?;

    let mut scaler = ScalingContext::get(
        Pixel::RGBA,
        width,
        height,
        Pixel::YUV420P,
        width,
        height,
        ScalingFlags::BILINEAR,
    )?;

    for frame_index in 0..frame_count {
        let rgba = frame_provider(frame_index)?;

        let mut rgba_frame = Video::new(Pixel::RGBA, width, height);
        write_rgba_to_frame_ptr(&rgba, &mut rgba_frame, width as usize, height as usize);

        let mut yuv_frame = Video::new(Pixel::YUV420P, width, height);
        scaler.run(&rgba_frame, &mut yuv_frame)?;
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

fn write_rgba_to_frame_ptr(rgba: &[u8], frame: &mut Video, width: usize, height: usize) {
    let stride = frame.stride(0);
    let row_len = width * 4;
    let data_ptr = frame.data_mut(0).as_mut_ptr();

    for y in 0..height {
        let src_start = y * row_len;
        let dst_start = y * stride;
        unsafe {
            ptr::copy_nonoverlapping(
                rgba.as_ptr().add(src_start),
                data_ptr.add(dst_start),
                row_len,
            );
        }
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
