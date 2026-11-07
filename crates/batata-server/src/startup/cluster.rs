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

    // Build Raft member list directly from cluster.conf (not the
    // SMM server_list which may have duplicates when the local IP
    // differs from cluster.conf entries). All nodes read the same
    // cluster.conf so they agree on the exact same member set.
    let cluster_addrs = configuration.cluster_member_addresses();
    let default_port = configuration.server_main_port();
    let raft_offset = batata_api::model::Member::DEFAULT_RAFT_OFFSET_PORT;
    let mut members = std::collections::BTreeMap::new();

    for addr_str in &cluster_addrs {
        let (ip, _main_port, member_raft_port) =
            parse_raft_member_address(addr_str, default_port, raft_offset)?;
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

/// Parse one `ip:port[?raft_port=xxx]` cluster member address.
///
/// Invalid ports used to fall back to the local main port, and `main - 1000`
/// wrapped/panicked for small ports. Either case produced a node_id that
/// disagreed with the other nodes' view of the same member, so the cluster
/// could never form a quorum. A malformed member is now a startup error.
fn parse_raft_member_address(
    addr_str: &str,
    default_port: u16,
    raft_offset: u16,
) -> Result<(String, u16, u16), String> {
    let addr_part = addr_str.split('?').next().unwrap_or(addr_str);
    let (ip, main_port) = if let Some((ip, port_str)) = addr_part.rsplit_once(':') {
        let port = port_str
            .parse::<u16>()
            .map_err(|e| format!("invalid cluster member port in {addr_str:?}: {e}"))?;
        (ip.to_string(), port)
    } else {
        (addr_part.to_string(), default_port)
    };

    let explicit_raft_port = addr_str.split('?').nth(1).and_then(|params| {
        params.split('&').find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            (k.trim() == "raft_port").then_some(v.trim())
        })
    });

    let member_raft_port = match explicit_raft_port {
        Some(value) => value
            .parse::<u16>()
            .map_err(|e| format!("invalid raft_port in {addr_str:?}: {e}"))?,
        None => main_port.checked_sub(raft_offset).ok_or_else(|| {
            format!(
                "cluster member {addr_str:?} has main port {main_port} below the raft offset \
                 {raft_offset}; set ?raft_port= explicitly"
            )
        })?,
    };

    Ok((ip, main_port, member_raft_port))
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

    const OFFSET: u16 = batata_api::model::Member::DEFAULT_RAFT_OFFSET_PORT;

    #[test]
    fn parses_member_port_and_derives_raft_port() {
        let (ip, main, raft) = parse_raft_member_address("127.0.0.1:8848", 8848, OFFSET).unwrap();
        assert_eq!(ip, "127.0.0.1");
        assert_eq!(main, 8848);
        assert_eq!(raft, 8848 - OFFSET);
    }

    #[test]
    fn parses_explicit_raft_port_override() {
        let (_, main, raft) =
            parse_raft_member_address("10.0.0.2:8848?raft_port=7948", 8848, OFFSET).unwrap();
        assert_eq!(main, 8848);
        assert_eq!(raft, 7948);
    }

    #[test]
    fn rejects_invalid_main_port_instead_of_guessing() {
        let err = parse_raft_member_address("127.0.0.1:not-a-port", 8848, OFFSET).unwrap_err();
        assert!(err.contains("invalid cluster member port"), "{err}");
    }

    #[test]
    fn rejects_invalid_explicit_raft_port() {
        let err =
            parse_raft_member_address("127.0.0.1:8848?raft_port=xyz", 8848, OFFSET).unwrap_err();
        assert!(err.contains("invalid raft_port"), "{err}");
    }

    #[test]
    fn rejects_main_port_below_raft_offset() {
        // 900 - 1000 would panic in debug / wrap in release.
        let err = parse_raft_member_address("127.0.0.1:900", 8848, OFFSET).unwrap_err();
        assert!(err.contains("below the raft offset"), "{err}");

        // An explicit override is still allowed for such a member.
        let (_, _, raft) =
            parse_raft_member_address("127.0.0.1:900?raft_port=7948", 8848, OFFSET).unwrap();
        assert_eq!(raft, 7948);
    }

    #[test]
    fn address_without_port_uses_default() {
        let (ip, main, raft) = parse_raft_member_address("127.0.0.1", 8848, OFFSET).unwrap();
        assert_eq!(ip, "127.0.0.1");
        assert_eq!(main, 8848);
        assert_eq!(raft, 8848 - OFFSET);
    }
}
