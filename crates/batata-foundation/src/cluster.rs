//! Cluster membership and coordination
//!
//! Provides traits for cluster member discovery, leader election awareness,
//! and member health tracking.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::FoundationError;

/// Cluster member information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Member {
    /// Node unique identifier
    pub id: String,
    /// Node IP address
    pub ip: String,
    /// Node port
    pub port: u16,
    /// Node address in "ip:port" format
    pub address: String,
    /// Current state
    pub state: MemberState,
    /// Metadata/labels attached to this member
    pub metadata: std::collections::HashMap<String, String>,
}

impl Member {
    pub fn new(ip: String, port: u16) -> Self {
        let address = format!("{ip}:{port}");
        let id = address.clone();
        Self {
            id,
            ip,
            port,
            address,
            state: MemberState::Up,
            metadata: std::collections::HashMap::new(),
        }
    }
}

/// Member state in the cluster
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemberState {
    Up,
    Down,
    Suspicious,
}

impl std::fmt::Display for MemberState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemberState::Up => write!(f, "UP"),
            MemberState::Down => write!(f, "DOWN"),
            MemberState::Suspicious => write!(f, "SUSPICIOUS"),
        }
    }
}

/// Cluster membership — read-only view of the cluster
#[async_trait]
pub trait ClusterMembership: Send + Sync {
    /// Get the local member
    fn self_member(&self) -> &Member;

    /// Check if running in standalone mode (single node)
    fn is_standalone(&self) -> bool;

    /// Check if this node is the leader
    fn is_leader(&self) -> bool;

    /// Get the leader's address, if known
    fn leader_address(&self) -> Option<String>;

    /// Get all cluster members
    fn all_members(&self) -> Vec<Member>;

    /// Get only healthy (UP) members
    fn healthy_members(&self) -> Vec<Member> {
        self.all_members()
            .into_iter()
            .filter(|m| m.state == MemberState::Up)
            .collect()
    }

    /// Get current member count
    fn member_count(&self) -> usize {
        self.all_members().len()
    }
}

/// Member discovery — how to find other cluster members
///
/// Implementations:
/// - File-based: read from a config file
/// - Address server: query a central address server
/// - Kubernetes: use K8s API for pod discovery
#[async_trait]
pub trait MemberLookup: Send + Sync {
    /// Start the member discovery process
    async fn start(&self) -> Result<(), FoundationError>;

    /// Stop the member discovery process
    async fn stop(&self) -> Result<(), FoundationError>;

    /// Get the current list of discovered members
    async fn get_members(&self) -> Result<Vec<Member>, FoundationError>;
}

/// Listener for cluster membership changes
#[async_trait]
pub trait MemberChangeListener: Send + Sync {
    /// Called when the cluster membership changes
    async fn on_member_change(&self, event: MemberChangeEvent);
}

/// Cluster membership change event
#[derive(Debug, Clone)]
pub struct MemberChangeEvent {
    /// Members that joined
    pub joined: Vec<Member>,
    /// Members that left
    pub left: Vec<Member>,
    /// Members whose state changed
    pub changed: Vec<Member>,
}
