// Console A2A agent management API endpoints
// Aligned with Nacos V3 Console API contract
// Uses config-backed A2aServerOperationService when available, falls back to in-memory AgentRegistry

use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, Scope, delete, get, post, put, web};

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::ai::model::{
        AgentDeleteQuery, AgentDetailQuery, AgentListQuery, AgentListResponse,
        AgentRegistrationRequest, AgentVersionListQuery, BatchAgentRegistrationRequest,
        BatchRegistrationResponse, RegisteredAgent, VersionDetail,
    },
    api::ai::{AgentRegistry, AgentRegistryStats},
    error,
    model::{self, common::AppState},
    secured,
    service::ai::A2aServerOperationService,
};

/// List agents
/// GET /v3/console/ai/a2a/list
#[get("/list")]
async fn list_agents(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<A2aServerOperationService>>>,
    params: web::Query<AgentListQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = params.into_inner();
    if let Some(svc) = a2a_service {
        let namespace = q.namespace_id.as_deref().unwrap_or("public");
        let search_type = q.search.as_deref().unwrap_or("blur");
        let page_no = q.page_no.unwrap_or(1);
        let page_size = q.page_size.unwrap_or(20);
        match svc
            .list_agents(
                namespace,
                q.agent_name.as_deref(),
                search_type,
                page_no,
                page_size,
            )
            .await
        {
            Ok(result) => model::common::Result::<AgentListResponse>::http_success(result),
            Err(e) => model::common::Result::<String>::http_response(
                500,
                500,
                e.to_string(),
                String::new(),
            ),
        }
    } else {
        let result = registry.list_with_search(&q);
        model::common::Result::<AgentListResponse>::http_success(result)
    }
}

/// Get agent by query params
/// GET /v3/console/ai/a2a?namespaceId=xxx&agentName=xxx
#[get("")]
async fn get_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<A2aServerOperationService>>>,
    query: web::Query<AgentDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    if let Some(svc) = a2a_service {
        let namespace = q.namespace_id.as_deref().unwrap_or("public");
        if let Some(ref name) = q.agent_name {
            match svc
                .get_agent_card(namespace, name, q.version.as_deref())
                .await
            {
                Ok(Some(agent)) => model::common::Result::<RegisteredAgent>::http_success(agent),
                Ok(None) => model::common::Result::<String>::http_response(
                    404,
                    404,
                    "Agent not found".to_string(),
                    String::new(),
                ),
                Err(e) => model::common::Result::<String>::http_response(
                    500,
                    500,
                    e.to_string(),
                    String::new(),
                ),
            }
        } else {
            model::common::Result::<String>::http_response(
                400,
                400,
                "agentName is required".to_string(),
                String::new(),
            )
        }
    } else {
        match registry.get_by_query(&q) {
            Some(agent) => model::common::Result::<RegisteredAgent>::http_success(agent),
            None => model::common::Result::<String>::http_response(
                404,
                404,
                "Agent not found".to_string(),
                String::new(),
            ),
        }
    }
}

/// Register a new agent
/// POST /v3/console/ai/a2a
#[post("")]
async fn register_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<A2aServerOperationService>>>,
    body: web::Json<AgentRegistrationRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let req_body = body.into_inner();
    if let Some(svc) = a2a_service {
        match svc
            .register_agent(&req_body.card, &req_body.namespace, "manual")
            .await
        {
            Ok(_id) => {
                // Also register in in-memory for backward compat
                let _ = registry.register(req_body.clone());
                match svc
                    .get_agent_card(&req_body.namespace, &req_body.card.name, None)
                    .await
                {
                    Ok(Some(agent)) => {
                        model::common::Result::<RegisteredAgent>::http_success(agent)
                    }
                    _ => model::common::Result::<bool>::http_success(true),
                }
            }
            Err(e) => model::common::Result::<String>::http_response(
                400,
                error::PARAMETER_VALIDATE_ERROR.code,
                e.to_string(),
                String::new(),
            ),
        }
    } else {
        match registry.register(req_body) {
            Ok(agent) => model::common::Result::<RegisteredAgent>::http_success(agent),
            Err(e) => model::common::Result::<String>::http_response(
                400,
                error::PARAMETER_VALIDATE_ERROR.code,
                e,
                String::new(),
            ),
        }
    }
}

