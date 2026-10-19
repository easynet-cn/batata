//! Instance lifecycle and idempotency semantics.
//!
//! Scenarios are ported from Nacos'
//! `EphemeralClientOperationServiceImplTest`,
//! `PersistentClientOperationServiceImplTest` and the v2 instance operator
//! tests, adapted to batata's in-memory `NamingService`.
//!
//! Two places deliberately assert *deviations* rather than blindly copying the
//! upstream assertions — both are recorded in the test name and the comment:
//! - batata has **no service-level ephemeral flag** yet (`ServiceMetadata::
//!   ephemeral` exists but is unused), so the AP/CP gate keys off the
//!   instance flag instead of the service flag.
//! - upstream never filters `enabled=false` or `weight=0` out of query
//!   results (`ServiceUtil.doSelectInstances` only honours `enableOnly` when
//!   it is explicitly requested, and no query path does). See
//!   `disabled_and_zero_weight_instances_are_still_returned`.

mod common;

use std::sync::Arc;

use batata_api::naming::model::Instance;
use batata_naming::service::{
    ClientOperationService, ClientOperationServiceProxy, EphemeralClientOperationService,
    InstancePatch, PersistentClientOperationService, apply_instance_patch,
};
use common::{
    GROUP, NS, endpoints, instance, instance_in_cluster, meta, patch, persistent_instance, query,
    register_ap, register_cp, service,
};

const SVC: &str = "svc";

// ---------------------------------------------------------------------------
// 1) AP (Distro) vs CP (Raft) registration paths
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ap_path_stores_ephemeral_instance() {
    let svc = Arc::new(service());
    assert!(register_ap(&svc, SVC, instance("10.0.0.1", 8080)).await);

    let got = query(&svc, SVC, "", false);
    assert_eq!(endpoints(&got), vec!["10.0.0.1:8080"]);
    assert!(got[0].ephemeral, "AP path must keep the instance ephemeral");
}

#[tokio::test]
async fn cp_path_stores_persistent_instance() {
    let svc = Arc::new(service());
    // No Raft node → documented dev-mode fallback writes straight to DashMap.
    assert!(register_cp(&svc, SVC, persistent_instance("10.0.0.1", 8080)).await);

    let got = query(&svc, SVC, "", false);
    assert_eq!(endpoints(&got), vec!["10.0.0.1:8080"]);
    assert!(
        !got[0].ephemeral,
        "CP path must keep the instance persistent"
    );
}

#[tokio::test]
async fn ap_path_rejects_persistent_instance() {
    // Upstream: `EphemeralClientOperationServiceImpl#registerInstance` throws
    // NacosRuntimeException("... can't register ephemeral instance.") when the
    // service is persistent. batata models the flag on the instance, so the
    // same rule applies: a persistent instance must never be published over
    // Distro, because that would drop Raft durability silently.
    let svc = Arc::new(service());
    assert!(
        !register_ap(&svc, SVC, persistent_instance("10.0.0.1", 8080)).await,
        "AP path must refuse a persistent instance instead of coercing it"
    );
    assert!(
        query(&svc, SVC, "", false).is_empty(),
        "rejected registration must not leave an instance behind"
    );
}

#[tokio::test]
async fn cp_path_rejects_ephemeral_instance() {
    // Mirror of the AP guard (upstream: "can't register persistent instance").
    let svc = Arc::new(service());
    assert!(
        !register_cp(&svc, SVC, instance("10.0.0.1", 8080)).await,
        "CP path must refuse an ephemeral instance"
    );
    assert!(query(&svc, SVC, "", false).is_empty());
}

#[tokio::test]
async fn proxy_dispatches_by_ephemeral_flag() {
    let svc = Arc::new(service());
    let proxy = ClientOperationServiceProxy::new(
        Arc::new(EphemeralClientOperationService::new(Arc::clone(&svc), None)),
        Arc::new(PersistentClientOperationService::new(
            Arc::clone(&svc),
            None,
        )),
    );

    // Two services, because a service's ephemeral flag is fixed by its first
    // registration — mixing both kinds in one service is refused, see
    // `tests/service_ephemeral.rs`.
    assert!(
        proxy
            .register_instance(NS, GROUP, "ap-svc", instance("10.0.0.1", 8080))
            .await
    );
    assert!(
        proxy
            .register_instance(NS, GROUP, "cp-svc", persistent_instance("10.0.0.2", 8080))
            .await
    );

    assert!(query(&svc, "ap-svc", "", false)[0].ephemeral);
    assert!(!query(&svc, "cp-svc", "", false)[0].ephemeral);
}

