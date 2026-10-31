//! Route-level tests for the AI resource importer (`/v3/admin/ai/import`,
//! `/v3/console/ai/import`).
//!
//! Upstream anchors: `AiResourceImportAdminController` and
//! `ConsoleAiResourceImportController` expose the same four paths, and the
//! openapi IT cases (`AiResourceImport*ApiOpenApiITCase`) pin the whole
//! request-validation matrix. Until now batata answered `/sources` with three
//! invented sources and validated/executed every item as `VALID`/`SUCCESS`
//! without ever looking at a source, so none of that matrix held.
//!
//! Like the upstream IT, these cases never let a request reach a real external
//! network: every case stops at a form boundary, a source routing error, or a
//! source that batata has not ported yet. The one thing they *do* prove about
//! the item pipeline is that it no longer fabricates success.
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run the same cases
//! against a real database. See `common/ai_store.rs`.

#[path = "common/ai_store.rs"]
mod ai_store;

use std::sync::Arc;

use ai_store::TestStore;
use batata_common::McpServerService;

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::PersistenceService;
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const ADMIN: &str = "/v3/admin/ai/import";
const CONSOLE: &str = "/v3/console/ai/import";

/// Cluster manager stub. Nothing on this path reads cluster state.
struct StubClusterManager;

impl ClusterManager for StubClusterManager {
    fn is_standalone(&self) -> bool {
        true
    }
    fn is_leader(&self) -> bool {
        true
    }
    fn is_cluster_healthy(&self) -> bool {
        true
    }
    fn leader_address(&self) -> Option<String> {
        None
    }
    fn local_address(&self) -> &str {
        "127.0.0.1:8848"
    }
    fn member_count(&self) -> usize {
        1
    }
    fn all_members_extended(&self) -> Vec<ExtendedMemberInfo> {
        vec![]
    }
    fn healthy_members_extended(&self) -> Vec<ExtendedMemberInfo> {
        vec![]
    }
    fn get_member(&self, _address: &str) -> Option<ExtendedMemberInfo> {
        None
    }
    fn get_self_member(&self) -> ExtendedMemberInfo {
        unimplemented!("not used by the AI import routes")
    }
    fn health_summary(&self) -> ClusterHealthSummary {
        ClusterHealthSummary::default()
    }
    fn refresh_self(&self) {}
    fn is_self(&self, _address: &str) -> bool {
        false
    }
    fn update_member_state(&self, _address: &str, _state: &str) -> Result<String, String> {
        Ok("UP".to_string())
    }
}

/// Build an app exposing both importer mounts.
///
/// An embedded store is enough: nothing here persists, but `AppState` and the
/// `McpServerService` trait object are what the handlers actually pull out of
/// the request, so both are wired the way `startup::http` wires them.
async fn build_app(
    store: Arc<dyn PersistenceService>,
    registry_endpoint: Option<&str>,
    registry: Arc<batata_ai::McpServerRegistry>,
) -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    let mut configuration = Configuration::default();
    // `secured!` gates every handler; the default config has console auth on.
    configuration.typed.core.auth.console.enabled = false;
    // `mcp-registry-protocol` ships without an endpoint; pointing it at the
    // mock is how the search/fetch path is exercised without the internet.
    if let Some(endpoint) = registry_endpoint {
        configuration.typed.ai.import.sources.insert(
            "mcp-registry-protocol".to_string(),
            batata_server_common::model::typed_config::AiImportSourceConfig {
                endpoint: Some(endpoint.to_string()),
                enabled: true,
            },
        );
    }

    let subscriber: Arc<dyn batata_common::ConfigSubscriptionService> =
        Arc::new(batata_core::ConfigSubscriberManager::new());
    let cluster: Arc<dyn ClusterManager> = Arc::new(StubClusterManager);

    let console_datasource = Arc::new(batata_console::datasource::local::LocalDataSource::new(
        store.clone(),
        cluster,
        subscriber.clone(),
        configuration.clone(),
        None,
        vec![],
    ));

    let app_state = Arc::new(AppState {
        configuration,
        cluster_manager: None,
        config_subscriber_manager: subscriber,
        console_datasource,
        oauth_service: None,
        auth_plugin: None,
        persistence: Some(store.clone()),
        auth_data: None,
        health_check_manager: None,
        raft_node: None,
        server_status: Arc::new(batata_server_common::ServerStatusManager::new()),
        control_plugin: None,
        encryption_service: None,
        plugin_state_providers: vec![],
        plugin_manager: None,
        log_level_setter: None,
        connection_manager: Arc::new(std::sync::OnceLock::new()),
    });

    // `McpServerRegistry` is the in-memory `McpServerService` that
    // `startup::http` falls back to, and it needs no persistence of its own.
    let mcp: Arc<dyn batata_common::McpServerService> = registry;

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            // `Data::new`, not `Data::from`: the handlers take
            // `Data<Arc<...>>`, so the registered type is the `Arc` itself.
            .app_data(web::Data::new(mcp))
            // Mounted exactly as the admin scope and the console configurator do.
            .service(
                web::scope("/v3/admin/ai").service(batata_console::v3::ai_import::admin_routes()),
            )
            .service(
                web::scope("/v3/console/ai")
                    .service(batata_console::v3::ai_import::console_routes()),
            ),
    )
    .await
}

