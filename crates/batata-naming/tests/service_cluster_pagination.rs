//! Service listing, cluster filtering and service-deletion boundaries.
//!
//! Upstream anchors:
//! - `ServiceUtil.pageServiceName` / `ServiceUtilTest.testPageServiceName` for
//!   pagination — notably `pageNo = 0` must return the **first** page, not an
//!   empty one.
//! - `ServiceUtil.selectInstances` / `checkCluster` for the `clusters` filter:
//!   a blank filter means "every cluster", otherwise the parameter is split on
//!   `,` and matched exactly.
//! - `ServiceOperatorV2Impl.delete` for the two deletion preconditions: the
//!   service must exist and must be empty.
//!
//! Two deliberate deviations are recorded in the test names:
//! - batata **sorts** service names before paginating (upstream paginates in
//!   the iteration order of the underlying set, which is not stable).
//! - batata accepts `"*"` as a "all clusters" wildcard and trims whitespace
//!   around comma-separated cluster names; upstream has neither.

mod common;

use std::collections::HashMap;

use batata_api::naming::{ClusterConfig, DeleteServiceError};
use common::{GROUP, NS, endpoints, instance, instance_in_cluster, query, service};

const SVC: &str = "svc";

// ---------------------------------------------------------------------------
// 1) Service listing pagination
// ---------------------------------------------------------------------------

/// Register services `svc-1 .. svc-N` (names are zero padded so that lexical
/// order matches numeric order) and return the service.
fn with_service_names(names: &[&str]) -> batata_naming::NamingService {
    let svc = service();
    for name in names {
        svc.register_instance(NS, GROUP, name, instance("10.0.0.1", 8080));
    }
    svc
}

#[test]
fn first_page_returns_the_first_slice() {
    let svc = with_service_names(&["svc-1", "svc-2", "svc-3"]);
    let (total, page) = svc.list_services(NS, GROUP, 1, 2);

    assert_eq!(total, 3, "total is the unpaginated service count");
    assert_eq!(page, vec!["svc-1", "svc-2"]);
}

#[test]
fn last_page_returns_only_the_remainder() {
    let svc = with_service_names(&["svc-1", "svc-2", "svc-3"]);
    let (_total, page) = svc.list_services(NS, GROUP, 2, 2);

    assert_eq!(page, vec!["svc-3"]);
}

#[test]
fn page_beyond_the_end_is_empty() {
    let svc = with_service_names(&["svc-1", "svc-2", "svc-3"]);
    let (total, page) = svc.list_services(NS, GROUP, 3, 2);

    assert_eq!(total, 3);
    assert!(
        page.is_empty(),
        "upstream `pageServiceName(3, 2, ...)` returns an empty list"
    );
}

/// Upstream `ServiceUtilTest#testPageServiceName` asserts
/// `pageServiceName(0, 1, services) == ["serviceA"]`. Before this was fixed,
/// batata computed `(0 - 1) * page_size` and cast the negative `i32` with
/// `as usize`, turning it into an enormous `skip` and returning nothing.
#[test]
fn page_no_zero_is_treated_as_the_first_page() {
    let svc = with_service_names(&["svc-1", "svc-2", "svc-3"]);
    let (_total, page) = svc.list_services(NS, GROUP, 0, 1);

    assert_eq!(page, vec!["svc-1"]);
}

#[test]
fn negative_page_no_is_treated_as_the_first_page() {
    let svc = with_service_names(&["svc-1", "svc-2", "svc-3"]);
    let (_total, page) = svc.list_services(NS, GROUP, -5, 2);

    assert_eq!(page, vec!["svc-1", "svc-2"]);
}

#[test]
fn zero_page_size_returns_no_rows_but_keeps_the_total() {
    let svc = with_service_names(&["svc-1", "svc-2"]);
    let (total, page) = svc.list_services(NS, GROUP, 1, 0);

    assert_eq!(total, 2);
    assert!(page.is_empty(), "a zero-sized page holds no names");
}

#[test]
fn negative_page_size_never_overflows_the_skip() {
    let svc = with_service_names(&["svc-1", "svc-2"]);
    let (total, page) = svc.list_services(NS, GROUP, 2, -10);

    assert_eq!(total, 2);
    assert!(page.is_empty());
}

/// `i32::MAX` is what the v3 admin list passes to fetch everything in one
/// page; the offset must saturate instead of wrapping.
#[test]
fn huge_page_size_returns_everything() {
    let svc = with_service_names(&["svc-1", "svc-2", "svc-3"]);
    let (total, page) = svc.list_services(NS, GROUP, 1, i32::MAX);

    assert_eq!(total, 3);
    assert_eq!(page, vec!["svc-1", "svc-2", "svc-3"]);
}

