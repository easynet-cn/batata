mod common;

use std::collections::HashMap;
use std::sync::Arc;

use batata_plugin_apollo::api::dto::GrayReleaseRuleDTO;
use batata_plugin_apollo::persistence::traits::ApolloPersistenceService;
use batata_plugin_apollo::service::GrayReleaseRuleService;

async fn create_rule(
    svc: &GrayReleaseRuleService,
    ns: &str,
    rules: &str,
    release_id: i64,
) {
    let dto = GrayReleaseRuleDTO {
        id: None,
        app_id: "app1".into(),
        cluster_name: "default".into(),
        namespace_name: ns.into(),
        branch_name: "gray".into(),
        rules: Some(rules.into()),
        release_id,
        branch_status: Some(1),
        priority: None,
        data_change_created_by: Some("tester".into()),
        data_change_last_modified_by: None,
        data_change_created_time: None,
    };
    svc.create(dto).await.unwrap();
}

fn labels(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[tokio::test]
async fn legacy_typed_rules_still_match() {
    let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;
    let svc = GrayReleaseRuleService::new(p);

    create_rule(
        &svc,
        "ns1",
        r#"[{"type":"LABEL","label":"env","value":"gray"}]"#,
        100,
    )
    .await;

    let matched = svc
        .match_gray_release_rule_with_context(
            "app1",
            "default",
            "ns1",
            "127.0.0.1",
            Some(&labels(&[("env", "gray")])),
            None,
            Some("env=gray"),
        )
        .await
        .unwrap();
    assert_eq!(matched, Some(100), "legacy LABEL rule should still match");

    create_rule(&svc, "ns2", r#"[{"type":"IP","ip":"192.168.1.0/24"}]"#, 200).await;
    let ip_match = svc
        .match_gray_release_rule_with_context(
            "app1",
            "default",
            "ns2",
            "192.168.1.10",
            Some(&labels(&[])),
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(ip_match, Some(200), "legacy IP rule (CIDR)");
}

#[tokio::test]
async fn upstream_format_app_ip_label_matching() {
    let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;
    let svc = GrayReleaseRuleService::new(p.clone());

    // Upstream GrayReleaseRuleItemTransformer JSON shape.
    // NOTE: rules are indexed by the CONFIG app (namespace owner, exact-case
    // lookup); item.clientAppId is compared against the REQUESTING app.
    create_rule(
        &svc,
        "up1",
        r#"[{"clientAppId":"consumer-a","clientIpList":["192.168.1.100"],"clientLabelList":["gray"]}]"#,
        300,
    )
    .await;

    // requesting-app mismatch → no match even with right ip/label
    let r = svc
        .match_gray_release_rule_with_context(
            "app1",
            "default",
            "up1",
            "192.168.1.100",
            Some(&labels(&[])),
            None,
            Some("gray"),
        )
        .await
        .unwrap();
    assert_eq!(r, None, "rule targets consumer-a, not the owner itself");

    // re-key the scenario: rule under the requesting app, targeting itself
    let dto = GrayReleaseRuleDTO {
        id: None,
        app_id: "consumer-a".into(),
        cluster_name: "default".into(),
        namespace_name: "up2".into(),
        branch_name: "gray".into(),
        rules: Some(
            r#"[{"clientAppId":"CONSUMER-A","clientIpList":["192.168.1.100"],"clientLabelList":["gray"]}]"#.into(),
        ),
        release_id: 300,
        branch_status: Some(1),
        priority: None,
        data_change_created_by: Some("tester".into()),
        data_change_last_modified_by: None,
        data_change_created_time: None,
    };
    svc.create(dto).await.unwrap();

    // exact ip + correct app; rule says CONSUMER-A, request sends
    // consumer-a → eq_ignore_ascii_case comparison
    let r = svc
        .match_gray_release_rule_with_context(
            "consumer-a",
            "default",
            "up2",
            "192.168.1.100",
            Some(&labels(&[])),
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(r, Some(300), "(app&&ip) leg, case-insensitive appId");

    // label leg with correct app
    let r = svc
        .match_gray_release_rule_with_context(
            "consumer-a",
            "default",
            "up2",
            "8.8.8.8",
            Some(&labels(&[])),
            None,
            Some("gray"),
        )
        .await
        .unwrap();
    assert_eq!(r, Some(300), "(app&&label) leg");

    // wrong label token
    let r = svc
        .match_gray_release_rule_with_context(
            "consumer-a",
            "default",
            "up2",
            "8.8.8.8",
            Some(&labels(&[])),
            None,
            Some("prod"),
        )
        .await
        .unwrap();
    assert_eq!(r, None);
}

#[tokio::test]
async fn upstream_wildcards_match_anything() {
    let p: Arc<dyn ApolloPersistenceService> = common::make_embedded_persistence().await;
    let svc = GrayReleaseRuleService::new(p);

    create_rule(
        &svc,
        "wild",
        r#"[{"clientAppId":"app1","clientIpList":["*"],"clientLabelList":["*"]}]"#,
        400,
    )
    .await;

    let r = svc
        .match_gray_release_rule_with_context(
            "app1",
            "default",
            "wild",
            "1.2.3.4",
            Some(&labels(&[])),
            None,
            Some("anything"),
        )
        .await
        .unwrap();
    assert_eq!(r, Some(400), "wildcard ip+label matches every client");
}
