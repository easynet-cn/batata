//! RocksDB-based log storage for Raft.
//!
//! Implements openraft's storage traits using RocksDB for persistence.

use crate::bincode;
use std::fmt::Debug;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

use openraft::storage::{IOFlushed, LogState, RaftLogStorage};
use openraft::{OptionalSend, RaftLogReader};
use rocksdb::{BlockBasedOptions, ColumnFamily, ColumnFamilyDescriptor, DB, Options};
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::types::{Entry, LogId, TypeConfig, Vote};

// Column family names
const CF_LOGS: &str = "logs";
const CF_STATE: &str = "state";

// State keys
const KEY_VOTE: &[u8] = b"vote";
const KEY_LAST_PURGED: &[u8] = b"last_purged";

// Default RocksDB performance tuning constants (used when no custom options provided)
const DEFAULT_WRITE_BUFFER_SIZE: usize = 64 * 1024 * 1024;
const DEFAULT_MAX_WRITE_BUFFER_NUMBER: i32 = 3;
const DEFAULT_BLOCK_CACHE_SIZE: usize = 256 * 1024 * 1024;
const DEFAULT_BLOOM_FILTER_BITS_PER_KEY: f64 = 10.0;

/// Write options for Raft votes and log entries.
///
/// openraft requires these writes to reach stable storage before the call
/// returns. A vote that only made it to the page cache is lost on power
/// failure, and the node may then vote again in the same term, which elects
/// two leaders; log entries that were acknowledged to the leader but not
/// flushed are lost the same way.
fn sync_write_options() -> rocksdb::WriteOptions {
    let mut opts = rocksdb::WriteOptions::default();
    opts.set_sync(true);
    opts
}

/// openraft 0.10 storage traits return `io::Error`; the subject/verb context
/// is attached by openraft itself, so preserve only the underlying error here.
fn io_error(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

/// Run a durable RocksDB write on Tokio's blocking pool.
///
/// Every write below uses `set_sync(true)`, i.e. an fsync. `RocksLogStore` is
/// driven by openraft's `RaftCore`, which -- like the Raft gRPC server, the
/// election deadline notifier and the heartbeat worker -- is spawned with
/// `tokio::spawn` on the actix `current_thread` runtime. One inline fsync (or
/// a compaction-induced write stall) therefore freezes *every* task on that
/// thread: heartbeats, AppendEntries replies and election timers all miss
/// their deadline, and openraft's RPC budget is only one heartbeat interval.
///
/// Awaiting the blocking pool keeps the write on the durability path (the
/// `IOFlushed` callback is still signalled only after the fsync) and keeps the
/// writes serialised, because `RaftCore` awaits each storage call in order.
/// The difference is that the runtime thread stays free to poll other tasks
/// while the disk is busy.
async fn blocking_write<F, T>(f: F) -> Result<T, std::io::Error>
where
    F: FnOnce() -> Result<T, std::io::Error> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| io_error(format!("RocksDB write task failed: {e}")))?
}

/// Run a synchronous RocksDB read on Tokio's blocking pool.
///
/// Reads are cheaper than fsyncs but still hit the disk, iterate memtables and
/// decompress blocks. On the actix `current_thread` runtime the log reader is
/// polled on the same `main` thread as the heartbeat worker and the election
/// deadline notifier, so a slow `try_get_log_entries` (for example while a
/// follower is catching up after a compaction stall) stalls all of them.
async fn blocking_read<F, T>(f: F) -> Result<T, std::io::Error>
where
    F: FnOnce() -> Result<T, std::io::Error> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| io_error(format!("RocksDB read task failed: {e}")))?
}

/// Look up a column family inside a blocking task, where `self` is not
/// available. Both CFs are created in `with_options`, so a missing one is a
/// corrupted database rather than a recoverable condition.
fn required_cf<'a>(db: &'a DB, name: &str) -> Result<&'a ColumnFamily, std::io::Error> {
    db.cf_handle(name)
        .ok_or_else(|| io_error(format!("{name} column family missing")))
}

/// Read the persisted vote; callable from a blocking task with only `&DB`.
fn load_vote_from(db: &DB) -> Result<Option<Vote>, std::io::Error> {
    let cf = required_cf(db, CF_STATE)?;
    match db.get_cf(cf, KEY_VOTE) {
        Ok(Some(bytes)) => Ok(Some(bincode::deserialize(&bytes).map_err(io_error)?)),
        Ok(None) => Ok(None),
        Err(e) => Err(io_error(e)),
    }
}

