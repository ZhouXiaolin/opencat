use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::Result;
use tracing::{Id, Subscriber, dispatcher::Dispatch};
use tracing_subscriber::{
    Registry,
    layer::{Context, Layer, SubscriberExt},
    registry::LookupSpan,
};

use super::{
    CompletedProfileSpan, ProfileCountEvent, RenderProfileAggregator, RenderProfileSummary,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfileOutputFormat {
    Text,
    Json,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfileConfig {
    pub enabled: bool,
    pub output_format: ProfileOutputFormat,
    pub emit_frame_records: bool,
}

impl ProfileConfig {
    pub(crate) fn from_env() -> Self {
        let enabled = std::env::var("OPENCAT_PROFILE")
            .map(|value| value == "1")
            .unwrap_or(false);
        let output_format = match std::env::var("OPENCAT_PROFILE_FORMAT").as_deref() {
            Ok("json") => ProfileOutputFormat::Json,
            Ok("both") => ProfileOutputFormat::Both,
            _ => ProfileOutputFormat::Text,
        };
        let emit_frame_records = std::env::var("OPENCAT_PROFILE_FRAMES")
            .map(|value| value == "1")
            .unwrap_or(false);
        Self {
            enabled,
            output_format,
            emit_frame_records,
        }
    }
}

#[derive(Debug)]
struct SpanState {
    name: &'static str,
    target: &'static str,
    parent: Option<&'static str>,
    parent_id: Option<Id>,
    frame: Option<u32>,
    started: Instant,
    child_inclusive_ms: f64,
    /// 仅对 target == "render.backend" 的 span 有值；0 表示该 backend span 没有
    /// render.backend 祖先（frame / transition 等上层不计入 backend 深度）。
    backend_depth: Option<usize>,
    /// 最近的 render.backend 祖先 span 的 name；仅对 backend span 本身有意义。
    backend_parent: Option<&'static str>,
    /// 仅对 render.transition::draw_transition span 有意义；其他 span 为 None。
    transition_kind: Option<&'static str>,
}

#[derive(Default)]
struct SharedState {
    spans: HashMap<Id, SpanState>,
    aggregator: RenderProfileAggregator,
}

#[derive(Clone, Default)]
pub(crate) struct RenderProfileLayer {
    shared: Arc<Mutex<SharedState>>,
}

impl RenderProfileLayer {
    fn take_summary(&self) -> RenderProfileSummary {
        let mut shared = self.shared.lock().expect("profile state lock");
        std::mem::take(&mut shared.aggregator).finish()
    }
}

#[derive(Default)]
struct SpanFields {
    frame: Option<u32>,
    transition_kind: Option<&'static str>,
}

#[derive(Default)]
struct EventFields {
    kind: Option<&'static str>,
    name: Option<&'static str>,
    result: Option<&'static str>,
    amount: Option<usize>,
}

struct ProfileFieldVisitor<'a> {
    span_fields: Option<&'a mut SpanFields>,
    event_fields: Option<&'a mut EventFields>,
}

impl tracing::field::Visit for ProfileFieldVisitor<'_> {
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        match field.name() {
            "frame" => {
                if let Some(span_fields) = self.span_fields.as_mut() {
                    span_fields.frame = Some(value as u32);
                }
            }
            "amount" => {
                if let Some(event_fields) = self.event_fields.as_mut() {
                    event_fields.amount = Some(value as usize);
                }
            }
            _ => {}
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let leaked: &'static str = Box::leak(value.to_string().into_boxed_str());
        match field.name() {
            "kind" => {
                if let Some(event_fields) = self.event_fields.as_mut() {
                    event_fields.kind = Some(leaked);
                }
            }
            "name" => {
                if let Some(event_fields) = self.event_fields.as_mut() {
                    event_fields.name = Some(leaked);
                }
            }
            "result" => {
                if let Some(event_fields) = self.event_fields.as_mut() {
                    event_fields.result = Some(leaked);
                }
            }
            "transition_kind" => {
                if let Some(span_fields) = self.span_fields.as_mut() {
                    span_fields.transition_kind = Some(leaked);
                }
            }
            _ => {}
        }
    }

    fn record_i64(&mut self, _field: &tracing::field::Field, _value: i64) {}

    fn record_bool(&mut self, _field: &tracing::field::Field, _value: bool) {}

    fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {}
}

