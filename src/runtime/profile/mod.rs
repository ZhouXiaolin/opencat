mod bus;

use std::collections::BTreeMap;

use crate::layout::LayoutPassStats;

pub(crate) use bus::{
    BackendCountMetric, BackendDurationMetric, BackendProfileEvent, BackendProfileSink,
    backend_span, record_backend_count, record_backend_duration, record_backend_elapsed,
    with_backend_profile_sink,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BackendSpanKey {
    pub depth: usize,
    pub parent: Option<&'static str>,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BackendSpanAggregate {
    pub inclusive_ms: f64,
    pub exclusive_ms: f64,
    pub count: usize,
}

impl BackendSpanAggregate {
    fn record(&mut self, inclusive_ms: f64, exclusive_ms: f64) {
        self.inclusive_ms += inclusive_ms;
        self.exclusive_ms += exclusive_ms;
        self.count += 1;
    }

    fn merge(&mut self, other: &BackendSpanAggregate) {
        self.inclusive_ms += other.inclusive_ms;
        self.exclusive_ms += other.exclusive_ms;
        self.count += other.count;
    }
}

#[derive(Clone, Copy, Debug)]
struct BackendSpanRecord {
    depth: usize,
    parent: Option<&'static str>,
    name: &'static str,
    inclusive_ms: f64,
    exclusive_ms: f64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BackendProfileReport {
    pub profile: BackendProfile,
    pub spans: BTreeMap<BackendSpanKey, BackendSpanAggregate>,
}

#[derive(Clone, Debug, Default)]
pub struct BackendProfile {
    pub rect_draw_ms: f64,
    pub text_draw_ms: f64,
    pub text_snapshot_record_ms: f64,
    pub text_snapshot_draw_ms: f64,
    pub item_picture_record_ms: f64,
    pub item_picture_draw_ms: f64,
    pub bitmap_draw_ms: f64,
    pub draw_script_draw_ms: f64,
    pub image_decode_ms: f64,
    pub video_decode_ms: f64,
    pub subtree_snapshot_record_ms: f64,
    pub subtree_snapshot_draw_ms: f64,
    pub light_leak_mask_ms: f64,
    pub light_leak_composite_ms: f64,
    pub scene_snapshot_cache_hits: usize,
    pub scene_snapshot_cache_misses: usize,
    pub subtree_snapshot_cache_hits: usize,
    pub subtree_snapshot_cache_misses: usize,
    pub subtree_snapshot_collision_rejected: usize,
    pub text_cache_hits: usize,
    pub text_cache_misses: usize,
    pub item_picture_cache_hits: usize,
    pub item_picture_cache_misses: usize,
    pub image_cache_hits: usize,
    pub image_cache_misses: usize,
    pub video_frame_cache_hits: usize,
    pub video_frame_cache_misses: usize,
    pub video_frame_decodes: usize,
    pub draw_rect_count: usize,
    pub draw_text_count: usize,
    pub draw_bitmap_count: usize,
    pub draw_script_count: usize,
    pub save_layer_count: usize,
}

impl BackendProfile {
    fn record_duration(&mut self, metric: BackendDurationMetric, ms: f64) {
        match metric {
            BackendDurationMetric::RectDraw => self.rect_draw_ms += ms,
            BackendDurationMetric::TextDraw => self.text_draw_ms += ms,
            BackendDurationMetric::TextSnapshotRecord => self.text_snapshot_record_ms += ms,
            BackendDurationMetric::TextSnapshotDraw => self.text_snapshot_draw_ms += ms,
            BackendDurationMetric::ItemPictureRecord => self.item_picture_record_ms += ms,
            BackendDurationMetric::ItemPictureDraw => self.item_picture_draw_ms += ms,
            BackendDurationMetric::BitmapDraw => self.bitmap_draw_ms += ms,
            BackendDurationMetric::DrawScriptDraw => self.draw_script_draw_ms += ms,
            BackendDurationMetric::ImageDecode => self.image_decode_ms += ms,
            BackendDurationMetric::VideoDecode => self.video_decode_ms += ms,
            BackendDurationMetric::SubtreeSnapshotRecord => self.subtree_snapshot_record_ms += ms,
            BackendDurationMetric::SubtreeSnapshotDraw => self.subtree_snapshot_draw_ms += ms,
            BackendDurationMetric::LightLeakMask => self.light_leak_mask_ms += ms,
            BackendDurationMetric::LightLeakComposite => self.light_leak_composite_ms += ms,
        }
    }

    fn record_count(&mut self, metric: BackendCountMetric, amount: usize) {
        match metric {
            BackendCountMetric::SceneSnapshotCacheHit => self.scene_snapshot_cache_hits += amount,
            BackendCountMetric::SceneSnapshotCacheMiss => {
                self.scene_snapshot_cache_misses += amount
            }
            BackendCountMetric::SubtreeSnapshotCacheHit => {
                self.subtree_snapshot_cache_hits += amount;
            }
            BackendCountMetric::SubtreeSnapshotCacheMiss => {
                self.subtree_snapshot_cache_misses += amount;
            }
            BackendCountMetric::SubtreeSnapshotCollisionRejected => {
                self.subtree_snapshot_collision_rejected += amount;
            }
            BackendCountMetric::TextCacheHit => self.text_cache_hits += amount,
            BackendCountMetric::TextCacheMiss => self.text_cache_misses += amount,
            BackendCountMetric::ItemPictureCacheHit => self.item_picture_cache_hits += amount,
            BackendCountMetric::ItemPictureCacheMiss => self.item_picture_cache_misses += amount,
            BackendCountMetric::ImageCacheHit => self.image_cache_hits += amount,
            BackendCountMetric::ImageCacheMiss => self.image_cache_misses += amount,
            BackendCountMetric::VideoFrameCacheHit => self.video_frame_cache_hits += amount,
            BackendCountMetric::VideoFrameCacheMiss => self.video_frame_cache_misses += amount,
            BackendCountMetric::VideoFrameDecode => self.video_frame_decodes += amount,
            BackendCountMetric::DrawRect => self.draw_rect_count += amount,
            BackendCountMetric::DrawText => self.draw_text_count += amount,
            BackendCountMetric::DrawBitmap => self.draw_bitmap_count += amount,
            BackendCountMetric::DrawScript => self.draw_script_count += amount,
            BackendCountMetric::SaveLayer => self.save_layer_count += amount,
        }
    }
}

impl BackendProfileSink for BackendProfile {
    fn record_backend_event(&mut self, event: BackendProfileEvent) {
        match event {
            BackendProfileEvent::Duration { metric, ms } => self.record_duration(metric, ms),
            BackendProfileEvent::Count { metric, amount } => self.record_count(metric, amount),
            BackendProfileEvent::SpanCompleted { .. } => {}
        }
    }
}

#[derive(Default)]
pub(crate) struct BackendProfileCollector {
    profile: BackendProfile,
    span_records: Vec<BackendSpanRecord>,
}

impl BackendProfileCollector {
    pub(crate) fn finish(self) -> BackendProfileReport {
        let mut spans = BTreeMap::<BackendSpanKey, BackendSpanAggregate>::new();
        for record in self.span_records {
            spans
                .entry(BackendSpanKey {
                    depth: record.depth,
                    parent: record.parent,
                    name: record.name,
                })
                .or_default()
                .record(record.inclusive_ms, record.exclusive_ms);
        }
        BackendProfileReport {
            profile: self.profile,
            spans,
        }
    }
}

impl BackendProfileSink for BackendProfileCollector {
    fn record_backend_event(&mut self, event: BackendProfileEvent) {
        match event {
            BackendProfileEvent::Duration { metric, ms } => {
                self.profile.record_duration(metric, ms);
            }
            BackendProfileEvent::Count { metric, amount } => {
                self.profile.record_count(metric, amount);
            }
            BackendProfileEvent::SpanCompleted {
                depth,
                name,
                parent,
                inclusive_ms,
                exclusive_ms,
            } => {
                self.span_records.push(BackendSpanRecord {
                    depth,
                    parent,
                    name,
                    inclusive_ms,
                    exclusive_ms,
                });
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct SceneBuildStats {
    pub resolve_ms: f64,
    pub layout_ms: f64,
    pub display_ms: f64,
    pub layout_pass: LayoutPassStats,
    pub contains_time_variant_paint: bool,
}

#[derive(Default)]
pub(crate) struct FrameProfile {
    pub script_ms: f64,
    pub frame_state_ms: f64,
    pub resolve_ms: f64,
    pub layout_ms: f64,
    pub display_ms: f64,
    pub backend_ms: f64,
    pub transition_ms: f64,
    pub slide_transition_ms: f64,
    pub light_leak_transition_ms: f64,
    pub slide_transition_frames: usize,
    pub light_leak_transition_frames: usize,
    pub reused_nodes: usize,
    pub layout_dirty_nodes: usize,
    pub raster_dirty_nodes: usize,
    pub composite_dirty_nodes: usize,
    pub structure_rebuilds: usize,
    pub backend: BackendProfile,
    pub backend_spans: BTreeMap<BackendSpanKey, BackendSpanAggregate>,
}

#[derive(Default)]
pub struct RenderProfiler {
    frames: Vec<FrameProfile>,
}

impl FrameProfile {
    pub(crate) fn merge_scene_stats(&mut self, stats: &SceneBuildStats) {
        self.resolve_ms += stats.resolve_ms;
        self.layout_ms += stats.layout_ms;
        self.display_ms += stats.display_ms;
        self.reused_nodes += stats.layout_pass.reused_nodes;
        self.layout_dirty_nodes += stats.layout_pass.layout_dirty_nodes;
        self.raster_dirty_nodes += stats.layout_pass.raster_dirty_nodes;
        self.composite_dirty_nodes += stats.layout_pass.composite_dirty_nodes;
        self.structure_rebuilds += usize::from(stats.layout_pass.structure_rebuild);
    }

    pub(crate) fn merge_backend_profile(&mut self, report: &BackendProfileReport) {
        let profile = &report.profile;
        self.backend.rect_draw_ms += profile.rect_draw_ms;
        self.backend.text_draw_ms += profile.text_draw_ms;
        self.backend.text_snapshot_record_ms += profile.text_snapshot_record_ms;
        self.backend.text_snapshot_draw_ms += profile.text_snapshot_draw_ms;
        self.backend.item_picture_record_ms += profile.item_picture_record_ms;
        self.backend.item_picture_draw_ms += profile.item_picture_draw_ms;
        self.backend.bitmap_draw_ms += profile.bitmap_draw_ms;
        self.backend.draw_script_draw_ms += profile.draw_script_draw_ms;
        self.backend.image_decode_ms += profile.image_decode_ms;
        self.backend.video_decode_ms += profile.video_decode_ms;
        self.backend.subtree_snapshot_record_ms += profile.subtree_snapshot_record_ms;
        self.backend.subtree_snapshot_draw_ms += profile.subtree_snapshot_draw_ms;
        self.backend.light_leak_mask_ms += profile.light_leak_mask_ms;
        self.backend.light_leak_composite_ms += profile.light_leak_composite_ms;
        self.backend.scene_snapshot_cache_hits += profile.scene_snapshot_cache_hits;
        self.backend.scene_snapshot_cache_misses += profile.scene_snapshot_cache_misses;
        self.backend.subtree_snapshot_cache_hits += profile.subtree_snapshot_cache_hits;
        self.backend.subtree_snapshot_cache_misses += profile.subtree_snapshot_cache_misses;
        self.backend.subtree_snapshot_collision_rejected += profile.subtree_snapshot_collision_rejected;
        self.backend.text_cache_hits += profile.text_cache_hits;
        self.backend.text_cache_misses += profile.text_cache_misses;
        self.backend.item_picture_cache_hits += profile.item_picture_cache_hits;
        self.backend.item_picture_cache_misses += profile.item_picture_cache_misses;
        self.backend.image_cache_hits += profile.image_cache_hits;
        self.backend.image_cache_misses += profile.image_cache_misses;
        self.backend.video_frame_cache_hits += profile.video_frame_cache_hits;
        self.backend.video_frame_cache_misses += profile.video_frame_cache_misses;
        self.backend.video_frame_decodes += profile.video_frame_decodes;
        self.backend.draw_rect_count += profile.draw_rect_count;
        self.backend.draw_text_count += profile.draw_text_count;
        self.backend.draw_bitmap_count += profile.draw_bitmap_count;
        self.backend.draw_script_count += profile.draw_script_count;
        self.backend.save_layer_count += profile.save_layer_count;
        for (key, aggregate) in &report.spans {
            self.backend_spans.entry(*key).or_default().merge(aggregate);
        }
    }
}

impl RenderProfiler {
    pub(crate) fn push(&mut self, frame: FrameProfile) {
        self.frames.push(frame);
    }

    pub fn print_summary(&self) {
        if self.frames.is_empty() {
            return;
        }

        eprintln!("Render profile:");
        eprintln!("  frames: {}", self.frames.len());
        eprintln!(
            "  avg ms/frame: script {:.2}, frame_state {:.2}, resolve {:.2}, layout {:.2}, display {:.2}, backend {:.2}, transition {:.2}",
            average(&self.frames, |frame| frame.script_ms),
            average(&self.frames, |frame| frame.frame_state_ms),
            average(&self.frames, |frame| frame.resolve_ms),
            average(&self.frames, |frame| frame.layout_ms),
            average(&self.frames, |frame| frame.display_ms),
            average(&self.frames, |frame| frame.backend_ms),
            average(&self.frames, |frame| frame.transition_ms),
        );
        eprintln!(
            "  p95 ms/frame: resolve {:.2}, layout {:.2}, display {:.2}, backend {:.2}, transition {:.2}",
            percentile_95(&self.frames, |frame| frame.resolve_ms),
            percentile_95(&self.frames, |frame| frame.layout_ms),
            percentile_95(&self.frames, |frame| frame.display_ms),
            percentile_95(&self.frames, |frame| frame.backend_ms),
            percentile_95(&self.frames, |frame| frame.transition_ms),
        );
        eprintln!(
            "  transition avg ms/active-frame: slide {:.2} ({} frames), light_leak {:.2} ({} frames)",
            average_when_counted(
                &self.frames,
                |frame| frame.slide_transition_ms,
                |frame| frame.slide_transition_frames,
            ),
            self.frames
                .iter()
                .map(|frame| frame.slide_transition_frames)
                .sum::<usize>(),
            average_when_counted(
                &self.frames,
                |frame| frame.light_leak_transition_ms,
                |frame| frame.light_leak_transition_frames,
            ),
            self.frames
                .iter()
                .map(|frame| frame.light_leak_transition_frames)
                .sum::<usize>(),
        );
        eprintln!(
            "  avg nodes/frame: reused {:.1}, layout_dirty {:.1}, raster_dirty {:.1}, composite_dirty {:.1}, structure_rebuilds {:.2}",
            average_usize(&self.frames, |frame| frame.reused_nodes),
            average_usize(&self.frames, |frame| frame.layout_dirty_nodes),
            average_usize(&self.frames, |frame| frame.raster_dirty_nodes),
            average_usize(&self.frames, |frame| frame.composite_dirty_nodes),
            average_usize(&self.frames, |frame| frame.structure_rebuilds),
        );
        eprintln!(
            "  backend avg ms/frame: rect {:.2}, text {:.2}, text_snapshot_record {:.2}, text_snapshot_draw {:.2}, item_picture_record {:.2}, item_picture_draw {:.2}, bitmap {:.2}, draw_script {:.2}, image_decode {:.2}, video_decode {:.2}, subtree_snapshot_record {:.2}, subtree_snapshot_draw {:.2}, light_leak_mask {:.2}, light_leak_composite {:.2}",
            average(&self.frames, |frame| frame.backend.rect_draw_ms),
            average(&self.frames, |frame| frame.backend.text_draw_ms),
            average(&self.frames, |frame| frame.backend.text_snapshot_record_ms),
            average(&self.frames, |frame| frame.backend.text_snapshot_draw_ms),
            average(&self.frames, |frame| frame.backend.item_picture_record_ms),
            average(&self.frames, |frame| frame.backend.item_picture_draw_ms),
            average(&self.frames, |frame| frame.backend.bitmap_draw_ms),
            average(&self.frames, |frame| frame.backend.draw_script_draw_ms),
            average(&self.frames, |frame| frame.backend.image_decode_ms),
            average(&self.frames, |frame| frame.backend.video_decode_ms),
            average(&self.frames, |frame| frame
                .backend
                .subtree_snapshot_record_ms),
            average(&self.frames, |frame| frame.backend.subtree_snapshot_draw_ms),
            average(&self.frames, |frame| frame.backend.light_leak_mask_ms),
            average(&self.frames, |frame| frame.backend.light_leak_composite_ms),
        );
        eprintln!(
            "  backend avg counts/frame: rect {:.1}, text {:.1}, bitmap {:.1}, draw_script {:.1}, save_layer {:.1}, text_hit {:.2}, text_miss {:.2}, item_hit {:.2}, item_miss {:.2}, scene_snapshot_hit {:.2}, scene_snapshot_miss {:.2}, subtree_snapshot_hit {:.2}, subtree_snapshot_miss {:.2}, subtree_collision_rejected {:.2}, img_hit {:.2}, img_miss {:.2}, video_hit {:.2}, video_miss {:.2}, video_decode {:.2}",
            average_usize(&self.frames, |frame| frame.backend.draw_rect_count),
            average_usize(&self.frames, |frame| frame.backend.draw_text_count),
            average_usize(&self.frames, |frame| frame.backend.draw_bitmap_count),
            average_usize(&self.frames, |frame| frame.backend.draw_script_count),
            average_usize(&self.frames, |frame| frame.backend.save_layer_count),
            average_usize(&self.frames, |frame| frame.backend.text_cache_hits),
            average_usize(&self.frames, |frame| frame.backend.text_cache_misses),
            average_usize(&self.frames, |frame| frame.backend.item_picture_cache_hits),
            average_usize(&self.frames, |frame| frame
                .backend
                .item_picture_cache_misses),
            average_usize(&self.frames, |frame| frame
                .backend
                .scene_snapshot_cache_hits),
            average_usize(&self.frames, |frame| frame
                .backend
                .scene_snapshot_cache_misses),
            average_usize(&self.frames, |frame| frame
                .backend
                .subtree_snapshot_cache_hits),
            average_usize(&self.frames, |frame| frame
                .backend
                .subtree_snapshot_cache_misses),
            average_usize(&self.frames, |frame| frame
                .backend
                .subtree_snapshot_collision_rejected),
            average_usize(&self.frames, |frame| frame.backend.image_cache_hits),
            average_usize(&self.frames, |frame| frame.backend.image_cache_misses),
            average_usize(&self.frames, |frame| frame.backend.video_frame_cache_hits),
            average_usize(&self.frames, |frame| frame.backend.video_frame_cache_misses),
            average_usize(&self.frames, |frame| frame.backend.video_frame_decodes),
        );
        self.print_backend_span_summary();
    }

    fn print_backend_span_summary(&self) {
        let mut aggregate = BTreeMap::<BackendSpanKey, BackendSpanAggregate>::new();
        for frame in &self.frames {
            for (key, value) in &frame.backend_spans {
                aggregate.entry(*key).or_default().merge(value);
            }
        }

        if aggregate.is_empty() {
            return;
        }

        eprintln!("  backend avg spans/frame:");
        let mut roots = aggregate
            .iter()
            .filter_map(|(key, value)| {
                (key.depth == 0 && key.parent.is_none()).then_some((*key, *value))
            })
            .collect::<Vec<_>>();
        roots.sort_by(|(left_key, left), (right_key, right)| {
            right
                .inclusive_ms
                .partial_cmp(&left.inclusive_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left_key.name.cmp(right_key.name))
        });

        for (key, _) in roots {
            print_backend_span_node(self.frames.len(), &aggregate, key);
        }
    }
}

fn print_backend_span_node(
    frame_count: usize,
    aggregate: &BTreeMap<BackendSpanKey, BackendSpanAggregate>,
    key: BackendSpanKey,
) {
    let Some(value) = aggregate.get(&key) else {
        return;
    };

    let indent = 2 + key.depth * 2;
    let padding = " ".repeat(indent);
    eprintln!(
        "{}{}: incl {:.2}, excl {:.2}, calls {:.2}",
        padding,
        key.name,
        value.inclusive_ms / frame_count as f64,
        value.exclusive_ms / frame_count as f64,
        value.count as f64 / frame_count as f64,
    );

    let mut children = aggregate
        .iter()
        .filter_map(|(child_key, child)| {
            (child_key.depth == key.depth + 1 && child_key.parent == Some(key.name))
                .then_some((*child_key, *child))
        })
        .collect::<Vec<_>>();
    children.sort_by(|(left_key, left), (right_key, right)| {
        right
            .inclusive_ms
            .partial_cmp(&left.inclusive_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left_key.name.cmp(right_key.name))
    });

    for (child_key, _) in children {
        print_backend_span_node(frame_count, aggregate, child_key);
    }
}

fn average(frames: &[FrameProfile], map: impl Fn(&FrameProfile) -> f64) -> f64 {
    frames.iter().map(map).sum::<f64>() / frames.len() as f64
}

fn average_usize(frames: &[FrameProfile], map: impl Fn(&FrameProfile) -> usize) -> f64 {
    frames.iter().map(map).sum::<usize>() as f64 / frames.len() as f64
}

fn percentile_95(frames: &[FrameProfile], map: impl Fn(&FrameProfile) -> f64) -> f64 {
    let mut values = frames.iter().map(map).collect::<Vec<_>>();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = ((values.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(values.len() - 1);
    values[index]
}

fn average_when_counted(
    frames: &[FrameProfile],
    value: impl Fn(&FrameProfile) -> f64,
    count: impl Fn(&FrameProfile) -> usize,
) -> f64 {
    let total_count = frames.iter().map(count).sum::<usize>();
    if total_count == 0 {
        return 0.0;
    }
    frames.iter().map(value).sum::<f64>() / total_count as f64
}
