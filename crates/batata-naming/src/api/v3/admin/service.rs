//! V3 Admin service management endpoints
//!
//! Provides HTTP handlers for service CRUD operations on the admin API.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use actix_web::{HttpRequest, Responder, delete, get, post, put, web};
use serde::{Deserialize, Serialize};

use batata_common::{
    ActionTypes, ApiType, DEFAULT_GROUP, DEFAULT_NAMESPACE_ID, SignType, default_page_no,
    default_page_size, impl_or_default,
};
use batata_server_common::{
    Secured, error, model::app_state::AppState, model::response::Result, secured,
};

use crate::service::ServiceMetadata;
use batata_api::naming::NamingServiceProvider;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceListQuery {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupName", alias = "groupNameParam")]
    group_name: Option<String>,
    #[serde(default, alias = "serviceName", alias = "serviceNameParam")]
    service_name: Option<String>,
    #[serde(default = "default_page_no", alias = "pageNo")]
    page_no: u64,
    #[serde(default = "default_page_size", alias = "pageSize")]
    page_size: u64,
    /// When true, return ServiceDetailInfo (with metadata, protectThreshold, etc.)
    #[serde(default, alias = "withInstances")]
    with_instances: bool,
    /// When true, exclude services that have no instances
    #[serde(default, alias = "ignoreEmptyService", alias = "hasIpCount")]
    ignore_empty_service: bool,
}

impl ServiceListQuery {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceDetailQuery {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupName")]
    group_name: Option<String>,
    #[serde(alias = "serviceName")]
    service_name: String,
}

impl ServiceDetailQuery {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceForm {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupName")]
    group_name: Option<String>,
    #[serde(alias = "serviceName")]
    service_name: String,
    #[serde(default)]
    ephemeral: Option<bool>,
    #[serde(default, alias = "protectThreshold")]
    protect_threshold: Option<f32>,
    #[serde(default)]
    metadata: Option<String>,
    #[serde(default)]
    selector: Option<String>,
}

impl ServiceForm {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);
}

/// Nacos-compatible Page response for service list.
/// Nacos returns `Page<ServiceView>` with `totalCount`, `pageNumber`, `pagesAvailable`, `pageItems`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceListResponse {
    total_count: i32,
    page_number: u64,
    pages_available: u64,
    page_items: Vec<ServiceInfoResponse>,
}

/// Service detail info response — aligned with Nacos ServiceDetailInfo.
/// The Java class uses `serviceName`, `groupName`, `namespaceId` etc.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceDetailResponse {
    namespace_id: String,
    service_name: String,
    group_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cluster_map: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, String>>,
    protect_threshold: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<SelectorResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ephemeral: Option<bool>,
}

/// Service info for list responses — aligned with Nacos `ServiceView` Java class.
/// Only includes fields that ServiceView has: name, groupName, clusterCount,
/// ipCount, healthyInstanceCount, triggerFlag (String).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceInfoResponse {
    name: String,
    group_name: String,
    cluster_count: i32,
    ip_count: i32,
    healthy_instance_count: i32,
    trigger_flag: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectorResponse {
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expression: Option<String>,
}

