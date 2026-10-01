//! Route-level test for the client resource search endpoint.
//!
//! The index is built by the search consumer as versions are published; this
//! covers the read side: `GET /v3/client/ai/resources/search`.
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-server --test resource_search_routes -- --ignored --nocapture
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use batata_ai::search::service::AiResourceSearchService;
use batata_ai::{
    McpServerIndex, McpServerOperationService,
    model::{McpCapability, McpServerRegistration, McpServerType, McpTool, McpTransport},
};
use batata_common::{ClusterHealthSummary, ClusterManager, ExtendedMemberInfo};
use batata_persistence::entity::{
    ai_resource, ai_resource_search_chunk, ai_resource_search_document, ai_resource_task,
    ai_resource_version,
};
use batata_persistence::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use batata_persistence::{AiResourcePersistence, ExternalDbPersistService};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::config::Configuration;

const NS: &str = "public";
const NAME: &str = "search-route-mcp";
/// A word that appears only in this server's indexed content.
const NEEDLE: &str = "zebrafish";

/// Connect to the test database, failing fast with an actionable message.
async fn connect_database(url: &str) -> sea_orm::DatabaseConnection {
    use std::time::Duration;
    let mut options = sea_orm::ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(3))
        .acquire_timeout(Duration::from_secs(3));
    match sea_orm::Database::connect(options).await {
        Ok(connection) => connection,
        Err(error) => panic!(
            "cannot reach the test database at {url}: {error}\n\
             hint: containers do not come back on their own after a machine \
             restart or a podman machine stop — try `podman start mysql postgres`"
        ),
    }
}

/// Cluster manager stub. The search route never touches cluster state.
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
        unimplemented!("not used by the search route")
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

async fn clean(store: &ExternalDbPersistService) {
    let db = store.db();
    ai_resource_search_chunk::Entity::delete_many()
        .filter(ai_resource_search_chunk::Column::ResourceName.eq(NAME))
        .exec(db)
        .await
        .expect("clean chunks");
    ai_resource_search_document::Entity::delete_many()
        .filter(ai_resource_search_document::Column::ResourceName.eq(NAME))
        .exec(db)
        .await
        .expect("clean documents");
    ai_resource_task::Entity::delete_many()
        .filter(ai_resource_task::Column::TaskKey.eq(batata_ai::search::task::task_key(
            NS, "mcp", NAME,
        )))
        .exec(db)
        .await
        .expect("clean tasks");
    ai_resource_version::Entity::delete_many()
        .filter(ai_resource_version::Column::Name.eq(NAME))
        .exec(db)
        .await
        .expect("clean versions");
    ai_resource::Entity::delete_many()
        .filter(ai_resource::Column::Name.eq(NAME))
        .exec(db)
        .await
        .expect("clean resources");
}

/// Build an app exposing the real client search route.
async fn build_app(
    store: Arc<ExternalDbPersistService>,
) -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    let mut configuration = Configuration::default();
    configuration.typed.core.auth.console.enabled = false;

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
        persistence: Some(store),
        health_check_manager: None,
        raft_node: None,
        server_status: Arc::new(batata_server_common::ServerStatusManager::new()),
        control_plugin: None,
        encryption_service: None,
        plugin_state_providers: vec![],
        plugin_manager: None,
        log_level_setter: None,
    });

    test::init_service(
        App::new()
            .app_data(web::Data::from(app_state))
            // Mounted as the server does: client routes live under `/v3/client`,
            // and the search module adds `/ai/resources/search`.
            .service(
                web::scope("/v3/client/ai").service(batata_ai::resource_search_client_routes()),
            ),
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
    (status, String::from_utf8_lossy(&test::read_body(resp).await).to_string())
}

/// Indexing a server makes it findable through the client search endpoint.
#[actix_web::test]
#[ignore]
async fn search_finds_an_indexed_resource() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));

    let mcp = McpServerOperationService::new(store.clone(), Arc::new(McpServerIndex::new()));
    let search = AiResourceSearchService::new(store.clone());

    clean(&store).await;
    let registration = McpServerRegistration {
        name: NAME.to_string(),
        display_name: NAME.to_string(),
        description: format!("a server about {NEEDLE}"),
        namespace: NS.to_string(),
        version: "1.0.0".to_string(),
        endpoint: "http://localhost:8080".to_string(),
        server_type: McpServerType::Http,
        transport: McpTransport::default(),
        capabilities: vec![McpCapability::Tool],
        tools: vec![McpTool {
            name: "identify".to_string(),
            description: format!("identifies a {NEEDLE}"),
            input_schema: serde_json::json!({"type": "object"}),
        }],
        resources: vec![],
        prompts: vec![],
        metadata: HashMap::new(),
        tags: vec![],
        auto_fetch_tools: true,
        health_check: None,
    };
    mcp.create_mcp_server(NS, &registration)
        .await
        .expect("create server");

    // Build the index the same way the consumer does on publish.
    let resource = store
        .ai_resource_find(NS, NAME, "mcp")
        .await
        .expect("find resource")
        .expect("resource must exist");
    let version = store
        .ai_resource_version_find(NS, NAME, "mcp", "1.0.0")
        .await
        .expect("find version")
        .expect("version must exist");
    assert!(
        search
            .rebuild_mcp_version(&resource, &version)
            .await
            .expect("rebuild"),
        "the index must be written"
    );

    let app = build_app(store.clone()).await;

    let (status, body) = get(
        &app,
        &format!("/v3/client/ai/resources/search?query={NEEDLE}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "search failed: {body}");
    assert!(body.contains(NAME), "must find the server: {body}");
    println!("ok: search finds an indexed resource");

    // A query is required; asking for nothing is a client error, not a scan.
    let (status, _) = get(&app, "/v3/client/ai/resources/search").await;
    assert!(
        status.is_client_error(),
        "a missing query must be rejected, got {status}"
    );
    println!("ok: search rejects an empty query");

    clean(&store).await;
}
