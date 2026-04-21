use anyhow::{Context, Result, anyhow};
use skia_safe::{
    BlurStyle, Canvas, ClipOp, Color4f, Data, Font, Image as SkiaImage, ImageInfo, MaskFilter,
    Paint, PaintStyle, PathBuilder, Picture, PictureRecorder, Point, RRect, Rect, TileMode,
    canvas::{SaveLayerRec, SrcRectConstraint},
    gradient_shader, image_filters, images,
};
use tracing::{Level, event, span};

use crate::{
    display::list::{
        BitmapDisplayItem, DisplayItem, DisplayRect, DisplayTransform, DrawScriptDisplayItem,
        LucideDisplayItem, RectDisplayItem, TextDisplayItem, TimelineDisplayItem,
    },
    frame_ctx::FrameCtx,
    resource::{
        assets::AssetsMap,
        bitmap_source::{BitmapSourceKind, bitmap_source_kind},
        media::{MediaContext, VideoFrameRequest},
    },
    runtime::cache::{
        CachedSubtreeImage, CachedSubtreeSnapshot, ImageCache, ItemPictureCache, SubtreeImageCache,
        SubtreeSnapshotCache, TextSnapshotCache,
    },
    runtime::{
        annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle, RecordedNodeSemantics},
        compositor::{LiveNodeItemExecution, OrderedSceneOp, OrderedSceneProgram},
        fingerprint::{
            SubtreeSnapshotFingerprint, item_paint_fingerprint,
            subtree_has_dirty_descendant_composite, text_paint_fingerprint,
        },
    },
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
    image_cache_hits: usize,
    image_cache_misses: usize,
    video_frame_cache_hits: usize,
    video_frame_cache_misses: usize,
    video_frame_decodes: usize,
}

struct TextDrawStats {
    cache_hits: usize,
    cache_misses: usize,
}

struct ItemPictureDrawStats {
    cache_hits: usize,
    cache_misses: usize,
}

#[derive(Clone, Copy)]
struct TextSnapshotPlacement {
    record_bounds: DisplayRect,
    draw_translation_x: f32,
    draw_translation_y: f32,
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
    display_tree: &'a AnnotatedDisplayTree,
    assets: &'a AssetsMap,
    media_ctx: Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
    image_cache: ImageCache,
    text_snapshot_cache: TextSnapshotCache,
    item_picture_cache: ItemPictureCache,
    subtree_snapshot_cache: Option<SubtreeSnapshotCache>,
    subtree_image_cache: Option<SubtreeImageCache>,
}

impl<'a> SkiaBackend<'a> {
    pub fn new_with_cache(
        canvas: &'a Canvas,
        _width: i32,
        _height: i32,
        display_tree: &'a AnnotatedDisplayTree,
        assets: &'a AssetsMap,
        image_cache: ImageCache,
        text_snapshot_cache: TextSnapshotCache,
        item_picture_cache: ItemPictureCache,
        subtree_snapshot_cache: Option<SubtreeSnapshotCache>,
        subtree_image_cache: Option<SubtreeImageCache>,
        media_ctx: Option<&'a mut MediaContext>,
        frame_ctx: &'a FrameCtx,
    ) -> Self {
        Self {
            canvas,
            display_tree,
            assets,
            image_cache,
            text_snapshot_cache,
            item_picture_cache,
            subtree_snapshot_cache,
            subtree_image_cache,
            media_ctx,
            frame_ctx,
        }
    }

    fn node_snapshot_fingerprint(
        &self,
        handle: AnnotatedNodeHandle,
    ) -> Option<SubtreeSnapshotFingerprint> {
        self.display_tree.analysis(handle).snapshot_fingerprint
    }

    /// 诊断：判断一次 subtree_snapshot 查询的子树，是否含有 composite 跨帧变化的后代。
    /// 读 `DisplayInvalidationTable`——`composite_dirty` 由 `mark_display_tree_composite_dirty`
    /// 在 pipeline 前段比较前后帧 `CompositeSig` 得出。精准区分"恒定非零"与"每帧抖动"。
    fn subtree_descendants_have_dirty_composite(&self, handle: AnnotatedNodeHandle) -> bool {
        subtree_has_dirty_descendant_composite(
            self.display_tree.node(handle),
            &self.display_tree.nodes,
            &self.display_tree.invalidation,
        )
    }

    fn draw_display_children(
        &mut self,
        children: &[AnnotatedNodeHandle],
        ancestor_has_non_unit_scale: bool,
    ) -> Result<()> {
        for &child_handle in children {
            self.draw_display_subtree(child_handle, ancestor_has_non_unit_scale)?;
        }
        Ok(())
    }

    fn draw_display_subtree(
        &mut self,
        handle: AnnotatedNodeHandle,
        ancestor_has_non_unit_scale: bool,
    ) -> Result<()> {
        let display_tree = self.display_tree;
        let node = display_tree.node(handle);
        let draw = node.draw_composite_semantics();
        if draw.opacity <= 0.0 {
            return Ok(());
        }

        let has_non_unit_scale = ancestor_has_non_unit_scale
            || transform_list_has_non_unit_scale(&draw.transform.transforms);
        self.canvas.save();
        apply_transform(self.canvas, draw.transform);
        let result = self.draw_display_subtree_after_transform(
            handle,
            draw.opacity,
            draw.backdrop_blur_sigma,
            display_tree.layer_bounds(handle),
            has_non_unit_scale,
        );
        self.canvas.restore();
        result
    }

