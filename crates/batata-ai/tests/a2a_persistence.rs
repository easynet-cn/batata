//! Real-database round-trip test for the A2A agent service.
//!
//! Verifies the migration from config-backed storage to `ai_resource` /
//! `ai_resource_version`: id stability, versioning, latest-version advance,
//! in-place update, listing and deletion.
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-ai --test a2a_persistence -- --ignored --nocapture
//!
//! DATABASE_URL="postgres://postgres:devterry@127.0.0.1:5432/batata_ai_test" \
//!   cargo test -p batata-ai --test a2a_persistence -- --ignored --nocapture
//! ```

use std::sync::Arc;

use batata_ai::model::{AgentCard, AgentCapabilities, AgentSkill};
use batata_ai::A2aServerOperationService;
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};
use batata_persistence::ExternalDbPersistService;

const NS: &str = "public";
const NAME: &str = "probe-agent";

fn card(version: &str) -> AgentCard {
    AgentCard {
        name: NAME.to_string(),
        display_name: NAME.to_string(),
        description: format!("desc-{version}"),
        version: version.to_string(),
        url: "http://localhost:8080".to_string(),
        protocol_version: "1.0".to_string(),
        capabilities: AgentCapabilities {
            streaming: true,
            tool_use: true,
            ..Default::default()
        },
        skills: vec![AgentSkill {
            name: "coding".to_string(),
            description: "code generation".to_string(),
            proficiency: 90,
            examples: vec![],
        }],
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        preferred_transport: None,
        provider: None,
        documentation_url: None,
        icon_url: None,
        supports_authenticated_extended_card: None,
        metadata: Default::default(),
        tags: vec![],
    }
}

/// Remove rows left by a previous run so the test is repeatable.
async fn clean(store: &ExternalDbPersistService) {
    let conn = store.db();
    ai_resource_version::Entity::delete_many()
        .filter(ai_resource_version::Column::Name.eq(NAME))
        .exec(conn)
        .await
        .expect("clean versions");
    ai_resource::Entity::delete_many()
        .filter(ai_resource::Column::Name.eq(NAME))
        .exec(conn)
        .await
        .expect("clean resources");
}

#[tokio::test]
#[ignore]
async fn a2a_agent_round_trip() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = Database::connect(&url)
        .await
        .unwrap_or_else(|e| panic!("connect {url}: {e}"));
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let svc = A2aServerOperationService::new(store.clone());
    println!("--- backend: {:?} ---", store.db().get_database_backend());

    clean(&store).await;

    // ---- register ----------------------------------------------------------
    let id = svc
        .register_agent(&card("1.0.0"), NS, "manual")
        .await
        .expect("register");
    assert!(!id.is_empty(), "register must return an id");

    assert!(
        svc.register_agent(&card("1.0.0"), NS, "manual").await.is_err(),
        "duplicate registration must fail"
    );
    println!("ok: register id={id}");

    // ---- read latest -------------------------------------------------------
    let agent = svc
        .get_agent_card(NS, NAME, None)
        .await
        .expect("get")
        .expect("agent must exist");
    assert_eq!(agent.card.name, NAME);
    assert_eq!(agent.card.version, "1.0.0");
    assert_eq!(agent.id, id, "id must be stable across a read");
    println!("ok: read latest");

    // ---- add a second version ---------------------------------------------
    svc.update_agent_card(&card("2.0.0"), NS, "manual")
        .await
        .expect("add version");

    let latest = svc
        .get_agent_card(NS, NAME, None)
        .await
        .expect("get")
        .expect("agent must exist");
    assert_eq!(latest.card.version, "2.0.0", "latest must advance");

    let old = svc
        .get_agent_card(NS, NAME, Some("1.0.0"))
        .await
        .expect("get")
        .expect("old version must remain readable");
    assert_eq!(old.card.version, "1.0.0");
    println!("ok: versioning");

    // ---- list versions -----------------------------------------------------
    let versions = svc.list_versions(NS, NAME).await.expect("list versions");
    assert_eq!(versions.len(), 2, "expected two versions");
    assert_eq!(
        versions.iter().filter(|v| v.is_latest).count(),
        1,
        "exactly one version must be marked latest"
    );
    assert!(
        versions.iter().any(|v| v.is_latest && v.version == "2.0.0"),
        "2.0.0 must be the latest"
    );
    println!("ok: list_versions");

    // ---- update an existing version in place -------------------------------
    let mut updated = card("2.0.0");
    updated.description = "changed".to_string();
    svc.update_agent_card(&updated, NS, "manual")
        .await
        .expect("update existing");

    let reread = svc
        .get_agent_card(NS, NAME, Some("2.0.0"))
        .await
        .expect("get")
        .expect("agent must exist");
    assert_eq!(reread.card.description, "changed", "content must be updated");
    assert_eq!(
        svc.list_versions(NS, NAME).await.expect("list versions").len(),
        2,
        "updating must not duplicate the version"
    );
    println!("ok: update in place");

    // ---- list agents -------------------------------------------------------
    let page = svc
        .list_agents(NS, Some(NAME), "accurate", 1, 10)
        .await
        .expect("list agents");
    assert_eq!(page.total_count, 1, "expected exactly one agent");
    assert_eq!(page.page_items[0].name, NAME);
    assert_eq!(page.page_items[0].latest_published_version, "2.0.0");
    println!("ok: list_agents total={}", page.total_count);

    // ---- delete one version ------------------------------------------------
    svc.delete_agent(NS, NAME, Some("1.0.0"))
        .await
        .expect("delete version");
    assert!(
        svc.get_agent_card(NS, NAME, Some("1.0.0"))
            .await
            .expect("get")
            .is_none(),
        "deleted version must be gone"
    );
    assert_eq!(
        svc.list_versions(NS, NAME).await.expect("list versions").len(),
        1
    );
    println!("ok: delete single version");

    // ---- delete the whole agent --------------------------------------------
    svc.delete_agent(NS, NAME, None).await.expect("delete agent");
    assert!(svc.get_agent_card(NS, NAME, None).await.expect("get").is_none());
    assert_eq!(
        svc.list_agents(NS, Some(NAME), "accurate", 1, 10)
            .await
            .expect("list agents")
            .total_count,
        0
    );
    println!("ok: delete agent");

    // Deleting something that is already gone must not error.
    svc.delete_agent(NS, NAME, None)
        .await
        .expect("deleting a missing agent must be a no-op");
    println!("ok: delete missing is idempotent");
}
