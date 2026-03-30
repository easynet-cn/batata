//! V3 Admin instance management endpoints

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpRequest, Responder, delete, get, post, put, web};
use serde::{Deserialize, Serialize};
use tracing::info;

use batata_api::naming::model::Instance;
use batata_common::{
    ActionTypes, ApiType, DEFAULT_GROUP, DEFAULT_NAMESPACE_ID, SignType, impl_or_default,
};
use batata_server_common::{
    Secured, error, model::app_state::AppState, model::response::Result, secured,
};

use batata_api::naming::NamingServiceProvider;

const DEFAULT_CLUSTER: &str = "DEFAULT";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceRegisterForm {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupName")]
    group_name: Option<String>,
    #[serde(alias = "serviceName")]
    service_name: String,
    ip: String,
    port: i32,
    #[serde(default, alias = "clusterName")]
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
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);

    impl_or_default!(cluster_name_or_default, cluster_name, DEFAULT_CLUSTER);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceDeregisterQuery {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupName")]
    group_name: Option<String>,
    #[serde(alias = "serviceName")]
    service_name: String,
    ip: String,
    port: i32,
    #[serde(default, alias = "clusterName")]
    cluster_name: Option<String>,
    #[serde(default)]
    ephemeral: Option<bool>,
}

impl InstanceDeregisterQuery {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);

    impl_or_default!(cluster_name_or_default, cluster_name, DEFAULT_CLUSTER);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceDetailQuery {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupName")]
    group_name: Option<String>,
    #[serde(alias = "serviceName")]
    service_name: String,
    ip: String,
    port: i32,
    #[serde(default, alias = "clusterName")]
    cluster_name: Option<String>,
}

impl InstanceDetailQuery {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);

    impl_or_default!(cluster_name_or_default, cluster_name, DEFAULT_CLUSTER);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceListQuery {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupName")]
    group_name: Option<String>,
    #[serde(alias = "serviceName")]
    service_name: String,
    #[serde(default, alias = "clusterName")]
    cluster_name: Option<String>,
    #[serde(default, alias = "healthyOnly")]
    healthy_only: Option<bool>,
}

impl InstanceListQuery {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataUpdateForm {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupName")]
    group_name: Option<String>,
    #[serde(alias = "serviceName")]
    service_name: String,
    #[serde(default, alias = "consistencyType")]
    #[allow(dead_code)]
    consistency_type: Option<String>,
    instances: String,
    metadata: String,
}

impl MetadataUpdateForm {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);
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
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    form: web::Form<InstanceRegisterForm>,
) -> impl Responder {
    if form.service_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            "Required parameter 'serviceName' is missing".to_string(),
            false,
        );
    }

    if form.ip.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            "Required parameter 'ip' is missing".to_string(),
            false,
        );
    }

    if form.port <= 0 {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
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
        instance_id: batata_api::naming::model::generate_instance_id(
            &form.ip,
            form.port,
            cluster_name,
            &form.service_name,
        ),
        ip: form.ip.clone(),
        port: form.port,
        weight: form.weight.unwrap_or(1.0),
        healthy: form.healthy.unwrap_or(true),
        enabled: form.enabled.unwrap_or(true),
        ephemeral: form.ephemeral.unwrap_or(true),
        cluster_name: cluster_name.to_string(),
        service_name: form.service_name.clone(),
        metadata,
        register_source: batata_api::naming::RegisterSource::Batata,
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
        Result::<bool>::http_response(
            500,
            error::SERVER_ERROR.code,
            "Failed to register instance".to_string(),
            false,
        )
    }
}

/// DELETE /v3/admin/ns/instance
#[delete("")]
async fn deregister_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    connection_manager: Option<web::Data<Arc<dyn batata_core::ClientConnectionManager>>>,
    params: web::Query<InstanceDeregisterQuery>,
) -> impl Responder {
    if params.service_name.is_empty() || params.ip.is_empty() || params.port <= 0 {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
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
        instance_id: batata_api::naming::model::generate_instance_id(
            &params.ip,
            params.port,
            cluster_name,
            &params.service_name,
        ),
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
        // Notify gRPC subscribers about the instance change
        if let Some(ref cm) = connection_manager {
            let subscribers =
                naming_service.get_subscribers(namespace_id, group_name, &params.service_name);
            if !subscribers.is_empty() {
                let service_info = naming_service.get_service(
                    namespace_id,
                    group_name,
                    &params.service_name,
                    "",
                    false,
                );
                let notification = batata_api::naming::model::NotifySubscriberRequest::for_service(
                    namespace_id,
                    group_name,
                    &params.service_name,
                    service_info,
                );
                use batata_api::remote::model::RequestTrait;
                let payload = notification.build_server_push_payload();
                let cm = cm.clone().into_inner();
                let subs = subscribers;
                tokio::spawn(async move {
                    cm.push_message_to_many(&subs, payload).await;
                });
            }
        }
        Result::<bool>::http_success(true)
    } else {
        Result::<bool>::http_response(
            404,
            error::INSTANCE_NOT_FOUND.code,
            "Instance not found".to_string(),
            false,
        )
    }
}

