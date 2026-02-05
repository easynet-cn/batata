// Console A2A agent management API endpoints
// This module provides console endpoints for A2A agent registration and management

use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, Scope, delete, get, post, put, web};

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::ai::model::{
        AgentListResponse, AgentQuery, AgentRegistrationRequest, BatchAgentRegistrationRequest,
        BatchRegistrationResponse, RegisteredAgent,
    },
    api::ai::{AgentRegistry, AgentRegistryStats},
    model::{self, common::AppState},
    secured,
};

/// List agents
#[get("")]
async fn list_agents(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    params: web::Query<AgentQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let result = registry.list(&params.into_inner());
    model::common::Result::<AgentListResponse>::http_success(result)
}

/// Get agent by namespace and name
#[get("/{namespace}/{name}")]
async fn get_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let (namespace, name) = path.into_inner();
    match registry.get(&namespace, &name) {
        Some(agent) => model::common::Result::<RegisteredAgent>::http_success(agent),
        None => model::common::Result::<String>::http_response(
            404,
            404,
            format!("Agent '{}' not found in namespace '{}'", name, namespace),
            String::new(),
        ),
    }
}

/// Register a new agent
#[post("")]
async fn register_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    body: web::Json<AgentRegistrationRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match registry.register(body.into_inner()) {
        Ok(agent) => model::common::Result::<RegisteredAgent>::http_success(agent),
        Err(e) => model::common::Result::<String>::http_response(400, 400, e, String::new()),
    }
}

/// Update an existing agent
#[put("/{namespace}/{name}")]
async fn update_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    path: web::Path<(String, String)>,
    body: web::Json<AgentRegistrationRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let (namespace, name) = path.into_inner();
    match registry.update(&namespace, &name, body.into_inner()) {
        Ok(agent) => model::common::Result::<RegisteredAgent>::http_success(agent),
        Err(e) => model::common::Result::<String>::http_response(404, 404, e, String::new()),
    }
}

/// Delete an agent
#[delete("/{namespace}/{name}")]
async fn delete_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/ai/a2a")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let (namespace, name) = path.into_inner();
    match registry.deregister(&namespace, &name) {
        Ok(()) => model::common::Result::<bool>::http_success(true),
        Err(e) => model::common::Result::<String>::http_response(404, 404, e, String::new()),
    }
}

/// Find agents by skill
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

/// Batch register agents
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

/// Get agent registry statistics
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
    web::scope("/ai/a2a/agents")
        .service(get_stats)
        .service(find_by_skill)
        .service(batch_register)
        .service(list_agents)
        .service(get_agent)
        .service(register_agent)
        .service(update_agent)
        .service(delete_agent)
}
