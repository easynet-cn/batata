//! Visibility enforcement test for the A2A agent service.
//!
//! Separate test binary for the same reason as `mcp_visibility`:
//! `VisibilityPluginManager` is a process-global singleton and
//! `with_visibility` only registers when none exists, so a shared binary would
//! leave auth disabled and nothing would be filtered.
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-ai --test a2a_visibility -- --ignored --nocapture
//!
//! DATABASE_URL="postgres://postgres:devterry@127.0.0.1:5432/batata_ai_test" \
//!   cargo test -p batata-ai --test a2a_visibility -- --ignored --nocapture
//! ```

mod common;

use std::sync::Arc;

use batata_ai::model::{AgentCard, AgentCapabilities, AgentSkill};
use batata_ai::A2aServerOperationService;
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use batata_persistence::ExternalDbPersistService;

const NS: &str = "public";
const NAME: &str = "vis-agent";

fn card() -> AgentCard {
    AgentCard {
        name: NAME.to_string(),
        display_name: NAME.to_string(),
        description: "private by default".to_string(),
        version: "1.0.0".to_string(),
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
        ..Default::default()
    }
}

async fn clean(store: &ExternalDbPersistService) {
    let db = store.db();
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

/// Build a service with visibility **enabled** (auth enabled, no auth plugin).
async fn setup() -> (Arc<ExternalDbPersistService>, A2aServerOperationService) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = common::connect_database(&url).await;
    let store = Arc::new(ExternalDbPersistService::new(conn));
    // `auth_enabled = true` makes the advisor return `Public` for an anonymous
    // caller, which is what we need to observe filtering.
    let svc = A2aServerOperationService::with_visibility(store.clone(), None, true);
    clean(&store).await;
    (store, svc)
}

/// A newly registered agent is private, so an anonymous caller must not see it.
#[tokio::test]
#[ignore]
async fn private_agent_hidden_from_anonymous_list() {
    let (store, svc) = setup().await;
    svc.register_agent(&card(), NS, "manual")
        .await
        .expect("register");

    let page = svc
        .list_agents(NS, Some(NAME), "accurate", 1, 10, None)
        .await
        .expect("list");
    assert_eq!(
        page.total_count, 0,
        "a private agent must be hidden from an anonymous caller"
    );

    clean(&store).await;
}

/// A private agent must not be readable by an anonymous caller.
#[tokio::test]
#[ignore]
async fn private_agent_card_rejected_for_anonymous() {
    let (store, svc) = setup().await;
    svc.register_agent(&card(), NS, "manual")
        .await
        .expect("register");

    let result = svc.get_agent_card(NS, NAME, None, None).await;
    assert!(
        result.is_err(),
        "reading a private agent anonymously must fail"
    );

    clean(&store).await;
}
