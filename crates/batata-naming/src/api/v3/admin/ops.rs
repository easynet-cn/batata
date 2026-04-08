//! V3 Admin naming operator endpoints

use std::sync::Arc;

use actix_web::{HttpRequest, Responder, get, put, web};
use serde::{Deserialize, Serialize};

use batata_common::{ActionTypes, ApiType, SignType};
use batata_server_common::{
    Secured, error, model::app_state::AppState, model::response::Result, secured,
};

use batata_api::naming::NamingServiceProvider;

/// Matches Nacos SwitchDomain serialization (all fields from the Java class).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SwitchesResponse {
    /// Fixed constant: UtilsAndCommons.SWITCH_DOMAIN_NAME
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    masters: Option<Vec<String>>,
    default_push_cache_millis: i64,
    client_beat_interval: i64,
    default_cache_millis: i64,
    distro_threshold: f32,
    health_check_enabled: bool,
    auto_change_health_check_enabled: bool,
    distro_enabled: bool,
    enable_standalone: bool,
    push_enabled: bool,
    check_times: i32,
    http_health_params: HealthParams,
    tcp_health_params: HealthParams,
    mysql_health_params: HealthParams,
    incremental_list: Vec<String>,
    default_instance_ephemeral: bool,
    light_beat_enabled: bool,
    disable_add_ip: bool,
    send_beat_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    overridden_server_status: Option<String>,
}

/// Health check timing parameters (matches Nacos SwitchDomain.HealthParams)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthParams {
    max: i32,
    min: i32,
    factor: f32,
}

impl HealthParams {
    fn http() -> Self {
        Self {
            max: 5000,
            min: 500,
            factor: 0.85,
        }
    }
    fn tcp() -> Self {
        Self {
            max: 5000,
            min: 1000,
            factor: 0.75,
        }
    }
    fn mysql() -> Self {
        Self {
            max: 3000,
            min: 2000,
            factor: 0.65,
        }
    }
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
    #[serde(alias = "logName")]
    log_name: String,
    #[serde(alias = "logLevel")]
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

    use std::sync::atomic::Ordering::Relaxed;

    // Read from SwitchDomain if available (reflects runtime changes), fallback to config
    let response = if let Some(sd) =
        req.app_data::<web::Data<std::sync::Arc<crate::switch_domain::SwitchDomain>>>()
    {
        SwitchesResponse {
            name: "00-00---000-NACOS_SWITCH_DOMAIN-000---00-00".to_string(),
            masters: None,
            default_push_cache_millis: sd.default_push_cache_millis.load(Relaxed),
            client_beat_interval: sd.client_beat_interval.load(Relaxed),
            default_cache_millis: sd.default_cache_millis.load(Relaxed),
            distro_threshold: 0.7,
            health_check_enabled: sd.is_health_check_enabled(),
            auto_change_health_check_enabled: true,
            distro_enabled: sd.is_distro_enabled(),
            enable_standalone: true,
            push_enabled: sd.is_push_enabled(),
            check_times: sd.get_check_times(),
            http_health_params: HealthParams::http(),
            tcp_health_params: HealthParams::tcp(),
            mysql_health_params: HealthParams::mysql(),
            incremental_list: vec![],
            default_instance_ephemeral: sd.default_instance_ephemeral.load(Relaxed),
            light_beat_enabled: true,
            disable_add_ip: false,
            send_beat_only: false,
            overridden_server_status: None,
        }
    } else {
        let config = &data.configuration;
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
            http_health_params: HealthParams::http(),
            tcp_health_params: HealthParams::tcp(),
            mysql_health_params: HealthParams::mysql(),
            incremental_list: vec![],
            default_instance_ephemeral: true,
            light_beat_enabled: true,
            disable_add_ip: false,
            send_beat_only: false,
            overridden_server_status: None,
        }
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
            error::PARAMETER_VALIDATE_ERROR.code,
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

    // Try to apply the switch change via SwitchDomain if available
    if let Some(switch_domain) =
        req.app_data::<web::Data<std::sync::Arc<crate::switch_domain::SwitchDomain>>>()
    {
        if switch_domain.update(&params.entry, &params.value) {
            tracing::info!(
                entry = %params.entry,
                value = %params.value,
                "Switch updated successfully"
            );
        } else {
            tracing::warn!(
                entry = %params.entry,
                "Unknown switch entry"
            );
        }
    } else {
        tracing::warn!(
            entry = %params.entry,
            value = %params.value,
            "Switch update requested but SwitchDomain not registered"
        );
    }

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
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
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
            instance_count += naming_service
                .get_instances(parts[0], parts[1], parts[2], "", false)
                .len() as i32;
        }
    }

    let subscriber_ids = naming_service.get_all_subscriber_ids();
    let subscribe_count = subscriber_ids.len() as i32;
    let cluster_node_count = data.cluster_manager().member_count() as i32;

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

    // Build filter directive
    let filter_directive = if params.log_name.is_empty() || params.log_name == "root" {
        params.log_level.clone()
    } else {
        format!("{}={}", params.log_name, params.log_level)
    };

    if let Some(ref setter) = data.log_level_setter {
        match setter(&filter_directive) {
            Ok(()) => {
                tracing::info!(
                    log_name = %params.log_name,
                    log_level = %params.log_level,
                    "Log level changed successfully"
                );
                Result::<bool>::http_success(true)
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to change log level");
                Result::<bool>::http_response(500, 500, format!("Failed: {}", e), false)
            }
        }
    } else {
        tracing::warn!("Log level change requested but no log_level_setter configured");
        Result::<bool>::http_success(true)
    }
}

pub fn routes() -> actix_web::Scope {
    web::scope("/ops")
        .service(get_switches)
        .service(update_switches)
        .service(get_metrics)
        .service(set_log_level)
}
