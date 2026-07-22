//! Lifecycle state types: unprepared draft → prepared composition.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::ir::asset_id::{AssetId, ResourceKind};
use crate::parse::ParsedComposition;
use crate::probe::catalog::PreparedResourceCatalog;

/// Unprepared composition: parsed tree plus the declarative host requirements
/// derived from it. Distinct from [`PreparedComposition`].
#[derive(Debug, Clone)]
pub struct CompositionDraft {
    pub(super) parsed: ParsedComposition,
    pub(super) requirements: HostRequirements,
}

/// Order-independent resource requirements the host must satisfy before prepare.
///
/// Every entry carries a namespaced [`AssetId`] that the host uses as the key
/// when returning metadata. Core never interprets locator strings; hosts derive
/// fetch strategy from the `AssetId` prefix (path / url / query) themselves.
#[derive(Debug, Clone)]
pub struct HostRequirements {
    pub(super) requests: Vec<ResourceRequest>,
}

/// One declared resource need, carrying the canonical identity the host must
/// use when returning metadata. Core never interprets locator strings — hosts
/// extract fetch strategy from the namespaced `AssetId` key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRequest {
    pub asset_id: AssetId,
    pub kind: ResourceKind,
}

/// Host-supplied inputs for prepare: resource metadata, optional subtitle text,
/// base font database, document font bytes, and external script texts. Building
/// inputs is fallible for duplicates; prepare is fallible for missing/undeclared
/// entries.
///
/// - `font_db` is the host base database (defaults / system faces). Core never
///   fetches fonts; it only merges declared document faces over this base.
/// - Document font bytes are keyed by the canonical font [`AssetId`] from
///   requirements (`font:path:…` / `font:url:…`), not by markup face id.
/// - External script texts are keyed by the canonical script [`AssetId`]
///   (`script:path:…` / `script:url:…`). Core injects them into drivers during
///   prepare and never reads script files itself (issue #20).
#[derive(Debug, Clone)]
pub struct HostInputs {
    /// Base/default font face bytes provided by the host. Core constructs
    /// the font database from these bytes during prepare, not from a
    /// prebuilt fontdb::Database.
    pub(super) base_font_faces: Vec<Vec<u8>>,
    /// Default sans-serif family name for the base font database.
    /// Defaults to "sans-serif" which fontdb resolves to its own fallback.
    pub(super) sans_serif_family: String,
    pub(super) catalog: PreparedResourceCatalog,
    pub(super) subtitle_texts: HashMap<AssetId, String>,
    /// Declared document font face bytes, keyed by stable font AssetId.
    pub(super) document_fonts: HashMap<AssetId, Vec<u8>>,
    /// Declared external script source texts, keyed by stable script AssetId.
    pub(super) script_texts: HashMap<AssetId, String>,
    pub(super) supplied: HashSet<AssetId>,
}

/// Prepared composition: validated host metadata applied, captions hydrated,
/// ready to open a pipeline. Distinct from [`CompositionDraft`].
#[derive(Debug, Clone)]
pub struct PreparedComposition {
    pub(super) parsed: ParsedComposition,
    pub(super) catalog: PreparedResourceCatalog,
    pub(super) font_db: Arc<fontdb::Database>,
}

/// Structured prepare / host-input failures. Hosts map these to their own error
/// surface.
///
/// - [`MissingInput`] / [`UndeclaredInput`] are raised by [`super::prepare`].
/// - [`DuplicateInput`] is raised when building [`HostInputs`] via `insert_*`
///   (the only public way to supply resource metadata), so prepare never sees a
///   second entry for the same id.
/// - Declared subtitles may omit text; that is not a missing-input error (empty
///   captions), matching `hydrate_captions`.
/// - Declared document fonts must be supplied as non-empty loadable bytes;
///   missing or empty fonts fail prepare (fail-fast).
/// - Declared external scripts must be supplied as non-empty text; missing
///   scripts fail prepare (fail-fast).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareError {
    /// A declared requirement has no matching host input.
    MissingInput {
        asset_id: AssetId,
        kind: ResourceKind,
    },
    /// The host supplied the same asset id more than once.
    DuplicateInput { asset_id: AssetId },
    /// The host supplied an asset id that is not in the draft requirements.
    UndeclaredInput { asset_id: AssetId },
    /// Host input is present but fails layout/time-critical validation
    /// (e.g. zero-size image, kind stored under the wrong catalog map).
    InvalidMetadata {
        asset_id: AssetId,
        kind: ResourceKind,
        reason: String,
    },
    /// Unexpected internal failure (e.g. caption parse). Rare on the pure path.
    Internal { message: String },
}

impl std::fmt::Display for PrepareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingInput { asset_id, kind } => {
                write!(
                    f,
                    "prepare missing host input for {kind:?} asset `{}`",
                    asset_id.key
                )
            }
            Self::DuplicateInput { asset_id } => {
                write!(
                    f,
                    "prepare duplicate host input for asset `{}`",
                    asset_id.key
                )
            }
            Self::UndeclaredInput { asset_id } => {
                write!(
                    f,
                    "prepare undeclared host input for asset `{}`",
                    asset_id.key
                )
            }
            Self::InvalidMetadata {
                asset_id,
                kind,
                reason,
            } => {
                write!(
                    f,
                    "prepare invalid {kind:?} metadata for asset `{}`: {reason}",
                    asset_id.key
                )
            }
            Self::Internal { message } => write!(f, "prepare internal error: {message}"),
        }
    }
}

impl std::error::Error for PrepareError {}
