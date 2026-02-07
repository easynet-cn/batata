//! V3 Admin config listener endpoints

use std::collections::HashMap;

use actix_web::{HttpMessage, HttpRequest, Responder, get, web};
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::config::model::ConfigListenerInfo,
    model::{self, common::AppState},
    secured,
};

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

pub fn routes() -> actix_web::Scope {
    web::scope("/listener").service(get_listener_state)
}
