//! Cluster and Raft initialization.
//!
//! Handles starting the cluster manager, discovering Raft members from cluster.conf,
//! waiting for peer readiness, and initializing the Raft cluster.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use batata_consistency::RaftNode;
use batata_core::cluster::ServerMemberManager;
use batata_server_common::model::config::Configuration;
use openraft::BasicNode;
use tracing::{error, info};

use super::{GrpcServers, raft_members};

/// Resolve which single node may call `Raft::initialize` on a cold start.
///
/// openraft's `initialize()` writes the initial membership *and immediately
/// campaigns*. When every member calls it, all of them campaign at the same
/// time and the winner depends on process scheduling. That makes the first
/// leader random, which in turn makes a cluster write benchmark measure two
/// different code paths (local `client_write` vs gRPC forwarding) from run to
/// run. Initializing exactly one node leaves the others to learn the
/// membership from its AppendEntries, so the designated node wins every time.
///
/// `configured < 0` returns `None` to restore the legacy all-initialize race
/// (break-glass only). `BTreeMap` ordering makes the automatic choice
/// independent of the order members were parsed from `cluster.conf`.
fn raft_bootstrap_leader_id(
    members: &BTreeMap<u64, BasicNode>,
    configured: i64,
) -> Result<Option<u64>, String> {
    if configured < 0 {
        return Ok(None);
    }
    if configured > 0 {
        let id = configured as u64;
        if !members.contains_key(&id) {
            return Err(format!(
                "raft bootstrap_leader={} is not a member of the Raft group (members: {:?})",
                configured,
                members.keys().collect::<Vec<_>>()
            ));
        }
        return Ok(Some(id));
    }
    Ok(members.keys().next().copied())
}

/// Only the designated bootstrap leader initializes; in legacy mode every
/// node does, preserving the old behaviour exactly.
fn should_initialize(self_id: u64, designated: Option<u64>) -> bool {
    designated.is_none_or(|id| self_id == id)
}

/// Start the cluster manager and initialize Raft if in cluster mode.
///
/// This is a no-op if the server is in standalone mode.
pub async fn init_cluster(
    configuration: &Configuration,
    server_member_manager: &Arc<ServerMemberManager>,
    raft_node: Option<&Arc<RaftNode>>,
    grpc_servers: &GrpcServers,
) -> Result<(), Box<dyn std::error::Error>> {
    let startup_mode = configuration.startup_mode();
    info!("Starting in {} mode", startup_mode);

    if configuration.is_standalone() {
        return Ok(());
    }

    // Wire the shared DistroProtocol to ServerMemberManager before starting
    // so it uses the same protocol instance as the gRPC handlers
    server_member_manager
        .set_distro_protocol(grpc_servers.distro_protocol().clone())
        .await;

    info!("Initializing cluster management...");
    if let Err(e) = server_member_manager.start().await {
        error!("Failed to start cluster manager: {}", e);
        return Err(e.to_string().into());
    }
    info!("Cluster management started successfully");

    // Initialize Raft cluster with discovered members (cluster mode — both
    // embedded and external-db)
    if let Some(raft_node) = raft_node {
        init_raft_cluster(configuration, raft_node).await?;
    }

    Ok(())
}

/// Build Raft member list from cluster.conf and initialize the cluster.
async fn init_raft_cluster(
    configuration: &Configuration,
    raft_node: &Arc<RaftNode>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "Initializing Raft cluster (self: node_id={}, addr={})",
        raft_node.node_id(),
        raft_node.addr()
    );

    // Build Raft membership directly from the shared member list (not the
    // SMM server_list, which may have duplicates when the local IP differs
    // from the configured entries). Every node parses the same explicit
    // `node_id=` values and reaches the same identity map.
    let members = raft_members::resolve_raft_members(configuration)?.by_id;
    for (node_id, node) in &members {
        info!("Raft member: node_id={}, addr={}", node_id, node.addr);
    }
    info!("Raft cluster: {} members from cluster.conf", members.len());

    // Wait for all peer Raft gRPC servers to be reachable before
    // initializing the cluster. This prevents premature leader
    // election when some peers haven't bound their ports yet.
    wait_for_raft_peers(
        raft_node,
        &members,
        configuration.raft_peer_connect_timeout_secs(),
        configuration.raft_peer_connect_retry_interval_ms(),
    )
    .await;

    let designated = raft_bootstrap_leader_id(&members, configuration.raft_bootstrap_leader())?;
    if !should_initialize(raft_node.node_id(), designated) {
        info!(
            "Raft bootstrap: node {} is not the designated leader (node {:?}); \
             skipping initialize and joining through the leader's AppendEntries",
            raft_node.node_id(),
            designated
        );
        return Ok(());
    }
    info!(
        "Raft bootstrap: node {} is the single designated initializer",
        raft_node.node_id()
    );

    if let Err(e) = raft_node.initialize(members).await {
        // Already initialized is OK (e.g. on restart)
        info!(
            "Raft cluster init result: {} (already initialized is OK)",
            e
        );
    } else {
        info!("Raft cluster initialized successfully");
    }

    Ok(())
}

