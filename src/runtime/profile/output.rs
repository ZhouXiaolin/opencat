use std::collections::BTreeMap;

use crate::runtime::profile::{
    BackendSpanAggregate, BackendSpanKey, FrameProfile, ProfileConfig, ProfileOutputFormat,
    RenderProfileSummary,
};

pub(crate) fn render_profile_text(summary: &RenderProfileSummary) -> String {
    let frame_count = summary.frames.len();
    if frame_count == 0 {
        return String::new();
    }
    let mut out = String::new();
    out.push_str("Render profile:\n");
    out.push_str(&format!("  frames: {}\n", frame_count));
    out.push_str(&format!(
        "  avg ms/frame: script {:.2}, frame_state {:.2}, resolve {:.2}, layout {:.2}, display {:.2}, backend {:.2}, transition {:.2}\n",
        average(summary, |frame| frame.script_ms),
        average(summary, |frame| frame.frame_state_ms),
        average(summary, |frame| frame.resolve_ms),
        average(summary, |frame| frame.layout_ms),
        average(summary, |frame| frame.display_ms),
        average(summary, |frame| frame.backend_ms),
        average(summary, |frame| frame.transition_ms),
    ));
    out.push_str(&format!(
        "  p95 ms/frame: resolve {:.2}, layout {:.2}, display {:.2}, backend {:.2}, transition {:.2}\n",
        percentile_95(summary, |frame| frame.resolve_ms),
        percentile_95(summary, |frame| frame.layout_ms),
        percentile_95(summary, |frame| frame.display_ms),
        percentile_95(summary, |frame| frame.backend_ms),
        percentile_95(summary, |frame| frame.transition_ms),
    ));
    out.push_str(&format!(
        "  transition avg ms/active-frame: slide {:.2} ({} frames), light_leak {:.2} ({} frames)\n",
        average_when_counted(
            summary,
            |frame| frame.slide_transition_ms,
            |frame| frame.slide_transition_frames,
        ),
        summary
            .frames
            .values()
            .map(|f| f.slide_transition_frames)
            .sum::<usize>(),
        average_when_counted(
            summary,
            |frame| frame.light_leak_transition_ms,
            |frame| frame.light_leak_transition_frames,
        ),
        summary
            .frames
            .values()
            .map(|f| f.light_leak_transition_frames)
            .sum::<usize>(),
    ));
    out.push_str(&format!(
        "  avg nodes/frame: reused {:.1}, layout_dirty {:.1}, raster_dirty {:.1}, composite_dirty {:.1}, structure_rebuilds {:.2}\n",
        average_usize(summary, |frame| frame.reused_nodes),
        average_usize(summary, |frame| frame.layout_dirty_nodes),
        average_usize(summary, |frame| frame.raster_dirty_nodes),
        average_usize(summary, |frame| frame.composite_dirty_nodes),
        average_usize(summary, |frame| frame.structure_rebuilds),
    ));
    out.push_str(&format!(
        "  backend avg ms/frame: subtree_snapshot_record {:.2}, subtree_snapshot_draw {:.2}, subtree_image_rasterize {:.2}, subtree_image_draw {:.2}, light_leak_mask {:.2}, light_leak_composite {:.2}\n",
        average(summary, |frame| frame.backend.subtree_snapshot_record_ms),
        average(summary, |frame| frame.backend.subtree_snapshot_draw_ms),
        average(summary, |frame| frame.backend.subtree_image_rasterize_ms),
        average(summary, |frame| frame.backend.subtree_image_draw_ms),
        average(summary, |frame| frame.backend.light_leak_mask_ms),
        average(summary, |frame| frame.backend.light_leak_composite_ms),
    ));
    out.push_str(&format!(
        "  backend avg counts/frame: rect {:.1}, text {:.1}, bitmap {:.1}, draw_script {:.1}, save_layer {:.1}, glyph_path_hit {:.2}, glyph_path_miss {:.2}, glyph_img_hit {:.2}, glyph_img_miss {:.2}, item_hit {:.2}, item_miss {:.2}, scene_snapshot_hit {:.2}, scene_snapshot_miss {:.2}, subtree_snapshot_hit {:.2}, subtree_snapshot_miss {:.2}, subtree_dirty_hit {:.2}, subtree_dirty_miss {:.2}, subtree_collision_rejected {:.2}, subtree_image_hit {:.2}, subtree_image_miss {:.2}, subtree_image_promote {:.2}, img_hit {:.2}, img_miss {:.2}, video_hit {:.2}, video_miss {:.2}, video_decode {:.2}\n",
        average_usize(summary, |frame| frame.backend.draw_rect_count),
        average_usize(summary, |frame| frame.backend.draw_text_count),
        average_usize(summary, |frame| frame.backend.draw_bitmap_count),
        average_usize(summary, |frame| frame.backend.draw_script_count),
        average_usize(summary, |frame| frame.backend.save_layer_count),
        average_usize(summary, |frame| frame.backend.glyph_path_cache_hits),
        average_usize(summary, |frame| frame.backend.glyph_path_cache_misses),
        average_usize(summary, |frame| frame.backend.glyph_image_cache_hits),
        average_usize(summary, |frame| frame.backend.glyph_image_cache_misses),
        average_usize(summary, |frame| frame.backend.item_picture_cache_hits),
        average_usize(summary, |frame| frame.backend.item_picture_cache_misses),
        average_usize(summary, |frame| frame.backend.scene_snapshot_cache_hits),
        average_usize(summary, |frame| frame.backend.scene_snapshot_cache_misses),
        average_usize(summary, |frame| frame.backend.subtree_snapshot_cache_hits),
        average_usize(summary, |frame| frame.backend.subtree_snapshot_cache_misses),
        average_usize(summary, |frame| frame.backend.subtree_snapshot_composite_dirty_hits),
        average_usize(summary, |frame| frame.backend.subtree_snapshot_composite_dirty_misses),
        average_usize(summary, |frame| frame.backend.subtree_snapshot_collision_rejected),
        average_usize(summary, |frame| frame.backend.subtree_image_cache_hits),
        average_usize(summary, |frame| frame.backend.subtree_image_cache_misses),
        average_usize(summary, |frame| frame.backend.subtree_image_promotions),
        average_usize(summary, |frame| frame.backend.image_cache_hits),
        average_usize(summary, |frame| frame.backend.image_cache_misses),
        average_usize(summary, |frame| frame.backend.video_frame_cache_hits),
        average_usize(summary, |frame| frame.backend.video_frame_cache_misses),
        average_usize(summary, |frame| frame.backend.video_frame_decodes),
    ));
    out.push_str(&format!(
        "  cache pressure avg/frame: item_evict {:.2}, item_repeat {:.2}, item_util {:.2}, subtree_evict {:.2}, subtree_repeat {:.2}, subtree_util {:.2}, subtree_image_evict {:.2}, subtree_image_repeat {:.2}, subtree_image_util {:.2}, glyph_path_evict {:.2}, glyph_path_repeat {:.2}, glyph_path_util {:.2}, image_evict {:.2}, image_repeat {:.2}, image_util {:.2}\n",
        average_usize(summary, |frame| frame.backend.item_picture_cache_evictions),
        average_usize(summary, |frame| frame.backend.item_picture_cache_record_repeats),
        average_usize(summary, |frame| frame.backend.item_picture_cache_capacity_utilization),
        average_usize(summary, |frame| frame.backend.subtree_snapshot_cache_evictions),
        average_usize(summary, |frame| frame.backend.subtree_snapshot_cache_record_repeats),
        average_usize(summary, |frame| frame.backend.subtree_snapshot_cache_capacity_utilization),
        average_usize(summary, |frame| frame.backend.subtree_image_cache_evictions),
        average_usize(summary, |frame| frame.backend.subtree_image_cache_record_repeats),
        average_usize(summary, |frame| frame.backend.subtree_image_cache_capacity_utilization),
        average_usize(summary, |frame| frame.backend.glyph_path_cache_evictions),
        average_usize(summary, |frame| frame.backend.glyph_path_cache_record_repeats),
        average_usize(summary, |frame| frame.backend.glyph_path_cache_capacity_utilization),
        average_usize(summary, |frame| frame.backend.image_cache_evictions),
        average_usize(summary, |frame| frame.backend.image_cache_record_repeats),
        average_usize(summary, |frame| frame.backend.image_cache_capacity_utilization),
    ));
    append_backend_span_summary(&mut out, summary);
    out
}

