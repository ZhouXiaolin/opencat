use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use skia_safe::{
    BlurStyle, Canvas, ClipOp, Color4f, Data, Font, Image as SkiaImage, ImageInfo, MaskFilter,
    Paint, PaintStyle, PathBuilder, Picture, PictureRecorder, Point, RRect, Rect, TileMode,
    canvas::{SaveLayerRec, SrcRectConstraint},
    gradient_shader, image_filters, images,
};

use crate::{
    display::list::{
        BitmapDisplayItem, DisplayCommand, DisplayItem, DisplayList, DisplayRect, DisplayTransform,
        DrawScriptDisplayItem, LucideDisplayItem, RectDisplayItem, TextDisplayItem,
    },
    display::tree::{DisplayNode, DisplayTree},
    frame_ctx::FrameCtx,
    resource::{
        assets::AssetsMap,
        bitmap_source::{BitmapSourceKind, bitmap_source_kind},
        media::{MediaContext, VideoFrameRequest},
    },
    runtime::cache::{
        ImageCache, ItemPictureCache, SceneStaticPictureCache, SubtreeSnapshotCache,
        TextSnapshotCache,
    },
    runtime::fingerprint::{
        PaintVariance, item_paint_fingerprint, scene_static_skeleton_fingerprint,
        text_paint_fingerprint,
    },
    runtime::profile::BackendProfile,
    scene::script::{
        CanvasCommand, ScriptColor, ScriptFontEdging, ScriptLineCap, ScriptLineJoin,
        ScriptPointMode,
    },
    style::{
        BackgroundFill, BoxShadow, DropShadow, GradientDirection, InsetShadow, ObjectFit, Transform,
    },
};

use super::{
    color::{script_color, skia_color},
    text as skia_text,
};

struct BitmapDrawStats {
    draw_ms: f64,
    image_decode_ms: f64,
    video_decode_ms: f64,
    image_cache_hits: usize,
    image_cache_misses: usize,
    video_frame_cache_hits: usize,
    video_frame_cache_misses: usize,
    video_frame_decodes: usize,
}

struct TextDrawStats {
    snapshot_record_ms: f64,
    snapshot_draw_ms: f64,
    cache_hits: usize,
    cache_misses: usize,
}

struct ItemPictureDrawStats {
    record_ms: f64,
    draw_ms: f64,
    cache_hits: usize,
    cache_misses: usize,
}

#[derive(Clone)]
struct DrawScriptPaintState {
    fill_color: ScriptColor,
    stroke_color: ScriptColor,
    line_width: f32,
    line_cap: ScriptLineCap,
    line_join: ScriptLineJoin,
    line_dash: Option<Vec<f32>>,
    line_dash_phase: f32,
    global_alpha: f32,
    anti_alias: bool,
}

impl Default for DrawScriptPaintState {
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
            anti_alias: true,
        }
    }
}

pub struct SkiaBackend<'a> {
    canvas: &'a Canvas,
    assets: &'a AssetsMap,
    media_ctx: Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
    image_cache: ImageCache,
    text_snapshot_cache: TextSnapshotCache,
    item_picture_cache: ItemPictureCache,
    subtree_snapshot_cache: Option<SubtreeSnapshotCache>,
    profile: Option<&'a mut BackendProfile>,
}

