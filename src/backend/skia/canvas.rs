use std::{cell::RefCell, collections::HashMap, time::Instant};

use anyhow::{Context, Result, anyhow};
use skia_safe::{
    BlurStyle, Canvas, ClipOp, Data, Image as SkiaImage, ImageInfo, MaskFilter, Paint, PaintStyle,
    PathBuilder, Picture, PictureRecorder, RRect, Rect, TileMode, canvas::SrcRectConstraint,
    gradient_shader, images,
};

use crate::{
    display::cache_key::text_picture_cache_key,
    display::list::{
        BitmapDisplayItem, CanvasDisplayItem, DisplayCommand, DisplayItem, DisplayList,
        DisplayRect, DisplayTransform, LucideDisplayItem, RectDisplayItem, TextDisplayItem,
    },
    display::{
        cache_key::subtree_picture_cache_key,
        tree::{DisplayNode, DisplayTree},
    },
    frame_ctx::FrameCtx,
    resource::{
        assets::AssetsMap,
        bitmap_source::{BitmapSourceKind, bitmap_source_kind},
        media::MediaContext,
    },
    runtime::profile::BackendProfile,
    scene::script::{CanvasCommand, ScriptColor, ScriptLineCap, ScriptLineJoin},
    style::{BackgroundFill, GradientDirection, ObjectFit, ShadowStyle, Transform},
};

use super::{
    cache::{SkiaImageCache, SkiaSubtreePictureCache, SkiaTextPictureCache},
    text as skia_text,
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
    snapshot_record_ms: f64,
    snapshot_draw_ms: f64,
    cache_hits: usize,
    cache_misses: usize,
}

#[derive(Clone)]
struct CanvasPaintState {
    fill_color: ScriptColor,
    stroke_color: ScriptColor,
    line_width: f32,
    line_cap: ScriptLineCap,
    line_join: ScriptLineJoin,
    line_dash: Option<Vec<f32>>,
    line_dash_phase: f32,
    global_alpha: f32,
}

