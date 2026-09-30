//! Live-database coverage for the Skill and AgentSpec version lifecycle.
//!
//! `force_publish` and `redraft` were added to both domains so their admin APIs
//! match upstream. They delegate to the shared `version_lifecycle`, which was
//! extracted from MCP — these tests prove it behaves correctly for resource
//! types it was not extracted from.
//!
//! Run with a database:
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-ai --test skill_agentspec_lifecycle -- --ignored --test-threads=1
//! ```

mod common;

use std::collections::HashMap;
use std::sync::Arc;

use batata_ai::{AgentSpecOperationService, SkillOperationService};
use batata_common::model::ai::agentspec::{AgentSpec, AgentSpecResource, AGENTSPEC_TYPE};
use batata_common::model::ai::skill::{Skill, SKILL_TYPE};
use batata_persistence::{AiResourcePersistence, ExternalDbPersistService};

const NS: &str = "public";

async fn setup() -> Arc<ExternalDbPersistService> {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = common::connect_database(&url).await;
    Arc::new(ExternalDbPersistService::new(conn))
}

/// Status of one version, read straight from the version table.
async fn status_of(
    store: &ExternalDbPersistService,
    rt: &str,
    name: &str,
    version: &str,
) -> String {
    store
        .ai_resource_version_find(NS, name, rt, version)
        .await
        .expect("version lookup")
        .expect("version must exist")
        .status
}

async fn clean(store: &ExternalDbPersistService, rt: &str, name: &str) {
    store
        .ai_resource_version_delete_all(NS, name, rt)
        .await
        .expect("clean versions");
    store
        .ai_resource_delete(NS, name, rt)
        .await
        .expect("clean resource");
}

/// Skill: force-publish bypasses the review gate, and redraft sends a version
/// back to draft.
#[tokio::test]
#[ignore]
async fn skill_force_publish_and_redraft() {
    let store = setup().await;
    let svc = SkillOperationService::new(store.clone(), None, false);
    let name = "lifecycle-skill";
    let version = "1.0.0";
    clean(&store, SKILL_TYPE, name).await;

    let card = Skill {
        name: name.to_string(),
        namespace_id: NS.to_string(),
        ..Default::default()
    };
    let created = svc
        .create_draft(NS, name, None, Some(version), Some(&card), "tester")
        .await
        .expect("create draft");
    assert_eq!(created, version);
    assert_eq!(
        status_of(&store, SKILL_TYPE, name, version).await,
        "draft",
        "a new draft starts in draft"
    );

    // The whole point of force-publish: a draft cannot normally be published.
    assert!(
        svc.publish(NS, name, version, None).await.is_err(),
        "publishing a draft must fail"
    );

    svc.force_publish(NS, name, version, None)
        .await
        .expect("force publish");
    assert_eq!(
        status_of(&store, SKILL_TYPE, name, version).await,
        "online",
        "force-publish bypasses the review gate"
    );
    println!("ok: skill force-publish");

    svc.redraft(NS, name, version, None)
        .await
        .expect("redraft");
    assert_eq!(
        status_of(&store, SKILL_TYPE, name, version).await,
        "draft",
        "redraft moves an online version back to draft"
    );
    println!("ok: skill redraft");

    clean(&store, SKILL_TYPE, name).await;
}

/// AgentSpec: same lifecycle, same shared core.
#[tokio::test]
#[ignore]
async fn agentspec_force_publish_and_redraft() {
    let store = setup().await;
    let svc = AgentSpecOperationService::new(store.clone(), None, false);
    let name = "lifecycle-agentspec";
    let version = "1.0.0";
    clean(&store, AGENTSPEC_TYPE, name).await;

    let spec = AgentSpec {
        name: name.to_string(),
        namespace_id: NS.to_string(),
        ..Default::default()
    };
    let created = svc
        .create_draft(NS, name, None, Some(version), Some(&spec), "tester")
        .await
        .expect("create draft");
    assert_eq!(created, version);
    assert_eq!(
        status_of(&store, AGENTSPEC_TYPE, name, version).await,
        "draft"
    );

    assert!(
        svc.publish(NS, name, version, None).await.is_err(),
        "publishing a draft must fail"
    );

    svc.force_publish(NS, name, version, None)
        .await
        .expect("force publish");
    assert_eq!(
        status_of(&store, AGENTSPEC_TYPE, name, version).await,
        "online"
    );
    println!("ok: agentspec force-publish");

    svc.redraft(NS, name, version, None)
        .await
        .expect("redraft");
    assert_eq!(
        status_of(&store, AGENTSPEC_TYPE, name, version).await,
        "draft"
    );
    println!("ok: agentspec redraft");

    clean(&store, AGENTSPEC_TYPE, name).await;
}

/// `version/meta` exists to answer "what is in this version" without shipping
/// resource file contents — that difference is the whole point of the endpoint.
#[tokio::test]
#[ignore]
async fn agentspec_version_meta_omits_resource_contents() {
    let store = setup().await;
    let svc = AgentSpecOperationService::new(store.clone(), None, false);
    let name = "meta-agentspec";
    let version = "1.0.0";
    clean(&store, AGENTSPEC_TYPE, name).await;

    let mut resource = HashMap::new();
    resource.insert(
        "text::notes.md".to_string(),
        AgentSpecResource {
            name: "notes.md".to_string(),
            resource_type: "text".to_string(),
            content: Some("the file body".to_string()),
            metadata: HashMap::new(),
        },
    );
    let spec = AgentSpec {
        name: name.to_string(),
        namespace_id: NS.to_string(),
        description: Some("meta test".to_string()),
        content: Some("main manifest".to_string()),
        resource,
        ..Default::default()
    };
    svc.create_draft(NS, name, None, Some(version), Some(&spec), "tester")
        .await
        .expect("create draft");

    let full = svc
        .get_version_detail(NS, name, version, None)
        .await
        .expect("detail lookup")
        .expect("version must exist");
    let meta = svc
        .get_version_meta(NS, name, version, None)
        .await
        .expect("meta lookup")
        .expect("version must exist");

    // The main content is the same in both.
    assert_eq!(full.content.as_deref(), Some("main manifest"));
    assert_eq!(meta.content.as_deref(), Some("main manifest"));

    // The distinction: resources carry name + type, but no file contents.
    assert_eq!(full.resource.len(), 1, "detail returns the resource");
    assert_eq!(meta.resource.len(), 1, "meta still lists the resource");
    let full_res = full.resource.values().next().expect("one resource");
    let meta_res = meta.resource.values().next().expect("one resource");
    assert_eq!(
        full_res.content.as_deref(),
        Some("the file body"),
        "detail includes the file body"
    );
    assert_eq!(
        meta_res.content, None,
        "meta must omit resource file contents"
    );
    assert_eq!(meta_res.name, full_res.name);
    assert_eq!(meta_res.resource_type, full_res.resource_type);
    println!("ok: version/meta omits resource contents");

    clean(&store, AGENTSPEC_TYPE, name).await;
}