/// Dispatch a request and return `(status, body)`.
async fn call<S>(app: &S, req: actix_http::Request) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    (
        status,
        String::from_utf8_lossy(&test::read_body(resp).await).to_string(),
    )
}

async fn get<S>(app: &S, uri: &str) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    call(app, test::TestRequest::get().uri(uri).to_request()).await
}

async fn post_form<S>(app: &S, uri: &str, form: &[(&str, &str)]) -> (StatusCode, String)
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let body = serde_urlencoded::to_string(form).expect("form encodes");
    call(
        app,
        test::TestRequest::post()
            .uri(uri)
            .insert_header((
                actix_web::http::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            ))
            .set_payload(body)
            .to_request(),
    )
    .await
}

/// Find one source by id inside a `data` array.
fn source<'a>(body: &'a serde_json::Value, source_id: &str) -> Option<&'a serde_json::Value> {
    body.get("data")
        .and_then(|data| data.as_array())
        .and_then(|items| {
            items
                .iter()
                .find(|each| each.get("sourceId").and_then(|v| v.as_str()) == Some(source_id))
        })
}

/// Open an embedded store and build an app on top of it.
async fn app() -> (
    impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
) {
    let store = TestStore::open().await;
    (build_app(
        store.store(),
        None,
        Arc::new(batata_ai::McpServerRegistry::new()),
    )
    .await,)
}

/// An app plus the MCP registry it writes into.
async fn app_with_registry(
    endpoint: Option<&str>,
) -> (
    impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    Arc<batata_ai::McpServerRegistry>,
) {
    let store = TestStore::open().await;
    let registry = Arc::new(batata_ai::McpServerRegistry::new());
    (
        build_app(store.store(), endpoint, registry.clone()).await,
        registry,
    )
}