/// Read the persisted last-purged log id from a blocking task.
fn load_last_purged_from(db: &DB) -> Result<Option<LogId>, std::io::Error> {
    let cf = required_cf(db, CF_STATE)?;
    match db.get_cf(cf, KEY_LAST_PURGED) {
        Ok(Some(bytes)) => Ok(Some(bincode::deserialize(&bytes).map_err(io_error)?)),
        Ok(None) => Ok(None),
        Err(e) => Err(io_error(e)),
    }
}

/// Find the last stored log entry by seeking to the last key in the log CF.
fn last_log_id_from(db: &DB) -> Result<Option<LogId>, std::io::Error> {
    let cf = required_cf(db, CF_LOGS)?;
    let mut iter = db.raw_iterator_cf(cf);
    iter.seek_to_last();

    if iter.valid()
        && let Some(value) = iter.value()
    {
        let entry: Entry = bincode::deserialize(value).map_err(io_error)?;
        return Ok(Some(entry.log_id));
    }

    Ok(None)
}

/// RocksDB-based log store for Raft
pub struct RocksLogStore {
    db: Arc<DB>,
    /// Cached last log ID for performance
    last_log_id: RwLock<Option<LogId>>,
    /// Cached vote
    vote: RwLock<Option<Vote>>,
    /// Cached last purged log ID
    last_purged: RwLock<Option<LogId>>,
}

impl RocksLogStore {
    /// Create a new RocksDB log store with default configuration
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        Self::with_options(path, None, None).await
    }

    /// Create a new RocksDB log store with pre-configured RocksDB options.
    ///
    /// Pass `None` for either parameter to use sensible defaults.
    pub async fn with_options<P: AsRef<Path>>(
        path: P,
        custom_db_opts: Option<Options>,
        custom_cf_opts: Option<Options>,
    ) -> Result<Self, std::io::Error> {
        let db_opts = custom_db_opts.unwrap_or_else(|| {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.create_missing_column_families(true);
            opts.set_write_buffer_size(DEFAULT_WRITE_BUFFER_SIZE);
            opts.set_max_write_buffer_number(DEFAULT_MAX_WRITE_BUFFER_NUMBER);
            opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
            opts.set_bottommost_compression_type(rocksdb::DBCompressionType::Zstd);
            opts
        });

        let cf_opts = custom_cf_opts.unwrap_or_else(|| {
            let mut opts = Options::default();
            opts.set_write_buffer_size(DEFAULT_WRITE_BUFFER_SIZE);
            opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
            opts.set_bottommost_compression_type(rocksdb::DBCompressionType::Zstd);
            let mut block_opts = BlockBasedOptions::default();
            let cache = rocksdb::Cache::new_lru_cache(DEFAULT_BLOCK_CACHE_SIZE);
            block_opts.set_block_cache(&cache);
            block_opts.set_bloom_filter(DEFAULT_BLOOM_FILTER_BITS_PER_KEY, false);
            opts.set_block_based_table_factory(&block_opts);
            opts
        });

        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_LOGS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_STATE, cf_opts),
        ];

        let db = DB::open_cf_descriptors(&db_opts, path, cfs).map_err(io_error)?;

        let store = Self {
            db: Arc::new(db),
            last_log_id: RwLock::new(None),
            vote: RwLock::new(None),
            last_purged: RwLock::new(None),
        };

        // Initialize cached values from storage asynchronously
        store.load_cached_values().await?;

        info!("RocksDB log store initialized");
        Ok(store)
    }

    /// Load cached values from storage on the blocking pool.
    ///
    /// All three reads share one task: startup is not latency critical, and
    /// doing them inline used to block the single-threaded runtime before the
    /// node had even joined the cluster.
    async fn load_cached_values(&self) -> Result<(), std::io::Error> {
        let db = self.db.clone();
        let (vote, last_purged, last_log_id) = blocking_read(move || {
            Ok((
                load_vote_from(&db)?,
                load_last_purged_from(&db)?,
                last_log_id_from(&db)?,
            ))
        })
        .await?;

        *self.vote.write().await = vote;
        *self.last_purged.write().await = last_purged;
        *self.last_log_id.write().await = last_log_id;
        Ok(())
    }

    /// Encode log index to bytes (big-endian for proper ordering)
    fn encode_log_index(index: u64) -> Vec<u8> {
        index.to_be_bytes().to_vec()
    }

    /// Decode log index from bytes
    /// Returns 0 if bytes are invalid (defensive fallback)
    #[allow(dead_code)]
    fn decode_log_index(bytes: &[u8]) -> u64 {
        let arr: [u8; 8] = match bytes.try_into() {
            Ok(arr) => arr,
            Err(_) => {
                tracing::error!(
                    "Failed to decode log index: expected 8 bytes, got {}",
                    bytes.len()
                );
                return 0;
            }
        };
        u64::from_be_bytes(arr)
    }

    /// Serialize an entry using bincode (3-5x faster than JSON, 40-60% smaller).
    fn serialize_entry(entry: &Entry) -> Result<Vec<u8>, std::io::Error> {
        bincode::serialize(entry).map_err(io_error)
    }

    /// Deserialize an entry from bincode.
    fn deserialize_entry(bytes: &[u8]) -> Result<Entry, std::io::Error> {
        bincode::deserialize(bytes).map_err(io_error)
    }
}

