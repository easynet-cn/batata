//! V3 Admin config listener endpoints
//!
//! Implements the Nacos V3 Admin config listener API:
//! - GET /nacos/v3/admin/cs/listener - Get listener state by config
//! - GET /nacos/v3/admin/cs/listener/ip - Get listener state by client IP

use std::collections::HashMap;

use actix_web::{HttpRequest, Responder, get, web};
use serde::Deserialize;

use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType, model, model::AppState, secured,
};

use crate::api::config_model::ConfigListenerInfo;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ListenerQuery {
    #[serde(default)]
    data_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    #[serde(default)]
    namespace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListenerByIpQuery {
    pub ip: String,
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub namespace_id: Option<String>,
}

/// GET /v3/admin/cs/listener
#[get("")]
async fn get_listener_state(
    req: HttpRequest,
    data: web::Data<AppState>,
    _params: web::Query<ListenerQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    model::common::Result::<Option<ConfigListenerInfo>>::http_success(ConfigListenerInfo {
        query_type: ConfigListenerInfo::QUERY_TYPE_CONFIG.to_string(),
        listeners_status: HashMap::new(),
    })
}

/// GET /v3/admin/cs/listener/ip
///
/// Query config listener state by client IP address.
/// This matches the Nacos V3 Admin ListenerController behavior.
#[get("/ip")]
async fn get_listener_by_ip(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ListenerByIpQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let namespace_id = params.namespace_id.as_deref().unwrap_or("");

    match data
        .console_datasource
        .config_listener_list_by_ip(&params.ip, params.all, namespace_id)
        .await
    {
        Ok(listener_info) => {
            model::common::Result::<ConfigListenerInfo>::http_success(listener_info)
        }
        Err(e) => {
            tracing::error!("Failed to get listener by IP {}: {}", params.ip, e);
            model::common::Result::<ConfigListenerInfo>::http_response(
                500,
                500,
                format!("Failed to get listener info: {}", e),
                ConfigListenerInfo::default(),
            )
        }
    }
}

pub fn routes() -> actix_web::Scope {
    web::scope("/listener")
        .service(get_listener_state)
        .service(get_listener_by_ip)
}
