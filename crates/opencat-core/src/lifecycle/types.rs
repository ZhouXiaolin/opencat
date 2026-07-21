//! Lifecycle state types: unprepared draft → prepared composition.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::ir::asset_id::AssetId;
use crate::parse::ParsedComposition;
use crate::probe::catalog::{ResourceCatalog, ResourceRequests};

/// Unprepared composition: parsed tree plus the declarative host requirements
/// derived from it. Distinct from [`PreparedComposition`].
#[derive(Debug, Clone)]
pub struct CompositionDraft {
    pub(super) parsed: ParsedComposition,
    pub(super) requirements: HostRequirements,
}

/// Order-independent resource requirements the host must satisfy before prepare.
#[derive(Debug, Clone)]
pub struct HostRequirements {
    pub(super) requests: Vec<ResourceRequest>,
    /// Back-compat surface for the existing probe/loader chain.
    pub(super) raw: ResourceRequests,
}

/// One declared resource need, carrying the canonical identity the host must
/// use when returning metadata. Locators are logical (no host base directory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRequest {
    pub asset_id: AssetId,
    pub kind: ResourceKind,
    pub locator: ResourceLocator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceKind {
    Image,
    Video,
    Audio,
    Subtitle,
    Lottie,
}

/// Logical resource location. Hosts interpret these against their own document
/// base (filesystem, VFS, URL) — core never resolves them to real paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceLocator {
    Unset,
    LogicalPath(String),
    Url(String),
    Query {
        query: String,
        count: usize,
        aspect_ratio: Option<String>,
    },
}

/// Host-supplied inputs for prepare: resource metadata, optional subtitle text,
/// and the base font database. Building inputs is fallible for duplicates;
/// prepare is fallible for missing/undeclared entries.
#[derive(Debug, Clone)]
pub struct HostInputs {
    pub(super) font_db: Arc<fontdb::Database>,
    pub(super) catalog: ResourceCatalog,
    pub(super) subtitle_texts: HashMap<String, String>,
    pub(super) supplied: HashSet<AssetId>,
}

/// Prepared composition: validated host metadata applied, captions hydrated,
/// ready to open a pipeline. Distinct from [`CompositionDraft`].
#[derive(Debug, Clone)]
pub struct PreparedComposition {
    pub(super) parsed: ParsedComposition,
    pub(super) catalog: ResourceCatalog,
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
                    asset_id.0
                )
            }
            Self::DuplicateInput { asset_id } => {
                write!(
                    f,
                    "prepare duplicate host input for asset `{}`",
                    asset_id.0
                )
            }
            Self::UndeclaredInput { asset_id } => {
                write!(
                    f,
                    "prepare undeclared host input for asset `{}`",
                    asset_id.0
                )
            }
            Self::Internal { message } => write!(f, "prepare internal error: {message}"),
        }
    }
}

impl std::error::Error for PrepareError {}
