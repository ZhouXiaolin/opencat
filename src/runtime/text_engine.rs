//! Deprecated: text engine moved to `crate::text` with cosmic-text backend.
//! This file is kept temporarily for compatibility and will be deleted when
//! all callers migrate.

pub(crate) use crate::text::{SharedTextMeasurer as SharedTextEngine, TextMeasureRequest, TextMeasurement, TextMeasurer as TextEngine};
