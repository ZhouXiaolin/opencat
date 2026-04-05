use std::{
    cell::RefCell,
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::Path,
    rc::Rc,
    time::Instant,
};

use anyhow::{Context, Result, anyhow};
use skia_safe::{
    Canvas, Data, Image as SkiaImage, ImageInfo, Paint, PaintStyle, Picture, PictureRecorder,
    RRect, Rect, canvas::SrcRectConstraint, images,
};

use crate::{
    assets::AssetsMap,
    display::list::{
        BitmapDisplayItem, DisplayCommand, DisplayItem, DisplayList, DisplayTransform,
        RectDisplayItem, TextDisplayItem,
    },
    frame_ctx::FrameCtx,
    layout::tree::LayoutRect,
    media::MediaContext,
    style::{ObjectFit, ShadowStyle, Transform},
    typography,
};

pub(crate) type ImageCache = Rc<RefCell<HashMap<String, Option<SkiaImage>>>>;
pub(crate) type TextPictureCache = Rc<RefCell<HashMap<u64, Picture>>>;

#[derive(Clone, Debug, Default)]
pub struct BackendProfile {
    pub rect_draw_ms: f64,
    pub text_draw_ms: f64,
    pub text_picture_record_ms: f64,
    pub text_picture_draw_ms: f64,
    pub bitmap_draw_ms: f64,
    pub image_decode_ms: f64,
    pub video_decode_ms: f64,
    pub picture_record_ms: f64,
    pub picture_draw_ms: f64,
    pub picture_cache_hits: usize,
    pub picture_cache_misses: usize,
    pub text_cache_hits: usize,
    pub text_cache_misses: usize,
    pub image_cache_hits: usize,
    pub image_cache_misses: usize,
    pub video_frame_decodes: usize,
    pub draw_rect_count: usize,
    pub draw_text_count: usize,
    pub draw_bitmap_count: usize,
    pub save_layer_count: usize,
}

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
    profile: Option<&'a mut BackendProfile>,
}

impl<'a> SkiaBackend<'a> {
    pub fn new(
        canvas: &'a Canvas,
        _width: i32,
        _height: i32,
        assets: &'a AssetsMap,
        media_ctx: Option<&'a mut MediaContext>,
        frame_ctx: &'a FrameCtx,
    ) -> Self {
        Self {
            canvas,
            assets,
            media_ctx,
            frame_ctx,
            image_cache: Rc::new(RefCell::new(HashMap::new())),
            text_picture_cache: Rc::new(RefCell::new(HashMap::new())),
            profile: None,
        }
    }

    pub fn new_with_cache(
        canvas: &'a Canvas,
        _width: i32,
        _height: i32,
        assets: &'a AssetsMap,
        image_cache: ImageCache,
        text_picture_cache: TextPictureCache,
        media_ctx: Option<&'a mut MediaContext>,
        frame_ctx: &'a FrameCtx,
    ) -> Self {
        Self {
            canvas,
            assets,
            image_cache,
            text_picture_cache,
            media_ctx,
            frame_ctx,
            profile: None,
        }
    }

    pub fn new_with_cache_and_profile(
        canvas: &'a Canvas,
        _width: i32,
        _height: i32,
        assets: &'a AssetsMap,
        image_cache: ImageCache,
        text_picture_cache: TextPictureCache,
        media_ctx: Option<&'a mut MediaContext>,
        frame_ctx: &'a FrameCtx,
        profile: Option<&'a mut BackendProfile>,
    ) -> Self {
        Self {
            canvas,
            assets,
            image_cache,
            text_picture_cache,
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

pub(crate) fn display_list_uses_video(list: &DisplayList, assets: &AssetsMap) -> bool {
    list.commands.iter().any(|command| match command {
        DisplayCommand::Draw {
            item: DisplayItem::Bitmap(bitmap),
        } => assets
            .path(&bitmap.asset_id)
            .map(is_video_path)
            .unwrap_or(false),
        _ => false,
    })
}

fn draw_rect(canvas: &Canvas, rect: &RectDisplayItem) {
    let style = &rect.paint;
    if style.background.is_none() && style.border_width.is_none() && style.shadow.is_none() {
        return;
    }

    let rect = layout_rect_to_skia(rect.bounds);

    if let Some(shadow) = style.shadow {
        draw_shadow(canvas, rect, style.border_radius, shadow);
    }

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    if style.border_radius > 0.0 {
        let rrect = RRect::new_rect_xy(rect, style.border_radius, style.border_radius);

        if let Some(color) = style.background {
            paint.set_color(color.to_skia());
            canvas.draw_rrect(rrect, &paint);
        }

        if let (Some(width), Some(color)) = (style.border_width, style.border_color) {
            paint.set_color(color.to_skia());
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(width);
            canvas.draw_rrect(rrect, &paint);
        }
    } else {
        if let Some(color) = style.background {
            paint.set_color(color.to_skia());
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

    if radius > 0.0 {
        let rrect = RRect::new_rect_xy(shadow_rect, radius + blur / 2.0, radius + blur / 2.0);
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

fn text_picture_cache_key(text: &TextDisplayItem) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.text.hash(&mut hasher);
    hash_text_style(&text.style, &mut hasher);
    text.allow_wrap.hash(&mut hasher);
    text.bounds.width.to_bits().hash(&mut hasher);
    text.bounds.height.to_bits().hash(&mut hasher);
    hasher.finish()
}

fn hash_text_style(style: &crate::style::ComputedTextStyle, hasher: &mut DefaultHasher) {
    style.color.hash(hasher);
    style.font_weight.hash(hasher);
    style.text_align.hash(hasher);
    style.text_px.to_bits().hash(hasher);
    style.letter_spacing.to_bits().hash(hasher);
    style.line_height.to_bits().hash(hasher);
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

    let image = if is_video_path(path) {
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
    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    let src_width = bitmap.width as f32;
    let src_height = bitmap.height as f32;

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
    stats.draw_ms = draw_started.elapsed().as_secs_f64() * 1000.0;
    Ok(stats)
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

fn cover_src_rect(src_width: f32, src_height: f32, dst: Rect) -> Rect {
    let fitted = fitted_rect(src_width, src_height, dst, true);
    let scale = fitted.width() / src_width;
    let visible_width = dst.width() / scale;
    let visible_height = dst.height() / scale;
    let x = (src_width - visible_width) / 2.0;
    let y = (src_height - visible_height) / 2.0;

    Rect::from_xywh(x, y, visible_width, visible_height)
}

pub(crate) fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "mp4" | "mov" | "m4v" | "webm" | "mkv" | "avi"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn new_image_cache() -> ImageCache {
    Rc::new(RefCell::new(HashMap::new()))
}

pub(crate) fn new_text_picture_cache() -> TextPictureCache {
    Rc::new(RefCell::new(HashMap::new()))
}