    fn draw_display_subtree_after_transform(
        &mut self,
        handle: AnnotatedNodeHandle,
        opacity: f32,
        backdrop_blur_sigma: Option<f32>,
        bounds: DisplayRect,
        has_non_unit_scale: bool,
    ) -> Result<()> {
        let subtree_cache = self.subtree_snapshot_cache.clone();
        if let Some(cache) = subtree_cache
            && let Some(fingerprint) = self.node_snapshot_fingerprint(handle)
        {
            let lookup = {
                let mut cache_ref = cache.borrow_mut();
                let cached = cache_ref.get_cloned(&fingerprint.primary);
                let resolution = resolve_subtree_snapshot_lookup(
                    fingerprint,
                    cached.as_ref().map(|entry| entry.secondary_fingerprint),
                );
                (resolution, cached)
            };

            match lookup {
                (SubtreeSnapshotResolution::Hit, Some(entry)) => {
                    event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_snapshot", result = "hit", amount = 1_u64);
                    if self.subtree_descendants_have_dirty_composite(handle) {
                        event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_snapshot_composite_dirty", result = "hit", amount = 1_u64);
                    }

                    // Image cache hit path
                    let cached_image = self
                        .subtree_image_cache
                        .as_ref()
                        .and_then(|ic| ic.borrow_mut().get_cloned(&fingerprint.primary))
                        .filter(|_| !has_non_unit_scale);
                    if let Some(cached) = cached_image {
                        event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_image", result = "hit", amount = 1_u64);
                        return self.draw_subtree_image(
                            &cached.image,
                            opacity,
                            backdrop_blur_sigma,
                            bounds,
                        );
                    }
                    event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_image", result = "miss", amount = 1_u64);

                    // Increment consecutive hits and resolve render mode
                    let consecutive_hits = entry.consecutive_hits + 1;
                    let render_mode = resolve_cached_subtree_render_mode(
                        self.subtree_image_cache
                            .as_ref()
                            .and_then(|ic| ic.borrow_mut().get_cloned(&fingerprint.primary))
                            .is_some(),
                        consecutive_hits,
                        entry.recorded_bounds,
                        bounds,
                        has_non_unit_scale,
                    );

                    // Update consecutive_hits in cache
                    let report = cache.borrow_mut().insert(
                        fingerprint.primary,
                        CachedSubtreeSnapshot {
                            picture: entry.picture.clone(),
                            secondary_fingerprint: entry.secondary_fingerprint,
                            consecutive_hits,
                            recorded_bounds: entry.recorded_bounds,
                        },
                    );
                    record_cache_pressure("subtree_snapshot", &report);

                    match render_mode {
                        CachedSubtreeRenderMode::DrawImage => {
                            // Should not happen since we already checked for image above and
                            // didn't find one, but handle gracefully
                            return self.draw_subtree_snapshot(
                                &entry.picture,
                                opacity,
                                backdrop_blur_sigma,
                                bounds,
                            );
                        }
                        CachedSubtreeRenderMode::PromoteToImage => {
                            let image = record_subtree_snapshot_image(
                                &entry.picture,
                                entry.recorded_bounds,
                            )?;
                            if let Some(image_cache) = &self.subtree_image_cache {
                                let report = image_cache.borrow_mut().insert(
                                    fingerprint.primary,
                                    CachedSubtreeImage {
                                        image: image.clone(),
                                        recorded_bounds: entry.recorded_bounds,
                                    },
                                );
                                record_cache_pressure("subtree_image", &report);
                            }
                            event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_image", result = "promote", amount = 1_u64);
                            return self.draw_subtree_image(
                                &image,
                                opacity,
                                backdrop_blur_sigma,
                                bounds,
                            );
                        }
                        CachedSubtreeRenderMode::DrawPicture => {
                            return self.draw_subtree_snapshot(
                                &entry.picture,
                                opacity,
                                backdrop_blur_sigma,
                                bounds,
                            );
                        }
                    }
                }
                (SubtreeSnapshotResolution::CollisionRejected, _) => {
                    event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_snapshot", result = "collision_rejected", amount = 1_u64);
                }
                (SubtreeSnapshotResolution::Miss, _) => {}
                (SubtreeSnapshotResolution::Hit, None) => {
                    unreachable!("Hit resolution requires cached entry")
                }
            }

            let picture = self.record_cached_subtree_snapshot(handle)?;
            let report = cache.borrow_mut().insert(
                fingerprint.primary,
                CachedSubtreeSnapshot {
                    picture: picture.clone(),
                    secondary_fingerprint: fingerprint.secondary,
                    consecutive_hits: 0,
                    recorded_bounds: bounds,
                },
            );
            record_cache_pressure("subtree_snapshot", &report);
            event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_snapshot", result = "miss", amount = 1_u64);
            if self.subtree_descendants_have_dirty_composite(handle) {
                event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_snapshot_composite_dirty", result = "miss", amount = 1_u64);
            }
            return self.draw_subtree_snapshot(&picture, opacity, backdrop_blur_sigma, bounds);
        }

        self.draw_display_subtree_contents(handle, opacity, backdrop_blur_sigma, bounds)
    }

    fn draw_display_subtree_contents(
        &mut self,
        handle: AnnotatedNodeHandle,
        opacity: f32,
        backdrop_blur_sigma: Option<f32>,
        bounds: DisplayRect,
    ) -> Result<()> {
        let display_tree = self.display_tree;
        let node = display_tree.node(handle);
        self.with_display_layer(opacity, backdrop_blur_sigma, bounds, |backend| {
            backend.draw_recorded_node_contents(node.recorded_semantics(), |backend| {
                backend.draw_display_children(display_tree.children(handle), false)
            })
        })
    }

    fn draw_ordered_scene(&mut self, scene: &OrderedSceneProgram) -> Result<()> {
        self.draw_ordered_scene_op(&scene.root, false)
    }

    fn draw_ordered_scene_op(
        &mut self,
        op: &OrderedSceneOp,
        ancestor_has_non_unit_scale: bool,
    ) -> Result<()> {
        match op {
            OrderedSceneOp::CachedSubtree { handle } => {
                self.draw_display_subtree(*handle, ancestor_has_non_unit_scale)
            }
            OrderedSceneOp::LiveSubtree {
                handle,
                item_execution,
                children,
            } => self.draw_live_ordered_subtree(
                *handle,
                *item_execution,
                children,
                ancestor_has_non_unit_scale,
            ),
        }
    }

    fn draw_live_ordered_subtree(
        &mut self,
        handle: AnnotatedNodeHandle,
        item_execution: LiveNodeItemExecution,
        children: &[OrderedSceneOp],
        ancestor_has_non_unit_scale: bool,
    ) -> Result<()> {
        let display_tree = self.display_tree;
        let node = display_tree.node(handle);
        let draw = node.draw_composite_semantics();
        if draw.opacity <= 0.0 {
            return Ok(());
        }

        let has_non_unit_scale = ancestor_has_non_unit_scale
            || transform_list_has_non_unit_scale(&draw.transform.transforms);
        self.canvas.save();
        apply_transform(self.canvas, draw.transform);
        let result = self.draw_live_ordered_subtree_after_transform(
            handle,
            item_execution,
            draw.opacity,
            draw.backdrop_blur_sigma,
            display_tree.layer_bounds(handle),
            children,
            has_non_unit_scale,
        );
        self.canvas.restore();
        result
    }

    fn draw_live_ordered_subtree_after_transform(
        &mut self,
        handle: AnnotatedNodeHandle,
        item_execution: LiveNodeItemExecution,
        opacity: f32,
        backdrop_blur_sigma: Option<f32>,
        bounds: DisplayRect,
        children: &[OrderedSceneOp],
        has_non_unit_scale: bool,
    ) -> Result<()> {
        let display_tree = self.display_tree;
        let node = display_tree.node(handle);
        if let DisplayItem::Timeline(timeline) = &node.item
            && timeline.transition.is_some()
        {
            return self.draw_timeline_transition_subtree(
                node.recorded_semantics(),
                timeline,
                opacity,
                backdrop_blur_sigma,
                bounds,
                children,
            );
        }

        self.with_display_layer(opacity, backdrop_blur_sigma, bounds, |backend| {
            backend.draw_display_item_with_execution(&node.item, item_execution)?;
            backend.with_recorded_clip(node.recorded_semantics(), |backend| {
                for child in children {
                    backend.draw_ordered_scene_op(child, has_non_unit_scale)?;
                }
                Ok(())
            })
        })
    }

