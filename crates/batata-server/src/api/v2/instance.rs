//! V2 Instance API handlers
//!
//! Implements the Nacos V2 instance management API endpoints:
//! - POST /nacos/v2/ns/instance - Register instance
//! - DELETE /nacos/v2/ns/instance - Deregister instance
//! - PUT /nacos/v2/ns/instance - Update instance
//! - PATCH /nacos/v2/ns/instance - Patch instance (partial update)
//! - GET /nacos/v2/ns/instance - Get instance detail
//! - GET /nacos/v2/ns/instance/list - Get instance list
//! - PUT /nacos/v2/ns/instance/beat - Instance heartbeat
//! - GET /nacos/v2/ns/instance/statuses/{key} - Get instance statuses
//! - PUT /nacos/v2/ns/instance/metadata/batch - Batch update metadata
//! - DELETE /nacos/v2/ns/instance/metadata/batch - Batch delete metadata

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, delete, get, patch, post, put, web};
use serde::{Deserialize, Serialize};
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

    // Record heartbeat for health check tracking
    if let Some(ref hc_manager) = data.health_check_manager {
        hc_manager
            .unhealthy_checker()
            .record_heartbeat(
                namespace_id,
                group_name,
                &form.service_name,
                &form.ip,
                form.port,
                cluster_name,
                instance.instance_heart_beat_time_out,
                instance.ip_delete_timeout,
            );
    }

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

        // Remove heartbeat tracking
        if let Some(ref hc_manager) = data.health_check_manager {
            hc_manager.unhealthy_checker().remove_heartbeat(
                namespace_id,
                group_name,
                &params.service_name,
                &params.ip,
                params.port,
                cluster_name,
            );
        }

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

/// Patch instance (partial update)
///
/// PATCH /nacos/v2/ns/instance
///
/// Partially updates an instance. Only provided fields are updated.
#[patch("")]
pub async fn patch_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Form<InstanceUpdateParam>,
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

    // Get existing instance to merge
    let instances = naming_service.get_instances(
        namespace_id,
        group_name,
        &form.service_name,
        cluster_name,
        false,
    );

    let instance_key = format!("{}#{}#{}", form.ip, form.port, cluster_name);

    if let Some(mut existing) = instances.into_iter().find(|i| i.key() == instance_key) {
        // Patch only provided fields
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
            Result::<bool>::http_response(500, 500, "Failed to patch instance".to_string(), false)
        }
    } else {
        Result::<bool>::http_response(
            404,
            404,
            format!(
                "instance not found, ip={}, port={}, cluster={}",
                form.ip, form.port, cluster_name
            ),
            false,
        )
    }
}

/// Instance heartbeat
///
/// PUT /nacos/v2/ns/instance/beat
///
/// Sends a heartbeat for an instance to keep it alive.
#[put("beat")]
pub async fn beat_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Form<InstanceBeatParam>,
) -> impl Responder {
    if form.service_name.is_empty() {
        return Result::<BeatResponse>::http_response(
            400,
            400,
            "Required parameter 'serviceName' is missing".to_string(),
            BeatResponse::default(),
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
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Parse beat info if provided
    if let Some(ref beat_str) = form.beat
        && let Ok(beat_info) = serde_json::from_str::<BeatInfo>(beat_str)
    {
        let cluster_name = beat_info.cluster.as_deref().unwrap_or("DEFAULT");
        let result = naming_service.heartbeat(
            namespace_id,
            group_name,
            &form.service_name,
            &beat_info.ip,
            beat_info.port,
            cluster_name,
        );

        // Update heartbeat tracking
        if result {
            if let Some(ref hc_manager) = data.health_check_manager {
                hc_manager.unhealthy_checker().record_heartbeat(
                    namespace_id,
                    group_name,
                    &form.service_name,
                    &beat_info.ip,
                    beat_info.port,
                    cluster_name,
                    15000,  // default heartbeat_timeout
                    30000,  // default ip_delete_timeout
                );
            }
        }

        return Result::<BeatResponse>::http_success(BeatResponse {
            client_beat_interval: 5000,
            light_beat_enabled: true,
            code: if result { 200 } else { 20404 },
        });
    }

    // If no beat JSON, try ip + port parameters
    if let (Some(ip), Some(port)) = (&form.ip, form.port) {
        let cluster_name = form.cluster_name.as_deref().unwrap_or("DEFAULT");
        let result = naming_service.heartbeat(
            namespace_id,
            group_name,
            &form.service_name,
            ip,
            port,
            cluster_name,
        );

        // Update heartbeat tracking
        if result {
            if let Some(ref hc_manager) = data.health_check_manager {
                hc_manager.unhealthy_checker().record_heartbeat(
                    namespace_id,
                    group_name,
                    &form.service_name,
                    ip,
                    port,
                    cluster_name,
                    15000,  // default heartbeat_timeout
                    30000,  // default ip_delete_timeout
                );
            }
        }

        Result::<BeatResponse>::http_success(BeatResponse {
            client_beat_interval: 5000,
            light_beat_enabled: true,
            code: if result { 200 } else { 20404 },
        })
    } else {
        Result::<BeatResponse>::http_response(
            400,
            400,
            "Either 'beat' JSON or 'ip'+'port' parameters are required".to_string(),
            BeatResponse::default(),
        )
    }
}

/// Get instance statuses by service key
///
/// GET /nacos/v2/ns/instance/statuses/{key}
///
/// Returns the health status of all instances for a service key.
#[get("statuses/{key}")]
pub async fn get_instance_statuses(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    path: web::Path<String>,
) -> impl Responder {
    let key = path.into_inner();

    // Key format: namespace@@groupName@@serviceName or groupName@@serviceName
    let parts: Vec<&str> = key.split("@@").collect();
    let (namespace_id, group_name, service_name) = match parts.len() {
        3 => (parts[0], parts[1], parts[2]),
        2 => ("public", parts[0], parts[1]),
        _ => {
            return Result::<HashMap<String, bool>>::http_response(
                400,
                400,
                format!("Invalid service key format: {}", key),
                HashMap::<String, bool>::new(),
            );
        }
    };

    let resource = format!("{}:{}:naming/{}", namespace_id, group_name, service_name);
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let instances = naming_service.get_instances(namespace_id, group_name, service_name, "", false);

    let statuses: HashMap<String, bool> = instances
        .into_iter()
        .map(|i| (format!("{}#{}", i.ip, i.port), i.healthy))
        .collect();

    Result::<HashMap<String, bool>>::http_success(statuses)
}

/// Beat info from client
#[derive(Debug, Deserialize)]
struct BeatInfo {
    ip: String,
    port: i32,
    #[serde(default)]
    cluster: Option<String>,
}

/// Instance beat request parameters
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceBeatParam {
    #[serde(default)]
    pub namespace_id: Option<String>,
    #[serde(default)]
    pub group_name: Option<String>,
    pub service_name: String,
    #[serde(default)]
    pub beat: Option<String>,
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(default)]
    pub port: Option<i32>,
    #[serde(default)]
    pub cluster_name: Option<String>,
}

impl InstanceBeatParam {
    pub fn namespace_id_or_default(&self) -> &str {
        self.namespace_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("public")
    }

    pub fn group_name_or_default(&self) -> &str {
        self.group_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("DEFAULT_GROUP")
    }
}

/// Beat response
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeatResponse {
    pub client_beat_interval: i64,
    pub light_beat_enabled: bool,
    pub code: i32,
}
