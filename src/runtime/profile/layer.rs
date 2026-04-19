use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::Result;
use tracing::{Id, Subscriber, dispatcher::Dispatch};
use tracing_subscriber::{
    layer::{Context, Layer, SubscriberExt},
    registry::LookupSpan,
    Registry,
};

use super::{CompletedProfileSpan, RenderProfileAggregator, RenderProfileSummary};

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
}

struct ProfileFieldVisitor<'a> {
    span_fields: Option<&'a mut SpanFields>,
}

impl tracing::field::Visit for ProfileFieldVisitor<'_> {
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "frame" {
            if let Some(span_fields) = self.span_fields.as_mut() {
                span_fields.frame = Some(value as u32);
            }
        }
    }

    fn record_i64(&mut self, _field: &tracing::field::Field, _value: i64) {}

    fn record_bool(&mut self, _field: &tracing::field::Field, _value: bool) {}

    fn record_str(&mut self, _field: &tracing::field::Field, _value: &str) {}

    fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {}
}

impl<S> Layer<S> for RenderProfileLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &Id,
        ctx: Context<'_, S>,
    ) {
        let metadata = attrs.metadata();
        let parent_id = attrs.parent().cloned();
        let parent_name: Option<&'static str> = parent_id.as_ref().and_then(|pid| {
            ctx.span(pid).map(|span| span.metadata().name())
        });
        let inherited_frame = parent_id.as_ref().and_then(|pid| {
            self.shared
                .lock()
                .expect("profile state lock")
                .spans
                .get(pid)
                .and_then(|state| state.frame)
        });

        let mut fields = SpanFields::default();
        let mut visitor = ProfileFieldVisitor {
            span_fields: Some(&mut fields),
        };
        attrs.record(&mut visitor);

        let frame = if metadata.target() == "render.pipeline" && metadata.name() == "frame" {
            fields.frame.or(inherited_frame)
        } else {
            inherited_frame
        };

        self.shared.lock().expect("profile state lock").spans.insert(
            id.clone(),
            SpanState {
                name: metadata.name(),
                target: metadata.target(),
                parent: parent_name,
                parent_id,
                frame,
                started: Instant::now(),
                child_inclusive_ms: 0.0,
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
        shared.aggregator.record_span(CompletedProfileSpan {
            frame,
            target: state.target,
            name: state.name,
            parent: state.parent,
            inclusive_ms,
            exclusive_ms,
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
    use super::{ProfileConfig, ProfileOutputFormat, profile_render};
    use anyhow::Result;
    use tracing::{Level, span};

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
}