/// GET /v3/admin/ns/service/list
#[get("list")]
async fn list_services(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    params: web::Query<ServiceListQuery>,
) -> impl Responder {
    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

    let resource = format!("{}:{}:naming/*", namespace_id, group_name);
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    // Get all services, then filter by service name pattern if provided
    let (_, raw_names) = naming_service.list_services(
        namespace_id,
        group_name,
        1,        // get all from page 1
        i32::MAX, // large page to get all
    );

    // Apply service name filter (blur/contains match) if serviceNameParam is provided
    let name_filtered: Vec<String> = if let Some(ref sn) = params.service_name {
        if !sn.is_empty() {
            raw_names
                .into_iter()
                .filter(|n| n.contains(sn.as_str()))
                .collect()
        } else {
            raw_names
        }
    } else {
        raw_names
    };

    // Filter out empty services if ignoreEmptyService=true (zero-copy snapshot).
    let filtered_names: Vec<String> = if params.ignore_empty_service {
        name_filtered
            .into_iter()
            .filter(|name| {
                !naming_service
                    .get_instances_snapshot(namespace_id, group_name, name, "", false)
                    .is_empty()
            })
            .collect()
    } else {
        name_filtered
    };

    // Apply pagination to filtered results
    let total_count = filtered_names.len() as i32;
    let start = ((params.page_no.max(1) - 1) * params.page_size) as usize;
    let end = (start + params.page_size as usize).min(filtered_names.len());
    let service_names: Vec<String> = if start < filtered_names.len() {
        filtered_names[start..end].to_vec()
    } else {
        vec![]
    };

    let total = total_count as u64;
    let page_size_val = params.page_size.max(1);
    let pages_available = if page_size_val > 0 {
        (total as f64 / page_size_val as f64).ceil() as u64
    } else {
        0
    };

    if params.with_instances {
        // Return ServiceDetailInfo format (for maintainer client listServicesWithDetail)
        let detail_list: Vec<ServiceDetailResponse> = service_names
            .iter()
            .map(|name| {
                let metadata = naming_service.get_service_metadata(namespace_id, group_name, name);
                // Zero-copy snapshot — we only need cluster names.
                let instances = naming_service.get_instances_snapshot(
                    namespace_id,
                    group_name,
                    name,
                    "",
                    false,
                );

                // Build cluster map from real cluster configs
                let mut cluster_names: HashSet<String> =
                    instances.iter().map(|i| i.cluster_name.clone()).collect();
                let cluster_configs =
                    naming_service.get_all_cluster_configs(namespace_id, group_name, name);
                let cluster_config_map: HashMap<&str, _> = cluster_configs
                    .iter()
                    .map(|c| (c.name.as_str(), c))
                    .collect();
                for cfg in &cluster_configs {
                    cluster_names.insert(cfg.name.clone());
                }

                let cluster_map: HashMap<String, serde_json::Value> = cluster_names
                    .into_iter()
                    .map(|c| {
                        let config = cluster_config_map.get(c.as_str());
                        (
                            c.clone(),
                            serde_json::json!({
                                "clusterName": c,
                                "healthChecker": {
                                    "type": config.map(|cfg| cfg.health_check_type.as_str()).unwrap_or("TCP"),
                                },
                                "healthyCheckPort": config.map(|cfg| cfg.check_port).unwrap_or(80),
                                "useInstancePortForCheck": config.map(|cfg| cfg.use_instance_port).unwrap_or(true),
                                "metadata": config.map(|cfg| &cfg.metadata).cloned().unwrap_or_default(),
                            }),
                        )
                    })
                    .collect();

                ServiceDetailResponse {
                    namespace_id: namespace_id.to_string(),
                    service_name: name.clone(),
                    group_name: group_name.to_string(),
                    cluster_map: if cluster_map.is_empty() {
                        None
                    } else {
                        Some(cluster_map)
                    },
                    metadata: metadata.as_ref().and_then(|m| {
                        if m.metadata.is_empty() {
                            None
                        } else {
                            Some(m.metadata.clone())
                        }
                    }),
                    protect_threshold: metadata
                        .as_ref()
                        .map(|m| m.protect_threshold)
                        .unwrap_or(0.0),
                    selector: metadata.as_ref().and_then(|m| {
                        if m.selector_type.is_empty() || m.selector_type == "none" {
                            None
                        } else {
                            Some(SelectorResponse {
                                r#type: m.selector_type.clone(),
                                expression: Some(m.selector_expression.clone()),
                            })
                        }
                    }),
                    ephemeral: metadata.as_ref().map(|m| m.ephemeral),
                }
            })
            .collect();

        let response = serde_json::json!({
            "totalCount": total_count,
            "pageNumber": params.page_no,
            "pagesAvailable": pages_available,
            "pageItems": detail_list,
        });
        Result::<serde_json::Value>::http_success(response)
    } else {
        // Return ServiceView format (default list)
        let service_list: Vec<ServiceInfoResponse> = service_names
            .iter()
            .map(|name| {
                // Zero-copy snapshot — we only need counts and cluster names.
                let instances = naming_service.get_instances_snapshot(
                    namespace_id,
                    group_name,
                    name,
                    "",
                    false,
                );
                let clusters: HashSet<_> =
                    instances.iter().map(|i| i.cluster_name.clone()).collect();
                let healthy_count = instances.iter().filter(|i| i.healthy && i.enabled).count();

                ServiceInfoResponse {
                    name: name.clone(),
                    group_name: group_name.to_string(),
                    cluster_count: clusters.len() as i32,
                    ip_count: instances.len() as i32,
                    healthy_instance_count: healthy_count as i32,
                    trigger_flag: String::new(),
                }
            })
            .collect();

        let response = ServiceListResponse {
            total_count,
            page_number: params.page_no,
            pages_available,
            page_items: service_list,
        };
        Result::<ServiceListResponse>::http_success(response)
    }
}