impl RaftLogReader<TypeConfig> for RocksLogStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry>, std::io::Error> {
        let start = match range.start_bound() {
            std::ops::Bound::Included(&n) => n,
            std::ops::Bound::Excluded(&n) => n + 1,
            std::ops::Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            std::ops::Bound::Included(&n) => n + 1,
            std::ops::Bound::Excluded(&n) => n,
            std::ops::Bound::Unbounded => u64::MAX,
        };

        let db = self.db.clone();
        let entries = blocking_read(move || {
            let cf = required_cf(&db, CF_LOGS)?;
            let start_key = RocksLogStore::encode_log_index(start);
            let end_key = RocksLogStore::encode_log_index(end);

            let mut entries = Vec::new();
            let mut iter = db.raw_iterator_cf(cf);
            iter.seek(&start_key);
            while iter.valid() {
                if let (Some(key), Some(value)) = (iter.key(), iter.value()) {
                    if key >= end_key.as_slice() {
                        break;
                    }
                    entries.push(RocksLogStore::deserialize_entry(value)?);
                }
                iter.next();
            }
            Ok(entries)
        })
        .await?;

        debug!(
            "Read {} log entries from range {}..{}",
            entries.len(),
            start,
            end
        );
        Ok(entries)
    }

    async fn read_vote(&mut self) -> Result<Option<Vote>, std::io::Error> {
        Ok(*self.vote.read().await)
    }
}

impl RaftLogStorage<TypeConfig> for RocksLogStore {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, std::io::Error> {
        let last_purged = *self.last_purged.read().await;
        let last_log_id = *self.last_log_id.read().await;

        // `purge()` may advance past the last entry actually stored (openraft
        // allows purging up to a log id that was never appended). In that case
        // the purged marker *is* the last known log id: reporting an older
        // entry (or None after a restart, when the log CF is empty) makes
        // openraft believe the node lost entries it had already compacted.
        // Keep the two fields consistent even after a restart, where the
        // in-memory last_log_id comes from a now-empty log CF.
        let effective_last = match (last_log_id, last_purged) {
            (Some(stored), Some(purged)) if purged.index >= stored.index => Some(purged),
            (None, Some(purged)) => Some(purged),
            (other, _) => other,
        };

        Ok(LogState {
            last_purged_log_id: last_purged,
            last_log_id: effective_last,
        })
    }

    async fn save_vote(&mut self, vote: &Vote) -> Result<(), std::io::Error> {
        let bytes = bincode::serialize(vote).map_err(io_error)?;
        let db = self.db.clone();
        blocking_write(move || {
            let cf = required_cf(&db, CF_STATE)?;
            db.put_cf_opt(cf, KEY_VOTE, &bytes, &sync_write_options())
                .map_err(io_error)
        })
        .await?;

        *self.vote.write().await = Some(*vote);
        debug!("Saved vote: {:?}", vote);
        Ok(())
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        // Clone the store for reading
        // Since we use Arc<DB>, cloning is cheap
        RocksLogStore {
            db: self.db.clone(),
            last_log_id: RwLock::new(*self.last_log_id.read().await),
            vote: RwLock::new(*self.vote.read().await),
            last_purged: RwLock::new(*self.last_purged.read().await),
        }
    }

    async fn append<I>(
        &mut self,
        entries: I,
        callback: IOFlushed<TypeConfig>,
    ) -> Result<(), std::io::Error>
    where
        I: IntoIterator<Item = Entry> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let entries: Vec<_> = entries.into_iter().collect();
        if entries.is_empty() {
            callback.io_completed(Ok(()));
            return Ok(());
        }