    fn draw_display_item_with_execution(
        &mut self,
        item: &DisplayItem,
        execution: LiveNodeItemExecution,
    ) -> Result<()> {
        match execution {
            LiveNodeItemExecution::Direct => self.draw_display_item(item),
            LiveNodeItemExecution::FrameLocalPicture => {
                self.draw_display_item_frame_local_picture(item)
            }
        }
    }

    fn draw_display_item(&mut self, item: &DisplayItem) -> Result<()> {
        let profile_span = span!(target: "render.backend", Level::TRACE, "draw_item");
        let _profile_span = profile_span.enter();
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
            event!(target: "render.cache", Level::TRACE, kind = "cache", name = "item_picture", result = "hit", amount = stats.cache_hits as u64);
            event!(target: "render.cache", Level::TRACE, kind = "cache", name = "item_picture", result = "miss", amount = stats.cache_misses as u64);
            match item {
                DisplayItem::Bitmap(_) => {
                    event!(target: "render.draw", Level::TRACE, kind = "draw", name = "bitmap", result = "count", amount = 1_u64);
                }
                DisplayItem::DrawScript(_) => {
                    event!(target: "render.draw", Level::TRACE, kind = "draw", name = "script", result = "count", amount = 1_u64);
                }
                DisplayItem::Lucide(_) => {}
                DisplayItem::Rect(_) | DisplayItem::Timeline(_) | DisplayItem::Text(_) => {}
            }
            return Ok(());
        }