// ---------------------------------------------------------------------------
// 2) Idempotency
// ---------------------------------------------------------------------------

#[test]
fn re_register_replaces_metadata_without_duplicating() {
    let svc = service();
    let mut first = instance("10.0.0.1", 8080);
    first.metadata = meta(&[("a", "1"), ("b", "2")]);
    svc.register_instance(NS, GROUP, SVC, first);

    let mut second = instance("10.0.0.1", 8080);
    second.metadata = meta(&[("c", "3")]);
    svc.register_instance(NS, GROUP, SVC, second);

    assert_eq!(svc.get_instance_count(NS, GROUP, SVC), 1);
    let got = query(&svc, SVC, "", false);
    assert_eq!(
        got[0].metadata,
        meta(&[("c", "3")]),
        "register is a full replace, not a merge"
    );
}

#[test]
fn deregister_missing_instance_is_a_noop_that_succeeds() {
    // Upstream `testRemoveInstanceIgnoresMissingClient`: removing something
    // that was never published must not fail and must not touch neighbours.
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    assert!(svc.deregister_instance(NS, GROUP, SVC, &instance("10.0.0.9", 8080)));
    assert_eq!(svc.get_instance_count(NS, GROUP, SVC), 1);
}

#[test]
fn deregister_from_unknown_service_succeeds_without_creating_it() {
    // Upstream `testDeregisterNonExistService`.
    let svc = service();
    assert!(svc.deregister_instance(NS, GROUP, "never-seen", &instance("10.0.0.1", 8080)));
    assert!(
        !svc.service_exists(NS, GROUP, "never-seen"),
        "deregistering must not materialise a service"
    );
}

#[test]
fn deregister_without_published_instance_succeeds() {
    // Upstream `testDeregisterWithoutPublishedInstance`: the service exists
    // (was published once) but holds no instance any more.
    let svc = service();
    let inst = instance("10.0.0.1", 8080);
    svc.register_instance(NS, GROUP, SVC, inst.clone());
    svc.deregister_instance(NS, GROUP, SVC, &inst);

    assert!(svc.deregister_instance(NS, GROUP, SVC, &inst));
    assert_eq!(svc.get_instance_count(NS, GROUP, SVC), 0);
    assert!(svc.service_exists(NS, GROUP, SVC));
}

// ---------------------------------------------------------------------------
// 3) Batch register / partial batch deregister
// ---------------------------------------------------------------------------

#[test]
fn batch_register_reconciles_the_full_instance_set() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    assert!(svc.batch_register_instances(
        NS,
        GROUP,
        SVC,
        vec![instance("10.0.0.2", 8080), instance("10.0.0.3", 8080)],
    ));

    assert_eq!(
        endpoints(&query(&svc, SVC, "", false)),
        vec!["10.0.0.2:8080", "10.0.0.3:8080"],
        "batch register is a reconciliation — it replaces, never appends"
    );
}

#[test]
fn batch_register_with_empty_list_clears_the_service() {
    let svc = service();
    svc.batch_register_instances(NS, GROUP, SVC, vec![instance("10.0.0.1", 8080)]);
    assert!(svc.batch_register_instances(NS, GROUP, SVC, vec![]));

    assert_eq!(svc.get_instance_count(NS, GROUP, SVC), 0);
}

#[test]
fn batch_deregister_removes_only_the_listed_instances() {
    let svc = service();
    for n in 1..=3 {
        svc.register_instance(NS, GROUP, SVC, instance(&format!("10.0.0.{n}"), 8080));
    }

    assert!(svc.batch_deregister_instances(
        NS,
        GROUP,
        SVC,
        &[instance("10.0.0.1", 8080), instance("10.0.0.3", 8080)],
    ));

    assert_eq!(
        endpoints(&query(&svc, SVC, "", false)),
        vec!["10.0.0.2:8080"]
    );
}

