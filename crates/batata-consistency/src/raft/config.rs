//! Raft configuration.
//!
//! Provides configuration settings for the Raft consensus implementation.

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

    /// Data directory for Raft-owned state (log and snapshots).
    ///
    /// Mirrors JRaft's `data/protocol/raft/{group}` layout: everything under
    /// this directory belongs to the consensus layer and nothing else should
    /// live here.
    pub data_dir: PathBuf,

    /// Where the state machine database lives.
    ///
    /// `None` means `{data_dir}/state`. The server always sets this to the
    /// shared business database path so that standalone and cluster modes
    /// write to the same place -- switching modes must not orphan the data.
    pub state_machine_dir: Option<PathBuf>,

    /// Size of a single snapshot chunk pushed to a follower, in bytes.
    ///
    /// Bounded by the gRPC message limit; see `snapshot_max_message_size`.
    pub snapshot_chunk_size: usize,

    /// Maximum size of a single log entry in bytes (default: 4MB)
    pub max_entry_size: u64,

    /// Snapshot transfer timeout in milliseconds (default: 30000ms)
    /// Used for full snapshot transfer between nodes. Should be > rpc_request_timeout_ms.
    pub snapshot_transfer_timeout_ms: u64,

    /// Max retries when forwarding write to leader (default: 3)
    pub forward_max_retries: u32,

    /// Initial retry delay in ms for leader forwarding (default: 200ms)
    /// Doubles each attempt (exponential backoff) with 25% jitter.
    pub forward_initial_delay_ms: u64,

    /// TCP keep-alive interval in seconds for Raft gRPC connections (default: 10s)
    pub grpc_tcp_keepalive_secs: u64,

    /// Enable TCP_NODELAY for Raft gRPC connections (default: true)
    pub grpc_tcp_nodelay: bool,

    /// HTTP/2 keep-alive interval in seconds for Raft gRPC connections (default: 10s)
    pub grpc_http2_keepalive_interval_secs: u64,

    /// HTTP/2 keep-alive timeout in seconds for Raft gRPC connections (default: 5s)
    pub grpc_http2_keepalive_timeout_secs: u64,
}

/// Upper bound for a single Raft gRPC message, derived from a chunk size.
///
/// Shared by the Raft server and the Raft client so the two can never drift
/// apart: twice the chunk (headroom for framing and metadata), never below
/// 4 MiB.
pub fn snapshot_max_message_size_for(chunk_size: usize) -> usize {
    let chunk = if chunk_size == 0 {
        DEFAULT_SNAPSHOT_CHUNK_SIZE
    } else {
        chunk_size.min(MAX_SNAPSHOT_CHUNK_SIZE)
    };
    (chunk * 2).max(4 * 1024 * 1024)
}

/// Default size of one snapshot chunk: 1 MiB.
pub const DEFAULT_SNAPSHOT_CHUNK_SIZE: usize = 1024 * 1024;

/// Largest chunk we accept. A chunk travels as a single gRPC message, so it
/// has to stay comfortably below the default 4 MiB decoding limit.
pub const MAX_SNAPSHOT_CHUNK_SIZE: usize = 3 * 1024 * 1024;

/// HTTP/2 flow-control window advertised for Raft traffic. It must exceed the
/// chunk size, otherwise every chunk stalls waiting for WINDOW_UPDATE.
pub const DEFAULT_RAFT_STREAM_WINDOW_SIZE: u32 = 4 * 1024 * 1024;

