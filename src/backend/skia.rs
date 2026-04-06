use std::{cell::RefCell, collections::HashMap, time::Instant};

use anyhow::{Context, Result, anyhow};
use skia_safe::{
    Canvas, ClipOp, Data, Image as SkiaImage, ImageInfo, Paint, PaintStyle, Picture,
    PictureRecorder, RRect, Rect, TileMode, canvas::SrcRectConstraint, gradient_shader, images,
};

use crate::{
    assets::AssetsMap,
    backend::cache::{ImageCache, SubtreePictureCache, TextPictureCache},
    cache_policy::{
        BitmapSourceKind, bitmap_source_kind, subtree_picture_cache_key, text_picture_cache_key,
    },
    display::list::{
        BitmapDisplayItem, BitmapPaintStyle, DisplayCommand, DisplayItem, DisplayList,
        DisplayTransform, LucideDisplayItem, RectDisplayItem, TextDisplayItem,
    },
    frame_ctx::FrameCtx,
    layout::tree::{LayoutNode, LayoutPaintKind, LayoutRect, LayoutTree},
    media::MediaContext,
    profile::BackendProfile,
    style::{BackgroundFill, GradientDirection, ObjectFit, ShadowStyle, Transform},
    typography,
};

struct BitmapDrawStats {
    draw_ms: f64,
    image_decode_ms: f64,
    video_decode_ms: f64,
    image_cache_hits: usize,
    image_cache_misses: usize,
    video_frame_decodes: usize,
}

struct TextDrawStats {
    picture_record_ms: f64,
    picture_draw_ms: f64,
    cache_hits: usize,
    cache_misses: usize,
}

pub struct SkiaBackend<'a> {
    canvas: &'a Canvas,
    assets: &'a AssetsMap,
    media_ctx: Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
    image_cache: ImageCache,
    text_picture_cache: TextPictureCache,
    subtree_picture_cache: Option<SubtreePictureCache>,
    profile: Option<&'a mut BackendProfile>,
}

impl<'a> SkiaBackend<'a> {
    pub fn new_with_cache_and_profile(
        canvas: &'a Canvas,
        _width: i32,
        _height: i32,
        assets: &'a AssetsMap,
        image_cache: ImageCache,
        text_picture_cache: TextPictureCache,
        subtree_picture_cache: Option<SubtreePictureCache>,
        media_ctx: Option<&'a mut MediaContext>,
        frame_ctx: &'a FrameCtx,
        profile: Option<&'a mut BackendProfile>,
    ) -> Self {
        Self {
            canvas,
            assets,
            image_cache,
            text_picture_cache,
            subtree_picture_cache,
            media_ctx,
            frame_ctx,
            profile,
        }
    }

    pub fn execute(&mut self, list: &DisplayList) -> Result<()> {
        for command in &list.commands {
            self.execute_command(command)?;
        }
        Ok(())
    }

    fn draw_layout_children(&mut self, children: &[LayoutNode]) -> Result<()> {
        let mut sorted = children.iter().collect::<Vec<_>>();
        sorted.sort_by_key(|child| child.paint.z_index);
        for child in sorted {
            self.draw_layout_subtree(child)?;
        }
        Ok(())
    }

    fn draw_layout_subtree(&mut self, layout: &LayoutNode) -> Result<()> {
        if layout.paint.visual.opacity <= 0.0 {
            return Ok(());
        }

        self.canvas.save();
        apply_transform(
            self.canvas,
            &DisplayTransform {
                translation_x: layout.rect.x,
                translation_y: layout.rect.y,
                bounds: LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: layout.rect.width,
                    height: layout.rect.height,
                },
                transforms: layout.paint.visual.transforms.clone(),
            },
        );

        let subtree_cache = self.subtree_picture_cache.clone();
        if let Some(cache) = subtree_cache {
            if let Some(key) = subtree_picture_cache_key(layout, self.assets) {
                if let Some(picture) = cache.borrow().get(&key).cloned() {
                    if let Some(profile) = self.profile.as_deref_mut() {
                        profile.subtree_picture_cache_hits += 1;
                    }
                    self.draw_subtree_picture(layout, &picture)?;
                    self.canvas.restore();
                    return Ok(());
                }

                let picture = self.record_cached_subtree_picture(layout)?;
                cache.borrow_mut().insert(key, picture.clone());
                if let Some(profile) = self.profile.as_deref_mut() {
                    profile.subtree_picture_cache_misses += 1;
                }
                self.draw_subtree_picture(layout, &picture)?;
                self.canvas.restore();
                return Ok(());
            }
        }