impl Default for CanvasPaintState {
    fn default() -> Self {
        Self {
            fill_color: ScriptColor {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            stroke_color: ScriptColor {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            line_width: 1.0,
            line_cap: ScriptLineCap::Butt,
            line_join: ScriptLineJoin::Miter,
            line_dash: None,
            line_dash_phase: 0.0,
            global_alpha: 1.0,
        }
    }
}

pub struct SkiaBackend<'a> {
    canvas: &'a Canvas,
    assets: &'a AssetsMap,
    media_ctx: Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
    image_cache: SkiaImageCache,
    text_picture_cache: SkiaTextPictureCache,
    subtree_picture_cache: Option<SkiaSubtreePictureCache>,
    profile: Option<&'a mut BackendProfile>,
}

impl<'a> SkiaBackend<'a> {
    pub fn new_with_cache_and_profile(
        canvas: &'a Canvas,
        _width: i32,
        _height: i32,
        assets: &'a AssetsMap,
        image_cache: SkiaImageCache,
        text_picture_cache: SkiaTextPictureCache,
        subtree_picture_cache: Option<SkiaSubtreePictureCache>,
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

    fn draw_display_children(&mut self, children: &[DisplayNode]) -> Result<()> {
        for child in children {
            self.draw_display_subtree(child)?;
        }
        Ok(())
    }

    fn draw_display_subtree(&mut self, node: &DisplayNode) -> Result<()> {
        if node.opacity <= 0.0 {
            return Ok(());
        }

        self.canvas.save();
        apply_transform(self.canvas, &node.transform);

        let subtree_cache = self.subtree_picture_cache.clone();
        if let Some(cache) = subtree_cache {
            if let Some(key) = subtree_picture_cache_key(node, self.assets) {
                if let Some(picture) = cache.borrow().get(&key).cloned() {
                    if let Some(profile) = self.profile.as_deref_mut() {
                        profile.subtree_snapshot_cache_hits += 1;
                    }
                    self.draw_subtree_picture(node, &picture)?;
                    self.canvas.restore();
                    return Ok(());
                }

                let picture = self.record_cached_subtree_picture(node)?;
                cache.borrow_mut().insert(key, picture.clone());
                if let Some(profile) = self.profile.as_deref_mut() {
                    profile.subtree_snapshot_cache_misses += 1;
                }
                self.draw_subtree_picture(node, &picture)?;
                self.canvas.restore();
                return Ok(());
            }
        }

        self.draw_display_subtree_contents(node)?;
        self.canvas.restore();
        Ok(())
    }

    fn draw_display_subtree_contents(&mut self, node: &DisplayNode) -> Result<()> {
        self.with_display_opacity(node.opacity, node.transform.bounds, |backend| {
            backend.draw_display_item(&node.item)?;
            if let Some(clip) = &node.clip {
                backend.canvas.save();
                clip_bounds(backend.canvas, clip.bounds, clip.border_radius);
                backend.draw_display_children(&node.children)?;
                backend.canvas.restore();
                Ok(())
            } else {
                backend.draw_display_children(&node.children)
            }
        })
    }

    fn draw_display_item(&mut self, item: &DisplayItem) -> Result<()> {
        match item {
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
                    profile.text_snapshot_record_ms += stats.snapshot_record_ms;
                    profile.text_snapshot_draw_ms += stats.snapshot_draw_ms;
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
            DisplayItem::Canvas(canvas) => {
                let started = Instant::now();
                draw_canvas_item(
                    self.canvas,
                    canvas,
                    self.assets,
                    &self.image_cache,
                    &mut self.media_ctx,
                    self.frame_ctx,
                )?;
                if let Some(profile) = self.profile.as_deref_mut() {
                    profile.draw_bitmap_count += 1;
                    profile.bitmap_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
                }
            }
            DisplayItem::Lucide(lucide) => {
                draw_lucide(self.canvas, lucide);
            }
        }
        Ok(())
    }

    fn record_cached_subtree_picture(&mut self, node: &DisplayNode) -> Result<Picture> {
        let started = Instant::now();
        let bounds = Rect::from_xywh(
            0.0,
            0.0,
            node.transform.bounds.width.max(1.0),
            node.transform.bounds.height.max(1.0),
        );
        let mut recorder = PictureRecorder::new();
        let recording_canvas = recorder.begin_recording(bounds, false);
        let mut backend = SkiaBackend::new_with_cache_and_profile(
            recording_canvas,
            node.transform.bounds.width as i32,
            node.transform.bounds.height as i32,
            self.assets,
            self.image_cache.clone(),
            self.text_picture_cache.clone(),
            self.subtree_picture_cache.clone(),
            None,
            self.frame_ctx,
            self.profile.as_deref_mut(),
        );
        backend.draw_display_item(&node.item)?;
        if let Some(clip) = &node.clip {
            backend.canvas.save();
            clip_bounds(backend.canvas, clip.bounds, clip.border_radius);
            backend.draw_display_children(&node.children)?;
            backend.canvas.restore();
        } else {
            backend.draw_display_children(&node.children)?;
        }
        let picture = recorder
            .finish_recording_as_picture(None)
            .ok_or_else(|| anyhow!("failed to record subtree picture"))?;
        if let Some(profile) = self.profile.as_deref_mut() {
            profile.scene_snapshot_record_ms += started.elapsed().as_secs_f64() * 1000.0;
        }
        Ok(picture)
    }

    fn draw_subtree_picture(&mut self, node: &DisplayNode, picture: &Picture) -> Result<()> {
        self.with_display_opacity(node.opacity, node.transform.bounds, |backend| {
            let started = Instant::now();
            backend.canvas.draw_picture(picture, None, None);
            if let Some(profile) = backend.profile.as_deref_mut() {
                profile.scene_snapshot_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
            }
            Ok(())
        })
    }

    fn with_display_opacity<T>(
        &mut self,
        opacity: f32,
        bounds: DisplayRect,
        draw: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let uses_layer = opacity < 1.0;
        if uses_layer {
            if let Some(profile) = self.profile.as_deref_mut() {
                profile.save_layer_count += 1;
            }
            let alpha = (opacity * 255.0).round() as u32;
            self.canvas
                .save_layer_alpha(layout_rect_to_skia(bounds), alpha);
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
            DisplayCommand::Clip { clip } => {
                clip_bounds(self.canvas, clip.bounds, clip.border_radius);
            }
            DisplayCommand::ApplyTransform { transform } => {
                apply_transform(self.canvas, transform);
            }
            DisplayCommand::Draw { item } => self.draw_display_item(item)?,
        }
        Ok(())
    }
}

pub(crate) fn record_display_list_composite_source<'a>(
    list: &DisplayList,
    width: i32,
    height: i32,
    assets: &'a AssetsMap,
    image_cache: SkiaImageCache,
    text_picture_cache: SkiaTextPictureCache,
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
        profile.scene_snapshot_record_ms += started.elapsed().as_secs_f64() * 1000.0;
    }
    Ok(picture)
}

