//! Engine-side audio contract.
//!
//! AudioPlan is the **sole composition-level canonical output** — core derives
//! typed microsecond-range segments from timeline/scene/transition offsets and
//! stores them on [`opencat_core::ir::CompositionInfo::audio_plan`] (see
//! [`opencat_core::media::collect_audio_plan`]). The engine exclusively reads
//! `pipeline.info().audio_plan` for decode/mix/encode and never re-walks the
//! composition tree to produce a second set of audio semantics.
//!
//! Decode, seek, mix, cache, and export are engine concerns; the plan itself
//! is a pure-core derivation with no IO or probe dependencies.
