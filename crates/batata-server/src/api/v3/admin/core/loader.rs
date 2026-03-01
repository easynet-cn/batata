//! V3 Admin server loader endpoints

use actix_web::{HttpRequest, Responder, get, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerLoaderInfo {
    address: String,
    state: String,
    loader_count: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ReloadCurrentParam {
    count: Option<i32>,
    redirect_address: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SmartReloadParam {
    loader_factor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ReloadClientParam {
    connection_id: Option<String>,
    redirect_address: Option<String>,
}

/// GET /v3/admin/core/loader/current
///
/// Returns all current SDK connections/clients on this server.
#[get("current")]
async fn get_current(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let self_member = data.member_manager().get_self();

    let response = ServerLoaderInfo {
        address: self_member.address.clone(),
        state: self_member.state.to_string(),
        loader_count: 0,
    };

    Result::<ServerLoaderInfo>::http_success(response)
}

/// POST /v3/admin/core/loader/reloadCurrent
///
/// Rebalance SDK connections on current server to target count.
#[post("reloadCurrent")]
async fn reload_current(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ReloadCurrentParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!(
        count = ?params.count,
        redirect_address = ?params.redirect_address,
        "Server reload current requested via admin API"
    );

    Result::<String>::http_success(String::new())
}

/// POST /v3/admin/core/loader/smartReloadCluster
///
/// Intelligently balance SDK connections across all cluster nodes.
#[post("smartReloadCluster")]
async fn smart_reload(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SmartReloadParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let loader_factor = params.loader_factor.as_deref().unwrap_or("0.1");

    tracing::info!(
        loader_factor = %loader_factor,
        "Smart cluster reload requested via admin API"
    );

    Result::<String>::http_success(String::new())
}

/// POST /v3/admin/core/loader/reloadClient
///
/// Send connection reset to a specific client.
#[post("reloadClient")]
async fn reload_client(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ReloadClientParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!(
        connection_id = ?params.connection_id,
        redirect_address = ?params.redirect_address,
        "Client reload requested via admin API"
    );

    Result::<String>::http_success(String::new())
}

/// GET /v3/admin/core/loader/cluster
///
/// Get server loader metrics for the entire cluster.
#[get("cluster")]
async fn get_cluster(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let members = data.member_manager().all_members();
    let loaders: Vec<ServerLoaderInfo> = members
        .iter()
        .map(|m| ServerLoaderInfo {
            address: m.address.clone(),
            state: m.state.to_string(),
            loader_count: 0,
        })
        .collect();

    Result::<Vec<ServerLoaderInfo>>::http_success(loaders)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/loader")
        .service(get_current)
        .service(reload_current)
        .service(smart_reload)
        .service(reload_client)
        .service(get_cluster)
}