        self.draw_layout_subtree_contents(layout)?;
        self.canvas.restore();
        Ok(())
    }

    fn draw_layout_subtree_contents(&mut self, layout: &LayoutNode) -> Result<()> {
        let local_bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: layout.rect.width,
            height: layout.rect.height,
        };
        self.with_layout_opacity(layout, |backend| {
            backend.draw_layout_node_paint(layout, local_bounds)?;
            if layout.paint.visual.clip_contents {
                backend.canvas.save();
                clip_bounds(
                    backend.canvas,
                    local_bounds,
                    layout.paint.visual.border_radius,
                );
                backend.draw_layout_children(&layout.children)?;
                backend.canvas.restore();
                Ok(())
            } else {
                backend.draw_layout_children(&layout.children)
            }
        })
    }

    fn draw_layout_node_paint(&mut self, layout: &LayoutNode, bounds: LayoutRect) -> Result<()> {
        match &layout.paint.kind {
            LayoutPaintKind::Div => {
                let started = Instant::now();
                draw_rect(
                    self.canvas,
                    &RectDisplayItem {
                        bounds,
                        paint: crate::display::list::RectPaintStyle {
                            background: layout.paint.visual.background,
                            border_radius: layout.paint.visual.border_radius,
                            border_width: layout.paint.visual.border_width,
                            border_color: layout.paint.visual.border_color,
                            shadow: layout.paint.visual.shadow,
                        },
                    },
                );
                if let Some(profile) = self.profile.as_deref_mut() {
                    profile.draw_rect_count += 1;
                    profile.rect_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
                }
            }
            LayoutPaintKind::Text(text) => {
                let started = Instant::now();
                let stats = draw_text(
                    self.canvas,
                    &TextDisplayItem {
                        bounds,
                        text: text.text.clone(),
                        style: text.style,
                        allow_wrap: text.allow_wrap,
                    },
                    &self.text_picture_cache,
                )?;
                if let Some(profile) = self.profile.as_deref_mut() {
                    profile.draw_text_count += 1;
                    profile.text_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
                    profile.text_picture_record_ms += stats.picture_record_ms;
                    profile.text_picture_draw_ms += stats.picture_draw_ms;
                    profile.text_cache_hits += stats.cache_hits;
                    profile.text_cache_misses += stats.cache_misses;
                }
            }
            LayoutPaintKind::Bitmap(bitmap) => {
                let stats = draw_bitmap(
                    self.canvas,
                    &BitmapDisplayItem {
                        bounds,
                        asset_id: bitmap.asset_id.clone(),
                        width: bitmap.width,
                        height: bitmap.height,
                        object_fit: bitmap.object_fit,
                        paint: BitmapPaintStyle {
                            background: layout.paint.visual.background,
                            border_radius: layout.paint.visual.border_radius,
                            border_width: layout.paint.visual.border_width,
                            border_color: layout.paint.visual.border_color,
                            shadow: layout.paint.visual.shadow,
                        },
                    },
                    self.assets,
                    &self.image_cache,
                    &mut self.media_ctx,
                    self.frame_ctx,
                )?;
                if let Some(profile) = self.profile.as_deref_mut() {
                    profile.draw_bitmap_count += 1;
                    profile.bitmap_draw_ms += stats.draw_ms;
                    profile.image_decode_ms += stats.image_decode_ms;
                    profile.video_decode_ms += stats.video_decode_ms;
                    profile.image_cache_hits += stats.image_cache_hits;
                    profile.image_cache_misses += stats.image_cache_misses;
                    profile.video_frame_decodes += stats.video_frame_decodes;
                }
            }
            LayoutPaintKind::Lucide(lucide) => {
                draw_lucide(
                    self.canvas,
                    &LucideDisplayItem {
                        bounds,
                        icon: lucide.icon.clone(),
                        paint: crate::display::list::LucidePaintStyle {
                            foreground: lucide.foreground,
                            background: layout.paint.visual.background,
                            border_width: layout.paint.visual.border_width,
                            border_color: layout.paint.visual.border_color,
                        },
                    },
                );
            }
        }
        Ok(())
    }

    fn record_cached_subtree_picture(&mut self, layout: &LayoutNode) -> Result<Picture> {
        let started = Instant::now();
        let bounds = Rect::from_xywh(
            0.0,
            0.0,
            layout.rect.width.max(1.0),
            layout.rect.height.max(1.0),
        );
        let mut recorder = PictureRecorder::new();
        let recording_canvas = recorder.begin_recording(bounds, false);
        let mut backend = SkiaBackend::new_with_cache_and_profile(
            recording_canvas,
            layout.rect.width as i32,
            layout.rect.height as i32,
            self.assets,
            self.image_cache.clone(),
            self.text_picture_cache.clone(),
            self.subtree_picture_cache.clone(),
            None,
            self.frame_ctx,
            self.profile.as_deref_mut(),
        );
        let local_bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: layout.rect.width,
            height: layout.rect.height,
        };
        backend.draw_layout_node_paint(layout, local_bounds)?;
        backend.draw_layout_children(&layout.children)?;
        let picture = recorder
            .finish_recording_as_picture(None)
            .ok_or_else(|| anyhow!("failed to record subtree picture"))?;
        if let Some(profile) = self.profile.as_deref_mut() {
            profile.picture_record_ms += started.elapsed().as_secs_f64() * 1000.0;
        }
        Ok(picture)
    }

    fn draw_subtree_picture(&mut self, layout: &LayoutNode, picture: &Picture) -> Result<()> {
        self.with_layout_opacity(layout, |backend| {
            let started = Instant::now();
            backend.canvas.draw_picture(picture, None, None);
            if let Some(profile) = backend.profile.as_deref_mut() {
                profile.picture_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
            }
            Ok(())
        })
    }

    fn with_layout_opacity<T>(
        &mut self,
        layout: &LayoutNode,
        draw: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let local_bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: layout.rect.width,
            height: layout.rect.height,
        };
        let uses_layer = layout.paint.visual.opacity < 1.0;
        if uses_layer {
            if let Some(profile) = self.profile.as_deref_mut() {
                profile.save_layer_count += 1;
            }
            let alpha = (layout.paint.visual.opacity * 255.0).round() as u32;
            self.canvas
                .save_layer_alpha(layout_rect_to_skia(local_bounds), alpha);
        }

        let result = draw(self);

        if uses_layer {
            self.canvas.restore();
        }

        result
    }

    fn execute_command(&mut self, command: &DisplayCommand) -> Result<()> {
        match command {
            DisplayCommand::Save => {
                self.canvas.save();
            }
            DisplayCommand::Restore => {
                self.canvas.restore();
            }
            DisplayCommand::SaveLayer { layer } => {
                if let Some(profile) = self.profile.as_deref_mut() {
                    profile.save_layer_count += 1;
                }
                let alpha = (layer.opacity * 255.0).round() as u32;
                self.canvas
                    .save_layer_alpha(layout_rect_to_skia(layer.bounds), alpha);
            }
            DisplayCommand::ApplyTransform { transform } => {
                apply_transform(self.canvas, transform);
            }
            DisplayCommand::Draw { item } => match item {
                DisplayItem::Rect(rect) => {
                    let started = Instant::now();
                    draw_rect(self.canvas, rect);
                    if let Some(profile) = self.profile.as_deref_mut() {
                        profile.draw_rect_count += 1;
                        profile.rect_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
                    }
                }
                DisplayItem::Text(text) => {
                    let started = Instant::now();
                    let stats = draw_text(self.canvas, text, &self.text_picture_cache)?;
                    if let Some(profile) = self.profile.as_deref_mut() {
                        profile.draw_text_count += 1;
                        profile.text_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
                        profile.text_picture_record_ms += stats.picture_record_ms;
                        profile.text_picture_draw_ms += stats.picture_draw_ms;
                        profile.text_cache_hits += stats.cache_hits;
                        profile.text_cache_misses += stats.cache_misses;
                    }
                }
                DisplayItem::Bitmap(bitmap) => {
                    let stats = draw_bitmap(
                        self.canvas,
                        bitmap,
                        self.assets,
                        &self.image_cache,
                        &mut self.media_ctx,
                        self.frame_ctx,
                    )?;
                    if let Some(profile) = self.profile.as_deref_mut() {
                        profile.draw_bitmap_count += 1;
                        profile.bitmap_draw_ms += stats.draw_ms;
                        profile.image_decode_ms += stats.image_decode_ms;
                        profile.video_decode_ms += stats.video_decode_ms;
                        profile.image_cache_hits += stats.image_cache_hits;
                        profile.image_cache_misses += stats.image_cache_misses;
                        profile.video_frame_decodes += stats.video_frame_decodes;
                    }
                }
                DisplayItem::Lucide(lucide) => {
                    draw_lucide(self.canvas, lucide);
                }
            },
        }
        Ok(())
    }
}

