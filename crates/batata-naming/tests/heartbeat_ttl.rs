//! Heartbeat and TTL semantics.
//!
//! Upstream anchors:
//! - `ClientBeatCheckTaskV2` / `ClientBeatCheckTaskV2Test`: an ephemeral
//!   instance that stops beating is first marked *unhealthy*
//!   (`UnhealthyInstanceChecker`) and only later deleted
//!   (`ExpiredInstanceChecker`); `ipDeleteTimeout` > `heartbeatTimeout` so the
//!   two never fire in the same pass.
//! - `UnhealthyInstanceChecker.doCheck()` only acts on instances that are
//!   *still* healthy, so a long-dead instance is not re-marked — and not
//!   re-published to Distro — on every interval.
//! - Persistent instances are not subject to heartbeat expiry: they are probed
//!   by server-side checks instead.
//! - Consul's TTL checks (`/v1/agent/check/pass/:id`) are renewed by touching
//!   `last_updated`; `TtlMonitor` derives expiry from that timestamp alone, so
//!   a renewal that carries the *same* status must still refresh it.
//!
//! The two checkers here are driven through their real `start()` loop with a
//! 1s interval. Timeouts are set to 0 so the outcome is decided by "any
//! elapsed time at all" rather than by a race against the clock.

mod common;

use std::sync::Arc;
use std::time::Duration;

use batata_naming::NamingService;
use batata_naming::healthcheck::config::HealthCheckConfig;
use batata_naming::healthcheck::interceptor::HealthCheckInterceptorChain;
use batata_naming::healthcheck::registry::{
    CheckStatus, CheckType, InstanceCheckConfig, InstanceCheckRegistry,
};
use batata_naming::healthcheck::ttl_processor::TtlHealthCheckProcessor;
use batata_naming::healthcheck::{ExpiredInstanceChecker, UnhealthyInstanceChecker};
use common::{GROUP, NS, instance, persistent_instance, service};

const SVC: &str = "svc";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn check_config(
    id: &str,
    node: &str,
    kind: CheckType,
    ttl: Option<Duration>,
) -> InstanceCheckConfig {
    InstanceCheckConfig {
        check_id: id.to_string(),
        name: format!("check {id}"),
        check_type: kind,
        namespace: NS.to_string(),
        group_name: GROUP.to_string(),
        service_name: SVC.to_string(),
        ip: "10.0.0.1".to_string(),
        port: 8080,
        cluster_name: "DEFAULT".to_string(),
        http_url: None,
        tcp_addr: None,
        grpc_addr: None,
        db_url: None,
        script: None,
        args: None,
        docker_container_id: None,
        interval: Duration::from_secs(10),
        timeout: Duration::from_secs(5),
        ttl,
        success_before_passing: 0,
        failures_before_critical: 0,
        deregister_critical_after: None,
        initial_status: CheckStatus::Passing,
        notes: String::new(),
        service_tags: vec![],
        node: node.to_string(),
    }
}

fn registry() -> Arc<InstanceCheckRegistry> {
    let naming: Arc<dyn batata_api::naming::NamingServiceProvider> = Arc::new(service());
    Arc::new(InstanceCheckRegistry::with_naming_service(naming))
}

fn config_1s() -> Arc<HealthCheckConfig> {
    let mut c = HealthCheckConfig::default();
    c.heartbeat_interval_secs = 1;
    c.health_check_enabled = true;
    Arc::new(c)
}

fn chain() -> Arc<HealthCheckInterceptorChain> {
    Arc::new(HealthCheckInterceptorChain::standalone(Arc::new(
        HealthCheckConfig::default(),
    )))
}

/// Run `checker.start()` in the background for `settle` ms, then abort it.
///
/// The checkers loop forever, so the tests drive them for one or two intervals
/// and then stop. `settle` is generous relative to the 1s interval while the
/// timeouts under test are 0, so nothing here races the clock.
async fn drive_for<T: Send + Sync + 'static>(checker: Arc<T>, settle: u64)
where
    T: CheckerLoop,
{
    let handle = tokio::spawn(async move { checker.start().await });
    tokio::time::sleep(Duration::from_millis(settle)).await;
    handle.abort();
}