/// GET /v3/admin/ns/service
#[get("")]
async fn get_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    params: web::Query<ServiceDetailQuery>,
) -> impl Responder {
    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

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

    if !naming_service.service_exists(namespace_id, group_name, &params.service_name) {
        return Result::<Option<ServiceDetailResponse>>::http_response(
            404,
            error::RESOURCE_NOT_FOUND.code,
            format!("service {} not found", params.service_name),
            None::<ServiceDetailResponse>,
        );
    }

    // Zero-copy snapshot — we only need cluster names.
    let instances = naming_service.get_instances_snapshot(
        namespace_id,
        group_name,
        &params.service_name,
        "",
        false,
    );
    let mut clusters: HashSet<_> = instances.iter().map(|i| i.cluster_name.clone()).collect();
    let metadata_opt =
        naming_service.get_service_metadata(namespace_id, group_name, &params.service_name);

    let (protect_threshold, metadata, selector, ephemeral) = if let Some(meta) = &metadata_opt {
        let sel = if meta.selector_type != "none" && !meta.selector_type.is_empty() {
            Some(SelectorResponse {
                r#type: meta.selector_type.clone(),
                expression: if meta.selector_expression.is_empty() {
                    None
                } else {
                    Some(meta.selector_expression.clone())
                },
            })
        } else {
            None
        };
        (
            meta.protect_threshold,
            if meta.metadata.is_empty() {
                None
            } else {
                Some(meta.metadata.clone())
            },
            sel,
            Some(meta.ephemeral),
        )
    } else {
        (0.0, None, None, None)
    };

    // Build clusterMap from real cluster configs — Nacos returns Map<String, ClusterInfo>
    let cluster_configs =
        naming_service.get_all_cluster_configs(namespace_id, group_name, &params.service_name);
    let cluster_config_map: HashMap<&str, _> = cluster_configs
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();

    // Merge cluster names from both instances and configs
    for cfg in &cluster_configs {
        clusters.insert(cfg.name.clone());
    }

    let cluster_map: HashMap<String, serde_json::Value> = clusters
        .into_iter()
        .map(|c| {
            let config = cluster_config_map.get(c.as_str());
            (
                c.clone(),
                serde_json::json!({
                    "clusterName": c,
                    "healthChecker": {
                        "type": config.map(|cfg| cfg.health_check_type.as_str()).unwrap_or("TCP"),
                    },
                    "healthyCheckPort": config.map(|cfg| cfg.check_port).unwrap_or(80),
                    "useInstancePortForCheck": config.map(|cfg| cfg.use_instance_port).unwrap_or(true),
                    "metadata": config.map(|cfg| &cfg.metadata).cloned().unwrap_or_default(),
                }),
            )
        })
        .collect();

    let response = ServiceDetailResponse {
        namespace_id: namespace_id.to_string(),
        service_name: params.service_name.clone(),
        group_name: group_name.to_string(),
        cluster_map: if cluster_map.is_empty() {
            None
        } else {
            Some(cluster_map)
        },
        metadata,
        protect_threshold,
        selector,
        ephemeral,
    };

    Result::<ServiceDetailResponse>::http_success(response)
}

