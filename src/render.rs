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
use taffy::{
    AvailableSpace, TaffyTree,
    prelude::{Dimension, JustifyContent as TaffyJustifyContent, LengthPercentage, Style},
};

use crate::{
    Composition, FrameCtx,
    nodes::{AbsoluteFill, AlignItems, JustifyContent, Text},
    style::{ColorToken, ComputedTextStyle, FlexDirection},
};

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
    draw_with_taffy(&node, &frame_ctx, canvas)?;

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

#[derive(Clone)]
struct LayoutPayload {
    draw_kind: DrawKind,
}

#[derive(Clone)]
enum DrawKind {
    AbsoluteFill { background: ColorToken },
    Text {
        text: String,
        color: ColorToken,
        font_size: f32,
    },
}

fn draw_with_taffy(node: &crate::Node, frame_ctx: &FrameCtx, canvas: &skia_safe::Canvas) -> Result<()> {
    let mut taffy: TaffyTree<LayoutPayload> = TaffyTree::new();
    let root = build_taffy_subtree(&mut taffy, node, frame_ctx, &ComputedTextStyle::default())?;

    taffy.compute_layout(
        root,
        taffy::geometry::Size {
            width: AvailableSpace::Definite(frame_ctx.width as f32),
            height: AvailableSpace::Definite(frame_ctx.height as f32),
        },
    )?;

    paint_subtree(&taffy, root, canvas)?;
    Ok(())
}

fn build_taffy_subtree(
    taffy: &mut TaffyTree<LayoutPayload>,
    node: &crate::Node,
    frame_ctx: &FrameCtx,
    inherited_style: &ComputedTextStyle,
) -> Result<taffy::NodeId> {
    if let Some(fill) = node.as_any().downcast_ref::<AbsoluteFill>() {
        let next_style = fill.resolve_text_style(inherited_style);
        let mut children = Vec::new();
        for child in fill.children_ref() {
            children.push(build_taffy_subtree(taffy, child, frame_ctx, &next_style)?);
        }

        let node_style = fill.style_ref();
        let style = Style {
            display: taffy::prelude::Display::Flex,
            flex_direction: map_flex_direction(node_style.flex_direction),
            justify_content: node_style.justify_content.map(map_justify),
            align_items: node_style.align_items.map(map_align),
            gap: taffy::geometry::Size {
                width: taffy::style::LengthPercentage::Length(node_style.gap.unwrap_or(0.0)),
                height: taffy::style::LengthPercentage::Length(node_style.gap.unwrap_or(0.0)),
            },
            size: taffy::geometry::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Percent(1.0),
            },
            ..Default::default()
        };

        let id = taffy.new_with_children(style, &children)?;
        taffy.set_node_context(
            id,
            Some(LayoutPayload {
                draw_kind: DrawKind::AbsoluteFill {
                    background: fill.background_color_value(),
                },
            }),
        )?;
        return Ok(id);
    }

    if let Some(text) = node.as_any().downcast_ref::<Text>() {
        let size = text.measured_size(inherited_style);
        let node_style = text.style_ref();
        let style = Style {
            flex_grow: node_style.flex_grow.unwrap_or(0.0),
            size: taffy::geometry::Size {
                width: Dimension::Length(size.0),
                height: Dimension::Length(size.1),
            },
            ..Default::default()
        };
        let id = taffy.new_leaf(style)?;
        taffy.set_node_context(
            id,
            Some(LayoutPayload {
                draw_kind: DrawKind::Text {
                    text: text.content().to_string(),
                    color: text.resolved_color(inherited_style),
                    font_size: text.resolved_font_size(inherited_style),
                },
            }),
        )?;
        return Ok(id);
    }

    Err(anyhow!("unknown node type encountered while building layout tree"))
}

fn map_flex_direction(value: Option<crate::style::FlexDirection>) -> taffy::prelude::FlexDirection {
    match value {
        None | Some(crate::style::FlexDirection::Row) => taffy::prelude::FlexDirection::Row,
        Some(crate::style::FlexDirection::Col) => taffy::prelude::FlexDirection::Column,
    }
}

fn paint_subtree(
    taffy: &TaffyTree<LayoutPayload>,
    node_id: taffy::NodeId,
    canvas: &skia_safe::Canvas,
) -> Result<()> {
    let layout = taffy.layout(node_id)?;
    let rect = Rect::from_xywh(layout.location.x, layout.location.y, layout.size.width, layout.size.height);

    if let Some(payload) = taffy.get_node_context(node_id) {
        match payload.draw_kind.clone() {
            DrawKind::AbsoluteFill { background } => {
                let mut paint = skia_safe::Paint::default();
                paint.set_color(background.to_skia());
                paint.set_anti_alias(true);
                canvas.draw_rect(rect, &paint);
            }
            DrawKind::Text {
                text,
                color,
                font_size,
            } => {
                Text::draw_resolved(canvas, rect.left, rect.top, text, color, font_size);
            }
        }
    }

    let children = taffy.children(node_id)?;
    for child in children {
        paint_subtree(taffy, child, canvas)?;
    }

    Ok(())
}

fn map_justify(value: JustifyContent) -> TaffyJustifyContent {
    match value {
        JustifyContent::Start => TaffyJustifyContent::FlexStart,
        JustifyContent::Center => TaffyJustifyContent::Center,
        JustifyContent::End => TaffyJustifyContent::FlexEnd,
        JustifyContent::Between => TaffyJustifyContent::SpaceBetween,
        JustifyContent::Around => TaffyJustifyContent::SpaceAround,
        JustifyContent::Evenly => TaffyJustifyContent::SpaceEvenly,
    }
}

fn map_align(value: AlignItems) -> taffy::prelude::AlignItems {
    match value {
        AlignItems::Start => taffy::prelude::AlignItems::FlexStart,
        AlignItems::Center => taffy::prelude::AlignItems::Center,
        AlignItems::End => taffy::prelude::AlignItems::FlexEnd,
        AlignItems::Stretch => taffy::prelude::AlignItems::Stretch,
    }
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
