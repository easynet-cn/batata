//! Shared core operations logic used by both V2 and V3 admin APIs.

use actix_web::{HttpRequest, web};
use serde::{Deserialize, Serialize};

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaftOpsParam {
    pub command: String,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogUpdateParam {
    pub log_name: String,
    pub log_level: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdsResponse {
    pub node_id: String,
    pub cluster_id: String,
}

pub async fn do_raft_ops(
    req: &HttpRequest,
    data: &web::Data<AppState>,
    params: &RaftOpsParam,
    api_type: ApiType,
) -> actix_web::HttpResponse {
    let resource = "*:*:*";
    secured!(
        Secured::builder(req, data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(api_type)
            .build()
    );

    tracing::info!(
        command = %params.command,
        group_id = ?params.group_id,
        api_type = %api_type,
        "Raft operation requested"
    );

    Result::<String>::http_success(format!("Raft command '{}' acknowledged", params.command))
}

pub async fn do_get_ids(
    req: &HttpRequest,
    data: &web::Data<AppState>,
    api_type: ApiType,
) -> actix_web::HttpResponse {
    let resource = "*:*:*";
    secured!(
        Secured::builder(req, data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(api_type)
            .build()
    );

    let self_member = data.cluster_manager().get_self_member();

    let response = IdsResponse {
        node_id: self_member.address.clone(),
        cluster_id: "batata-cluster".to_string(),
    };

    Result::<IdsResponse>::http_success(response)
}

pub async fn do_set_log_level(
    req: &HttpRequest,
    data: &web::Data<AppState>,
    params: &LogUpdateParam,
    api_type: ApiType,
) -> actix_web::HttpResponse {
    let resource = "*:*:*";
    secured!(
        Secured::builder(req, data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(api_type)
            .build()
    );

    tracing::info!(
        log_name = %params.log_name,
        log_level = %params.log_level,
        api_type = %api_type,
        "Log level change requested"
    );

    Result::<bool>::http_success(true)
}
