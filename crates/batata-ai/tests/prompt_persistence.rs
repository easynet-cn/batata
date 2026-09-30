//! Live-database coverage for the prompt service.
//!
//! The prompt domain had **no tests at all**, and it is the last domain still
//! backed by configs rather than `ai_resource`. These pin down the behaviour
//! that is observable today so the storage migration has a regression baseline:
//! the same assertions must still hold once prompts live in `ai_resource`.
//!
//! Run with a database:
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-ai --test prompt_persistence -- --ignored --test-threads=1
//! ```

mod common;

use std::sync::Arc;

use batata_ai::PromptOperationService;
use batata_persistence::{AiResourcePersistence, ExternalDbPersistService};

const NS: &str = "public";

/// Each test uses its own key: prompts share the config namespace and a leaked
/// one would make the next test see a duplicate version.
const KEY: &str = "prompt-baseline";

async fn setup() -> Arc<ExternalDbPersistService> {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = common::connect_database(&url).await;
    Arc::new(ExternalDbPersistService::new(conn))
}

async fn publish(
    svc: &PromptOperationService,
    version: &str,
    template: &str,
) -> anyhow::Result<bool> {
    svc.publish_version(
        NS,
        KEY,
        version,
        template,
        Some("init"),
        Some("baseline prompt"),
        vec![],
        None,
        "tester",
        "127.0.0.1",
    )
    .await
}

async fn clean(svc: &PromptOperationService) {
    let _ = svc.delete_prompt(NS, KEY, "tester").await;
}

/// A published version can be read back by version, by label and by listing.
#[tokio::test]
#[ignore]
async fn publish_then_query_a_version() {
    let store = setup().await;
    let svc = PromptOperationService::new(store.clone());
    clean(&svc).await;

    assert!(publish(&svc, "1.0.0", "hello {{name}}").await.expect("publish"));

    let info = svc
        .query_prompt(NS, KEY, Some("1.0.0"), None, None)
        .await
        .expect("query")
        .expect("the version must exist");
    assert_eq!(info.template, "hello {{name}}");
    assert_eq!(info.version, "1.0.0");
    println!("ok: version queryable");

    // Latest resolution: no version and no label means the newest.
    let latest = svc
        .query_detail(NS, KEY, None, None)
        .await
        .expect("query detail")
        .expect("latest must resolve");
    assert_eq!(latest.version, "1.0.0");
    println!("ok: latest resolves");

    clean(&svc).await;
}

/// The client contract: an unchanged MD5 means "not modified", not "missing".
#[tokio::test]
#[ignore]
async fn query_prompt_returns_none_when_the_client_md5_matches() {
    let store = setup().await;
    let svc = PromptOperationService::new(store.clone());
    clean(&svc).await;

    publish(&svc, "1.0.0", "hello").await.expect("publish");

    let info = svc
        .query_prompt(NS, KEY, Some("1.0.0"), None, None)
        .await
        .expect("query")
        .expect("must exist");
    let md5 = info.md5.clone().expect("a published version carries an md5");

    // Same md5 → nothing to transfer.
    let unchanged = svc
        .query_prompt(NS, KEY, Some("1.0.0"), None, Some(&md5))
        .await
        .expect("query");
    assert!(unchanged.is_none(), "an unchanged md5 must report no body");

    // A different md5 → the body is sent.
    let changed = svc
        .query_prompt(NS, KEY, Some("1.0.0"), None, Some("stale"))
        .await
        .expect("query");
    assert!(changed.is_some(), "a stale md5 must return the body");
    println!("ok: md5 conditional query");

    clean(&svc).await;
}

/// Guards the API relies on: no duplicate versions, and a well-formed version.
#[tokio::test]
#[ignore]
async fn publish_rejects_duplicates_and_malformed_versions() {
    let store = setup().await;
    let svc = PromptOperationService::new(store.clone());
    clean(&svc).await;

    publish(&svc, "1.0.0", "hello").await.expect("first publish");

    assert!(
        publish(&svc, "1.0.0", "hello again").await.is_err(),
        "a duplicate version must be refused"
    );
    assert!(
        publish(&svc, "not-a-version", "hello").await.is_err(),
        "a malformed version must be refused"
    );
    assert!(
        publish(&svc, "1.0.1", "").await.is_err(),
        "an empty template must be refused"
    );
    println!("ok: publish guards");

    clean(&svc).await;
}

