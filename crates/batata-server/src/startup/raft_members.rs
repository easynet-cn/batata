//! Explicit Raft identities derived from the shared cluster member list.
//!
//! OpenRaft 0.10 includes the node id in a log's identity, so it must be
//! stable across toolchain upgrades and address changes. Every cluster member
//! therefore carries `node_id=<nonzero u64>`; the local value is persisted in
//! the Raft data directory and checked on every start. There is deliberately
//! no address-hash fallback.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use batata_consistency::NodeId;
use batata_server_common::model::config::Configuration;
use tracing::info;

const NODE_ID_PARAM: &str = "node_id";
const NODE_ID_FILE: &str = "node-id";

/// One Raft peer with its explicit identity and transport address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaftMember {
    pub node_id: NodeId,
    pub raft_addr: String,
}

/// Membership assembled from `cluster.conf` / `batata.member.list`.
#[derive(Debug)]
pub struct RaftMembers {
    pub by_id: BTreeMap<NodeId, openraft::BasicNode>,
    pub local: RaftMember,
}

/// Parse the shared member list and verify this process's persisted identity.
pub fn resolve_raft_members(configuration: &Configuration) -> Result<RaftMembers, String> {
    let members = build_raft_members(
        &configuration.cluster_member_addresses(),
        configuration.server_main_port(),
        configuration.server_main_port(),
        configuration.raft_port(),
        batata_api::model::Member::DEFAULT_RAFT_OFFSET_PORT,
    )?;
    ensure_local_node_id(
        Path::new(&configuration.raft_data_dir()),
        members.local.node_id,
    )?;
    Ok(members)
}

/// Build the Raft membership map and identify the local member.
///
/// The member list is the cluster-wide source of truth; every node parses the
/// same entries and reaches the same `node_id -> raft address` mapping.
pub fn build_raft_members(
    addresses: &[String],
    default_port: u16,
    local_main_port: u16,
    local_raft_port: u16,
    raft_offset: u16,
) -> Result<RaftMembers, String> {
    let mut by_id = BTreeMap::new();
    let mut seen_addrs = BTreeSet::new();
    let mut local = None;

    for addr_str in addresses {
        let (ip, member_main_port, member_raft_port) =
            parse_raft_address(addr_str, default_port, raft_offset)?;
        let node_id = query_param(addr_str, NODE_ID_PARAM)
            .ok_or_else(|| {
                format!(
                    "cluster member {addr_str:?} is missing ?node_id=<nonzero u64>; \
                     address-derived Raft ids are no longer supported"
                )
            })?
            .parse::<NodeId>()
            .map_err(|e| format!("invalid node_id in {addr_str:?}: {e}"))?;

        if node_id == 0 {
            return Err(format!(
                "cluster member {addr_str:?} has node_id=0; Raft node ids must be positive"
            ));
        }
        if by_id.contains_key(&node_id) {
            return Err(format!(
                "duplicate Raft node_id {node_id} in cluster member list"
            ));
        }

        let raft_addr = format!("{ip}:{member_raft_port}");
        if !seen_addrs.insert(raft_addr.clone()) {
            return Err(format!(
                "duplicate Raft address {raft_addr} in cluster member list"
            ));
        }

        if member_main_port == local_main_port {
            if member_raft_port != local_raft_port {
                return Err(format!(
                    "local cluster member {addr_str:?} declares raft_port {member_raft_port}, \
                     but this process binds Raft gRPC on {local_raft_port}"
                ));
            }
            if local.is_some() {
                return Err(format!(
                    "more than one cluster member uses local main port {local_main_port}"
                ));
            }
            local = Some(RaftMember {
                node_id,
                raft_addr: raft_addr.clone(),
            });
        }

        by_id.insert(node_id, openraft::BasicNode { addr: raft_addr });
    }

    if by_id.is_empty() {
        return Err("no cluster members configured for Raft initialization".to_string());
    }
    let local = local.ok_or_else(|| {
        format!(
            "no cluster member matches this process's main port {local_main_port}; \
             add an entry with an explicit node_id"
        )
    })?;

    Ok(RaftMembers { by_id, local })
}

/// Persist the configured id on first start and reject later identity changes.
///
/// The file is a self-check, not the cluster source of truth: all peers use
/// `cluster.conf` to discover who owns an id. A mismatch means the local data
/// belongs to a different generation and must not be reused silently.
pub fn ensure_local_node_id(data_dir: &Path, node_id: NodeId) -> Result<(), String> {
    if node_id == 0 {
        return Err("Raft node_id must be positive".to_string());
    }
    let path = data_dir.join(NODE_ID_FILE);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let persisted: NodeId = content.trim().parse().map_err(|e| {
                format!("invalid persisted Raft node id in {}: {e}", path.display())
            })?;
            if persisted != node_id {
                return Err(format!(
                    "Raft node id mismatch: member list assigns {node_id}, but {} contains \
                     {persisted}. Keep the persisted identity, or stop the cluster, wipe Raft \
                     data and re-initialize membership.",
                    path.display()
                ));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(data_dir)
                .map_err(|e| format!("failed to create {}: {e}", data_dir.display()))?;
            std::fs::write(&path, node_id.to_string())
                .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
            info!("Persisted Raft node id {node_id} to {}", path.display());
        }
        Err(e) => {
            return Err(format!("failed to read {}: {e}", path.display()));
        }
    }
    Ok(())
}

/// Read one query parameter from a member address.
fn query_param<'a>(address: &'a str, key: &str) -> Option<&'a str> {
    let (_, params) = address.split_once('?')?;
    params.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k.trim() == key).then_some(v.trim())
    })
}