/// POST /v3/admin/ns/service
#[post("")]
async fn create_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    form: web::Form<ServiceForm>,
) -> impl Responder {
    if form.service_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            "Required parameter 'serviceName' is missing".to_string(),
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

    if naming_service.service_exists(namespace_id, group_name, &form.service_name) {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            format!("service {} already exists", form.service_name),
            false,
        );
    }

    let metadata: HashMap<String, String> = form
        .metadata
        .as_ref()
        .and_then(|m| serde_json::from_str(m).ok())
        .unwrap_or_default();

    let (selector_type, selector_expression) = if let Some(selector) = &form.selector {
        let selector_obj: serde_json::Value = serde_json::from_str(selector).unwrap_or_default();
        (
            selector_obj["type"].as_str().unwrap_or("none").to_string(),
            selector_obj["expression"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        )
    } else {
        ("none".to_string(), String::new())
    };

    let service_metadata = ServiceMetadata {
        protect_threshold: form.protect_threshold.unwrap_or(0.0),
        metadata,
        selector_type,
        selector_expression,
        ephemeral: form.ephemeral.unwrap_or(false),
        ..Default::default()
    };

    naming_service.set_service_metadata(
        namespace_id,
        group_name,
        &form.service_name,
        service_metadata,
    );

    Result::<bool>::http_success(true)
}

/// PUT /v3/admin/ns/service
#[put("")]
async fn update_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    form: web::Form<ServiceForm>,
) -> impl Responder {
    if form.service_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            "Required parameter 'serviceName' is missing".to_string(),
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

    if !naming_service.service_exists(namespace_id, group_name, &form.service_name) {
        return Result::<bool>::http_response(
            404,
            error::RESOURCE_NOT_FOUND.code,
            format!("service {} not found", form.service_name),
            false,
        );
    }

    if let Some(threshold) = form.protect_threshold {
        naming_service.update_service_protect_threshold(
            namespace_id,
            group_name,
            &form.service_name,
            threshold,
        );
    }

    if let Some(metadata_str) = &form.metadata
        && let Ok(metadata) = serde_json::from_str::<HashMap<String, String>>(metadata_str)
    {
        naming_service.update_service_metadata_map(
            namespace_id,
            group_name,
            &form.service_name,
            metadata,
        );
    }

    if let Some(selector) = &form.selector {
        let selector_obj: serde_json::Value = serde_json::from_str(selector).unwrap_or_default();
        let selector_type = selector_obj["type"].as_str().unwrap_or("none");
        let selector_expression = selector_obj["expression"].as_str().unwrap_or("");

        naming_service.update_service_selector(
            namespace_id,
            group_name,
            &form.service_name,
            selector_type,
            selector_expression,
        );
    }

    Result::<bool>::http_success(true)
}

/// DELETE /v3/admin/ns/service
#[delete("")]
async fn delete_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    params: web::Query<ServiceDetailQuery>,
) -> impl Responder {
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

    if !naming_service.service_exists(namespace_id, group_name, &params.service_name) {
        return Result::<bool>::http_response(
            404,
            error::RESOURCE_NOT_FOUND.code,
            format!("service {} not found", params.service_name),
            false,
        );
    }

    // Zero-copy snapshot — empty-check only.
    let instances = naming_service.get_instances_snapshot(
        namespace_id,
        group_name,
        &params.service_name,
        "",
        false,
    );
    if !instances.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            format!(
                "service {} has {} instances, cannot delete",
                params.service_name,
                instances.len()
            ),
            false,
        );
    }

    naming_service.delete_service_metadata(namespace_id, group_name, &params.service_name);

    Result::<bool>::http_success(true)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubscriberQuery {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupName")]
    group_name: Option<String>,
    #[serde(alias = "serviceName")]
    service_name: String,
    #[serde(default = "default_page_no", alias = "pageNo")]
    page_no: u64,
    #[serde(default = "default_page_size", alias = "pageSize")]
    page_size: u64,
}

