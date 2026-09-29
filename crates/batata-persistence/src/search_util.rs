//! Keyword matching shared by the non-SQL backends.
//!
//! The SQL backend does this in the database; RocksDB has no `LIKE`, so the
//! same scoring rules are applied in memory. Keeping the rules in one place
//! stops the two paths from drifting apart.

use crate::model::{AiResourceSearchChunkInfo, AiResourceSearchHitInfo};

/// Status a chunk must have to be searchable.
pub const CHUNK_STATUS_ENABLED: &str = "enabled";

/// Score when the canonical text matches.
pub const SCORE_CANONICAL: f64 = 1.0;
/// Score when only the raw chunk text matches.
pub const SCORE_CHUNK_TEXT: f64 = 0.8;

/// Filter and score chunks against a keyword query.
///
/// Mirrors the SQL `CASE`: 1.0 for a `canonical_text` match, 0.8 for
/// `chunk_text`. Non-matching chunks are dropped, exactly as the SQL `WHERE`
/// would. Results are ordered by descending score and capped at `limit`.
pub fn keyword_hits(
    chunks: Vec<AiResourceSearchChunkInfo>,
    text: &str,
    resource_types: &[&str],
    limit: u64,
) -> Vec<AiResourceSearchHitInfo> {
    let needle = text.trim().to_lowercase();
    if needle.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut hits: Vec<AiResourceSearchHitInfo> = chunks
        .into_iter()
        .filter(|chunk| chunk.status == CHUNK_STATUS_ENABLED)
        .filter(|chunk| {
            resource_types.is_empty() || resource_types.contains(&chunk.resource_type.as_str())
        })
        .filter_map(|chunk| {
            let score = if chunk.canonical_text.to_lowercase().contains(&needle) {
                SCORE_CANONICAL
            } else if chunk.chunk_text.to_lowercase().contains(&needle) {
                SCORE_CHUNK_TEXT
            } else {
                return None;
            };
            Some(AiResourceSearchHitInfo {
                document_id: chunk.document_id,
                chunk_id: chunk.id,
                resource_type: chunk.resource_type,
                resource_name: chunk.resource_name,
                resource_version: chunk.resource_version,
                chunk_type: chunk.chunk_type,
                score,
            })
        })
        .collect();

    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(limit as usize);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: i64, canonical: &str, text: &str, status: &str) -> AiResourceSearchChunkInfo {
        AiResourceSearchChunkInfo {
            id,
            document_id: 1,
            namespace_id: "public".to_string(),
            resource_type: "mcp".to_string(),
            resource_name: "a".to_string(),
            resource_version: "1.0.0".to_string(),
            chunk_type: "description".to_string(),
            chunk_text: text.to_string(),
            canonical_text: canonical.to_string(),
            language: None,
            chunk_hash: String::new(),
            metadata: None,
            status: status.to_string(),
            gmt_create: None,
            gmt_modified: None,
        }
    }

    #[test]
    fn canonical_match_scores_higher() {
        let hits = keyword_hits(
            vec![chunk(1, "mcp a description echo", "echo", "enabled")],
            "echo",
            &[],
            10,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].score, SCORE_CANONICAL);
    }

    #[test]
    fn chunk_text_only_match_scores_lower() {
        let hits = keyword_hits(
            vec![chunk(1, "mcp a description nothing", "echo tool", "enabled")],
            "echo",
            &[],
            10,
        );
        assert_eq!(hits[0].score, SCORE_CHUNK_TEXT);
    }

    #[test]
    fn match_is_case_insensitive() {
        let hits = keyword_hits(
            vec![chunk(1, "ECHO", "ECHO", "enabled")],
            "echo",
            &[],
            10,
        );
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn disabled_chunks_are_excluded() {
        let hits = keyword_hits(
            vec![chunk(1, "echo", "echo", "disabled")],
            "echo",
            &[],
            10,
        );
        assert!(hits.is_empty());
    }

    #[test]
    fn resource_type_filter_applies() {
        let chunks = vec![chunk(1, "echo", "echo", "enabled")];
        assert!(keyword_hits(chunks.clone(), "echo", &["skill"], 10).is_empty());
        assert_eq!(keyword_hits(chunks, "echo", &["mcp"], 10).len(), 1);
    }

    #[test]
    fn ordered_by_descending_score_and_capped() {
        let chunks = vec![
            chunk(1, "nothing", "echo", "enabled"),
            chunk(2, "echo top", "echo", "enabled"),
        ];
        let hits = keyword_hits(chunks, "echo", &[], 10);
        assert_eq!(hits[0].score, SCORE_CANONICAL, "higher score first");
        assert_eq!(hits[0].chunk_id, 2);
        assert_eq!(keyword_hits(Vec::new(), "echo", &[], 10).len(), 0);
    }

    #[test]
    fn blank_query_returns_nothing() {
        assert!(keyword_hits(
            vec![chunk(1, "echo", "echo", "enabled")],
            "   ",
            &[],
            10
        )
        .is_empty());
    }
}