/// Both checkers expose the same (endless) `start()` loop.
trait CheckerLoop: Send + Sync {
    fn start(&self) -> impl std::future::Future<Output = ()> + Send;
}
impl CheckerLoop for UnhealthyInstanceChecker {
    fn start(&self) -> impl std::future::Future<Output = ()> + Send {
        UnhealthyInstanceChecker::start(self)
    }
}
impl CheckerLoop for ExpiredInstanceChecker {
    fn start(&self) -> impl std::future::Future<Output = ()> + Send {
        ExpiredInstanceChecker::start(self)
    }
}

/// Seed the shared heartbeat map the way a client beat would.
fn record(
    unhealthy: &UnhealthyInstanceChecker,
    ip: &str,
    heartbeat_timeout: i64,
    ip_delete_timeout: i64,
    ephemeral: bool,
) {
    unhealthy.record_heartbeat(
        NS,
        GROUP,
        SVC,
        ip,
        8080,
        "DEFAULT",
        heartbeat_timeout,
        ip_delete_timeout,
        ephemeral,
    );
}

/// An `ExpiredInstanceChecker` over the beats recorded by `unhealthy` — the two
/// share one map in production too.
fn expired_over(
    svc: &Arc<NamingService>,
    unhealthy: &UnhealthyInstanceChecker,
) -> Arc<ExpiredInstanceChecker> {
    Arc::new(ExpiredInstanceChecker::new(
        Arc::clone(svc),
        config_1s(),
        true,
        Arc::clone(&unhealthy.heartbeat_map),
        chain(),
    ))
}

fn instances(svc: &NamingService) -> Vec<batata_api::naming::model::Instance> {
    svc.get_instances_snapshot(NS, GROUP, SVC, "", false)
        .into_iter()
        .map(|arc| (*arc).clone())
        .collect()
}

fn version(svc: &NamingService) -> i64 {
    svc.get_distro_version(&batata_naming::service::build_service_key(NS, GROUP, SVC))
}

// ---------------------------------------------------------------------------
// 1) `NamingService::heartbeat`
// ---------------------------------------------------------------------------