impl<'a> SkiaBackend<'a> {
    pub fn new_with_cache_and_profile(
        canvas: &'a Canvas,
        _width: i32,
        _height: i32,
        assets: &'a AssetsMap,
        image_cache: ImageCache,
        text_snapshot_cache: TextSnapshotCache,
        item_picture_cache: ItemPictureCache,
        subtree_snapshot_cache: Option<SubtreeSnapshotCache>,
        media_ctx: Option<&'a mut MediaContext>,
        frame_ctx: &'a FrameCtx,
        profile: Option<&'a mut BackendProfile>,
    ) -> Self {
        Self {
            canvas,
            assets,
            image_cache,
            text_snapshot_cache,
            item_picture_cache,
            subtree_snapshot_cache,
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

    fn draw_display_children_static_only(&mut self, children: &[DisplayNode]) -> Result<()> {
        for child in children {
            self.draw_display_subtree_static_only(child)?;
        }
        Ok(())
    }

    fn draw_display_children_dynamic_only(&mut self, children: &[DisplayNode]) -> Result<()> {
        for child in children {
            self.draw_display_subtree_dynamic_only(child)?;
        }
        Ok(())
    }

    fn draw_display_subtree(&mut self, node: &DisplayNode) -> Result<()> {
        if node.opacity <= 0.0 {
            return Ok(());
        }

        self.canvas.save();
        apply_transform(self.canvas, &node.transform);
        let result = self.draw_display_subtree_after_transform(node);
        self.canvas.restore();
        result
    }

    fn draw_display_subtree_after_transform(&mut self, node: &DisplayNode) -> Result<()> {
        let subtree_cache = self.subtree_snapshot_cache.clone();
        if let Some(cache) = subtree_cache {
            if let Some(key) = node.snapshot_fingerprint {
                if let Some(snapshot) = cache.borrow_mut().get_cloned(&key) {
                    if let Some(profile) = self.profile.as_deref_mut() {
                        profile.subtree_snapshot_cache_hits += 1;
                    }
                    self.draw_subtree_snapshot(node, &snapshot)?;
                    return Ok(());
                }

                let snapshot = self.record_cached_subtree_snapshot(node)?;
                cache.borrow_mut().insert(key, snapshot.clone());
                if let Some(profile) = self.profile.as_deref_mut() {
                    profile.subtree_snapshot_cache_misses += 1;
                }
                self.draw_subtree_snapshot(node, &snapshot)?;
                return Ok(());
            }
        }

        self.draw_display_subtree_contents(node)
    }

    fn draw_display_subtree_contents(&mut self, node: &DisplayNode) -> Result<()> {
        self.with_display_opacity(node.opacity, node.layer_bounds(), |backend| {
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

    fn draw_display_subtree_static_only(&mut self, node: &DisplayNode) -> Result<()> {
        if node.opacity <= 0.0
            || node.paint_variance == PaintVariance::TimeVariant
            || node.composite_dirty
        {
            return Ok(());
        }

        self.canvas.save();
        apply_transform(self.canvas, &node.transform);
        let result = if !node.subtree_contains_dynamic {
            self.draw_display_subtree_after_transform(node)
        } else {
            self.with_display_opacity(node.opacity, node.layer_bounds(), |backend| {
                backend.draw_display_item(&node.item)?;
                if let Some(clip) = &node.clip {
                    backend.canvas.save();
                    clip_bounds(backend.canvas, clip.bounds, clip.border_radius);
                    backend.draw_display_children_static_only(&node.children)?;
                    backend.canvas.restore();
                    Ok(())
                } else {
                    backend.draw_display_children_static_only(&node.children)
                }
            })
        };
        self.canvas.restore();
        result
    }

    fn draw_display_subtree_dynamic_only(&mut self, node: &DisplayNode) -> Result<()> {
        if node.opacity <= 0.0 || !node.subtree_contains_dynamic {
            return Ok(());
        }

        self.canvas.save();
        apply_transform(self.canvas, &node.transform);
        let result = if node.paint_variance == PaintVariance::TimeVariant || node.composite_dirty {
            self.draw_display_subtree_contents(node)
        } else {
            self.with_display_opacity(node.opacity, node.layer_bounds(), |backend| {
                if let Some(clip) = &node.clip {
                    backend.canvas.save();
                    clip_bounds(backend.canvas, clip.bounds, clip.border_radius);
                    backend.draw_display_children_dynamic_only(&node.children)?;
                    backend.canvas.restore();
                    Ok(())
                } else {
                    backend.draw_display_children_dynamic_only(&node.children)
                }
            })
        };
        self.canvas.restore();
        result
    }

    fn draw_display_item(&mut self, item: &DisplayItem) -> Result<()> {
        if should_cache_item_picture(item)
            && let Some(cache_key) = item_paint_fingerprint(item, self.assets)
        {
            let stats = draw_item_picture_cached(
                self.canvas,
                item,
                cache_key,
                self.assets,
                &self.image_cache,
                &self.text_snapshot_cache,
                &self.item_picture_cache,
                &mut self.media_ctx,
                self.frame_ctx,
            )?;
            if let Some(profile) = self.profile.as_deref_mut() {
                profile.item_picture_record_ms += stats.record_ms;
                profile.item_picture_draw_ms += stats.draw_ms;
                profile.item_picture_cache_hits += stats.cache_hits;
                profile.item_picture_cache_misses += stats.cache_misses;
                match item {
                    DisplayItem::Bitmap(_) => {
                        profile.draw_bitmap_count += 1;
                        profile.bitmap_draw_ms += stats.draw_ms;
                    }
                    DisplayItem::DrawScript(_) => {
                        profile.draw_script_count += 1;
                        profile.draw_script_draw_ms += stats.draw_ms;
                    }
                    DisplayItem::Lucide(_) => {}
                    DisplayItem::Rect(_) | DisplayItem::Text(_) => {}
                }
            }
            return Ok(());
        }

        self.draw_display_item_uncached(item)
    }

    fn draw_display_item_uncached(&mut self, item: &DisplayItem) -> Result<()> {
        match item {
            DisplayItem::Rect(rect) => {
                let started = Instant::now();
                if let Some(shadow) = rect.paint.box_shadow {
                    draw_box_shadow(self.canvas, rect.bounds, rect.paint.border_radius, shadow);
                }
                if let Some(shadow) = rect.paint.drop_shadow {
                    draw_item_drop_shadow(self.canvas, rect.bounds, shadow, |canvas| {
                        draw_rect(canvas, rect);
                        Ok(())
                    })?;
                }
                draw_rect(self.canvas, rect);
                if let Some(profile) = self.profile.as_deref_mut() {
                    profile.draw_rect_count += 1;
                    profile.rect_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
                }
            }
            DisplayItem::Text(text) => {
                let started = Instant::now();
                if let Some(shadow) = text.drop_shadow {
                    draw_item_drop_shadow(self.canvas, text.bounds, shadow, |canvas| {
                        draw_text(canvas, text, &self.text_snapshot_cache).map(|_| ())
                    })?;
                }
                let stats = draw_text(self.canvas, text, &self.text_snapshot_cache)?;
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
                if let Some(shadow) = bitmap.paint.box_shadow {
                    draw_box_shadow(
                        self.canvas,
                        bitmap.bounds,
                        bitmap.paint.border_radius,
                        shadow,
                    );
                }
                if let Some(shadow) = bitmap.paint.drop_shadow {
                    draw_item_drop_shadow(self.canvas, bitmap.bounds, shadow, |canvas| {
                        draw_bitmap(
                            canvas,
                            bitmap,
                            self.assets,
                            &self.image_cache,
                            &mut self.media_ctx,
                            self.frame_ctx,
                        )
                        .map(|_| ())
                    })?;
                }
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
                    profile.video_frame_cache_hits += stats.video_frame_cache_hits;
                    profile.video_frame_cache_misses += stats.video_frame_cache_misses;
                    profile.video_frame_decodes += stats.video_frame_decodes;
                }
            }
            DisplayItem::DrawScript(script) => {
                let started = Instant::now();
                if let Some(shadow) = script.drop_shadow {
                    draw_item_drop_shadow(self.canvas, script.bounds, shadow, |canvas| {
                        draw_script_item(
                            canvas,
                            script,
                            self.assets,
                            &self.image_cache,
                            &mut self.media_ctx,
                            self.frame_ctx,
                        )
                    })?;
                }
                draw_script_item(
                    self.canvas,
                    script,
                    self.assets,
                    &self.image_cache,
                    &mut self.media_ctx,
                    self.frame_ctx,
                )?;
                if let Some(profile) = self.profile.as_deref_mut() {
                    profile.draw_script_count += 1;
                    profile.draw_script_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
                }
            }
            DisplayItem::Lucide(lucide) => {
                if let Some(shadow) = lucide.paint.drop_shadow {
                    draw_item_drop_shadow(self.canvas, lucide.bounds, shadow, |canvas| {
                        draw_lucide(canvas, lucide);
                        Ok(())
                    })?;
                }
                draw_lucide(self.canvas, lucide);
            }
        }
        Ok(())
    }

    fn record_cached_subtree_snapshot(&mut self, node: &DisplayNode) -> Result<Picture> {
        let started = Instant::now();
        let layer_bounds = node.layer_bounds();
        let bounds = layout_rect_to_skia(layer_bounds);
        let mut recorder = PictureRecorder::new();
        let recording_canvas = recorder.begin_recording(bounds, false);
        let mut backend = SkiaBackend::new_with_cache_and_profile(
            recording_canvas,
            layer_bounds.width.max(1.0) as i32,
            layer_bounds.height.max(1.0) as i32,
            self.assets,
            self.image_cache.clone(),
            self.text_snapshot_cache.clone(),
            self.item_picture_cache.clone(),
            self.subtree_snapshot_cache.clone(),
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
        let snapshot = recorder
            .finish_recording_as_picture(None)
            .ok_or_else(|| anyhow!("failed to record subtree snapshot"))?;
        if let Some(profile) = self.profile.as_deref_mut() {
            profile.scene_snapshot_record_ms += started.elapsed().as_secs_f64() * 1000.0;
        }
        Ok(snapshot)
    }

    fn draw_subtree_snapshot(&mut self, node: &DisplayNode, snapshot: &Picture) -> Result<()> {
        self.with_display_opacity(node.opacity, node.layer_bounds(), |backend| {
            let started = Instant::now();
            backend.canvas.draw_picture(snapshot, None, None);
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
                let bounds = layout_rect_to_skia(layer.bounds);
                if let Some(sigma) = layer.backdrop_blur_sigma.filter(|s| *s > 0.0) {
                    let alpha = (layer.opacity * 255.0).round() as u32;
                    let mut paint = Paint::default();
                    paint.set_alpha(alpha as u8);
                    let backdrop = image_filters::blur(
                        (sigma, sigma),
                        TileMode::Clamp,
                        None,
                        None::<skia_safe::image_filters::CropRect>,
                    );
                    if let Some(backdrop) = backdrop {
                        let rec = SaveLayerRec::default()
                            .bounds(&bounds)
                            .paint(&paint)
                            .backdrop(&backdrop);
                        self.canvas.save_layer(&rec);
                    } else {
                        self.canvas.save_layer_alpha(bounds, alpha);
                    }
                } else {
                    let alpha = (layer.opacity * 255.0).round() as u32;
                    self.canvas.save_layer_alpha(bounds, alpha);
                }
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
    image_cache: ImageCache,
    text_snapshot_cache: TextSnapshotCache,
    item_picture_cache: ItemPictureCache,
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
        text_snapshot_cache,
        item_picture_cache,
        None,
        media_ctx,
        frame_ctx,
        profile.as_deref_mut(),
    );
    backend.execute(list)?;
    let snapshot = recorder
        .finish_recording_as_picture(None)
        .ok_or_else(|| anyhow!("failed to record display list snapshot"))?;
    if let Some(profile) = profile {
        profile.scene_snapshot_record_ms += started.elapsed().as_secs_f64() * 1000.0;
    }
    Ok(snapshot)
}

pub(crate) fn draw_display_tree_with_subtree_cache<'a>(
    display_tree: &DisplayTree,
    canvas: &'a Canvas,
    assets: &'a AssetsMap,
    image_cache: ImageCache,
    text_snapshot_cache: TextSnapshotCache,
    item_picture_cache: ItemPictureCache,
    subtree_snapshot_cache: SubtreeSnapshotCache,
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
        text_snapshot_cache,
        item_picture_cache,
        Some(subtree_snapshot_cache),
        media_ctx,
        frame_ctx,
        profile,
    );
    backend.draw_display_subtree(&display_tree.root)
}

pub(crate) fn draw_display_tree_layered_video<'a>(
    display_tree: &DisplayTree,
    canvas: &'a Canvas,
    assets: &'a AssetsMap,
    image_cache: ImageCache,
    text_snapshot_cache: TextSnapshotCache,
    item_picture_cache: ItemPictureCache,
    subtree_snapshot_cache: SubtreeSnapshotCache,
    scene_static_picture_cache: SceneStaticPictureCache,
    media_ctx: Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
    mut profile: Option<&'a mut BackendProfile>,
) -> Result<()> {
    let skeleton_fp = scene_static_skeleton_fingerprint(&display_tree.root);
    let static_snapshot = if let Some(snapshot) = scene_static_picture_cache
        .borrow_mut()
        .get_cloned(&skeleton_fp)
    {
        if let Some(profile) = profile.as_deref_mut() {
            profile.scene_static_cache_hits += 1;
        }
        snapshot
    } else {
        let snapshot = record_display_tree_static_skeleton(
            display_tree,
            display_tree.root.transform.bounds.width as i32,
            display_tree.root.transform.bounds.height as i32,
            assets,
            image_cache.clone(),
            text_snapshot_cache.clone(),
            item_picture_cache.clone(),
            subtree_snapshot_cache.clone(),
            frame_ctx,
            profile.as_deref_mut(),
        )?;
        scene_static_picture_cache
            .borrow_mut()
            .insert(skeleton_fp, snapshot.clone());
        if let Some(profile) = profile.as_deref_mut() {
            profile.scene_static_cache_misses += 1;
        }
        snapshot
    };

    if let Some(profile) = profile.as_deref_mut() {
        let started = Instant::now();
        canvas.draw_picture(&static_snapshot, None, None);
        profile.scene_static_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
    } else {
        canvas.draw_picture(&static_snapshot, None, None);
    }

    let dynamic_started = Instant::now();
    let mut backend = SkiaBackend::new_with_cache_and_profile(
        canvas,
        display_tree.root.transform.bounds.width as i32,
        display_tree.root.transform.bounds.height as i32,
        assets,
        image_cache,
        text_snapshot_cache,
        item_picture_cache,
        Some(subtree_snapshot_cache),
        media_ctx,
        frame_ctx,
        profile.as_deref_mut(),
    );
    backend.draw_display_subtree_dynamic_only(&display_tree.root)?;
    if let Some(profile) = profile {
        profile.scene_dynamic_draw_ms += dynamic_started.elapsed().as_secs_f64() * 1000.0;
    }
    Ok(())
}

pub(crate) fn record_display_tree_composite_source_with_subtree_cache<'a>(
    display_tree: &DisplayTree,
    width: i32,
    height: i32,
    assets: &'a AssetsMap,
    image_cache: ImageCache,
    text_snapshot_cache: TextSnapshotCache,
    item_picture_cache: ItemPictureCache,
    subtree_snapshot_cache: SubtreeSnapshotCache,
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
        text_snapshot_cache,
        item_picture_cache,
        Some(subtree_snapshot_cache),
        media_ctx,
        frame_ctx,
        profile.as_deref_mut(),
    );
    backend.draw_display_subtree(&display_tree.root)?;
    let snapshot = recorder
        .finish_recording_as_picture(None)
        .ok_or_else(|| anyhow!("failed to record display tree snapshot"))?;
    if let Some(profile) = profile {
        profile.scene_snapshot_record_ms += started.elapsed().as_secs_f64() * 1000.0;
    }
    Ok(snapshot)
}

pub(crate) fn record_display_tree_static_skeleton<'a>(
    display_tree: &DisplayTree,
    width: i32,
    height: i32,
    assets: &'a AssetsMap,
    image_cache: ImageCache,
    text_snapshot_cache: TextSnapshotCache,
    item_picture_cache: ItemPictureCache,
    subtree_snapshot_cache: SubtreeSnapshotCache,
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
        text_snapshot_cache,
        item_picture_cache,
        Some(subtree_snapshot_cache),
        None,
        frame_ctx,
        profile.as_deref_mut(),
    );
    backend.draw_display_subtree_static_only(&display_tree.root)?;
    let snapshot = recorder
        .finish_recording_as_picture(None)
        .ok_or_else(|| anyhow!("failed to record display tree static skeleton"))?;
    if let Some(profile) = profile {
        profile.scene_static_record_ms += started.elapsed().as_secs_f64() * 1000.0;
    }
    Ok(snapshot)
}

