//! Config sync endpoints for cross-environment configuration synchronization
//!
//! Provides endpoints for syncing configurations between environments.
//! The sync operates by exporting configs from the local server and publishing
//! them to the target environment via its V2 config API.

use actix_web::{HttpResponse, Responder, get, post, web};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use batata_server_common::model::AppState;

/// Hard cap on the number of configs synced in one request.
const MAX_SYNC_BATCH: usize = 1000;
/// Maximum number of in-flight HTTP POSTs during a sync.
const SYNC_CONCURRENCY: usize = 8;
/// Wall-clock bound for the whole sync batch.
const SYNC_TOTAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// Per-request timeout for the target HTTP client.
const SYNC_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Reject a sync batch that is too large to push safely in one request.
fn check_sync_batch_size(count: usize) -> Result<(), String> {
    if count > MAX_SYNC_BATCH {
        return Err(format!(
            "sync batch of {} configs exceeds the limit of {}; narrow the query (dataIds/group)",
            count, MAX_SYNC_BATCH
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// A configuration sync target environment.
pub struct SyncEnvironment {
    /// Display name of the sync target environment.
    pub name: String,
    /// Base URL of the target environment.
    pub url: String,
    /// Status of the environment (e.g. `available`).
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request body for triggering a configuration sync to a target environment.
pub struct SyncRequest {
    /// Base URL of the target environment to sync to.
    pub target_url: String,
    /// Optional namespace to scope the sync to.
    pub namespace_id: Option<String>,
    /// Optional list of data IDs to sync.
    pub data_ids: Option<Vec<String>>,
    /// Optional group to scope the sync to.
    pub group: Option<String>,
    /// Conflict resolution policy (`overwrite`, `skip`, `abort`).
    pub policy: Option<String>, // "overwrite", "skip", "abort"
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Aggregate result of a configuration sync operation.
pub struct SyncResult {
    /// Total number of configs considered for sync.
    pub total: u32,
    /// Number of configs successfully synced.
    pub synced: u32,
    /// Number of configs skipped due to conflicts.
    pub skipped: u32,
    /// Number of configs that failed to sync.
    pub failed: u32,
    /// Per-config sync results.
    pub details: Vec<SyncItemResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Per-config result of a configuration sync operation.
pub struct SyncItemResult {
    /// Data ID of the synced config.
    pub data_id: String,
    /// Group of the synced config.
    pub group: String,
    /// Sync status (`synced`, `skipped`, `failed`).
    pub status: String, // "synced", "skipped", "failed"
    /// Optional detail message for the sync result.
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// A historical record of a configuration sync operation.
pub struct SyncHistoryEntry {
    /// Unique identifier of the sync history entry.
    pub id: u64,
    /// Base URL of the target environment.
    pub target_url: String,
    /// Namespace the sync operated on.
    pub namespace_id: String,
    /// Total number of configs considered for sync.
    pub total: u32,
    /// Number of configs successfully synced.
    pub synced: u32,
    /// Number of configs that failed to sync.
    pub failed: u32,
    /// Operator that triggered the sync.
    pub operator: String,
    /// Creation time of the sync record.
    pub created_time: String,
}

/// GET /v3/console/sync/environments
/// Returns available sync target environments from configuration
#[get("/environments")]
pub async fn get_sync_environments(data: web::Data<AppState>) -> impl Responder {
    // Read sync targets from configuration
    // Format: comma-separated URLs in batata.sync.targets
    let targets = data
        .configuration
        .config
        .get_string("batata.sync.targets")
        .unwrap_or_default();

    let environments: Vec<SyncEnvironment> = if targets.is_empty() {
        Vec::new()
    } else {
        targets
            .split(',')
            .map(|url| {
                let url = url.trim().to_string();
                SyncEnvironment {
                    name: url.clone(),
                    url,
                    status: "available".to_string(),
                }
            })
            .collect()
    };

    HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": environments
    }))
}

/// POST /v3/console/sync/configs
/// Trigger config sync to a target environment
#[post("/configs")]
pub async fn sync_configs(
    data: web::Data<AppState>,
    body: web::Json<SyncRequest>,
) -> impl Responder {
    let target_url = &body.target_url;
    let namespace = body.namespace_id.as_deref().unwrap_or("");
    // Owned so each concurrent sync task can clone it independently.
    let policy = body.policy.clone().unwrap_or_else(|| "skip".to_string());

    if target_url.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": 400,
            "message": "target_url is required",
        }));
    }

    // Get configs from persistence
    let configs = if let Some(ref persistence) = data.persistence {
        let group = body.group.as_deref();
        let data_ids_clone = body.data_ids.clone();
        match persistence
            .config_find_for_export(namespace, group, data_ids_clone, None)
            .await
        {
            Ok(configs) => configs,
            Err(e) => {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "code": 500,
                    "message": format!("Failed to read configs: {}", e),
                }));
            }
        }
    } else {
        Vec::new()
    };

    // Bound the number of configs we are willing to push in one request, so a
    // whole namespace cannot turn into an unbounded sequence of HTTP POSTs.
    if let Err(msg) = check_sync_batch_size(configs.len()) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": 400,
            "message": msg,
        }));
    }
    let total = configs.len() as u32;

    // Sync each config to target via HTTP. A per-request timeout bounds each
    // POST, `buffer_unordered` bounds concurrency, and the outer
    // `tokio::time::timeout` bounds the total wall-clock duration.
    let client = match reqwest::Client::builder()
        .timeout(SYNC_REQUEST_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": format!("Failed to create HTTP client: {}", e),
            }));
        }
    };

    let post_url = format!("{}/nacos/v2/cs/config", target_url.trim_end_matches('/'));

    let outcomes_fut = stream::iter(configs.iter().enumerate())
        .map(|(index, config)| {
            let client = client.clone();
            let post_url = post_url.clone();
            let policy = policy.clone();
            async move {
                let result = client
                    .post(&post_url)
                    .form(&[
                        ("dataId", &config.data_id),
                        ("group", &config.group),
                        ("tenant", &config.tenant),
                        ("content", &config.content),
                        ("type", &config.config_type),
                    ])
                    .send()
                    .await;

                let detail = match result {
                    Ok(resp) if resp.status().is_success() => SyncItemResult {
                        data_id: config.data_id.clone(),
                        group: config.group.clone(),
                        status: "synced".to_string(),
                        message: None,
                    },
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        if policy == "skip" && status == 409 {
                            SyncItemResult {
                                data_id: config.data_id.clone(),
                                group: config.group.clone(),
                                status: "skipped".to_string(),
                                message: Some("Already exists on target".to_string()),
                            }
                        } else {
                            SyncItemResult {
                                data_id: config.data_id.clone(),
                                group: config.group.clone(),
                                status: "failed".to_string(),
                                message: Some(format!("HTTP {}", status)),
                            }
                        }
                    }
                    Err(e) => SyncItemResult {
                        data_id: config.data_id.clone(),
                        group: config.group.clone(),
                        status: "failed".to_string(),
                        message: Some(e.to_string()),
                    },
                };
                (index, detail)
            }
        })
        .buffer_unordered(SYNC_CONCURRENCY)
        .collect::<Vec<_>>();

    // Bound the whole batch in wall-clock time so a slow target cannot hold
    // the admin request open indefinitely.
    let mut outcomes = match tokio::time::timeout(SYNC_TOTAL_TIMEOUT, outcomes_fut).await {
        Ok(v) => v,
        Err(_) => {
            return HttpResponse::GatewayTimeout().json(serde_json::json!({
                "code": 504,
                "message": format!(
                    "config sync timed out after {:?}; narrow the query (dataIds/group)",
                    SYNC_TOTAL_TIMEOUT
                ),
            }));
        }
    };

    // Preserve the original request/response order despite out-of-order
    // completion, then aggregate counters and details.
    outcomes.sort_by_key(|(index, _)| *index);
    let mut synced = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;
    let mut details = Vec::with_capacity(outcomes.len());
    for (_, detail) in outcomes {
        match detail.status.as_str() {
            "synced" => synced += 1,
            "skipped" => skipped += 1,
            _ => failed += 1,
        }
        details.push(detail);
    }

    HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": SyncResult { total, synced, skipped, failed, details }
    }))
}

