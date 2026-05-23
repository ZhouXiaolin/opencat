//! Shared JS script runtime files.
//!
//! These JS files implement the user-facing animation and style API
//! (`ctx.getNode()`, `ctx.animate()`, `ctx.canvas()`, etc.) and are
//! evaluated by both the desktop engine (via QuickJS) and the web
//! frontend (via the browser's native JS engine).
//!
//! The files call into platform-specific "native" functions
//! (`__record_*`, `__canvas_*`, `__animate_*`) which are provided
//! by the host environment.

pub mod animate;
pub mod bindings;
pub mod dispatch;
pub mod helpers;
pub mod host;
pub mod js_context;
pub mod mutations;
pub mod precomputed_host;
pub mod recorder;
pub mod runtime;
pub mod runtime_cache;
pub mod script_runner;
pub mod text_units;

pub use host::{ScriptDriverId, ScriptHost, driver_id_from_source};
pub use mutations::*;
pub use precomputed_host::PrecomputedScriptHost;
pub use runtime_cache::ScriptRuntimeCache;
pub use script_runner::ScriptRunner;

use crate::style::{
    AlignItems, BoxShadow, BoxShadowStyle, DropShadow, DropShadowStyle, FlexDirection, InsetShadow,
    InsetShadowStyle, JustifyContent, ObjectFit, Position, TextAlign,
};

#[derive(Clone, Debug, Default)]
pub struct ScriptDriver {
    pub source: String,
}

impl ScriptDriver {
    pub fn from_source(source: &str) -> anyhow::Result<Self> {
        Ok(Self {
            source: source.to_string(),
        })
    }

    pub fn cache_key(&self) -> u64 {
        driver_id_from_source(&self.source).0
    }
}

pub fn driver_from_source(source: &str) -> anyhow::Result<ScriptDriver> {
    ScriptDriver::from_source(source)
}

#[derive(Debug, Clone)]
pub struct ScriptTextSource {
    pub text: String,
    pub kind: ScriptTextSourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptTextSourceKind {
    TextNode,
    Caption,
}

pub fn position_from_name(name: &str) -> Option<Position> {
    match name {
        "relative" => Some(Position::Relative),
        "absolute" => Some(Position::Absolute),
        _ => None,
    }
}

pub fn flex_direction_from_name(name: &str) -> Option<FlexDirection> {
    match name {
        "row" => Some(FlexDirection::Row),
        "col" | "column" => Some(FlexDirection::Col),
        _ => None,
    }
}

pub fn justify_content_from_name(name: &str) -> Option<JustifyContent> {
    match name {
        "start" => Some(JustifyContent::Start),
        "center" => Some(JustifyContent::Center),
        "end" => Some(JustifyContent::End),
        "between" => Some(JustifyContent::Between),
        "around" => Some(JustifyContent::Around),
        "evenly" => Some(JustifyContent::Evenly),
        _ => None,
    }
}

pub fn align_items_from_name(name: &str) -> Option<AlignItems> {
    match name {
        "start" => Some(AlignItems::Start),
        "center" => Some(AlignItems::Center),
        "end" => Some(AlignItems::End),
        "stretch" => Some(AlignItems::Stretch),
        _ => None,
    }
}

pub fn object_fit_from_name(name: &str) -> Option<ObjectFit> {
    match name {
        "contain" => Some(ObjectFit::Contain),
        "cover" => Some(ObjectFit::Cover),
        "fill" => Some(ObjectFit::Fill),
        _ => None,
    }
}

pub fn box_shadow_from_name(name: &str) -> Option<BoxShadow> {
    match name {
        "2xs" => Some(BoxShadow::from_style(BoxShadowStyle::TwoXs)),
        "xs" => Some(BoxShadow::from_style(BoxShadowStyle::Xs)),
        "sm" => Some(BoxShadow::from_style(BoxShadowStyle::Sm)),
        "base" | "default" => Some(BoxShadow::from_style(BoxShadowStyle::Base)),
        "md" => Some(BoxShadow::from_style(BoxShadowStyle::Md)),
        "lg" => Some(BoxShadow::from_style(BoxShadowStyle::Lg)),
        "xl" => Some(BoxShadow::from_style(BoxShadowStyle::Xl)),
        "2xl" => Some(BoxShadow::from_style(BoxShadowStyle::TwoXl)),
        "3xl" => Some(BoxShadow::from_style(BoxShadowStyle::ThreeXl)),
        _ => None,
    }
}

pub fn inset_shadow_from_name(name: &str) -> Option<InsetShadow> {
    match name {
        "2xs" => Some(InsetShadow::from_style(InsetShadowStyle::TwoXs)),
        "xs" => Some(InsetShadow::from_style(InsetShadowStyle::Xs)),
        "base" | "default" => Some(InsetShadow::from_style(InsetShadowStyle::Base)),
        "sm" => Some(InsetShadow::from_style(InsetShadowStyle::Sm)),
        "md" => Some(InsetShadow::from_style(InsetShadowStyle::Md)),
        _ => None,
    }
}

pub fn drop_shadow_from_name(name: &str) -> Option<DropShadow> {
    match name {
        "xs" => Some(DropShadow::from_style(DropShadowStyle::Xs)),
        "sm" => Some(DropShadow::from_style(DropShadowStyle::Sm)),
        "base" | "default" => Some(DropShadow::from_style(DropShadowStyle::Base)),
        "md" => Some(DropShadow::from_style(DropShadowStyle::Md)),
        "lg" => Some(DropShadow::from_style(DropShadowStyle::Lg)),
        "xl" => Some(DropShadow::from_style(DropShadowStyle::Xl)),
        "2xl" => Some(DropShadow::from_style(DropShadowStyle::TwoXl)),
        "3xl" => Some(DropShadow::from_style(DropShadowStyle::ThreeXl)),
        _ => None,
    }
}

pub fn text_align_from_name(name: &str) -> Option<TextAlign> {
    match name {
        "left" => Some(TextAlign::Left),
        "center" => Some(TextAlign::Center),
        "right" => Some(TextAlign::Right),
        _ => None,
    }
}
