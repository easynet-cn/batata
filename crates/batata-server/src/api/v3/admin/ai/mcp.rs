//! V3 Admin MCP server management endpoints
//! Aligned with Nacos V3 Admin API contract for SDK compatibility
//! Uses config-backed persistence when available, falls back to in-memory registry

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use serde::Deserialize;

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::ai::{
        McpServerRegistry, McpServerService,
        model::{McpDeleteQuery, McpDetailQuery, McpListQuery, McpServerRegistration},
    },
    model::{common::AppState, response::Result},
    secured,
};

/// Form data for MCP create/update - accepts JSON-as-string params (Nacos SDK compatible).
/// The nacos-maintainer-client sends:
///   mcpName=xxx&namespaceId=xxx&serverSpecification=<JSON>&toolSpecification=<JSON>&endpointSpecification=<JSON>
///
/// Aligned with Nacos McpDetailForm + McpRequestUtil.parseMcpServerBasicInfo():
/// If `name` is empty in the serverSpecification JSON, it falls back to the
/// form-level `mcpName` parameter.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Fields deserialized from HTTP form, may not all be read in Rust
struct McpForm {
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: Option<String>,
    #[serde(default, alias = "mcpName")]
    pub mcp_name: Option<String>,
    #[serde(default, alias = "mcpId")]
    pub mcp_id: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default, alias = "serverSpecification")]
    pub server_specification: Option<String>,
    #[serde(default, alias = "toolSpecification")]
    pub tool_specification: Option<String>,
    #[serde(default, alias = "endpointSpecification")]
    pub endpoint_specification: Option<String>,
    #[serde(default)]
    pub latest: Option<bool>,
    #[serde(default, alias = "overrideExisting")]
    pub override_existing: Option<bool>,
}

impl McpForm {
    fn into_registration(self) -> std::result::Result<McpServerRegistration, String> {
        // Parse serverSpecification JSON string into the registration
        let server_json = self
            .server_specification
            .unwrap_or_else(|| "{}".to_string());
        let mut reg: McpServerRegistration = serde_json::from_str(&server_json)
            .map_err(|e| format!("Invalid serverSpecification: {}", e))?;

        // Backfill: if name is empty in serverSpecification, fill from mcpName form param (Nacos SDK compat)
        if reg.name.is_empty()
            && let Some(mcp_name) = &self.mcp_name
            && !mcp_name.is_empty()
        {
            reg.name = mcp_name.clone();
        }

        if let Some(ns) = self.namespace_id
            && !ns.is_empty()
        {
            reg.namespace = ns;
        }

        // Parse toolSpecification if present
        if let Some(tool_json) = self.tool_specification
            && !tool_json.is_empty()
            && let Ok(tools) = serde_json::from_str(&tool_json)
        {
            reg.tools = tools;
        }

        Ok(reg)
    }
}

/// GET /v3/admin/ai/mcp/list
#[get("list")]
async fn list_mcp(
    req: HttpRequest,
    data: web::Data<AppState>,
    registry: web::Data<Arc<McpServerRegistry>>,
    mcp_service: Option<web::Data<Arc<dyn McpServerService>>>,
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
    mcp_service: Option<web::Data<Arc<dyn McpServerService>>>,
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
    mcp_service: Option<web::Data<Arc<dyn McpServerService>>>,
    form: web::Form<McpForm>,
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
    if registration.name.is_empty() {
        return Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            "MCP server name is required (via mcpName param or name in serverSpecification)",
        );
    }
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
    mcp_service: Option<web::Data<Arc<dyn McpServerService>>>,
    query: web::Query<McpDetailQuery>,
    form: web::Form<McpForm>,
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
    mcp_service: Option<web::Data<Arc<dyn McpServerService>>>,
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
    #[serde(alias = "namespaceId")]
    pub namespace_id: Option<String>,
    #[serde(alias = "mcpName")]
    pub mcp_name: String,
    #[serde(alias = "endpointUrl")]
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
