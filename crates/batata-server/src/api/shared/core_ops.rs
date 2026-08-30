//! Shared core operations logic used by both V2 and V3 admin APIs.

use actix_web::{HttpRequest, web};
use serde::{Deserialize, Serialize};

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `RaftOpsParam` struct.
pub struct RaftOpsParam {
    /// `command` field.
    pub command: String,
    #[serde(default, alias = "groupId")]
    /// `group_id` field.
    pub group_id: Option<String>,
    #[serde(default)]
    /// `value` field.
    pub value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `LogUpdateParam` struct.
pub struct LogUpdateParam {
    #[serde(alias = "logName")]
    /// `log_name` field.
    pub log_name: String,
    #[serde(alias = "logLevel")]
    /// `log_level` field.
    pub log_level: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
/// `IdsResponse` struct.
pub struct IdsResponse {
    /// `node_id` field.
    pub node_id: String,
    /// `cluster_id` field.
    pub cluster_id: String,
}

/// `do_raft_ops` function.
///
/// # Arguments
/// - `req`: `req : & HttpRequest . ty`.
/// - `data`: `data : & web :: Data < AppState > . ty`.
/// - `params`: `params : & RaftOpsParam . ty`.
/// - `api_type`: `api_type : ApiType . ty`.
///
/// # Returns
/// `actix_web :: HttpResponse`.
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

/// `do_get_ids` function.
///
/// # Arguments
/// - `req`: `req : & HttpRequest . ty`.
/// - `data`: `data : & web :: Data < AppState > . ty`.
/// - `api_type`: `api_type : ApiType . ty`.
///
/// # Returns
/// `actix_web :: HttpResponse`.
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

/// `do_set_log_level` function.
///
/// # Arguments
/// - `req`: `req : & HttpRequest . ty`.
/// - `data`: `data : & web :: Data < AppState > . ty`.
/// - `params`: `params : & LogUpdateParam . ty`.
/// - `api_type`: `api_type : ApiType . ty`.
///
/// # Returns
/// `actix_web :: HttpResponse`.
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