/// Wait for peer Raft gRPC servers to become reachable before initialization.
async fn wait_for_raft_peers(
    raft_node: &RaftNode,
    members: &std::collections::BTreeMap<u64, openraft::BasicNode>,
    timeout_secs: u64,
    retry_interval_ms: u64,
) {
    let self_raft_addr = raft_node.addr().to_string();
    let peer_addrs: Vec<String> = members
        .values()
        .filter(|n| n.addr != self_raft_addr)
        .map(|n| n.addr.clone())
        .collect();

    if peer_addrs.is_empty() {
        return;
    }

    info!(
        "Waiting for {} Raft peer(s) to become reachable (timeout: {}s)...",
        peer_addrs.len(),
        timeout_secs
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);

    for addr in &peer_addrs {
        loop {
            match tokio::net::TcpStream::connect(addr).await {
                Ok(_) => {
                    info!("Raft peer {} is reachable", addr);
                    break;
                }
                Err(_) => {
                    if std::time::Instant::now() >= deadline {
                        tracing::warn!(
                            "Timeout waiting for Raft peer {} - proceeding anyway",
                            addr
                        );
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(retry_interval_ms)).await;
                }
            }
        }
    }
    info!("All Raft peers checked, proceeding with initialization");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn members(ids: &[u64]) -> BTreeMap<u64, BasicNode> {
        ids.iter()
            .map(|id| {
                (
                    *id,
                    BasicNode {
                        addr: format!("127.0.0.1:{}", 8848 + id * 10),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn auto_bootstrap_picks_the_lowest_member_id() {
        let group = members(&[1, 2, 3]);
        assert_eq!(raft_bootstrap_leader_id(&group, 0).unwrap(), Some(1));
    }

    #[test]
    fn auto_bootstrap_is_independent_of_parse_order() {
        let forward = members(&[1, 2, 3]);
        let reversed = members(&[3, 2, 1]);
        assert_eq!(
            raft_bootstrap_leader_id(&forward, 0).unwrap(),
            raft_bootstrap_leader_id(&reversed, 0).unwrap()
        );
    }

    #[test]
    fn auto_bootstrap_handles_non_contiguous_ids() {
        let group = members(&[7, 11, 42]);
        assert_eq!(raft_bootstrap_leader_id(&group, 0).unwrap(), Some(7));
    }

    #[test]
    fn explicit_bootstrap_leader_is_honoured() {
        let group = members(&[1, 2, 3]);
        assert_eq!(raft_bootstrap_leader_id(&group, 3).unwrap(), Some(3));
    }

    #[test]
    fn explicit_bootstrap_leader_outside_the_group_fails_closed() {
        let group = members(&[1, 2, 3]);
        let result = raft_bootstrap_leader_id(&group, 9);
        assert!(result.is_err(), "unknown id must fail closed: {result:?}");
        let error = result.unwrap_err();
        assert!(error.contains("bootstrap_leader=9"), "{error}");
        assert!(error.contains("not a member"), "{error}");
    }

    #[test]
    fn empty_membership_has_no_designated_leader() {
        let group = BTreeMap::new();
        assert_eq!(raft_bootstrap_leader_id(&group, 0).unwrap(), None);
    }

    #[test]
    fn negative_value_restores_legacy_all_initialize() {
        let group = members(&[1, 2, 3]);
        assert_eq!(raft_bootstrap_leader_id(&group, -1).unwrap(), None);
        assert!(should_initialize(1, None));
        assert!(should_initialize(2, None));
        assert!(should_initialize(3, None));
    }

    #[test]
    fn only_the_designated_node_initializes() {
        assert!(should_initialize(1, Some(1)));
        assert!(!should_initialize(2, Some(1)));
        assert!(!should_initialize(3, Some(1)));
        assert!(should_initialize(3, Some(3)));
        assert!(!should_initialize(1, Some(3)));
    }

    /// Static wiring guard: a future edit that calls `initialize()` on every
    /// node again reintroduces the election race. Keep the helper invocation
    /// pinned here so that change cannot land silently.
    #[test]
    fn init_raft_cluster_wires_deterministic_bootstrap() {
        let source = include_str!("cluster.rs");
        for token in [
            concat!("let designated = raft_bootstrap_leader_id", "("),
            concat!("configuration.raft_bootstrap_leader", "()"),
            concat!("!should_initialize(raft_node.node_id(), ", "designated)"),
            concat!(
                "skipping initialize and joining through the leader",
                "'s AppendEntries"
            ),
            concat!("raft_node.initialize", "(members).await"),
        ] {
            assert!(source.contains(token), "bootstrap wiring missing: {token}");
        }
    }
}