#[test]
fn names_are_sorted_so_pagination_is_stable() {
    // Deliberate deviation: upstream paginates in set-iteration order.
    let svc = with_service_names(&["zeta", "alpha", "mid"]);
    let (_total, page) = svc.list_services(NS, GROUP, 1, 10);

    assert_eq!(page, vec!["alpha", "mid", "zeta"]);
}

#[test]
fn group_and_namespace_scope_the_listing() {
    let svc = service();
    svc.register_instance(NS, GROUP, "in-default", instance("10.0.0.1", 8080));
    svc.register_instance(NS, "OTHER_GROUP", "in-other", instance("10.0.0.2", 8080));
    svc.register_instance(
        "other-ns",
        GROUP,
        "other-ns-svc",
        instance("10.0.0.3", 8080),
    );

    let (total_default, page_default) = svc.list_services(NS, GROUP, 1, 10);
    assert_eq!(total_default, 1);
    assert_eq!(page_default, vec!["in-default"]);

    // An empty group name means "every group in this namespace".
    let (total_all, mut page_all) = svc.list_services(NS, "", 1, 10);
    assert_eq!(total_all, 2);
    page_all.sort();
    assert_eq!(page_all, vec!["in-default", "in-other"]);

    let (total_ns, page_ns) = svc.list_services("other-ns", GROUP, 1, 10);
    assert_eq!(total_ns, 1);
    assert_eq!(page_ns, vec!["other-ns-svc"]);
}

#[test]
fn a_service_without_instances_is_still_listed() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));
    svc.deregister_instance(NS, GROUP, SVC, &instance("10.0.0.1", 8080));

    // Nacos keeps the empty shell until `EmptyServiceAutoCleanerV2` reclaims it.
    assert!(svc.service_exists(NS, GROUP, SVC));
    let (total, page) = svc.list_services(NS, GROUP, 1, 10);
    assert_eq!(total, 1);
    assert_eq!(page, vec![SVC]);
}

#[test]
fn unknown_namespace_lists_nothing() {
    let svc = with_service_names(&["svc-1"]);
    let (total, page) = svc.list_services("nope", GROUP, 1, 10);

    assert_eq!(total, 0);
    assert!(page.is_empty());
}

// ---------------------------------------------------------------------------
// 2) Cluster filtering
// ---------------------------------------------------------------------------

fn multi_cluster_service() -> batata_naming::NamingService {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance_in_cluster("10.0.0.1", 8080, "A"));
    svc.register_instance(NS, GROUP, SVC, instance_in_cluster("10.0.0.2", 8080, "B"));
    svc.register_instance(NS, GROUP, SVC, instance_in_cluster("10.0.0.3", 8080, "C"));
    svc
}

#[test]
fn empty_cluster_filter_returns_every_cluster() {
    let svc = multi_cluster_service();

    // Upstream `checkCluster`: an empty cluster set matches everything.
    assert_eq!(
        endpoints(&query(&svc, SVC, "", false)),
        vec!["10.0.0.1:8080", "10.0.0.2:8080", "10.0.0.3:8080"]
    );
}

#[test]
fn comma_separated_cluster_filter_is_a_union() {
    let svc = multi_cluster_service();

    assert_eq!(
        endpoints(&query(&svc, SVC, "A,C", false)),
        vec!["10.0.0.1:8080", "10.0.0.3:8080"]
    );
}

#[test]
fn a_single_cluster_name_filters_to_that_cluster() {
    let svc = multi_cluster_service();

    assert_eq!(
        endpoints(&query(&svc, SVC, "B", false)),
        vec!["10.0.0.2:8080"]
    );
}

#[test]
fn unknown_cluster_name_returns_nothing() {
    let svc = multi_cluster_service();

    assert!(query(&svc, SVC, "NOPE", false).is_empty());
    // A partially unknown list still yields the clusters that do exist.
    assert_eq!(
        endpoints(&query(&svc, SVC, "A,NOPE", false)),
        vec!["10.0.0.1:8080"]
    );
}

#[test]
fn cluster_filter_is_not_a_prefix_match() {
    let svc = multi_cluster_service();

    assert!(
        query(&svc, SVC, "AB", false).is_empty(),
        "upstream matches cluster names exactly"
    );
}