/// Parse `ip:port[?raft_port=xxx]` into its address components.
///
/// Invalid ports never fall back to a guessed value: a malformed member would
/// make peers disagree about the cluster layout and prevent quorum.
fn parse_raft_address(
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

    let member_raft_port = match query_param(addr_str, "raft_port") {
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

#[cfg(test)]
mod tests {
    use super::*;

    const OFFSET: u16 = batata_api::model::Member::DEFAULT_RAFT_OFFSET_PORT;

    fn members(addresses: &[&str]) -> Result<RaftMembers, String> {
        let owned: Vec<String> = addresses.iter().map(|s| (*s).to_string()).collect();
        build_raft_members(&owned, 8848, 8858, 8858 - OFFSET, OFFSET)
    }

    #[test]
    fn explicit_node_ids_form_the_membership_map() {
        let parsed = members(&[
            "127.0.0.1:8848?node_id=1",
            "127.0.0.1:8858?node_id=2",
            "127.0.0.1:8868?node_id=3",
        ])
        .unwrap();

        assert_eq!(parsed.by_id.len(), 3);
        assert_eq!(
            parsed.by_id[&1].addr,
            format!("127.0.0.1:{}", 8848 - OFFSET)
        );
        assert_eq!(
            parsed.by_id[&2].addr,
            format!("127.0.0.1:{}", 8858 - OFFSET)
        );
        assert_eq!(
            parsed.by_id[&3].addr,
            format!("127.0.0.1:{}", 8868 - OFFSET)
        );
        assert_eq!(parsed.local.node_id, 2);
        assert_eq!(
            parsed.local.raft_addr,
            format!("127.0.0.1:{}", 8858 - OFFSET)
        );
    }

    #[test]
    fn node_id_parsing_is_independent_of_parameter_order() {
        let parsed = members(&["127.0.0.1:8858?node_id=7&raft_port=7858"]).unwrap();
        assert_eq!(parsed.local.node_id, 7);
        assert_eq!(parsed.local.raft_addr, "127.0.0.1:7858");

        let parsed = members(&["127.0.0.1:8858?raft_port=7858&node_id=7"]).unwrap();
        assert_eq!(parsed.local.node_id, 7);
    }

    #[test]
    fn rejects_member_without_explicit_node_id() {
        let err = members(&["127.0.0.1:8858"]).unwrap_err();
        assert!(err.contains("missing ?node_id"), "{err}");
    }

    #[test]
    fn rejects_zero_and_invalid_node_ids() {
        let err = members(&["127.0.0.1:8858?node_id=0"]).unwrap_err();
        assert!(err.contains("node_id=0"), "{err}");

        let err = members(&["127.0.0.1:8858?node_id=not-a-number"]).unwrap_err();
        assert!(err.contains("invalid node_id"), "{err}");
    }

    #[test]
    fn rejects_duplicate_node_ids_and_addresses() {
        let err = members(&["127.0.0.1:8848?node_id=1", "127.0.0.1:8858?node_id=1"]).unwrap_err();
        assert!(err.contains("duplicate Raft node_id 1"), "{err}");

        let err = members(&["127.0.0.1:8848?node_id=1", "127.0.0.1:8848?node_id=2"]).unwrap_err();
        assert!(err.contains("duplicate Raft address"), "{err}");
    }

    #[test]
    fn rejects_missing_or_conflicting_local_entry() {
        let err = members(&["127.0.0.1:8848?node_id=1"]).unwrap_err();
        assert!(err.contains("matches this process's main port"), "{err}");

        let err = members(&["127.0.0.1:8858?node_id=1&raft_port=7948"]).unwrap_err();
        assert!(err.contains("binds Raft gRPC"), "{err}");
    }

    #[test]
    fn rejects_invalid_ports_without_guessing() {
        let err = members(&["127.0.0.1:not-a-port?node_id=1"]).unwrap_err();
        assert!(err.contains("invalid cluster member port"), "{err}");

        let err = members(&["127.0.0.1:8858?node_id=1&raft_port=xyz"]).unwrap_err();
        assert!(err.contains("invalid raft_port"), "{err}");

        let err = members(&["127.0.0.1:900?node_id=1"]).unwrap_err();
        assert!(err.contains("below the raft offset"), "{err}");
    }

    #[test]
    fn address_without_port_uses_default() {
        let owned = vec!["127.0.0.1?node_id=1".to_string()];
        let parsed = build_raft_members(&owned, 8858, 8858, 8858 - OFFSET, OFFSET).unwrap();
        assert_eq!(
            parsed.by_id[&1].addr,
            format!("127.0.0.1:{}", 8858 - OFFSET)
        );
    }

    #[test]
    fn node_id_file_is_written_once_and_guards_against_changes() {
        let dir = std::env::temp_dir().join(format!(
            "batata-node-id-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        ensure_local_node_id(&dir, 42).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join(NODE_ID_FILE)).unwrap(),
            "42"
        );
        ensure_local_node_id(&dir, 42).unwrap();

        let err = ensure_local_node_id(&dir, 43).unwrap_err();
        assert!(err.contains("node id mismatch"), "{err}");
        assert!(err.contains("42"), "{err}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_unreadable_persisted_node_id() {
        let dir = std::env::temp_dir().join(format!(
            "batata-node-id-corrupt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(NODE_ID_FILE), "not-a-number").unwrap();

        let err = ensure_local_node_id(&dir, 1).unwrap_err();
        assert!(err.contains("invalid persisted Raft node id"), "{err}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
