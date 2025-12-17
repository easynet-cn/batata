// Raft configuration
// Provides configuration settings for the Raft consensus implementation

use std::path::PathBuf;
use std::time::Duration;

use crate::model::common::Configuration;

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
    /// Create configuration from application configuration
    pub fn from_configuration(config: &Configuration) -> Self {
        let mut raft_config = Self::default();

        // Election timeout
        if let Ok(val) = config.config.get_int("nacos.core.protocol.raft.data.election_timeout_ms") {
            raft_config.election_timeout_ms = val as u64;
        }

        // Heartbeat interval
        if let Ok(val) = config.config.get_int("nacos.core.protocol.raft.data.heartbeat_interval_ms") {
            raft_config.heartbeat_interval_ms = val as u64;
        }

        // Snapshot interval
        if let Ok(val) = config.config.get_int("nacos.core.protocol.raft.data.snapshot_interval_secs") {
            raft_config.snapshot_interval_secs = val as u64;
        }

        // Snapshot threshold
        if let Ok(val) = config.config.get_int("nacos.core.protocol.raft.data.snapshot_threshold") {
            raft_config.snapshot_threshold = val as u64;
        }

        // RPC timeout
        if let Ok(val) = config.config.get_int("nacos.core.protocol.raft.data.rpc_request_timeout_ms") {
            raft_config.rpc_request_timeout_ms = val as u64;
        }

        // Max payload entries
        if let Ok(val) = config.config.get_int("nacos.core.protocol.raft.data.max_payload_entries") {
            raft_config.max_payload_entries = val as u64;
        }

        // Data directory
        if let Ok(val) = config.config.get_string("nacos.core.protocol.raft.data.dir") {
            raft_config.data_dir = PathBuf::from(val);
        }

        raft_config
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
    }

    #[test]
    fn test_duration_conversions() {
        let config = RaftConfig::default();
        assert_eq!(config.election_timeout(), Duration::from_millis(5000));
        assert_eq!(config.heartbeat_interval(), Duration::from_millis(1000));
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
}