/// Deviation: batata trims whitespace around each comma-separated name, which
/// upstream does not do (`"A, B"` becomes `{"A", " B"}` there).
#[test]
fn whitespace_around_cluster_names_is_trimmed() {
    let svc = multi_cluster_service();

    assert_eq!(
        endpoints(&query(&svc, SVC, "A, B", false)),
        vec!["10.0.0.1:8080", "10.0.0.2:8080"]
    );
}

/// Deviation: `"*"` is a batata-only "all clusters" wildcard; upstream would
/// treat it as a literal cluster name.
#[test]
fn star_is_a_wildcard_for_all_clusters() {
    let svc = multi_cluster_service();

    assert_eq!(
        endpoints(&query(&svc, SVC, "*", false)),
        vec!["10.0.0.1:8080", "10.0.0.2:8080", "10.0.0.3:8080"]
    );
}

#[test]
fn cluster_filter_and_healthy_only_compose() {
    let svc = multi_cluster_service();
    let unhealthy = instance_in_cluster("10.0.0.4", 8080, "D");
    svc.register_instance(NS, GROUP, SVC, unhealthy.clone());
    svc.update_instance_health(NS, GROUP, SVC, "10.0.0.4", 8080, "D", false);

    assert!(
        query(&svc, SVC, "D", true).is_empty(),
        "an unhealthy instance drops out of a healthy-only query"
    );
    assert_eq!(
        endpoints(&query(&svc, SVC, "D", false)),
        vec!["10.0.0.4:8080"],
        "but is still returned when health is not filtered"
    );
}

// ---------------------------------------------------------------------------
// 3) Deleting a service
// ---------------------------------------------------------------------------

#[test]
fn deleting_an_unknown_service_is_not_found() {
    let svc = service();

    assert_eq!(
        svc.delete_service(NS, GROUP, "nope"),
        Err(DeleteServiceError::NotFound)
    );
}

#[test]
fn deleting_a_non_empty_service_is_rejected() {
    let svc = with_service_names(&[SVC]);

    assert_eq!(
        svc.delete_service(NS, GROUP, SVC),
        Err(DeleteServiceError::NotEmpty { count: 1 }),
        "upstream refuses: \"Service ... is not empty, can't be delete\""
    );
    // Rejection must be side-effect free.
    assert!(svc.service_exists(NS, GROUP, SVC));
    assert_eq!(svc.get_instance_count(NS, GROUP, SVC), 1);
}

#[test]
fn an_emptied_service_can_be_deleted() {
    let svc = with_service_names(&[SVC]);
    svc.deregister_instance(NS, GROUP, SVC, &instance("10.0.0.1", 8080));

    assert_eq!(svc.delete_service(NS, GROUP, SVC), Ok(()));
    assert!(!svc.service_exists(NS, GROUP, SVC));
    let (total, page) = svc.list_services(NS, GROUP, 1, 10);
    assert_eq!(total, 0);
    assert!(page.is_empty());
}

#[test]
fn deleting_twice_reports_not_found_the_second_time() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));
    svc.deregister_instance(NS, GROUP, SVC, &instance("10.0.0.1", 8080));

    assert_eq!(svc.delete_service(NS, GROUP, SVC), Ok(()));
    assert_eq!(
        svc.delete_service(NS, GROUP, SVC),
        Err(DeleteServiceError::NotFound)
    );
}

/// `ServiceOperatorV2Impl#delete` drops the singleton, which is what lets a
/// re-created service be re-pinned to a different ephemeral flag.
#[test]
fn deleting_releases_the_resolved_service_ephemeral_flag() {
    let svc = service();
    svc.register_instance(NS, GROUP, SVC, instance("10.0.0.1", 8080));

    // First sighting pins the flag; later callers must obey it, exactly as
    // `Service.equals`/`hashCode` ignoring `ephemeral` makes
    // `ServiceManager.getSingleton` return the original instance.
    assert!(svc.resolve_service_ephemeral(NS, GROUP, SVC, true));
    assert!(
        svc.resolve_service_ephemeral(NS, GROUP, SVC, false),
        "the first writer wins: the caller's own flag is ignored"
    );

    svc.deregister_instance(NS, GROUP, SVC, &instance("10.0.0.1", 8080));
    assert_eq!(svc.delete_service(NS, GROUP, SVC), Ok(()));
    assert_eq!(svc.service_ephemeral(NS, GROUP, SVC), None);

    // The next registration is free to pin the opposite flag.
    assert!(!svc.resolve_service_ephemeral(NS, GROUP, SVC, false));
}

/// Instances in *any* cluster block the delete — upstream checks
/// `getPushData(service).getHosts()`, which spans all clusters.
#[test]
fn instances_in_any_cluster_block_the_delete() {
    let svc = multi_cluster_service();

    assert_eq!(
        svc.delete_service(NS, GROUP, SVC),
        Err(DeleteServiceError::NotEmpty { count: 3 })
    );
}

