//! V3 Admin naming operator endpoints

use std::sync::Arc;

use actix_web::{HttpRequest, Responder, get, put, web};
use serde::{Deserialize, Serialize};

use batata_common::{ActionTypes, ApiType, SignType};
use batata_server_common::{Secured, model::app_state::AppState, model::response::Result, secured};

use crate::service::NamingService;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SwitchesResponse {
    name: String,
    default_push_cache_millis: i64,
    client_beat_interval: i64,
    default_cache_millis: i64,
    distro_threshold: f64,
    health_check_enabled: bool,
    distro_enabled: bool,
    push_enabled: bool,
    check_times: i32,
    incremental_list: bool,
    default_instance_ephemeral: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SwitchUpdateParam {
    entry: String,
    value: String,
    #[serde(default)]
    debug: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NamingMetricsResponse {
    service_count: i32,
    instance_count: i32,
    subscribe_count: i32,
    cluster_node_count: i32,
    responsible_service_count: i32,
    responsible_instance_count: i32,
    cpu: f64,
    load: f64,
    mem: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogLevelParam {
    log_name: String,
    log_level: String,
}

/// GET /v3/admin/ns/ops/switches
#[get("switches")]
async fn get_switches(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let config = &data.configuration;

    let response = SwitchesResponse {
        name: "00-00---000-NACOS_SWITCH_DOMAIN-000---00-00".to_string(),
        default_push_cache_millis: 10000,
        client_beat_interval: 5000,
        default_cache_millis: 3000,
        distro_threshold: 0.7,
        health_check_enabled: config.is_health_check(),
        distro_enabled: !config.is_standalone(),
        push_enabled: true,
        check_times: config.max_health_check_fail_count(),
        incremental_list: true,
        default_instance_ephemeral: true,
    };

    Result::<SwitchesResponse>::http_success(response)
}

/// PUT /v3/admin/ns/ops/switches
#[put("switches")]
async fn update_switches(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SwitchUpdateParam>,
) -> impl Responder {
    if params.entry.is_empty() || params.value.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameters 'entry' and 'value' are missing".to_string(),
            false,
        );
    }

    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!(
        entry = %params.entry,
        value = %params.value,
        "Switch update requested via admin API"
    );

    Result::<String>::http_success(format!(
        "ok, entry: {}, value: {}",
        params.entry, params.value
    ))
}

/// GET /v3/admin/ns/ops/metrics
#[get("metrics")]
async fn get_metrics(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
) -> impl Responder {
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

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

/// PUT /v3/admin/ns/ops/log
#[put("log")]
async fn set_log_level(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<LogLevelParam>,
) -> impl Responder {
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!(
        log_name = %params.log_name,
        log_level = %params.log_level,
        "Log level change requested via admin API"
    );

    Result::<bool>::http_success(true)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/ops")
        .service(get_switches)
        .service(update_switches)
        .service(get_metrics)
        .service(set_log_level)
}
