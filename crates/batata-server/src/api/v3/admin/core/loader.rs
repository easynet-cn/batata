//! V3 Admin server loader endpoints
//!
//! Implements SDK connection load balancing across cluster nodes.
//! Following the Nacos 3.x ServerLoaderController pattern for SDK compatibility.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use actix_web::{HttpRequest, Responder, get, post, web};
use serde::{Deserialize, Serialize};

use batata_api::remote::model::{ServerLoaderInfoRequest, ServerLoaderInfoResponse};
use batata_core::ClientConnectionManager;
use batata_core::service::cluster_client::ClusterClientManager;

use crate::{
    ActionTypes, ApiType, Secured, SignType, error, model::common::AppState,
    model::response::Result, secured,
};

/// Timeout for a single peer `ServerLoaderInfoRequest`.
///
/// Kept short so a slow or unreachable peer cannot block the admin endpoint;
/// on timeout the peer is reported with zeroed metrics and the failure is
/// logged. Expressed as a named constant per project rules (no magic numbers).
const PEER_LOADER_INFO_TIMEOUT: Duration = Duration::from_secs(2);

/// Per-node loader metric
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerLoaderMetric {
    address: String,
    sdk_con_count: usize,
    con_count: usize,
    load: String,
    cpu: String,
}

/// Cluster-wide loader metrics summary
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerLoaderMetrics {
    detail: Vec<ServerLoaderMetric>,
    member_count: usize,
    metrics_count: usize,
    completed: bool,
    max: usize,
    min: usize,
    avg: usize,
    threshold: String,
    total: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionInfo {
    connection_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReloadCurrentParam {
    count: Option<usize>,
    #[serde(alias = "redirectAddress")]
    redirect_address: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SmartReloadParam {
    #[serde(alias = "loaderFactor")]
    loader_factor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReloadClientParam {
    #[serde(alias = "connectionId")]
    connection_id: Option<String>,
    #[serde(alias = "redirectAddress")]
    redirect_address: Option<String>,
}

/// Get the current node's loader metric using real system metrics
fn get_self_loader_metric(
    address: &str,
    connection_manager: &dyn ClientConnectionManager,
) -> ServerLoaderMetric {
    use sysinfo::System;

    let sdk_con_count = connection_manager.connection_count();

    let mut sys = System::new();
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_usage: f32 =
        sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32;

    let load_avg = System::load_average().one;

    ServerLoaderMetric {
        address: address.to_string(),
        sdk_con_count,
        con_count: sdk_con_count,
        load: format!("{:.2}", load_avg),
        cpu: format!("{:.1}", cpu_usage),
    }
}

/// Query a single peer for its `ServerLoaderInfoResponse` and map it to a
/// `ServerLoaderMetric`. On error (circuit breaker open, connect failure,
/// timeout, decode error) returns a zeroed metric and logs the failure so
/// the UI still sees the peer but can tell it did not report.
///
/// This is the Rust equivalent of Nacos
/// `ServerLoaderController.getAllServerLoaders()` issuing a
/// `ServerLoaderInfoRequest` per remote member.
async fn fetch_peer_loader_metric(
    ccm: &ClusterClientManager,
    peer_address: &str,
) -> ServerLoaderMetric {
    let request = ServerLoaderInfoRequest::new();
    let result = tokio::time::timeout(
        PEER_LOADER_INFO_TIMEOUT,
        ccm.send_request(peer_address, request),
    )
    .await;

    match result {
        Ok(Ok(payload)) => {
            // `ResponseTrait` does not expose a `from_payload` helper, so
            // decode the JSON body directly. Body bytes are borrowed from
            // the payload and deserialized into the typed response.
            let bytes: &[u8] = payload
                .body
                .as_ref()
                .map(|b| b.value.as_slice())
                .unwrap_or(&[]);
            let response: ServerLoaderInfoResponse =
                serde_json::from_slice(bytes).unwrap_or_else(|e| {
                    tracing::warn!(
                        peer = peer_address,
                        error = %e,
                        "Failed to decode peer ServerLoaderInfoResponse — reporting zeros"
                    );
                    ServerLoaderInfoResponse::new()
                });
            let metrics = &response.loader_metrics;
            let sdk_con_count: usize = metrics
                .get("sdkConCount")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            let load = metrics
                .get("load")
                .cloned()
                .unwrap_or_else(|| "0.00".to_string());
            let cpu = metrics
                .get("cpu")
                .cloned()
                .unwrap_or_else(|| "0.0".to_string());
            ServerLoaderMetric {
                address: peer_address.to_string(),
                sdk_con_count,
                con_count: sdk_con_count,
                load,
                cpu,
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(
                peer = peer_address,
                error = %e,
                "Failed to fetch peer loader metrics — reporting zeros"
            );
            ServerLoaderMetric {
                address: peer_address.to_string(),
                sdk_con_count: 0,
                con_count: 0,
                load: "0.00".to_string(),
                cpu: "0.0".to_string(),
            }
        }
        Err(_) => {
            tracing::warn!(
                peer = peer_address,
                timeout_ms = PEER_LOADER_INFO_TIMEOUT.as_millis() as u64,
                "Timed out fetching peer loader metrics — reporting zeros"
            );
            ServerLoaderMetric {
                address: peer_address.to_string(),
                sdk_con_count: 0,
                con_count: 0,
                load: "0.00".to_string(),
                cpu: "0.0".to_string(),
            }
        }
    }
}

/// Build cluster-wide metrics from a list of per-node metrics
fn build_cluster_metrics(
    details: Vec<ServerLoaderMetric>,
    member_count: usize,
) -> ServerLoaderMetrics {
    let metrics_count = details.len();
    let max = details.iter().map(|m| m.sdk_con_count).max().unwrap_or(0);
    let min = details.iter().map(|m| m.sdk_con_count).min().unwrap_or(0);
    let total: usize = details.iter().map(|m| m.sdk_con_count).sum();
    let avg = if metrics_count > 0 {
        total / metrics_count
    } else {
        0
    };
    let threshold = format!("{:.1}", avg as f64 * 1.1);
    let completed = metrics_count == member_count;

    ServerLoaderMetrics {
        detail: details,
        member_count,
        metrics_count,
        completed,
        max,
        min,
        avg,
        threshold,
        total,
    }
}

/// GET /v3/admin/core/loader/current
///
/// Returns all current SDK connections/clients on this server.
#[get("current")]
async fn get_current(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<dyn ClientConnectionManager>>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let ids = connection_manager.get_all_connection_ids();
    let connections: HashMap<String, ConnectionInfo> = ids
        .into_iter()
        .map(|id| {
            let info = ConnectionInfo {
                connection_id: id.clone(),
            };
            (id, info)
        })
        .collect();

    Result::<HashMap<String, ConnectionInfo>>::http_success(connections)
}

/// POST /v3/admin/core/loader/reloadCurrent
///
/// Rebalance SDK connections on current server to target count.
/// Excess connections (current - count) will be sent ConnectResetRequest
/// to reconnect to another server.
#[post("reloadCurrent")]
async fn reload_current(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<dyn ClientConnectionManager>>,
    params: web::Query<ReloadCurrentParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let target_count = params.count.unwrap_or(0);
    let redirect = params.redirect_address.as_deref();

    tracing::info!(
        target_count,
        redirect_address = ?redirect,
        "Server reload current requested via admin API"
    );

    let ejected = connection_manager.load_count(target_count, redirect).await;

    Result::<String>::http_success(format!("Reload completed, ejected {} connections", ejected))
}

/// POST /v3/admin/core/loader/smartReloadCluster
///
/// Intelligently balance SDK connections across all cluster nodes.
/// Calculates avg connections, finds overloaded servers (> avg * (1 + factor))
/// and underloaded servers (< avg * (1 - factor)), then sends reload requests
/// from overloaded to underloaded nodes.
#[post("smartReloadCluster")]
async fn smart_reload(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<dyn ClientConnectionManager>>,
    cluster_client: Option<web::Data<Arc<ClusterClientManager>>>,
    params: web::Query<SmartReloadParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let loader_factor: f64 = params
        .loader_factor
        .as_deref()
        .unwrap_or("0.1")
        .parse()
        .unwrap_or(0.1);

    tracing::info!(
        loader_factor = %loader_factor,
        "Smart cluster reload requested via admin API"
    );

    // Get self metrics
    let self_member = data.cluster_manager().get_self_member();
    let self_metric =
        get_self_loader_metric(&self_member.address, connection_manager.as_ref().as_ref());
    let all_members = data.cluster_manager().all_members_extended();
    let mut details = vec![self_metric.clone()];

    // For every remote cluster member, fetch the real metrics via gRPC
    // `ServerLoaderInfoRequest`. Matches Nacos ServerLoaderController
    // behavior. Without a cluster client (standalone mode) peers are
    // reported with zeroed metrics — but `all_members` is typically just
    // self in that case.
    for member in &all_members {
        if member.address == self_member.address {
            continue;
        }
        match cluster_client.as_ref() {
            Some(ccm) => {
                let metric = fetch_peer_loader_metric(ccm.as_ref().as_ref(), &member.address).await;
                details.push(metric);
            }
            None => {
                details.push(ServerLoaderMetric {
                    address: member.address.clone(),
                    sdk_con_count: 0,
                    con_count: 0,
                    load: "0.00".to_string(),
                    cpu: "0.0".to_string(),
                });
            }
        }
    }

    let total: usize = details.iter().map(|m| m.sdk_con_count).sum();
    let avg = if !details.is_empty() {
        total / details.len()
    } else {
        return Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            "Smart reload failed, no cluster members found".to_string(),
            String::new(),
        );
    };

    let over_limit = (avg as f64 * (1.0 + loader_factor)) as usize;
    let under_limit = (avg as f64 * (1.0 - loader_factor)) as usize;

    // If this node is overloaded, eject connections down to over_limit
    if self_metric.sdk_con_count > over_limit {
        // Find an underloaded server to redirect to
        let redirect_server = details
            .iter()
            .find(|m| m.sdk_con_count < under_limit && m.address != self_member.address)
            .map(|m| m.address.clone());

        let redirect = redirect_server.as_deref();
        let ejected = connection_manager.load_count(over_limit, redirect).await;

        tracing::info!(
            over_limit,
            under_limit,
            ejected,
            redirect = ?redirect,
            "Smart reload: ejected overloaded connections from self"
        );
    }

    Result::<String>::http_success("Smart reload completed".to_string())
}

/// POST /v3/admin/core/loader/reloadClient
///
/// Send a ConnectResetRequest to a specific client connection,
/// instructing it to reconnect (optionally to a different server).
#[post("reloadClient")]
async fn reload_client(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<dyn ClientConnectionManager>>,
    params: web::Query<ReloadClientParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let connection_id = match &params.connection_id {
        Some(id) if !id.is_empty() => id.as_str(),
        _ => {
            return Result::<String>::http_response(
                400,
                error::PARAMETER_VALIDATE_ERROR.code,
                "connectionId is required".to_string(),
                String::new(),
            );
        }
    };

    let redirect = params.redirect_address.as_deref();

    tracing::info!(
        connection_id,
        redirect_address = ?redirect,
        "Client reload requested via admin API"
    );

    let success = connection_manager
        .load_single(connection_id, redirect)
        .await;

    if success {
        Result::<String>::http_success("Client reload request sent".to_string())
    } else {
        Result::<String>::http_response(
            404,
            error::RESOURCE_NOT_FOUND.code,
            format!("Connection {} not found", connection_id),
            String::new(),
        )
    }
}

/// GET /v3/admin/core/loader/cluster
///
/// Get server loader metrics for the entire cluster.
/// Returns per-node metrics (sdkConCount, cpu, load) and aggregate statistics
/// (max, min, avg, total, threshold).
#[get("cluster")]
async fn get_cluster(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<dyn ClientConnectionManager>>,
    cluster_client: Option<web::Data<Arc<ClusterClientManager>>>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let self_member = data.cluster_manager().get_self_member();
    let all_members = data.cluster_manager().all_members_extended();
    let member_count = all_members.len();

    // Build self metric with real data
    let self_metric =
        get_self_loader_metric(&self_member.address, connection_manager.as_ref().as_ref());

    let mut details = vec![self_metric];

    // Fetch real metrics for every remote member via gRPC
    // `ServerLoaderInfoRequest` (matches Nacos ServerLoaderController).
    // Fallback to zeros only when no cluster client is wired (standalone).
    for member in &all_members {
        if member.address == self_member.address {
            continue;
        }
        match cluster_client.as_ref() {
            Some(ccm) => {
                let metric = fetch_peer_loader_metric(ccm.as_ref().as_ref(), &member.address).await;
                details.push(metric);
            }
            None => {
                details.push(ServerLoaderMetric {
                    address: member.address.clone(),
                    sdk_con_count: 0,
                    con_count: 0,
                    load: "0.00".to_string(),
                    cpu: "0.0".to_string(),
                });
            }
        }
    }

    details.sort_by(|a, b| a.address.cmp(&b.address));
    let metrics = build_cluster_metrics(details, member_count);

    Result::<ServerLoaderMetrics>::http_success(metrics)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/loader")
        .service(get_current)
        .service(reload_current)
        .service(smart_reload)
        .service(reload_client)
        .service(get_cluster)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use batata_common::ConnectionInfo;
    use batata_core::ClientConnectionManager;

    /// Stub connection manager with a configurable connection count. Used to
    /// prove that `get_self_loader_metric` pulls the real value from the
    /// connection manager instead of returning a hardcoded zero (the pre-fix
    /// peer behavior).
    struct StubConnManager {
        count: usize,
    }

    #[async_trait]
    impl ClientConnectionManager for StubConnManager {
        fn connection_count(&self) -> usize {
            self.count
        }
        fn has_connection(&self, _id: &str) -> bool {
            false
        }
        fn get_all_connection_ids(&self) -> Vec<String> {
            Vec::new()
        }
        fn get_connection_info(&self, _id: &str) -> Option<ConnectionInfo> {
            None
        }
        fn get_all_connection_infos(&self) -> Vec<ConnectionInfo> {
            Vec::new()
        }
        fn connections_for_ip(&self, _ip: &str) -> usize {
            0
        }
        async fn push_message(&self, _id: &str, _payload: batata_api::grpc::Payload) -> bool {
            false
        }
        async fn push_message_to_many(
            &self,
            _ids: &[String],
            _payload: batata_api::grpc::Payload,
        ) -> usize {
            0
        }
        async fn load_single(&self, _id: &str, _redirect: Option<&str>) -> bool {
            false
        }
        async fn load_count(&self, _target: usize, _redirect: Option<&str>) -> usize {
            0
        }
        fn touch_connection(&self, _id: &str) {}
    }

    #[test]
    fn self_loader_metric_reflects_real_connection_count() {
        let stub = StubConnManager { count: 7 };
        let metric = get_self_loader_metric("10.0.0.1:8848", &stub);

        assert_eq!(metric.address, "10.0.0.1:8848");
        assert_eq!(
            metric.sdk_con_count, 7,
            "sdk_con_count must come from the connection manager, not a hardcoded value"
        );
        assert_eq!(metric.con_count, 7, "con_count must mirror sdk_con_count");

        // Load and CPU are real system-derived values; they must be
        // well-formed floats (not the faked "0.00" / "0.0" peer placeholders
        // except by coincidence). We assert parseability rather than exact
        // content to keep the test environment-independent.
        assert!(
            metric.load.parse::<f32>().is_ok(),
            "load must be a parseable float, got '{}'",
            metric.load
        );
        assert!(
            metric.cpu.parse::<f32>().is_ok(),
            "cpu must be a parseable float, got '{}'",
            metric.cpu
        );
    }

    #[test]
    fn build_cluster_metrics_aggregates_correctly() {
        let details = vec![
            ServerLoaderMetric {
                address: "a:8848".to_string(),
                sdk_con_count: 10,
                con_count: 10,
                load: "0.50".to_string(),
                cpu: "15.0".to_string(),
            },
            ServerLoaderMetric {
                address: "b:8848".to_string(),
                sdk_con_count: 20,
                con_count: 20,
                load: "1.00".to_string(),
                cpu: "30.0".to_string(),
            },
        ];
        let agg = build_cluster_metrics(details, 2);
        assert_eq!(agg.member_count, 2);
        assert_eq!(agg.metrics_count, 2);
        assert!(
            agg.completed,
            "completed must be true when metrics match members"
        );
        assert_eq!(agg.max, 20);
        assert_eq!(agg.min, 10);
        assert_eq!(agg.avg, 15);
        assert_eq!(agg.total, 30);
        assert_eq!(
            agg.threshold, "16.5",
            "threshold is avg * 1.1 formatted to one decimal"
        );
    }
}
