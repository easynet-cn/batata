//! Config sync endpoints for cross-environment configuration synchronization
//!
//! Provides endpoints for syncing configurations between environments.
//! The sync operates by exporting configs from the local server and publishing
//! them to the target environment via its V2 config API.

use actix_web::{HttpResponse, Responder, get, post, web};
use serde::{Deserialize, Serialize};

use batata_server_common::model::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncEnvironment {
    pub name: String,
    pub url: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    pub target_url: String,
    pub namespace_id: Option<String>,
    pub data_ids: Option<Vec<String>>,
    pub group: Option<String>,
    pub policy: Option<String>, // "overwrite", "skip", "abort"
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub total: u32,
    pub synced: u32,
    pub skipped: u32,
    pub failed: u32,
    pub details: Vec<SyncItemResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncItemResult {
    pub data_id: String,
    pub group: String,
    pub status: String, // "synced", "skipped", "failed"
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncHistoryEntry {
    pub id: u64,
    pub target_url: String,
    pub namespace_id: String,
    pub total: u32,
    pub synced: u32,
    pub failed: u32,
    pub operator: String,
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
    let policy = body.policy.as_deref().unwrap_or("skip");

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

    // Sync each config to target via HTTP
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
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

    let mut synced = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;
    let mut details = Vec::new();

    for config in &configs {
        let result = client
            .post(format!(
                "{}/nacos/v2/cs/config",
                target_url.trim_end_matches('/')
            ))
            .form(&[
                ("dataId", &config.data_id),
                ("group", &config.group),
                ("tenant", &config.tenant),
                ("content", &config.content),
                ("type", &config.config_type),
            ])
            .send()
            .await;

        match result {
            Ok(resp) if resp.status().is_success() => {
                synced += 1;
                details.push(SyncItemResult {
                    data_id: config.data_id.clone(),
                    group: config.group.clone(),
                    status: "synced".to_string(),
                    message: None,
                });
            }
            Ok(resp) => {
                let status = resp.status().as_u16();
                if policy == "skip" && status == 409 {
                    skipped += 1;
                    details.push(SyncItemResult {
                        data_id: config.data_id.clone(),
                        group: config.group.clone(),
                        status: "skipped".to_string(),
                        message: Some("Already exists on target".to_string()),
                    });
                } else {
                    failed += 1;
                    details.push(SyncItemResult {
                        data_id: config.data_id.clone(),
                        group: config.group.clone(),
                        status: "failed".to_string(),
                        message: Some(format!("HTTP {}", status)),
                    });
                }
            }
            Err(e) => {
                failed += 1;
                details.push(SyncItemResult {
                    data_id: config.data_id.clone(),
                    group: config.group.clone(),
                    status: "failed".to_string(),
                    message: Some(e.to_string()),
                });
            }
        }
    }

    let total = configs.len() as u32;

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

pub fn routes() -> actix_web::Scope {
    web::scope("/sync")
        .service(get_sync_environments)
        .service(sync_configs)
        .service(get_sync_history)
}
