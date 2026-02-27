//! V3 Admin health management endpoints

use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, get, put, web};
use serde::{Deserialize, Serialize};

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured, service::naming::NamingService,
};

const DEFAULT_NAMESPACE_ID: &str = "public";
const DEFAULT_GROUP: &str = "DEFAULT_GROUP";
const DEFAULT_CLUSTER: &str = "DEFAULT";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceHealthParam {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    ip: String,
    port: i32,
    #[serde(default)]
    cluster_name: Option<String>,
    healthy: bool,
}

impl InstanceHealthParam {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);

    impl_or_default!(cluster_name_or_default, cluster_name, DEFAULT_CLUSTER);
}

/// PUT /v3/admin/ns/health/instance
#[put("instance")]
async fn update_health(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<InstanceHealthParam>,
) -> impl Responder {
    if params.service_name.is_empty() || params.ip.is_empty() {
        return Result::<bool>::http_response(
            400,
            400,
            "Required parameters missing".to_string(),
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

    let instances = naming_service.get_instances(
        namespace_id,
        group_name,
        &params.service_name,
        cluster_name,
        false,
    );

    let instance_exists = instances
        .iter()
        .any(|i| i.ip == params.ip && i.port == params.port);

    if !instance_exists {
        return Result::<bool>::http_response(
            404,
            404,
            format!(
                "instance {}:{} not found in service {}",
                params.ip, params.port, params.service_name
            ),
            false,
        );
    }

    let success = naming_service.update_instance_health(
        namespace_id,
        group_name,
        &params.service_name,
        &params.ip,
        params.port,
        cluster_name,
        params.healthy,
    );

    if success {
        Result::<bool>::http_success(true)
    } else {
        Result::<bool>::http_response(
            500,
            500,
            "Failed to update instance health status".to_string(),
            false,
        )
    }
}

/// GET /v3/admin/ns/health/checkers
#[get("checkers")]
async fn get_checkers(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    #[derive(Serialize)]
    struct CheckerInfo {
        name: String,
        description: String,
    }

    let checkers = vec![
        CheckerInfo {
            name: "TCP".to_string(),
            description: "TCP health check".to_string(),
        },
        CheckerInfo {
            name: "HTTP".to_string(),
            description: "HTTP health check".to_string(),
        },
        CheckerInfo {
            name: "NONE".to_string(),
            description: "No health check".to_string(),
        },
    ];

    Result::<Vec<CheckerInfo>>::http_success(checkers)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/health")
        .service(update_health)
        .service(get_checkers)
}
