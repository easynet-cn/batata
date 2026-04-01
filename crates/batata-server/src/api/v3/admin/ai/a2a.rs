//! V3 Admin A2A agent management endpoints
//! Aligned with Nacos V3 Admin API contract
//! Uses config-backed persistence when available, falls back to in-memory registry

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::ai::{
        A2aAgentService, AgentRegistry,
        model::{
            AgentDeleteQuery, AgentDetailQuery, AgentListQuery, AgentRegistrationRequest,
            AgentVersionListQuery,
        },
    },
    model::{common::AppState, response::Result},
    secured,
};

/// Form data for A2A agent registration - accepts JSON-as-string params like Nacos.
/// The nacos-maintainer-client sends: agentCard=<JSON>&agentName=xxx&namespaceId=xxx
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentForm {
    #[serde(default, alias = "agentCard")]
    pub agent_card: Option<String>,
    #[serde(default, alias = "agentName")]
    pub agent_name: Option<String>,
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    #[serde(default, alias = "registrationType")]
    pub registration_type: Option<String>,
    #[serde(default)]
    pub latest: Option<bool>,
}

impl AgentForm {
    fn into_registration(self) -> std::result::Result<AgentRegistrationRequest, String> {
        let card_json = self.agent_card.unwrap_or_else(|| "{}".to_string());
        let card =
            serde_json::from_str(&card_json).map_err(|e| format!("Invalid agentCard: {}", e))?;
        let namespace = self.namespace_id.unwrap_or_default();
        Ok(AgentRegistrationRequest { card, namespace })
    }
}

/// POST /v3/admin/ai/a2a
#[post("")]
async fn register_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<dyn A2aAgentService>>>,
    form: web::Form<AgentForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let registration = match form.into_inner().into_registration() {
        Ok(r) => r,
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e,
            );
        }
    };
    if let Some(svc) = a2a_service {
        let namespace = if registration.namespace.is_empty() {
            "public"
        } else {
            &registration.namespace
        };
        match svc
            .register_agent(&registration.card, namespace, "sdk")
            .await
        {
            Ok(_) => {
                let _ = registry.register(registration);
                HttpResponse::Ok().json(Result::success(true))
            }
            Err(e) => Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e.to_string(),
            ),
        }
    } else {
        match registry.register(registration) {
            Ok(agent) => HttpResponse::Ok().json(Result::success(agent)),
            Err(e) => {
                Result::<()>::http_bad_request(&batata_common::error::PARAMETER_VALIDATE_ERROR, e)
            }
        }
    }
}

/// GET /v3/admin/ai/a2a?namespaceId=xxx&agentName=xxx
#[get("")]
async fn get_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<dyn A2aAgentService>>>,
    query: web::Query<AgentDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = query.into_inner();
    if let Some(svc) = a2a_service {
        let namespace = q.namespace_id.as_deref().unwrap_or("public");
        let name = q.agent_name.as_deref().unwrap_or("");
        match svc.get_agent_card(namespace, name, None).await {
            Ok(Some(agent)) => HttpResponse::Ok().json(Result::success(agent)),
            Ok(None) => Result::<()>::http_not_found(
                &batata_common::error::AGENT_NOT_FOUND,
                "Agent not found",
            ),
            Err(e) => Result::<()>::http_internal_error(e),
        }
    } else {
        match registry.get_by_query(&q) {
            Some(agent) => HttpResponse::Ok().json(Result::success(agent)),
            None => Result::<()>::http_not_found(
                &batata_common::error::AGENT_NOT_FOUND,
                "Agent not found",
            ),
        }
    }
}

/// PUT /v3/admin/ai/a2a?namespaceId=xxx&agentName=xxx
#[put("")]
async fn update_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<dyn A2aAgentService>>>,
    query: web::Query<AgentDetailQuery>,
    form: web::Form<AgentForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    let registration = match form.into_inner().into_registration() {
        Ok(r) => r,
        Err(e) => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e,
            );
        }
    };
    let name = q
        .agent_name
        .unwrap_or_else(|| registration.card.name.clone());

    if let Some(svc) = a2a_service {
        match svc
            .update_agent_card(&registration.card, namespace, "sdk")
            .await
        {
            Ok(()) => {
                let _ = registry.update(namespace, &name, registration);
                HttpResponse::Ok().json(Result::success(true))
            }
            Err(e) => {
                Result::<()>::http_not_found(&batata_common::error::AGENT_NOT_FOUND, e.to_string())
            }
        }
    } else {
        match registry.update(namespace, &name, registration) {
            Ok(agent) => HttpResponse::Ok().json(Result::success(agent)),
            Err(e) => Result::<()>::http_not_found(&batata_common::error::AGENT_NOT_FOUND, e),
        }
    }
}