fn draw_rect(canvas: &Canvas, rect: &RectDisplayItem) {
    let style = &rect.paint;
    if style.background.is_none() && style.border_width.is_none() && style.inset_shadow.is_none() {
        return;
    }

    let bounds = rect.bounds;
    let rect = layout_rect_to_skia(bounds);
    let radii = effective_corner_radius(rect, style.border_radius);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    apply_blur_effect(&mut paint, style.blur_sigma);

    if radii.iter().any(|&r| r > 0.0) {
        let rrect = make_rrect(rect, style.border_radius);

        if let Some(background) = style.background {
            apply_background_paint(&mut paint, background, rect);
            canvas.draw_rrect(rrect, &paint);
        }

        if let Some(shadow) = style.inset_shadow {
            draw_inset_shadow(canvas, bounds, style.border_radius, shadow);
        }

        if let (Some(width), Some(color)) = (style.border_width, style.border_color) {
            paint.set_color(skia_color(color));
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(width);
            canvas.draw_rrect(rrect, &paint);
        }
    } else {
        if let Some(background) = style.background {
            apply_background_paint(&mut paint, background, rect);
            canvas.draw_rect(rect, &paint);
        }

        if let Some(shadow) = style.inset_shadow {
            draw_inset_shadow(canvas, bounds, style.border_radius, shadow);
        }

        if let (Some(width), Some(color)) = (style.border_width, style.border_color) {
            paint.set_color(skia_color(color));
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(width);
            canvas.draw_rect(rect, &paint);
        }
    }
}

