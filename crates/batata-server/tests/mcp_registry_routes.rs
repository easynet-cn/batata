//! Route-level test for the MCP *Registry* OpenAPI (`GET /v0/servers`).
//!
//! This is a separate server from the main one — `mcp_registry_server` binds
//! its own port (default 9080) and mounts `configure_mcp_registry` on its own
//! `App`. Nothing else in the tree exercises these two handlers, and the
//! registry is held as an `Arc`, which is exactly the shape that makes
//! `web::Data` registration easy to get wrong: `Data::from(arc)` registers
//! `Data<T>` while `Data::new(arc)` registers `Data<Arc<T>>`, and the two are
//! different keys in the app-data type map.
//!
//! So this test wires the app up the same way production does rather than
//! picking whichever form makes the handler happy.

use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{App, test};
use batata_ai::model::{
    McpCapability, McpServerRegistration, McpServerType, McpTool, McpTransport,
};
use batata_ai::{McpServerRegistry, configure_mcp_registry, registry_app_data};

const NS: &str = "public";
const NAME: &str = "registry-route-mcp";

fn registration(version: &str) -> McpServerRegistration {
    McpServerRegistration {
        name: NAME.to_string(),
        display_name: "Registry MCP".to_string(),
        description: "a server in the registry".to_string(),
        namespace: NS.to_string(),
        version: version.to_string(),
        endpoint: "http://localhost:9090".to_string(),
        server_type: McpServerType::Http,
        transport: McpTransport::default(),
        capabilities: vec![McpCapability::Tool],
        tools: vec![McpTool {
            name: "echo".to_string(),
            description: "echoes".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        }],
        resources: vec![],
        prompts: vec![],
        metadata: std::collections::HashMap::new(),
        tags: vec![],
        auto_fetch_tools: true,
        health_check: None,
    }
}

/// Build the registry server's app the way `mcp_registry_server` does.
async fn build_app(
    registry: Arc<McpServerRegistry>,
) -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    test::init_service(
        App::new()
            // The same helper `mcp_registry_server` uses, so a registration
            // that drifts out of step with the handlers fails here.
            .app_data(registry_app_data(registry))
            .configure(configure_mcp_registry),
    )
    .await
}

async fn get<S>(app: &S, uri: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let resp = test::call_service(app, test::TestRequest::get().uri(uri).to_request()).await;
    let status = resp.status();
    (
        status,
        String::from_utf8_lossy(&test::read_body(resp).await).to_string(),
    )
}

/// GET /v0/servers lists what was registered.
#[actix_web::test]
async fn list_servers_returns_the_registered_catalog() {
    let registry = Arc::new(McpServerRegistry::new());
    let server = registry
        .register(registration("1.0.0"))
        .expect("register server");

    let app = build_app(registry).await;
    let (status, body) = get(&app, "/v0/servers").await;

    assert_eq!(
        status,
        StatusCode::OK,
        "the registry routes must resolve their app data: {body}"
    );
    assert!(body.contains(&format!("\"name\":\"{NAME}\"")), "{body}");
    assert!(body.contains("\"version\":\"1.0.0\""), "{body}");
    assert!(body.contains("\"count\":1"), "{body}");

    // And the detail endpoint resolves the same server by id.
    let (status, body) = get(&app, &format!("/v0/servers/{}", server.id)).await;
    assert_eq!(status, StatusCode::OK, "detail must resolve: {body}");
    assert!(body.contains(&format!("\"name\":\"{NAME}\"")), "{body}");
}

/// GET /v0/servers/{id} reports 404 for an unknown id.
#[actix_web::test]
async fn get_server_reports_not_found_for_an_unknown_id() {
    let registry = Arc::new(McpServerRegistry::new());
    let app = build_app(registry).await;

    let (status, body) = get(&app, "/v0/servers/does-not-exist").await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the registry routes must resolve their app data: {body}"
    );
    assert!(body.contains("not found"), "{body}");
}
