//! Raft configuration.
//!
//! Provides configuration settings for the Raft consensus implementation.
//! Consensus timing is derived centrally by [`RaftTiming`]; the upstream
//! comparison and the full derivation table live in `docs/raft-timing.md`.

use std::path::PathBuf;
use std::time::Duration;

/// Configuration for Raft consensus protocol
#[derive(Clone, Debug)]
pub struct RaftConfig {
    /// Minimum election timeout in milliseconds (library default: 3000ms).
    /// If a follower doesn't hear from leader within this time, it starts an election.
    ///
    /// `to_openraft_config` maps this to `election_timeout_min` and derives
    /// `election_timeout_max` as `2 * this`. The server does NOT use the
    /// library default: `batata-server-common`'s typed config supplies 1000ms
    /// through `startup/persistence.rs`.
    pub election_timeout_ms: u64,

    /// Heartbeat interval in milliseconds (library default: 300ms).
    /// Leader sends heartbeats at this interval. openraft requires it to be
    /// strictly smaller than `election_timeout_min`.
    pub heartbeat_interval_ms: u64,

    /// Snapshot interval in seconds (default: 1800s = 30 minutes)
    /// How often to take snapshots for log compaction
    pub snapshot_interval_secs: u64,

    /// Snapshot threshold - number of log entries before triggering snapshot
    pub snapshot_threshold: u64,

    /// RPC request timeout in milliseconds (default: 5000ms).
    ///
    /// Scope: the channel-level ceiling for Raft gRPC calls, the fallback when
    /// openraft supplies a zero `RPCOption` TTL, the snapshot-timeout floor,
    /// and the `R` term in the election-cycle derivation. It does NOT bound an
    /// individual heartbeat/AppendEntries/Vote RPC: openraft 0.10.1 passes
    /// `heartbeat_interval` as `RPCOption::hard_ttl()` and the transport must
    /// honour `soft_ttl()`. See `docs/raft-timing.md`.
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

/// Deadlines for the write/forwarding path, all derived from one `RaftConfig`.
///
/// Previously these were unrelated constants in `node.rs` (15s / 5s / 5s),
/// so changing the election timeout could leave a wait shorter than the
/// election window it was meant to cover. One "election cycle" is one maximum
/// election window plus one RPC round; it plays the same role as Consul's
/// `rpc_hold_timeout` (7s for a 5s election window). See `docs/raft-timing.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RaftTiming {
    /// `election_timeout_min` handed to openraft.
    pub election_timeout_min: Duration,
    /// `election_timeout_max` handed to openraft (`2 * min`).
    pub election_timeout_max: Duration,
    /// One maximum election window plus one RPC round.
    pub election_cycle: Duration,
    /// Bounded wait for a leader to be discovered during an election.
    pub leaderless_wait: Duration,
    /// Bounded wait for a follower to apply a forwarded write locally.
    pub apply_wait: Duration,
    /// Overall deadline for a local `client_write`; two cycles cover a
    /// leadership change that happens while the write is in flight.
    pub client_write: Duration,
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

    /// Get RPC timeout as Duration.
    ///
    /// Channel ceiling and fallback for Raft RPCs; the per-request deadline is
    /// `RPCOption::soft_ttl()`, not this value. See the field documentation.
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

    /// Derive every consensus deadline from this configuration.
    ///
    /// Uses saturating arithmetic so a malformed config can never panic here;
    /// call [`RaftConfig::validate`] first to reject malformed values.
    pub fn timing(&self) -> RaftTiming {
        let election_min = Duration::from_millis(self.election_timeout_ms);
        let election_max = Duration::from_millis(self.election_timeout_ms.saturating_mul(2));
        let election_cycle = election_max.saturating_add(self.rpc_timeout());
        RaftTiming {
            election_timeout_min: election_min,
            election_timeout_max: election_max,
            election_cycle,
            leaderless_wait: election_cycle,
            apply_wait: election_cycle,
            client_write: election_cycle.saturating_mul(2),
        }
    }