impl SubscriberQuery {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubscriberInfoResponse {
    addr_str: String,
    agent: String,
}

/// GET /v3/admin/ns/service/subscribers
#[get("subscribers")]
async fn get_subscribers(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    params: web::Query<SubscriberQuery>,
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

    let subscriber_ids =
        naming_service.get_subscribers(namespace_id, group_name, &params.service_name);

    let total = subscriber_ids.len() as i32;
    let start = ((params.page_no - 1) * params.page_size) as usize;
    let end = (start + params.page_size as usize).min(subscriber_ids.len());
    let page_items: Vec<SubscriberInfoResponse> = subscriber_ids
        .get(start..end)
        .unwrap_or_default()
        .iter()
        .map(|s| SubscriberInfoResponse {
            addr_str: s.clone(),
            agent: s.clone(),
        })
        .collect();

    let response = serde_json::json!({
        "totalCount": total,
        "pageNumber": params.page_no,
        "pagesAvailable": (total as f64 / params.page_size as f64).ceil() as i32,
        "pageItems": page_items,
    });

    Result::<serde_json::Value>::http_success(response)
}

/// GET /v3/admin/ns/service/selector/types
#[get("selector/types")]
async fn get_selector_types(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let types = vec!["none".to_string(), "label".to_string()];
    Result::<Vec<String>>::http_success(types)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateClusterForm {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupName")]
    group_name: Option<String>,
    #[serde(alias = "serviceName")]
    service_name: String,
    #[serde(alias = "clusterName")]
    cluster_name: String,
    #[serde(alias = "checkPort")]
    check_port: Option<i32>,
    #[serde(alias = "useInstancePort4Check")]
    use_instance_port4_check: Option<bool>,
    #[serde(default, alias = "healthChecker")]
    health_checker: Option<ClusterHealthCheckerForm>,
    #[serde(default)]
    metadata: Option<HashMap<String, String>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClusterHealthCheckerForm {
    #[serde(default = "default_check_type")]
    r#type: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    headers: Option<String>,
}

fn default_check_type() -> String {
    "TCP".to_string()
}

impl UpdateClusterForm {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);
}

/// PUT /v3/admin/ns/service/cluster
///
/// Update cluster configuration within a service context.
/// This matches Nacos V3 ServiceControllerV3.updateCluster behavior.
#[put("cluster")]
async fn update_service_cluster(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    form: web::Form<UpdateClusterForm>,
) -> impl Responder {
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

    if form.health_checker.is_some()
        || form.check_port.is_some()
        || form.use_instance_port4_check.is_some()
    {
        let check_type = form
            .health_checker
            .as_ref()
            .map(|c| c.r#type.as_str())
            .unwrap_or("TCP");
        let check_port = form.check_port.unwrap_or(80);
        let use_instance_port = form.use_instance_port4_check.unwrap_or(true);
        naming_service.update_cluster_health_check(
            namespace_id,
            group_name,
            &form.service_name,
            &form.cluster_name,
            check_type,
            check_port,
            use_instance_port,
        );
    }

    if let Some(metadata) = &form.metadata {
        naming_service.update_cluster_metadata(
            namespace_id,
            group_name,
            &form.service_name,
            &form.cluster_name,
            metadata.clone(),
        );
    }

    Result::<bool>::http_success(true)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/service")
        .service(create_service)
        .service(delete_service)
        .service(update_service)
        .service(get_service)
        .service(list_services)
        .service(get_subscribers)
        .service(get_selector_types)
        .service(update_service_cluster)
}
