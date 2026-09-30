use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::mpsc::{Receiver, sync_channel};

use anyhow::{Context, Result, anyhow};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::{
    ChannelLayout, Dictionary, codec,
    codec::packet::Packet,
    format,
    software::{
        resampling::context::Context as ResamplingContext,
        scaling::{context::Context as ScalingContext, flag::Flags as ScalingFlags},
    },
    util::{
        format::{
            pixel::Pixel,
            sample::{Sample, Type as SampleType},
        },
        frame::{audio::Audio as AudioFrame, video::Video},
        rational::Rational,
    },
};

use crate::media::decode::AudioTrack;

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

/// Depth of the bounded channel feeding the encode thread. Large enough to
/// keep the encoder busy while the render thread produces the next frame,
/// small enough to bound the number of RGBA frames held in memory.
const FRAME_QUEUE_DEPTH: usize = 3;

/// Encode-thread body: consumes rendered RGBA frames from `frames` and runs
/// the exact per-frame sequence the previous synchronous loop ran
/// (copy → swscale → send_frame → drain packets), then EOF drain, audio mux
/// and trailer. Runs concurrently with rendering, but the frame order and all
/// encoder/muxer calls are identical to the serial loop, so the output file is
/// byte-for-byte unchanged.
#[allow(clippy::too_many_arguments)]
fn encode_worker(
    mut output: format::context::Output,
    output_path: PathBuf,
    mut video_encoder: ffmpeg::codec::encoder::video::Encoder,
    video_stream_index: usize,
    video_packet_time_base: Rational,
    stream_time_base: Rational,
    video_frame_duration: i64,
    width: u32,
    height: u32,
    frame_count: u32,
    audio_track: Option<AudioTrack>,
    frames: Receiver<Vec<u8>>,
    mut on_video_frame_encoded: impl FnMut(u32, u32),
) -> Result<()> {
    let audio_context = match &audio_track {
        Some(track) if !track.is_empty() => Some(create_audio_output_context(
            &mut output,
            &output_path,
            track,
        )?),
        _ => None,
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
        let rgba = frames
            .recv()
            .context("render channel closed before all video frames were encoded")?;

        let mut rgba_frame = Video::new(Pixel::RGBA, width, height);
        write_rgba_to_frame_ptr(&rgba, &mut rgba_frame, width as usize, height as usize);

        let mut yuv_frame = Video::new(Pixel::YUV420P, width, height);
        scaler.run(&rgba_frame, &mut yuv_frame)?;
        yuv_frame.set_pts(Some(frame_index as i64));

        video_encoder.send_frame(&yuv_frame)?;
        write_video_packets(
            &mut video_encoder,
            &mut output,
            video_stream_index,
            video_packet_time_base,
            stream_time_base,
            video_frame_duration,
        )?;
        on_video_frame_encoded(frame_index + 1, frame_count);
    }

    video_encoder.send_eof()?;
    write_video_packets(
        &mut video_encoder,
        &mut output,
        video_stream_index,
        video_packet_time_base,
        stream_time_base,
        video_frame_duration,
    )?;

    if let Some(audio_context) = audio_context {
        write_audio_track(audio_context, &mut output)?;
    }

    output.write_trailer()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn encode_rgba_frames(
    output_path: impl AsRef<Path>,
    width: u32,
    height: u32,
    fps: u32,
    frame_count: u32,
    config: &Mp4Config,
    audio_track: Option<&AudioTrack>,
    on_video_frame_encoded: impl FnMut(u32, u32) + Send + 'static,
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

    let video_codec = select_video_encoder()?;

    let nominal_time_base = Rational(1, fps as i32);
    let stream_time_base = Rational(1, 90_000);

    let mut video_encoder_ctx = ffmpeg::codec::context::Context::new_with_codec(video_codec)
        .encoder()
        .video()?;
    video_encoder_ctx.set_width(width);
    video_encoder_ctx.set_height(height);
    video_encoder_ctx.set_format(Pixel::YUV420P);
    video_encoder_ctx.set_time_base(nominal_time_base);
    video_encoder_ctx.set_frame_rate(Some(Rational(fps as i32, 1)));

    if output
        .format()
        .flags()
        .contains(format::flag::Flags::GLOBAL_HEADER)
    {
        video_encoder_ctx.set_flags(codec::flag::Flags::GLOBAL_HEADER);
    }

    let mut encode_options = Dictionary::new();
    if video_codec.id() == codec::Id::H264 {
        encode_options.set("crf", &config.crf.to_string());
        encode_options.set("preset", &config.preset);
    }
    let video_encoder = video_encoder_ctx.open_as_with(video_codec, encode_options)?;
    let video_packet_time_base = nominal_time_base;
    let video_frame_duration = 1_i64;

    let video_stream_index = {
        let mut stream = output.add_stream(video_codec)?;
        stream.set_time_base(stream_time_base);
        stream.set_rate(Rational(fps as i32, 1));
        stream.set_avg_frame_rate(Rational(fps as i32, 1));
        stream.set_parameters(&video_encoder);
        stream.index()
    };

    // The audio output context borrows the track for the whole mux; hand the
    // encode thread its own clone so it can own the data ('static spawn).
    let audio_track_owned = audio_track.map(AudioTrack::clone);
    let output_path = output_path.to_path_buf();

    // Producer/consumer: rendering stays on this thread (the JS/Skia render
    // pipeline is not Send), while swscale + x264 + muxing run on a dedicated
    // thread fed by a bounded channel. The channel applies back-pressure so at
    // most FRAME_QUEUE_DEPTH rendered frames queue up while the encoder works
    // through earlier ones — encode overlaps render instead of serializing
    // behind it.
    let (frame_tx, frame_rx) = sync_channel::<Vec<u8>>(FRAME_QUEUE_DEPTH);

    let encode_thread = std::thread::Builder::new()
        .name("opencat-encode".to_string())
        .spawn(move || {
            encode_worker(
                output,
                output_path,
                video_encoder,
                video_stream_index,
                video_packet_time_base,
                stream_time_base,
                video_frame_duration,
                width,
                height,
                frame_count,
                audio_track_owned,
                frame_rx,
                on_video_frame_encoded,
            )
        })?;

    // Producer loop: identical per-frame render calls, in the same order, as
    // the previous serial loop.
    let mut provider_error = None;
    for frame_index in 0..frame_count {
        match frame_provider(frame_index) {
            Ok(rgba) => {
                // Blocks while the encoder is FRAME_QUEUE_DEPTH frames behind;
                // fails only when the encode thread already exited on error.
                if frame_tx.send(rgba).is_err() {
                    break;
                }
            }
            Err(err) => {
                provider_error = Some(err);
                break;
            }
        }
    }
    drop(frame_tx);

    let encode_result = encode_thread
        .join()
        .map_err(|panic| anyhow!("mp4 encode thread panicked: {panic:?}"))
        .and_then(std::convert::identity);

    // A render error takes precedence: the encode thread's "channel closed"
    // error is only the consequence of the producer stopping early.
    match provider_error {
        Some(err) => Err(err),
        None => encode_result,
    }
}

fn select_video_encoder() -> Result<ffmpeg::Codec> {
    const SOFTWARE_H264_ENCODERS: [&str; 2] = ["libx264", "libopenh264"];

    for name in SOFTWARE_H264_ENCODERS {
        let Some(codec) = ffmpeg::encoder::find_by_name(name) else {
            continue;
        };

        if codec_supports_pixel_format(codec, Pixel::YUV420P) {
            eprintln!("[export] using software H.264 encoder `{}`", codec.name());
            return Ok(codec);
        }
    }

    Err(anyhow!(
        "no compatible software H.264 encoder found for yuv420p input frames (tried libx264 and libopenh264)"
    ))
}

fn codec_supports_pixel_format(codec: ffmpeg::Codec, pixel_format: Pixel) -> bool {
    codec
        .video()
        .ok()
        .and_then(|video| video.formats())
        .map(|mut formats| formats.any(|format| format == pixel_format))
        .unwrap_or(true)
}

struct AudioOutputContext<'a> {
    track: &'a AudioTrack,
    encoder: ffmpeg::codec::encoder::audio::Encoder,
    resampler: ResamplingContext,
    stream_index: usize,
    packet_time_base: Rational,
    stream_time_base: Rational,
    frame_size: usize,
    variable_frame_size: bool,
}