/// Labels route to a version, and listings see the prompt.
#[tokio::test]
#[ignore]
async fn bind_label_then_query_by_label() {
    let store = setup().await;
    let svc = PromptOperationService::new(store.clone());
    clean(&svc).await;

    publish(&svc, "1.0.0", "hello").await.expect("publish");
    assert!(
        svc.bind_label(NS, KEY, "stable", "1.0.0", "tester", "127.0.0.1")
            .await
            .expect("bind label")
    );

    let by_label = svc
        .query_detail(NS, KEY, None, Some("stable"))
        .await
        .expect("query")
        .expect("the label must resolve");
    assert_eq!(by_label.version, "1.0.0");
    println!("ok: label resolves to a version");

    // An unknown label is an error, not an empty result.
    assert!(
        svc.query_detail(NS, KEY, None, Some("nope"))
            .await
            .is_err(),
        "an unknown label must be reported"
    );

    let versions = svc
        .list_versions(NS, KEY, 1, 10)
        .await
        .expect("list versions");
    assert_eq!(versions.total_count, 1);
    println!("ok: version listing");

    let prompts = svc
        .list_prompts(NS, Some(KEY), Some("accurate"), None, 1, 10)
        .await
        .expect("list prompts");
    assert!(
        prompts.page_items.iter().any(|p| p.prompt_key == KEY),
        "the prompt must appear in the listing: {:?}",
        prompts.page_items.iter().map(|p| &p.prompt_key).collect::<Vec<_>>()
    );
    println!("ok: prompt listing");

    clean(&svc).await;
}

/// Deleting removes the versions too, so nothing resolves afterwards.
#[tokio::test]
#[ignore]
async fn delete_prompt_removes_everything() {
    let store = setup().await;
    let svc = PromptOperationService::new(store.clone());
    clean(&svc).await;

    publish(&svc, "1.0.0", "hello").await.expect("publish");
    assert!(svc.delete_prompt(NS, KEY, "tester").await.expect("delete"));

    let gone = svc
        .query_detail(NS, KEY, Some("1.0.0"), None)
        .await
        .expect("query");
    assert!(gone.is_none(), "the version must be gone");
    assert!(
        svc.get_meta(NS, KEY).await.is_none(),
        "the metadata must be gone"
    );
    println!("ok: delete removes versions and metadata");
}

/// Status of one version, read straight from the version table.
async fn status_of(store: &ExternalDbPersistService, version: &str) -> String {
    store
        .ai_resource_version_find(NS, KEY, "prompt", version)
        .await
        .expect("version lookup")
        .expect("version must exist")
        .status
}

/// The draft lifecycle, driven through the shared `version_lifecycle`.
#[tokio::test]
#[ignore]
async fn prompt_version_lifecycle() {
    let store = setup().await;
    let svc = PromptOperationService::new(store.clone());
    clean(&svc).await;

    let version = svc
        .create_draft(NS, KEY, Some("1.0.0"), "draft body", Some("desc"), None, "tester")
        .await
        .expect("create draft");
    assert_eq!(version, "1.0.0");
    assert_eq!(status_of(&store, "1.0.0").await, "draft");

    // A draft is not publishable: it has to be reviewed first.
    assert!(
        svc.publish(NS, KEY, "1.0.0").await.is_err(),
        "publishing a draft must fail"
    );

    svc.submit(NS, KEY, "1.0.0").await.expect("submit");
    assert_eq!(status_of(&store, "1.0.0").await, "reviewing");
    println!("ok: draft → reviewing");

    svc.publish(NS, KEY, "1.0.0").await.expect("publish");
    assert_eq!(status_of(&store, "1.0.0").await, "online");

    // Publishing makes the version the one served by default.
    let served = svc
        .query_detail(NS, KEY, None, None)
        .await
        .expect("query")
        .expect("a published version must be served");
    assert_eq!(served.version, "1.0.0");
    assert_eq!(served.template, "draft body");
    println!("ok: publish serves the version");

    svc.offline(NS, KEY, "1.0.0").await.expect("offline");
    assert_eq!(status_of(&store, "1.0.0").await, "offline");
    svc.online(NS, KEY, "1.0.0").await.expect("online");
    assert_eq!(status_of(&store, "1.0.0").await, "online");
    println!("ok: offline → online");

    svc.redraft(NS, KEY, "1.0.0").await.expect("redraft");
    assert_eq!(status_of(&store, "1.0.0").await, "draft");

    // The point of force-publish: it skips the review gate.
    svc.force_publish(NS, KEY, "1.0.0")
        .await
        .expect("force publish");
    assert_eq!(status_of(&store, "1.0.0").await, "online");
    println!("ok: force publish bypasses review");

    clean(&svc).await;
}

/// A draft can be edited and then discarded.
#[tokio::test]
#[ignore]
async fn draft_can_be_updated_and_discarded() {
    let store = setup().await;
    let svc = PromptOperationService::new(store.clone());
    clean(&svc).await;

    svc.create_draft(NS, KEY, Some("2.0.0"), "first", None, None, "tester")
        .await
        .expect("create draft");

    let updated = svc
        .update_draft(NS, KEY, "second", Some("tweak"), None, "tester")
        .await
        .expect("update draft");
    assert_eq!(updated, "2.0.0");
    let draft = svc
        .query_detail(NS, KEY, Some("2.0.0"), None)
        .await
        .expect("query")
        .expect("the draft must exist");
    assert_eq!(draft.template, "second");
    println!("ok: draft updated");

    svc.delete_draft(NS, KEY).await.expect("delete draft");
    assert!(
        store
            .ai_resource_version_find(NS, KEY, "prompt", "2.0.0")
            .await
            .expect("lookup")
            .is_none(),
        "the draft must be gone"
    );
    println!("ok: draft discarded");

    clean(&svc).await;
}
