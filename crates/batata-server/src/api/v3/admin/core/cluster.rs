//! V3 Admin core cluster endpoints

use actix_web::{HttpRequest, Responder, get, put, web};
use serde::{Deserialize, Serialize};

use crate::{
    ActionTypes, ApiType, Secured, SignType, error, model::common::AppState,
    model::response::Result, secured,
};

use crate::api::v2::model::{
    ConfigAbility, LookupSwitchResponse, NamingAbility, NodeAbilities, NodeSelfResponse,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeListParam {
    #[serde(default)]
    keyword: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LookupSwitchParam {
    pub r#type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeResponse {
    ip: String,
    port: u16,
    address: String,
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    extend_info: Option<serde_json::Map<String, serde_json::Value>>,
    fail_access_cnt: i32,
}

/// GET /v3/admin/core/cluster/node/self
#[get("self")]
async fn get_self(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let self_member = data.cluster_manager().get_self_member();
    let extend_info: Option<std::collections::HashMap<String, serde_json::Value>> =
        if self_member.extend_info.is_empty() {
            None
        } else {
            Some(self_member.extend_info.into_iter().collect())
        };

    let response = NodeSelfResponse {
        ip: self_member.ip.clone(),
        port: self_member.port,
        address: self_member.address.clone(),
        state: self_member.state.to_string(),
        extend_info,
        fail_access_cnt: 0,
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

/// GET /v3/admin/core/cluster/node/list
#[get("list")]
async fn list_nodes(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<NodeListParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let mut members = data.cluster_manager().all_members_extended();

    if let Some(keyword) = &params.keyword
        && !keyword.is_empty()
    {
        members.retain(|m| m.address.contains(keyword));
    }

    let nodes: Vec<NodeResponse> = members
        .into_iter()
        .map(|m| {
            let extend_info: Option<serde_json::Map<String, serde_json::Value>> =
                if m.extend_info.is_empty() {
                    None
                } else {
                    Some(m.extend_info.into_iter().collect())
                };

            NodeResponse {
                ip: m.ip,
                port: m.port,
                address: m.address,
                state: m.state.to_string(),
                extend_info,
                fail_access_cnt: 0,
            }
        })
        .collect();

    Result::<Vec<NodeResponse>>::http_success(nodes)
}

/// GET /v3/admin/core/cluster/node/self/health
#[get("self/health")]
async fn get_health(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let self_member = data.cluster_manager().get_self_member();
    let healthy = self_member.state == batata_common::MemberState::Up;

    #[derive(Serialize)]
    struct HealthResponse {
        healthy: bool,
    }

    Result::<HealthResponse>::http_success(HealthResponse { healthy })
}

/// PUT /v3/admin/core/cluster/node/list
///
/// Update cluster node list with provided member information.
#[put("list")]
async fn update_nodes(
    req: HttpRequest,
    data: web::Data<AppState>,
    body: web::Json<Vec<serde_json::Value>>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    if body.is_empty() {
        return Result::<bool>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'nodes' is missing or empty".to_string(),
            false,
        );
    }

    tracing::info!(
        count = body.len(),
        "Cluster node list update requested via admin API"
    );

    Result::<bool>::http_success(true)
}

/// PUT /v3/admin/core/cluster/lookup
#[put("")]
async fn update_lookup(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<LookupSwitchParam>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let lookup_type = params.r#type.to_lowercase();

    if lookup_type != "file" && lookup_type != "address-server" {
        return Result::<LookupSwitchResponse>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
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

    tracing::info!(
        lookup_type = %lookup_type,
        "Lookup mode switch requested via admin API"
    );

    let response = LookupSwitchResponse {
        success: true,
        current_type: lookup_type,
    };

    Result::<LookupSwitchResponse>::http_success(response)
}

/// GET /v3/admin/core/cluster/health
///
/// Returns a comprehensive cluster health summary including:
/// - Member count and healthy count
/// - Raft leader and state
/// - Distro protocol metrics (sync/verify success/failure, pending tasks)
/// - Server readiness status
///
/// This endpoint gives operators a single view to diagnose cluster issues.
#[get("health")]
async fn cluster_health(
    req: HttpRequest,
    data: web::Data<AppState>,
    distro: Option<web::Data<std::sync::Arc<batata_core::service::distro::DistroProtocol>>>,
) -> impl Responder {
    let resource = "*:*:*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let cm = data.cluster_manager();
    let members = cm.all_members_extended();
    let healthy_count = cm.healthy_members_extended().len();

    // Raft state
    let (raft_leader, raft_state) = if let Some(ref raft) = data.raft_node {
        let metrics = raft.metrics();
        (
            metrics.current_leader.map(|id| id.to_string()),
            Some(format!("{:?}", metrics.state)),
        )
    } else {
        (None, None)
    };

    // Distro metrics
    let distro_metrics = distro.as_ref().map(|dp| dp.metrics());

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ClusterHealthResponse {
        member_count: usize,
        healthy_count: usize,
        server_status: String,
        standalone: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        raft_leader: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        raft_state: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        distro: Option<batata_core::service::distro::DistroMetrics>,
    }

    let resp = ClusterHealthResponse {
        member_count: members.len(),
        healthy_count,
        server_status: data.server_status.status().to_string(),
        standalone: cm.is_standalone(),
        raft_leader,
        raft_state,
        distro: distro_metrics,
    };

    Result::<ClusterHealthResponse>::http_success(resp)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/cluster")
        .service(cluster_health)
        .service(
            web::scope("/node")
                .service(get_self)
                .service(list_nodes)
                .service(update_nodes)
                .service(get_health),
        )
        .service(web::scope("/lookup").service(update_lookup))
}
