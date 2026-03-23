//! V3 Admin server loader endpoints
//!
//! Implements SDK connection load balancing across cluster nodes.
//! Following the Nacos 3.x ServerLoaderController/NacosServerLoaderService pattern.

use std::sync::Arc;

use actix_web::{HttpRequest, Responder, get, post, web};
use serde::{Deserialize, Serialize};

use batata_core::service::remote::ConnectionManager;

use crate::{
    ActionTypes, ApiType, Secured, SignType, error, model::common::AppState,
    model::response::Result, secured,
};

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
    redirect_address: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SmartReloadParam {
    loader_factor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReloadClientParam {
    connection_id: Option<String>,
    redirect_address: Option<String>,
}

/// Get the current node's loader metric using real system metrics
fn get_self_loader_metric(
    address: &str,
    connection_manager: &ConnectionManager,
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
    connection_manager: web::Data<Arc<ConnectionManager>>,
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
    let connections: Vec<ConnectionInfo> = ids
        .into_iter()
        .map(|id| ConnectionInfo { connection_id: id })
        .collect();

    Result::<Vec<ConnectionInfo>>::http_success(connections)
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
    connection_manager: web::Data<Arc<ConnectionManager>>,
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
    connection_manager: web::Data<Arc<ConnectionManager>>,
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
    let self_metric = get_self_loader_metric(&self_member.address, &connection_manager);
    let all_members = data.cluster_manager().all_members_extended();
    // In standalone mode, only self metrics available
    // Build metrics with just self (cluster mode would gather from all nodes via gRPC)
    let mut details = vec![self_metric.clone()];

    // For other cluster members, we report 0 connections (in a real cluster,
    // we'd query via gRPC ServerLoaderInfoRequest - same as Nacos does)
    for member in &all_members {
        if member.address != self_member.address {
            details.push(ServerLoaderMetric {
                address: member.address.clone(),
                sdk_con_count: 0,
                con_count: 0,
                load: "0.00".to_string(),
                cpu: "0.0".to_string(),
            });
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
    connection_manager: web::Data<Arc<ConnectionManager>>,
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
    connection_manager: web::Data<Arc<ConnectionManager>>,
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
    let self_metric = get_self_loader_metric(&self_member.address, &connection_manager);

    let mut details = vec![self_metric];

    // For other cluster members, we'd ideally query via gRPC.
    // In standalone mode, only self is available.
    for member in &all_members {
        if member.address != self_member.address {
            details.push(ServerLoaderMetric {
                address: member.address.clone(),
                sdk_con_count: 0,
                con_count: 0,
                load: "0.00".to_string(),
                cpu: "0.0".to_string(),
            });
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