fn spread_radius(
    border_radius: crate::style::BorderRadius,
    spread: f32,
) -> crate::style::BorderRadius {
    crate::style::BorderRadius {
        top_left: (border_radius.top_left + spread).max(0.0),
        top_right: (border_radius.top_right + spread).max(0.0),
        bottom_right: (border_radius.bottom_right + spread).max(0.0),
        bottom_left: (border_radius.bottom_left + spread).max(0.0),
    }
}

fn draw_box_shadow(
    canvas: &Canvas,
    bounds: DisplayRect,
    border_radius: crate::style::BorderRadius,
    shadow: BoxShadow,
) {
    let shadow_bounds = if shadow.spread != 0.0 {
        bounds.outset(shadow.spread, shadow.spread, shadow.spread, shadow.spread)
    } else {
        bounds
    };
    let rect = layout_rect_to_skia(shadow_bounds.translate(shadow.offset_x, shadow.offset_y));
    let sr = spread_radius(border_radius, shadow.spread);
    let radii = effective_corner_radius(rect, sr);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(PaintStyle::Fill);
    paint.set_color(skia_color(shadow.color));

    if let Some(mask_filter) = MaskFilter::blur(BlurStyle::Normal, shadow.blur_sigma, false) {
        paint.set_mask_filter(mask_filter);
    }

    if radii.iter().any(|&r| r > 0.0) {
        let rrect = make_rrect(rect, sr);
        canvas.draw_rrect(rrect, &paint);
    } else {
        canvas.draw_rect(rect, &paint);
    }
}

fn draw_inset_shadow(
    canvas: &Canvas,
    bounds: DisplayRect,
    border_radius: crate::style::BorderRadius,
    shadow: InsetShadow,
) {
    let shadow_bounds = if shadow.spread != 0.0 {
        bounds.outset(shadow.spread, shadow.spread, shadow.spread, shadow.spread)
    } else {
        bounds
    };
    let rect = layout_rect_to_skia(shadow_bounds.translate(shadow.offset_x, shadow.offset_y));
    let sr = spread_radius(border_radius, shadow.spread);
    let radii = effective_corner_radius(rect, sr);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(PaintStyle::Fill);
    paint.set_color(skia_color(shadow.color));

    if let Some(mask_filter) = MaskFilter::blur(BlurStyle::Inner, shadow.blur_sigma, false) {
        paint.set_mask_filter(mask_filter);
    }

    canvas.save();
    clip_bounds(canvas, bounds, border_radius);
    if radii.iter().any(|&r| r > 0.0) {
        let rrect = make_rrect(rect, sr);
        canvas.draw_rrect(rrect, &paint);
    } else {
        canvas.draw_rect(rect, &paint);
    }
    canvas.restore();
}

fn draw_item_drop_shadow(
    canvas: &Canvas,
    bounds: DisplayRect,
    shadow: DropShadow,
    draw: impl FnOnce(&Canvas) -> Result<()>,
) -> Result<()> {
    let (left, top, right, bottom) = shadow.outsets();
    let shadow_bounds = layout_rect_to_skia(bounds.outset(left, top, right, bottom));
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let shadow_filter = image_filters::drop_shadow_only(
        (shadow.offset_x, shadow.offset_y),
        (shadow.blur_sigma, shadow.blur_sigma),
        color4f_from_token(shadow.color),
        None::<skia_safe::ColorSpace>,
        None::<skia_safe::ImageFilter>,
        None::<skia_safe::image_filters::CropRect>,
    )
    .ok_or_else(|| anyhow!("failed to create drop shadow filter"))?;
    paint.set_image_filter(shadow_filter);
    let layer = SaveLayerRec::default().bounds(&shadow_bounds).paint(&paint);
    canvas.save_layer(&layer);
    let result = draw(canvas);
    canvas.restore();
    result
}

fn draw_text(
    canvas: &Canvas,
    text: &TextDisplayItem,
    text_snapshot_cache: &TextSnapshotCache,
) -> Result<TextDrawStats> {
    let cache_key = text_paint_fingerprint(text);
    if let Some(snapshot) = text_snapshot_cache.borrow_mut().get_cloned(&cache_key) {
        let draw_started = Instant::now();
        canvas.save();
        canvas.translate((text.bounds.x, text.bounds.y));
        canvas.draw_picture(&snapshot, None, None);
        canvas.restore();
        Ok(TextDrawStats {
            snapshot_record_ms: 0.0,
            snapshot_draw_ms: draw_started.elapsed().as_secs_f64() * 1000.0,
            cache_hits: 1,
            cache_misses: 0,
        })
    } else {
        let record_started = Instant::now();
        let snapshot = record_text_snapshot(text)?;
        let snapshot_record_ms = record_started.elapsed().as_secs_f64() * 1000.0;
        text_snapshot_cache
            .borrow_mut()
            .insert(cache_key, snapshot.clone());

        let draw_started = Instant::now();
        canvas.save();
        canvas.translate((text.bounds.x, text.bounds.y));
        canvas.draw_picture(&snapshot, None, None);
        canvas.restore();
        Ok(TextDrawStats {
            snapshot_record_ms,
            snapshot_draw_ms: draw_started.elapsed().as_secs_f64() * 1000.0,
            cache_hits: 0,
            cache_misses: 1,
        })
    }
}

