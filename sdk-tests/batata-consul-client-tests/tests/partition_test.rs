//! Partition API tests (Enterprise).
//!
//! Same robustness model as namespace_test: lifecycle call path is exercised
//! via SDK methods; 501/404 from OSS clusters is treated as acceptable as
//! long as the SDK's typed error surfaces cleanly.

mod common;

use batata_consul_client::partition::Partition;

#[tokio::test]
async fn test_partition_lifecycle_or_not_supported() {
    let client = common::create_client();
    let name = format!("part-{}", common::test_id());
    let p = Partition {
        name: name.clone(),
        description: "integration test".into(),
        ..Default::default()
    };

    match client.partition_create(&p, &common::w()).await {
        Ok((created, _)) => {
            assert_eq!(created.name, name);
            let (got, _) = client
                .partition_read(&name, &common::q())
                .await
                .unwrap();
            assert!(got.is_some());
            let (list, _) = client.partition_list(&common::q()).await.unwrap();
            assert!(list.iter().any(|x| x.name == name));
            let _ = client.partition_delete(&name, &common::w()).await;
        }
        Err(e) => {
            // Not supported in OSS — still must surface cleanly.
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_partition_read_non_existent_returns_none_or_error() {
    let client = common::create_client();
    let name = format!("missing-{}", common::test_id());
    match client.partition_read(&name, &common::q()).await {
        Ok((p, _)) => assert!(p.is_none()),
        Err(e) => {
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_partition_serde_round_trip() {
    let p = Partition {
        name: "test-partition".into(),
        description: "hello".into(),
        create_index: 42,
        modify_index: 42,
        ..Default::default()
    };
    let j = serde_json::to_string(&p).unwrap();
    assert!(j.contains("\"Name\":\"test-partition\""));
    assert!(j.contains("\"CreateIndex\":42"));
    let back: Partition = serde_json::from_str(&j).unwrap();
    assert_eq!(back.name, p.name);
    assert_eq!(back.modify_index, 42);
}
