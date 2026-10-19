//! Service-level ephemeral flag — Nacos `ServiceManager` singleton semantics.
//!
//! Upstream anchors:
//! - `com.alibaba.nacos.naming.core.v2.ServiceManager#getSingleton` uses
//!   `ConcurrentHashMap.computeIfAbsent`, and `Service#equals/hashCode` cover
//!   only (namespace, group, name) — so `ephemeral` is **not** part of the
//!   identity and is fixed by the *first* registration.
//! - `EphemeralClientOperationServiceImpl#registerInstance` throws
//!   `NacosRuntimeException("Current service %s is persistent service, can't
//!   register ephemeral instance.")`; `PersistentClientOperationServiceImpl`
//!   has the mirror message. Anchored by
//!   `EphemeralClientOperationServiceImplTest#testRegisterPersistentInstance`
//!   and `#testBatchRegisterPersistentInstance`.
//! - `ServiceOperatorV2Impl#delete` and `EmptyServiceAutoCleanerV2` call
//!   `removeSingleton`, so a recreated service re-fixes the flag.
//! - Deregistration is **not** gated upstream; it must stay ungated here.

mod common;

use std::sync::Arc;

use batata_api::naming::NamingServiceProvider;
use batata_naming::NamingService;
use batata_naming::service::{
    ClientOperationService, ClientOperationServiceProxy, EphemeralClientOperationService,
    PersistentClientOperationService, effective_update_ephemeral,
};
use common::{GROUP, NS, instance, persistent_instance, query, service};

const SVC: &str = "svc";

#[test]
fn flag_is_unresolved_before_any_client_write() {
    // A `bool` field cannot express this, which is why the resolved flag lives
    // in a keyed map rather than on `ServiceMetadata`.
    let svc = service();
    assert_eq!(svc.service_ephemeral(NS, GROUP, SVC), None);
}

#[test]
fn first_registration_fixes_the_flag_for_every_later_one() {
    // `computeIfAbsent` semantics: the second caller's own flag is ignored.
    let svc = service();
    assert!(svc.resolve_service_ephemeral(NS, GROUP, SVC, true));
    assert!(
        svc.resolve_service_ephemeral(NS, GROUP, SVC, false),
        "an ephemeral service stays ephemeral — the flag is not per-caller"
    );

    let other = service();
    assert!(!other.resolve_service_ephemeral(NS, GROUP, SVC, false));
    assert!(
        !other.resolve_service_ephemeral(NS, GROUP, SVC, true),
        "a persistent service stays persistent"
    );
}

#[tokio::test]
async fn ap_path_refuses_a_service_that_was_first_registered_persistent() {
    // Upstream `testRegisterPersistentInstance`.
    let svc = Arc::new(service());
    let proxy = ClientOperationServiceProxy::new(
        Arc::new(EphemeralClientOperationService::new(Arc::clone(&svc), None)),
        Arc::new(PersistentClientOperationService::new(
            Arc::clone(&svc),
            None,
        )),
    );

    // First registration is persistent → the service is persistent from now on.
    assert!(
        proxy
            .register_instance(NS, GROUP, SVC, persistent_instance("10.0.0.1", 8080))
            .await
    );

    assert!(
        !proxy
            .register_instance(NS, GROUP, SVC, instance("10.0.0.2", 8080))
            .await,
        "an ephemeral instance cannot join a persistent service"
    );
    assert_eq!(
        query(&svc, SVC, "", false).len(),
        1,
        "the rejected instance must not be stored"
    );
}

#[tokio::test]
async fn cp_path_refuses_a_service_that_was_first_registered_ephemeral() {
    // Mirror of the AP-path check.
    let svc = Arc::new(service());
    let proxy = ClientOperationServiceProxy::new(
        Arc::new(EphemeralClientOperationService::new(Arc::clone(&svc), None)),
        Arc::new(PersistentClientOperationService::new(
            Arc::clone(&svc),
            None,
        )),
    );

    assert!(
        proxy
            .register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080))
            .await
    );

    assert!(
        !proxy
            .register_instance(NS, GROUP, SVC, persistent_instance("10.0.0.2", 8080))
            .await,
        "a persistent instance cannot join an ephemeral service"
    );
    assert_eq!(query(&svc, SVC, "", false).len(), 1);
}

#[tokio::test]
async fn batch_registration_is_ephemeral_only() {
    // Upstream `BatchInstanceRequestHandler` builds the service with
    // `Service.newService(..., true)` and `batchRegisterInstance` throws for a
    // persistent singleton; `PersistentClientOperationServiceImpl
    // #batchRegisterInstance` is still an empty TODO. So for a persistent
    // service the batch gate must read `false`.
    let svc = service();
    svc.resolve_service_ephemeral(NS, GROUP, SVC, false);
    assert!(
        !svc.resolve_service_ephemeral(NS, GROUP, SVC, true),
        "batch register seeds the flag as ephemeral but must obey an existing persistent service"
    );
}

