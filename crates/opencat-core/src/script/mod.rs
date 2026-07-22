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
//!
//! Isolation unit (issue #20): one [`ScriptRealm`] per pipeline. Drivers
//! inside a composition share that realm; separate pipelines never share
//! ctx / dispatcher / globals. Hosts only implement [`js_context::JsContext`].

pub mod animate;
pub mod bindings;
pub mod dispatch;
pub mod helpers;
pub mod host;
pub mod js_context;
pub mod mutations;
pub mod precomputed_host;
pub mod realm;
pub mod recorder;
pub mod runtime;
pub mod text_units;

pub use host::{ScriptDriverId, ScriptHost, ScriptTargetRegistry, driver_id_from_source};
pub use mutations::*;
pub use precomputed_host::PrecomputedScriptHost;
pub use realm::ScriptRealm;

use crate::ir::asset_id::AssetId;
use crate::style::{
    AlignItems, BoxShadow, BoxShadowStyle, DropShadow, DropShadowStyle, FlexDirection, InsetShadow,
    InsetShadowStyle, JustifyContent, ObjectFit, Position, TextAlign,
};

/// External script file declared by logical path/url. Host supplies the text via
/// [`crate::lifecycle::HostInputs::insert_script_text`]; core never reads files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptExternalRef {
    pub asset_id: AssetId,
    /// Logical path or URL as authored (no host base-dir join).
    pub locator: String,
}

#[derive(Clone, Debug, Default)]
pub struct ScriptDriver {
    /// Final JS source. Inline scripts set this at parse; external scripts are
    /// empty until prepare injects host-supplied text.
    pub source: String,
    /// Primary external (single-path case). Prefer [`Self::externals`] for the
    /// full list used by prepare requirements.
    pub external: Option<ScriptExternalRef>,
    /// Ordered external locators when a node joins multiple path scripts (or
    /// mixes path + inline). Empty for pure-inline drivers.
    pub externals: Vec<ScriptExternalRef>,
    /// Inline fragments to re-join with external texts at prepare (order:
    /// all inlines first, then each external text — matches historical
    /// join_scripts for pure-inline; for mix, inlines prefix the externals).
    pub inline_fragments: Vec<String>,
}

impl ScriptDriver {
    pub fn from_source(source: &str) -> anyhow::Result<Self> {
        Ok(Self {
            source: source.to_string(),
            external: None,
            externals: Vec::new(),
            inline_fragments: Vec::new(),
        })
    }

    /// Declared external script. Source text is injected during lifecycle prepare.
    pub fn from_external(locator: impl Into<String>) -> Self {
        let locator = locator.into();
        let asset_id = asset_id_for_script_locator(&locator);
        let external = ScriptExternalRef {
            asset_id: asset_id.clone(),
            locator,
        };
        Self {
            source: String::new(),
            external: Some(external.clone()),
            externals: vec![external],
            inline_fragments: Vec::new(),
        }
    }

    /// Mixed or multi-external declaration. Prepare joins inline fragments with
    /// host-supplied external texts (in declaration order of externals).
    pub fn from_pieces(inline_fragments: Vec<String>, external_locators: Vec<String>) -> Self {
        let externals: Vec<ScriptExternalRef> = external_locators
            .into_iter()
            .map(|locator| {
                let asset_id = asset_id_for_script_locator(&locator);
                ScriptExternalRef { asset_id, locator }
            })
            .collect();
        let external = externals.first().cloned();
        // Stable install key from the ordered asset id list.
        let key_material = externals
            .iter()
            .map(|e| e.asset_id.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = key_material;
        Self {
            source: String::new(),
            external,
            externals,
            inline_fragments,
        }
    }

    pub fn cache_key(&self) -> u64 {
        self.driver_id().0
    }

    pub fn driver_id(&self) -> ScriptDriverId {
        if !self.externals.is_empty() {
            let key = self
                .externals
                .iter()
                .map(|e| e.asset_id.0.as_str())
                .collect::<Vec<_>>()
                .join("\0");
            driver_id_from_source(&key)
        } else if let Some(ext) = &self.external {
            driver_id_from_source(&ext.asset_id.0)
        } else {
            driver_id_from_source(&self.source)
        }
    }

    pub fn is_external_pending(&self) -> bool {
        !self.externals.is_empty() && self.source.is_empty()
    }

    /// Apply host-supplied script texts (keyed by AssetId string) and produce
    /// the final joined source. Missing externals leave source empty.
    pub fn resolve_with_host_texts(&mut self, texts: &std::collections::HashMap<String, String>) {
        if self.externals.is_empty() {
            return;
        }
        let mut parts = self.inline_fragments.clone();
        for ext in &self.externals {
            if let Some(t) = texts.get(&ext.asset_id.0) {
                parts.push(t.clone());
            } else {
                // Incomplete — leave source empty so prepare can fail-fast.
                self.source.clear();
                return;
            }
        }
        self.source = parts.join("\n");
    }
}

/// Canonical AssetId for a logical script locator (path or url).
pub fn asset_id_for_script_locator(locator: &str) -> AssetId {
    if locator.starts_with("http://") || locator.starts_with("https://") {
        AssetId(format!("script:url:{locator}"))
    } else {
        AssetId(format!("script:path:{locator}"))
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