pub(crate) fn record_display_list_picture<'a>(
    list: &DisplayList,
    width: i32,
    height: i32,
    assets: &'a AssetsMap,
    image_cache: ImageCache,
    text_picture_cache: TextPictureCache,
    media_ctx: Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
    mut profile: Option<&'a mut BackendProfile>,
) -> Result<Picture> {
    let started = Instant::now();
    let bounds = Rect::from_xywh(0.0, 0.0, width as f32, height as f32);
    let mut recorder = PictureRecorder::new();
    let recording_canvas = recorder.begin_recording(bounds, false);
    let mut backend = SkiaBackend::new_with_cache_and_profile(
        recording_canvas,
        width,
        height,
        assets,
        image_cache,
        text_picture_cache,
        None,
        media_ctx,
        frame_ctx,
        profile.as_deref_mut(),
    );
    backend.execute(list)?;
    let picture = recorder
        .finish_recording_as_picture(None)
        .ok_or_else(|| anyhow!("failed to record display list picture"))?;
    if let Some(profile) = profile {
        profile.picture_record_ms += started.elapsed().as_secs_f64() * 1000.0;
    }
    Ok(picture)
}

pub(crate) fn draw_layout_tree_with_subtree_cache<'a>(
    layout_tree: &LayoutTree,
    canvas: &'a Canvas,
    assets: &'a AssetsMap,
    image_cache: ImageCache,
    text_picture_cache: TextPictureCache,
    subtree_picture_cache: SubtreePictureCache,
    media_ctx: Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
    profile: Option<&'a mut BackendProfile>,
) -> Result<()> {
    let mut backend = SkiaBackend::new_with_cache_and_profile(
        canvas,
        layout_tree.root.rect.width as i32,
        layout_tree.root.rect.height as i32,
        assets,
        image_cache,
        text_picture_cache,
        Some(subtree_picture_cache),
        media_ctx,
        frame_ctx,
        profile,
    );
    backend.draw_layout_subtree(&layout_tree.root)
}