#[tokio::test]
async fn deregistration_is_not_gated() {
    // Neither upstream implementation checks the flag on deregister, so an
    // instance must always be removable through whatever path asks.
    let svc = Arc::new(service());
    let ephemeral_op = EphemeralClientOperationService::new(Arc::clone(&svc), None);
    let persistent_op = PersistentClientOperationService::new(Arc::clone(&svc), None);

    let inst = persistent_instance("10.0.0.1", 8080);
    svc.register_instance(NS, GROUP, SVC, inst.clone());

    assert!(
        ephemeral_op
            .deregister_instance(NS, GROUP, SVC, &inst)
            .await,
        "AP deregister must succeed even for a persistent instance"
    );
    assert!(query(&svc, SVC, "", false).is_empty());

    let inst = instance("10.0.0.2", 8080);
    svc.register_instance(NS, GROUP, SVC, inst.clone());
    assert!(
        persistent_op
            .deregister_instance(NS, GROUP, SVC, &inst)
            .await,
        "CP deregister must succeed even for an ephemeral instance"
    );
    assert!(query(&svc, SVC, "", false).is_empty());
}

#[test]
fn flag_is_released_when_the_empty_service_is_reaped() {
    // `EmptyServiceAutoCleanerV2` → `removeSingleton`: a recreated service
    // re-fixes the flag instead of inheriting the old one.
    let svc = service();
    svc.resolve_service_ephemeral(NS, GROUP, SVC, true);

    let inst = instance("10.0.0.1", 8080);
    svc.register_instance(NS, GROUP, SVC, inst.clone());
    svc.deregister_instance(NS, GROUP, SVC, &inst);
    assert!(svc.clean_empty_services(0) > 0);

    assert_eq!(svc.service_ephemeral(NS, GROUP, SVC), None);
    assert!(
        !svc.resolve_service_ephemeral(NS, GROUP, SVC, false),
        "the recreated service is free to be persistent again"
    );
}

#[test]
fn flag_is_released_when_the_service_is_deleted() {
    // `ServiceOperatorV2Impl#delete` → `removeSingleton`.
    let svc = service();
    svc.resolve_service_ephemeral(NS, GROUP, SVC, true);
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    svc.delete_service_metadata(NS, GROUP, SVC);

    assert_eq!(svc.service_ephemeral(NS, GROUP, SVC), None);
}

#[test]
fn update_inherits_the_stored_flag_when_the_form_omits_ephemeral() {
    // batata implements an update as a re-registration, so an omitted
    // `ephemeral` must not default to `true` — that would push a persistent
    // instance onto the Distro path and drop its Raft durability. Nacos'
    // `InstanceOperatorClientImpl#updateInstance` only rewrites metadata and
    // never re-routes the service, so inheriting is the faithful behaviour.
    let svc = Arc::new(service());
    svc.resolve_service_ephemeral(NS, GROUP, SVC, false);
    svc.register_instance(NS, GROUP, SVC, persistent_instance("10.0.0.1", 8080));
    let provider: Arc<dyn NamingServiceProvider> = svc.clone();

    let resolve = |requested: Option<bool>| {
        effective_update_ephemeral(
            &provider, NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT", requested,
        )
    };

    assert!(!resolve(None), "must inherit the stored persistent flag");
    assert!(
        resolve(Some(true)),
        "an explicit `ephemeral` still wins — the caller asked for the change"
    );
    // No stored instance and no resolved flag: fall back to Nacos' own default
    // (`Service.newService(ns, group, name)` ⇒ ephemeral = true).
    assert!(effective_update_ephemeral(
        &provider, NS, GROUP, "fresh", "10.0.0.9", 8080, "DEFAULT", None
    ));
}

#[test]
fn service_metadata_reports_the_resolved_flag() {
    // `ServiceMetadata::ephemeral` used to be dead and always `false`, which
    // described every ephemeral service as persistent.
    let svc: NamingService = service();
    svc.resolve_service_ephemeral(NS, GROUP, "ap", true);
    svc.register_instance(NS, GROUP, "ap", instance("10.0.0.1", 8080));
    svc.resolve_service_ephemeral(NS, GROUP, "cp", false);
    svc.register_instance(NS, GROUP, "cp", persistent_instance("10.0.0.2", 8080));

    assert!(svc.get_service_metadata(NS, GROUP, "ap").unwrap().ephemeral);
    assert!(!svc.get_service_metadata(NS, GROUP, "cp").unwrap().ephemeral);
}
