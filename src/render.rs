use std::path::Path;
use std::sync::Arc;

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
use skia_safe::{AlphaType, Canvas, ColorType, ImageInfo, Rect, image::CachingHint, surfaces};
use taffy::{
    AvailableSpace, TaffyTree,
    prelude::{Dimension, JustifyContent as TaffyJustifyContent, Style},
};

use crate::{
    Composition, FrameCtx,
    nodes::{Div, AlignItems, JustifyContent, Position, Text},
    style::ComputedTextStyle,
    view::ComponentNode,
};

/// Trait for drawing a node at a given position.
/// This allows paint_subtree to delegate drawing to the node itself.
trait Drawer: Send + Sync {
    fn draw(&self, canvas: &Canvas, bounds: Rect);
}

impl Drawer for Text {
    fn draw(&self, canvas: &Canvas, bounds: Rect) {
        self.draw_at(canvas, bounds.left, bounds.top, &ComputedTextStyle::default());
    }
}

/// A Drawer that holds resolved text style for proper text rendering.
struct TextDrawer {
    text: Arc<Text>,
    computed_style: ComputedTextStyle,
}

impl Drawer for TextDrawer {
    fn draw(&self, canvas: &Canvas, bounds: Rect) {
        self.text.draw_at(canvas, bounds.left, bounds.top, &self.computed_style);
    }
}

struct DivDrawer {
    div: Div,
}

impl Drawer for DivDrawer {
    fn draw(&self, canvas: &Canvas, bounds: Rect) {
        let mut paint = skia_safe::Paint::default();
        paint.set_anti_alias(true);

        if let Some(color) = self.div.style.bg_color {
            paint.set_color(color.to_skia());
            canvas.draw_rect(bounds, &paint);
        }
    }
}

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

struct LayoutPayload {
    drawer: Arc<dyn Drawer>,
    opacity: f32,
}

// Manual Clone implementation since Arc<dyn Drawer> doesn't auto-derive Clone
impl Clone for LayoutPayload {
    fn clone(&self) -> Self {
        Self {
            drawer: Arc::clone(&self.drawer),
            opacity: self.opacity,
        }
    }
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
    if let Some(component) = node.as_any().downcast_ref::<ComponentNode>() {
        let resolved = component.render(frame_ctx);
        return build_taffy_subtree(taffy, &resolved, frame_ctx, inherited_style);
    }