pub(crate) fn record_layout_tree_picture_with_subtree_cache<'a>(
    layout_tree: &LayoutTree,
    width: i32,
    height: i32,
    assets: &'a AssetsMap,
    image_cache: ImageCache,
    text_picture_cache: TextPictureCache,
    subtree_picture_cache: SubtreePictureCache,
    media_ctx: Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
    mut profile: Option<&'a mut BackendProfile>,
) -> Result<Picture> {
    let started = Instant::now();
    let bounds = Rect::from_xywh(0.0, 0.0, width as f32, height as f32);
    let mut recorder = PictureRecorder::new();
    let recording_canvas = recorder.begin_recording(bounds, false);
    let mut backend = SkiaBackend::new_with_cache_and_profile(
        recording_canvas,
        width,
        height,
        assets,
        image_cache,
        text_picture_cache,
        Some(subtree_picture_cache),
        media_ctx,
        frame_ctx,
        profile.as_deref_mut(),
    );
    backend.draw_layout_subtree(&layout_tree.root)?;
    let picture = recorder
        .finish_recording_as_picture(None)
        .ok_or_else(|| anyhow!("failed to record layout tree picture"))?;
    if let Some(profile) = profile {
        profile.picture_record_ms += started.elapsed().as_secs_f64() * 1000.0;
    }
    Ok(picture)
}

