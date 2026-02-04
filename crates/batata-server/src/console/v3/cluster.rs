// Console cluster management API endpoints
// This module provides web console endpoints for cluster node management and monitoring

use actix_web::{HttpMessage, HttpRequest, Responder, Scope, get, post, put, web};
use batata_api::model::NodeState;
use serde::{Deserialize, Serialize};

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::model::Member,
    model::{self, common::AppState},
    secured,
};

// Re-export cluster response types from batata_console
pub use batata_console::model::{
    ClusterHealthResponse, ClusterHealthSummary as ClusterHealthSummaryResponse, SelfMemberResponse,
};

// Parameters for cluster node query
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetNodesParam {
    pub keyword: Option<String>,
    pub with_health: Option<bool>,
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

    let mut members = data.member_manager().all_members();

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

    let members = data.member_manager().healthy_members();
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

    let summary = data.member_manager().health_summary();
    let healthy = data.member_manager().is_cluster_healthy();
    let standalone_mode = data.member_manager().is_standalone();

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

    let self_member = data.member_manager().get_self();
    let version = self_member
        .extend_info
        .read()
        .ok()
        .and_then(|info| {
            info.get(Member::VERSION)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    let response = SelfMemberResponse {
        ip: self_member.ip.clone(),
        port: self_member.port,
        address: self_member.address.clone(),
        state: self_member.state.to_string(),
        is_standalone: data.member_manager().is_standalone(),
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

    match data.member_manager().get_member(&address) {
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

    let count = data.member_manager().member_count();
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

    let standalone_mode = data.member_manager().is_standalone();
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

    data.member_manager().refresh_self();
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

    // Parse the state string to NodeState
    let new_state = match body.state.to_uppercase().as_str() {
        "UP" => NodeState::Up,
        "DOWN" => NodeState::Down,
        "SUSPICIOUS" => NodeState::Suspicious,
        "STARTING" => NodeState::Starting,
        "ISOLATION" => NodeState::Isolation,
        _ => {
            return model::common::Result::<String>::http_response(
                400,
                400,
                format!(
                    "Invalid state: {}. Valid states are: UP, DOWN, SUSPICIOUS, STARTING, ISOLATION",
                    body.state
                ),
                String::new(),
            );
        }
    };

    // Get the current member state before update
    let previous_state = match data.member_manager().get_member(&address) {
        Some(member) => member.state.to_string(),
        None => {
            return model::common::Result::<String>::http_response(
                404,
                404,
                format!("Member not found: {}", address),
                String::new(),
            );
        }
    };

    // Update the member state
    data.member_manager()
        .update_member_state(&address, new_state)
        .await;

    let response = UpdateMemberStateResponse {
        success: true,
        previous_state,
        new_state: body.state.to_uppercase(),
        address,
    };

    model::common::Result::<UpdateMemberStateResponse>::http_success(response)
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
        is_leader: data.member_manager().is_leader(),
        leader_address: data.member_manager().leader_address(),
        local_address: data.member_manager().local_address().to_string(),
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
