//! Naming service console endpoints
//!
//! Provides HTTP handlers for service discovery operations.

use std::sync::Arc;

use actix_web::{delete, get, post, put, web, Responder, Scope};
use serde::Deserialize;

use batata_api::Page;

use crate::datasource::{
    ConsoleDataSource, ServiceDetail, ServiceListItem, SubscriberInfo,
};

use super::namespace::ApiResult;

pub const DEFAULT_NAMESPACE_ID: &str = "public";
pub const DEFAULT_GROUP_NAME: &str = "DEFAULT_GROUP";

/// Service form for create/update/delete operations
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceForm {
    #[serde(default)]
    pub namespace_id: String,
    #[serde(default = "default_group")]
    pub group_name: String,
    pub service_name: String,
    #[serde(default)]
    pub protect_threshold: f32,
    #[serde(default)]
    pub metadata: String,
    #[serde(default)]
    pub selector: String,
}

fn default_group() -> String {
    DEFAULT_GROUP_NAME.to_string()
}

/// Service list query parameters
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListQuery {
    #[serde(default)]
    pub namespace_id: String,
    #[serde(default)]
    pub group_name: String,
    #[serde(default)]
    pub service_name_param: String,
    #[serde(default = "default_page_no")]
    pub page_no: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    #[serde(default)]
    pub with_instances: bool,
}

fn default_page_no() -> u32 {
    1
}

fn default_page_size() -> u32 {
    10
}

/// Subscriber query parameters
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriberQuery {
    #[serde(default)]
    pub namespace_id: String,
    #[serde(default = "default_group")]
    pub group_name: String,
    pub service_name: String,
    #[serde(default = "default_page_no")]
    pub page_no: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

/// Cluster update form
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateClusterForm {
    #[serde(default)]
    pub namespace_id: String,
    #[serde(default = "default_group")]
    pub group_name: String,
    pub service_name: String,
    pub cluster_name: String,
    #[serde(default = "default_check_port")]
    pub check_port: i32,
    #[serde(default = "default_use_instance_port")]
    pub use_instance_port: bool,
    #[serde(default = "default_health_check_type")]
    pub health_check_type: String,
    #[serde(default)]
    pub metadata: String,
}

fn default_check_port() -> i32 {
    80
}

fn default_use_instance_port() -> bool {
    true
}

fn default_health_check_type() -> String {
    "TCP".to_string()
}

/// Create a new service
#[post("")]
pub async fn create_service(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    form: web::Json<ServiceForm>,
) -> impl Responder {
    let namespace_id = if form.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        &form.namespace_id
    };

    match datasource
        .service_create(
            namespace_id,
            &form.group_name,
            &form.service_name,
            form.protect_threshold,
            &form.metadata,
            &form.selector,
        )
        .await
    {
        Ok(result) => ApiResult::<bool>::http_success(result),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

/// Delete a service
#[delete("")]
pub async fn delete_service(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<ServiceForm>,
) -> impl Responder {
    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        &params.namespace_id
    };

    match datasource
        .service_delete(namespace_id, &params.group_name, &params.service_name)
        .await
    {
        Ok(result) => ApiResult::<bool>::http_success(result),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

/// Update a service
#[put("")]
pub async fn update_service(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    form: web::Json<ServiceForm>,
) -> impl Responder {
    let namespace_id = if form.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        &form.namespace_id
    };

    match datasource
        .service_update(
            namespace_id,
            &form.group_name,
            &form.service_name,
            form.protect_threshold,
            &form.metadata,
            &form.selector,
        )
        .await
    {
        Ok(result) => ApiResult::<bool>::http_success(result),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

/// Get service detail
#[get("")]
pub async fn get_service(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<ServiceForm>,
) -> impl Responder {
    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        &params.namespace_id
    };

    match datasource
        .service_get(namespace_id, &params.group_name, &params.service_name)
        .await
    {
        Ok(Some(detail)) => ApiResult::<ServiceDetail>::http_success(detail),
        Ok(None) => ApiResult::<ServiceDetail>::http_response(
            404,
            20004,
            "Service not found".to_string(),
            ServiceDetail::default(),
        ),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

/// List services with pagination
#[get("list")]
pub async fn list_services(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<ServiceListQuery>,
) -> impl Responder {
    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        &params.namespace_id
    };

    match datasource
        .service_list(
            namespace_id,
            &params.group_name,
            &params.service_name_param,
            params.page_no,
            params.page_size,
            params.with_instances,
        )
        .await
    {
        Ok(page) => ApiResult::<Page<ServiceListItem>>::http_success(page),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

/// Get service subscribers
#[get("subscribers")]
pub async fn get_subscribers(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<SubscriberQuery>,
) -> impl Responder {
    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        &params.namespace_id
    };

    match datasource
        .service_subscribers(
            namespace_id,
            &params.group_name,
            &params.service_name,
            params.page_no,
            params.page_size,
        )
        .await
    {
        Ok(page) => ApiResult::<Page<SubscriberInfo>>::http_success(page),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

/// Get available selector types
#[get("selector/types")]
pub async fn get_selector_types(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
) -> impl Responder {
    let types = datasource.service_selector_types();
    ApiResult::<Vec<String>>::http_success(types)
}

/// Update cluster configuration
#[put("cluster")]
pub async fn update_cluster(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    form: web::Json<UpdateClusterForm>,
) -> impl Responder {
    let namespace_id = if form.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        &form.namespace_id
    };

    match datasource
        .service_cluster_update(
            namespace_id,
            &form.group_name,
            &form.service_name,
            &form.cluster_name,
            form.check_port,
            form.use_instance_port,
            &form.health_check_type,
            &form.metadata,
        )
        .await
    {
        Ok(result) => ApiResult::<bool>::http_success(result),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

pub fn routes() -> Scope {
    web::scope("/ns/service")
        .service(create_service)
        .service(delete_service)
        .service(update_service)
        .service(get_service)
        .service(list_services)
        .service(get_subscribers)
        .service(get_selector_types)
        .service(update_cluster)
}
