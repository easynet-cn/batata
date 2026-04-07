//! Core context traits: DbContext, ConfigContext, ClusterContext and related types.

/// Database access context trait
///
/// Implementations provide access to the database connection.
/// This allows services to work with any type that can provide
/// database access without depending on concrete types.
pub trait DbContext: Send + Sync {
    /// Get the database URL or identifier
    fn db_url(&self) -> &str;
}

/// Configuration access context trait
///
/// Provides access to application configuration values
/// that are needed by various services.
pub trait ConfigContext: Send + Sync {
    /// Maximum allowed content size for configurations
    fn max_content(&self) -> u64;

    /// Whether authentication is enabled
    fn auth_enabled(&self) -> bool;

    /// JWT token expiration time in seconds
    fn token_expire_seconds(&self) -> i64;

    /// Secret key for JWT signing
    fn secret_key(&self) -> String;

    /// Get the server's main port
    fn main_port(&self) -> u16;

    /// Get the console port
    fn console_port(&self) -> u16;
}

/// Cluster member information
#[derive(Debug, Clone)]
pub struct MemberInfo {
    pub ip: String,
    pub port: u16,
    pub address: String,
    pub state: MemberState,
}

/// Member state in the cluster
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Cluster management context trait
///
/// Provides access to cluster state and member management.
pub trait ClusterContext: Send + Sync {
    /// Check if running in standalone mode
    fn is_standalone(&self) -> bool;

    /// Check if this node is the leader
    fn is_leader(&self) -> bool;

    /// Get the leader's address if known
    fn leader_address(&self) -> Option<String>;

    /// Get all cluster members
    fn all_members(&self) -> Vec<MemberInfo>;

    /// Get only healthy cluster members
    fn healthy_members(&self) -> Vec<MemberInfo>;

    /// Get the current member count
    fn member_count(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_member_state_display() {
        assert_eq!(format!("{}", MemberState::Up), "UP");
        assert_eq!(format!("{}", MemberState::Down), "DOWN");
        assert_eq!(format!("{}", MemberState::Suspicious), "SUSPICIOUS");
    }

    #[test]
    fn test_member_info_creation() {
        let member = MemberInfo {
            ip: "192.168.1.1".to_string(),
            port: 8848,
            address: "192.168.1.1:8848".to_string(),
            state: MemberState::Up,
        };
        assert_eq!(member.ip, "192.168.1.1");
        assert_eq!(member.port, 8848);
    }
}