    if let Some(div) = node.as_any().downcast_ref::<Div>() {
        let next_style = div.resolve_text_style(inherited_style);
        let mut children = Vec::new();
        for child in div.children_ref() {
            children.push(build_taffy_subtree(taffy, child, frame_ctx, &next_style)?);
        }

        let node_style = div.style_ref();

        // Determine position mode
        let position = node_style.position.unwrap_or(Position::Relative);

        // Build size based on position
        let size = if position == Position::Absolute {
            // For absolute positioned elements, use explicit size or auto
            taffy::geometry::Size {
                width: node_style.width.map(|w| Dimension::Length(w)).unwrap_or(Dimension::Auto),
                height: node_style.height.map(|h| Dimension::Length(h)).unwrap_or(Dimension::Auto),
            }
        } else {
            // For relative (flex container), default to 100% if no explicit size
            taffy::geometry::Size {
                width: node_style.width.map(|w| Dimension::Length(w)).unwrap_or(Dimension::Percent(1.0)),
                height: node_style.height.map(|h| Dimension::Length(h)).unwrap_or(Dimension::Percent(1.0)),
            }
        };

        // Build inset for absolute positioning
        let inset = taffy::geometry::Rect {
            left: node_style
                .inset_left
                .map(taffy::style::LengthPercentageAuto::Length)
                .unwrap_or(taffy::style::LengthPercentageAuto::Auto),
            top: node_style
                .inset_top
                .map(taffy::style::LengthPercentageAuto::Length)
                .unwrap_or(taffy::style::LengthPercentageAuto::Auto),
            right: node_style
                .inset_right
                .map(taffy::style::LengthPercentageAuto::Length)
                .unwrap_or(taffy::style::LengthPercentageAuto::Auto),
            bottom: node_style
                .inset_bottom
                .map(taffy::style::LengthPercentageAuto::Length)
                .unwrap_or(taffy::style::LengthPercentageAuto::Auto),
        };

        // Build padding
        let padding = taffy::geometry::Rect {
            left: node_style
                .padding_x
                .or(node_style.padding)
                .map(taffy::style::LengthPercentage::Length)
                .unwrap_or(taffy::style::LengthPercentage::Length(0.0)),
            top: node_style
                .padding_y
                .or(node_style.padding)
                .map(taffy::style::LengthPercentage::Length)
                .unwrap_or(taffy::style::LengthPercentage::Length(0.0)),
            right: node_style
                .padding_x
                .or(node_style.padding)
                .map(taffy::style::LengthPercentage::Length)
                .unwrap_or(taffy::style::LengthPercentage::Length(0.0)),
            bottom: node_style
                .padding_y
                .or(node_style.padding)
                .map(taffy::style::LengthPercentage::Length)
                .unwrap_or(taffy::style::LengthPercentage::Length(0.0)),
        };

        // Build margin
        let margin = taffy::geometry::Rect {
            left: node_style
                .margin_x
                .or(node_style.margin)
                .map(taffy::style::LengthPercentageAuto::Length)
                .unwrap_or(taffy::style::LengthPercentageAuto::Length(0.0)),
            top: node_style
                .margin_y
                .or(node_style.margin)
                .map(taffy::style::LengthPercentageAuto::Length)
                .unwrap_or(taffy::style::LengthPercentageAuto::Length(0.0)),
            right: node_style
                .margin_x
                .or(node_style.margin)
                .map(taffy::style::LengthPercentageAuto::Length)
                .unwrap_or(taffy::style::LengthPercentageAuto::Length(0.0)),
            bottom: node_style
                .margin_y
                .or(node_style.margin)
                .map(taffy::style::LengthPercentageAuto::Length)
                .unwrap_or(taffy::style::LengthPercentageAuto::Length(0.0)),
        };

        let style = Style {
            display: taffy::prelude::Display::Flex,
            position: map_position(position),
            inset,
            size,
            padding,
            margin,
            flex_direction: map_flex_direction(node_style.flex_direction),
            justify_content: node_style.justify_content.map(map_justify),
            align_items: node_style.align_items.map(map_align),
            gap: taffy::geometry::Size {
                width: taffy::style::LengthPercentage::Length(node_style.gap.unwrap_or(0.0)),
                height: taffy::style::LengthPercentage::Length(node_style.gap.unwrap_or(0.0)),
            },
            flex_grow: node_style.flex_grow.unwrap_or(0.0),
            ..Default::default()
        };

        let id = taffy.new_with_children(style, &children)?;
        taffy.set_node_context(
            id,
            Some(LayoutPayload {
                drawer: Arc::new(DivDrawer { div: div.clone() }),
                opacity: node_style.opacity.unwrap_or(1.0),
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
                drawer: Arc::new(TextDrawer {
                    text: Arc::new(text.clone()),
                    computed_style: text.resolve_text_style(inherited_style),
                }),
                opacity: node_style.opacity.unwrap_or(1.0),
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

fn map_position(value: Position) -> taffy::style::Position {
    match value {
        Position::Relative => taffy::style::Position::Relative,
        Position::Absolute => taffy::style::Position::Absolute,
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
        if payload.opacity <= 0.0 {
            return Ok(());
        }

        let uses_layer = payload.opacity < 1.0;
        if uses_layer {
            let alpha = (payload.opacity * 255.0).round() as u32;
            canvas.save_layer_alpha(rect, alpha);
        }

        payload.drawer.draw(canvas, rect);

        let children = taffy.children(node_id)?;
        for child in children {
            paint_subtree(taffy, child, canvas)?;
        }

        if uses_layer {
            canvas.restore();
        }

        return Ok(());
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
