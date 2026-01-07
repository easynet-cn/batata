// Raft configuration
// Provides configuration settings for the Raft consensus implementation

use std::path::PathBuf;
use std::time::Duration;

/// Configuration for Raft consensus protocol
#[derive(Clone, Debug)]
pub struct RaftConfig {
    /// Election timeout in milliseconds (default: 5000ms)
    /// If a follower doesn't hear from leader within this time, it starts an election
    pub election_timeout_ms: u64,

    /// Heartbeat interval in milliseconds (default: 1000ms)
    /// Leader sends heartbeats at this interval
    pub heartbeat_interval_ms: u64,

    /// Snapshot interval in seconds (default: 1800s = 30 minutes)
    /// How often to take snapshots for log compaction
    pub snapshot_interval_secs: u64,

    /// Snapshot threshold - number of log entries before triggering snapshot
    pub snapshot_threshold: u64,

    /// RPC request timeout in milliseconds (default: 5000ms)
    pub rpc_request_timeout_ms: u64,

    /// Maximum entries per append request (default: 300)
    pub max_payload_entries: u64,

    /// Data directory for Raft storage
    pub data_dir: PathBuf,

    /// Maximum size of a single log entry in bytes (default: 4MB)
    pub max_entry_size: u64,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            election_timeout_ms: 5000,
            heartbeat_interval_ms: 1000,
            snapshot_interval_secs: 1800,
            snapshot_threshold: 10000,
            rpc_request_timeout_ms: 5000,
            max_payload_entries: 300,
            data_dir: PathBuf::from("./data/raft"),
            max_entry_size: 4 * 1024 * 1024, // 4MB
        }
    }
}

impl RaftConfig {
    /// Create a new RaftConfig with custom settings
    pub fn new(
        election_timeout_ms: u64,
        heartbeat_interval_ms: u64,
        snapshot_interval_secs: u64,
        snapshot_threshold: u64,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            election_timeout_ms,
            heartbeat_interval_ms,
            snapshot_interval_secs,
            snapshot_threshold,
            rpc_request_timeout_ms: 5000,
            max_payload_entries: 300,
            data_dir,
            max_entry_size: 4 * 1024 * 1024,
        }
    }

    /// Get election timeout as Duration
    pub fn election_timeout(&self) -> Duration {
        Duration::from_millis(self.election_timeout_ms)
    }

    /// Get heartbeat interval as Duration
    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.heartbeat_interval_ms)
    }

    /// Get RPC timeout as Duration
    pub fn rpc_timeout(&self) -> Duration {
        Duration::from_millis(self.rpc_request_timeout_ms)
    }

    /// Get the log store directory
    pub fn log_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    /// Get the state machine directory
    pub fn state_machine_dir(&self) -> PathBuf {
        self.data_dir.join("state")
    }

    /// Get the snapshot directory
    pub fn snapshot_dir(&self) -> PathBuf {
        self.data_dir.join("snapshots")
    }

    /// Ensure all data directories exist
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(self.log_dir())?;
        std::fs::create_dir_all(self.state_machine_dir())?;
        std::fs::create_dir_all(self.snapshot_dir())?;
        Ok(())
    }

    /// Convert to openraft Config
    pub fn to_openraft_config(&self) -> openraft::Config {
        openraft::Config {
            cluster_name: "batata".to_string(),
            election_timeout_min: self.election_timeout_ms,
            election_timeout_max: self.election_timeout_ms * 2,
            heartbeat_interval: self.heartbeat_interval_ms,
            snapshot_policy: openraft::SnapshotPolicy::LogsSinceLast(self.snapshot_threshold),
            max_payload_entries: self.max_payload_entries,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RaftConfig::default();
        assert_eq!(config.election_timeout_ms, 5000);
        assert_eq!(config.heartbeat_interval_ms, 1000);
        assert_eq!(config.snapshot_interval_secs, 1800);
        assert_eq!(config.snapshot_threshold, 10000);
        assert_eq!(config.rpc_request_timeout_ms, 5000);
        assert_eq!(config.max_payload_entries, 300);
        assert_eq!(config.max_entry_size, 4 * 1024 * 1024);
    }

    #[test]
    fn test_duration_conversions() {
        let config = RaftConfig::default();
        assert_eq!(config.election_timeout(), Duration::from_millis(5000));
        assert_eq!(config.heartbeat_interval(), Duration::from_millis(1000));
        assert_eq!(config.rpc_timeout(), Duration::from_millis(5000));
    }

    #[test]
    fn test_directory_paths() {
        let config = RaftConfig {
            data_dir: PathBuf::from("/tmp/raft"),
            ..Default::default()
        };
        assert_eq!(config.log_dir(), PathBuf::from("/tmp/raft/logs"));
        assert_eq!(config.state_machine_dir(), PathBuf::from("/tmp/raft/state"));
        assert_eq!(config.snapshot_dir(), PathBuf::from("/tmp/raft/snapshots"));
    }

    #[test]
    fn test_new_config() {
        let config = RaftConfig::new(
            3000,
            500,
            900,
            5000,
            PathBuf::from("/data/raft"),
        );
        assert_eq!(config.election_timeout_ms, 3000);
        assert_eq!(config.heartbeat_interval_ms, 500);
        assert_eq!(config.snapshot_interval_secs, 900);
        assert_eq!(config.snapshot_threshold, 5000);
        assert_eq!(config.data_dir, PathBuf::from("/data/raft"));
    }

    #[test]
    fn test_to_openraft_config() {
        let config = RaftConfig::default();
        let openraft_config = config.to_openraft_config();

        assert_eq!(openraft_config.cluster_name, "batata");
        assert_eq!(openraft_config.election_timeout_min, 5000);
        assert_eq!(openraft_config.election_timeout_max, 10000);
        assert_eq!(openraft_config.heartbeat_interval, 1000);
        assert_eq!(openraft_config.max_payload_entries, 300);
    }

    #[test]
    fn test_config_clone() {
        let config1 = RaftConfig::default();
        let config2 = config1.clone();

        assert_eq!(config1.election_timeout_ms, config2.election_timeout_ms);
        assert_eq!(config1.heartbeat_interval_ms, config2.heartbeat_interval_ms);
        assert_eq!(config1.data_dir, config2.data_dir);
    }

    #[test]
    fn test_ensure_dirs() {
        let temp_dir = std::env::temp_dir().join(format!("raft_test_{}", std::process::id()));
        let config = RaftConfig {
            data_dir: temp_dir.clone(),
            ..Default::default()
        };

        // Should succeed creating directories
        assert!(config.ensure_dirs().is_ok());

        // Verify directories exist
        assert!(temp_dir.exists());
        assert!(config.log_dir().exists());
        assert!(config.state_machine_dir().exists());
        assert!(config.snapshot_dir().exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