fn draw_rect(canvas: &Canvas, rect: &RectDisplayItem) {
    let style = &rect.paint;
    if style.background.is_none() && style.border_width.is_none() && style.shadow.is_none() {
        return;
    }

    let rect = layout_rect_to_skia(rect.bounds);
    let radius = effective_corner_radius(rect, style.border_radius);

    if let Some(shadow) = style.shadow {
        draw_shadow(canvas, rect, radius, shadow);
    }

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    if radius > 0.0 {
        let rrect = RRect::new_rect_xy(rect, radius, radius);

        if let Some(background) = style.background {
            apply_background_paint(&mut paint, background, rect);
            canvas.draw_rrect(rrect, &paint);
        }

        if let (Some(width), Some(color)) = (style.border_width, style.border_color) {
            paint.set_color(color.to_skia());
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(width);
            canvas.draw_rrect(rrect, &paint);
        }
    } else {
        if let Some(background) = style.background {
            apply_background_paint(&mut paint, background, rect);
            canvas.draw_rect(rect, &paint);
        }

        if let (Some(width), Some(color)) = (style.border_width, style.border_color) {
            paint.set_color(color.to_skia());
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(width);
            canvas.draw_rect(rect, &paint);
        }
    }
}

fn draw_shadow(canvas: &Canvas, rect: Rect, radius: f32, shadow: ShadowStyle) {
    let (blur, offset_y) = match shadow {
        ShadowStyle::SM => (2.0, 1.0),
        ShadowStyle::MD => (4.0, 3.0),
        ShadowStyle::LG => (10.0, 6.0),
        ShadowStyle::XL => (20.0, 10.0),
    };

    let mut paint = Paint::default();
    paint.set_color(skia_safe::Color::from_argb(30, 0, 0, 0));
    paint.set_anti_alias(true);

    let shadow_rect = Rect::from_xywh(
        rect.left() - blur / 2.0,
        rect.top() + offset_y - blur / 2.0,
        rect.width() + blur,
        rect.height() + blur,
    );

    let radius = effective_corner_radius(shadow_rect, radius + blur / 2.0);
    if radius > 0.0 {
        let rrect = RRect::new_rect_xy(shadow_rect, radius, radius);
        canvas.draw_rrect(rrect, &paint);
    } else {
        canvas.draw_rect(shadow_rect, &paint);
    }
}

fn draw_text(
    canvas: &Canvas,
    text: &TextDisplayItem,
    text_picture_cache: &RefCell<HashMap<u64, Picture>>,
) -> Result<TextDrawStats> {
    let cache_key = text_picture_cache_key(text);
    if let Some(picture) = text_picture_cache.borrow().get(&cache_key).cloned() {
        let draw_started = Instant::now();
        canvas.save();
        canvas.translate((text.bounds.x, text.bounds.y));
        canvas.draw_picture(&picture, None, None);
        canvas.restore();
        Ok(TextDrawStats {
            picture_record_ms: 0.0,
            picture_draw_ms: draw_started.elapsed().as_secs_f64() * 1000.0,
            cache_hits: 1,
            cache_misses: 0,
        })
    } else {
        let record_started = Instant::now();
        let picture = record_text_picture(text)?;
        let picture_record_ms = record_started.elapsed().as_secs_f64() * 1000.0;
        text_picture_cache
            .borrow_mut()
            .insert(cache_key, picture.clone());

        let draw_started = Instant::now();
        canvas.save();
        canvas.translate((text.bounds.x, text.bounds.y));
        canvas.draw_picture(&picture, None, None);
        canvas.restore();
        Ok(TextDrawStats {
            picture_record_ms,
            picture_draw_ms: draw_started.elapsed().as_secs_f64() * 1000.0,
            cache_hits: 0,
            cache_misses: 1,
        })
    }
}

