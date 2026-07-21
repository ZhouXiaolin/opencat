//! Back-compat alias: [`LiveScriptHost`] is now [`super::ScriptRealm`].
//!
//! Prefer [`super::ScriptRealm`] in new code. This module keeps the old name so
//! existing imports compile during the expand phase of issue #20 / #24.

use super::realm::ScriptRealm;

/// Historical name for the pipeline-owned script realm.
pub type LiveScriptHost<C> = ScriptRealm<C>;
