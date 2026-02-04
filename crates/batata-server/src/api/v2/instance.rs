//! V2 Instance API handlers
//!
//! Implements the Nacos V2 instance management API endpoints:
//! - POST /nacos/v2/ns/instance - Register instance
//! - DELETE /nacos/v2/ns/instance - Deregister instance
//! - PUT /nacos/v2/ns/instance - Update instance
//! - GET /nacos/v2/ns/instance - Get instance detail
//! - GET /nacos/v2/ns/instance/list - Get instance list
//! - PUT /nacos/v2/ns/instance/metadata/batch - Batch update metadata
//! - DELETE /nacos/v2/ns/instance/metadata/batch - Batch delete metadata

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, delete, get, post, put, web};
use tracing::info;

use crate::{
    ActionTypes, ApiType, Secured, SignType, api::naming::model::Instance, model::common::AppState,
    model::response::Result, secured, service::naming::NamingService,
};

use super::model::{
    BatchMetadataParam, InstanceDeregisterParam, InstanceDetailParam, InstanceListParam,
    InstanceListResponse, InstanceRegisterParam, InstanceResponse, InstanceUpdateParam,
};

/// Register instance
///
/// POST /nacos/v2/ns/instance
///
/// Registers an instance to a service.
#[post("")]
pub async fn register_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Form<InstanceRegisterParam>,
) -> impl Responder {
    // Validate required parameters
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

    // Check authorization
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

    // Parse metadata if provided
    let metadata: HashMap<String, String> = form
        .metadata
        .as_ref()
        .and_then(|m| serde_json::from_str(m).ok())
        .unwrap_or_default();

    // Build instance
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

    // Register instance
    let result =
        naming_service.register_instance(namespace_id, group_name, &form.service_name, instance);

    if result {
        info!(
            namespace_id = %namespace_id,
            group_name = %group_name,
            service_name = %form.service_name,
            ip = %form.ip,
            port = %form.port,
            "Instance registered successfully"
        );
        Result::<bool>::http_success(true)
    } else {
        Result::<bool>::http_response(500, 500, "Failed to register instance".to_string(), false)
    }
}

/// Deregister instance
///
/// DELETE /nacos/v2/ns/instance
///
/// Deregisters an instance from a service.
#[delete("")]
pub async fn deregister_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<InstanceDeregisterParam>,
) -> impl Responder {
    // Validate required parameters
    if params.service_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'serviceName' is missing".to_string(),
            false,
        );
    }

    if params.ip.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'ip' is missing".to_string(),
            false,
        );
    }

    if params.port <= 0 {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'port' is invalid".to_string(),
            false,
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();
    let cluster_name = params.cluster_name_or_default();

    // Check authorization
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

    // Build instance for deregistration
    let instance = Instance {
        instance_id: format!("{}#{}#{}", params.ip, params.port, cluster_name),
        ip: params.ip.clone(),
        port: params.port,
        cluster_name: cluster_name.to_string(),
        service_name: params.service_name.clone(),
        ephemeral: params.ephemeral.unwrap_or(true),
        ..Default::default()
    };

    // Deregister instance
    let result = naming_service.deregister_instance(
        namespace_id,
        group_name,
        &params.service_name,
        &instance,
    );

    if result {
        info!(
            namespace_id = %namespace_id,
            group_name = %group_name,
            service_name = %params.service_name,
            ip = %params.ip,
            port = %params.port,
            "Instance deregistered successfully"
        );
        Result::<bool>::http_success(true)
    } else {
        Result::<bool>::http_response(404, 404, "Instance not found".to_string(), false)
    }
}

/// Update instance
///
/// PUT /nacos/v2/ns/instance
///
/// Updates an existing instance.
#[put("")]
pub async fn update_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Form<InstanceUpdateParam>,
) -> impl Responder {
    // Validate required parameters
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

    // Check authorization
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

    // Parse metadata if provided
    let metadata: HashMap<String, String> = form
        .metadata
        .as_ref()
        .and_then(|m| serde_json::from_str(m).ok())
        .unwrap_or_default();

    // Build updated instance
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

    // Update by re-registering (which updates if exists)
    let result =
        naming_service.register_instance(namespace_id, group_name, &form.service_name, instance);

    if result {
        info!(
            namespace_id = %namespace_id,
            group_name = %group_name,
            service_name = %form.service_name,
            ip = %form.ip,
            port = %form.port,
            "Instance updated successfully"
        );
        Result::<bool>::http_success(true)
    } else {
        Result::<bool>::http_response(500, 500, "Failed to update instance".to_string(), false)
    }
}

