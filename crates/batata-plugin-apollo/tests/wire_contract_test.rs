//! Wire-level DTO contract tests against live upstream Apollo DTOs.
//!
//! Baseline: apollo 69a2b1fc, apollo-java 2f4b4a42. Every fixture below uses
//! the exact keys Gson emits (field-name identity, nulls omitted). A regression
//! in any `serde(rename)` / type must fail an assertion here, not just compile.

use batata_plugin_apollo::api::dto::{
    AccessKeyDTO, ApolloConfig, AppDTO, GrayReleaseRuleDTO, NamespaceGrayReleaseDTO,
    NotificationMessageDTO, ReleaseHistoryDTO,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

fn round_trip<T: DeserializeOwned + Serialize>(input: Value) -> Value {
    let parsed: T = serde_json::from_value(input).expect("fixture must deserialize");
    serde_json::to_value(parsed).expect("DTO must serialize")
}

#[test]
fn base_dto_last_modified_time_uses_upstream_key() {
    let ts = "2026-11-27T12:00:00.000+0800";
    let parsed: AppDTO = serde_json::from_value(json!({
        "appId": "app-1",
        "name": "demo",
        "orgId": "org-1",
        "orgName": "org",
        "ownerName": "alice",
        "ownerEmail": "a@b.c",
        "dataChangeLastModifiedTime": ts,
    }))
    .unwrap();
    assert_eq!(parsed.data_change_last_time.as_deref(), Some(ts));

    let value = serde_json::to_value(parsed).unwrap();
    assert_eq!(value["dataChangeLastModifiedTime"], json!(ts));
    assert!(
        !value
            .as_object()
            .unwrap()
            .contains_key("dataChangeLastTime")
    );

    // Older batata clients may still send the shortened key.
    let legacy: AppDTO = serde_json::from_value(json!({
        "appId": "app-1",
        "name": "demo",
        "orgId": "org-1",
        "orgName": "org",
        "ownerName": "alice",
        "ownerEmail": "a@b.c",
        "dataChangeLastTime": ts,
    }))
    .unwrap();
    assert_eq!(legacy.data_change_last_time.as_deref(), Some(ts));
}

#[test]
fn gray_release_rule_emits_upstream_rule_items_array() {
    let value = round_trip::<GrayReleaseRuleDTO>(json!({
        "appId": "app-1",
        "clusterName": "default",
        "namespaceName": "application",
        "branchName": "gray",
        "ruleItems": [{
            "clientAppId": "app-1",
            "clientIpList": ["*"],
            "clientLabelList": ["gray"]
        }],
        "releaseId": 42,
    }));

    assert!(value["ruleItems"].is_array());
    assert_eq!(value["ruleItems"][0]["clientAppId"], json!("app-1"));
    assert_eq!(value["ruleItems"][0]["clientIpList"], json!(["*"]));
    assert!(!value.as_object().unwrap().contains_key("rules"));
}

#[test]
fn gray_release_rule_accepts_legacy_escaped_rules_string() {
    let value = round_trip::<GrayReleaseRuleDTO>(json!({
        "appId": "app-1",
        "clusterName": "default",
        "namespaceName": "application",
        "branchName": "gray",
        "rules": "[{\"type\":\"IP\",\"ip\":\"192.168.1.0/24\"}]",
        "releaseId": 42,
    }));

    assert_eq!(value["ruleItems"][0]["ip"], json!("192.168.1.0/24"));
    assert!(!value.as_object().unwrap().contains_key("rules"));
}

#[test]
fn notification_details_are_numeric_notification_ids() {
    let value = round_trip::<NotificationMessageDTO>(json!({
        "details": {
            "app-1+default+application+release": 12345,
            "app-1+default+application+item": 7
        }
    }));

    assert_eq!(
        value["details"]["app-1+default+application+release"],
        json!(12345)
    );
    assert_eq!(value["details"]["app-1+default+application+item"], json!(7));
}

#[test]
fn release_history_operation_context_is_a_json_object() {
    let value = round_trip::<ReleaseHistoryDTO>(json!({
        "appId": "app-1",
        "clusterName": "default",
        "namespaceName": "application",
        "branchName": "gray",
        "releaseId": 42,
        "previousReleaseId": 41,
        "operation": 2,
        "operationContext": {"sourceBranch": "gray", "baseReleaseId": 41},
    }));

    assert_eq!(value["operationContext"]["sourceBranch"], json!("gray"));
    assert_eq!(value["operationContext"]["baseReleaseId"], json!(41));
}

#[test]
fn access_key_enabled_is_not_is_enabled() {
    let value = round_trip::<AccessKeyDTO>(json!({
        "appId": "app-1",
        "secret": "s",
        "mode": 1,
        "enabled": true,
    }));
    assert_eq!(value["enabled"], json!(true));
    assert!(!value.as_object().unwrap().contains_key("isEnabled"));

    let legacy = round_trip::<AccessKeyDTO>(json!({
        "appId": "app-1",
        "secret": "s",
        "mode": 1,
        "isEnabled": false,
    }));
    assert_eq!(legacy["enabled"], json!(false));
    assert!(!legacy.as_object().unwrap().contains_key("isEnabled"));
}

#[test]
fn apollo_config_carries_incremental_sync_fields() {
    let value = round_trip::<ApolloConfig>(json!({
        "appId": "app-1",
        "cluster": "default",
        "namespaceName": "application",
        "releaseKey": "20261127120000-1",
        "configurations": {"k": "v"},
        "configSyncType": "IncrementalSync",
        "configurationChanges": [{
            "key": "k",
            "newValue": "v",
            "configurationChangeType": "ADDED"
        }],
    }));

    assert_eq!(value["configSyncType"], json!("IncrementalSync"));
    assert_eq!(value["configurationChanges"][0]["newValue"], json!("v"));
    assert_eq!(
        value["configurationChanges"][0]["configurationChangeType"],
        json!("ADDED")
    );

    let full_sync = round_trip::<ApolloConfig>(json!({
        "appId": "app-1",
        "cluster": "default",
        "namespaceName": "application",
        "releaseKey": "key",
        "configurations": {},
    }));
    // Upstream omits both keys when incremental sync is disabled.
    assert!(
        !full_sync
            .as_object()
            .unwrap()
            .contains_key("configSyncType")
    );
    assert!(
        !full_sync
            .as_object()
            .unwrap()
            .contains_key("configurationChanges")
    );
}

#[test]
fn absent_delete_branch_defaults_to_upstream_true() {
    let parsed: NamespaceGrayReleaseDTO = serde_json::from_value(json!({
        "branchName": "gray",
        "releaseTitle": "merge",
        "releasedBy": "alice",
    }))
    .unwrap();
    assert!(parsed.delete_branch);

    let explicit: NamespaceGrayReleaseDTO = serde_json::from_value(json!({
        "branchName": "gray",
        "releaseTitle": "merge",
        "releasedBy": "alice",
        "deleteBranch": false,
    }))
    .unwrap();
    assert!(!explicit.delete_branch);
}