fn should_cache_item_picture(item: &DisplayItem) -> bool {
    matches!(
        item,
        DisplayItem::Bitmap(_) | DisplayItem::DrawScript(_) | DisplayItem::Lucide(_)
    )
}

fn draw_item_picture_cached(
    canvas: &Canvas,
    item: &DisplayItem,
    cache_key: u64,
    assets: &AssetsMap,
    image_cache: &ImageCache,
    text_snapshot_cache: &TextSnapshotCache,
    item_picture_cache: &ItemPictureCache,
    media_ctx: &mut Option<&mut MediaContext>,
    frame_ctx: &FrameCtx,
) -> Result<ItemPictureDrawStats> {
    let visual_bounds = item.visual_bounds();
    if let Some(snapshot) = item_picture_cache.borrow_mut().get_cloned(&cache_key) {
        let draw_started = Instant::now();
        canvas.save();
        canvas.translate((visual_bounds.x, visual_bounds.y));
        canvas.draw_picture(&snapshot, None, None);
        canvas.restore();
        return Ok(ItemPictureDrawStats {
            record_ms: 0.0,
            draw_ms: draw_started.elapsed().as_secs_f64() * 1000.0,
            cache_hits: 1,
            cache_misses: 0,
        });
    }

    let record_started = Instant::now();
    let snapshot = record_item_picture(
        item,
        assets,
        image_cache,
        text_snapshot_cache,
        media_ctx,
        frame_ctx,
    )?;
    let record_ms = record_started.elapsed().as_secs_f64() * 1000.0;
    item_picture_cache
        .borrow_mut()
        .insert(cache_key, snapshot.clone());

    let draw_started = Instant::now();
    canvas.save();
    canvas.translate((visual_bounds.x, visual_bounds.y));
    canvas.draw_picture(&snapshot, None, None);
    canvas.restore();
    Ok(ItemPictureDrawStats {
        record_ms,
        draw_ms: draw_started.elapsed().as_secs_f64() * 1000.0,
        cache_hits: 0,
        cache_misses: 1,
    })
}

fn record_item_picture(
    item: &DisplayItem,
    assets: &AssetsMap,
    image_cache: &ImageCache,
    text_snapshot_cache: &TextSnapshotCache,
    media_ctx: &mut Option<&mut MediaContext>,
    frame_ctx: &FrameCtx,
) -> Result<Picture> {
    let visual_bounds = item.visual_bounds();
    let bounds = Rect::from_xywh(
        0.0,
        0.0,
        visual_bounds.width.max(1.0),
        visual_bounds.height.max(1.0),
    );
    let mut recorder = PictureRecorder::new();
    let recording_canvas = recorder.begin_recording(bounds, false);
    recording_canvas.translate((-visual_bounds.x, -visual_bounds.y));
    draw_display_item_direct(
        recording_canvas,
        item,
        assets,
        image_cache,
        text_snapshot_cache,
        media_ctx,
        frame_ctx,
    )?;
    recorder
        .finish_recording_as_picture(None)
        .ok_or_else(|| anyhow!("failed to record item picture"))
}

fn draw_display_item_direct(
    canvas: &Canvas,
    item: &DisplayItem,
    assets: &AssetsMap,
    image_cache: &ImageCache,
    text_snapshot_cache: &TextSnapshotCache,
    media_ctx: &mut Option<&mut MediaContext>,
    frame_ctx: &FrameCtx,
) -> Result<()> {
    match item {
        DisplayItem::Rect(rect) => {
            if let Some(shadow) = rect.paint.box_shadow {
                draw_box_shadow(canvas, rect.bounds, rect.paint.border_radius, shadow);
            }
            if let Some(shadow) = rect.paint.drop_shadow {
                draw_item_drop_shadow(canvas, rect.bounds, shadow, |canvas| {
                    draw_rect(canvas, rect);
                    Ok(())
                })?;
            }
            draw_rect(canvas, rect);
        }
        DisplayItem::Text(text) => {
            if let Some(shadow) = text.drop_shadow {
                draw_item_drop_shadow(canvas, text.bounds, shadow, |canvas| {
                    draw_text(canvas, text, text_snapshot_cache).map(|_| ())
                })?;
            }
            let _ = draw_text(canvas, text, text_snapshot_cache)?;
        }
        DisplayItem::Bitmap(bitmap) => {
            if let Some(shadow) = bitmap.paint.box_shadow {
                draw_box_shadow(canvas, bitmap.bounds, bitmap.paint.border_radius, shadow);
            }
            if let Some(shadow) = bitmap.paint.drop_shadow {
                draw_item_drop_shadow(canvas, bitmap.bounds, shadow, |canvas| {
                    draw_bitmap(canvas, bitmap, assets, image_cache, media_ctx, frame_ctx)
                        .map(|_| ())
                })?;
            }
            let _ = draw_bitmap(canvas, bitmap, assets, image_cache, media_ctx, frame_ctx)?;
        }
        DisplayItem::DrawScript(script) => {
            if let Some(shadow) = script.drop_shadow {
                draw_item_drop_shadow(canvas, script.bounds, shadow, |canvas| {
                    draw_script_item(canvas, script, assets, image_cache, media_ctx, frame_ctx)
                })?;
            }
            draw_script_item(canvas, script, assets, image_cache, media_ctx, frame_ctx)?;
        }
        DisplayItem::Lucide(lucide) => {
            if let Some(shadow) = lucide.paint.drop_shadow {
                draw_item_drop_shadow(canvas, lucide.bounds, shadow, |canvas| {
                    draw_lucide(canvas, lucide);
                    Ok(())
                })?;
            }
            draw_lucide(canvas, lucide);
        }
    }
    Ok(())
}

fn record_text_snapshot(text: &TextDisplayItem) -> Result<Picture> {
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
        .ok_or_else(|| anyhow!("failed to record text snapshot"))
}

