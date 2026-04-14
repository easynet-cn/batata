//! Cluster manager trait and related types.

use super::core_ctx::MemberState;

/// Cluster health summary
#[derive(Clone, Debug, Default)]
pub struct ClusterHealthSummary {
    pub total: usize,
    pub up: usize,
    pub down: usize,
    pub suspicious: usize,
    pub starting: usize,
    pub isolation: usize,
}

impl ClusterHealthSummary {
    pub fn is_healthy(&self) -> bool {
        self.up > self.total / 2
    }
}

/// Extended member information with metadata
///
/// Provides richer member data than `MemberInfo`, including
/// extend_info metadata used by Consul and other plugins.
#[derive(Debug, Clone)]
pub struct ExtendedMemberInfo {
    pub ip: String,
    pub port: u16,
    pub address: String,
    pub state: MemberState,
    pub extend_info: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Cluster manager trait
///
/// Abstracts cluster membership management operations.
/// This allows plugins (Consul, Console) to depend on the trait
/// rather than the concrete `ServerMemberManager` type.
pub trait ClusterManager: Send + Sync {
    /// Check if running in standalone mode
    fn is_standalone(&self) -> bool;

    /// Check if this node is the leader
    fn is_leader(&self) -> bool;

    /// Check if the cluster is healthy (majority of nodes are up)
    fn is_cluster_healthy(&self) -> bool;

    /// Get the leader's address if known
    fn leader_address(&self) -> Option<String>;

    /// Get the local node's address
    fn local_address(&self) -> &str;

    /// Get the current member count
    fn member_count(&self) -> usize;

    /// Get all cluster members
    fn all_members_extended(&self) -> Vec<ExtendedMemberInfo>;

    /// Get only healthy cluster members
    fn healthy_members_extended(&self) -> Vec<ExtendedMemberInfo>;

    /// Get a member by address
    fn get_member(&self, address: &str) -> Option<ExtendedMemberInfo>;

    /// Get self member info
    fn get_self_member(&self) -> ExtendedMemberInfo;

    /// Get cluster health summary
    fn health_summary(&self) -> ClusterHealthSummary;

    /// Refresh self member's last refresh timestamp
    fn refresh_self(&self);

    /// Check if a given address is the local node
    fn is_self(&self, address: &str) -> bool;

    /// Update a member's state.
    ///
    /// `state` is a case-insensitive state name (`UP`, `DOWN`, `SUSPICIOUS`,
    /// `STARTING`, `ISOLATION`). Returns the previous state string on success,
    /// `Err` if the state name is invalid or the address is not a known member.
    fn update_member_state(&self, address: &str, state: &str) -> Result<String, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_health_summary_is_healthy() {
        let healthy = ClusterHealthSummary {
            total: 3,
            up: 2,
            down: 1,
            ..Default::default()
        };
        assert!(healthy.is_healthy());

        let unhealthy = ClusterHealthSummary {
            total: 3,
            up: 1,
            down: 2,
            ..Default::default()
        };
        assert!(!unhealthy.is_healthy());
    }

    #[test]
    fn test_extended_member_info() {
        let member = ExtendedMemberInfo {
            ip: "10.0.0.1".to_string(),
            port: 8848,
            address: "10.0.0.1:8848".to_string(),
            state: MemberState::Up,
            extend_info: std::collections::BTreeMap::new(),
        };
        assert_eq!(member.ip, "10.0.0.1");
        assert_eq!(member.state, MemberState::Up);
    }
}