/// PUT /v3/admin/ns/instance
#[put("")]
async fn update_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    connection_manager: Option<web::Data<Arc<dyn batata_core::ClientConnectionManager>>>,
    form: web::Form<InstanceRegisterForm>,
) -> impl Responder {
    if form.service_name.is_empty() || form.ip.is_empty() || form.port <= 0 {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
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
        instance_id: batata_api::naming::model::generate_instance_id(
            &form.ip,
            form.port,
            cluster_name,
            &form.service_name,
        ),
        ip: form.ip.clone(),
        port: form.port,
        weight: form.weight.unwrap_or(1.0),
        healthy: form.healthy.unwrap_or(true),
        enabled: form.enabled.unwrap_or(true),
        ephemeral: form.ephemeral.unwrap_or(true),
        cluster_name: cluster_name.to_string(),
        service_name: form.service_name.clone(),
        metadata,
        register_source: batata_api::naming::RegisterSource::Batata,
    };

    naming_service.register_instance(namespace_id, group_name, &form.service_name, instance);

    // Notify gRPC subscribers about the instance update
    if let Some(ref cm) = connection_manager {
        let subscribers =
            naming_service.get_subscribers(namespace_id, group_name, &form.service_name);
        if !subscribers.is_empty() {
            let service_info =
                naming_service.get_service(namespace_id, group_name, &form.service_name, "", false);
            let notification = batata_api::naming::model::NotifySubscriberRequest::for_service(
                namespace_id,
                group_name,
                &form.service_name,
                service_info,
            );
            use batata_api::remote::model::RequestTrait;
            let payload = notification.build_server_push_payload();
            let cm = cm.clone().into_inner();
            let subs = subscribers;
            tokio::spawn(async move {
                cm.push_message_to_many(&subs, payload).await;
            });
        }
    }

    Result::<bool>::http_success(true)
}

