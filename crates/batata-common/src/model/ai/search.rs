//! Search index result models.
//!
//! These live in `batata-common` rather than `batata-ai` for the same reason as
//! `ai_trace`: the console handlers depend on this crate, not on `batata-ai`,
//! and search results have to cross that boundary through the
//! `McpServerService` trait.

use serde::Serialize;

/// One ranked AI resource search result.
///
/// Upstream's client search response carries the canonical resource identity
/// plus a relevance score; the matched chunk types are included so callers can
/// see why a resource matched.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResourceSearchHit {
    /// Resource type (e.g. `mcp`).
    pub resource_type: String,
    /// Resource name.
    pub resource_name: String,
    /// Resource version.
    pub resource_version: String,
    /// Best match score across the resource's chunks.
    pub score: f64,
    /// Chunk types that matched, in recall order.
    pub matched_chunk_types: Vec<String>,
}

/// Upper bound on a search query string, matching upstream
/// `AiResourceSearchForm.MAX_QUERY_LENGTH`.
pub const MAX_QUERY_LENGTH: usize = 1024;

/// Upper bound on the page size, matching upstream
/// `AiResourceSearchForm.MAX_LIMIT`.
pub const MAX_PAGE_SIZE: u64 = 100;

/// Default page size, matching upstream `DEFAULT_LIMIT`.
pub const DEFAULT_PAGE_SIZE: u64 = 20;
