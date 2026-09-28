//! Keyword search over the relational index.
//!
//! Mirrors the recall → rank → paginate pipeline of upstream
//! `AiResourceSearchService`, minus the vector channel: with no pgvector the
//! keyword hits *are* the ranking, which is exactly what upstream's
//! `recallWithMaxScore` degenerates to.

use batata_common::model::ai::search::AiResourceSearchHit;
use batata_persistence::PersistenceService;

/// Upper bound on recalled chunks, matching upstream
/// `DEFAULT_MAX_RECALL_CANDIDATES`.
pub const DEFAULT_MAX_RECALL_CANDIDATES: u64 = 10_000;

/// Default page size, matching upstream `DEFAULT_NUMBERED_PAGE_SIZE`.
pub const DEFAULT_PAGE_SIZE: u64 = 20;

/// Search the index and return one page of ranked resources.
///
/// Chunks are recalled by keyword, collapsed per resource keeping the highest
/// score, then ordered by descending score and paginated. Ties are broken by
/// resource name so paging is stable.
pub async fn search(
    persistence: &dyn PersistenceService,
    namespace_id: &str,
    text: &str,
    resource_types: &[&str],
    page_no: u64,
    page_size: u64,
) -> anyhow::Result<batata_api::model::Page<AiResourceSearchHit>> {
    let page_no = page_no.max(1);
    let page_size = if page_size == 0 {
        DEFAULT_PAGE_SIZE
    } else {
        page_size
    };

    let hits = persistence
        .search_chunk_search(
            namespace_id,
            text,
            resource_types,
            DEFAULT_MAX_RECALL_CANDIDATES,
        )
        .await?;

    let ranked = rank(hits);

    let total_count = ranked.len() as u64;
    let start = ((page_no - 1) * page_size) as usize;
    let items = ranked.into_iter().skip(start).take(page_size as usize).collect();

    Ok(batata_api::model::Page::new(
        total_count,
        page_no,
        page_size,
        items,
    ))
}

/// Collapse chunk hits per resource, keeping the best score.
///
/// Pure and deterministic, so it is unit-tested without a database.
pub(crate) fn rank(
    hits: Vec<batata_persistence::model::AiResourceSearchHitInfo>,
) -> Vec<AiResourceSearchHit> {
    let mut by_resource: Vec<AiResourceSearchHit> = Vec::new();

    for hit in hits {
        match by_resource.iter_mut().find(|candidate| {
            candidate.resource_type == hit.resource_type
                && candidate.resource_name == hit.resource_name
                && candidate.resource_version == hit.resource_version
        }) {
            Some(candidate) => {
                if hit.score > candidate.score {
                    candidate.score = hit.score;
                }
                if !candidate.matched_chunk_types.contains(&hit.chunk_type) {
                    candidate.matched_chunk_types.push(hit.chunk_type);
                }
            }
            None => by_resource.push(AiResourceSearchHit {
                resource_type: hit.resource_type,
                resource_name: hit.resource_name,
                resource_version: hit.resource_version,
                score: hit.score,
                matched_chunk_types: vec![hit.chunk_type],
            }),
        }
    }

    by_resource.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.resource_name.cmp(&b.resource_name))
    });
    by_resource
}

/// Resource types the index currently serves.
pub const SEARCHABLE_RESOURCE_TYPES: [&str; 1] = [crate::repository::resource_type::MCP];

#[cfg(test)]
mod tests {
    use super::*;
    use batata_persistence::model::AiResourceSearchHitInfo;

    fn hit(
        name: &str,
        version: &str,
        chunk_type: &str,
        score: f64,
    ) -> AiResourceSearchHitInfo {
        AiResourceSearchHitInfo {
            document_id: 1,
            chunk_id: 1,
            resource_type: "mcp".to_string(),
            resource_name: name.to_string(),
            resource_version: version.to_string(),
            chunk_type: chunk_type.to_string(),
            score,
        }
    }

    #[test]
    fn one_resource_keeps_its_best_score() {
        let ranked = rank(vec![
            hit("a", "1.0.0", "description", 0.4),
            hit("a", "1.0.0", "mcp_content", 1.0),
        ]);
        assert_eq!(ranked.len(), 1, "chunks of one resource collapse");
        assert_eq!(ranked[0].score, 1.0);
        assert_eq!(ranked[0].matched_chunk_types.len(), 2);
    }

    #[test]
    fn ordered_by_descending_score() {
        let ranked = rank(vec![
            hit("low", "1.0.0", "tag", 0.4),
            hit("high", "1.0.0", "description", 1.0),
        ]);
        assert_eq!(ranked[0].resource_name, "high");
        assert_eq!(ranked[1].resource_name, "low");
    }

    #[test]
    fn ties_broken_by_name_for_stable_paging() {
        let ranked = rank(vec![
            hit("zeta", "1.0.0", "tag", 0.8),
            hit("alpha", "1.0.0", "tag", 0.8),
        ]);
        assert_eq!(ranked[0].resource_name, "alpha");
        assert_eq!(ranked[1].resource_name, "zeta");
    }

    #[test]
    fn versions_are_ranked_separately() {
        let ranked = rank(vec![
            hit("a", "1.0.0", "tag", 0.8),
            hit("a", "2.0.0", "tag", 0.4),
        ]);
        assert_eq!(ranked.len(), 2, "different versions are distinct results");
    }
}
