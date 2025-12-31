// Console cluster management API endpoints
// This module provides web console endpoints for cluster node management and monitoring

use actix_web::{HttpMessage, HttpRequest, Responder, Scope, get, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::model::Member,
    core::service::cluster::ClusterHealthSummary,
    model::{self, common::AppState},
    secured,
};

// Parameters for cluster node query
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetNodesParam {
    pub keyword: Option<String>,
    pub with_health: Option<bool>,
}

// Response for cluster health status
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterHealthResponse {
    pub is_healthy: bool,
    pub summary: ClusterHealthSummaryResponse,
    pub standalone: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterHealthSummaryResponse {
    pub total: usize,
    pub up: usize,
    pub down: usize,
    pub suspicious: usize,
    pub starting: usize,
    pub isolation: usize,
}

impl From<ClusterHealthSummary> for ClusterHealthSummaryResponse {
    fn from(summary: ClusterHealthSummary) -> Self {
        Self {
            total: summary.total,
            up: summary.up,
            down: summary.down,
            suspicious: summary.suspicious,
            starting: summary.starting,
            isolation: summary.isolation,
        }
    }
}

// Response for self member info
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfMemberResponse {
    pub ip: String,
    pub port: u16,
    pub address: String,
    pub state: String,
    pub is_standalone: bool,
    pub version: String,
}

// Parameters for member state update (reserved for future use)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct UpdateMemberStateParam {
    pub address: String,
    pub state: String,
}

/// Get all cluster nodes
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
        members.retain(|e| e.address.contains(keyword));
    }

    model::common::Result::<Vec<Member>>::http_success(members)
}

/// Get healthy cluster nodes only
#[get("nodes/healthy")]
async fn get_healthy_nodes(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "/v3/core/cluster")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let members = data.server_member_manager.healthy_members();
    model::common::Result::<Vec<Member>>::http_success(members)
}

/// Get cluster health status
#[get("health")]
async fn get_health(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "/v3/core/cluster")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let summary = data.server_member_manager.health_summary();
    let healthy = data.server_member_manager.is_cluster_healthy();
    let standalone_mode = data.server_member_manager.is_standalone();

    let response = ClusterHealthResponse {
        is_healthy: healthy,
        summary: summary.into(),
        standalone: standalone_mode,
    };

    model::common::Result::<ClusterHealthResponse>::http_success(response)
}

/// Get self (local) member information
#[get("self")]
async fn get_self(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "/v3/core/cluster")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let self_member = data.server_member_manager.get_self();
    let version = self_member
        .extend_info
        .read()
        .unwrap()
        .get(Member::VERSION)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let response = SelfMemberResponse {
        ip: self_member.ip.clone(),
        port: self_member.port,
        address: self_member.address.clone(),
        state: self_member.state.to_string(),
        is_standalone: data.server_member_manager.is_standalone(),
        version,
    };

    model::common::Result::<SelfMemberResponse>::http_success(response)
}

/// Get a specific member by address
#[get("node/{address}")]
async fn get_node(
    req: HttpRequest,
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "/v3/core/cluster")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let address = path.into_inner();

    match data.server_member_manager.get_member(&address) {
        Some(member) => model::common::Result::<Member>::http_success(member),
        None => model::common::Result::<String>::http_response(
            404,
            404,
            format!("Member not found: {}", address),
            String::new(),
        ),
    }
}

/// Get member count
#[get("count")]
async fn get_member_count(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "/v3/core/cluster")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let count = data.server_member_manager.member_count();
    model::common::Result::<usize>::http_success(count)
}

/// Check if running in standalone mode
#[get("standalone")]
async fn check_standalone(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "/v3/core/cluster")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let standalone_mode = data.server_member_manager.is_standalone();
    model::common::Result::<bool>::http_success(standalone_mode)
}

/// Trigger refresh of local member
#[post("self/refresh")]
async fn refresh_self(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "/v3/core/cluster")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    data.server_member_manager.refresh_self();
    model::common::Result::<bool>::http_success(true)
}

pub fn routes() -> Scope {
    web::scope("/core/cluster")
        .service(get_nodes)
        .service(get_healthy_nodes)
        .service(get_health)
        .service(get_self)
        .service(get_node)
        .service(get_member_count)
        .service(check_standalone)
        .service(refresh_self)
}
