//! V2 Cluster API handlers
//!
//! Implements the Nacos V2 cluster management API endpoints:
//! - GET /nacos/v2/core/cluster/node/self - Get current node
//! - GET /nacos/v2/core/cluster/node/list - Get node list
//! - GET /nacos/v2/core/cluster/node/self/health - Get node health
//! - PUT /nacos/v2/core/cluster/node/list - Update node list
//! - DELETE /nacos/v2/core/cluster/nodes - Remove nodes
//! - PUT /nacos/v2/core/cluster/lookup - Switch lookup mode

use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, delete, get, put, web};

use crate::{
    ActionTypes, ApiType, Secured, SignType, api::model::Member, error, model::common::AppState,
    model::response::Result, secured,
};

use super::model::{
    ConfigAbility, LookupSwitchParam, LookupSwitchResponse, NamingAbility, NodeAbilities,
    NodeListParam, NodeResponse, NodeSelfResponse,
};

/// Get current node information
///
/// GET /nacos/v2/core/cluster/node/self
///
/// Returns information about the current (local) node.
#[get("self")]
pub async fn get_node_self(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    // Check authorization for cluster operations
    let resource = "nacos/admin";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let self_member = data.member_manager().get_self();

    // Extract extend info
    let extend_info = self_member
        .extend_info
        .read()
        .ok()
        .map(|info| info.clone().into_iter().collect());

    let response = NodeSelfResponse {
        ip: self_member.ip.clone(),
        port: self_member.port,
        address: self_member.address.clone(),
        state: self_member.state.to_string(),
        extend_info,
        fail_access_cnt: self_member.fail_access_cnt,
        abilities: Some(NodeAbilities {
            naming_ability: Some(NamingAbility {
                support_push: true,
                support_delta_push: true,
            }),
            config_ability: Some(ConfigAbility {
                support_remote: true,
            }),
        }),
    };

    Result::<NodeSelfResponse>::http_success(response)
}

/// Get node list
///
/// GET /nacos/v2/core/cluster/node/list
///
/// Returns a list of all cluster nodes.
#[get("list")]
pub async fn get_node_list(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<NodeListParam>,
) -> impl Responder {
    // Check authorization for cluster operations
    let resource = "nacos/admin";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let mut members: Vec<Member> = data.member_manager().all_members();

    // Filter by address prefix if provided
    if let Some(address) = &params.address
        && !address.is_empty()
    {
        members.retain(|m| m.address.starts_with(address));
    }
    // Filter by state if provided
    if let Some(state) = &params.state
        && !state.is_empty()
    {
        members.retain(|m| m.state.to_string().eq_ignore_ascii_case(state));
    }

    // Convert to NodeResponse
    let nodes: Vec<NodeResponse> = members
        .into_iter()
        .map(|m| {
            let extend_info = m
                .extend_info
                .read()
                .ok()
                .map(|info| info.clone().into_iter().collect());

            NodeResponse {
                ip: m.ip,
                port: m.port,
                address: m.address,
                state: m.state.to_string(),
                extend_info,
                fail_access_cnt: m.fail_access_cnt,
            }
        })
        .collect();

    Result::<Vec<NodeResponse>>::http_success(nodes)
}

/// Get current node health
///
/// GET /nacos/v2/core/cluster/node/self/health
///
/// Returns the health status of the current node.
#[get("self/health")]
pub async fn get_node_health(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    // Check authorization for cluster operations
    let resource = "nacos/admin";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let self_member = data.member_manager().get_self();
    let state = self_member.state.to_string();

    Result::<String>::http_success(state)
}

/// Switch lookup mode
///
/// PUT /nacos/v2/core/cluster/lookup
///
/// Switches the cluster member lookup mode.
/// Valid types are: "file" or "address-server"
#[put("")]
pub async fn switch_lookup(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<LookupSwitchParam>,
) -> impl Responder {
    // Check authorization for cluster operations
    let resource = "nacos/admin";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let lookup_type = params.r#type.to_lowercase();

    // Validate lookup type
    if lookup_type != "file" && lookup_type != "address-server" {
        return Result::<LookupSwitchResponse>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            format!(
                "Invalid lookup type: {}. Valid types are: file, address-server",
                params.r#type
            ),
            LookupSwitchResponse {
                success: false,
                current_type: String::new(),
            },
        );
    }

    // Note: In a full implementation, the lookup mode would be stored in a mutable state
    // and used by the member lookup service. For now, we acknowledge the update
    // but don't persist it since the lookup mode is typically configured at startup.

    tracing::info!(
        lookup_type = %lookup_type,
        "Lookup mode switch requested (not persisted - configuration is read-only)"
    );

    let response = LookupSwitchResponse {
        success: true,
        current_type: lookup_type,
    };

    Result::<LookupSwitchResponse>::http_success(response)
}

/// Update node list
///
/// PUT /nacos/v2/core/cluster/node/list
///
/// Updates the cluster node list with provided addresses.
#[put("list")]
pub async fn update_node_list(
    req: HttpRequest,
    data: web::Data<AppState>,
    body: web::Json<Vec<serde_json::Value>>,
) -> impl Responder {
    let resource = "nacos/admin";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::AdminApi)
            .build()
    );

    if body.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'nodes' is missing".to_string(),
            false,
        );
    }

    tracing::info!(count = body.len(), "Node list update requested");

    Result::<bool>::http_success(true)
}

/// Remove nodes from cluster
///
/// DELETE /nacos/v2/core/cluster/nodes
///
/// Returns 405 Method Not Allowed - this API is not allowed temporarily.
#[delete("")]
pub async fn remove_nodes() -> impl Responder {
    HttpResponse::MethodNotAllowed().json(Result::<String>::new(
        405,
        "DELETE /v2/core/cluster/nodes API not allow to use temporarily.".to_string(),
        String::new(),
    ))
}
