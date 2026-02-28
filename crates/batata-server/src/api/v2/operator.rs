//! V2 Operator API handlers
//!
//! Implements the Nacos V2 operator management API endpoints:
//! - GET /nacos/v2/ns/operator/switches - Get system switches
//! - PUT /nacos/v2/ns/operator/switches - Update system switches
//! - GET /nacos/v2/ns/operator/metrics - Get naming service metrics
//!
//! Nacos registers these under both `/v2/ns/operator` and `/v2/ns/ops` (dual path).

use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, get, put, web};

use crate::{
    ActionTypes, ApiType, Secured, SignType, error, model::common::AppState,
    model::response::Result, secured, service::naming::NamingService,
};

use serde::Deserialize;

use super::model::{NamingMetricsResponse, SwitchUpdateParam, SwitchesResponse};

/// Request parameters for metrics endpoint
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsParam {
    #[serde(default = "default_true")]
    pub only_status: bool,
}

fn default_true() -> bool {
    true
}

/// Get system switches (internal implementation)
fn build_switches_response(config: &crate::model::common::Configuration) -> SwitchesResponse {
    SwitchesResponse {
        name: "00-00---000-NACOS_SWITCH_DOMAIN-000---00-00".to_string(),
        masters: None,
        addr_server_domain: None,
        addr_server_port: None,
        default_push_cache_millis: 10000,
        client_beat_interval: 5000,
        default_cache_millis: 3000,
        distro_threshold: 0.7,
        health_check_enabled: config.is_health_check(),
        distro_enabled: !config.is_standalone(),
        push_enabled: true,
        check_times: config.max_health_check_fail_count(),
        http_health_params: None,
        tcp_health_params: None,
        mysql_health_params: None,
        incremental_list: true,
        servers: None,
        overridden_server_status: None,
        default_instance_ephemeral: true,
    }
}

/// Get system switches
///
/// GET /nacos/v2/ns/operator/switches
///
/// Returns the current system configuration switches.
#[get("")]
pub async fn get_switches(data: web::Data<AppState>) -> impl Responder {
    let response = build_switches_response(&data.configuration);
    Result::<SwitchesResponse>::http_success(response)
}

/// Update system switches
///
/// PUT /nacos/v2/ns/operator/switches
///
/// Updates a specific system configuration switch.
#[put("")]
pub async fn update_switches(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<SwitchUpdateParam>,
) -> impl Responder {
    if form.entry.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'entry' is missing".to_string(),
            false,
        );
    }

    if form.value.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'value' is missing".to_string(),
            false,
        );
    }

    // Check authorization for operator operations
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Note: In a full implementation, switches would be stored in a mutable state
    // and synchronized across the cluster. For now, we acknowledge the update
    // but don't persist it since configuration is loaded at startup.

    tracing::info!(
        entry = %form.entry,
        value = %form.value,
        debug = ?form.debug,
        "Switch update requested (not persisted - configuration is read-only)"
    );

    // Return success - actual switch persistence would require additional infrastructure
    Result::<String>::http_success(format!("ok, entry: {}, value: {}", form.entry, form.value))
}