#[test]
fn batch_and_single_registration_mix_cleanly() {
    let svc = service();
    svc.batch_register_instances(
        NS,
        GROUP,
        SVC,
        vec![instance("10.0.0.1", 8080), instance("10.0.0.2", 8080)],
    );
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.3", 8080));

    assert_eq!(svc.get_instance_count(NS, GROUP, SVC), 3);
}

// ---------------------------------------------------------------------------
// 4) Update (PUT) vs patch (PATCH)
// ---------------------------------------------------------------------------

#[test]
fn full_update_replaces_metadata_wholesale() {
    let svc = service();
    let mut original = instance("10.0.0.1", 8080);
    original.metadata = meta(&[("a", "1"), ("b", "2")]);
    svc.register_instance(NS, GROUP, SVC, original);

    // A PUT rebuilds the instance from the submitted form — metadata keys the
    // caller omits are gone.
    let mut updated = instance("10.0.0.1", 8080);
    updated.metadata = meta(&[("b", "9")]);
    svc.register_instance(NS, GROUP, SVC, updated);

    let got = query(&svc, SVC, "", false);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].metadata, meta(&[("b", "9")]));
}

#[test]
fn patch_keeps_old_keys_and_overrides_new_ones() {
    let svc = service();
    let mut original = instance("10.0.0.1", 8080);
    original.metadata = meta(&[("a", "1"), ("b", "2")]);
    svc.register_instance(NS, GROUP, SVC, original);

    assert!(patch(
        &svc,
        SVC,
        "10.0.0.1",
        8080,
        "DEFAULT",
        InstancePatch {
            weight: Some(2.5),
            enabled: Some(false),
            metadata: Some(meta(&[("b", "3"), ("c", "4")])),
            ..Default::default()
        },
    ));

    let got = query(&svc, SVC, "", false);
    assert_eq!(got.len(), 1);
    assert_eq!(
        got[0].metadata,
        meta(&[("a", "1"), ("b", "3"), ("c", "4")]),
        "'a' must survive, 'b' must be overridden, 'c' must be added"
    );
    assert_eq!(got[0].weight, 2.5);
    assert!(!got[0].enabled);
    // Fields the patch left as None keep their stored value.
    assert!(got[0].healthy);
    assert!(got[0].ephemeral);
}

#[test]
fn patch_on_missing_instance_fails() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    assert!(
        !patch(
            &svc,
            SVC,
            "10.0.0.9",
            8080,
            "DEFAULT",
            InstancePatch {
                weight: Some(2.0),
                ..Default::default()
            },
        ),
        "patching an unpublished instance must report failure (HTTP 404)"
    );
}

#[test]
fn apply_instance_patch_leaves_unspecified_fields_untouched() {
    let mut inst = instance("10.0.0.1", 8080);
    inst.metadata = meta(&[("keep", "yes")]);
    let before = inst.clone();

    apply_instance_patch(&mut inst, InstancePatch::default());

    assert_eq!(inst.weight, before.weight);
    assert_eq!(inst.healthy, before.healthy);
    assert_eq!(inst.enabled, before.enabled);
    assert_eq!(inst.ephemeral, before.ephemeral);
    assert_eq!(inst.metadata, before.metadata);
}

// ---------------------------------------------------------------------------
// 5) Multi-instance query
// ---------------------------------------------------------------------------

#[test]
fn cluster_filter_selects_only_matching_instances() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance_in_cluster("10.0.0.1", 8080, "c1"));
    svc.register_instance(NS, GROUP, SVC, instance_in_cluster("10.0.0.2", 8080, "c2"));

    assert_eq!(query(&svc, SVC, "", false).len(), 2);
    assert_eq!(
        endpoints(&query(&svc, SVC, "c1", false)),
        vec!["10.0.0.1:8080"]
    );
    assert_eq!(
        endpoints(&query(&svc, SVC, "c1,c2", false)),
        vec!["10.0.0.1:8080", "10.0.0.2:8080"],
        "a comma-separated cluster list is a union"
    );
    assert!(query(&svc, SVC, "nope", false).is_empty());
}

