//! V3 Admin server loader endpoints

use actix_web::{HttpMessage, HttpRequest, Responder, get, post, web};
use serde::Serialize;

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

/// GET /v3/admin/core/loader/current
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
#[post("reloadCurrent")]
async fn reload_current(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!("Server reload current requested via admin API");

    Result::<bool>::http_success(true)
}

/// POST /v3/admin/core/loader/smartReloadCluster
#[post("smartReloadCluster")]
async fn smart_reload(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!("Smart cluster reload requested via admin API");

    Result::<bool>::http_success(true)
}

/// GET /v3/admin/core/loader/reloadClient
#[get("reloadClient")]
async fn reload_client(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    Result::<bool>::http_success(true)
}

/// GET /v3/admin/core/loader/cluster
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
