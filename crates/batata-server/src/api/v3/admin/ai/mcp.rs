//! V3 Admin MCP server management endpoints
//! Aligned with Nacos V3 Admin API contract
//! Uses config-backed persistence when available, falls back to in-memory registry

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::ai::{
        McpServerOperationService, McpServerRegistry,
        model::{McpDeleteQuery, McpDetailQuery, McpListQuery, McpServerRegistration},
    },
    model::{common::AppState, response::Result},
    secured,
};

/// GET /v3/admin/ai/mcp/list
#[get("list")]
async fn list_mcp(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    mcp_service: Option<web::Data<Arc<McpServerOperationService>>>,
    query: web::Query<McpListQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = query.into_inner();
    if let Some(svc) = mcp_service {
        let namespace = q.namespace_id.as_deref().unwrap_or("public");
        let search_type = q.search.as_deref().unwrap_or("blur");
        let page_no = q.page_no.unwrap_or(1);
        let page_size = q.page_size.unwrap_or(20);
        let result = svc.list_mcp_servers(
            namespace,
            q.mcp_name.as_deref(),
            search_type,
            page_no,
            page_size,
        );
        HttpResponse::Ok().json(Result::success(result))
    } else {
        let result = registry.list_with_search(&q);
        HttpResponse::Ok().json(Result::success(result))
    }
}

/// GET /v3/admin/ai/mcp?namespaceId=xxx&mcpName=xxx
#[get("")]
async fn get_mcp(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    mcp_service: Option<web::Data<Arc<McpServerOperationService>>>,
    query: web::Query<McpDetailQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let q = query.into_inner();
    if let Some(svc) = mcp_service {
        let namespace = q.namespace_id.as_deref().unwrap_or("public");
        match svc
            .get_mcp_server_detail(
                namespace,
                q.mcp_id.as_deref(),
                q.mcp_name.as_deref(),
                q.version.as_deref(),
            )
            .await
        {
            Ok(Some(server)) => HttpResponse::Ok().json(Result::success(server)),
            Ok(None) => Result::<()>::http_not_found(
                &batata_common::error::MCP_SERVER_NOT_FOUND,
                "MCP server not found",
            ),
            Err(e) => Result::<()>::http_internal_error(e),
        }
    } else {
        match registry.get_by_query(&q) {
            Some(server) => HttpResponse::Ok().json(Result::success(server)),
            None => Result::<()>::http_not_found(
                &batata_common::error::MCP_SERVER_NOT_FOUND,
                "MCP server not found",
            ),
        }
    }
}

/// POST /v3/admin/ai/mcp
#[post("")]
async fn create_mcp(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    mcp_service: Option<web::Data<Arc<McpServerOperationService>>>,
    body: web::Json<McpServerRegistration>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Ai)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let registration = body.into_inner();
    if let Some(svc) = mcp_service {
        let namespace = if registration.namespace.is_empty() {
            "public"
        } else {
            &registration.namespace
        };
        match svc.create_mcp_server(namespace, &registration).await {
            Ok(id) => {
                let _ = registry.register(registration);
                HttpResponse::Ok().json(Result::success(id))
            }
            Err(e) => Result::<()>::http_bad_request(
                &batata_common::error::PARAMETER_VALIDATE_ERROR,
                e.to_string(),
            ),
        }
    } else {
        match registry.register(registration) {
            Ok(server) => HttpResponse::Ok().json(Result::success(server)),
            Err(e) => {
                Result::<()>::http_bad_request(&batata_common::error::PARAMETER_VALIDATE_ERROR, e)
            }
        }
    }
}

/// PUT /v3/admin/ai/mcp?namespaceId=xxx&mcpName=xxx
#[put("")]
async fn update_mcp(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    mcp_service: Option<web::Data<Arc<McpServerOperationService>>>,
    query: web::Query<McpDetailQuery>,
    body: web::Json<McpServerRegistration>,
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
    let registration = body.into_inner();
    let name = q.mcp_name.unwrap_or_else(|| registration.name.clone());

    if let Some(svc) = mcp_service {
        match svc.update_mcp_server(namespace, &registration).await {
            Ok(()) => {
                let _ = registry.update(namespace, &name, registration);
                HttpResponse::Ok().json(Result::success(true))
            }
            Err(e) => Result::<()>::http_not_found(
                &batata_common::error::MCP_SERVER_NOT_FOUND,
                e.to_string(),
            ),
        }
    } else {
        match registry.update(namespace, &name, registration) {
            Ok(server) => HttpResponse::Ok().json(Result::success(server)),
            Err(e) => Result::<()>::http_not_found(&batata_common::error::MCP_SERVER_NOT_FOUND, e),
        }
    }
}

/// DELETE /v3/admin/ai/mcp?namespaceId=xxx&mcpName=xxx
#[delete("")]
async fn delete_mcp(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    mcp_service: Option<web::Data<Arc<McpServerOperationService>>>,
    query: web::Query<McpDeleteQuery>,
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
    let name = q.mcp_name.as_deref().unwrap_or("");

    if let Some(svc) = mcp_service {
        match svc
            .delete_mcp_server(
                namespace,
                Some(name),
                q.mcp_id.as_deref(),
                q.version.as_deref(),
            )
            .await
        {
            Ok(()) => {
                let _ = registry.delete_by_query(&q);
                HttpResponse::Ok().json(Result::success(true))
            }
            Err(e) => Result::<()>::http_not_found(
                &batata_common::error::MCP_SERVER_NOT_FOUND,
                e.to_string(),
            ),
        }
    } else {
        match registry.delete_by_query(&q) {
            Ok(()) => HttpResponse::Ok().json(Result::success(true)),
            Err(e) => Result::<()>::http_not_found(&batata_common::error::MCP_SERVER_NOT_FOUND, e),
        }
    }
}

/// Query parameters for endpoint management
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpEndpointQuery {
    pub namespace_id: Option<String>,
    pub mcp_name: String,
    pub endpoint_url: Option<String>,
}

/// PUT /v3/admin/ai/mcp/endpoint
#[put("endpoint")]
async fn register_mcp_endpoint(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    query: web::Query<McpEndpointQuery>,
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

    match registry.register_endpoint(namespace, &q.mcp_name, &endpoint_url) {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_not_found(&batata_common::error::MCP_SERVER_NOT_FOUND, e),
    }
}

/// DELETE /v3/admin/ai/mcp/endpoint
#[delete("endpoint")]
async fn deregister_mcp_endpoint(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    query: web::Query<McpEndpointQuery>,
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

    match registry.deregister_endpoint(namespace, &q.mcp_name) {
        Ok(()) => HttpResponse::Ok().json(Result::success(true)),
        Err(e) => Result::<()>::http_not_found(&batata_common::error::MCP_SERVER_NOT_FOUND, e),
    }
}

pub fn routes() -> actix_web::Scope {
    web::scope("/mcp")
        .service(list_mcp)
        .service(register_mcp_endpoint)
        .service(deregister_mcp_endpoint)
        .service(create_mcp)
        .service(update_mcp)
        .service(get_mcp)
        .service(delete_mcp)
}