#[test]
fn weight_is_clamped_not_defaulted_at_zero() {
    // `MIN_WEIGHT_VALUE` is 0.0, so 0.0 is a legal weight (it means "never
    // route here"), while anything negative falls back to 1.0.
    let svc = service();
    let mut zero = instance("10.0.0.1", 8080);
    zero.weight = 0.0;
    let mut negative = instance("10.0.0.2", 8080);
    negative.weight = -3.0;
    svc.register_instance(NS, GROUP, SVC, zero);
    svc.register_instance(NS, GROUP, SVC, negative);

    let got = query(&svc, SVC, "", false);
    let zero_stored = got.iter().find(|i| i.ip == "10.0.0.1").unwrap();
    let negative_stored = got.iter().find(|i| i.ip == "10.0.0.2").unwrap();
    assert_eq!(zero_stored.weight, 0.0);
    assert_eq!(negative_stored.weight, 1.0);
}

#[test]
fn disabled_and_zero_weight_instances_are_still_returned() {
    // Deliberate upstream-parity assertion, NOT a "should be filtered" one:
    // Nacos `ServiceUtil.doSelectInstances` only drops disabled instances when
    // `enableOnly=true`, and no server query path passes it; weight is never a
    // filter either. Filtering here would break SDKs that disable an instance
    // and still expect to see it in the list.
    let svc = service();
    let mut disabled = instance("10.0.0.1", 8080);
    disabled.enabled = false;
    let mut zero_weight = instance("10.0.0.2", 8080);
    zero_weight.weight = 0.0;
    svc.register_instance(NS, GROUP, SVC, disabled);
    svc.register_instance(NS, GROUP, SVC, zero_weight);

    assert_eq!(
        endpoints(&query(&svc, SVC, "", false)),
        vec!["10.0.0.1:8080", "10.0.0.2:8080"]
    );
}

#[test]
fn healthy_only_is_orthogonal_to_enabled() {
    let svc = service();
    let mut disabled_but_healthy = instance("10.0.0.1", 8080);
    disabled_but_healthy.enabled = false;
    let mut unhealthy = instance("10.0.0.2", 8080);
    unhealthy.healthy = false;
    svc.register_instance(NS, GROUP, SVC, disabled_but_healthy);
    svc.register_instance(NS, GROUP, SVC, unhealthy);

    assert_eq!(
        endpoints(&query(&svc, SVC, "", true)),
        vec!["10.0.0.1:8080"],
        "healthyOnly filters on `healthy` only — disabled stays visible"
    );
}

#[test]
fn deregistering_one_of_many_leaves_the_rest() {
    let svc = service();
    for n in 1..=3 {
        svc.register_instance(NS, GROUP, SVC, instance(&format!("10.0.0.{n}"), 8080));
    }
    assert!(svc.deregister_instance(NS, GROUP, SVC, &instance("10.0.0.2", 8080)));

    assert_eq!(
        endpoints(&query(&svc, SVC, "", false)),
        vec!["10.0.0.1:8080", "10.0.0.3:8080"]
    );
}

/// Guard against a regression where `Instance::default()` was used as the
/// update payload: an unspecified cluster must still land in `DEFAULT`.
#[test]
fn update_keeps_the_same_storage_key() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    let anon = Instance::new("10.0.0.1".to_string(), 8080);
    svc.register_instance(NS, GROUP, SVC, anon);

    assert_eq!(svc.get_instance_count(NS, GROUP, SVC), 1);
    assert_eq!(query(&svc, SVC, "", false)[0].cluster_name, "DEFAULT");
}

#[tokio::test]
async fn ap_service_rejection_leaves_service_absent() {
    // A rejected write must not create bookkeeping side effects (service
    // entry, name index) that the empty-service reaper would then have to
    // clean up.
    let svc = Arc::new(service());
    assert!(!register_ap(&svc, SVC, persistent_instance("10.0.0.1", 8080)).await);
    assert_eq!(svc.service_count(), 0);
}