    /// Check the timing invariants openraft and the write path rely on.
    ///
    /// Pure: T1 only publishes this contract, it does not change start-up
    /// behaviour. `RaftNode::new_with_full_options` calls it once the derived
    /// deadlines are wired into `node.rs`.
    pub fn validate(&self) -> Result<(), String> {
        let election = self.election_timeout_ms;
        let heartbeat = self.heartbeat_interval_ms;

        if heartbeat == 0 {
            return Err("heartbeat_interval_ms must be greater than zero".into());
        }
        // openraft 0.10 Config::validate rejects
        // `election_timeout_min <= heartbeat_interval`; fail early with an
        // actionable message instead of deep inside Raft::new.
        if election <= heartbeat {
            return Err(format!(
                "election_timeout_ms ({election}) must be greater than heartbeat_interval_ms ({heartbeat})"
            ));
        }
        if self.rpc_request_timeout_ms == 0 {
            return Err("rpc_request_timeout_ms must be greater than zero".into());
        }
        let Some(election_max) = election.checked_mul(2) else {
            return Err("election_timeout_ms overflows when doubled".into());
        };
        let Some(cycle) = election_max.checked_add(self.rpc_request_timeout_ms) else {
            return Err("election cycle overflows when adding rpc_request_timeout_ms".into());
        };
        if cycle.checked_mul(2).is_none() {
            return Err("client_write deadline overflows".into());
        }
        if self.snapshot_transfer_timeout_ms <= self.rpc_request_timeout_ms {
            return Err(format!(
                "snapshot_transfer_timeout_ms ({}) must exceed rpc_request_timeout_ms ({})",
                self.snapshot_transfer_timeout_ms, self.rpc_request_timeout_ms
            ));
        }
        // openraft 0.10.1 hands `heartbeat_interval` to the network as the
        // `RPCOption::hard_ttl()` for heartbeat and streaming replication
        // (`core/heartbeat/worker.rs`, `replication/mod.rs`). `rpc_timeout` is
        // only a channel-level ceiling: if it were tighter than the heartbeat,
        // the tonic endpoint timeout would fire before the transport could
        // honour `soft_ttl()`, and every heartbeat would look like a network
        // failure. Keep this check after the snapshot check so the existing
        // diagnostics for that pair stay specific.
        if self.rpc_request_timeout_ms <= heartbeat {
            return Err(format!(
                "rpc_request_timeout_ms ({}) must exceed heartbeat_interval_ms ({}) so the channel timeout cannot preempt openraft's soft TTL",
                self.rpc_request_timeout_ms, heartbeat
            ));
        }
        if self.forward_max_retries > 0 && self.forward_initial_delay_ms == 0 {
            return Err(
                "forward_initial_delay_ms must be greater than zero when forwarding is enabled"
                    .into(),
            );
        }

        // Documented invariants of the derived budgets. A future formula that
        // drops the election window from `leaderless_wait` must fail here.
        let timing = self.timing();
        if timing.leaderless_wait < timing.election_timeout_max {
            return Err("leaderless_wait must cover at least one maximum election window".into());
        }
        if timing.client_write <= timing.leaderless_wait {
            return Err("client_write deadline must exceed the leaderless wait".into());
        }
        Ok(())
    }