pub(crate) fn render_profile_json(summary: &RenderProfileSummary) -> anyhow::Result<String> {
    let frame_count = summary.frames.len();
    let json = if frame_count > 0 {
        format!(
            r#"{{"type":"render_profile_summary","frames":{},"avg_ms_per_frame":{{"script":{:.2},"frame_state":{:.2},"resolve":{:.2},"layout":{:.2},"display":{:.2},"backend":{:.2},"transition":{:.2}}}}}"#,
            frame_count,
            average(summary, |f| f.script_ms),
            average(summary, |f| f.frame_state_ms),
            average(summary, |f| f.resolve_ms),
            average(summary, |f| f.layout_ms),
            average(summary, |f| f.display_ms),
            average(summary, |f| f.backend_ms),
            average(summary, |f| f.transition_ms),
        )
    } else {
        r#"{"type":"render_profile_summary","frames":0}"#.to_string()
    };
    Ok(json)
}

pub(crate) fn print_profile_summary(
    summary: &RenderProfileSummary,
    config: &ProfileConfig,
) -> anyhow::Result<()> {
    match config.output_format {
        ProfileOutputFormat::Text => {
            eprintln!("{}", render_profile_text(summary));
        }
        ProfileOutputFormat::Json => {
            eprintln!("{}", render_profile_json(summary)?);
        }
        ProfileOutputFormat::Both => {
            eprintln!("{}", render_profile_text(summary));
            eprintln!("{}", render_profile_json(summary)?);
        }
    }
    Ok(())
}

