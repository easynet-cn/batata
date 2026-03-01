//! V3 Admin config metrics endpoints

use actix_web::{HttpRequest, Responder, get, web};
use serde::Serialize;

use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType,
    model::{AppState, Result},
    secured,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClusterMetrics {
    total_count: u64,
    cluster_node_count: i32,
    is_standalone: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IpMetrics {
    node_address: String,
    total_count: u64,
}

async fn get_config_count(data: &web::Data<AppState>) -> u64 {
    let persistence = data.persistence();
    match persistence
        .config_search_page(1, 1, "", "", "", "", vec![], vec![], "")
        .await
    {
        Ok(page) => page.total_count,
        Err(_) => 0,
    }
}

/// GET /v3/admin/cs/metrics/cluster
#[get("cluster")]
async fn cluster_metrics(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:config/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let count = get_config_count(&data).await;

    let response = ClusterMetrics {
        total_count: count,
        cluster_node_count: data.member_manager().all_members().len() as i32,
        is_standalone: data.configuration.is_standalone(),
    };

    Result::<ClusterMetrics>::http_success(response)
}

/// GET /v3/admin/cs/metrics/ip
#[get("ip")]
async fn ip_metrics(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:config/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let count = get_config_count(&data).await;

    let response = IpMetrics {
        node_address: data.member_manager().local_address().to_string(),
        total_count: count,
    };

    Result::<IpMetrics>::http_success(response)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/metrics")
        .service(cluster_metrics)
        .service(ip_metrics)
}