    /// Convert to openraft Config
    pub fn to_openraft_config(&self) -> openraft::Config {
        openraft::Config {
            cluster_name: "batata".to_string(),
            election_timeout_min: self.election_timeout_ms,
            election_timeout_max: self.election_timeout_ms * 2,
            heartbeat_interval: self.heartbeat_interval_ms,
            // A follower that briefly loses heartbeats (for example while the
            // process is busy on synchronous storage I/O) must not immediately
            // raise its term and disrupt a healthy leader. openraft leaves this
            // `Option` as `None`, and `get_enable_pre_vote()` treats `None` as
            // `false`. See `docs/raft-timing.md` §6.1 for the measured failure.
            enable_pre_vote: Some(true),
            // openraft defaults this `Option` to `None`, which
            // `Config::enable_leader_restore()` reads as `true`: a restarted
            // former leader campaigns immediately in a higher term without
            // waiting for an election timeout. The upstream rationale is to
            // avoid reusing a log id when the old leader replicated an entry
            // before its local flush; `RocksLogStore::append` fsyncs every
            // batch before returning and fires `IOFlushed` only afterwards,
            // so that window does not exist here. The regression matrix
            // restarts nodes between its six combinations, where an immediate
            // campaign only inflates terms. With PreVote enabled the restarted
            // node instead goes through the normal pre-elect/election timing.
            // See `docs/raft-timing.md` §4.
            enable_leader_restore: Some(false),
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

    /// openraft itself validates the converted config. Both default sets --
    /// the library default (3000/300) and the production typed default
    /// (1000/200) -- must be accepted, otherwise Raft::new fails at startup.
    #[test]
    fn openraft_accepts_both_default_timing_sets() {
        for (election, heartbeat) in [(3000, 300), (1000, 200)] {
            let config = RaftConfig {
                election_timeout_ms: election,
                heartbeat_interval_ms: heartbeat,
                ..Default::default()
            };
            assert!(config.validate().is_ok(), "{election}/{heartbeat}");
            assert!(
                config.to_openraft_config().validate().is_ok(),
                "{election}/{heartbeat}"
            );
        }
    }

    /// All deadlines are derived from the election window and the RPC round.
    /// The old hardcoded 5s leaderless budget was shorter than the library
    /// default's 6s election max -- the exact mismatch T3 removes.
    #[test]
    fn timing_is_derived_from_election_window_and_rpc_round() {
        let timing = RaftConfig::default().timing();
        assert_eq!(timing.election_timeout_min, Duration::from_millis(3000));
        assert_eq!(timing.election_timeout_max, Duration::from_millis(6000));
        assert_eq!(timing.election_cycle, Duration::from_millis(11000));
        assert_eq!(timing.leaderless_wait, Duration::from_millis(11000));
        assert_eq!(timing.apply_wait, Duration::from_millis(11000));
        assert_eq!(timing.client_write, Duration::from_millis(22000));
        assert!(timing.leaderless_wait > Duration::from_millis(6000));
        assert!(timing.client_write > timing.leaderless_wait);
    }

    /// Production defaults: 1000ms election min -> 2000ms max, plus the
    /// 5000ms RPC round gives a 7s election cycle (Consul's rpc_hold_timeout
    /// is 7s for a 5s election window).
    #[test]
    fn production_timing_covers_the_election_window() {
        let config = RaftConfig {
            election_timeout_ms: 1000,
            heartbeat_interval_ms: 200,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
        let timing = config.timing();
        assert_eq!(timing.election_timeout_max, Duration::from_millis(2000));
        assert_eq!(timing.election_cycle, Duration::from_millis(7000));
        assert_eq!(timing.leaderless_wait, Duration::from_millis(7000));
        assert!(timing.leaderless_wait >= timing.election_timeout_max);
    }

    /// Every invariant has a test, so removing a guard from `validate()`
    /// turns at least one case green that must stay red.
    #[test]
    fn validate_rejects_invalid_timing_combinations() {
        let cases: Vec<(RaftConfig, &str)> = vec![
            (
                RaftConfig {
                    heartbeat_interval_ms: 0,
                    ..Default::default()
                },
                "heartbeat_interval_ms",
            ),
            (
                RaftConfig {
                    election_timeout_ms: 200,
                    heartbeat_interval_ms: 200,
                    ..Default::default()
                },
                "election_timeout_ms",
            ),
            (
                RaftConfig {
                    rpc_request_timeout_ms: 0,
                    ..Default::default()
                },
                "rpc_request_timeout_ms",
            ),
            (
                RaftConfig {
                    election_timeout_ms: u64::MAX,
                    ..Default::default()
                },
                "overflow",
            ),
            (
                RaftConfig {
                    snapshot_transfer_timeout_ms: 100,
                    rpc_request_timeout_ms: 100,
                    ..Default::default()
                },
                "snapshot_transfer_timeout_ms",
            ),
            (
                RaftConfig {
                    forward_initial_delay_ms: 0,
                    ..Default::default()
                },
                "forward_initial_delay_ms",
            ),
        ];
        for (config, needle) in cases {
            let err = config
                .validate()
                .expect_err("invalid timing combination must be rejected");
            assert!(err.contains(needle), "expected {needle:?} in {err:?}");
        }
    }

    /// `rpc_request_timeout_ms` is a channel ceiling, not a per-RPC deadline.
    /// openraft derives `RPCOption::hard_ttl()` from `heartbeat_interval`; a
    /// channel timeout at or below that would preempt the transport's
    /// `soft_ttl()` and turn healthy heartbeats into spurious RPC failures.
    /// Removing the guard in `validate()` must make this test fail.
    #[test]
    fn validate_rejects_rpc_timeout_tighter_than_heartbeat() {
        let equal = RaftConfig {
            rpc_request_timeout_ms: 300,
            ..Default::default()
        };
        let err = equal
            .validate()
            .expect_err("a channel timeout equal to the heartbeat must be rejected");
        assert!(err.contains("must exceed heartbeat_interval_ms"), "{err}");

        let smaller = RaftConfig {
            rpc_request_timeout_ms: 100,
            ..Default::default()
        };
        let err = smaller
            .validate()
            .expect_err("a channel timeout below the heartbeat must be rejected");
        assert!(err.contains("must exceed heartbeat_interval_ms"), "{err}");

        // Success path: one millisecond above the heartbeat is valid.
        let valid = RaftConfig {
            rpc_request_timeout_ms: 301,
            ..Default::default()
        };
        assert!(valid.validate().is_ok());
    }

    /// Both real default sets satisfy the new invariant; the production values
    /// are the ones the regression matrix actually runs.
    #[test]
    fn production_and_library_defaults_keep_rpc_timeout_above_heartbeat() {
        for (election, heartbeat) in [(3000, 300), (1000, 200)] {
            let config = RaftConfig {
                election_timeout_ms: election,
                heartbeat_interval_ms: heartbeat,
                ..Default::default()
            };
            assert!(
                config.rpc_request_timeout_ms > heartbeat,
                "{election}/{heartbeat}"
            );
            assert!(config.validate().is_ok(), "{election}/{heartbeat}");
        }
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
        // `None` is treated as disabled by openraft; a transient stall would
        // then turn into a term bump on every missed heartbeat.
        assert_eq!(openraft_config.enable_pre_vote, Some(true));
        assert_eq!(openraft_config.enable_leader_restore, Some(false));
    }

    /// openraft 0.10.1 reads `None` as `true` for `enable_leader_restore`, so
    /// a restarted former leader campaigns immediately in a higher term. The
    /// regression matrix restarts nodes between its six combinations, where
    /// that only inflates terms. Removing the explicit `Some(false)` must turn
    /// this test red.
    #[test]
    fn to_openraft_config_disables_immediate_leader_restore() {
        let config = RaftConfig::default();
        let openraft_config = config.to_openraft_config();
        assert_eq!(openraft_config.enable_leader_restore, Some(false));
        // PreVote is what makes the normal election path safe after a restart.
        assert_eq!(openraft_config.enable_pre_vote, Some(true));
        assert!(openraft_config.validate().is_ok());

        // Document the upstream default this guard exists to override.
        assert_eq!(openraft::Config::default().enable_leader_restore, None);
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
