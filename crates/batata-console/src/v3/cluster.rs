// Console cluster management API endpoints
// This module provides web console endpoints for cluster node management and monitoring

use actix_web::{HttpMessage, HttpRequest, Responder, Scope, get, post, put, web};
use serde::{Deserialize, Serialize};

use crate::model::Member as ConsoleMember;

use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType, error,
    model::{self, AppState},
    secured,
};

// Re-export cluster response types from crate::model (batata-console)
pub use crate::model::{
    ClusterHealthResponse, ClusterHealthSummary as ClusterHealthSummaryResponse, SelfMemberResponse,
};

// Parameters for cluster node query
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetNodesParam {
    pub keyword: Option<String>,
    pub with_instances: Option<bool>,
    pub page_no: Option<u64>,
    pub page_size: Option<u64>,
    pub namespace_id: Option<String>,
}

/// Parameters for member state update
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemberStateParam {
    /// New state for the member (UP, DOWN, SUSPICIOUS, STARTING, ISOLATION)
    pub state: String,
}

/// Response for member state update
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemberStateResponse {
    /// Whether the update was successful
    pub success: bool,
    /// Previous state
    pub previous_state: String,
    /// New state
    pub new_state: String,
    /// Member address
    pub address: String,
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

    let mut members: Vec<ConsoleMember> = data.console_datasource.cluster_all_members();

    if let Some(keyword) = &params.keyword
        && !keyword.is_empty()
    {
        members.retain(|e| e.address.contains(keyword));
    }

    // Apply pagination
    let page_no = params.page_no.unwrap_or(1).max(1) as usize;
    let page_size = params.page_size.unwrap_or(10).max(1) as usize;
    let start = (page_no - 1) * page_size;
    let paginated: Vec<ConsoleMember> = members.into_iter().skip(start).take(page_size).collect();

    model::common::Result::<Vec<ConsoleMember>>::http_success(paginated)
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

    let members: Vec<ConsoleMember> = data.console_datasource.cluster_healthy_members();
    model::common::Result::<Vec<ConsoleMember>>::http_success(members)
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

    let response = data.console_datasource.cluster_get_health();

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

    let response = data.console_datasource.cluster_get_self();

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

    match data.console_datasource.cluster_get_member(&address) {
        Some(member) => model::common::Result::<ConsoleMember>::http_success(member),
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

    let count = data.console_datasource.cluster_member_count();
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

    let standalone_mode = data.console_datasource.cluster_is_standalone();
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

    data.console_datasource.cluster_refresh_self();
    model::common::Result::<bool>::http_success(true)
}

/// Update member state
///
/// Updates the state of a specific cluster member.
/// Valid states are: UP, DOWN, SUSPICIOUS, STARTING, ISOLATION
#[put("node/{address}/state")]
async fn update_member_state(
    req: HttpRequest,
    data: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateMemberStateParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "/v3/core/cluster")
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let address = path.into_inner();
    let state = body.state.to_uppercase();

    match data
        .console_datasource
        .cluster_update_member_state(&address, &state)
        .await
    {
        Ok(previous_state) => {
            let response = UpdateMemberStateResponse {
                success: true,
                previous_state,
                new_state: state,
                address,
            };
            model::common::Result::<UpdateMemberStateResponse>::http_success(response)
        }
        Err(e) => model::common::Result::<String>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// Get leader information
///
/// Returns the current leader node address and whether this node is the leader.
#[get("leader")]
async fn get_leader(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "/v3/core/cluster")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct LeaderResponse {
        is_leader: bool,
        leader_address: Option<String>,
        local_address: String,
    }

    let response = LeaderResponse {
        is_leader: data.console_datasource.cluster_is_leader(),
        leader_address: data.console_datasource.cluster_leader_address(),
        local_address: data.console_datasource.cluster_local_address(),
    };

    model::common::Result::<LeaderResponse>::http_success(response)
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
        .service(update_member_state)
        .service(get_leader)
}