fn record_text_picture(text: &TextDisplayItem) -> Result<Picture> {
    let width = text.bounds.width.max(1.0);
    let height = text.bounds.height.max(1.0);
    let bounds = Rect::from_xywh(0.0, 0.0, width, height);
    let mut recorder = PictureRecorder::new();
    let recording_canvas = recorder.begin_recording(bounds, false);
    typography::draw_text(
        recording_canvas,
        &text.text,
        0.0,
        0.0,
        text.bounds.width,
        text.allow_wrap,
        &text.style,
    );
    recorder
        .finish_recording_as_picture(None)
        .ok_or_else(|| anyhow!("failed to record text picture"))
}

fn draw_bitmap(
    canvas: &Canvas,
    bitmap: &BitmapDisplayItem,
    assets: &AssetsMap,
    image_cache: &RefCell<HashMap<String, Option<SkiaImage>>>,
    media_ctx: &mut Option<&mut MediaContext>,
    frame_ctx: &FrameCtx,
) -> Result<BitmapDrawStats> {
    let path = assets
        .path(&bitmap.asset_id)
        .ok_or_else(|| anyhow!("missing asset path for {}", bitmap.asset_id.0))?;

    let mut stats = BitmapDrawStats {
        draw_ms: 0.0,
        image_decode_ms: 0.0,
        video_decode_ms: 0.0,
        image_cache_hits: 0,
        image_cache_misses: 0,
        video_frame_decodes: 0,
    };

    let image = if bitmap_source_kind(path) == BitmapSourceKind::Video {
        let media = media_ctx
            .as_deref_mut()
            .ok_or_else(|| anyhow!("video asset requires media context: {}", path.display()))?;
        let target_time = frame_ctx.frame as f64 / frame_ctx.fps as f64;
        let decode_started = Instant::now();
        let (data, width, height) = media
            .get_bitmap(path, target_time)
            .with_context(|| format!("failed to decode video frame: {}", path.display()))?;
        stats.video_decode_ms = decode_started.elapsed().as_secs_f64() * 1000.0;
        stats.video_frame_decodes = 1;
        let info = ImageInfo::new(
            (width as i32, height as i32),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        images::raster_from_data(&info, Data::new_copy(&data), width as usize * 4).ok_or_else(
            || {
                anyhow!(
                    "failed to create skia image from video frame: {}",
                    path.display()
                )
            },
        )?
    } else {
        let key = bitmap.asset_id.0.clone();
        let mut cache = image_cache.borrow_mut();
        if let Some(Some(img)) = cache.get(&key) {
            stats.image_cache_hits = 1;
            img.clone()
        } else {
            let decode_started = Instant::now();
            let encoded = std::fs::read(path)
                .with_context(|| format!("failed to read image asset: {}", path.display()))?;
            let data = skia_safe::Data::new_copy(&encoded);
            let image = skia_safe::Image::from_encoded(data)
                .ok_or_else(|| anyhow!("failed to decode image asset: {}", path.display()))?;
            stats.image_decode_ms = decode_started.elapsed().as_secs_f64() * 1000.0;
            stats.image_cache_misses = 1;
            cache.insert(key, Some(image.clone()));
            image
        }
    };

    let draw_started = Instant::now();
    let dst = layout_rect_to_skia(bitmap.bounds);
    let radius = effective_corner_radius(dst, bitmap.paint.border_radius);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    let src_width = bitmap.width as f32;
    let src_height = bitmap.height as f32;

    if let Some(shadow) = bitmap.paint.shadow {
        draw_shadow(canvas, dst, radius, shadow);
    }

    if let Some(color) = bitmap.paint.background {
        let mut background_paint = Paint::default();
        background_paint.set_anti_alias(true);
        apply_background_paint(&mut background_paint, color, dst);
        if radius > 0.0 {
            let rrect = RRect::new_rect_xy(dst, radius, radius);
            canvas.draw_rrect(rrect, &background_paint);
        } else {
            canvas.draw_rect(dst, &background_paint);
        }
    }

    let needs_clip = radius > 0.0;
    if needs_clip {
        let rrect = RRect::new_rect_xy(dst, radius, radius);
        canvas.save();
        canvas.clip_rrect(rrect, ClipOp::Intersect, true);
    }

    match bitmap.object_fit {
        ObjectFit::Fill => {
            canvas.draw_image_rect(image, None, dst, &paint);
        }
        ObjectFit::Contain => {
            let fitted = fitted_rect(src_width, src_height, dst, false);
            canvas.draw_image_rect(image, None, fitted, &paint);
        }
        ObjectFit::Cover => {
            let src = cover_src_rect(src_width, src_height, dst);
            canvas.draw_image_rect(image, Some((&src, SrcRectConstraint::Strict)), dst, &paint);
        }
    }

    if needs_clip {
        canvas.restore();
    }

    if let (Some(width), Some(color)) = (bitmap.paint.border_width, bitmap.paint.border_color) {
        let mut border_paint = Paint::default();
        border_paint.set_anti_alias(true);
        border_paint.set_color(color.to_skia());
        border_paint.set_style(PaintStyle::Stroke);
        border_paint.set_stroke_width(width);

        if radius > 0.0 {
            let rrect = RRect::new_rect_xy(dst, radius, radius);
            canvas.draw_rrect(rrect, &border_paint);
        } else {
            canvas.draw_rect(dst, &border_paint);
        }
    }

    stats.draw_ms = draw_started.elapsed().as_secs_f64() * 1000.0;
    Ok(stats)
}

fn effective_corner_radius(rect: Rect, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }

    radius.min(rect.width() / 2.0).min(rect.height() / 2.0)
}

