//! RocksDB-based log storage for Raft.
//!
//! Implements openraft's storage traits using RocksDB for persistence.

use crate::bincode;
use std::fmt::Debug;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
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

/// Look up a column family inside a blocking task, where `self` is not
/// available. Both CFs are created in `with_options`, so a missing one is a
/// corrupted database rather than a recoverable condition.
fn required_cf<'a>(db: &'a DB, name: &str) -> Result<&'a ColumnFamily, std::io::Error> {
    db.cf_handle(name)
        .ok_or_else(|| io_error(format!("{name} column family missing")))
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

    /// Load cached values from storage
    async fn load_cached_values(&self) -> Result<(), std::io::Error> {
        // Load vote
        let vote = self.load_vote_internal()?;
        *self.vote.write().await = vote;

        // Load last purged
        let last_purged = self.load_last_purged_internal()?;
        *self.last_purged.write().await = last_purged;

        // Calculate last log ID
        let last_log_id = self.calculate_last_log_id()?;
        *self.last_log_id.write().await = last_log_id;

        Ok(())
    }

    /// Get column family handle for logs
    /// Panics are acceptable here since column families are created during DB initialization.
    /// If they don't exist, it indicates a severe DB corruption that cannot be recovered.
    fn cf_logs(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_LOGS)
            .expect("CF_LOGS must exist - database may be corrupted")
    }

    /// Get column family handle for state
    fn cf_state(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_STATE)
            .expect("CF_STATE must exist - database may be corrupted")
    }

    /// Encode log index to bytes (big-endian for proper ordering)
    /// This operation is infallible since we're writing to a pre-allocated Vec
    fn encode_log_index(index: u64) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8);
        // Safe: writing to a Vec with sufficient capacity always succeeds
        buf.write_u64::<BigEndian>(index)
            .expect("Writing to Vec should never fail");
        buf
    }

    /// Decode log index from bytes
    /// Returns 0 if bytes are invalid (defensive fallback)
    #[allow(dead_code)]
    fn decode_log_index(bytes: &[u8]) -> u64 {
        let mut cursor = std::io::Cursor::new(bytes);
        cursor.read_u64::<BigEndian>().unwrap_or_else(|e| {
            tracing::error!("Failed to decode log index: {}", e);
            0
        })
    }

    /// Serialize an entry using bincode (3-5x faster than JSON, 40-60% smaller).
    fn serialize_entry(entry: &Entry) -> Result<Vec<u8>, std::io::Error> {
        bincode::serialize(entry).map_err(io_error)
    }

    /// Deserialize an entry from bincode.
    fn deserialize_entry(bytes: &[u8]) -> Result<Entry, std::io::Error> {
        bincode::deserialize(bytes).map_err(io_error)
    }

    /// Load vote from storage
    fn load_vote_internal(&self) -> Result<Option<Vote>, std::io::Error> {
        match self.db.get_cf(self.cf_state(), KEY_VOTE) {
            Ok(Some(bytes)) => {
                let vote: Vote = bincode::deserialize(&bytes).map_err(io_error)?;
                Ok(Some(vote))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(io_error(e)),
        }
    }

    /// Load last purged log ID
    fn load_last_purged_internal(&self) -> Result<Option<LogId>, std::io::Error> {
        match self.db.get_cf(self.cf_state(), KEY_LAST_PURGED) {
            Ok(Some(bytes)) => {
                let log_id: LogId = bincode::deserialize(&bytes).map_err(io_error)?;
                Ok(Some(log_id))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(io_error(e)),
        }
    }

    /// Calculate the last log ID by scanning the log store
    fn calculate_last_log_id(&self) -> Result<Option<LogId>, std::io::Error> {
        let mut iter = self.db.raw_iterator_cf(self.cf_logs());
        iter.seek_to_last();

        if iter.valid()
            && let Some(value) = iter.value()
        {
            let entry = Self::deserialize_entry(value)?;
            return Ok(Some(entry.log_id));
        }

        Ok(None)
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

        let mut entries = Vec::new();
        let start_key = Self::encode_log_index(start);
        let end_key = Self::encode_log_index(end);

        let mut iter = self.db.raw_iterator_cf(self.cf_logs());
        iter.seek(&start_key);

        while iter.valid() {
            if let (Some(key), Some(value)) = (iter.key(), iter.value()) {
                if key >= end_key.as_slice() {
                    break;
                }
                let entry = Self::deserialize_entry(value)?;
                entries.push(entry);
            }
            iter.next();
        }

        debug!("Read {} log entries from range {:?}", entries.len(), range);
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

        Ok(LogState {
            last_purged_log_id: last_purged,
            last_log_id,
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
        blocking_write(move || {
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
            db.write_opt(batch, &sync_write_options()).map_err(io_error)
        })
        .await?;

        // Update cached last log ID
        *self.last_log_id.write().await = last_log_id;

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

        *self.last_purged.write().await = Some(log_id);

        debug!("Purged logs up to index {}", log_id.index);
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
}
