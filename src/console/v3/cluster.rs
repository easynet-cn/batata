// Console cluster management API endpoints
// This module provides web console endpoints for cluster node management and monitoring

use actix_web::{HttpMessage, HttpRequest, Responder, Scope, get, web};
use serde::{Deserialize, Serialize};

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::model::Member,
    model::{self, common::AppState},
    secured,
};

// Parameters for cluster node query
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetNodesParam {
    pub keyword: Option<String>,
}

#[get("nodes")]
async fn get_nodes(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<GetNodesParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "/v3/core/cluster")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let mut members = data.server_member_manager.all_members();

    if let Some(keyword) = &params.keyword
        && !keyword.is_empty()
    {
        members = members
            .into_iter()
            .filter(|e| e.address.contains(keyword))
            .collect();
    }

    model::common::Result::<Vec<Member>>::http_success(members)
}

pub fn routes() -> Scope {
    web::scope("/core/cluster").service(get_nodes)
}