/// Get naming service metrics
///
/// GET /nacos/v2/ns/operator/metrics
///
/// Returns metrics about the naming service including counts and resource usage.
#[get("metrics")]
pub async fn get_metrics(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<MetricsParam>,
) -> impl Responder {
    // Check authorization for operator operations
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // If onlyStatus is true (default), return only the status string
    if params.only_status {
        return Result::<String>::http_success("UP".to_string());
    }

    // Get service keys to count services and instances
    let service_keys = naming_service.get_all_service_keys();
    let service_count = service_keys.len() as i32;

    // Count total instances across all services
    let mut instance_count = 0;
    for key in &service_keys {
        let parts: Vec<&str> = key.split("@@").collect();
        if parts.len() == 3 {
            instance_count +=
                naming_service.get_instance_count(parts[0], parts[1], parts[2]) as i32;
        }
    }

    // Get subscriber count (approximate - count unique subscriber connections)
    let subscriber_ids = naming_service.get_all_subscriber_ids();
    let subscribe_count = subscriber_ids.len() as i32;

    // Get cluster node count
    let cluster_node_count = data.member_manager().all_members().len() as i32;

    // In standalone mode, this node is responsible for everything
    let is_standalone = data.configuration.is_standalone();
    let (responsible_service_count, responsible_instance_count) = if is_standalone {
        (service_count, instance_count)
    } else {
        // In cluster mode, would need to calculate based on distro protocol
        // For simplicity, estimate as total / cluster_size
        let divisor = cluster_node_count.max(1);
        (service_count / divisor, instance_count / divisor)
    };

    // Get system metrics
    let sys_info = sysinfo::System::new_all();
    let cpu_usage = sys_info.global_cpu_usage() as f64;
    let total_memory = sys_info.total_memory() as f64;
    let used_memory = sys_info.used_memory() as f64;
    let memory_usage = if total_memory > 0.0 {
        (used_memory / total_memory) * 100.0
    } else {
        0.0
    };

    let response = NamingMetricsResponse {
        service_count,
        instance_count,
        subscribe_count,
        cluster_node_count,
        responsible_service_count,
        responsible_instance_count,
        cpu: cpu_usage,
        load: memory_usage,
        mem: used_memory,
    };

    Result::<NamingMetricsResponse>::http_success(response)
}

// Plain handler functions for /v2/ns/ops/* dual-path registration (without attribute macros).
// These delegate to the same logic as the macro-annotated handlers above.

pub async fn get_switches_handler(data: web::Data<AppState>) -> impl Responder {
    let response = build_switches_response(&data.configuration);
    Result::<SwitchesResponse>::http_success(response)
}

pub async fn update_switches_handler(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<SwitchUpdateParam>,
) -> impl Responder {
    if form.entry.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'entry' is missing".to_string(),
            false,
        );
    }
    if form.value.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'value' is missing".to_string(),
            false,
        );
    }
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );
    tracing::info!(
        entry = %form.entry,
        value = %form.value,
        debug = ?form.debug,
        "Switch update requested via /ops alias (not persisted)"
    );
    Result::<String>::http_success(format!("ok, entry: {}, value: {}", form.entry, form.value))
}

pub async fn get_metrics_handler(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<MetricsParam>,
) -> impl Responder {
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // If onlyStatus is true (default), return only the status string
    if params.only_status {
        return Result::<String>::http_success("UP".to_string());
    }

    let service_keys = naming_service.get_all_service_keys();
    let service_count = service_keys.len() as i32;
    let mut instance_count = 0;
    for key in &service_keys {
        let parts: Vec<&str> = key.split("@@").collect();
        if parts.len() == 3 {
            instance_count +=
                naming_service.get_instance_count(parts[0], parts[1], parts[2]) as i32;
        }
    }
    let subscriber_ids = naming_service.get_all_subscriber_ids();
    let subscribe_count = subscriber_ids.len() as i32;
    let cluster_node_count = data.member_manager().all_members().len() as i32;
    let is_standalone = data.configuration.is_standalone();
    let (responsible_service_count, responsible_instance_count) = if is_standalone {
        (service_count, instance_count)
    } else {
        let divisor = cluster_node_count.max(1);
        (service_count / divisor, instance_count / divisor)
    };
    let sys_info = sysinfo::System::new_all();
    let cpu_usage = sys_info.global_cpu_usage() as f64;
    let total_memory = sys_info.total_memory() as f64;
    let used_memory = sys_info.used_memory() as f64;
    let memory_usage = if total_memory > 0.0 {
        (used_memory / total_memory) * 100.0
    } else {
        0.0
    };
    let response = NamingMetricsResponse {
        service_count,
        instance_count,
        subscribe_count,
        cluster_node_count,
        responsible_service_count,
        responsible_instance_count,
        cpu: cpu_usage,
        load: memory_usage,
        mem: used_memory,
    };
    Result::<NamingMetricsResponse>::http_success(response)
}
