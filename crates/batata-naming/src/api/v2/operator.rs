//! V2 Operator API handlers
//!
//! Implements the Nacos V2 operator management API endpoints:
//! - GET /nacos/v2/ns/operator/switches - Get system switches
//! - PUT /nacos/v2/ns/operator/switches - Update system switches
//! - GET /nacos/v2/ns/operator/metrics - Get naming service metrics
//!
//! Nacos registers these under both `/v2/ns/operator` and `/v2/ns/ops` (dual path).

use std::sync::Arc;

use actix_web::{HttpRequest, Responder, get, put, web};

use batata_common::{ActionTypes, ApiType, SignType};
use batata_server_common::error;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};

use batata_api::naming::NamingServiceProvider;

use serde::Deserialize;

use super::model::{NamingMetricsResponse, SwitchUpdateParam, SwitchesResponse};

/// Request parameters for metrics endpoint
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsParam {
    #[serde(default = "default_true", alias = "onlyStatus")]
    pub only_status: bool,
}

fn default_true() -> bool {
    true
}

/// Get system switches (internal implementation)
fn build_switches_response(config: &batata_server_common::Configuration) -> SwitchesResponse {
    use super::model::SwitchHealthParams;
    SwitchesResponse {
        name: "00-00---000-NACOS_SWITCH_DOMAIN-000---00-00".to_string(),
        masters: None,
        default_push_cache_millis: 10000,
        client_beat_interval: 5000,
        default_cache_millis: 3000,
        distro_threshold: 0.7,
        health_check_enabled: config.is_health_check(),
        auto_change_health_check_enabled: true,
        distro_enabled: !config.is_standalone(),
        enable_standalone: true,
        push_enabled: true,
        check_times: config.max_health_check_fail_count(),
        http_health_params: SwitchHealthParams {
            max: 5000,
            min: 500,
            factor: 0.85,
        },
        tcp_health_params: SwitchHealthParams {
            max: 5000,
            min: 1000,
            factor: 0.75,
        },
        mysql_health_params: SwitchHealthParams {
            max: 3000,
            min: 2000,
            factor: 0.65,
        },
        incremental_list: vec![],
        default_instance_ephemeral: true,
        light_beat_enabled: true,
        disable_add_ip: false,
        send_beat_only: false,
        overridden_server_status: None,
    }
}

/// Shared implementation for updating switches.
async fn do_update_switches(
    req: &HttpRequest,
    data: &web::Data<AppState>,
    form: &SwitchUpdateParam,
) -> actix_web::HttpResponse {
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
        Secured::builder(req, data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Apply via SwitchDomain if available
    if let Some(switch_domain) =
        req.app_data::<web::Data<std::sync::Arc<crate::switch_domain::SwitchDomain>>>()
    {
        if switch_domain.update(&form.entry, &form.value) {
            tracing::info!(
                entry = %form.entry,
                value = %form.value,
                "Switch updated successfully"
            );
        } else {
            tracing::warn!(entry = %form.entry, "Unknown switch entry");
        }
    } else {
        tracing::warn!(
            entry = %form.entry,
            value = %form.value,
            "Switch update: SwitchDomain not registered"
        );
    }

    Result::<String>::http_success(format!("ok, entry: {}, value: {}", form.entry, form.value))
}

/// Shared implementation for getting metrics.
async fn do_get_metrics(
    req: &HttpRequest,
    data: &web::Data<AppState>,
    naming_service: &web::Data<Arc<dyn NamingServiceProvider>>,
    params: &MetricsParam,
) -> actix_web::HttpResponse {
    // Check authorization for operator operations
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(req, data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // If onlyStatus is true (default), return only the status string
    if params.only_status {
        let status = data.server_status.status().to_string();
        return Result::<String>::http_success(status);
    }

    // Get service keys to count services and instances
    let service_keys = naming_service.get_all_service_keys();
    let service_count = service_keys.len() as i32;

    // Count total instances across all services
    let mut instance_count = 0;
    for key in &service_keys {
        let parts: Vec<&str> = key.split("@@").collect();
        if parts.len() == 3 {
            instance_count += naming_service
                .get_instances(parts[0], parts[1], parts[2], "", false)
                .len() as i32;
        }
    }

    // Get subscriber count (approximate - count unique subscriber connections)
    let subscriber_ids = naming_service.get_all_subscriber_ids();
    let subscribe_count = subscriber_ids.len() as i32;

    // Get cluster node count
    let cluster_node_count = data.cluster_manager().member_count() as i32;

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

/// Get system switches
///
/// GET /nacos/v2/ns/operator/switches
///
/// Returns the current system configuration switches.
#[get("switches")]
pub async fn get_switches(data: web::Data<AppState>) -> impl Responder {
    let response = build_switches_response(&data.configuration);
    Result::<SwitchesResponse>::http_success(response)
}

/// Update system switches
///
/// PUT /nacos/v2/ns/operator/switches
///
/// Updates a specific system configuration switch.
#[put("switches")]
pub async fn update_switches(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<SwitchUpdateParam>,
) -> impl Responder {
    do_update_switches(&req, &data, &form).await
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
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    params: web::Query<MetricsParam>,
) -> impl Responder {
    do_get_metrics(&req, &data, &naming_service, &params).await
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
    do_update_switches(&req, &data, &form).await
}

pub async fn get_metrics_handler(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    params: web::Query<MetricsParam>,
) -> impl Responder {
    do_get_metrics(&req, &data, &naming_service, &params).await
}