// ---------------------------------------------------------------------------
// 4) Cluster configuration
// ---------------------------------------------------------------------------

fn cluster_config(name: &str) -> ClusterConfig {
    ClusterConfig {
        name: name.to_string(),
        ..Default::default()
    }
}

#[test]
fn creating_a_cluster_for_an_unknown_service_fails() {
    let svc = service();

    let err = svc
        .create_cluster_config(NS, GROUP, SVC, "A", "tcp", 80, true, HashMap::new())
        .unwrap_err();
    assert!(err.contains("not found"), "unexpected error: {err}");
}

#[test]
fn creating_the_same_cluster_twice_fails() {
    let svc = multi_cluster_service();
    assert!(
        svc.create_cluster_config(NS, GROUP, SVC, "A", "tcp", 80, true, HashMap::new())
            .is_ok()
    );

    let err = svc
        .create_cluster_config(NS, GROUP, SVC, "A", "http", 8080, false, HashMap::new())
        .unwrap_err();
    assert!(err.contains("already exists"), "unexpected error: {err}");
}

#[test]
fn cluster_config_round_trips_and_upper_cases_the_checker() {
    let svc = multi_cluster_service();
    svc.create_cluster_config(
        NS,
        GROUP,
        SVC,
        "A",
        "http",
        8080,
        false,
        HashMap::from([("zone".to_string(), "z1".to_string())]),
    )
    .expect("cluster creation succeeds for an existing service");

    let cfg = svc
        .get_cluster_config(NS, GROUP, SVC, "A")
        .expect("cluster config is stored");
    assert_eq!(cfg.name, "A");
    assert_eq!(cfg.health_check_type, "HTTP");
    assert_eq!(cfg.check_port, 8080);
    assert!(!cfg.use_instance_port);
    assert_eq!(cfg.metadata.get("zone").map(String::as_str), Some("z1"));
}

#[test]
fn cluster_configs_are_scoped_to_their_service() {
    let svc = multi_cluster_service();
    svc.register_instance(NS, GROUP, "other", instance_in_cluster("10.0.0.9", 80, "Z"));
    svc.set_cluster_config(NS, GROUP, SVC, "A", cluster_config("A"));
    svc.set_cluster_config(NS, GROUP, "other", "Z", cluster_config("Z"));

    let names: Vec<String> = svc
        .get_all_cluster_configs(NS, GROUP, SVC)
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(
        names,
        vec!["A"],
        "the `svc##` prefix must not leak into `other`"
    );
}

#[test]
fn deleting_a_cluster_config_leaves_the_others_alone() {
    let svc = multi_cluster_service();
    svc.set_cluster_config(NS, GROUP, SVC, "A", cluster_config("A"));
    svc.set_cluster_config(NS, GROUP, SVC, "B", cluster_config("B"));

    svc.delete_cluster_config(NS, GROUP, SVC, "A");

    assert!(svc.get_cluster_config(NS, GROUP, SVC, "A").is_none());
    assert!(svc.get_cluster_config(NS, GROUP, SVC, "B").is_some());
}

#[test]
fn cluster_statistics_group_instances_by_cluster() {
    let svc = multi_cluster_service();
    svc.register_instance(NS, GROUP, SVC, instance_in_cluster("10.0.0.4", 8080, "A"));
    svc.update_instance_health(NS, GROUP, SVC, "10.0.0.4", 8080, "A", false);

    let mut stats = svc.get_cluster_statistics(NS, GROUP, SVC);
    stats.sort_by(|a, b| a.cluster_name.cmp(&b.cluster_name));

    assert_eq!(stats.len(), 3, "one entry per cluster");
    let a = &stats[0];
    assert_eq!(a.cluster_name, "A");
    assert_eq!(a.total_instances, 2);
    assert_eq!(a.healthy_instances, 1);
    assert_eq!(a.unhealthy_instances, 1);
    assert_eq!(stats[1].cluster_name, "B");
    assert_eq!(stats[1].total_instances, 1);
    assert_eq!(stats[2].cluster_name, "C");
}

#[test]
fn cluster_statistics_for_an_unknown_cluster_is_none() {
    let svc = multi_cluster_service();

    assert!(
        svc.get_single_cluster_statistics(NS, GROUP, SVC, "NOPE")
            .is_none()
    );
    assert!(
        svc.get_cluster_statistics(NS, GROUP, "unknown-svc")
            .is_empty()
    );
}