/// GET /v3/console/sync/history
/// Returns sync history. Currently not persisted; for production use,
/// consider adding a database table for sync audit records.
#[get("/history")]
pub async fn get_sync_history() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "totalCount": 0,
            "pageNumber": 1,
            "pagesAvailable": 0,
            "pageItems": []
        }
    }))
}

/// Register the config sync routes under `/sync`.
pub fn routes() -> actix_web::Scope {
    web::scope("/sync")
        .service(get_sync_environments)
        .service(sync_configs)
        .service(get_sync_history)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_sync_batch_size_allows_within_cap() {
        // Success path: empty and exactly-at-cap batches are accepted.
        assert!(check_sync_batch_size(0).is_ok());
        assert!(check_sync_batch_size(MAX_SYNC_BATCH).is_ok());
    }

    #[test]
    fn check_sync_batch_size_rejects_over_cap() {
        // Guard: an over-cap batch must be rejected with a clear error rather
        // than silently truncated. Removing the check makes this fail.
        let err = check_sync_batch_size(MAX_SYNC_BATCH + 1).unwrap_err();
        assert!(err.contains("narrow the query"));
    }

    #[test]
    fn sync_bounds_are_sane() {
        // Success path: concurrency and duration bounds are positive and the
        // total timeout covers at least one per-request timeout.
        assert!(SYNC_CONCURRENCY > 0 && SYNC_CONCURRENCY <= 64);
        assert!(SYNC_TOTAL_TIMEOUT >= SYNC_REQUEST_TIMEOUT);
    }
}