        self.draw_display_item_uncached(item)
    }

    fn draw_display_item_frame_local_picture(&mut self, item: &DisplayItem) -> Result<()> {
        let profile_span =
            span!(target: "render.backend", Level::TRACE, "draw_item_frame_local_picture");
        let _profile_span = profile_span.enter();
        let snapshot = record_item_picture(
            item,
            self.assets,
            &self.image_cache,
            &self.text_snapshot_cache,
            &mut self.media_ctx,
            self.frame_ctx,
        )?;

        let semantics = item.picture_semantics();
        self.canvas.save();
        self.canvas
            .translate((semantics.draw_translation_x, semantics.draw_translation_y));
        self.canvas.draw_picture(&snapshot, None, None);
        self.canvas.restore();

        if matches!(item, DisplayItem::DrawScript(_)) {
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "script", result = "count", amount = 1_u64);
        }

        Ok(())
    }

    fn draw_display_item_uncached(&mut self, item: &DisplayItem) -> Result<()> {
        match item {
            DisplayItem::Rect(rect) => {
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
                event!(target: "render.draw", Level::TRACE, kind = "draw", name = "rect", result = "count", amount = 1_u64);
            }
            DisplayItem::Timeline(timeline) => {
                draw_timeline_base(self.canvas, timeline)?;
                event!(target: "render.draw", Level::TRACE, kind = "draw", name = "timeline", result = "count", amount = 1_u64);
            }
            DisplayItem::Text(text) => {
                if let Some(shadow) = text.drop_shadow {
                    draw_item_drop_shadow(self.canvas, text.bounds, shadow, |canvas| {
                        draw_text(canvas, text, &self.text_snapshot_cache).map(|_| ())
                    })?;
                }
                let stats = draw_text(self.canvas, text, &self.text_snapshot_cache)?;
                event!(target: "render.draw", Level::TRACE, kind = "draw", name = "text", result = "count", amount = 1_u64);
                event!(target: "render.cache", Level::TRACE, kind = "cache", name = "text", result = "hit", amount = stats.cache_hits as u64);
                event!(target: "render.cache", Level::TRACE, kind = "cache", name = "text", result = "miss", amount = stats.cache_misses as u64);
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
                event!(target: "render.draw", Level::TRACE, kind = "draw", name = "bitmap", result = "count", amount = 1_u64);
                event!(target: "render.cache", Level::TRACE, kind = "cache", name = "image", result = "hit", amount = stats.image_cache_hits as u64);
                event!(target: "render.cache", Level::TRACE, kind = "cache", name = "image", result = "miss", amount = stats.image_cache_misses as u64);
                event!(target: "render.cache", Level::TRACE, kind = "cache", name = "video_frame", result = "hit", amount = stats.video_frame_cache_hits as u64);
                event!(target: "render.cache", Level::TRACE, kind = "cache", name = "video_frame", result = "miss", amount = stats.video_frame_cache_misses as u64);
                event!(target: "render.cache", Level::TRACE, kind = "cache", name = "video_frame", result = "decode", amount = stats.video_frame_decodes as u64);
            }
            DisplayItem::DrawScript(script) => {
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
                event!(target: "render.draw", Level::TRACE, kind = "draw", name = "script", result = "count", amount = 1_u64);
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

    fn record_cached_subtree_snapshot(&mut self, handle: AnnotatedNodeHandle) -> Result<Picture> {
        let profile_span = span!(target: "render.backend", Level::TRACE, "subtree_snapshot_record");
        let _profile_span = profile_span.enter();
        let display_tree = self.display_tree;
        let node = display_tree.node(handle);
        let layer_bounds = display_tree.layer_bounds(handle);
        let bounds = layout_rect_to_skia(layer_bounds);
        let mut recorder = PictureRecorder::new();
        let recording_canvas = recorder.begin_recording(bounds, false);
        let mut backend = SkiaBackend::new_with_cache(
            recording_canvas,
            layer_bounds.width.max(1.0) as i32,
            layer_bounds.height.max(1.0) as i32,
            self.display_tree,
            self.assets,
            self.image_cache.clone(),
            self.text_snapshot_cache.clone(),
            self.item_picture_cache.clone(),
            self.subtree_snapshot_cache.clone(),
            self.subtree_image_cache.clone(),
            self.media_ctx.as_deref_mut(),
            self.frame_ctx,
        );
        let subtree = OrderedSceneProgram::build_subtree(display_tree, handle);
        backend.draw_display_item_with_execution(
            node.recorded_semantics().item,
            subtree.item_execution,
        )?;
        backend.with_recorded_clip(node.recorded_semantics(), |backend| {
            for child in &subtree.children {
                backend.draw_ordered_scene_op(child, false)?;
            }
            Ok(())
        })?;
        let snapshot = recorder
            .finish_recording_as_picture(None)
            .ok_or_else(|| anyhow!("failed to record subtree snapshot"))?;
        Ok(snapshot)
    }

    /// 为 Timeline 转场合成录制一张独立的 from/to 子场景 `Picture`。
    ///
    /// 转场外壳的 `progress` 每帧变化，是 TimeVariant，**不进缓存**；
    /// 而 from/to 子场景的 `frame_ctx` 在转场段内被 `frozen_script_frame_ctx`
    /// 冻结，`snapshot_fingerprint` 跨帧稳定，可以命中 `SubtreeSnapshotCache`。
    ///
    /// 快路径：命中缓存时直接 clone 缓存里的 `Picture`，跳过一次 `PictureRecorder`
    /// 的新建与 `finish_recording`。慢路径：录制后写入缓存，让后续转场帧复用。
    fn acquire_transition_scene_picture(
        &mut self,
        handle: AnnotatedNodeHandle,
    ) -> Result<Picture> {
        let fingerprint = self.node_snapshot_fingerprint(handle);
        let cache = self.subtree_snapshot_cache.clone();

        if let (Some(cache), Some(fingerprint)) = (cache.as_ref(), fingerprint) {
            let cached = cache.borrow_mut().get_cloned(&fingerprint.primary);
            match resolve_subtree_snapshot_lookup(
                fingerprint,
                cached.as_ref().map(|entry| entry.secondary_fingerprint),
            ) {
                SubtreeSnapshotResolution::Hit => {
                    let entry = cached.expect("Hit resolution requires cached entry");
                    let report = cache.borrow_mut().insert(
                        fingerprint.primary,
                        CachedSubtreeSnapshot {
                            picture: entry.picture.clone(),
                            secondary_fingerprint: entry.secondary_fingerprint,
                            consecutive_hits: entry.consecutive_hits + 1,
                            recorded_bounds: entry.recorded_bounds,
                        },
                    );
                    record_cache_pressure("subtree_snapshot", &report);
                    event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_snapshot", result = "hit", amount = 1_u64);
                    if self.subtree_descendants_have_dirty_composite(handle) {
                        event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_snapshot_composite_dirty", result = "hit", amount = 1_u64);
                    }
                    return Ok(entry.picture);
                }
                SubtreeSnapshotResolution::CollisionRejected => {
                    event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_snapshot", result = "collision_rejected", amount = 1_u64);
                }
                SubtreeSnapshotResolution::Miss => {}
            }
        }

        let picture = self.record_cached_subtree_snapshot(handle)?;

        if let (Some(cache), Some(fingerprint)) = (cache.as_ref(), fingerprint) {
            let bounds = self.display_tree.layer_bounds(handle);
            let report = cache.borrow_mut().insert(
                fingerprint.primary,
                CachedSubtreeSnapshot {
                    picture: picture.clone(),
                    secondary_fingerprint: fingerprint.secondary,
                    consecutive_hits: 0,
                    recorded_bounds: bounds,
                },
            );
            record_cache_pressure("subtree_snapshot", &report);
            event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_snapshot", result = "miss", amount = 1_u64);
            if self.subtree_descendants_have_dirty_composite(handle) {
                event!(target: "render.cache", Level::TRACE, kind = "cache", name = "subtree_snapshot_composite_dirty", result = "miss", amount = 1_u64);
            }
        }

        Ok(picture)
    }

    fn draw_timeline_transition_subtree(
        &mut self,
        recorded: RecordedNodeSemantics<'_>,
        timeline: &TimelineDisplayItem,
        opacity: f32,
        backdrop_blur_sigma: Option<f32>,
        bounds: DisplayRect,
        children: &[OrderedSceneOp],
    ) -> Result<()> {
        let Some(transition) = timeline.transition.as_ref() else {
            unreachable!("timeline transition draw requires active transition metadata");
        };
        if children.len() != 2 {
            return Err(anyhow!(
                "timeline transition requires exactly 2 children, got {}",
                children.len()
            ));
        }

        self.with_display_layer(opacity, backdrop_blur_sigma, bounds, |backend| {
            draw_timeline_base(backend.canvas, timeline)?;
            backend.with_recorded_clip(recorded, |backend| {
                let from_snapshot =
                    backend.acquire_transition_scene_picture(children[0].handle())?;
                let to_snapshot =
                    backend.acquire_transition_scene_picture(children[1].handle())?;
                let transition_span = span!(
                    target: "render.transition",
                    Level::TRACE,
                    "draw_transition",
                    transition_kind = match transition.kind {
                        crate::scene::transition::TransitionKind::Slide(_) => "slide",
                        crate::scene::transition::TransitionKind::LightLeak(_) => "light_leak",
                        _ => "other",
                    }
                );
                let _guard = transition_span.enter();
                super::transition::draw_transition(
                    backend.canvas,
                    &from_snapshot,
                    &to_snapshot,
                    transition.progress,
                    transition.kind,
                    timeline.bounds.width.max(1.0) as i32,
                    timeline.bounds.height.max(1.0) as i32,
                )
            })
        })
    }

    fn draw_subtree_snapshot(
        &mut self,
        snapshot: &Picture,
        opacity: f32,
        backdrop_blur_sigma: Option<f32>,
        bounds: DisplayRect,
    ) -> Result<()> {
        self.with_display_layer(opacity, backdrop_blur_sigma, bounds, |backend| {
            let profile_span =
                span!(target: "render.backend", Level::TRACE, "subtree_snapshot_draw");
            let _profile_span = profile_span.enter();
            backend.canvas.draw_picture(snapshot, None, None);
            Ok(())
        })
    }

    fn draw_subtree_image(
        &mut self,
        image: &SkiaImage,
        opacity: f32,
        backdrop_blur_sigma: Option<f32>,
        bounds: DisplayRect,
    ) -> Result<()> {
        self.with_display_layer(opacity, backdrop_blur_sigma, bounds, |backend| {
            let profile_span = span!(target: "render.backend", Level::TRACE, "subtree_image_draw");
            let _profile_span = profile_span.enter();
            backend.canvas.draw_image(image, (bounds.x, bounds.y), None);
            Ok(())
        })
    }

    fn draw_recorded_node_contents(
        &mut self,
        recorded: RecordedNodeSemantics<'_>,
        draw_children: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        self.draw_display_item(recorded.item)?;
        self.with_recorded_clip(recorded, draw_children)
    }

    fn with_recorded_clip<T>(
        &mut self,
        recorded: RecordedNodeSemantics<'_>,
        draw: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        if let Some(clip) = recorded.clip {
            self.canvas.save();
            clip_bounds(self.canvas, clip.bounds, clip.border_radius);
            let result = draw(self);
            self.canvas.restore();
            result
        } else {
            draw(self)
        }
    }

    fn with_display_layer<T>(
        &mut self,
        opacity: f32,
        backdrop_blur_sigma: Option<f32>,
        bounds: DisplayRect,
        draw: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let backdrop_blur_sigma = backdrop_blur_sigma.filter(|sigma| *sigma > 0.0);
        let uses_layer = opacity < 1.0 || backdrop_blur_sigma.is_some();
        if uses_layer {
            event!(target: "render.layer", Level::TRACE, kind = "layer", name = "save_layer", result = "count", amount = 1_u64);
            let bounds = layout_rect_to_skia(bounds);
            if let Some(sigma) = backdrop_blur_sigma {
                let alpha = (opacity * 255.0).round() as u32;
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
                let alpha = (opacity * 255.0).round() as u32;
                self.canvas.save_layer_alpha(bounds, alpha);
            }
        }

        let result = draw(self);

        if uses_layer {
            self.canvas.restore();
        }

        result
    }
}

pub(crate) fn draw_ordered_scene_cached<'a>(
    display_tree: &AnnotatedDisplayTree,
    ordered_scene: &OrderedSceneProgram,
    canvas: &'a Canvas,
    assets: &'a AssetsMap,
    image_cache: ImageCache,
    text_snapshot_cache: TextSnapshotCache,
    item_picture_cache: ItemPictureCache,
    subtree_snapshot_cache: SubtreeSnapshotCache,
    subtree_image_cache: SubtreeImageCache,
    media_ctx: Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
) -> Result<()> {
    let mut backend = SkiaBackend::new_with_cache(
        canvas,
        display_tree.root_node().transform.bounds.width as i32,
        display_tree.root_node().transform.bounds.height as i32,
        display_tree,
        assets,
        image_cache,
        text_snapshot_cache,
        item_picture_cache,
        Some(subtree_snapshot_cache),
        Some(subtree_image_cache),
        media_ctx,
        frame_ctx,
    );
    backend.draw_ordered_scene(ordered_scene)
}

pub(crate) fn record_display_tree_snapshot<'a>(
    display_tree: &AnnotatedDisplayTree,
    width: i32,
    height: i32,
    assets: &'a AssetsMap,
    image_cache: ImageCache,
    text_snapshot_cache: TextSnapshotCache,
    item_picture_cache: ItemPictureCache,
    subtree_snapshot_cache: SubtreeSnapshotCache,
    subtree_image_cache: SubtreeImageCache,
    media_ctx: Option<&'a mut MediaContext>,
    frame_ctx: &'a FrameCtx,
) -> Result<Picture> {
    let profile_span =
        span!(target: "render.backend", Level::TRACE, "display_tree_snapshot_record");
    let _profile_span = profile_span.enter();
    let bounds = Rect::from_xywh(0.0, 0.0, width as f32, height as f32);
    let mut recorder = PictureRecorder::new();
    let recording_canvas = recorder.begin_recording(bounds, false);
    let mut backend = SkiaBackend::new_with_cache(
        recording_canvas,
        width,
        height,
        display_tree,
        assets,
        image_cache,
        text_snapshot_cache,
        item_picture_cache,
        Some(subtree_snapshot_cache),
        Some(subtree_image_cache),
        media_ctx,
        frame_ctx,
    );
    backend.draw_display_subtree(display_tree.root, false)?;
    let snapshot = recorder
        .finish_recording_as_picture(None)
        .ok_or_else(|| anyhow!("failed to record display tree snapshot"))?;
    Ok(snapshot)
}