pub(crate) fn draw_display_tree_with_subtree_cache<'a>(
    display_tree: &DisplayTree,
    canvas: &'a Canvas,
    assets: &'a AssetsMap,
    image_cache: SkiaImageCache,
    text_picture_cache: SkiaTextPictureCache,
    subtree_picture_cache: SkiaSubtreePictureCache,
    media_ctx: Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
    profile: Option<&'a mut BackendProfile>,
) -> Result<()> {
    let mut backend = SkiaBackend::new_with_cache_and_profile(
        canvas,
        display_tree.root.transform.bounds.width as i32,
        display_tree.root.transform.bounds.height as i32,
        assets,
        image_cache,
        text_picture_cache,
        Some(subtree_picture_cache),
        media_ctx,
        frame_ctx,
        profile,
    );
    backend.draw_display_subtree(&display_tree.root)
}

pub(crate) fn record_display_tree_composite_source_with_subtree_cache<'a>(
    display_tree: &DisplayTree,
    width: i32,
    height: i32,
    assets: &'a AssetsMap,
    image_cache: SkiaImageCache,
    text_picture_cache: SkiaTextPictureCache,
    subtree_picture_cache: SkiaSubtreePictureCache,
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
    backend.draw_display_subtree(&display_tree.root)?;
    let picture = recorder
        .finish_recording_as_picture(None)
        .ok_or_else(|| anyhow!("failed to record display tree picture"))?;
    if let Some(profile) = profile {
        profile.scene_snapshot_record_ms += started.elapsed().as_secs_f64() * 1000.0;
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
    apply_blur_effect(&mut paint, style.blur_sigma);

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
            snapshot_record_ms: 0.0,
            snapshot_draw_ms: draw_started.elapsed().as_secs_f64() * 1000.0,
            cache_hits: 1,
            cache_misses: 0,
        })
    } else {
        let record_started = Instant::now();
        let picture = record_text_picture(text)?;
        let snapshot_record_ms = record_started.elapsed().as_secs_f64() * 1000.0;
        text_picture_cache
            .borrow_mut()
            .insert(cache_key, picture.clone());

        let draw_started = Instant::now();
        canvas.save();
        canvas.translate((text.bounds.x, text.bounds.y));
        canvas.draw_picture(&picture, None, None);
        canvas.restore();
        Ok(TextDrawStats {
            snapshot_record_ms,
            snapshot_draw_ms: draw_started.elapsed().as_secs_f64() * 1000.0,
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
    skia_text::draw_text(
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
    apply_blur_effect(&mut paint, bitmap.paint.blur_sigma);

    let src_width = bitmap.width as f32;
    let src_height = bitmap.height as f32;

    if let Some(shadow) = bitmap.paint.shadow {
        draw_shadow(canvas, dst, radius, shadow);
    }

    if let Some(color) = bitmap.paint.background {
        let mut background_paint = Paint::default();
        background_paint.set_anti_alias(true);
        apply_blur_effect(&mut background_paint, bitmap.paint.blur_sigma);
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
        apply_blur_effect(&mut border_paint, bitmap.paint.blur_sigma);
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

fn draw_canvas_item(
    canvas: &Canvas,
    item: &CanvasDisplayItem,
    assets: &AssetsMap,
    image_cache: &RefCell<HashMap<String, Option<SkiaImage>>>,
    media_ctx: &mut Option<&mut MediaContext>,
    frame_ctx: &FrameCtx,
) -> Result<()> {
    let mut state = CanvasPaintState::default();
    let mut path = PathBuilder::new();

    canvas.save();
    canvas.clip_rect(layout_rect_to_skia(item.bounds), ClipOp::Intersect, true);

    for command in &item.commands {
        match command {
            CanvasCommand::Save => {
                canvas.save();
            }
            CanvasCommand::Restore => {
                canvas.restore();
            }
            CanvasCommand::SetFillStyle { color } => {
                state.fill_color = *color;
            }
            CanvasCommand::SetStrokeStyle { color } => {
                state.stroke_color = *color;
            }
            CanvasCommand::SetLineWidth { width } => {
                state.line_width = *width;
            }
            CanvasCommand::SetLineCap { cap } => {
                state.line_cap = *cap;
            }
            CanvasCommand::SetLineJoin { join } => {
                state.line_join = *join;
            }
            CanvasCommand::SetLineDash { intervals, phase } => {
                state.line_dash = Some(intervals.clone());
                state.line_dash_phase = *phase;
            }
            CanvasCommand::ClearLineDash => {
                state.line_dash = None;
                state.line_dash_phase = 0.0;
            }
            CanvasCommand::SetGlobalAlpha { alpha } => {
                state.global_alpha = *alpha;
            }
            CanvasCommand::Translate { x, y } => {
                canvas.translate((*x, *y));
            }
            CanvasCommand::Scale { x, y } => {
                canvas.scale((*x, *y));
            }
            CanvasCommand::Rotate { degrees } => {
                canvas.rotate(*degrees, None);
            }
            CanvasCommand::ClipRect {
                x,
                y,
                width,
                height,
            } => {
                canvas.clip_rect(
                    Rect::from_xywh(*x, *y, *width, *height),
                    ClipOp::Intersect,
                    true,
                );
            }
            CanvasCommand::Clear { color } => match color {
                Some(color) => {
                    canvas.clear(apply_script_alpha(*color, state.global_alpha));
                }
                None => {
                    canvas.clear(skia_safe::Color::TRANSPARENT);
                }
            },
            CanvasCommand::FillRect {
                x,
                y,
                width,
                height,
                color,
            } => {
                let mut paint = fill_paint_for_canvas_state(&state);
                paint.set_color(apply_script_alpha(*color, state.global_alpha));
                canvas.draw_rect(Rect::from_xywh(*x, *y, *width, *height), &paint);
            }
            CanvasCommand::FillRRect {
                x,
                y,
                width,
                height,
                radius,
            } => {
                let paint = fill_paint_for_canvas_state(&state);
                let rect = Rect::from_xywh(*x, *y, *width, *height);
                let rrect = RRect::new_rect_xy(rect, *radius, *radius);
                canvas.draw_rrect(rrect, &paint);
            }
            CanvasCommand::StrokeRect {
                x,
                y,
                width,
                height,
                color,
                stroke_width,
            } => {
                let mut paint = stroke_paint_for_canvas_state(&state);
                paint.set_color(apply_script_alpha(*color, state.global_alpha));
                paint.set_stroke_width(*stroke_width);
                canvas.draw_rect(Rect::from_xywh(*x, *y, *width, *height), &paint);
            }
            CanvasCommand::StrokeRRect {
                x,
                y,
                width,
                height,
                radius,
            } => {
                let paint = stroke_paint_for_canvas_state(&state);
                let rect = Rect::from_xywh(*x, *y, *width, *height);
                let rrect = RRect::new_rect_xy(rect, *radius, *radius);
                canvas.draw_rrect(rrect, &paint);
            }
            CanvasCommand::DrawLine { x0, y0, x1, y1 } => {
                let paint = stroke_paint_for_canvas_state(&state);
                canvas.draw_line((*x0, *y0), (*x1, *y1), &paint);
            }
            CanvasCommand::FillCircle { cx, cy, radius } => {
                let paint = fill_paint_for_canvas_state(&state);
                canvas.draw_circle((*cx, *cy), *radius, &paint);
            }
            CanvasCommand::StrokeCircle { cx, cy, radius } => {
                let paint = stroke_paint_for_canvas_state(&state);
                canvas.draw_circle((*cx, *cy), *radius, &paint);
            }
            CanvasCommand::BeginPath => {
                path = PathBuilder::new();
            }
            CanvasCommand::MoveTo { x, y } => {
                path.move_to((*x, *y));
            }
            CanvasCommand::LineTo { x, y } => {
                path.line_to((*x, *y));
            }
            CanvasCommand::QuadTo { cx, cy, x, y } => {
                path.quad_to((*cx, *cy), (*x, *y));
            }
            CanvasCommand::CubicTo {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            } => {
                path.cubic_to((*c1x, *c1y), (*c2x, *c2y), (*x, *y));
            }
            CanvasCommand::ClosePath => {
                path.close();
            }
            CanvasCommand::FillPath => {
                let paint = fill_paint_for_canvas_state(&state);
                let path_snapshot = path.snapshot();
                canvas.draw_path(&path_snapshot, &paint);
            }
            CanvasCommand::StrokePath => {
                let paint = stroke_paint_for_canvas_state(&state);
                let path_snapshot = path.snapshot();
                canvas.draw_path(&path_snapshot, &paint);
            }
            CanvasCommand::DrawImage {
                asset_id,
                x,
                y,
                width,
                height,
                object_fit,
            } => {
                let image = load_asset_image(
                    &crate::resource::assets::AssetId(asset_id.clone()),
                    assets,
                    image_cache,
                    media_ctx,
                    frame_ctx,
                )?;
                let dst = Rect::from_xywh(*x, *y, *width, *height);
                let src_width = image.width() as f32;
                let src_height = image.height() as f32;
                let paint = Paint::default();
                match object_fit {
                    ObjectFit::Fill => {
                        canvas.draw_image_rect(image, None, dst, &paint);
                    }
                    ObjectFit::Contain => {
                        let fitted = fitted_rect(src_width, src_height, dst, false);
                        canvas.draw_image_rect(image, None, fitted, &paint);
                    }
                    ObjectFit::Cover => {
                        let src = cover_src_rect(src_width, src_height, dst);
                        canvas.draw_image_rect(
                            image,
                            Some((&src, SrcRectConstraint::Strict)),
                            dst,
                            &paint,
                        );
                    }
                }
            }
        }
    }

    canvas.restore();
    Ok(())
}

fn apply_script_alpha(color: ScriptColor, global_alpha: f32) -> skia_safe::Color {
    let alpha = ((color.a as f32) * global_alpha.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    skia_safe::Color::from_argb(alpha, color.r, color.g, color.b)
}

fn fill_paint_for_canvas_state(state: &CanvasPaintState) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(PaintStyle::Fill);
    paint.set_color(apply_script_alpha(state.fill_color, state.global_alpha));
    paint
}

fn stroke_paint_for_canvas_state(state: &CanvasPaintState) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(PaintStyle::Stroke);
    paint.set_color(apply_script_alpha(state.stroke_color, state.global_alpha));
    paint.set_stroke_width(state.line_width.max(0.0));
    paint.set_stroke_cap(match state.line_cap {
        ScriptLineCap::Butt => skia_safe::paint::Cap::Butt,
        ScriptLineCap::Round => skia_safe::paint::Cap::Round,
        ScriptLineCap::Square => skia_safe::paint::Cap::Square,
    });
    paint.set_stroke_join(match state.line_join {
        ScriptLineJoin::Miter => skia_safe::paint::Join::Miter,
        ScriptLineJoin::Round => skia_safe::paint::Join::Round,
        ScriptLineJoin::Bevel => skia_safe::paint::Join::Bevel,
    });
    if let Some(intervals) = &state.line_dash {
        if let Some(path_effect) = skia_safe::PathEffect::dash(intervals, state.line_dash_phase) {
            paint.set_path_effect(path_effect);
        }
    }
    paint
}

fn load_asset_image(
    asset_id: &crate::resource::assets::AssetId,
    assets: &AssetsMap,
    image_cache: &RefCell<HashMap<String, Option<SkiaImage>>>,
    media_ctx: &mut Option<&mut MediaContext>,
    frame_ctx: &FrameCtx,
) -> Result<SkiaImage> {
    let path = assets
        .path(asset_id)
        .ok_or_else(|| anyhow!("missing asset path for {}", asset_id.0))?;

    if bitmap_source_kind(path) == BitmapSourceKind::Video {
        let media = media_ctx
            .as_deref_mut()
            .ok_or_else(|| anyhow!("video asset requires media context: {}", path.display()))?;
        let target_time = frame_ctx.frame as f64 / frame_ctx.fps as f64;
        let (data, width, height) = media
            .get_bitmap(path, target_time)
            .with_context(|| format!("failed to decode video frame: {}", path.display()))?;
        let info = ImageInfo::new(
            (width as i32, height as i32),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        return images::raster_from_data(&info, Data::new_copy(&data), width as usize * 4)
            .ok_or_else(|| {
                anyhow!(
                    "failed to create skia image from video frame: {}",
                    path.display()
                )
            });
    }

    let key = asset_id.0.clone();
    let mut cache = image_cache.borrow_mut();
    if let Some(Some(img)) = cache.get(&key) {
        return Ok(img.clone());
    }

    let encoded = std::fs::read(path)
        .with_context(|| format!("failed to read image asset: {}", path.display()))?;
    let data = skia_safe::Data::new_copy(&encoded);
    let image = skia_safe::Image::from_encoded(data)
        .ok_or_else(|| anyhow!("failed to decode image asset: {}", path.display()))?;
    cache.insert(key, Some(image.clone()));
    Ok(image)
}

fn effective_corner_radius(rect: Rect, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }

    radius.min(rect.width() / 2.0).min(rect.height() / 2.0)
}

fn apply_blur_effect(paint: &mut Paint, blur_sigma: Option<f32>) {
    let Some(sigma) = blur_sigma.filter(|sigma| *sigma > 0.0) else {
        return;
    };

    if let Some(mask_filter) = MaskFilter::blur(BlurStyle::Normal, sigma, false) {
        paint.set_mask_filter(mask_filter);
    }
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

fn layout_rect_to_skia(rect: DisplayRect) -> Rect {
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
                GradientDirection::ToBottom => (
                    (bounds.center_x(), bounds.top()),
                    (bounds.center_x(), bounds.bottom()),
                ),
                GradientDirection::ToTop => (
                    (bounds.center_x(), bounds.bottom()),
                    (bounds.center_x(), bounds.top()),
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

fn clip_bounds(canvas: &Canvas, bounds: DisplayRect, border_radius: f32) {
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
