//! V3 Console service management endpoints
//!
//! Provides HTTP handlers for service discovery operations on the main server.

use std::collections::HashMap;

use actix_web::{
    HttpMessage, HttpRequest, HttpResponse, Responder, Scope, delete, get, post, put, web,
};
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured,
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
    #[serde(default)]
    has_ip_count: Option<bool>,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateClusterForm {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    cluster_name: String,
    #[serde(default)]
    health_checker: Option<HealthCheckerForm>,
    #[serde(default)]
    metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct HealthCheckerForm {
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

/// GET /ns/service/list
#[get("list")]
async fn list_services(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ServiceListQuery>,
) -> impl Responder {
    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

    let resource = format!("{}:{}:naming/*", namespace_id, group_name);
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let service_name_filter = params.service_name.as_deref().unwrap_or("");
    let has_ip_count = params.has_ip_count.unwrap_or(false);

    match data
        .console_datasource
        .service_list(
            namespace_id,
            group_name,
            service_name_filter,
            params.page_no,
            params.page_size,
            has_ip_count,
        )
        .await
    {
        Ok((count, service_list)) => {
            let response = serde_json::json!({
                "count": count,
                "serviceList": service_list,
            });
            Result::<serde_json::Value>::http_success(response)
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// GET /ns/service
#[get("")]
async fn get_service(
    req: HttpRequest,
    data: web::Data<AppState>,
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
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match data
        .console_datasource
        .service_get(namespace_id, group_name, &params.service_name)
        .await
    {
        Ok(Some(service)) => Result::<serde_json::Value>::http_success(service),
        Ok(None) => Result::<Option<serde_json::Value>>::http_response(
            404,
            20004,
            format!("service {} not found", params.service_name),
            None::<serde_json::Value>,
        ),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// POST /ns/service
#[post("")]
async fn create_service(
    req: HttpRequest,
    data: web::Data<AppState>,
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
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let threshold = form.protect_threshold.unwrap_or(0.0);
    let metadata_str = form.metadata.as_deref().unwrap_or("");
    let selector_str = form.selector.as_deref().unwrap_or("");

    match data
        .console_datasource
        .service_create(
            namespace_id,
            group_name,
            &form.service_name,
            threshold,
            metadata_str,
            selector_str,
        )
        .await
    {
        Ok(result) => Result::<bool>::http_success(result),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// PUT /ns/service
#[put("")]
async fn update_service(
    req: HttpRequest,
    data: web::Data<AppState>,
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
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match data
        .console_datasource
        .service_update(
            namespace_id,
            group_name,
            &form.service_name,
            form.protect_threshold,
            form.metadata.as_deref(),
            form.selector.as_deref(),
        )
        .await
    {
        Ok(result) => Result::<bool>::http_success(result),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// DELETE /ns/service
#[delete("")]
async fn delete_service(
    req: HttpRequest,
    data: web::Data<AppState>,
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
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match data
        .console_datasource
        .service_delete(namespace_id, group_name, &params.service_name)
        .await
    {
        Ok(result) => Result::<bool>::http_success(result),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// GET /ns/subscriber/list
#[get("list")]
async fn list_subscribers(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SubscriberQuery>,
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
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match data
        .console_datasource
        .service_subscriber_list(
            namespace_id,
            group_name,
            &params.service_name,
            params.page_no,
            params.page_size,
        )
        .await
    {
        Ok((count, subscribers)) => {
            let subscriber_list: Vec<serde_json::Value> = subscribers
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "addrStr": s,
                        "agent": s,
                    })
                })
                .collect();

            let response = serde_json::json!({
                "count": count,
                "subscribers": subscriber_list,
            });
            Result::<serde_json::Value>::http_success(response)
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// GET /ns/service/selector/types
#[get("selector/types")]
async fn get_selector_types() -> impl Responder {
    let types = vec!["none".to_string(), "label".to_string()];
    Result::<Vec<String>>::http_success(types)
}

/// PUT /ns/service/cluster
#[put("cluster")]
async fn update_cluster(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Json<UpdateClusterForm>,
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
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let health_checker_type = form.health_checker.as_ref().map(|c| c.r#type.as_str());

    match data
        .console_datasource
        .service_update_cluster(
            namespace_id,
            group_name,
            &form.service_name,
            &form.cluster_name,
            health_checker_type,
            form.metadata.clone(),
        )
        .await
    {
        Ok(result) => Result::<bool>::http_success(result),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub fn routes() -> Scope {
    web::scope("/ns")
        .service(
            web::scope("/service")
                .service(create_service)
                .service(delete_service)
                .service(update_service)
                .service(get_service)
                .service(list_services)
                .service(get_selector_types)
                .service(update_cluster),
        )
        .service(web::scope("/subscriber").service(list_subscribers))
}