/// GET /sources — the four upstream sources, sanitized.
///
/// Upstream asserts `mcp-official` reports plugin name `mcp-registry` (the
/// *importer type*, not the source id) and that no runtime field leaks.
#[actix_web::test]
async fn sources_lists_upstream_plugins_without_runtime_fields() {
    let (app,) = app().await;
    let (status, body) = get(&app, &format!("{}/sources", ADMIN)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");

    let all = body.get("data").and_then(|d| d.as_array()).expect("array");
    assert_eq!(all.len(), 4, "{all:?}");

    // Upstream SPI order: protocol, official, well-known, skills.sh.
    let ids: Vec<&str> = all
        .iter()
        .filter_map(|each| each.get("sourceId").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(
        ids,
        vec![
            "mcp-registry-protocol",
            "mcp-official",
            "skills-well-known",
            "skills-sh"
        ],
        "{ids:?}"
    );

    let official = source(&body, "mcp-official").expect("mcp-official listed");
    assert_eq!(official["pluginName"], "mcp-registry");
    assert_eq!(official["displayName"], "Official MCP Registry");
    assert_eq!(official["enabled"], true);
    for capability in ["search", "validate", "execute"] {
        assert!(
            official["capabilities"]
                .as_array()
                .expect("capabilities")
                .iter()
                .any(|each| each == capability),
            "missing capability {capability}"
        );
    }

    let skills = source(&body, "skills-sh").expect("skills-sh listed");
    assert_eq!(skills["pluginName"], "skills-sh");
    assert_eq!(skills["displayName"], "skills.sh");

    for each in all {
        assert!(each.get("endpoint").is_none(), "endpoint leaked: {each}");
        assert!(
            each.get("properties").is_none(),
            "properties leaked: {each}"
        );
        assert!(each.get("authRef").is_none(), "authRef leaked: {each}");
    }
}

/// GET /sources?resourceType=… — filter, and an unknown type yields empty.
#[actix_web::test]
async fn sources_filters_by_resource_type() {
    let (app,) = app().await;

    let (status, body) = get(&app, &format!("{}/sources?resourceType=mcp", ADMIN)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    let mcp = body.get("data").and_then(|d| d.as_array()).expect("array");
    assert_eq!(mcp.len(), 2, "{mcp:?}");
    assert!(source(&body, "mcp-official").is_some());
    assert!(
        source(&body, "skills-sh").is_none(),
        "skill source leaked into mcp"
    );

    let (status, body) = get(&app, &format!("{}/sources?resourceType=skill", ADMIN)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    let skill = body.get("data").and_then(|d| d.as_array()).expect("array");
    assert_eq!(skill.len(), 2, "{skill:?}");
    assert!(source(&body, "skills-sh").is_some());
    assert!(
        source(&body, "mcp-official").is_none(),
        "mcp source leaked into skill"
    );

    let (status, body) = get(
        &app,
        &format!("{}/sources?resourceType=unsupported-resource-type", ADMIN),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(
        body.get("data").and_then(|d| d.as_array()).map(Vec::len),
        Some(0),
        "{body}"
    );
}

/// POST /search — `resourceType` and `sourceId` are mandatory.
#[actix_web::test]
async fn search_requires_resource_type_and_source_id() {
    let (app,) = app().await;

    let (status, body) = post_form(
        &app,
        &format!("{}/search", ADMIN),
        &[("sourceId", "mcp-official"), ("query", "nacos")],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_MISSING.code
    );
    assert!(body.contains("`resourceType`"), "{body}");

    let (status, body) = post_form(
        &app,
        &format!("{}/search", ADMIN),
        &[("resourceType", "mcp"), ("query", "nacos")],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_MISSING.code
    );
    assert!(body.contains("`sourceId`"), "{body}");
}

/// POST /search — an unparseable `options` is a validation error, not a 500.
#[actix_web::test]
async fn search_rejects_unparseable_options() {
    let (app,) = app().await;
    let (status, body) = post_form(
        &app,
        &format!("{}/search", ADMIN),
        &[
            ("resourceType", "mcp"),
            ("sourceId", "mcp-official"),
            ("options", "{"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_VALIDATE_ERROR.code
    );
    assert!(body.contains("`options`"), "{body}");
}

/// POST /search — an unknown source is RESOURCE_NOT_FOUND (404).
#[actix_web::test]
async fn search_reports_an_unknown_source_as_not_found() {
    let (app,) = app().await;
    let (status, body) = post_form(
        &app,
        &format!("{}/search", ADMIN),
        &[
            ("resourceType", "mcp"),
            ("sourceId", "openapi-it-unknown-source"),
            ("options", "{}"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::RESOURCE_NOT_FOUND.code
    );
    assert!(body.contains("plugin not found"), "{body}");
}

/// POST /search — a source that does not handle the type is a 400.
#[actix_web::test]
async fn search_rejects_a_source_that_does_not_handle_the_type() {
    let (app,) = app().await;
    let (status, body) = post_form(
        &app,
        &format!("{}/search", ADMIN),
        &[
            ("resourceType", "skill"),
            ("sourceId", "mcp-official"),
            ("options", "{}"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_VALIDATE_ERROR.code
    );
    assert!(body.contains("does not support resource type"), "{body}");
}

/// POST /search — a source whose endpoint was never configured is unusable.
///
/// Upstream rejects this inside `build()`, before any HTTP call is made.
#[actix_web::test]
async fn search_rejects_a_source_without_an_endpoint() {
    let (app,) = app().await;
    let (status, body) = post_form(
        &app,
        &format!("{}/search", ADMIN),
        &[
            ("resourceType", "mcp"),
            ("sourceId", "mcp-registry-protocol"),
            ("options", "{}"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_VALIDATE_ERROR.code
    );
    assert!(body.contains("endpoint is missing"), "{body}");
}

/// POST /validate — `selectedItems` is mandatory, parseable and non-empty.
#[actix_web::test]
async fn validate_checks_selected_items_before_anything_else() {
    let (app,) = app().await;

    // Absent → PARAMETER_MISSING.
    let (status, body) = post_form(
        &app,
        &format!("{}/validate", ADMIN),
        &[("resourceType", "skill"), ("sourceId", "skills-sh")],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_MISSING.code
    );
    assert!(body.contains("`selectedItems`"), "{body}");

    // Unparseable → PARAMETER_VALIDATE_ERROR on that parameter.
    let (status, body) = post_form(
        &app,
        &format!("{}/validate", ADMIN),
        &[
            ("resourceType", "skill"),
            ("sourceId", "skills-sh"),
            ("selectedItems", "{"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_VALIDATE_ERROR.code
    );
    assert!(body.contains("`selectedItems`"), "{body}");

    // An empty list parses fine, so it fails one step later.
    let (status, body) = post_form(
        &app,
        &format!("{}/validate", ADMIN),
        &[
            ("resourceType", "skill"),
            ("sourceId", "skills-sh"),
            ("selectedItems", "[]"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_VALIDATE_ERROR.code
    );
    assert!(body.contains("selected items must not be empty"), "{body}");
}

/// POST /validate — an unparseable `options` beats a valid `selectedItems`.
#[actix_web::test]
async fn validate_rejects_unparseable_options() {
    let (app,) = app().await;
    let (status, body) = post_form(
        &app,
        &format!("{}/validate", ADMIN),
        &[
            ("resourceType", "skill"),
            ("sourceId", "skills-sh"),
            ("selectedItems", SELECTED_ITEMS),
            ("options", "{"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_VALIDATE_ERROR.code
    );
    assert!(body.contains("`options`"), "{body}");
}

/// POST /validate — an unknown source is 404 here too.
#[actix_web::test]
async fn validate_reports_an_unknown_source_as_not_found() {
    let (app,) = app().await;
    let (status, body) = post_form(
        &app,
        &format!("{}/validate", ADMIN),
        &[
            ("resourceType", "skill"),
            ("sourceId", "openapi-it-unknown-source"),
            ("selectedItems", SELECTED_ITEMS),
            ("options", "{}"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::RESOURCE_NOT_FOUND.code
    );
    assert!(body.contains("plugin not found"), "{body}");
}

/// POST /execute — the same `selectedItems` boundaries apply.
#[actix_web::test]
async fn execute_checks_selected_items_before_anything_else() {
    let (app,) = app().await;

    let (status, body) = post_form(
        &app,
        &format!("{}/execute", ADMIN),
        &[("resourceType", "skill"), ("sourceId", "skills-sh")],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_MISSING.code
    );
    assert!(body.contains("`selectedItems`"), "{body}");

    let (status, body) = post_form(
        &app,
        &format!("{}/execute", ADMIN),
        &[
            ("resourceType", "skill"),
            ("sourceId", "skills-sh"),
            ("selectedItems", "[]"),
            ("skipInvalid", "true"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_VALIDATE_ERROR.code
    );
    assert!(body.contains("selected items must not be empty"), "{body}");
}

/// POST /execute — an unportable source reports FAILED, never SUCCESS.
///
/// This is the regression that matters: the old handler answered `SUCCESS`
/// for every item it was handed, so a caller could not tell "imported" from
/// "not even attempted". `skipInvalid` is what moves the verdict to SKIPPED.
#[actix_web::test]
async fn execute_marks_unimportable_items_failed_or_skipped() {
    let (app,) = app().await;

    let (status, body) = post_form(
        &app,
        &format!("{}/execute", ADMIN),
        &[
            ("resourceType", "skill"),
            ("sourceId", "skills-sh"),
            ("selectedItems", SELECTED_ITEMS),
            ("skipInvalid", "false"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    let data = &body["data"];
    assert_eq!(data["totalCount"], 1);
    assert_eq!(data["successCount"], 0);
    assert_eq!(data["failedCount"], 1);
    assert_eq!(data["skippedCount"], 0);
    assert_eq!(
        data["success"], false,
        "failed items must not report success"
    );
    assert_eq!(data["results"][0]["status"], "FAILED");
    assert!(
        data["results"][0]["errorMessage"]
            .as_str()
            .unwrap_or_default()
            .contains("not implemented yet"),
        "{}",
        data["results"][0]
    );

    let (status, body) = post_form(
        &app,
        &format!("{}/execute", ADMIN),
        &[
            ("resourceType", "skill"),
            ("sourceId", "skills-sh"),
            ("selectedItems", SELECTED_ITEMS),
            ("skipInvalid", "true"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    let data = &body["data"];
    assert_eq!(data["failedCount"], 0);
    assert_eq!(data["skippedCount"], 1);
    assert_eq!(data["results"][0]["status"], "SKIPPED");
    // `success` tracks *failed* items only, so a skipped batch still succeeds.
    assert_eq!(data["success"], true);
}

/// POST /validate — the same source yields INVALID items with the reason.
#[actix_web::test]
async fn validate_marks_unfetchable_items_invalid() {
    let (app,) = app().await;
    let (status, body) = post_form(
        &app,
        &format!("{}/validate", ADMIN),
        &[
            ("resourceType", "skill"),
            ("sourceId", "skills-well-known"),
            ("selectedItems", SELECTED_ITEMS),
        ],
    )
    .await;
    // `skills-well-known` ships without an endpoint, so it never gets that far.
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("endpoint is missing"), "{body}");

    let (status, body) = post_form(
        &app,
        &format!("{}/validate", ADMIN),
        &[
            ("resourceType", "skill"),
            ("sourceId", "skills-sh"),
            ("selectedItems", SELECTED_ITEMS),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    let items = body["data"]["items"].as_array().expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["status"], "INVALID");
    assert!(
        items[0]["errors"][0]
            .as_str()
            .unwrap_or_default()
            .contains("not implemented yet"),
        "{}",
        items[0]
    );
    assert_eq!(items[0]["externalId"], "external-skill");
}

/// The console mount serves the same four paths.
///
/// Upstream registers a separate controller with `ApiType.CONSOLE_API`; if the
/// console scope is missing, the UI has no importer at all.
#[actix_web::test]
async fn the_console_mount_serves_the_same_paths() {
    let (app,) = app().await;
    let (status, body) = get(&app, &format!("{}/sources", CONSOLE)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(
        body.get("data").and_then(|d| d.as_array()).map(Vec::len),
        Some(4),
        "{body}"
    );

    let (status, body) = post_form(
        &app,
        &format!("{}/search", CONSOLE),
        &[("resourceType", "mcp")],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        error_code(&body),
        batata_server_common::error::PARAMETER_MISSING.code
    );
}

// ============================================================================
// Mock MCP Registry
// ============================================================================

/// One page of a `GET /v0/servers` response, shaped like the official registry.
fn registry_page() -> serde_json::Value {
    serde_json::json!({
        "servers": [{
            "server": {
                "name": "io.github.example/demo",
                "description": "demo server",
                "version": "1.2.3",
                "status": "active",
                "repository": {"url": "https://example.com/repo", "source": "github"},
                "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}],
                "packages": []
            },
            "meta": {
                "official": {
                    "publishedAt": "2026-01-01T00:00:00Z",
                    "isLatest": true,
                    "status": "active"
                }
            }
        }],
        "metadata": {"nextCursor": null}
    })
}

/// Start a throwaway MCP Registry protocol endpoint on an ephemeral port.
///
/// Everything above stops before the network; these last cases are the ones
/// that prove the fetch → validate → import pipeline actually works, so the
/// source has to answer something.
async fn start_registry_mock() -> String {
    async fn page() -> actix_web::HttpResponse {
        actix_web::HttpResponse::Ok().json(registry_page())
    }

    let server = actix_web::HttpServer::new(|| {
        actix_web::App::new().route("/v0/servers", web::get().to(page))
    })
    .bind(("127.0.0.1", 0))
    .expect("bind the mock registry");
    let addr = server.addrs()[0];
    actix_web::rt::spawn(server.run());
    format!("http://{}/v0/servers", addr)
}

/// POST /search — a real registry page becomes candidate items.
#[actix_web::test]
async fn search_maps_a_registry_page_to_candidates() {
    let endpoint = start_registry_mock().await;
    let (app, _) = app_with_registry(Some(&endpoint)).await;

    let (status, body) = post_form(
        &app,
        &format!("{}/search", ADMIN),
        &[
            ("resourceType", "mcp"),
            ("sourceId", "mcp-registry-protocol"),
            ("query", "demo"),
            ("limit", "10"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    let data = &body["data"];
    // Upstream echoes the *plugin name* as `sourceId`, not the request's value.
    assert_eq!(data["sourceId"], "mcp-registry-protocol");
    assert_eq!(data["resourceType"], "mcp");
    assert_eq!(data["hasMore"], false);

    let items = data["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0]["externalId"], "io.github.example/demo");
    assert_eq!(items[0]["name"], "io.github.example/demo");
    assert_eq!(items[0]["version"], "1.2.3");
    assert_eq!(items[0]["description"], "demo server");
    assert_eq!(items[0]["metadata"]["protocol"], "streamable-http");
    assert_eq!(
        items[0]["metadata"]["repository"],
        "https://example.com/repo"
    );
    assert_eq!(items[0]["metadata"]["status"], "active");
}

/// POST /execute — a fetched MCP server is really created.
///
/// This is the case that separates "import" from "stub": the registry has to
/// hold the server afterwards, which is what `validate` then reports as a
/// conflict.
#[actix_web::test]
async fn execute_imports_a_fetched_mcp_server() {
    let endpoint = start_registry_mock().await;
    let (app, registry) = app_with_registry(Some(&endpoint)).await;

    let (status, body) = post_form(
        &app,
        &format!("{}/execute", ADMIN),
        &[
            ("resourceType", "mcp"),
            ("sourceId", "mcp-registry-protocol"),
            ("selectedItems", &mcp_selected_items()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    let data = &body["data"];
    assert_eq!(data["totalCount"], 1);
    assert_eq!(data["successCount"], 1, "{data}");
    assert_eq!(data["failedCount"], 0);
    assert_eq!(data["success"], true);
    assert_eq!(data["results"][0]["status"], "SUCCESS");
    assert_eq!(data["results"][0]["resourceName"], "io.github.example/demo");
    assert_eq!(data["results"][0]["version"], "1.2.3");

    let stored = registry
        .get_mcp_server_detail(
            "public",
            None,
            Some("io.github.example/demo"),
            Some("1.2.3"),
            None,
        )
        .await
        .expect("lookup");
    assert!(stored.is_some(), "the import did not reach the registry");
    assert_eq!(stored.expect("server").endpoint, "https://example.com/mcp");
}

/// POST /validate — VALID before the import, CONFLICT afterwards.
#[actix_web::test]
async fn validate_reports_a_conflict_once_the_server_exists() {
    let endpoint = start_registry_mock().await;
    let (app, _) = app_with_registry(Some(&endpoint)).await;

    let (status, body) = post_form(
        &app,
        &format!("{}/validate", ADMIN),
        &[
            ("resourceType", "mcp"),
            ("sourceId", "mcp-registry-protocol"),
            ("selectedItems", &mcp_selected_items()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(body["data"]["items"][0]["status"], "VALID");
    assert_eq!(body["data"]["items"][0]["exists"], false);
    assert_eq!(body["data"]["items"][0]["version"], "1.2.3");

    post_form(
        &app,
        &format!("{}/execute", ADMIN),
        &[
            ("resourceType", "mcp"),
            ("sourceId", "mcp-registry-protocol"),
            ("selectedItems", &mcp_selected_items()),
        ],
    )
    .await;

    let (status, body) = post_form(
        &app,
        &format!("{}/validate", ADMIN),
        &[
            ("resourceType", "mcp"),
            ("sourceId", "mcp-registry-protocol"),
            ("selectedItems", &mcp_selected_items()),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    let item = &body["data"]["items"][0];
    assert_eq!(item["status"], "CONFLICT", "{item}");
    assert_eq!(item["exists"], true);
    assert_eq!(item["conflictType"], "existing");
}

/// POST /execute — re-importing without `overwriteExisting` fails the item.
///
/// Covers the per-item failure that happens *after* a successful fetch, which
/// is the only branch the offline cases cannot reach.
#[actix_web::test]
async fn execute_fails_an_existing_server_unless_overwrite_is_allowed() {
    let endpoint = start_registry_mock().await;
    let (app, _) = app_with_registry(Some(&endpoint)).await;

    let first = {
        let items = mcp_selected_items();
        post_form(
            &app,
            &format!("{}/execute", ADMIN),
            &[
                ("resourceType", "mcp"),
                ("sourceId", "mcp-registry-protocol"),
                ("selectedItems", &items),
                ("overwriteExisting", "true"),
            ],
        )
        .await
    };
    assert_eq!(first.0, StatusCode::OK, "{}", first.1);

    let items = mcp_selected_items();
    let (status, body) = post_form(
        &app,
        &format!("{}/execute", ADMIN),
        &[
            ("resourceType", "mcp"),
            ("sourceId", "mcp-registry-protocol"),
            ("selectedItems", &items),
            ("overwriteExisting", "false"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    let data = &body["data"];
    assert_eq!(data["failedCount"], 1, "{data}");
    assert_eq!(data["success"], false);
    assert_eq!(data["results"][0]["status"], "FAILED");
    assert!(
        data["results"][0]["errorMessage"]
            .as_str()
            .unwrap_or_default()
            .contains("already exists"),
        "{}",
        data["results"][0]
    );

    // `skipInvalid` turns the same failure into a skip.
    let (status, body) = post_form(
        &app,
        &format!("{}/execute", ADMIN),
        &[
            ("resourceType", "mcp"),
            ("sourceId", "mcp-registry-protocol"),
            ("selectedItems", &mcp_selected_items()),
            ("overwriteExisting", "false"),
            ("skipInvalid", "true"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(body["data"]["results"][0]["status"], "SKIPPED");
    assert_eq!(body["data"]["skippedCount"], 1);
    let _ = endpoint;
}

/// One `selectedItems` entry naming a server the mock registry serves.
fn mcp_selected_items() -> String {
    r#"[{"externalId":"io.github.example/demo","name":"io.github.example/demo",
         "version":"1.2.3","metadata":{"source":"batata-it"}}]"#
        .to_string()
}

/// The `code` field of a v3 error body.
fn error_code(body: &str) -> i32 {
    serde_json::from_str::<serde_json::Value>(body)
        .expect("json")
        .get("code")
        .and_then(|code| code.as_i64())
        .expect("code") as i32
}

/// One `selectedItems` entry, JSON-encoded the way the SDK sends it.
const SELECTED_ITEMS: &str = "[{\"externalId\":\"external-skill\",\"name\":\"openapi-skill\",\
                              \"version\":\"1.0.0\",\"metadata\":{\"source\":\"openapi-it\"}}]";
