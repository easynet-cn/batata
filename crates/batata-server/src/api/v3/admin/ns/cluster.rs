//! V3 Admin cluster management endpoints

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, put, web};
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured, service::naming::NamingService,
};

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
    web::scope("/cluster").service(update_cluster)
}