fn draw_rect(canvas: &Canvas, rect: &RectDisplayItem) {
    let style = &rect.paint;
    let has_any_border = style.border_width.is_some()
        || style.border_top_width.is_some()
        || style.border_right_width.is_some()
        || style.border_bottom_width.is_some()
        || style.border_left_width.is_some();
    if style.background.is_none() && !has_any_border && style.inset_shadow.is_none() {
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
    } else {
        if let Some(background) = style.background {
            apply_background_paint(&mut paint, background, rect);
            canvas.draw_rect(rect, &paint);
        }

        if let Some(shadow) = style.inset_shadow {
            draw_inset_shadow(canvas, bounds, style.border_radius, shadow);
        }
    }

    draw_node_border(
        canvas,
        rect,
        style.border_radius,
        style.border_width,
        style.border_top_width,
        style.border_right_width,
        style.border_bottom_width,
        style.border_left_width,
        style.border_color,
        style.border_style,
        style.blur_sigma,
    );
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
    let placement = text_snapshot_placement(text);
    let cache_key = text_paint_fingerprint(text);
    if let Some(snapshot) = text_snapshot_cache.borrow_mut().get_cloned(&cache_key) {
        canvas.save();
        canvas.translate((placement.draw_translation_x, placement.draw_translation_y));
        canvas.draw_picture(&snapshot, None, None);
        canvas.restore();
        Ok(TextDrawStats {
            cache_hits: 1,
            cache_misses: 0,
        })
    } else {
        let snapshot = record_text_snapshot(text)?;
        let report = text_snapshot_cache
            .borrow_mut()
            .insert(cache_key, snapshot.clone());
        record_cache_pressure("text", &report);

        canvas.save();
        canvas.translate((placement.draw_translation_x, placement.draw_translation_y));
        canvas.draw_picture(&snapshot, None, None);
        canvas.restore();
        Ok(TextDrawStats {
            cache_hits: 0,
            cache_misses: 1,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubtreeSnapshotResolution {
    Hit,
    Miss,
    CollisionRejected,
}

pub(crate) fn resolve_subtree_snapshot_lookup(
    query_fingerprint: crate::runtime::fingerprint::SubtreeSnapshotFingerprint,
    cached_secondary: Option<u64>,
) -> SubtreeSnapshotResolution {
    match cached_secondary {
        None => SubtreeSnapshotResolution::Miss,
        Some(secondary) if secondary == query_fingerprint.secondary => {
            SubtreeSnapshotResolution::Hit
        }
        Some(_) => SubtreeSnapshotResolution::CollisionRejected,
    }
}

const SUBTREE_IMAGE_PROMOTION_HITS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CachedSubtreeRenderMode {
    DrawImage,
    DrawPicture,
    PromoteToImage,
}

fn resolve_cached_subtree_render_mode(
    has_cached_image: bool,
    consecutive_hits: usize,
    recorded_bounds: DisplayRect,
    current_bounds: DisplayRect,
    has_non_unit_scale: bool,
) -> CachedSubtreeRenderMode {
    if has_cached_image && !has_non_unit_scale {
        return CachedSubtreeRenderMode::DrawImage;
    }
    if should_promote_snapshot_to_image(
        consecutive_hits,
        recorded_bounds,
        current_bounds,
        has_non_unit_scale,
    ) {
        CachedSubtreeRenderMode::PromoteToImage
    } else {
        CachedSubtreeRenderMode::DrawPicture
    }
}

fn transform_list_has_non_unit_scale(transforms: &[Transform]) -> bool {
    transforms.iter().any(|transform| match *transform {
        Transform::Scale(value) | Transform::ScaleX(value) | Transform::ScaleY(value) => {
            (value - 1.0).abs() > f32::EPSILON
        }
        _ => false,
    })
}

fn should_promote_snapshot_to_image(
    consecutive_hits: usize,
    recorded_bounds: DisplayRect,
    current_bounds: DisplayRect,
    has_non_unit_scale: bool,
) -> bool {
    consecutive_hits >= SUBTREE_IMAGE_PROMOTION_HITS
        && !has_non_unit_scale
        && recorded_bounds.x.to_bits() == current_bounds.x.to_bits()
        && recorded_bounds.y.to_bits() == current_bounds.y.to_bits()
        && recorded_bounds.width.to_bits() == current_bounds.width.to_bits()
        && recorded_bounds.height.to_bits() == current_bounds.height.to_bits()
}

fn record_subtree_snapshot_image(
    snapshot: &Picture,
    recorded_bounds: DisplayRect,
) -> Result<SkiaImage> {
    let profile_span = span!(target: "render.backend", Level::TRACE, "subtree_image_rasterize");
    let _profile_span = profile_span.enter();
    let width = recorded_bounds.width.max(1.0).round() as i32;
    let height = recorded_bounds.height.max(1.0).round() as i32;
    let mut surface = skia_safe::surfaces::raster_n32_premul((width, height))
        .ok_or_else(|| anyhow!("failed to create subtree image surface"))?;
    surface.canvas().save();
    surface
        .canvas()
        .translate((-recorded_bounds.x, -recorded_bounds.y));
    surface.canvas().draw_picture(snapshot, None, None);
    surface.canvas().restore();
    Ok(surface.image_snapshot())
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
    let semantics = item.picture_semantics();
    if let Some(snapshot) = item_picture_cache.borrow_mut().get_cloned(&cache_key) {
        canvas.save();
        canvas.translate((semantics.draw_translation_x, semantics.draw_translation_y));
        canvas.draw_picture(&snapshot, None, None);
        canvas.restore();
        return Ok(ItemPictureDrawStats {
            cache_hits: 1,
            cache_misses: 0,
        });
    }

    let snapshot = record_item_picture(
        item,
        assets,
        image_cache,
        text_snapshot_cache,
        media_ctx,
        frame_ctx,
    )?;
    let report = item_picture_cache
        .borrow_mut()
        .insert(cache_key, snapshot.clone());
    record_cache_pressure("item_picture", &report);

    canvas.save();
    canvas.translate((semantics.draw_translation_x, semantics.draw_translation_y));
    canvas.draw_picture(&snapshot, None, None);
    canvas.restore();
    Ok(ItemPictureDrawStats {
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
    let semantics = item.picture_semantics();
    let bounds = Rect::from_xywh(
        semantics.record_bounds.x,
        semantics.record_bounds.y,
        semantics.record_bounds.width,
        semantics.record_bounds.height,
    );
    let mut recorder = PictureRecorder::new();
    let recording_canvas = recorder.begin_recording(bounds, false);
    recording_canvas.translate((
        semantics.record_translation_x,
        semantics.record_translation_y,
    ));
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
        DisplayItem::Timeline(timeline) => {
            draw_timeline_base(canvas, timeline)?;
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

fn draw_timeline_base(canvas: &Canvas, timeline: &TimelineDisplayItem) -> Result<()> {
    let rect = RectDisplayItem {
        bounds: timeline.bounds,
        paint: timeline.paint.clone(),
    };
    if let Some(shadow) = rect.paint.box_shadow {
        draw_box_shadow(canvas, rect.bounds, rect.paint.border_radius, shadow);
    }
    if let Some(shadow) = rect.paint.drop_shadow {
        draw_item_drop_shadow(canvas, rect.bounds, shadow, |canvas| {
            draw_rect(canvas, &rect);
            Ok(())
        })?;
    }
    draw_rect(canvas, &rect);
    Ok(())
}

fn record_text_snapshot(text: &TextDisplayItem) -> Result<Picture> {
    let placement = text_snapshot_placement(text);
    let bounds = Rect::from_xywh(
        placement.record_bounds.x,
        placement.record_bounds.y,
        placement.record_bounds.width,
        placement.record_bounds.height,
    );
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

fn text_snapshot_placement(text: &TextDisplayItem) -> TextSnapshotPlacement {
    TextSnapshotPlacement {
        record_bounds: DisplayRect {
            x: 0.0,
            y: 0.0,
            width: text.bounds.width.max(1.0),
            height: text.bounds.height.max(1.0),
        },
        draw_translation_x: text.bounds.x,
        draw_translation_y: text.bounds.y,
    }
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
        let video_bitmap = media
            .get_video_bitmap(path, request)
            .with_context(|| format!("failed to decode video frame: {}", path.display()))?;
        if video_bitmap.frame_cache_hit {
            stats.video_frame_cache_hits = 1;
        } else {
            stats.video_frame_cache_misses = 1;
            stats.video_frame_decodes = 1;
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
            let encoded = std::fs::read(path)
                .with_context(|| format!("failed to read image asset: {}", path.display()))?;
            let data = skia_safe::Data::new_copy(&encoded);
            let image = skia_safe::Image::from_encoded(data)
                .ok_or_else(|| anyhow!("failed to decode image asset: {}", path.display()))?;
            stats.image_cache_misses = 1;
            let report = cache.insert(key, Some(image.clone()));
            record_cache_pressure("image", &report);
            image
        }
    };

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

    draw_node_border(
        canvas,
        dst,
        bitmap.paint.border_radius,
        bitmap.paint.border_width,
        bitmap.paint.border_top_width,
        bitmap.paint.border_right_width,
        bitmap.paint.border_bottom_width,
        bitmap.paint.border_left_width,
        bitmap.paint.border_color,
        bitmap.paint.border_style,
        bitmap.paint.blur_sigma,
    );

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
    let report = cache.insert(key, Some(image.clone()));
    record_cache_pressure("image", &report);
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

fn apply_border_dash_effect(
    paint: &mut Paint,
    width: f32,
    border_style: crate::style::BorderStyle,
) {
    match border_style {
        crate::style::BorderStyle::Solid => {}
        crate::style::BorderStyle::Dashed => {
            let unit = width.max(1.0) * 2.0;
            if let Some(effect) = skia_safe::PathEffect::dash(&[unit, unit], 0.0) {
                paint.set_path_effect(effect);
            }
        }
        crate::style::BorderStyle::Dotted => {
            paint.set_stroke_cap(skia_safe::paint::Cap::Round);
            let gap = width.max(1.0) * 2.0;
            if let Some(effect) = skia_safe::PathEffect::dash(&[0.0, gap], 0.0) {
                paint.set_path_effect(effect);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_node_border(
    canvas: &Canvas,
    rect: Rect,
    radius: crate::style::BorderRadius,
    border_width: Option<f32>,
    border_top_width: Option<f32>,
    border_right_width: Option<f32>,
    border_bottom_width: Option<f32>,
    border_left_width: Option<f32>,
    border_color: Option<crate::style::ColorToken>,
    border_style: Option<crate::style::BorderStyle>,
    blur_sigma: Option<f32>,
) {
    let Some(color) = border_color else {
        return;
    };
    let uniform = border_width.unwrap_or(0.0);
    let top_w = border_top_width.unwrap_or(uniform);
    let right_w = border_right_width.unwrap_or(uniform);
    let bottom_w = border_bottom_width.unwrap_or(uniform);
    let left_w = border_left_width.unwrap_or(uniform);
    if top_w <= 0.0 && right_w <= 0.0 && bottom_w <= 0.0 && left_w <= 0.0 {
        return;
    }

    let stroke_style = border_style.unwrap_or_default();
    let skia_col = skia_color(color);

    match stroke_style {
        crate::style::BorderStyle::Solid => {
            draw_border_fill_ring(
                canvas, rect, radius, top_w, right_w, bottom_w, left_w, skia_col, blur_sigma,
            );
        }
        crate::style::BorderStyle::Dashed | crate::style::BorderStyle::Dotted => {
            draw_per_side_borders(
                canvas,
                rect,
                radius,
                top_w,
                right_w,
                bottom_w,
                left_w,
                skia_col,
                stroke_style,
                blur_sigma,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_border_fill_ring(
    canvas: &Canvas,
    outer_rect: Rect,
    outer_radius: crate::style::BorderRadius,
    top_w: f32,
    right_w: f32,
    bottom_w: f32,
    left_w: f32,
    color: skia_safe::Color,
    blur_sigma: Option<f32>,
) {
    let inner_left = outer_rect.left + left_w.max(0.0);
    let inner_top = outer_rect.top + top_w.max(0.0);
    let inner_right = outer_rect.right - right_w.max(0.0);
    let inner_bottom = outer_rect.bottom - bottom_w.max(0.0);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    apply_blur_effect(&mut paint, blur_sigma);
    paint.set_color(color);
    paint.set_style(PaintStyle::Fill);

    let outer_rrect = make_rrect(outer_rect, outer_radius);

    if inner_right <= inner_left || inner_bottom <= inner_top {
        canvas.draw_rrect(outer_rrect, &paint);
        return;
    }

    let inner_rect = Rect::from_ltrb(inner_left, inner_top, inner_right, inner_bottom);
    let inner_radius = crate::style::BorderRadius {
        top_left: (outer_radius.top_left - top_w.max(left_w)).max(0.0),
        top_right: (outer_radius.top_right - top_w.max(right_w)).max(0.0),
        bottom_right: (outer_radius.bottom_right - bottom_w.max(right_w)).max(0.0),
        bottom_left: (outer_radius.bottom_left - bottom_w.max(left_w)).max(0.0),
    };
    let inner_rrect = make_rrect(inner_rect, inner_radius);

    let mut builder = skia_safe::PathBuilder::new_with_fill_type(skia_safe::PathFillType::EvenOdd);
    builder.add_rrect(outer_rrect, None::<skia_safe::PathDirection>, None::<usize>);
    builder.add_rrect(inner_rrect, None::<skia_safe::PathDirection>, None::<usize>);
    let path = builder.snapshot();
    canvas.draw_path(&path, &paint);
}

#[allow(clippy::too_many_arguments)]
fn draw_per_side_borders(
    canvas: &Canvas,
    rect: Rect,
    radius: crate::style::BorderRadius,
    top_w: f32,
    right_w: f32,
    bottom_w: f32,
    left_w: f32,
    color: skia_safe::Color,
    border_style: crate::style::BorderStyle,
    blur_sigma: Option<f32>,
) {
    let left = rect.left;
    let top = rect.top;
    let right = rect.right;
    let bottom = rect.bottom;
    let radii = effective_corner_radius(rect, radius);
    let r_tl = radii[0];
    let r_tr = radii[1];
    let r_br = radii[2];
    let r_bl = radii[3];

    let build_paint = |width: f32| -> Paint {
        let mut p = Paint::default();
        p.set_anti_alias(true);
        apply_blur_effect(&mut p, blur_sigma);
        p.set_color(color);
        p.set_style(PaintStyle::Stroke);
        p.set_stroke_width(width);
        p.set_stroke_cap(skia_safe::paint::Cap::Butt);
        apply_border_dash_effect(&mut p, width, border_style);
        p
    };

    let draw_segment = |x0: f32, y0: f32, x1: f32, y1: f32, width: f32| {
        let mut builder = PathBuilder::new();
        builder.move_to((x0, y0));
        builder.line_to((x1, y1));
        let path = builder.snapshot();
        canvas.draw_path(&path, &build_paint(width));
    };

    if top_w > 0.0 {
        let y = top + top_w / 2.0;
        let x0 = if top_w == left_w && r_tl > 0.0 {
            left + r_tl
        } else if left_w > 0.0 {
            left + left_w
        } else {
            left
        };
        let x1 = if top_w == right_w && r_tr > 0.0 {
            right - r_tr
        } else if right_w > 0.0 {
            right - right_w
        } else {
            right
        };
        if x1 > x0 {
            draw_segment(x0, y, x1, y, top_w);
        }
    }

    if right_w > 0.0 {
        let x = right - right_w / 2.0;
        let y0 = if right_w == top_w && r_tr > 0.0 {
            top + r_tr
        } else if top_w > 0.0 {
            top + top_w
        } else {
            top
        };
        let y1 = if right_w == bottom_w && r_br > 0.0 {
            bottom - r_br
        } else if bottom_w > 0.0 {
            bottom - bottom_w
        } else {
            bottom
        };
        if y1 > y0 {
            draw_segment(x, y0, x, y1, right_w);
        }
    }

    if bottom_w > 0.0 {
        let y = bottom - bottom_w / 2.0;
        let x0 = if bottom_w == left_w && r_bl > 0.0 {
            left + r_bl
        } else if left_w > 0.0 {
            left + left_w
        } else {
            left
        };
        let x1 = if bottom_w == right_w && r_br > 0.0 {
            right - r_br
        } else if right_w > 0.0 {
            right - right_w
        } else {
            right
        };
        if x1 > x0 {
            draw_segment(x0, y, x1, y, bottom_w);
        }
    }

    if left_w > 0.0 {
        let x = left + left_w / 2.0;
        let y0 = if left_w == top_w && r_tl > 0.0 {
            top + r_tl
        } else if top_w > 0.0 {
            top + top_w
        } else {
            top
        };
        let y1 = if left_w == bottom_w && r_bl > 0.0 {
            bottom - r_bl
        } else if bottom_w > 0.0 {
            bottom - bottom_w
        } else {
            bottom
        };
        if y1 > y0 {
            draw_segment(x, y0, x, y1, left_w);
        }
    }

    let draw_corner_arc = |cx: f32, cy: f32, corner_r: f32, width: f32, start_deg: f32| {
        let arc_r = (corner_r - width / 2.0).max(0.0);
        if arc_r <= 0.0 {
            return;
        }
        let arc_rect = Rect::from_xywh(cx - arc_r, cy - arc_r, 2.0 * arc_r, 2.0 * arc_r);
        let mut builder = PathBuilder::new();
        builder.arc_to(arc_rect, start_deg, 90.0, true);
        let path = builder.snapshot();
        canvas.draw_path(&path, &build_paint(width));
    };

    if r_tl > 0.0 && top_w > 0.0 && top_w == left_w {
        draw_corner_arc(left + r_tl, top + r_tl, r_tl, top_w, 180.0);
    }
    if r_tr > 0.0 && top_w > 0.0 && top_w == right_w {
        draw_corner_arc(right - r_tr, top + r_tr, r_tr, top_w, 270.0);
    }
    if r_br > 0.0 && bottom_w > 0.0 && bottom_w == right_w {
        draw_corner_arc(right - r_br, bottom - r_br, r_br, bottom_w, 0.0);
    }
    if r_bl > 0.0 && bottom_w > 0.0 && bottom_w == left_w {
        draw_corner_arc(left + r_bl, bottom - r_bl, r_bl, bottom_w, 90.0);
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

fn record_cache_pressure<K>(
    cache_name: &'static str,
    report: &crate::runtime::cache::lru::CacheMutationReport<K>,
) {
    if !report.evicted.is_empty() {
        event!(target: "render.cache", Level::TRACE, kind = "eviction", name = cache_name, result = "count", amount = report.evicted.len() as u64);
    }
    if report.replaced {
        event!(target: "render.cache", Level::TRACE, kind = "repeat", name = cache_name, result = "count", amount = 1_u64);
    }
    event!(target: "render.cache", Level::TRACE, kind = "utilization", name = cache_name, result = "count", amount = report.utilization as u64);
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

#[cfg(test)]
mod resolve_tests {
    use super::{SubtreeSnapshotResolution, resolve_subtree_snapshot_lookup};
    use crate::runtime::fingerprint::SubtreeSnapshotFingerprint;

    #[test]
    fn hit_returns_hit_when_secondary_matches() {
        let fp = SubtreeSnapshotFingerprint {
            primary: 1,
            secondary: 100,
        };
        let cached_secondary = Some(100u64);
        assert_eq!(
            resolve_subtree_snapshot_lookup(fp, cached_secondary),
            SubtreeSnapshotResolution::Hit,
        );
    }

    #[test]
    fn collision_returns_reject_when_primary_same_but_secondary_differs() {
        let fp = SubtreeSnapshotFingerprint {
            primary: 1,
            secondary: 100,
        };
        let cached_secondary = Some(999u64);
        assert_eq!(
            resolve_subtree_snapshot_lookup(fp, cached_secondary),
            SubtreeSnapshotResolution::CollisionRejected,
        );
    }

    #[test]
    fn miss_returns_miss_when_cache_has_nothing() {
        let fp = SubtreeSnapshotFingerprint {
            primary: 1,
            secondary: 100,
        };
        assert_eq!(
            resolve_subtree_snapshot_lookup(fp, None),
            SubtreeSnapshotResolution::Miss,
        );
    }
}

#[cfg(test)]
mod promotion_tests {
    use super::{
        SUBTREE_IMAGE_PROMOTION_HITS, should_promote_snapshot_to_image,
        transform_list_has_non_unit_scale,
    };
    use crate::{display::list::DisplayRect, style::Transform};

    fn bounds(width: f32, height: f32) -> DisplayRect {
        DisplayRect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    #[test]
    fn promotion_requires_threshold_and_matching_bounds() {
        let recorded = bounds(320.0, 180.0);
        assert!(!should_promote_snapshot_to_image(
            SUBTREE_IMAGE_PROMOTION_HITS - 1,
            recorded,
            recorded,
            false,
        ));
        assert!(should_promote_snapshot_to_image(
            SUBTREE_IMAGE_PROMOTION_HITS,
            recorded,
            recorded,
            false,
        ));
    }

    #[test]
    fn promotion_rejects_scale_and_bounds_changes() {
        let recorded = bounds(320.0, 180.0);
        let resized = bounds(640.0, 360.0);
        assert!(!should_promote_snapshot_to_image(
            SUBTREE_IMAGE_PROMOTION_HITS,
            recorded,
            resized,
            false,
        ));
        assert!(!should_promote_snapshot_to_image(
            SUBTREE_IMAGE_PROMOTION_HITS,
            recorded,
            recorded,
            true,
        ));
    }

    #[test]
    fn detects_non_unit_scale_transforms() {
        assert!(!transform_list_has_non_unit_scale(&[]));
        assert!(!transform_list_has_non_unit_scale(&[Transform::Scale(1.0)]));
        assert!(transform_list_has_non_unit_scale(&[Transform::Scale(1.25)]));
        assert!(transform_list_has_non_unit_scale(&[Transform::ScaleX(0.8)]));
        assert!(transform_list_has_non_unit_scale(&[Transform::ScaleY(1.1)]));
        assert!(!transform_list_has_non_unit_scale(&[
            Transform::TranslateX(20.0)
        ]));
    }
}

#[cfg(test)]
mod materialization_tests {
    use super::{
        CachedSubtreeRenderMode, SUBTREE_IMAGE_PROMOTION_HITS, resolve_cached_subtree_render_mode,
    };
    use crate::display::list::DisplayRect;

    fn bounds(width: f32, height: f32) -> DisplayRect {
        DisplayRect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    #[test]
    fn image_hit_wins_when_scale_is_absent() {
        let recorded = bounds(320.0, 180.0);
        assert_eq!(
            resolve_cached_subtree_render_mode(true, 0, recorded, recorded, false),
            CachedSubtreeRenderMode::DrawImage,
        );
    }

    #[test]
    fn picture_hit_promotes_once_threshold_is_reached() {
        let recorded = bounds(320.0, 180.0);
        assert_eq!(
            resolve_cached_subtree_render_mode(
                false,
                SUBTREE_IMAGE_PROMOTION_HITS,
                recorded,
                recorded,
                false,
            ),
            CachedSubtreeRenderMode::PromoteToImage,
        );
    }

    #[test]
    fn scale_forces_picture_fallback_even_when_image_exists() {
        let recorded = bounds(320.0, 180.0);
        assert_eq!(
            resolve_cached_subtree_render_mode(true, 99, recorded, recorded, true),
            CachedSubtreeRenderMode::DrawPicture,
        );
    }
}
