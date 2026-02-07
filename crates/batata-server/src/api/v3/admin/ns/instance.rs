//! V3 Admin instance management endpoints

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, delete, get, post, put, web};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    ActionTypes, ApiType, Secured, SignType, api::naming::model::Instance, model::common::AppState,
    model::response::Result, secured, service::naming::NamingService,
};

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
struct InstanceDetailQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    ip: String,
    port: i32,
    #[serde(default)]
    cluster_name: Option<String>,
}

impl InstanceDetailQuery {
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
    cluster_name: Option<String>,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataUpdateForm {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    instances: String,
    metadata: String,
}

impl MetadataUpdateForm {
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
    hosts: Vec<Instance>,
}

/// POST /v3/admin/ns/instance
#[post("")]
async fn register_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Json<InstanceRegisterForm>,
) -> impl Responder {
    if form.service_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'serviceName' is missing".to_string(),
            false,
        );
    }

    if form.ip.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'ip' is missing".to_string(),
            false,
        );
    }

    if form.port <= 0 {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'port' is invalid".to_string(),
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
            .api_type(ApiType::AdminApi)
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

    let result =
        naming_service.register_instance(namespace_id, group_name, &form.service_name, instance);

    if result {
        info!(
            namespace_id = %namespace_id,
            group_name = %group_name,
            service_name = %form.service_name,
            ip = %form.ip,
            port = %form.port,
            "Instance registered via admin API"
        );
        Result::<bool>::http_success(true)
    } else {
        Result::<bool>::http_response(500, 500, "Failed to register instance".to_string(), false)
    }
}

/// DELETE /v3/admin/ns/instance
#[delete("")]
async fn deregister_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<InstanceDeregisterQuery>,
) -> impl Responder {
    if params.service_name.is_empty() || params.ip.is_empty() || params.port <= 0 {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameters 'serviceName', 'ip', 'port' are missing or invalid".to_string(),
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
            .api_type(ApiType::AdminApi)
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

    let result = naming_service.deregister_instance(
        namespace_id,
        group_name,
        &params.service_name,
        &instance,
    );

    if result {
        Result::<bool>::http_success(true)
    } else {
        Result::<bool>::http_response(404, 404, "Instance not found".to_string(), false)
    }
}

/// PUT /v3/admin/ns/instance
#[put("")]
async fn update_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Json<InstanceRegisterForm>,
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
            .api_type(ApiType::AdminApi)
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

/// GET /v3/admin/ns/instance
#[get("")]
async fn get_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<InstanceDetailQuery>,
) -> impl Responder {
    if params.service_name.is_empty() || params.ip.is_empty() || params.port <= 0 {
        return Result::<String>::http_response(
            400,
            400,
            "Required parameters missing or invalid".to_string(),
            String::new(),
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
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let instances = naming_service.get_instances(
        namespace_id,
        group_name,
        &params.service_name,
        cluster_name,
        false,
    );

    let instance_key = format!("{}#{}#{}", params.ip, params.port, cluster_name);

    if let Some(instance) = instances.into_iter().find(|i| i.key() == instance_key) {
        let response = InstanceResponse {
            instance_id: instance.instance_id,
            ip: instance.ip,
            port: instance.port,
            weight: instance.weight,
            healthy: instance.healthy,
            enabled: instance.enabled,
            ephemeral: instance.ephemeral,
            cluster_name: instance.cluster_name,
            service_name: instance.service_name,
            metadata: if instance.metadata.is_empty() {
                None
            } else {
                Some(instance.metadata)
            },
        };
        Result::<InstanceResponse>::http_success(response)
    } else {
        Result::<Option<InstanceResponse>>::http_response(
            404,
            404,
            format!(
                "instance not found, ip={}, port={}, cluster={}",
                params.ip, params.port, cluster_name
            ),
            None::<InstanceResponse>,
        )
    }
}

/// GET /v3/admin/ns/instance/list
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
    let cluster = params.cluster_name.as_deref().unwrap_or("");
    let healthy_only = params.healthy_only.unwrap_or(false);

    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, params.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let mut instances = naming_service.get_instances(
        namespace_id,
        group_name,
        &params.service_name,
        cluster,
        false,
    );

    if healthy_only {
        instances.retain(|i| i.healthy);
    }

    let response = InstanceListResponse { hosts: instances };

    Result::<InstanceListResponse>::http_success(response)
}

/// PUT /v3/admin/ns/instance/metadata
#[put("metadata")]
async fn update_metadata(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Json<MetadataUpdateForm>,
) -> impl Responder {
    if form.service_name.is_empty() || form.instances.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameters 'serviceName' and 'instances' are missing".to_string(),
            false,
        );
    }

    let namespace_id = form.namespace_id_or_default();
    let group_name = form.group_name_or_default();

    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, form.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let metadata: HashMap<String, String> =
        serde_json::from_str(&form.metadata).unwrap_or_default();

    if metadata.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Invalid metadata format".to_string(),
            false,
        );
    }

    let instance_keys: Vec<(&str, &str)> = form
        .instances
        .split(',')
        .filter_map(|s| {
            let parts: Vec<&str> = s.trim().split(':').collect();
            if parts.len() == 2 {
                Some((parts[0], parts[1]))
            } else {
                None
            }
        })
        .collect();

    let instances =
        naming_service.get_instances(namespace_id, group_name, &form.service_name, "", false);

    for instance in instances {
        let matches = instance_keys
            .iter()
            .any(|(ip, port)| instance.ip == *ip && instance.port.to_string() == *port);

        if matches {
            let mut updated_instance = instance.clone();
            for (k, v) in &metadata {
                updated_instance.metadata.insert(k.clone(), v.clone());
            }
            naming_service.register_instance(
                namespace_id,
                group_name,
                &form.service_name,
                updated_instance,
            );
        }
    }

    Result::<bool>::http_success(true)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/instance")
        .service(register_instance)
        .service(deregister_instance)
        .service(update_instance)
        .service(get_instance)
        .service(list_instances)
        .service(update_metadata)
}
