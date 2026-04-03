//! Cluster and Raft initialization.
//!
//! Handles starting the cluster manager, discovering Raft members from cluster.conf,
//! waiting for peer readiness, and initializing the Raft cluster.

use std::sync::Arc;
use std::time::Duration;

use batata_consistency::RaftNode;
use batata_core::cluster::ServerMemberManager;
use batata_server_common::model::config::Configuration;
use tracing::{error, info};

use super::GrpcServers;

/// Start the cluster manager and initialize Raft if in distributed embedded mode.
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

    // Initialize Raft cluster with discovered members (distributed embedded mode)
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

    // Build Raft member list directly from cluster.conf (not the
    // SMM server_list which may have duplicates when the local IP
    // differs from cluster.conf entries). All nodes read the same
    // cluster.conf so they agree on the exact same member set.
    let cluster_addrs = configuration.cluster_member_addresses();
    let default_port = configuration.server_main_port();
    let raft_offset = batata_api::model::Member::DEFAULT_RAFT_OFFSET_PORT;
    let mut members = std::collections::BTreeMap::new();

    for addr_str in &cluster_addrs {
        let addr_part = addr_str.split('?').next().unwrap_or(addr_str);
        let (ip, main_port) = if let Some((ip, port_str)) = addr_part.rsplit_once(':') {
            (
                ip.to_string(),
                port_str.parse::<u16>().unwrap_or(default_port),
            )
        } else {
            (addr_part.to_string(), default_port)
        };

        // Check for explicit raft_port in query params
        let member_raft_port = addr_str
            .split('?')
            .nth(1)
            .and_then(|params| {
                params.split('&').find_map(|kv| {
                    let (k, v) = kv.split_once('=')?;
                    if k.trim() == "raft_port" {
                        v.trim().parse::<u16>().ok()
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| main_port - raft_offset);

        let raft_addr = format!("{}:{}", ip, member_raft_port);
        let node_id = batata_consistency::calculate_node_id(&raft_addr);
        info!("Raft member: node_id={}, addr={}", node_id, raft_addr);
        members.insert(node_id, openraft::BasicNode { addr: raft_addr });
    }

    info!("Raft cluster: {} members from cluster.conf", members.len());

    if members.is_empty() {
        error!("No cluster members in cluster.conf for Raft initialization");
        return Ok(());
    }

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