fn draw_bitmap(
    canvas: &Canvas,
    bitmap: &BitmapDisplayItem,
    assets: &AssetsMap,
    image_cache: &ImageCache,
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
        video_frame_cache_hits: 0,
        video_frame_cache_misses: 0,
        video_frame_decodes: 0,
    };

    let image = if bitmap_source_kind(path) == BitmapSourceKind::Video {
        let media = media_ctx
            .as_deref_mut()
            .ok_or_else(|| anyhow!("video asset requires media context: {}", path.display()))?;
        let request = bitmap
            .video_timing
            .map(|timing| VideoFrameRequest {
                composition_time_secs: frame_ctx.frame as f64 / frame_ctx.fps as f64,
                timing,
                quality: media.video_preview_quality(),
            })
            .ok_or_else(|| {
                anyhow!(
                    "video bitmap is missing timing metadata: {}",
                    path.display()
                )
            })?;
        let decode_started = Instant::now();
        let video_bitmap = media
            .get_video_bitmap(path, request)
            .with_context(|| format!("failed to decode video frame: {}", path.display()))?;
        if video_bitmap.frame_cache_hit {
            stats.video_frame_cache_hits = 1;
        } else {
            stats.video_frame_cache_misses = 1;
            stats.video_frame_decodes = 1;
            stats.video_decode_ms = decode_started.elapsed().as_secs_f64() * 1000.0;
        }
        let info = ImageInfo::new(
            (video_bitmap.width as i32, video_bitmap.height as i32),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        images::raster_from_data(
            &info,
            Data::new_copy(&video_bitmap.data),
            video_bitmap.width as usize * 4,
        )
        .ok_or_else(|| {
            anyhow!(
                "failed to create skia image from video frame: {}",
                path.display()
            )
        })?
    } else {
        let key = bitmap.asset_id.0.clone();
        let mut cache = image_cache.borrow_mut();
        if let Some(Some(img)) = cache.get_cloned(&key) {
            stats.image_cache_hits = 1;
            img
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
    let radii = effective_corner_radius(dst, bitmap.paint.border_radius);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    apply_blur_effect(&mut paint, bitmap.paint.blur_sigma);

    let src_width = bitmap.width as f32;
    let src_height = bitmap.height as f32;

    if let Some(color) = bitmap.paint.background {
        let mut background_paint = Paint::default();
        background_paint.set_anti_alias(true);
        apply_blur_effect(&mut background_paint, bitmap.paint.blur_sigma);
        apply_background_paint(&mut background_paint, color, dst);
        if radii.iter().any(|&r| r > 0.0) {
            let rrect = make_rrect(dst, bitmap.paint.border_radius);
            canvas.draw_rrect(rrect, &background_paint);
        } else {
            canvas.draw_rect(dst, &background_paint);
        }
    }

    let needs_clip = radii.iter().any(|&r| r > 0.0);
    if needs_clip {
        let rrect = make_rrect(dst, bitmap.paint.border_radius);
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

    if let Some(shadow) = bitmap.paint.inset_shadow {
        draw_inset_shadow(canvas, bitmap.bounds, bitmap.paint.border_radius, shadow);
    }

    if let (Some(width), Some(color)) = (bitmap.paint.border_width, bitmap.paint.border_color) {
        let mut border_paint = Paint::default();
        border_paint.set_anti_alias(true);
        apply_blur_effect(&mut border_paint, bitmap.paint.blur_sigma);
        border_paint.set_color(skia_color(color));
        border_paint.set_style(PaintStyle::Stroke);
        border_paint.set_stroke_width(width);

        if radii.iter().any(|&r| r > 0.0) {
            let rrect = make_rrect(dst, bitmap.paint.border_radius);
            canvas.draw_rrect(rrect, &border_paint);
        } else {
            canvas.draw_rect(dst, &border_paint);
        }
    }

    stats.draw_ms = draw_started.elapsed().as_secs_f64() * 1000.0;
    Ok(stats)
}

fn draw_script_item(
    canvas: &Canvas,
    item: &DrawScriptDisplayItem,
    assets: &AssetsMap,
    image_cache: &ImageCache,
    media_ctx: &mut Option<&mut MediaContext>,
    frame_ctx: &FrameCtx,
) -> Result<()> {
    let mut state = DrawScriptPaintState::default();
    let mut path = PathBuilder::new();

    canvas.save();
    canvas.clip_rect(layout_rect_to_skia(item.bounds), ClipOp::Intersect, true);

    for command in &item.commands {
        match command {
            CanvasCommand::Save => {
                canvas.save();
            }
            CanvasCommand::SaveLayer { alpha, bounds } => {
                let mut paint = Paint::default();
                paint.set_alpha(
                    (255.0 * (state.global_alpha * *alpha).clamp(0.0, 1.0)).round() as u8,
                );
                let bounds_rect = bounds
                    .map(|bounds| Rect::from_xywh(bounds[0], bounds[1], bounds[2], bounds[3]));
                let layer = if let Some(bounds_rect) = bounds_rect.as_ref() {
                    SaveLayerRec::default().bounds(bounds_rect).paint(&paint)
                } else {
                    SaveLayerRec::default().paint(&paint)
                };
                canvas.save_layer(&layer);
            }
            CanvasCommand::Restore => {
                canvas.restore();
            }
            CanvasCommand::RestoreToCount { count } => {
                canvas.restore_to_count((*count).max(1) as usize);
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
            CanvasCommand::SetAntiAlias { enabled } => {
                state.anti_alias = *enabled;
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
                anti_alias,
            } => {
                canvas.clip_rect(
                    Rect::from_xywh(*x, *y, *width, *height),
                    ClipOp::Intersect,
                    *anti_alias,
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
            CanvasCommand::DrawPaint { color, anti_alias } => {
                let mut paint = Paint::default();
                paint.set_anti_alias(*anti_alias);
                paint.set_style(PaintStyle::Fill);
                paint.set_color(apply_script_alpha(*color, state.global_alpha));
                canvas.draw_paint(&paint);
            }
            CanvasCommand::DrawText {
                text,
                x,
                y,
                color,
                anti_alias,
                stroke,
                stroke_width,
                font_size,
                font_scale_x,
                font_skew_x,
                font_subpixel,
                font_edging,
            } => {
                let mut paint = Paint::default();
                paint.set_anti_alias(*anti_alias);
                paint.set_style(if *stroke {
                    PaintStyle::Stroke
                } else {
                    PaintStyle::Fill
                });
                paint.set_stroke_width((*stroke_width).max(0.0));
                paint.set_color(apply_script_alpha(*color, state.global_alpha));

                let mut font = Font::default();
                if let Some(typeface) = skia_safe::FontMgr::new()
                    .legacy_make_typeface(None, skia_safe::FontStyle::normal())
                {
                    font.set_typeface(typeface);
                }
                font.set_size((*font_size).max(1.0));
                font.set_scale_x(*font_scale_x);
                font.set_skew_x(*font_skew_x);
                font.set_subpixel(*font_subpixel);
                font.set_edging(match font_edging {
                    ScriptFontEdging::Alias => skia_safe::font::Edging::Alias,
                    ScriptFontEdging::AntiAlias => skia_safe::font::Edging::AntiAlias,
                    ScriptFontEdging::SubpixelAntiAlias => {
                        skia_safe::font::Edging::SubpixelAntiAlias
                    }
                });

                canvas.draw_str(text, (*x, *y), &font, &paint);
            }
            CanvasCommand::FillRect {
                x,
                y,
                width,
                height,
                color,
            } => {
                let mut paint = fill_paint_for_draw_script(&state);
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
                let paint = fill_paint_for_draw_script(&state);
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
                let mut paint = stroke_paint_for_draw_script(&state);
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
                let paint = stroke_paint_for_draw_script(&state);
                let rect = Rect::from_xywh(*x, *y, *width, *height);
                let rrect = RRect::new_rect_xy(rect, *radius, *radius);
                canvas.draw_rrect(rrect, &paint);
            }
            CanvasCommand::DrawLine { x0, y0, x1, y1 } => {
                let paint = stroke_paint_for_draw_script(&state);
                canvas.draw_line((*x0, *y0), (*x1, *y1), &paint);
            }
            CanvasCommand::FillCircle { cx, cy, radius } => {
                let paint = fill_paint_for_draw_script(&state);
                canvas.draw_circle((*cx, *cy), *radius, &paint);
            }
            CanvasCommand::StrokeCircle { cx, cy, radius } => {
                let paint = stroke_paint_for_draw_script(&state);
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
            CanvasCommand::AddRectPath {
                x,
                y,
                width,
                height,
            } => {
                path.add_rect(
                    Rect::from_xywh(*x, *y, *width, *height),
                    None::<skia_safe::PathDirection>,
                    None::<usize>,
                );
            }
            CanvasCommand::AddRRectPath {
                x,
                y,
                width,
                height,
                radius,
            } => {
                path.add_rrect(
                    RRect::new_rect_xy(Rect::from_xywh(*x, *y, *width, *height), *radius, *radius),
                    None::<skia_safe::PathDirection>,
                    None::<usize>,
                );
            }
            CanvasCommand::AddOvalPath {
                x,
                y,
                width,
                height,
            } => {
                path.add_oval(
                    Rect::from_xywh(*x, *y, *width, *height),
                    None::<skia_safe::PathDirection>,
                    None::<usize>,
                );
            }
            CanvasCommand::AddArcPath {
                x,
                y,
                width,
                height,
                start_angle,
                sweep_angle,
            } => {
                path.add_arc(
                    Rect::from_xywh(*x, *y, *width, *height),
                    *start_angle,
                    *sweep_angle,
                );
            }
            CanvasCommand::FillPath => {
                let paint = fill_paint_for_draw_script(&state);
                let path_snapshot = path.snapshot();
                canvas.draw_path(&path_snapshot, &paint);
            }
            CanvasCommand::StrokePath => {
                let paint = stroke_paint_for_draw_script(&state);
                let path_snapshot = path.snapshot();
                canvas.draw_path(&path_snapshot, &paint);
            }
            CanvasCommand::DrawImage {
                asset_id,
                x,
                y,
                width,
                height,
                src_rect,
                alpha,
                anti_alias,
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
                let mut paint = Paint::default();
                paint.set_anti_alias(*anti_alias);
                paint.set_alpha(
                    (255.0 * (state.global_alpha * *alpha).clamp(0.0, 1.0)).round() as u8,
                );
                if let Some(src_rect) = src_rect {
                    let src = Rect::from_xywh(src_rect[0], src_rect[1], src_rect[2], src_rect[3]);
                    canvas.draw_image_rect(
                        image,
                        Some((&src, SrcRectConstraint::Strict)),
                        dst,
                        &paint,
                    );
                } else {
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
            CanvasCommand::DrawArc {
                cx,
                cy,
                rx,
                ry,
                start_angle,
                sweep_angle,
                use_center,
            } => {
                let paint = fill_paint_for_draw_script(&state);
                let rect = Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0);
                let mut builder = PathBuilder::new();
                if *use_center {
                    builder.move_to((*cx, *cy));
                    builder.arc_to(rect, *start_angle, *sweep_angle, false);
                    builder.close();
                } else {
                    builder.arc_to(rect, *start_angle, *sweep_angle, false);
                }
                let arc_path = builder.snapshot();
                canvas.draw_path(&arc_path, &paint);
            }
            CanvasCommand::StrokeArc {
                cx,
                cy,
                rx,
                ry,
                start_angle,
                sweep_angle,
            } => {
                let paint = stroke_paint_for_draw_script(&state);
                let rect = Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0);
                let mut builder = PathBuilder::new();
                builder.arc_to(rect, *start_angle, *sweep_angle, false);
                let arc_path = builder.snapshot();
                canvas.draw_path(&arc_path, &paint);
            }
            CanvasCommand::FillOval { cx, cy, rx, ry } => {
                let paint = fill_paint_for_draw_script(&state);
                let rect = Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0);
                canvas.draw_oval(rect, &paint);
            }
            CanvasCommand::StrokeOval { cx, cy, rx, ry } => {
                let paint = stroke_paint_for_draw_script(&state);
                let rect = Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0);
                canvas.draw_oval(rect, &paint);
            }
            CanvasCommand::ClipPath { anti_alias } => {
                let clip_path = path.snapshot();
                canvas.clip_path(&clip_path, ClipOp::Intersect, *anti_alias);
                // Reset path builder after clip so it doesn't interfere with subsequent path ops
                path = PathBuilder::new();
            }
            CanvasCommand::ClipRRect {
                x,
                y,
                width,
                height,
                radius,
                anti_alias,
            } => {
                let rect = Rect::from_xywh(*x, *y, *width, *height);
                let rrect = RRect::new_rect_xy(rect, *radius, *radius);
                canvas.clip_rrect(rrect, ClipOp::Intersect, *anti_alias);
            }
            CanvasCommand::DrawPoints { mode, points } => {
                let paint = stroke_paint_for_draw_script(&state);
                let pts: Vec<(f32, f32)> = points
                    .chunks_exact(2)
                    .map(|chunk| (chunk[0], chunk[1]))
                    .collect();
                match mode {
                    ScriptPointMode::Points => {
                        for &(x, y) in &pts {
                            canvas.draw_circle((x, y), paint.stroke_width() / 2.0, &paint);
                        }
                    }
                    ScriptPointMode::Lines => {
                        for chunk in pts.chunks_exact(2) {
                            canvas.draw_line(
                                (chunk[0].0, chunk[0].1),
                                (chunk[1].0, chunk[1].1),
                                &paint,
                            );
                        }
                    }
                    ScriptPointMode::Polygon => {
                        if pts.len() >= 2 {
                            let mut pb = PathBuilder::new();
                            pb.move_to(pts[0]);
                            for &pt in &pts[1..] {
                                pb.line_to(pt);
                            }
                            pb.close();
                            let poly_path = pb.snapshot();
                            canvas.draw_path(&poly_path, &paint);
                        }
                    }
                }
            }
            CanvasCommand::FillDRRect {
                outer_x,
                outer_y,
                outer_width,
                outer_height,
                outer_radius,
                inner_x,
                inner_y,
                inner_width,
                inner_height,
                inner_radius,
            } => {
                let paint = fill_paint_for_draw_script(&state);
                let outer_rect = Rect::from_xywh(*outer_x, *outer_y, *outer_width, *outer_height);
                let outer = RRect::new_rect_xy(outer_rect, *outer_radius, *outer_radius);
                let inner_rect = Rect::from_xywh(*inner_x, *inner_y, *inner_width, *inner_height);
                let inner = RRect::new_rect_xy(inner_rect, *inner_radius, *inner_radius);
                canvas.draw_drrect(outer, inner, &paint);
            }
            CanvasCommand::StrokeDRRect {
                outer_x,
                outer_y,
                outer_width,
                outer_height,
                outer_radius,
                inner_x,
                inner_y,
                inner_width,
                inner_height,
                inner_radius,
            } => {
                let paint = stroke_paint_for_draw_script(&state);
                let outer_rect = Rect::from_xywh(*outer_x, *outer_y, *outer_width, *outer_height);
                let outer = RRect::new_rect_xy(outer_rect, *outer_radius, *outer_radius);
                let inner_rect = Rect::from_xywh(*inner_x, *inner_y, *inner_width, *inner_height);
                let inner = RRect::new_rect_xy(inner_rect, *inner_radius, *inner_radius);
                canvas.draw_drrect(outer, inner, &paint);
            }
            CanvasCommand::Skew { sx, sy } => {
                let matrix = skia_safe::Matrix::skew((*sx, *sy));
                canvas.concat(&matrix);
            }
            CanvasCommand::DrawImageSimple {
                asset_id,
                x,
                y,
                alpha,
                anti_alias,
            } => {
                let image = load_asset_image(
                    &crate::resource::assets::AssetId(asset_id.clone()),
                    assets,
                    image_cache,
                    media_ctx,
                    frame_ctx,
                )?;
                let mut paint = Paint::default();
                paint.set_anti_alias(*anti_alias);
                paint.set_alpha(
                    (255.0 * (state.global_alpha * *alpha).clamp(0.0, 1.0)).round() as u8,
                );
                canvas.draw_image(image, (*x, *y), Some(&paint));
            }
            CanvasCommand::Concat { matrix } => {
                let m = skia_safe::Matrix::new_all(
                    matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5], matrix[6],
                    matrix[7], matrix[8],
                );
                canvas.concat(&m);
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
    let color = script_color(color);
    skia_safe::Color::from_argb(alpha, color.r(), color.g(), color.b())
}

fn fill_paint_for_draw_script(state: &DrawScriptPaintState) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(state.anti_alias);
    paint.set_style(PaintStyle::Fill);
    paint.set_color(apply_script_alpha(state.fill_color, state.global_alpha));
    paint
}

fn stroke_paint_for_draw_script(state: &DrawScriptPaintState) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(state.anti_alias);
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
    image_cache: &ImageCache,
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
        let request = VideoFrameRequest {
            composition_time_secs: frame_ctx.frame as f64 / frame_ctx.fps as f64,
            timing: crate::resource::media::VideoFrameTiming::default(),
            quality: media.video_preview_quality(),
        };
        let video_bitmap = media
            .get_video_bitmap(path, request)
            .with_context(|| format!("failed to decode video frame: {}", path.display()))?;
        let info = ImageInfo::new(
            (video_bitmap.width as i32, video_bitmap.height as i32),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        return images::raster_from_data(
            &info,
            Data::new_copy(&video_bitmap.data),
            video_bitmap.width as usize * 4,
        )
        .ok_or_else(|| {
            anyhow!(
                "failed to create skia image from video frame: {}",
                path.display()
            )
        });
    }

    let key = asset_id.0.clone();
    let mut cache = image_cache.borrow_mut();
    if let Some(Some(img)) = cache.get_cloned(&key) {
        return Ok(img);
    }

    let encoded = std::fs::read(path)
        .with_context(|| format!("failed to read image asset: {}", path.display()))?;
    let data = skia_safe::Data::new_copy(&encoded);
    let image = skia_safe::Image::from_encoded(data)
        .ok_or_else(|| anyhow!("failed to decode image asset: {}", path.display()))?;
    cache.insert(key, Some(image.clone()));
    Ok(image)
}

fn effective_corner_radius(rect: Rect, radius: crate::style::BorderRadius) -> [f32; 4] {
    let clamp = |r: f32| {
        if r <= 0.0 {
            return 0.0;
        }
        r.min(rect.width() / 2.0).min(rect.height() / 2.0)
    };
    [
        clamp(radius.top_left),
        clamp(radius.top_right),
        clamp(radius.bottom_right),
        clamp(radius.bottom_left),
    ]
}

fn make_rrect(rect: Rect, radius: crate::style::BorderRadius) -> RRect {
    let radii = effective_corner_radius(rect, radius);
    let points = [
        Point::from((radii[0], radii[0])),
        Point::from((radii[1], radii[1])),
        Point::from((radii[2], radii[2])),
        Point::from((radii[3], radii[3])),
    ];
    RRect::new_rect_radii(rect, &points)
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
        paint.set_color(skia_color(stroke_color));
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
            paint.set_color(skia_color(color));
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
                Some(via) => vec![skia_color(from), skia_color(via), skia_color(to)],
                None => vec![skia_color(from), skia_color(to)],
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
                paint.set_color(skia_color(from));
            }
        }
    }
}

fn color4f_from_token(token: crate::style::ColorToken) -> Color4f {
    let (r, g, b, a) = token.rgba();
    Color4f::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    )
}

fn clip_bounds(canvas: &Canvas, bounds: DisplayRect, border_radius: crate::style::BorderRadius) {
    let rect = layout_rect_to_skia(bounds);
    let radii = effective_corner_radius(rect, border_radius);
    if radii.iter().any(|&r| r > 0.0) {
        let rrect = make_rrect(rect, border_radius);
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
