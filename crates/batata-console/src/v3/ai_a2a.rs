// Console A2A agent management API endpoints
// Aligned with Nacos V3 Console API contract
// Uses Arc<dyn A2aAgentService> trait object (wired in batata-server)

use std::sync::Arc;

use actix_web::{HttpRequest, Responder, Scope, delete, get, post, put, web};

use batata_common::model::ai::a2a::{
    AgentDeleteQuery, AgentDetailQuery, AgentListQuery, AgentListResponse,
    AgentRegistrationRequest, AgentRegistryStats, AgentVersionListQuery,
    BatchAgentRegistrationRequest, BatchRegistrationResponse, RegisteredAgent,
};
use batata_common::{A2aAgentService, model::ai::VersionDetail};
use batata_server_common::error;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response as common_response;
use batata_server_common::secured::Secured;
use batata_server_common::{ActionTypes, ApiType, SignType, secured};

/// List agents
/// GET /v3/console/ai/a2a/list
#[get("/list")]
async fn list_agents(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn A2aAgentService>>,
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
        Ok(result) => common_response::Result::<AgentListResponse>::http_success(result),
        Err(e) => common_response::Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// Get agent by query params
/// GET /v3/console/ai/a2a?namespaceId=xxx&agentName=xxx
#[get("")]
async fn get_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn A2aAgentService>>,
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
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    if let Some(ref name) = q.agent_name {
        match svc
            .get_agent_card(namespace, name, q.version.as_deref())
            .await
        {
            Ok(Some(agent)) => common_response::Result::<RegisteredAgent>::http_success(agent),
            Ok(None) => common_response::Result::<String>::http_response(
                404,
                error::AGENT_NOT_FOUND.code,
                "Agent not found".to_string(),
                String::new(),
            ),
            Err(e) => common_response::Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            ),
        }
    } else {
        common_response::Result::<String>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            "agentName is required".to_string(),
            String::new(),
        )
    }
}

/// Register a new agent
/// POST /v3/console/ai/a2a
#[post("")]
async fn register_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn A2aAgentService>>,
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
    match svc
        .register_agent(&req_body.card, &req_body.namespace, "manual")
        .await
    {
        Ok(_id) => match svc
            .get_agent_card(&req_body.namespace, &req_body.card.name, None)
            .await
        {
            Ok(Some(agent)) => common_response::Result::<RegisteredAgent>::http_success(agent),
            _ => common_response::Result::<bool>::http_success(true),
        },
        Err(e) => common_response::Result::<String>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// Update an existing agent
/// PUT /v3/console/ai/a2a?namespaceId=xxx&agentName=xxx
#[put("")]
async fn update_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn A2aAgentService>>,
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

    let mut card = req_body.card;
    card.name = name.clone();
    match svc.update_agent_card(&card, &namespace, "manual").await {
        Ok(()) => match svc.get_agent_card(&namespace, &name, None).await {
            Ok(Some(agent)) => common_response::Result::<RegisteredAgent>::http_success(agent),
            _ => common_response::Result::<bool>::http_success(true),
        },
        Err(e) => common_response::Result::<String>::http_response(
            404,
            error::AGENT_NOT_FOUND.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// Delete an agent
/// DELETE /v3/console/ai/a2a?namespaceId=xxx&agentName=xxx
#[delete("")]
async fn delete_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn A2aAgentService>>,
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
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    if let Some(ref name) = q.agent_name {
        match svc
            .delete_agent(namespace, name, q.version.as_deref())
            .await
        {
            Ok(()) => common_response::Result::<bool>::http_success(true),
            Err(e) => common_response::Result::<String>::http_response(
                404,
                error::AGENT_NOT_FOUND.code,
                e.to_string(),
                String::new(),
            ),
        }
    } else {
        common_response::Result::<String>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            "agentName is required".to_string(),
            String::new(),
        )
    }
}

/// List agent versions
/// GET /v3/console/ai/a2a/version/list
#[get("/version/list")]
async fn list_versions(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn A2aAgentService>>,
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

    match svc.list_versions(namespace, name).await {
        Ok(versions) => common_response::Result::<Vec<VersionDetail>>::http_success(versions),
        Err(e) => common_response::Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// Find agents by skill (Batata extension)
/// GET /v3/console/ai/a2a/by-skill/{skill}
#[get("/by-skill/{skill}")]
async fn find_by_skill(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn A2aAgentService>>,
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
    match svc.find_by_skill(&skill).await {
        Ok(agents) => common_response::Result::<Vec<RegisteredAgent>>::http_success(agents),
        Err(e) => common_response::Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// Batch register agents (Batata extension)
/// POST /v3/console/ai/a2a/batch
#[post("/batch")]
async fn batch_register(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn A2aAgentService>>,
    body: web::Json<BatchAgentRegistrationRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match svc.batch_register(body.into_inner()).await {
        Ok(result) => common_response::Result::<BatchRegistrationResponse>::http_success(result),
        Err(e) => common_response::Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// Get agent registry statistics (Batata extension)
/// GET /v3/console/ai/a2a/stats
#[get("/stats")]
async fn get_stats(
    req: HttpRequest,
    data: web::Data<AppState>,
    svc: web::Data<Arc<dyn A2aAgentService>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match svc.stats().await {
        Ok(stats) => common_response::Result::<AgentRegistryStats>::http_success(stats),
        Err(e) => common_response::Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
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