/// Get instance detail
///
/// GET /nacos/v2/ns/instance
///
/// Retrieves details of a specific instance.
#[get("")]
pub async fn get_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<InstanceDetailParam>,
) -> impl Responder {
    // Validate required parameters
    if params.service_name.is_empty() {
        return Result::<String>::http_response(
            400,
            400,
            "Required parameter 'serviceName' is missing".to_string(),
            String::new(),
        );
    }

    if params.ip.is_empty() {
        return Result::<String>::http_response(
            400,
            400,
            "Required parameter 'ip' is missing".to_string(),
            String::new(),
        );
    }

    if params.port <= 0 {
        return Result::<String>::http_response(
            400,
            400,
            "Required parameter 'port' is invalid".to_string(),
            String::new(),
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();
    let cluster_name = params.cluster_name_or_default();

    // Check authorization
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

    // Get all instances and find the specific one
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

/// Get instance list
///
/// GET /nacos/v2/ns/instance/list
///
/// Retrieves list of instances for a service.
#[get("list")]
pub async fn get_instance_list(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<InstanceListParam>,
) -> impl Responder {
    // Validate required parameters
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

    // Check authorization
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

    // Get service with all info
    let service = naming_service.get_service(
        namespace_id,
        group_name,
        &params.service_name,
        clusters,
        healthy_only,
    );

    // Transform instances to response format
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

/// Batch update instance metadata
///
/// PUT /nacos/v2/ns/instance/metadata/batch
///
/// Updates metadata for multiple instances.
#[put("metadata/batch")]
pub async fn batch_update_metadata(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Form<BatchMetadataParam>,
) -> impl Responder {
    // Validate required parameters
    if form.service_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'serviceName' is missing".to_string(),
            false,
        );
    }

    if form.instances.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'instances' is missing".to_string(),
            false,
        );
    }

    let namespace_id = form.namespace_id_or_default();
    let group_name = form.group_name_or_default();

    // Check authorization
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

    // Parse metadata
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

    // Parse instance list (format: "ip:port,ip:port,...")
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

    if instance_keys.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Invalid instances format. Expected: ip:port,ip:port,...".to_string(),
            false,
        );
    }

    // Get existing instances and update metadata
    let instances =
        naming_service.get_instances(namespace_id, group_name, &form.service_name, "", false);

    let mut updated_count = 0;
    for instance in instances {
        let matches = instance_keys
            .iter()
            .any(|(ip, port)| instance.ip == *ip && instance.port.to_string() == *port);

        if matches {
            // Merge metadata and re-register
            let mut updated_instance = instance.clone();
            for (k, v) in &metadata {
                updated_instance.metadata.insert(k.clone(), v.clone());
            }

            if naming_service.register_instance(
                namespace_id,
                group_name,
                &form.service_name,
                updated_instance,
            ) {
                updated_count += 1;
            }
        }
    }

    info!(
        namespace_id = %namespace_id,
        group_name = %group_name,
        service_name = %form.service_name,
        updated_count = %updated_count,
        "Batch metadata update completed"
    );

    Result::<bool>::http_success(true)
}

/// Batch delete instance metadata
///
/// DELETE /nacos/v2/ns/instance/metadata/batch
///
/// Deletes metadata keys from multiple instances.
#[delete("metadata/batch")]
pub async fn batch_delete_metadata(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<BatchMetadataParam>,
) -> impl Responder {
    // Validate required parameters
    if params.service_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'serviceName' is missing".to_string(),
            false,
        );
    }

    if params.instances.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameter 'instances' is missing".to_string(),
            false,
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

    // Check authorization
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

    // Parse metadata keys to delete
    let keys_to_delete: Vec<String> = serde_json::from_str(&params.metadata).unwrap_or_default();

    if keys_to_delete.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Invalid metadata format. Expected JSON array of keys".to_string(),
            false,
        );
    }

    // Parse instance list
    let instance_keys: Vec<(&str, &str)> = params
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

    if instance_keys.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Invalid instances format".to_string(),
            false,
        );
    }

    // Get existing instances and delete metadata keys
    let instances =
        naming_service.get_instances(namespace_id, group_name, &params.service_name, "", false);

    let mut updated_count = 0;
    for instance in instances {
        let matches = instance_keys
            .iter()
            .any(|(ip, port)| instance.ip == *ip && instance.port.to_string() == *port);

        if matches {
            let mut updated_instance = instance.clone();
            for key in &keys_to_delete {
                updated_instance.metadata.remove(key);
            }

            if naming_service.register_instance(
                namespace_id,
                group_name,
                &params.service_name,
                updated_instance,
            ) {
                updated_count += 1;
            }
        }
    }

    info!(
        namespace_id = %namespace_id,
        group_name = %group_name,
        service_name = %params.service_name,
        updated_count = %updated_count,
        "Batch metadata delete completed"
    );

    Result::<bool>::http_success(true)
}
