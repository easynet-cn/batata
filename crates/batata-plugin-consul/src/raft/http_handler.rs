/// HTTP handlers for Consul Raft inter-node communication.
///
/// These endpoints are registered on the Consul HTTP server and handle
/// Raft protocol messages (append-entries, vote, install-snapshot)
/// from other cluster nodes.
use std::sync::Arc;

use actix_web::{HttpResponse, web};
use tracing::error;

use super::node::ConsulRaftNode;
use super::types::*;

/// POST /consul-raft/append-entries
pub async fn append_entries(
    raft_node: web::Data<Arc<ConsulRaftNode>>,
    body: web::Json<openraft::raft::AppendEntriesRequest<ConsulTypeConfig>>,
) -> HttpResponse {
    match raft_node.raft().append_entries(body.into_inner()).await {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(e) => {
            error!("Consul Raft append_entries error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// POST /consul-raft/vote
pub async fn vote(
    raft_node: web::Data<Arc<ConsulRaftNode>>,
    body: web::Json<openraft::raft::VoteRequest<NodeId>>,
) -> HttpResponse {
    match raft_node.raft().vote(body.into_inner()).await {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(e) => {
            error!("Consul Raft vote error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// POST /consul-raft/install-snapshot
pub async fn install_snapshot(
    raft_node: web::Data<Arc<ConsulRaftNode>>,
    body: web::Json<openraft::raft::InstallSnapshotRequest<ConsulTypeConfig>>,
) -> HttpResponse {
    match raft_node
        .raft()
        .install_snapshot(body.into_inner())
        .await
    {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(e) => {
            error!("Consul Raft install_snapshot error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// Register Consul Raft routes on the Consul HTTP server.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/consul-raft")
            .route("/append-entries", web::post().to(append_entries))
            .route("/vote", web::post().to(vote))
            .route("/install-snapshot", web::post().to(install_snapshot)),
    );
}