/// Update an existing agent
/// PUT /v3/console/ai/a2a?namespaceId=xxx&agentName=xxx
#[put("")]
async fn update_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<A2aServerOperationService>>>,
    query: web::Query<AgentDetailQuery>,
    body: web::Json<AgentRegistrationRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public").to_string();
    let req_body = body.into_inner();
    let fallback_name = req_body.card.name.clone();
    let name = q.agent_name.unwrap_or(fallback_name);

    if let Some(svc) = a2a_service {
        let mut card = req_body.card.clone();
        card.name = name.clone();
        match svc.update_agent_card(&card, &namespace, "manual").await {
            Ok(()) => {
                let _ = registry.update(&namespace, &name, req_body.clone());
                match svc.get_agent_card(&namespace, &name, None).await {
                    Ok(Some(agent)) => {
                        model::common::Result::<RegisteredAgent>::http_success(agent)
                    }
                    _ => model::common::Result::<bool>::http_success(true),
                }
            }
            Err(e) => model::common::Result::<String>::http_response(
                404,
                error::RESOURCE_NOT_FOUND.code,
                e.to_string(),
                String::new(),
            ),
        }
    } else {
        match registry.update(&namespace, &name, req_body) {
            Ok(agent) => model::common::Result::<RegisteredAgent>::http_success(agent),
            Err(e) => model::common::Result::<String>::http_response(
                404,
                error::RESOURCE_NOT_FOUND.code,
                e,
                String::new(),
            ),
        }
    }
}

/// Delete an agent
/// DELETE /v3/console/ai/a2a?namespaceId=xxx&agentName=xxx
#[delete("")]
async fn delete_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<A2aServerOperationService>>>,
    query: web::Query<AgentDeleteQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    if let Some(svc) = a2a_service {
        let namespace = q.namespace_id.as_deref().unwrap_or("public");
        if let Some(ref name) = q.agent_name {
            match svc
                .delete_agent(namespace, name, q.version.as_deref())
                .await
            {
                Ok(()) => {
                    let _ = registry.delete_by_query(&AgentDeleteQuery {
                        namespace_id: Some(namespace.to_string()),
                        agent_name: Some(name.clone()),
                        version: q.version.clone(),
                    });
                    model::common::Result::<bool>::http_success(true)
                }
                Err(e) => model::common::Result::<String>::http_response(
                    404,
                    error::RESOURCE_NOT_FOUND.code,
                    e.to_string(),
                    String::new(),
                ),
            }
        } else {
            model::common::Result::<String>::http_response(
                400,
                400,
                "agentName is required".to_string(),
                String::new(),
            )
        }
    } else {
        match registry.delete_by_query(&q) {
            Ok(()) => model::common::Result::<bool>::http_success(true),
            Err(e) => model::common::Result::<String>::http_response(
                404,
                error::RESOURCE_NOT_FOUND.code,
                e,
                String::new(),
            ),
        }
    }
}

/// List agent versions
/// GET /v3/console/ai/a2a/version/list
#[get("/version/list")]
async fn list_versions(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<A2aServerOperationService>>>,
    query: web::Query<AgentVersionListQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    let name = q.agent_name.as_deref().unwrap_or("");

    if let Some(svc) = a2a_service {
        match svc.list_versions(namespace, name).await {
            Ok(versions) => model::common::Result::<Vec<VersionDetail>>::http_success(versions),
            Err(e) => model::common::Result::<String>::http_response(
                500,
                500,
                e.to_string(),
                String::new(),
            ),
        }
    } else {
        let versions = registry.list_versions(namespace, name);
        model::common::Result::<Vec<RegisteredAgent>>::http_success(versions)
    }
}

/// Find agents by skill (Batata extension)
/// GET /v3/console/ai/a2a/by-skill/{skill}
#[get("/by-skill/{skill}")]
async fn find_by_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    path: web::Path<String>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let skill = path.into_inner();
    let agents = registry.find_by_skill(&skill);
    model::common::Result::<Vec<RegisteredAgent>>::http_success(agents)
}

/// Batch register agents (Batata extension)
/// POST /v3/console/ai/a2a/batch
#[post("/batch")]
async fn batch_register(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    body: web::Json<BatchAgentRegistrationRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = registry.batch_register(body.into_inner());
    model::common::Result::<BatchRegistrationResponse>::http_success(result)
}

/// Get agent registry statistics (Batata extension)
/// GET /v3/console/ai/a2a/stats
#[get("/stats")]
async fn get_stats(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let stats = registry.stats();
    model::common::Result::<AgentRegistryStats>::http_success(stats)
}

pub fn routes() -> Scope {
    web::scope("/ai/a2a")
        .service(get_stats)
        .service(list_versions)
        .service(find_by_skill)
        .service(batch_register)
        .service(list_agents)
        .service(register_agent)
        .service(update_agent)
        .service(get_agent)
        .service(delete_agent)
}
