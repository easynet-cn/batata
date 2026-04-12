//! V3 Admin lock introspection endpoints
//!
//! Provides a read-only listing of distributed lock state from `CF_LOCKS`.
//! This is the Batata analogue of Nacos `LockManager.showLocks()`.

use actix_web::{HttpRequest, Responder, get, web};
use serde::Deserialize;

use crate::{error, model::common::AppState, model::response::Result};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ListLocksQuery {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
}

impl Default for ListLocksQuery {
    fn default() -> Self {
        Self {
            namespace_id: None,
        }
    }
}

/// GET /v3/admin/core/lock/list
///
/// Returns all locks currently stored in Raft state (both held and free).
/// In standalone (non-Raft) mode this endpoint returns an empty list because
/// the in-memory `LockService` is not surfaced here — lock introspection is
/// only meaningful in cluster mode where locks survive restart.
#[get("list")]
async fn list_locks(
    _req: HttpRequest,
    data: web::Data<AppState>,
    query: web::Query<ListLocksQuery>,
) -> impl Responder {
    let Some(ref raft) = data.raft_node else {
        let empty: Vec<serde_json::Value> = vec![];
        return Result::<Vec<serde_json::Value>>::http_success(empty);
    };

    let ns = query.namespace_id.as_deref().unwrap_or("");
    match raft.list_locks(ns) {
        Ok(locks) => Result::<Vec<serde_json::Value>>::http_success(locks),
        Err(e) => {
            let empty: Vec<serde_json::Value> = vec![];
            Result::<Vec<serde_json::Value>>::http_response(
                500,
                error::SERVER_ERROR.code,
                format!("Failed to list locks: {}", e),
                empty,
            )
        }
    }
}

pub fn routes() -> actix_web::Scope {
    web::scope("/lock").service(list_locks)
}