/// GET /v3/admin/ns/instance
#[get("")]
async fn get_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    params: web::Query<InstanceDetailQuery>,
) -> impl Responder {
    if params.service_name.is_empty() || params.ip.is_empty() || params.port <= 0 {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
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

    let instances = naming_service.get_instances_by_source(
        namespace_id,
        group_name,
        &params.service_name,
        cluster_name,
        false,
        Some(batata_api::naming::RegisterSource::Batata),
    );

    let instance_key =
        crate::service::build_instance_key_parts(&params.ip, params.port, cluster_name);

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
            error::RESOURCE_NOT_FOUND.code,
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
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    params: web::Query<InstanceListQuery>,
) -> impl Responder {
    if params.service_name.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
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

    let mut instances = naming_service.get_instances_by_source(
        namespace_id,
        group_name,
        &params.service_name,
        cluster,
        false,
        Some(batata_api::naming::RegisterSource::Batata),
    );

    if healthy_only {
        instances.retain(|i| i.healthy);
    }

    // Return List<Instance> directly (not wrapped in InstanceListResponse)
    // to match Nacos maintainer client's expected format
    Result::<Vec<batata_api::naming::Instance>>::http_success(instances)
}

/// PUT /v3/admin/ns/instance/metadata/batch
#[put("metadata/batch")]
async fn update_metadata(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    form: web::Form<MetadataUpdateForm>,
) -> impl Responder {
    if form.service_name.is_empty() || form.instances.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
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
            error::PARAMETER_VALIDATE_ERROR.code,
            "Invalid metadata format".to_string(),
            false,
        );
    }

    let instance_keys = parse_instance_keys(&form.instances);

    let instances = naming_service.get_instances_by_source(
        namespace_id,
        group_name,
        &form.service_name,
        "",
        false,
        Some(batata_api::naming::RegisterSource::Batata),
    );

    for instance in instances {
        let matches = instance_keys
            .iter()
            .any(|(ip, port)| instance.ip == *ip && instance.port == *port);

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

/// PUT /v3/admin/ns/instance/partial
///
/// Partially updates an instance. Only provided fields are updated.
#[put("partial")]
async fn partial_update_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    form: web::Form<InstanceRegisterForm>,
) -> impl Responder {
    if form.service_name.is_empty() || form.ip.is_empty() || form.port <= 0 {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
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

    let instances = naming_service.get_instances_by_source(
        namespace_id,
        group_name,
        &form.service_name,
        cluster_name,
        false,
        Some(batata_api::naming::RegisterSource::Batata),
    );

    let instance_key = crate::service::build_instance_key_parts(&form.ip, form.port, cluster_name);

    if let Some(mut existing) = instances.into_iter().find(|i| i.key() == instance_key) {
        if let Some(weight) = form.weight {
            existing.weight = weight;
        }
        if let Some(healthy) = form.healthy {
            existing.healthy = healthy;
        }
        if let Some(enabled) = form.enabled {
            existing.enabled = enabled;
        }
        if let Some(ephemeral) = form.ephemeral {
            existing.ephemeral = ephemeral;
        }
        if let Some(ref metadata_str) = form.metadata
            && let Ok(metadata) = serde_json::from_str::<HashMap<String, String>>(metadata_str)
        {
            for (k, v) in metadata {
                existing.metadata.insert(k, v);
            }
        }

        let result = naming_service.register_instance(
            namespace_id,
            group_name,
            &form.service_name,
            existing,
        );

        if result {
            Result::<bool>::http_success(true)
        } else {
            Result::<bool>::http_response(
                500,
                error::SERVER_ERROR.code,
                "Failed to partial update instance".to_string(),
                false,
            )
        }
    } else {
        Result::<bool>::http_response(
            404,
            error::RESOURCE_NOT_FOUND.code,
            format!(
                "instance not found, ip={}, port={}, cluster={}",
                form.ip, form.port, cluster_name
            ),
            false,
        )
    }
}

/// Parse instance keys from the instances parameter.
/// Supports two formats:
/// 1. JSON array of Instance objects: [{"ip":"10.0.0.1","port":8080,...}, ...]
/// 2. Comma-separated ip:port pairs: "10.0.0.1:8080,10.0.0.2:8080"
fn parse_instance_keys(instances_str: &str) -> Vec<(String, i32)> {
    // Try JSON array format first (Nacos maintainer client format)
    if let Ok(instances) = serde_json::from_str::<Vec<serde_json::Value>>(instances_str) {
        return instances
            .iter()
            .filter_map(|v| {
                let ip = v.get("ip").and_then(|v| v.as_str())?;
                let port = v.get("port").and_then(|v| v.as_i64())? as i32;
                Some((ip.to_string(), port))
            })
            .collect();
    }

    // Fall back to comma-separated ip:port format
    instances_str
        .split(',')
        .filter_map(|s| {
            let parts: Vec<&str> = s.trim().split(':').collect();
            if parts.len() == 2 {
                let port = parts[1].parse::<i32>().ok()?;
                Some((parts[0].to_string(), port))
            } else {
                None
            }
        })
        .collect()
}

/// Parse keys to delete from the metadata parameter.
/// Supports two formats:
/// 1. JSON object: {"key1":"val1","key2":"val2"} - keys of the object are deleted
/// 2. JSON array of strings: ["key1","key2"]
fn parse_metadata_keys_to_delete(metadata_str: &str) -> Vec<String> {
    // Try JSON object format first (Nacos maintainer client format)
    if let Ok(obj) = serde_json::from_str::<HashMap<String, serde_json::Value>>(metadata_str) {
        return obj.into_keys().collect();
    }
    // Try JSON array of strings
    serde_json::from_str::<Vec<String>>(metadata_str).unwrap_or_default()
}

/// DELETE /v3/admin/ns/instance/metadata/batch
///
/// Batch delete metadata keys from multiple instances.
#[delete("metadata/batch")]
async fn delete_metadata_batch(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    params: web::Query<MetadataUpdateForm>,
) -> impl Responder {
    if params.service_name.is_empty() || params.instances.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            "Required parameters 'serviceName' and 'instances' are missing".to_string(),
            false,
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

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

    let keys_to_delete = parse_metadata_keys_to_delete(&params.metadata);

    if keys_to_delete.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            "Invalid metadata format".to_string(),
            false,
        );
    }

    let instance_keys = parse_instance_keys(&params.instances);

    if instance_keys.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            "Invalid instances format".to_string(),
            false,
        );
    }

    let instances = naming_service.get_instances_by_source(
        namespace_id,
        group_name,
        &params.service_name,
        "",
        false,
        Some(batata_api::naming::RegisterSource::Batata),
    );

    for instance in instances {
        let matches = instance_keys
            .iter()
            .any(|(ip, port)| instance.ip == *ip && instance.port == *port);

        if matches {
            let mut updated_instance = instance.clone();
            for key in &keys_to_delete {
                updated_instance.metadata.remove(key);
            }
            naming_service.register_instance(
                namespace_id,
                group_name,
                &params.service_name,
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
        .service(partial_update_instance)
        .service(get_instance)
        .service(list_instances)
        .service(update_metadata)
        .service(delete_metadata_batch)
}