#[test]
fn heartbeat_revives_an_unhealthy_instance() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));
    assert!(svc.update_instance_health(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT", false));

    assert!(svc.heartbeat(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT"));
    assert!(instances(&svc)[0].healthy);
}

#[test]
fn heartbeat_does_not_create_a_missing_instance() {
    let svc = service();

    assert!(!svc.heartbeat(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT"));
    assert_eq!(svc.get_instance_count(NS, GROUP, SVC), 0);
}

#[test]
fn heartbeat_leaves_a_healthy_instance_untouched() {
    let svc = service();
    let mut inst = instance("10.0.0.1", 8080);
    inst.weight = 3.0;
    inst.metadata.insert("k".to_string(), "v".to_string());
    svc.register_instance(NS, GROUP, SVC, inst);

    assert!(svc.heartbeat(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT"));

    let all = instances(&svc);
    assert_eq!(all[0].weight, 3.0, "a beat must not rewrite the instance");
    assert_eq!(all[0].metadata.get("k").map(String::as_str), Some("v"));
}

/// `update_instance_health` advances the distro version (a health flip is a
/// local mutation peers must hear about), but a plain heartbeat that changes
/// nothing must not — otherwise every beat in the cluster would broadcast.
#[test]
fn heartbeat_does_not_advance_the_distro_version() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));
    let after_register = version(&svc);

    assert!(svc.heartbeat(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT"));
    assert_eq!(version(&svc), after_register);

    // Reviving an unhealthy instance *is* a state change, so it is published.
    assert!(svc.update_instance_health(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT", false));
    let after_flip = version(&svc);
    assert!(svc.heartbeat(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT"));
    assert!(after_flip > after_register, "the flip itself was published");
    assert_eq!(
        version(&svc),
        after_flip,
        "the beat that revived it is not published again"
    );
}

#[test]
fn heartbeat_targets_only_the_named_instance() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.2", 8080));

    assert!(svc.heartbeat(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT"));

    let all = instances(&svc);
    let other = all.iter().find(|i| i.ip == "10.0.0.2").expect("10.0.0.2");
    assert!(other.healthy, "a beat never touches a sibling instance");
}

/// The lookup key is `ip#port#cluster`; a blank cluster does not fall back to
/// `DEFAULT` here (unlike `deregister_instance`, which does).
#[test]
fn heartbeat_with_a_blank_cluster_matches_nothing() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    assert!(!svc.heartbeat(NS, GROUP, SVC, "10.0.0.1", 8080, ""));
}

// ---------------------------------------------------------------------------
// 2) TTL checks
// ---------------------------------------------------------------------------

/// The documented exception to edge-triggered de-duplication: `TtlMonitor`
/// decides expiry purely from `last_updated`, so a renewal carrying the *same*
/// status still has to refresh the timestamp or every other node would see the
/// check as expired.
#[tokio::test]
async fn ttl_update_refreshes_last_updated_even_when_the_status_is_unchanged() {
    let reg = registry();
    let key = reg.register_check(check_config(
        "ttl",
        "",
        CheckType::Ttl,
        Some(Duration::from_secs(30)),
    ));

    let before = reg.get_check(&key).expect("registered").1.last_updated;
    tokio::time::sleep(Duration::from_millis(15)).await;
    reg.ttl_update(&key, CheckStatus::Passing, Some("still alive".to_string()))
        .await;

    let (_, status) = reg.get_check(&key).expect("registered");
    assert_eq!(status.status, CheckStatus::Passing);
    assert!(
        status.last_updated > before,
        "a renewal must move `last_updated` ({} !> {})",
        status.last_updated,
        before
    );
    assert_eq!(status.output, "still alive");
}

#[tokio::test]
async fn ttl_update_on_an_unknown_check_is_a_no_op() {
    let reg = registry();
    reg.ttl_update("nope", CheckStatus::Critical, None).await;
    assert_eq!(reg.get_all_checks().len(), 0);
}

/// TTL has no success/failure thresholds — the status changes immediately.
#[tokio::test]
async fn ttl_update_applies_the_status_immediately() {
    let reg = registry();
    let key = reg.register_check(check_config(
        "ttl",
        "",
        CheckType::Ttl,
        Some(Duration::from_secs(30)),
    ));

    reg.ttl_update(&key, CheckStatus::Critical, Some("gone".to_string()))
        .await;

    let (_, status) = reg.get_check(&key).expect("registered");
    assert_eq!(status.status, CheckStatus::Critical);
    assert_eq!(status.output, "gone");
}

/// Expiry is measured from `last_updated`, never from a status: a passing check
/// whose beat went stale is expired just the same.
///
/// The comparison is `now - last_updated > ttl`, strictly — so within the same
/// millisecond even a zero TTL is not yet expired.
#[test]
fn ttl_expiration_is_measured_from_last_updated() {
    let reg = registry();
    let key = reg.register_check(check_config(
        "ttl",
        "",
        CheckType::Ttl,
        Some(Duration::from_secs(30)),
    ));
    let (_, status) = reg.get_check(&key).expect("registered");
    assert_eq!(status.status, CheckStatus::Passing);

    assert!(
        !TtlHealthCheckProcessor::check_expiration(&status, Duration::from_secs(3600)),
        "a freshly renewed check is not expired"
    );
    let elapsed = elapsed_ms(&status);
    assert_eq!(
        TtlHealthCheckProcessor::check_expiration(&status, Duration::from_millis(0)),
        elapsed > 0,
        "a zero TTL expires as soon as (and only when) time has elapsed"
    );

    std::thread::sleep(Duration::from_millis(5));
    assert!(
        TtlHealthCheckProcessor::check_expiration(&status, Duration::from_millis(0)),
        "any elapsed time at all outlives a zero TTL"
    );
}

fn elapsed_ms(status: &batata_naming::healthcheck::registry::InstanceCheckStatus) -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
        - status.last_updated
}

// ---------------------------------------------------------------------------
// 3) Check identity is (node, check_id)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_same_check_id_on_two_nodes_is_independent() {
    let reg = registry();
    let k1 = reg.register_check(check_config(
        "serfHealth",
        "c1",
        CheckType::Ttl,
        Some(Duration::from_secs(30)),
    ));
    let k2 = reg.register_check(check_config(
        "serfHealth",
        "c2",
        CheckType::Ttl,
        Some(Duration::from_secs(30)),
    ));
    assert_ne!(k1, k2);
    assert_eq!(reg.get_all_checks().len(), 2);

    reg.ttl_update(&k1, CheckStatus::Critical, None).await;

    assert_eq!(reg.get_check(&k1).unwrap().1.status, CheckStatus::Critical);
    assert_eq!(
        reg.get_check(&k2).unwrap().1.status,
        CheckStatus::Passing,
        "one node's check must not be overwritten by another's"
    );
}

#[test]
fn a_node_qualified_check_is_reachable_only_by_its_registry_key() {
    let reg = registry();
    let key = reg.register_check(check_config("serfHealth", "c1", CheckType::Tcp, None));

    assert_eq!(key, "c1#serfHealth");
    assert!(reg.get_check(&key).is_some());
    assert!(
        reg.get_check("serfHealth").is_none(),
        "the bare id must not resolve once the check carries a node"
    );
}

/// Nacos-side checks have no node, so the key stays the bare check id — the
/// node dimension must remain a no-op for them.
#[test]
fn a_check_without_a_node_keeps_the_bare_id_as_its_key() {
    let reg = registry();
    let key = reg.register_check(check_config("tcp", "", CheckType::Tcp, None));

    assert_eq!(key, "tcp");
    assert!(reg.get_check("tcp").is_some());
}

// ---------------------------------------------------------------------------
// 4) Beat-driven checkers (real loop, 1s interval)
// ---------------------------------------------------------------------------

/// `ClientBeatCheckTaskV2Test#testRunUnhealthyInstanceWithoutExpire`: an
/// instance that stopped beating is marked unhealthy, and stays registered.
#[tokio::test]
async fn unhealthy_checker_marks_a_timed_out_instance_unhealthy() {
    let svc = Arc::new(service());
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    let checker = Arc::new(UnhealthyInstanceChecker::new(
        Arc::clone(&svc),
        config_1s(),
        chain(),
    ));
    record(&checker, "10.0.0.1", 0, 30_000, true);

    drive_for(Arc::clone(&checker), 1_400).await;

    assert!(
        !instances(&svc)[0].healthy,
        "a beat older than `heartbeatTimeout` marks the instance unhealthy"
    );
    assert_eq!(svc.get_instance_count(NS, GROUP, SVC), 1, "and keeps it");
}

/// And it must not re-mark (and re-publish) an instance that is already
/// unhealthy — upstream guards on `instance.isHealthy()`.
#[tokio::test]
async fn unhealthy_checker_does_not_republish_an_already_unhealthy_instance() {
    let svc = Arc::new(service());
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));
    assert!(svc.update_instance_health(NS, GROUP, SVC, "10.0.0.1", 8080, "DEFAULT", false));
    let after_flip = version(&svc);

    let checker = Arc::new(UnhealthyInstanceChecker::new(
        Arc::clone(&svc),
        config_1s(),
        chain(),
    ));
    record(&checker, "10.0.0.1", 0, 30_000, true);

    // Two passes: without the guard each one would publish again.
    drive_for(Arc::clone(&checker), 2_400).await;

    assert_eq!(
        version(&svc),
        after_flip,
        "an already-unhealthy instance must not be re-published every interval"
    );
}

#[tokio::test]
async fn unhealthy_checker_leaves_a_fresh_instance_healthy() {
    let svc = Arc::new(service());
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    let checker = Arc::new(UnhealthyInstanceChecker::new(
        Arc::clone(&svc),
        config_1s(),
        chain(),
    ));
    record(&checker, "10.0.0.1", 60_000, 120_000, true);

    drive_for(Arc::clone(&checker), 1_400).await;

    assert!(instances(&svc)[0].healthy);
}

/// `ExpiredInstanceChecker` is the second stage: past `ipDeleteTimeout` the
/// ephemeral instance is removed outright.
#[tokio::test]
async fn expired_checker_deletes_an_expired_ephemeral_instance() {
    let svc = Arc::new(service());
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    let beats = Arc::new(UnhealthyInstanceChecker::new(
        Arc::clone(&svc),
        config_1s(),
        chain(),
    ));
    record(&beats, "10.0.0.1", 0, 0, true);
    let checker = expired_over(&svc, &beats);

    drive_for(checker, 1_400).await;

    assert_eq!(
        svc.get_instance_count(NS, GROUP, SVC),
        0,
        "past `ipDeleteTimeout` the ephemeral instance is deleted"
    );
}

/// Persistent instances are probed by server-side checks; a missing client beat
/// must never delete them.
#[tokio::test]
async fn expired_checker_never_deletes_a_persistent_instance() {
    let svc = Arc::new(service());
    svc.register_instance(NS, GROUP, SVC, persistent_instance("10.0.0.1", 8080));

    let beats = Arc::new(UnhealthyInstanceChecker::new(
        Arc::clone(&svc),
        config_1s(),
        chain(),
    ));
    record(&beats, "10.0.0.1", 0, 0, false);
    let checker = expired_over(&svc, &beats);

    drive_for(checker, 1_400).await;

    assert_eq!(svc.get_instance_count(NS, GROUP, SVC), 1);
}
