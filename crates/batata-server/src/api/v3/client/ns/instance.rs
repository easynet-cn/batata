//! V3 Client instance SDK API endpoints

use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    ActionTypes, ApiType, Secured, SignType, api::naming::model::Instance, model::common::AppState,
    model::response::Result, secured, service::naming::NamingService,
};
use actix_web::{HttpMessage, HttpRequest, Responder, delete, get, post, web};
use serde::{Deserialize, Serialize};

const DEFAULT_NAMESPACE_ID: &str = "public";
const DEFAULT_GROUP: &str = "DEFAULT_GROUP";
const DEFAULT_CLUSTER: &str = "DEFAULT";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceRegisterForm {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    ip: String,
    port: i32,
    #[serde(default)]
    cluster_name: Option<String>,
    #[serde(default)]
    weight: Option<f64>,
    #[serde(default)]
    healthy: Option<bool>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    ephemeral: Option<bool>,
    #[serde(default)]
    metadata: Option<String>,
}

impl InstanceRegisterForm {
    fn namespace_id_or_default(&self) -> &str {
        self.namespace_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_NAMESPACE_ID)
    }

    fn group_name_or_default(&self) -> &str {
        self.group_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_GROUP)
    }

    fn cluster_name_or_default(&self) -> &str {
        self.cluster_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_CLUSTER)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceDeregisterQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    ip: String,
    port: i32,
    #[serde(default)]
    cluster_name: Option<String>,
    #[serde(default)]
    ephemeral: Option<bool>,
}

impl InstanceDeregisterQuery {
    fn namespace_id_or_default(&self) -> &str {
        self.namespace_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_NAMESPACE_ID)
    }

    fn group_name_or_default(&self) -> &str {
        self.group_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_GROUP)
    }

    fn cluster_name_or_default(&self) -> &str {
        self.cluster_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_CLUSTER)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceListQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    #[serde(default)]
    clusters: Option<String>,
    #[serde(default)]
    healthy_only: Option<bool>,
}

impl InstanceListQuery {
    fn namespace_id_or_default(&self) -> &str {
        self.namespace_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_NAMESPACE_ID)
    }

    fn group_name_or_default(&self) -> &str {
        self.group_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_GROUP)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceResponse {
    instance_id: String,
    ip: String,
    port: i32,
    weight: f64,
    healthy: bool,
    enabled: bool,
    ephemeral: bool,
    cluster_name: String,
    service_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceListResponse {
    name: String,
    group_name: String,
    clusters: String,
    cache_millis: i64,
    hosts: Vec<InstanceResponse>,
    last_ref_time: i64,
    checksum: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reach_protection_threshold: Option<bool>,
}

/// POST /v3/client/ns/instance
#[post("")]
async fn register_or_beat(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Form<InstanceRegisterForm>,
) -> impl Responder {
    if form.service_name.is_empty() || form.ip.is_empty() || form.port <= 0 {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameters 'serviceName', 'ip', 'port' are missing or invalid".to_string(),
            false,
        );
    }

    let namespace_id = form.namespace_id_or_default();
    let group_name = form.group_name_or_default();
    let cluster_name = form.cluster_name_or_default();

    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, form.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let metadata: HashMap<String, String> = form
        .metadata
        .as_ref()
        .and_then(|m| serde_json::from_str(m).ok())
        .unwrap_or_default();

    let instance = Instance {
        instance_id: format!("{}#{}#{}", form.ip, form.port, cluster_name),
        ip: form.ip.clone(),
        port: form.port,
        weight: form.weight.unwrap_or(1.0),
        healthy: form.healthy.unwrap_or(true),
        enabled: form.enabled.unwrap_or(true),
        ephemeral: form.ephemeral.unwrap_or(true),
        cluster_name: cluster_name.to_string(),
        service_name: form.service_name.clone(),
        metadata,
        instance_heart_beat_interval: 5000,
        instance_heart_beat_time_out: 15000,
        ip_delete_timeout: 30000,
        instance_id_generator: String::new(),
    };

    naming_service.register_instance(namespace_id, group_name, &form.service_name, instance);

    Result::<bool>::http_success(true)
}

/// DELETE /v3/client/ns/instance
#[delete("")]
async fn deregister(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<InstanceDeregisterQuery>,
) -> impl Responder {
    if params.service_name.is_empty() || params.ip.is_empty() || params.port <= 0 {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameters missing or invalid".to_string(),
            false,
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();
    let cluster_name = params.cluster_name_or_default();

    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, params.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let instance = Instance {
        instance_id: format!("{}#{}#{}", params.ip, params.port, cluster_name),
        ip: params.ip.clone(),
        port: params.port,
        cluster_name: cluster_name.to_string(),
        service_name: params.service_name.clone(),
        ephemeral: params.ephemeral.unwrap_or(true),
        ..Default::default()
    };

    naming_service.deregister_instance(namespace_id, group_name, &params.service_name, &instance);

    Result::<bool>::http_success(true)
}

/// GET /v3/client/ns/instance/list
#[get("list")]
async fn list_instances(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<InstanceListQuery>,
) -> impl Responder {
    if params.service_name.is_empty() {
        return Result::<String>::http_response(
            400,
            400,
            "Required parameter 'serviceName' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();
    let clusters = params.clusters.as_deref().unwrap_or("");
    let healthy_only = params.healthy_only.unwrap_or(false);

    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, params.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let service = naming_service.get_service(
        namespace_id,
        group_name,
        &params.service_name,
        clusters,
        healthy_only,
    );

    let hosts: Vec<InstanceResponse> = service
        .hosts
        .into_iter()
        .map(|i| InstanceResponse {
            instance_id: i.instance_id,
            ip: i.ip,
            port: i.port,
            weight: i.weight,
            healthy: i.healthy,
            enabled: i.enabled,
            ephemeral: i.ephemeral,
            cluster_name: i.cluster_name,
            service_name: i.service_name,
            metadata: if i.metadata.is_empty() {
                None
            } else {
                Some(i.metadata)
            },
        })
        .collect();

    let response = InstanceListResponse {
        name: service.name,
        group_name: service.group_name,
        clusters: service.clusters,
        cache_millis: service.cache_millis,
        hosts,
        last_ref_time: service.last_ref_time,
        checksum: service.checksum,
        reach_protection_threshold: if service.reach_protection_threshold {
            Some(true)
        } else {
            None
        },
    };

    Result::<InstanceListResponse>::http_success(response)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/instance")
        .service(register_or_beat)
        .service(deregister)
        .service(list_instances)
}