/// HTTP/2 frame size for Raft traffic. Larger frames mean fewer syscalls per
/// chunk; 64 KiB is the practical sweet spot.
pub const DEFAULT_RAFT_MAX_FRAME_SIZE: u32 = 64 * 1024;

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            election_timeout_ms: 3000,
            heartbeat_interval_ms: 300,
            snapshot_interval_secs: 1800,
            snapshot_threshold: 10000,
            rpc_request_timeout_ms: 5000,
            max_payload_entries: 300,
            data_dir: PathBuf::from("./data/protocol/raft"),
            state_machine_dir: None,
            snapshot_chunk_size: DEFAULT_SNAPSHOT_CHUNK_SIZE,
            max_entry_size: 4 * 1024 * 1024, // 4MB
            snapshot_transfer_timeout_ms: 30000,
            forward_max_retries: 3,
            forward_initial_delay_ms: 200,
            grpc_tcp_keepalive_secs: 10,
            grpc_tcp_nodelay: true,
            grpc_http2_keepalive_interval_secs: 10,
            grpc_http2_keepalive_timeout_secs: 5,
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
            data_dir,
            ..Default::default()
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

    /// Get snapshot transfer timeout as Duration
    pub fn snapshot_transfer_timeout(&self) -> Duration {
        Duration::from_millis(self.snapshot_transfer_timeout_ms)
    }

    /// Get the Raft log store directory
    pub fn log_dir(&self) -> PathBuf {
        self.data_dir.join("log")
    }

    /// Get the state machine directory.
    ///
    /// Defaults to `{data_dir}/state`, but the server overrides it with the
    /// shared business database path so both deployment modes agree.
    pub fn state_machine_dir(&self) -> PathBuf {
        self.state_machine_dir
            .clone()
            .unwrap_or_else(|| self.data_dir.join("state"))
    }

    /// Get the snapshot directory
    pub fn snapshot_dir(&self) -> PathBuf {
        self.data_dir.join("snapshot")
    }

    /// Ensure all data directories exist
    pub async fn ensure_dirs(&self) -> std::io::Result<()> {
        tokio::fs::create_dir_all(&self.data_dir).await?;
        tokio::fs::create_dir_all(self.log_dir()).await?;
        tokio::fs::create_dir_all(self.state_machine_dir()).await?;
        tokio::fs::create_dir_all(self.snapshot_dir()).await?;
        Ok(())
    }

    /// Snapshot chunk size actually used, clamped into a safe range.
    ///
    /// A configured `0` falls back to the default; anything above
    /// `MAX_SNAPSHOT_CHUNK_SIZE` is capped so a chunk always fits inside the
    /// gRPC message limit instead of failing the transfer at runtime.
    pub fn effective_snapshot_chunk_size(&self) -> usize {
        if self.snapshot_chunk_size == 0 {
            DEFAULT_SNAPSHOT_CHUNK_SIZE
        } else {
            self.snapshot_chunk_size.min(MAX_SNAPSHOT_CHUNK_SIZE)
        }
    }

    /// Upper bound for a single Raft gRPC message.
    ///
    /// Derived from the chunk size so the two can never drift apart: twice the
    /// chunk (headroom for framing and metadata), never below 4 MiB.
    pub fn snapshot_max_message_size(&self) -> usize {
        snapshot_max_message_size_for(self.snapshot_chunk_size)
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
        assert_eq!(config.election_timeout_ms, 3000);
        assert_eq!(config.heartbeat_interval_ms, 300);
        assert_eq!(config.snapshot_interval_secs, 1800);
        assert_eq!(config.snapshot_threshold, 10000);
        assert_eq!(config.rpc_request_timeout_ms, 5000);
        assert_eq!(config.max_payload_entries, 300);
        assert_eq!(config.max_entry_size, 4 * 1024 * 1024);
        assert_eq!(config.snapshot_transfer_timeout_ms, 30000);
    }

    #[test]
    fn test_duration_conversions() {
        let config = RaftConfig::default();
        assert_eq!(config.election_timeout(), Duration::from_millis(3000));
        assert_eq!(config.heartbeat_interval(), Duration::from_millis(300));
        assert_eq!(config.rpc_timeout(), Duration::from_millis(5000));
        assert_eq!(
            config.snapshot_transfer_timeout(),
            Duration::from_millis(30000)
        );
    }

    #[test]
    fn test_directory_paths() {
        let config = RaftConfig {
            data_dir: PathBuf::from("/tmp/raft"),
            ..Default::default()
        };
        assert_eq!(config.log_dir(), PathBuf::from("/tmp/raft/log"));
        assert_eq!(config.state_machine_dir(), PathBuf::from("/tmp/raft/state"));
        assert_eq!(config.snapshot_dir(), PathBuf::from("/tmp/raft/snapshot"));
    }

    /// Raft-owned state must stay out of the business database directory, and
    /// the state machine must be pointable at that shared database -- that is
    /// what keeps standalone and cluster layouts identical.
    #[test]
    fn test_state_machine_dir_can_be_moved_out_of_raft_root() {
        let config = RaftConfig {
            data_dir: PathBuf::from("/data/protocol/raft"),
            state_machine_dir: Some(PathBuf::from("/data/batata_rocksdb")),
            ..Default::default()
        };
        assert_eq!(config.log_dir(), PathBuf::from("/data/protocol/raft/log"));
        assert_eq!(
            config.snapshot_dir(),
            PathBuf::from("/data/protocol/raft/snapshot")
        );
        assert_eq!(
            config.state_machine_dir(),
            PathBuf::from("/data/batata_rocksdb")
        );
        // The business DB is never nested under the Raft root.
        assert!(!config.state_machine_dir().starts_with(&config.data_dir));
    }

    #[test]
    fn test_default_snapshot_chunk_size() {
        let config = RaftConfig::default();
        assert_eq!(config.snapshot_chunk_size, DEFAULT_SNAPSHOT_CHUNK_SIZE);
        assert_eq!(config.effective_snapshot_chunk_size(), 1024 * 1024);
    }

    /// A chunk may never exceed the gRPC message limit: zero falls back to the
    /// default, oversized values are capped.
    #[test]
    fn test_snapshot_chunk_size_is_clamped() {
        let zero = RaftConfig {
            snapshot_chunk_size: 0,
            ..Default::default()
        };
        assert_eq!(
            zero.effective_snapshot_chunk_size(),
            DEFAULT_SNAPSHOT_CHUNK_SIZE
        );

        let huge = RaftConfig {
            snapshot_chunk_size: 64 * 1024 * 1024,
            ..Default::default()
        };
        assert_eq!(
            huge.effective_snapshot_chunk_size(),
            MAX_SNAPSHOT_CHUNK_SIZE
        );

        let custom = RaftConfig {
            snapshot_chunk_size: 256 * 1024,
            ..Default::default()
        };
        assert_eq!(custom.effective_snapshot_chunk_size(), 256 * 1024);
    }

    /// The message limit follows the chunk size, and always leaves headroom.
    #[test]
    fn test_snapshot_max_message_size_tracks_chunk_size() {
        let small = RaftConfig {
            snapshot_chunk_size: 256 * 1024,
            ..Default::default()
        };
        // Small chunks still get the 4 MiB floor.
        assert_eq!(small.snapshot_max_message_size(), 4 * 1024 * 1024);

        let large = RaftConfig {
            snapshot_chunk_size: MAX_SNAPSHOT_CHUNK_SIZE,
            ..Default::default()
        };
        assert_eq!(large.snapshot_max_message_size(), 6 * 1024 * 1024);
        assert!(large.snapshot_max_message_size() > large.effective_snapshot_chunk_size());
    }

    #[test]
    fn test_new_config() {
        let config = RaftConfig::new(3000, 500, 900, 5000, PathBuf::from("/data/raft"));
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
        assert_eq!(openraft_config.election_timeout_min, 3000);
        assert_eq!(openraft_config.election_timeout_max, 6000);
        assert_eq!(openraft_config.heartbeat_interval, 300);
        assert_eq!(openraft_config.max_payload_entries, 300);
    }

    /// The configured chunk size must reach the network layer, and the window
    /// must stay larger than a chunk -- otherwise every chunk stalls.
    #[test]
    fn test_chunk_size_reaches_the_network_config() {
        use crate::raft::network::RaftNetworkConfig;

        let config = RaftConfig {
            snapshot_chunk_size: 256 * 1024,
            ..Default::default()
        };
        let net = RaftNetworkConfig::from_raft_config(&config);
        assert_eq!(net.snapshot_chunk_size, 256 * 1024);
        assert_eq!(net.max_message_size, config.snapshot_max_message_size());
        assert!(
            net.http2_initial_stream_window_size as usize >= net.snapshot_chunk_size,
            "flow-control window must not be smaller than a chunk"
        );

        // An oversized chunk is clamped before it reaches the wire.
        let huge = RaftConfig {
            snapshot_chunk_size: 64 * 1024 * 1024,
            ..Default::default()
        };
        let net = RaftNetworkConfig::from_raft_config(&huge);
        assert_eq!(net.snapshot_chunk_size, MAX_SNAPSHOT_CHUNK_SIZE);
    }

    #[test]
    fn test_config_clone() {
        let config1 = RaftConfig::default();
        let config2 = config1.clone();

        assert_eq!(config1.election_timeout_ms, config2.election_timeout_ms);
        assert_eq!(config1.heartbeat_interval_ms, config2.heartbeat_interval_ms);
        assert_eq!(config1.data_dir, config2.data_dir);
    }

    #[tokio::test]
    async fn test_ensure_dirs() {
        let temp_dir = std::env::temp_dir().join(format!("raft_test_{}", std::process::id()));
        let config = RaftConfig {
            data_dir: temp_dir.clone(),
            ..Default::default()
        };

        // Should succeed creating directories
        assert!(config.ensure_dirs().await.is_ok());

        // Verify directories exist
        assert!(temp_dir.exists());
        assert!(config.log_dir().exists());
        assert!(config.state_machine_dir().exists());
        assert!(config.snapshot_dir().exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
