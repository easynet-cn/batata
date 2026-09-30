//! Live-database coverage for skill ZIP precheck and batch upload.
//!
//! `POST /upload/precheck` and `POST /upload/batch` both read a ZIP that may
//! hold several skills — one-level subdirectories, each with its own SKILL.md.
//! These tests drive the service behind them.
//!
//! Run with a database:
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-ai --test skill_upload -- --ignored --test-threads=1
//! ```

mod common;

use std::io::{Cursor, Write};
use std::sync::Arc;

use batata_ai::SkillOperationService;
use batata_common::model::ai::skill::{SKILL_TYPE, precheck_code};
use batata_persistence::{AiResourcePersistence, ExternalDbPersistService};

const NS: &str = "public";

async fn setup() -> Arc<ExternalDbPersistService> {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = common::connect_database(&url).await;
    Arc::new(ExternalDbPersistService::new(conn))
}

/// Build a ZIP in memory from (path, content) pairs.
fn build_zip(files: &[(&str, &str)]) -> Vec<u8> {
    let writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut writer = writer;
    for (path, content) in files {
        writer.start_file(*path, options).unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

async fn clean(store: &ExternalDbPersistService, name: &str) {
    store
        .ai_resource_version_delete_all(NS, name, SKILL_TYPE)
        .await
        .expect("clean versions");
    store
        .ai_resource_delete(NS, name, SKILL_TYPE)
        .await
        .expect("clean resource");
}

/// A skill that does not exist yet is ready to upload at its manifest version.
#[tokio::test]
#[ignore]
async fn precheck_marks_a_new_skill_ready() {
    let store = setup().await;
    let svc = SkillOperationService::new(store.clone(), None, false);
    let name = "upload-precheck-new";
    clean(&store, name).await;

    let zip_bytes = build_zip(&[(
        "alpha/SKILL.md",
        "---\nname: upload-precheck-new\nversion: 1.0.0\n---\n",
    )]);

    let results = svc
        .precheck_upload_from_zip(NS, &zip_bytes, None)
        .await
        .expect("precheck");
    assert_eq!(results.len(), 1, "one entry per directory");

    let result = &results[0];
    assert_eq!(result.skill_name, name);
    assert!(!result.exists, "the skill must not exist yet");
    assert_eq!(
        result.precheck_code.as_deref(),
        Some(precheck_code::READY),
        "a new skill is ready: {:?}",
        result.reason
    );
    assert_eq!(result.target_version.as_deref(), Some("1.0.0"));
    println!("ok: precheck READY");

    // Nothing was persisted: that is the point of a precheck.
    assert!(
        store
            .ai_resource_find(NS, name, SKILL_TYPE)
            .await
            .expect("lookup")
            .is_none(),
        "precheck must not create anything"
    );
    println!("ok: precheck persisted nothing");

    clean(&store, name).await;
}

/// A directory without a SKILL.md is reported, not silently ignored.
#[tokio::test]
#[ignore]
async fn precheck_flags_a_directory_without_a_manifest() {
    let store = setup().await;
    let svc = SkillOperationService::new(store.clone(), None, false);

    let zip_bytes = build_zip(&[("gamma/notes.txt", "no manifest here")]);

    let results = svc
        .precheck_upload_from_zip(NS, &zip_bytes, None)
        .await
        .expect("precheck");
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].precheck_code.as_deref(),
        Some(precheck_code::NOT_A_SKILL)
    );
    assert!(
        results[0]
            .reason
            .as_deref()
            .is_some_and(|r| r.contains("SKILL.md")),
        "must explain the missing manifest: {:?}",
        results[0].reason
    );
    println!("ok: precheck NOT_A_SKILL");
}

/// Batch upload uses best effort: a bad entry does not discard the good ones.
#[tokio::test]
#[ignore]
async fn batch_upload_reports_success_and_failure() {
    let store = setup().await;
    let svc = SkillOperationService::new(store.clone(), None, false);
    let name = "upload-batch-ok";
    clean(&store, name).await;

    let zip_bytes = build_zip(&[
        (
            "alpha/SKILL.md",
            "---\nname: upload-batch-ok\nversion: 1.0.0\n---\n",
        ),
        ("gamma/notes.txt", "no manifest here"),
    ]);

    let result = svc
        .batch_upload_from_zip(NS, &zip_bytes, false, None)
        .await
        .expect("batch upload");

    assert_eq!(result.succeeded, vec![name.to_string()], "one succeeded");
    assert_eq!(result.failed.len(), 1, "one failed");
    assert_eq!(result.results.len(), 2, "both shapes stay in sync");
    assert!(
        result.results.iter().any(|r| r.success),
        "the successful item is recorded"
    );
    println!("ok: batch upload best effort");

    // The good skill really landed.
    assert!(
        store
            .ai_resource_find(NS, name, SKILL_TYPE)
            .await
            .expect("lookup")
            .is_some(),
        "the uploaded skill must exist"
    );

    clean(&store, name).await;
}
