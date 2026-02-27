//! V3 Admin service management endpoints
//!
//! Provides HTTP handlers for service CRUD operations on the admin API.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, delete, get, post, put, web};
use serde::{Deserialize, Serialize};

use batata_naming::service::ServiceMetadata;

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured, service::naming::NamingService,
};

const DEFAULT_NAMESPACE_ID: &str = "public";
const DEFAULT_GROUP: &str = "DEFAULT_GROUP";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ServiceListQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    #[serde(default)]
    service_name: Option<String>,
    #[serde(default = "default_page_no")]
    page_no: u64,
    #[serde(default = "default_page_size")]
    page_size: u64,
}

fn default_page_no() -> u64 {
    1
}

fn default_page_size() -> u64 {
    20
}

impl ServiceListQuery {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceDetailQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
}

impl ServiceDetailQuery {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceForm {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    #[serde(default)]
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceListResponse {
    count: i32,
    service_list: Vec<ServiceInfoResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceInfoResponse {
    name: String,
    group_name: String,
    cluster_count: i32,
    ip_count: i32,
    healthy_instance_count: i32,
    trigger_flag: bool,
    protect_threshold: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<SelectorResponse>,
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
    naming_service: web::Data<Arc<NamingService>>,
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

    let (total_count, service_names) = naming_service.list_services(
        namespace_id,
        group_name,
        params.page_no as i32,
        params.page_size as i32,
    );

    let service_list: Vec<ServiceInfoResponse> = service_names
        .iter()
        .map(|name| {
            let instances = naming_service.get_instances(namespace_id, group_name, name, "", false);
            let clusters: HashSet<_> = instances.iter().map(|i| i.cluster_name.clone()).collect();
            let healthy_count = instances.iter().filter(|i| i.healthy && i.enabled).count();
            let metadata_opt = naming_service.get_service_metadata(namespace_id, group_name, name);
            let (protect_threshold, metadata, selector) = if let Some(meta) = metadata_opt {
                let sel = if meta.selector_type != "none" && !meta.selector_type.is_empty() {
                    Some(SelectorResponse {
                        r#type: meta.selector_type,
                        expression: if meta.selector_expression.is_empty() {
                            None
                        } else {
                            Some(meta.selector_expression)
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
                        Some(meta.metadata)
                    },
                    sel,
                )
            } else {
                (0.0, None, None)
            };

            ServiceInfoResponse {
                name: name.clone(),
                group_name: group_name.to_string(),
                cluster_count: clusters.len() as i32,
                ip_count: instances.len() as i32,
                healthy_instance_count: healthy_count as i32,
                trigger_flag: false,
                protect_threshold,
                metadata,
                selector,
            }
        })
        .collect();

    let response = ServiceListResponse {
        count: total_count,
        service_list,
    };

    Result::<ServiceListResponse>::http_success(response)
}

/// GET /v3/admin/ns/service
#[get("")]
async fn get_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
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
        return Result::<Option<ServiceInfoResponse>>::http_response(
            404,
            20004,
            format!("service {} not found", params.service_name),
            None::<ServiceInfoResponse>,
        );
    }

    let instances =
        naming_service.get_instances(namespace_id, group_name, &params.service_name, "", false);
    let clusters: HashSet<_> = instances.iter().map(|i| i.cluster_name.clone()).collect();
    let healthy_count = instances.iter().filter(|i| i.healthy && i.enabled).count();
    let metadata_opt =
        naming_service.get_service_metadata(namespace_id, group_name, &params.service_name);

    let (protect_threshold, metadata, selector) = if let Some(meta) = metadata_opt {
        let sel = if meta.selector_type != "none" && !meta.selector_type.is_empty() {
            Some(SelectorResponse {
                r#type: meta.selector_type,
                expression: if meta.selector_expression.is_empty() {
                    None
                } else {
                    Some(meta.selector_expression)
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
                Some(meta.metadata)
            },
            sel,
        )
    } else {
        (0.0, None, None)
    };

    let response = ServiceInfoResponse {
        name: params.service_name.clone(),
        group_name: group_name.to_string(),
        cluster_count: clusters.len() as i32,
        ip_count: instances.len() as i32,
        healthy_instance_count: healthy_count as i32,
        trigger_flag: false,
        protect_threshold,
        metadata,
        selector,
    };

    Result::<ServiceInfoResponse>::http_success(response)
}

/// POST /v3/admin/ns/service
#[post("")]
async fn create_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Json<ServiceForm>,
) -> impl Responder {
    if form.service_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
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
            400,
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
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Json<ServiceForm>,
) -> impl Responder {
    if form.service_name.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
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
            404,
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
    naming_service: web::Data<Arc<NamingService>>,
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
            404,
            format!("service {} not found", params.service_name),
            false,
        );
    }

    let instances =
        naming_service.get_instances(namespace_id, group_name, &params.service_name, "", false);
    if !instances.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
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
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    #[serde(default = "default_page_no")]
    page_no: u64,
    #[serde(default = "default_page_size")]
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
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<SubscriberQuery>,
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

pub fn routes() -> actix_web::Scope {
    web::scope("/service")
        .service(create_service)
        .service(delete_service)
        .service(update_service)
        .service(get_service)
        .service(list_services)
        .service(get_subscribers)
        .service(get_selector_types)
}