/// DELETE /v3/admin/ai/a2a?namespaceId=xxx&agentName=xxx
#[delete("")]
async fn delete_agent(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<dyn A2aAgentService>>>,
    query: web::Query<AgentDeleteQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    let name = q.agent_name.as_deref().unwrap_or("");

    if let Some(svc) = a2a_service {
        match svc
            .delete_agent(namespace, name, q.version.as_deref())
            .await
        {
            Ok(()) => {
                let _ = registry.delete_by_query(&q);
                HttpResponse::Ok().json(Result::success(true))
            }
            Err(e) => {
                Result::<()>::http_not_found(&batata_common::error::AGENT_NOT_FOUND, e.to_string())
            }
        }
    } else {
        match registry.delete_by_query(&q) {
            Ok(()) => HttpResponse::Ok().json(Result::success(true)),
            Err(e) => Result::<()>::http_not_found(&batata_common::error::AGENT_NOT_FOUND, e),
        }
    }
}

/// GET /v3/admin/ai/a2a/list
#[get("list")]
async fn list_agents(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<dyn A2aAgentService>>>,
    query: web::Query<AgentListQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = query.into_inner();
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
            Ok(result) => HttpResponse::Ok().json(Result::success(result)),
            Err(e) => Result::<()>::http_internal_error(e),
        }
    } else {
        let result = registry.list_with_search(&q);
        HttpResponse::Ok().json(Result::success(result))
    }
}

/// GET /v3/admin/ai/a2a/version/list
#[get("version/list")]
async fn list_versions(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    a2a_service: Option<web::Data<Arc<dyn A2aAgentService>>>,
    query: web::Query<AgentVersionListQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    let name = q.agent_name.as_deref().unwrap_or("");

    if let Some(svc) = a2a_service {
        match svc.list_versions(namespace, name).await {
            Ok(versions) => HttpResponse::Ok().json(Result::success(versions)),
            Err(e) => Result::<()>::http_internal_error(e),
        }
    } else {
        let versions = registry.list_versions(namespace, name);
        HttpResponse::Ok().json(Result::success(versions))
    }
}

/// Query parameters for agent endpoint management
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndpointQuery {
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    #[serde(alias = "agentName")]
    pub agent_name: String,
    #[serde(alias = "endpointUrl")]
    pub endpoint_url: Option<String>,
}

/// PUT /v3/admin/ai/a2a/endpoint
#[put("endpoint")]
async fn register_agent_endpoint(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    query: web::Query<AgentEndpointQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public");
    let endpoint_url = match q.endpoint_url {
        Some(url) => url,
        None => {
            return Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_MISSING,
                "endpointUrl is required",
            );
        }
    };

    match registry.register_endpoint(namespace, &q.agent_name, &endpoint_url) {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_not_found(&batata_common::error::AGENT_NOT_FOUND, e),
    }
}

/// DELETE /v3/admin/ai/a2a/endpoint
#[delete("endpoint")]
async fn deregister_agent_endpoint(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<AgentRegistry>>,
    query: web::Query<AgentEndpointQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = query.into_inner();
    let namespace = q.namespace_id.as_deref().unwrap_or("public");

    match registry.deregister_endpoint(namespace, &q.agent_name) {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_not_found(&batata_common::error::AGENT_NOT_FOUND, e),
    }
}

pub fn routes() -> actix_web::Scope {
    web::scope("/a2a")
        .service(list_agents)
        .service(list_versions)
        .service(register_agent_endpoint)
        .service(deregister_agent_endpoint)
        .service(register_agent)
        .service(update_agent)
        .service(get_agent)
        .service(delete_agent)
}