fn average(summary: &RenderProfileSummary, map: impl Fn(&FrameProfile) -> f64) -> f64 {
    if summary.frames.is_empty() {
        return 0.0;
    }
    summary.frames.values().map(map).sum::<f64>() / summary.frames.len() as f64
}

fn average_usize(summary: &RenderProfileSummary, map: impl Fn(&FrameProfile) -> usize) -> f64 {
    if summary.frames.is_empty() {
        return 0.0;
    }
    summary.frames.values().map(map).sum::<usize>() as f64 / summary.frames.len() as f64
}

fn percentile_95(summary: &RenderProfileSummary, map: impl Fn(&FrameProfile) -> f64) -> f64 {
    let mut values: Vec<f64> = summary.frames.values().map(map).collect();
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = ((values.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(values.len() - 1);
    values[index]
}

fn average_when_counted(
    summary: &RenderProfileSummary,
    value: impl Fn(&FrameProfile) -> f64,
    count: impl Fn(&FrameProfile) -> usize,
) -> f64 {
    let total_count = summary.frames.values().map(count).sum::<usize>();
    if total_count == 0 {
        return 0.0;
    }
    summary.frames.values().map(value).sum::<f64>() / total_count as f64
}

fn append_backend_span_summary(out: &mut String, summary: &RenderProfileSummary) {
    let mut aggregate = BTreeMap::<BackendSpanKey, BackendSpanAggregate>::new();
    for frame in summary.frames.values() {
        for (key, value) in &frame.backend_spans {
            let entry = aggregate.entry(*key).or_default();
            entry.inclusive_ms += value.inclusive_ms;
            entry.exclusive_ms += value.exclusive_ms;
            entry.count += value.count;
        }
    }

    if aggregate.is_empty() {
        return;
    }

    let frame_count = summary.frames.len();
    out.push_str("  backend avg spans/frame:\n");

    let mut roots: Vec<(BackendSpanKey, BackendSpanAggregate)> = aggregate
        .iter()
        .filter_map(|(key, value)| {
            (key.depth == 0 && key.parent.is_none()).then_some((*key, *value))
        })
        .collect();
    roots.sort_by(|(left_key, left), (right_key, right)| {
        right
            .inclusive_ms
            .partial_cmp(&left.inclusive_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left_key.name.cmp(right_key.name))
    });

    for (key, _) in &roots {
        append_backend_span_node(out, frame_count, &aggregate, *key);
    }
}

fn append_backend_span_node(
    out: &mut String,
    frame_count: usize,
    aggregate: &BTreeMap<BackendSpanKey, BackendSpanAggregate>,
    key: BackendSpanKey,
) {
    let Some(value) = aggregate.get(&key) else {
        return;
    };

    let indent = 2 + key.depth * 2;
    let padding = " ".repeat(indent);
    out.push_str(&format!(
        "{}{}: incl {:.2}, excl {:.2}, calls {:.2}\n",
        padding,
        key.name,
        value.inclusive_ms / frame_count as f64,
        value.exclusive_ms / frame_count as f64,
        value.count as f64 / frame_count as f64,
    ));

    let mut children: Vec<(BackendSpanKey, BackendSpanAggregate)> = aggregate
        .iter()
        .filter_map(|(child_key, child)| {
            (child_key.depth == key.depth + 1 && child_key.parent == Some(key.name))
                .then_some((*child_key, *child))
        })
        .collect();
    children.sort_by(|(left_key, left), (right_key, right)| {
        right
            .inclusive_ms
            .partial_cmp(&left.inclusive_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left_key.name.cmp(right_key.name))
    });

    for (child_key, _) in &children {
        append_backend_span_node(out, frame_count, aggregate, *child_key);
    }
}

#[cfg(test)]
mod tests {
    use super::{render_profile_json, render_profile_text};
    use crate::runtime::profile::{BackendSpanAggregate, BackendSpanKey, RenderProfileSummary};

    #[test]
    fn text_output_contains_expected_sections() {
        let mut summary = RenderProfileSummary::default();
        let frame = summary.frames.entry(0).or_default();
        frame.resolve_ms = 16.61;
        frame.layout_ms = 0.57;
        frame.display_ms = 0.19;
        frame.backend_ms = 77.29;
        frame.transition_ms = 3.71;
        frame.backend_spans.insert(
            BackendSpanKey {
                depth: 0,
                parent: None,
                name: "display_tree_direct_draw",
            },
            BackendSpanAggregate {
                inclusive_ms: 77.27,
                exclusive_ms: 0.10,
                count: 1,
            },
        );

        let text = render_profile_text(&summary);

        assert!(text.contains("Render profile:"));
        assert!(text.contains("avg ms/frame"));
        assert!(text.contains("backend avg spans/frame"));
    }

    #[test]
    fn json_output_contains_summary_type_and_frame_count() {
        let summary = RenderProfileSummary::default();
        let json = render_profile_json(&summary).expect("json should serialize");

        assert!(json.contains("\"type\":\"render_profile_summary\""));
        assert!(json.contains("\"frames\":0"));
    }
}