fn apply_transform(canvas: &Canvas, transform: &DisplayTransform) {
    canvas.translate((transform.translation_x, transform.translation_y));
    if transform.transforms.is_empty() {
        return;
    }

    let rect = layout_rect_to_skia(transform.bounds);
    let center_x = rect.width() / 2.0;
    let center_y = rect.height() / 2.0;

    for transform in transform.transforms.iter().rev() {
        match *transform {
            Transform::TranslateX(x) => {
                canvas.translate((x, 0.0));
            }
            Transform::TranslateY(y) => {
                canvas.translate((0.0, y));
            }
            Transform::Translate(x, y) => {
                canvas.translate((x, y));
            }
            Transform::Scale(value) => {
                canvas.translate((center_x, center_y));
                canvas.scale((value, value));
                canvas.translate((-center_x, -center_y));
            }
            Transform::ScaleX(value) => {
                canvas.translate((center_x, center_y));
                canvas.scale((value, 1.0));
                canvas.translate((-center_x, -center_y));
            }
            Transform::ScaleY(value) => {
                canvas.translate((center_x, center_y));
                canvas.scale((1.0, value));
                canvas.translate((-center_x, -center_y));
            }
            Transform::RotateDeg(deg) => {
                canvas.rotate(deg, Some((center_x, center_y).into()));
            }
            Transform::SkewXDeg(deg) => {
                canvas.translate((center_x, center_y));
                canvas.skew((deg.to_radians().tan(), 0.0));
                canvas.translate((-center_x, -center_y));
            }
            Transform::SkewYDeg(deg) => {
                canvas.translate((center_x, center_y));
                canvas.skew((0.0, deg.to_radians().tan()));
                canvas.translate((-center_x, -center_y));
            }
            Transform::SkewDeg(x_deg, y_deg) => {
                canvas.translate((center_x, center_y));
                canvas.skew((x_deg.to_radians().tan(), y_deg.to_radians().tan()));
                canvas.translate((-center_x, -center_y));
            }
        }
    }
}

fn layout_rect_to_skia(rect: LayoutRect) -> Rect {
    Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
}

fn fitted_rect(src_width: f32, src_height: f32, dst: Rect, cover: bool) -> Rect {
    let src_aspect = src_width / src_height;
    let dst_aspect = dst.width() / dst.height();

    let scale = if cover {
        if src_aspect > dst_aspect {
            dst.height() / src_height
        } else {
            dst.width() / src_width
        }
    } else if src_aspect > dst_aspect {
        dst.width() / src_width
    } else {
        dst.height() / src_height
    };

    let width = src_width * scale;
    let height = src_height * scale;
    let x = dst.left + (dst.width() - width) / 2.0;
    let y = dst.top + (dst.height() - height) / 2.0;

    Rect::from_xywh(x, y, width, height)
}

