use crate::layout::LayoutPassStats;

#[derive(Clone, Debug, Default)]
pub struct BackendProfile {
    pub rect_draw_ms: f64,
    pub text_draw_ms: f64,
    pub text_snapshot_record_ms: f64,
    pub text_snapshot_draw_ms: f64,
    pub bitmap_draw_ms: f64,
    pub draw_script_draw_ms: f64,
    pub image_decode_ms: f64,
    pub video_decode_ms: f64,
    pub scene_snapshot_record_ms: f64,
    pub scene_snapshot_draw_ms: f64,
    pub light_leak_mask_ms: f64,
    pub light_leak_composite_ms: f64,
    pub scene_snapshot_cache_hits: usize,
    pub scene_snapshot_cache_misses: usize,
    pub subtree_snapshot_cache_hits: usize,
    pub subtree_snapshot_cache_misses: usize,
    pub text_cache_hits: usize,
    pub text_cache_misses: usize,
    pub image_cache_hits: usize,
    pub image_cache_misses: usize,
    pub video_frame_decodes: usize,
    pub draw_rect_count: usize,
    pub draw_text_count: usize,
    pub draw_bitmap_count: usize,
    pub draw_script_count: usize,
    pub save_layer_count: usize,
}

#[derive(Default)]
pub(crate) struct SceneBuildStats {
    pub resolve_ms: f64,
    pub layout_ms: f64,
    pub display_ms: f64,
    pub layout_pass: LayoutPassStats,
    pub contains_video: bool,
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

    pub(crate) fn merge_backend_profile(&mut self, profile: &BackendProfile) {
        self.backend.rect_draw_ms += profile.rect_draw_ms;
        self.backend.text_draw_ms += profile.text_draw_ms;
        self.backend.text_snapshot_record_ms += profile.text_snapshot_record_ms;
        self.backend.text_snapshot_draw_ms += profile.text_snapshot_draw_ms;
        self.backend.bitmap_draw_ms += profile.bitmap_draw_ms;
        self.backend.draw_script_draw_ms += profile.draw_script_draw_ms;
        self.backend.image_decode_ms += profile.image_decode_ms;
        self.backend.video_decode_ms += profile.video_decode_ms;
        self.backend.scene_snapshot_record_ms += profile.scene_snapshot_record_ms;
        self.backend.scene_snapshot_draw_ms += profile.scene_snapshot_draw_ms;
        self.backend.light_leak_mask_ms += profile.light_leak_mask_ms;
        self.backend.light_leak_composite_ms += profile.light_leak_composite_ms;
        self.backend.scene_snapshot_cache_hits += profile.scene_snapshot_cache_hits;
        self.backend.scene_snapshot_cache_misses += profile.scene_snapshot_cache_misses;
        self.backend.subtree_snapshot_cache_hits += profile.subtree_snapshot_cache_hits;
        self.backend.subtree_snapshot_cache_misses += profile.subtree_snapshot_cache_misses;
        self.backend.text_cache_hits += profile.text_cache_hits;
        self.backend.text_cache_misses += profile.text_cache_misses;
        self.backend.image_cache_hits += profile.image_cache_hits;
        self.backend.image_cache_misses += profile.image_cache_misses;
        self.backend.video_frame_decodes += profile.video_frame_decodes;
        self.backend.draw_rect_count += profile.draw_rect_count;
        self.backend.draw_text_count += profile.draw_text_count;
        self.backend.draw_bitmap_count += profile.draw_bitmap_count;
        self.backend.draw_script_count += profile.draw_script_count;
        self.backend.save_layer_count += profile.save_layer_count;
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
            "  backend avg ms/frame: rect {:.2}, text {:.2}, text_snapshot_record {:.2}, text_snapshot_draw {:.2}, bitmap {:.2}, draw_script {:.2}, image_decode {:.2}, video_decode {:.2}, scene_snapshot_record {:.2}, scene_snapshot_draw {:.2}, light_leak_mask {:.2}, light_leak_composite {:.2}",
            average(&self.frames, |frame| frame.backend.rect_draw_ms),
            average(&self.frames, |frame| frame.backend.text_draw_ms),
            average(&self.frames, |frame| frame.backend.text_snapshot_record_ms),
            average(&self.frames, |frame| frame.backend.text_snapshot_draw_ms),
            average(&self.frames, |frame| frame.backend.bitmap_draw_ms),
            average(&self.frames, |frame| frame.backend.draw_script_draw_ms),
            average(&self.frames, |frame| frame.backend.image_decode_ms),
            average(&self.frames, |frame| frame.backend.video_decode_ms),
            average(&self.frames, |frame| frame.backend.scene_snapshot_record_ms),
            average(&self.frames, |frame| frame.backend.scene_snapshot_draw_ms),
            average(&self.frames, |frame| frame.backend.light_leak_mask_ms),
            average(&self.frames, |frame| frame.backend.light_leak_composite_ms),
        );
        eprintln!(
            "  backend avg counts/frame: rect {:.1}, text {:.1}, bitmap {:.1}, draw_script {:.1}, save_layer {:.1}, text_hit {:.2}, text_miss {:.2}, scene_snapshot_hit {:.2}, scene_snapshot_miss {:.2}, subtree_snapshot_hit {:.2}, subtree_snapshot_miss {:.2}, img_hit {:.2}, img_miss {:.2}, video_decode {:.2}",
            average_usize(&self.frames, |frame| frame.backend.draw_rect_count),
            average_usize(&self.frames, |frame| frame.backend.draw_text_count),
            average_usize(&self.frames, |frame| frame.backend.draw_bitmap_count),
            average_usize(&self.frames, |frame| frame.backend.draw_script_count),
            average_usize(&self.frames, |frame| frame.backend.save_layer_count),
            average_usize(&self.frames, |frame| frame.backend.text_cache_hits),
            average_usize(&self.frames, |frame| frame.backend.text_cache_misses),
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
            average_usize(&self.frames, |frame| frame.backend.image_cache_hits),
            average_usize(&self.frames, |frame| frame.backend.image_cache_misses),
            average_usize(&self.frames, |frame| frame.backend.video_frame_decodes),
        );
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
