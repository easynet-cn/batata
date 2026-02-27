//! V3 Admin cluster management endpoints

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, get, post, put, web};
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured, service::naming::NamingService,
};

use batata_api::naming::model::CreateClusterForm;
use batata_naming::ClusterStatistics;

const DEFAULT_NAMESPACE_ID: &str = "public";
const DEFAULT_GROUP: &str = "DEFAULT_GROUP";

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

/// POST /v3/admin/ns/cluster
#[post("")]
async fn create_cluster(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    form: web::Json<CreateClusterForm>,
) -> impl Responder {
    let namespace_id = form
        .namespace_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_NAMESPACE_ID);
    let group_name = form
        .group_name
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_GROUP);

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

    // Parse health checker config
    let (health_check_type, check_port, use_instance_port) =
        if let Some(checker) = &form.health_checker {
            (
                checker.r#type.clone(),
                checker.check_port.unwrap_or(0),
                checker.use_instance_port.unwrap_or(true),
            )
        } else {
            ("TCP".to_string(), 0, true)
        };

    let metadata = form.metadata.clone().unwrap_or_default();

    match naming_service.create_cluster_config(
        namespace_id,
        group_name,
        &form.service_name,
        &form.cluster_name,
        &health_check_type,
        check_port,
        use_instance_port,
        metadata,
    ) {
        Ok(_) => Result::<bool>::http_success(true),
        Err(e) => actix_web::HttpResponse::BadRequest().json(Result::<()>::fail(e)),
    }
}

/// GET /v3/admin/ns/cluster/statistics
#[get("statistics")]
async fn get_cluster_statistics(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    let (namespace_id, group_name, service_name) = path.into_inner();

    let resource = format!("{}:{}:naming/{}", namespace_id, group_name, service_name);
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let stats = naming_service.get_cluster_statistics(&namespace_id, &group_name, &service_name);
    Result::<Vec<ClusterStatistics>>::http_success(stats)
}

/// PUT /v3/admin/ns/cluster
#[put("")]
async fn update_cluster(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
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
            .api_type(ApiType::AdminApi)
            .build()
    );

    if let Some(checker) = &form.health_checker {
        naming_service.update_cluster_health_check(
            namespace_id,
            group_name,
            &form.service_name,
            &form.cluster_name,
            &checker.r#type,
            80,
            true,
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
    web::scope("/cluster")
        .service(create_cluster)
        .service(get_cluster_statistics)
        .service(update_cluster)
}
