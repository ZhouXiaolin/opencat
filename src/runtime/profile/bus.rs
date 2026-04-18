use std::{
    cell::{Cell, RefCell},
    time::Instant,
};

thread_local! {
    static CURRENT_BACKEND_PROFILE_SINK: Cell<Option<BackendProfileSinkSlot>> = const { Cell::new(None) };
    static BACKEND_PROFILE_SINK_STACK: RefCell<Vec<Option<BackendProfileSinkSlot>>> = const { RefCell::new(Vec::new()) };
    static ACTIVE_BACKEND_PROFILE_SPANS: RefCell<Vec<ActiveBackendProfileSpan>> = const { RefCell::new(Vec::new()) };
    static PENDING_BACKEND_PROFILE_SPAN_EVENTS: RefCell<Vec<BackendProfileEvent>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BackendDurationMetric {
    RectDraw,
    TextDraw,
    TextSnapshotRecord,
    TextSnapshotDraw,
    ItemPictureRecord,
    ItemPictureDraw,
    BitmapDraw,
    DrawScriptDraw,
    ImageDecode,
    VideoDecode,
    SubtreeSnapshotRecord,
    SubtreeSnapshotDraw,
    LightLeakMask,
    LightLeakComposite,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BackendCountMetric {
    SceneSnapshotCacheHit,
    SceneSnapshotCacheMiss,
    SubtreeSnapshotCacheHit,
    SubtreeSnapshotCacheMiss,
    TextCacheHit,
    TextCacheMiss,
    ItemPictureCacheHit,
    ItemPictureCacheMiss,
    ImageCacheHit,
    ImageCacheMiss,
    VideoFrameCacheHit,
    VideoFrameCacheMiss,
    VideoFrameDecode,
    DrawRect,
    DrawText,
    DrawBitmap,
    DrawScript,
    SaveLayer,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BackendProfileEvent {
    Duration {
        metric: BackendDurationMetric,
        ms: f64,
    },
    Count {
        metric: BackendCountMetric,
        amount: usize,
    },
    SpanCompleted {
        depth: usize,
        name: &'static str,
        parent: Option<&'static str>,
        inclusive_ms: f64,
        exclusive_ms: f64,
    },
}

pub(crate) trait BackendProfileSink {
    fn record_backend_event(&mut self, event: BackendProfileEvent);
}

#[derive(Clone, Copy)]
struct BackendProfileSinkSlot {
    data: *mut (),
    dispatch: unsafe fn(*mut (), BackendProfileEvent),
}

struct ActiveBackendProfileSpan {
    name: &'static str,
    started: Instant,
    child_inclusive_ms: f64,
}

pub(crate) struct BackendProfileSpan {
    active: bool,
}

unsafe fn dispatch_backend_event<S: BackendProfileSink>(data: *mut (), event: BackendProfileEvent) {
    // SAFETY: `data` was created from `&mut S` in `with_backend_profile_sink` and remains valid
    // until the surrounding scope guard removes the slot from the thread-local stack.
    unsafe { (&mut *(data.cast::<S>())).record_backend_event(event) };
}

struct BackendProfileSinkScopeGuard;

impl Drop for BackendProfileSinkScopeGuard {
    fn drop(&mut self) {
        BACKEND_PROFILE_SINK_STACK.with(|stack| {
            let previous = stack.borrow_mut().pop().flatten();
            CURRENT_BACKEND_PROFILE_SINK.with(|current| {
                current.set(previous);
            });
        });
    }
}

impl Drop for BackendProfileSpan {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        let completed = ACTIVE_BACKEND_PROFILE_SPANS.with(|spans| {
            let mut spans = spans.borrow_mut();
            let Some(active_span) = spans.pop() else {
                return None;
            };
            let inclusive_ms = active_span.started.elapsed().as_secs_f64() * 1000.0;
            let exclusive_ms = (inclusive_ms - active_span.child_inclusive_ms).max(0.0);
            let depth = spans.len();
            let parent = spans.last_mut().map(|parent| {
                parent.child_inclusive_ms += inclusive_ms;
                parent.name
            });
            Some((depth, active_span.name, parent, inclusive_ms, exclusive_ms))
        });

        let Some((depth, name, parent, inclusive_ms, exclusive_ms)) = completed else {
            return;
        };

        PENDING_BACKEND_PROFILE_SPAN_EVENTS.with(|events| {
            events
                .borrow_mut()
                .push(BackendProfileEvent::SpanCompleted {
                    depth,
                    name,
                    parent,
                    inclusive_ms,
                    exclusive_ms,
                });
        });
    }
}

pub(crate) fn with_backend_profile_sink<T, S: BackendProfileSink>(
    sink: &mut S,
    f: impl FnOnce() -> T,
) -> T {
    let span_event_start = PENDING_BACKEND_PROFILE_SPAN_EVENTS.with(|events| events.borrow().len());
    let slot = BackendProfileSinkSlot {
        data: sink as *mut S as *mut (),
        dispatch: dispatch_backend_event::<S>,
    };
    let previous = CURRENT_BACKEND_PROFILE_SINK.with(|current| {
        let previous = current.get();
        current.set(Some(slot));
        previous
    });
    BACKEND_PROFILE_SINK_STACK.with(|stack| {
        stack.borrow_mut().push(previous);
    });
    let guard = BackendProfileSinkScopeGuard;
    let result = f();
    flush_pending_span_events(span_event_start);
    drop(guard);
    result
}

pub(crate) fn publish_backend_event(event: BackendProfileEvent) {
    CURRENT_BACKEND_PROFILE_SINK.with(|current| {
        if let Some(sink) = current.get() {
            // SAFETY: sink slots are installed by `with_backend_profile_sink` for the duration
            // of synchronous rendering work on the current thread and removed by the scope guard.
            unsafe { (sink.dispatch)(sink.data, event) };
        }
    });
}

pub(crate) fn record_backend_duration(metric: BackendDurationMetric, ms: f64) {
    publish_backend_event(BackendProfileEvent::Duration { metric, ms });
}

pub(crate) fn record_backend_elapsed(metric: BackendDurationMetric, started: Instant) {
    record_backend_duration(metric, started.elapsed().as_secs_f64() * 1000.0);
}

pub(crate) fn record_backend_count(metric: BackendCountMetric, amount: usize) {
    publish_backend_event(BackendProfileEvent::Count { metric, amount });
}

pub(crate) fn backend_span(name: &'static str) -> BackendProfileSpan {
    let active = CURRENT_BACKEND_PROFILE_SINK.with(|current| current.get().is_some());
    if active {
        ACTIVE_BACKEND_PROFILE_SPANS.with(|spans| {
            spans.borrow_mut().push(ActiveBackendProfileSpan {
                name,
                started: Instant::now(),
                child_inclusive_ms: 0.0,
            });
        });
    }
    BackendProfileSpan { active }
}

fn flush_pending_span_events(start: usize) {
    let pending = PENDING_BACKEND_PROFILE_SPAN_EVENTS.with(|events| {
        let mut events = events.borrow_mut();
        if start >= events.len() {
            return Vec::new();
        }
        events.drain(start..).collect::<Vec<_>>()
    });
    for event in pending {
        publish_backend_event(event);
    }
}
