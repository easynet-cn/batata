//! Instance console endpoints
//!
//! Provides HTTP handlers for instance management operations.

use std::sync::Arc;

use actix_web::{Responder, Scope, get, put, web};
use serde::Deserialize;

use batata_api::Page;

use crate::datasource::{ConsoleDataSource, InstanceInfo};

use super::namespace::ApiResult;

pub const DEFAULT_NAMESPACE_ID: &str = "public";
pub const DEFAULT_GROUP_NAME: &str = "DEFAULT_GROUP";
pub const DEFAULT_CLUSTER_NAME: &str = "DEFAULT";

/// Instance list query parameters
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceListQuery {
    #[serde(default)]
    pub namespace_id: String,
    #[serde(default = "default_group")]
    pub group_name: String,
    pub service_name: String,
    #[serde(default)]
    pub cluster_name: String,
    #[serde(default = "default_page_no")]
    pub page_no: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_group() -> String {
    DEFAULT_GROUP_NAME.to_string()
}

fn default_page_no() -> u32 {
    1
}

fn default_page_size() -> u32 {
    10
}

/// Instance update form
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceForm {
    #[serde(default)]
    pub namespace_id: String,
    #[serde(default = "default_group")]
    pub group_name: String,
    pub service_name: String,
    pub ip: String,
    pub port: i32,
    #[serde(default = "default_cluster")]
    pub cluster_name: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
    #[serde(default = "default_healthy")]
    pub healthy: bool,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_ephemeral")]
    pub ephemeral: bool,
    #[serde(default)]
    pub metadata: String,
}

fn default_cluster() -> String {
    DEFAULT_CLUSTER_NAME.to_string()
}

fn default_weight() -> f64 {
    1.0
}

fn default_healthy() -> bool {
    true
}

fn default_enabled() -> bool {
    true
}

fn default_ephemeral() -> bool {
    true
}

/// List instances of a service
#[get("list")]
pub async fn list_instances(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<InstanceListQuery>,
) -> impl Responder {
    let namespace_id = if params.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        &params.namespace_id
    };

    match datasource
        .instance_list(
            namespace_id,
            &params.group_name,
            &params.service_name,
            &params.cluster_name,
            params.page_no,
            params.page_size,
        )
        .await
    {
        Ok(page) => ApiResult::<Page<InstanceInfo>>::http_success(page),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

/// Update an instance
#[put("")]
pub async fn update_instance(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    form: web::Json<InstanceForm>,
) -> impl Responder {
    let namespace_id = if form.namespace_id.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        &form.namespace_id
    };

    // Validate weight
    let weight = if form.weight <= 0.0 { 1.0 } else { form.weight };

    match datasource
        .instance_update(
            namespace_id,
            &form.group_name,
            &form.service_name,
            &form.cluster_name,
            &form.ip,
            form.port,
            weight,
            form.healthy,
            form.enabled,
            form.ephemeral,
            &form.metadata,
        )
        .await
    {
        Ok(result) => ApiResult::<bool>::http_success(result),
        Err(e) => ApiResult::http_internal_error(e),
    }
}

pub fn routes() -> Scope {
    web::scope("/ns/instance")
        .service(list_instances)
        .service(update_instance)
}