fn create_audio_output_context<'a>(
    output: &mut format::context::Output,
    output_path: &Path,
    track: &'a AudioTrack,
) -> Result<AudioOutputContext<'a>> {
    let audio_codec = ffmpeg::encoder::find(
        output
            .format()
            .codec(output_path, ffmpeg::media::Type::Audio),
    )
    .or_else(|| ffmpeg::encoder::find(codec::Id::AAC))
    .ok_or_else(|| anyhow!("AAC encoder not found in local ffmpeg"))?
    .audio()?;

    let global_header = output
        .format()
        .flags()
        .contains(format::flag::Flags::GLOBAL_HEADER);

    let mut stream = output.add_stream(audio_codec)?;
    let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
    let mut audio_encoder = context.encoder().audio()?;

    let encoder_format = audio_codec
        .formats()
        .and_then(|mut formats| formats.next())
        .unwrap_or(Sample::F32(SampleType::Planar));
    let channel_layout = audio_codec
        .channel_layouts()
        .map(|layouts| layouts.best(track.channels as i32))
        .unwrap_or(ChannelLayout::STEREO);

    if global_header {
        audio_encoder.set_flags(codec::flag::Flags::GLOBAL_HEADER);
    }

    audio_encoder.set_rate(track.sample_rate as i32);
    audio_encoder.set_channel_layout(channel_layout);
    audio_encoder.set_format(encoder_format);
    audio_encoder.set_bit_rate(192_000);
    audio_encoder.set_time_base((1, track.sample_rate as i32));

    let encoder = audio_encoder.open_as(audio_codec)?;
    stream.set_time_base((1, track.sample_rate as i32));
    stream.set_parameters(&encoder);

    let variable_frame_size = encoder.codec().is_some_and(|codec| {
        codec
            .capabilities()
            .contains(ffmpeg::codec::capabilities::Capabilities::VARIABLE_FRAME_SIZE)
    });
    let frame_size = encoder.frame_size().max(1024) as usize;
    let resampler = ResamplingContext::get(
        Sample::F32(SampleType::Packed),
        ChannelLayout::STEREO,
        track.sample_rate,
        encoder.format(),
        encoder.channel_layout(),
        encoder.rate(),
    )?;

    Ok(AudioOutputContext {
        track,
        encoder,
        resampler,
        stream_index: stream.index(),
        packet_time_base: Rational(1, track.sample_rate as i32),
        stream_time_base: stream.time_base(),
        frame_size,
        variable_frame_size,
    })
}