impl<S> Layer<S> for RenderProfileLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let metadata = attrs.metadata();
        let parent_id = attrs
            .parent()
            .cloned()
            .or_else(|| ctx.current_span().id().cloned());
        let parent_name: Option<&'static str> = parent_id
            .as_ref()
            .and_then(|pid| ctx.span(pid).map(|span| span.metadata().name()));

        let mut fields = SpanFields::default();
        let mut visitor = ProfileFieldVisitor {
            span_fields: Some(&mut fields),
            event_fields: None,
        };
        attrs.record(&mut visitor);

        let is_backend = metadata.target() == "render.backend";
        let mut shared = self.shared.lock().expect("profile state lock");

        let inherited_frame = parent_id
            .as_ref()
            .and_then(|pid| shared.spans.get(pid).and_then(|state| state.frame));

        // 只在 render.backend 子树内累积 depth；跨过 frame / transition 祖先。
        let (backend_depth, backend_parent) = if is_backend {
            let mut cursor = parent_id.clone();
            let mut result: (Option<usize>, Option<&'static str>) = (Some(0), None);
            while let Some(pid) = cursor {
                match shared.spans.get(&pid) {
                    Some(parent_state) if parent_state.target == "render.backend" => {
                        let depth = parent_state.backend_depth.unwrap_or(0) + 1;
                        result = (Some(depth), Some(parent_state.name));
                        break;
                    }
                    Some(parent_state) => {
                        cursor = parent_state.parent_id.clone();
                    }
                    None => break,
                }
            }
            result
        } else {
            (None, None)
        };

        let frame = if metadata.target() == "render.pipeline" && metadata.name() == "frame" {
            fields.frame.or(inherited_frame)
        } else {
            inherited_frame
        };

        shared.spans.insert(
            id.clone(),
            SpanState {
                name: metadata.name(),
                target: metadata.target(),
                parent: parent_name,
                parent_id,
                frame,
                started: Instant::now(),
                child_inclusive_ms: 0.0,
                backend_depth,
                backend_parent,
                transition_kind: fields.transition_kind,
            },
        );
    }

    fn on_close(&self, id: Id, _ctx: Context<'_, S>) {
        let mut shared = self.shared.lock().expect("profile state lock");
        let Some(state) = shared.spans.remove(&id) else {
            return;
        };
        let Some(frame) = state.frame else {
            return;
        };

        let inclusive_ms = state.started.elapsed().as_secs_f64() * 1000.0;

        if let Some(parent_id) = &state.parent_id {
            if let Some(parent_state) = shared.spans.get_mut(parent_id) {
                parent_state.child_inclusive_ms += inclusive_ms;
            }
        }

        let exclusive_ms = (inclusive_ms - state.child_inclusive_ms).max(0.0);
        // backend span 只暴露 "最近的 backend 祖先" 作为语义 parent，避免把
        // render.pipeline::frame 之类的非 backend 祖先混进聚合树。
        let parent = if state.target == "render.backend" {
            state.backend_parent
        } else {
            state.parent
        };
        shared.aggregator.record_span(CompletedProfileSpan {
            frame,
            target: state.target,
            name: state.name,
            parent,
            inclusive_ms,
            exclusive_ms,
            backend_depth: state.backend_depth,
            transition_kind: state.transition_kind,
        });
    }

    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        if !matches!(
            metadata.target(),
            "render.cache" | "render.draw" | "render.layer"
        ) {
            return;
        }

        let mut fields = EventFields::default();
        let mut visitor = ProfileFieldVisitor {
            span_fields: None,
            event_fields: Some(&mut fields),
        };
        event.record(&mut visitor);

        let Some(kind) = fields.kind else { return };
        let Some(name) = fields.name else { return };
        let Some(result) = fields.result else { return };

        let mut shared = self.shared.lock().expect("profile state lock");
        let frame = shared
            .spans
            .values()
            .filter_map(|s| s.frame)
            .last()
            .unwrap_or(0);
        shared.aggregator.record_count(ProfileCountEvent {
            frame,
            target: metadata.target(),
            kind,
            name,
            result,
            amount: fields.amount.unwrap_or(1),
        });
    }
}