fn draw_lucide(canvas: &Canvas, item: &LucideDisplayItem) {
    let Some(paths) = crate::lucide_icons::lucide_icon_paths(&item.icon) else {
        return;
    };

    let dst = layout_rect_to_skia(item.bounds);
    let scale = (dst.width() / 24.0).min(dst.height() / 24.0);
    if scale <= 0.0 {
        return;
    }
    let draw_width = 24.0 * scale;
    let draw_height = 24.0 * scale;
    let offset_x = (dst.width() - draw_width) / 2.0;
    let offset_y = (dst.height() - draw_height) / 2.0;

    let fill_paint = item.paint.background.map(|color| {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        apply_background_paint(&mut paint, color, dst);
        paint.set_style(PaintStyle::Fill);
        paint
    });

    let stroke_width = match item.paint.border_width {
        Some(width) if width > 0.0 => Some(width),
        Some(_) => None,
        None => Some(2.0),
    };
    let stroke_color = item.paint.border_color.unwrap_or(item.paint.foreground);

    let stroke_paint = stroke_width.map(|width| {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(stroke_color.to_skia());
        paint.set_style(PaintStyle::Stroke);
        paint.set_stroke_width(width / scale);
        paint.set_stroke_cap(skia_safe::paint::Cap::Round);
        paint.set_stroke_join(skia_safe::paint::Join::Round);
        paint
    });

    canvas.save();
    canvas.translate((dst.left() + offset_x, dst.top() + offset_y));
    canvas.scale((scale, scale));

    for path_data in paths {
        if let Some(path) = skia_safe::Path::from_svg(path_data) {
            if let Some(fill_paint) = fill_paint.as_ref() {
                canvas.draw_path(&path, fill_paint);
            }
            if let Some(stroke_paint) = stroke_paint.as_ref() {
                canvas.draw_path(&path, stroke_paint);
            }
        }
    }

    canvas.restore();
}

fn apply_background_paint(paint: &mut Paint, background: BackgroundFill, bounds: Rect) {
    match background {
        BackgroundFill::Solid(color) => {
            paint.set_shader(None);
            paint.set_color(color.to_skia());
        }
        BackgroundFill::LinearGradient {
            direction,
            from,
            via,
            to,
        } => {
            let points = match direction {
                GradientDirection::ToRight => (
                    (bounds.left(), bounds.center_y()),
                    (bounds.right(), bounds.center_y()),
                ),
                GradientDirection::ToLeft => (
                    (bounds.right(), bounds.center_y()),
                    (bounds.left(), bounds.center_y()),
                ),
                GradientDirection::ToBottomRight => (
                    (bounds.left(), bounds.top()),
                    (bounds.right(), bounds.bottom()),
                ),
            };
            let colors = match via {
                Some(via) => vec![from.to_skia(), via.to_skia(), to.to_skia()],
                None => vec![from.to_skia(), to.to_skia()],
            };
            let positions = via.map(|_| [0.0, 0.5, 1.0]);
            if let Some(shader) = gradient_shader::linear(
                points,
                colors.as_slice(),
                positions.as_ref().map(|positions| positions.as_slice()),
                TileMode::Clamp,
                None,
                None,
            ) {
                paint.set_shader(shader);
            } else {
                paint.set_shader(None);
                paint.set_color(from.to_skia());
            }
        }
    }
}

fn clip_bounds(canvas: &Canvas, bounds: LayoutRect, border_radius: f32) {
    let rect = layout_rect_to_skia(bounds);
    let radius = effective_corner_radius(rect, border_radius);
    if radius > 0.0 {
        let rrect = RRect::new_rect_xy(rect, radius, radius);
        canvas.clip_rrect(rrect, ClipOp::Intersect, true);
    } else {
        canvas.clip_rect(rect, ClipOp::Intersect, true);
    }
}

fn cover_src_rect(src_width: f32, src_height: f32, dst: Rect) -> Rect {
    let fitted = fitted_rect(src_width, src_height, dst, true);
    let scale = fitted.width() / src_width;
    let visible_width = dst.width() / scale;
    let visible_height = dst.height() / scale;
    let x = (src_width - visible_width) / 2.0;
    let y = (src_height - visible_height) / 2.0;

    Rect::from_xywh(x, y, visible_width, visible_height)
}