        let last_log_id = entries.last().map(|entry| entry.log_id);
        // Serialize before crossing into the blocking task so only plain byte
        // vectors (unambiguously `Send`) are moved across the boundary.
        let prepared = entries
            .iter()
            .map(|entry| {
                Ok((
                    Self::encode_log_index(entry.log_id.index),
                    Self::serialize_entry(entry)?,
                ))
            })
            .collect::<Result<Vec<(Vec<u8>, Vec<u8>)>, std::io::Error>>()?;
        let appended = prepared.len();

        let db = self.db.clone();
        blocking_write(move || {
            let cf = required_cf(&db, CF_LOGS)?;
            let mut batch = rocksdb::WriteBatch::default();
            for (key, value) in &prepared {
                batch.put_cf(cf, key, value);
            }
            db.write_opt(batch, &sync_write_options()).map_err(io_error)
        })
        .await?;

        if let Some(log_id) = last_log_id {
            *self.last_log_id.write().await = Some(log_id);
        }

        if appended > 1 {
            debug!("Appended {} log entries", appended);
        }
        callback.io_completed(Ok(()));
        Ok(())
    }

    async fn truncate_after(&mut self, last_log_id: Option<LogId>) -> Result<(), std::io::Error> {
        // Keep entries up to and including last_log_id; None clears all logs.
        let start = last_log_id.map(|id| id.index + 1).unwrap_or(0);
        let start_key = Self::encode_log_index(start);

        let db = self.db.clone();
        let actual_last_log_id = blocking_write(move || {
            let cf = required_cf(&db, CF_LOGS)?;
            let mut batch = rocksdb::WriteBatch::default();
            let mut iter = db.raw_iterator_cf(cf);
            iter.seek(&start_key);
            while iter.valid() {
                if let Some(key) = iter.key() {
                    batch.delete_cf(cf, key);
                }
                iter.next();
            }
            db.write_opt(batch, &sync_write_options())
                .map_err(io_error)?;
            // The truncation point may be beyond the last stored entry (or the
            // log may be emptied entirely). openraft expects get_log_state()
            // to report the last entry that actually remains, not the bound
            // that was requested. Recompute from the DB rather than trusting
            // the argument.
            last_log_id_from(&db)
        })
        .await?;

        // Update cached last log ID to what is really stored after truncation.
        *self.last_log_id.write().await = actual_last_log_id;

        debug!("Truncated logs after {:?}", last_log_id);
        Ok(())
    }

    async fn purge(&mut self, log_id: LogId) -> Result<(), std::io::Error> {
        // Delete all entries with index <= log_id.index
        let end_key = Self::encode_log_index(log_id.index + 1);

        let last_purged_bytes = bincode::serialize(&log_id).map_err(io_error)?;
        let db = self.db.clone();
        blocking_write(move || {
            let cf_logs = required_cf(&db, CF_LOGS)?;
            let cf_state = required_cf(&db, CF_STATE)?;
            let mut batch = rocksdb::WriteBatch::default();
            let mut iter = db.raw_iterator_cf(cf_logs);
            iter.seek_to_first();

            while iter.valid() {
                if let Some(key) = iter.key() {
                    if key >= end_key.as_slice() {
                        break;
                    }
                    batch.delete_cf(cf_logs, key);
                }
                iter.next();
            }

            // Save last purged log ID in the same durable batch.
            batch.put_cf(cf_state, KEY_LAST_PURGED, &last_purged_bytes);
            db.write_opt(batch, &sync_write_options()).map_err(io_error)
        })
        .await?;

        let purged_index = log_id.index;
        *self.last_purged.write().await = Some(log_id);
        // Purging past the last stored entry deletes the whole log CF. Keep
        // the cached last log id at the purged marker so get_log_state() does
        // not report an entry that no longer exists (or a stale smaller one).
        let mut last = self.last_log_id.write().await;
        if last
            .as_ref()
            .is_none_or(|current| log_id.index >= current.index)
        {
            *last = Some(log_id);
        }

        debug!("Purged logs up to index {}", purged_index);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_log_index() {
        let index = 12345u64;
        let encoded = RocksLogStore::encode_log_index(index);
        let decoded = RocksLogStore::decode_log_index(&encoded);
        assert_eq!(index, decoded);
    }

    #[test]
    fn test_encode_log_index_ordering() {
        // Test that big-endian encoding preserves ordering for RocksDB
        let indices = [0u64, 1, 100, 1000, u64::MAX];
        let encoded: Vec<_> = indices
            .iter()
            .map(|&i| RocksLogStore::encode_log_index(i))
            .collect();

        for i in 0..encoded.len() - 1 {
            assert!(
                encoded[i] < encoded[i + 1],
                "Encoding should preserve ordering"
            );
        }
    }

    /// The whole point of `blocking_write` is that the fsync does not run on
    /// the (single-threaded) async worker. Replacing `spawn_blocking` with a
    /// direct call must turn this test red.
    #[tokio::test(flavor = "current_thread")]
    async fn blocking_write_runs_off_the_async_worker_thread() {
        let async_thread = std::thread::current().id();
        let blocking_thread =
            blocking_write(move || Ok::<_, std::io::Error>(std::thread::current().id()))
                .await
                .expect("blocking write must succeed");
        assert_ne!(
            async_thread, blocking_thread,
            "durable writes must not occupy the async worker thread"
        );
    }

    /// A failed fsync must surface as the same `io::Error` the storage trait
    /// expects, not as a swallowed join error.
    #[tokio::test]
    async fn blocking_write_propagates_io_errors() {
        let err = blocking_write(|| Err::<(), _>(io_error("fsync failed")))
            .await
            .expect_err("write failure must be propagated");
        assert!(err.to_string().contains("fsync failed"));
    }

    /// Reads must not occupy the async worker either: a follower catching up
    /// can iterate thousands of log entries while heartbeats need that same
    /// single-threaded runtime.
    #[tokio::test(flavor = "current_thread")]
    async fn blocking_read_runs_off_the_async_worker_thread() {
        let async_thread = std::thread::current().id();
        let blocking_thread =
            blocking_read(move || Ok::<_, std::io::Error>(std::thread::current().id()))
                .await
                .expect("blocking read must succeed");
        assert_ne!(
            async_thread, blocking_thread,
            "RocksDB reads must not occupy the async worker thread"
        );
    }

    #[tokio::test]
    async fn blocking_read_propagates_io_errors() {
        let err = blocking_read(|| Err::<(), _>(io_error("read failed")))
            .await
            .expect_err("read failure must be propagated");
        assert!(err.to_string().contains("read failed"));
    }

    /// Architecture guard: removing the blocking-pool hop from either read
    /// path must turn this test red, even if the helper itself still exists.
    #[test]
    fn log_store_read_paths_use_the_blocking_pool() {
        let src = include_str!("log_store.rs");

        let entries = src
            .find("async fn try_get_log_entries")
            .expect("try_get_log_entries must exist");
        let entries_body = &src[entries..entries + 2200];
        assert!(
            entries_body.contains("blocking_read("),
            "try_get_log_entries must run its RocksDB iteration on the blocking pool"
        );

        let cached = src
            .find("async fn load_cached_values")
            .expect("load_cached_values must exist");
        let cached_body = &src[cached..cached + 1200];
        assert!(
            cached_body.contains("blocking_read("),
            "startup cache loading must also use the blocking pool"
        );
    }

    /// Success path for the migrated reader: opening an empty store and
    /// reading a range must still work from inside `spawn_blocking`.
    #[tokio::test]
    async fn empty_log_store_reads_return_an_empty_range() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut store = RocksLogStore::new(dir.path())
            .await
            .expect("open log store");
        let entries =
            <RocksLogStore as RaftLogReader<TypeConfig>>::try_get_log_entries(&mut store, 0u64..10)
                .await
                .expect("read log range");
        assert!(entries.is_empty());
    }

    /// openraft permits purging up to a log id that was never appended. That
    /// empties the log CF but persists the purged marker. On restart
    /// `load_cached_values()` rebuilds `last_log_id` from an empty log CF
    /// (None), so `get_log_state()` must fall back to the durable marker
    /// instead of telling openraft that compacted entries were lost.
    #[tokio::test]
    async fn purged_marker_survives_a_restart_as_last_log_id() {
        use crate::raft::types::CommittedLeaderId;
        use openraft::storage::RaftLogStorage;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("raft-log");
        let purged = LogId::new(
            CommittedLeaderId {
                term: 2,
                node_id: 1,
            },
            7,
        );

        {
            let mut store = RocksLogStore::new(path.clone())
                .await
                .expect("open log store");
            store
                .purge(purged)
                .await
                .expect("purge past the last stored entry");
            let state = store.get_log_state().await.expect("live log state");
            assert_eq!(state.last_purged_log_id, Some(purged));
            assert_eq!(state.last_log_id, Some(purged));
        }

        let mut reopened = RocksLogStore::new(path).await.expect("reopen log store");
        let state = reopened.get_log_state().await.expect("reopened log state");
        assert_eq!(state.last_purged_log_id, Some(purged));
        assert_eq!(
            state.last_log_id,
            Some(purged),
            "a restart after compaction must still report the durable purged marker"
        );
    }
}