fn write_audio_track(
    mut audio: AudioOutputContext<'_>,
    output: &mut format::context::Output,
) -> Result<()> {
    let total_frames = audio.track.sample_frames();
    let chunk_size = if audio.variable_frame_size {
        audio.frame_size.max(1024)
    } else {
        audio.frame_size.max(1)
    };
    let mut next_pts = 0_i64;
    let mut sample_cursor = 0_usize;

    while sample_cursor < total_frames {
        let remaining = total_frames - sample_cursor;
        let chunk_frames = remaining.min(chunk_size);
        let padded_frames = if audio.variable_frame_size {
            chunk_frames
        } else {
            chunk_size
        };

        let mut input = AudioFrame::new(
            Sample::F32(SampleType::Packed),
            padded_frames,
            ChannelLayout::STEREO,
        );
        input.set_rate(audio.track.sample_rate);
        input.set_pts(Some(next_pts));

        {
            let plane = input.plane_mut::<(f32, f32)>(0);
            for sample in plane.iter_mut() {
                *sample = (0.0, 0.0);
            }
            for (idx, sample) in plane.iter_mut().take(chunk_frames).enumerate() {
                let src = (sample_cursor + idx) * 2;
                *sample = (audio.track.samples[src], audio.track.samples[src + 1]);
            }
        }

        let mut converted = AudioFrame::empty();
        audio.resampler.run(&input, &mut converted)?;
        converted.set_pts(Some(next_pts));
        audio.encoder.send_frame(&converted)?;
        write_audio_packets(
            &mut audio.encoder,
            output,
            audio.stream_index,
            audio.packet_time_base,
            audio.stream_time_base,
        )?;

        sample_cursor += chunk_frames;
        next_pts += chunk_frames as i64;
    }

    audio.encoder.send_eof()?;
    write_audio_packets(
        &mut audio.encoder,
        output,
        audio.stream_index,
        audio.packet_time_base,
        audio.stream_time_base,
    )?;
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

fn write_video_packets(
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

fn write_audio_packets(
    encoder: &mut ffmpeg::codec::encoder::audio::Encoder,
    output: &mut format::context::Output,
    stream_index: usize,
    packet_time_base: Rational,
    stream_time_base: Rational,
) -> Result<()> {
    let mut packet = Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        packet.rescale_ts(packet_time_base, stream_time_base);
        packet.set_stream(stream_index);
        packet.write_interleaved(output)?;
    }

    Ok(())
}