pub(crate) fn profile_render<T>(
    config: &ProfileConfig,
    f: impl FnOnce() -> Result<T>,
) -> Result<(T, Option<RenderProfileSummary>)> {
    if !config.enabled {
        return Ok((f()?, None));
    }

    let layer = RenderProfileLayer::default();
    let subscriber = Registry::default().with(layer.clone());
    let dispatch = Dispatch::new(subscriber);
    let result = tracing::dispatcher::with_default(&dispatch, f)?;
    Ok((result, Some(layer.take_summary())))
}

#[cfg(test)]
mod tests {
    use super::super::BackendSpanKey;
    use super::{ProfileConfig, ProfileOutputFormat, profile_render};
    use anyhow::Result;
    use tracing::{Level, span};

    #[test]
    fn backend_span_depth_ignores_non_backend_ancestors() -> Result<()> {
        // 契约：backend span 的 depth / parent 应该只在 render.backend 子树内计算。
        // frame (render.pipeline) → display_tree_direct_draw (render.backend)
        //                         → subtree_snapshot_record (render.backend)
        //
        // display_tree_direct_draw: depth=0, parent=None (跨过 "frame")
        // subtree_snapshot_record:  depth=1, parent=Some("display_tree_direct_draw")
        let config = ProfileConfig {
            enabled: true,
            output_format: ProfileOutputFormat::Text,
            emit_frame_records: false,
        };
        let (_, summary) = profile_render(&config, || {
            let frame = span!(
                target: "render.pipeline",
                Level::TRACE,
                "frame",
                frame = 0_u64,
                width = 100_i64,
                height = 100_i64,
                fps = 30_i64,
                mode = "scene"
            );
            let _frame_guard = frame.enter();

            let outer = span!(target: "render.backend", Level::TRACE, "display_tree_direct_draw");
            let _outer_guard = outer.enter();

            let inner = span!(target: "render.backend", Level::TRACE, "subtree_snapshot_record");
            let _inner_guard = inner.enter();
            Ok::<_, anyhow::Error>(())
        })?;

        let summary = summary.expect("summary should exist");
        let frame = summary.frames.get(&0).expect("frame summary exists");
        assert!(
            frame.backend_spans.contains_key(&BackendSpanKey {
                depth: 0,
                parent: None,
                name: "display_tree_direct_draw",
            }),
            "root backend span must not inherit `frame` as parent, spans = {:?}",
            frame.backend_spans.keys().collect::<Vec<_>>()
        );
        assert!(
            frame.backend_spans.contains_key(&BackendSpanKey {
                depth: 1,
                parent: Some("display_tree_direct_draw"),
                name: "subtree_snapshot_record",
            }),
            "nested backend span parent must be nearest backend ancestor, spans = {:?}",
            frame.backend_spans.keys().collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn profile_config_reads_env_flags() {
        unsafe {
            std::env::set_var("OPENCAT_PROFILE", "1");
            std::env::set_var("OPENCAT_PROFILE_FORMAT", "json");
            std::env::set_var("OPENCAT_PROFILE_FRAMES", "1");
        }

        let config = ProfileConfig::from_env();

        assert!(config.enabled);
        assert_eq!(config.output_format, ProfileOutputFormat::Json);
        assert!(config.emit_frame_records);

        unsafe {
            std::env::remove_var("OPENCAT_PROFILE");
            std::env::remove_var("OPENCAT_PROFILE_FORMAT");
            std::env::remove_var("OPENCAT_PROFILE_FRAMES");
        }
    }

    #[test]
    fn profile_render_returns_summary_only_when_enabled() -> Result<()> {
        let disabled = ProfileConfig {
            enabled: false,
            output_format: ProfileOutputFormat::Text,
            emit_frame_records: false,
        };
        let (_, disabled_summary) = profile_render(&disabled, || Ok::<_, anyhow::Error>(42))?;
        assert!(disabled_summary.is_none());

        let enabled = ProfileConfig {
            enabled: true,
            output_format: ProfileOutputFormat::Text,
            emit_frame_records: false,
        };
        let (_, enabled_summary) = profile_render(&enabled, || {
            let root = span!(target: "render.pipeline", Level::TRACE, "frame", frame = 0_u64, width = 1920_i64, height = 1080_i64, fps = 30_i64, mode = "scene");
            let _guard = root.enter();
            Ok::<_, anyhow::Error>(42)
        })?;

        assert!(enabled_summary.is_some());
        Ok(())
    }

    #[test]
    fn tracing_layer_captures_backend_spans_and_events() -> anyhow::Result<()> {
        use tracing::{Level, event, span};

        let config = ProfileConfig {
            enabled: true,
            output_format: ProfileOutputFormat::Text,
            emit_frame_records: false,
        };
        let (_, summary) = profile_render(&config, || {
            let frame_span = span!(
                target: "render.pipeline",
                Level::TRACE,
                "frame",
                frame = 7_u64,
                width = 1920_i64,
                height = 1080_i64,
                fps = 30_i64,
                mode = "scene"
            );
            let _frame_guard = frame_span.enter();
            let backend_span = span!(
                target: "render.backend",
                Level::TRACE,
                "subtree_snapshot_record"
            );
            let _backend_guard = backend_span.enter();
            event!(
                target: "render.cache",
                Level::TRACE,
                kind = "cache",
                name = "subtree_snapshot",
                result = "miss",
                amount = 1_u64
            );
            Ok::<_, anyhow::Error>(())
        })?;

        let summary = summary.expect("summary should exist");
        let frame = summary.frames.get(&7).expect("frame summary should exist");
        assert!(frame.backend.subtree_snapshot_record_ms >= 0.0);
        assert_eq!(frame.backend.subtree_snapshot_cache_misses, 1);
        Ok(())
    }

    #[test]
    fn tracing_layer_propagates_transition_kind() -> anyhow::Result<()> {
        use tracing::{Level, span};

        // 端到端契约：在 render.transition::draw_transition span 上挂 transition_kind 字段，
        // ProfileLayer 必须把它带进 CompletedProfileSpan，最终落到对应 frame 字段。
        // canvas.rs::draw_timeline_transition_subtree 是该 span 的实际发布点。
        let config = ProfileConfig {
            enabled: true,
            output_format: ProfileOutputFormat::Text,
            emit_frame_records: false,
        };
        let (_, summary) = profile_render(&config, || {
            let frame_span = span!(
                target: "render.pipeline",
                Level::TRACE,
                "frame",
                frame = 21_u64,
                width = 1280_i64,
                height = 720_i64,
                fps = 30_i64,
                mode = "scene"
            );
            let _frame_guard = frame_span.enter();
            let transition_span = span!(
                target: "render.transition",
                Level::TRACE,
                "draw_transition",
                transition_kind = "light_leak"
            );
            let _t_guard = transition_span.enter();
            Ok::<_, anyhow::Error>(())
        })?;

        let summary = summary.expect("summary should exist");
        let frame = summary.frames.get(&21).expect("frame summary should exist");
        assert_eq!(
            frame.light_leak_transition_frames, 1,
            "light_leak transition span should bump the active-frame count"
        );
        assert!(
            frame.light_leak_transition_ms > 0.0,
            "light_leak transition span should record positive ms, got {}",
            frame.light_leak_transition_ms
        );
        assert!(frame.transition_ms >= frame.light_leak_transition_ms);
        Ok(())
    }
}
