//! V3 Admin config operations endpoints

use actix_web::{HttpRequest, Responder, get, post, put, web};
use serde::Deserialize;

use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType,
    model::{AppState, Result},
    secured,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogLevelParam {
    log_name: String,
    log_level: String,
}

/// POST /v3/admin/cs/ops/localCache
#[post("localCache")]
async fn dump_local_cache(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:config/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!("Local cache dump requested via admin API");

    Result::<bool>::http_success(true)
}

/// PUT /v3/admin/cs/ops/log
#[put("log")]
async fn set_log_level(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<LogLevelParam>,
) -> impl Responder {
    let resource = "*:*:config/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!(
        log_name = %params.log_name,
        log_level = %params.log_level,
        "Config log level change requested via admin API"
    );

    Result::<bool>::http_success(true)
}

/// GET /v3/admin/cs/ops/derby
#[get("derby")]
async fn derby_query(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:config/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    // Derby is not applicable to Batata (it uses MySQL/PostgreSQL)
    Result::<String>::http_response(
        501,
        501,
        "Derby operations are not supported. Batata uses MySQL/PostgreSQL.".to_string(),
        String::new(),
    )
}

/// POST /v3/admin/cs/ops/derby/import
#[post("derby/import")]
async fn derby_import(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:config/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    Result::<String>::http_response(
        501,
        501,
        "Derby operations are not supported. Batata uses MySQL/PostgreSQL.".to_string(),
        String::new(),
    )
}

pub fn routes() -> actix_web::Scope {
    web::scope("/ops")
        .service(dump_local_cache)
        .service(set_log_level)
        .service(derby_query)
        .service(derby_import)
}
