// RocksDB-based log storage for Raft
// Implements openraft's storage traits using RocksDB for persistence

// Allow large error types - StorageError is from openraft and follows their design patterns
#![allow(clippy::result_large_err)]

use std::fmt::Debug;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use openraft::storage::{LogFlushed, LogState, RaftLogStorage};
use openraft::{
    Entry, ErrorSubject, ErrorVerb, LogId, OptionalSend, RaftLogReader, StorageError, Vote,
};
use rocksdb::{BlockBasedOptions, ColumnFamily, ColumnFamilyDescriptor, DB, Options};
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::types::{NodeId, TypeConfig};

// Column family names
const CF_LOGS: &str = "logs";
const CF_STATE: &str = "state";

// State keys
const KEY_VOTE: &[u8] = b"vote";
const KEY_LAST_PURGED: &[u8] = b"last_purged";

// RocksDB performance tuning constants
/// Write buffer size: 64MB for better write throughput
const WRITE_BUFFER_SIZE: usize = 64 * 1024 * 1024;
/// Maximum number of write buffers for write stall prevention
const MAX_WRITE_BUFFER_NUMBER: i32 = 3;
/// Block cache size: 256MB for read optimization
const BLOCK_CACHE_SIZE: usize = 256 * 1024 * 1024;
/// Bloom filter bits per key for faster lookups
const BLOOM_FILTER_BITS_PER_KEY: f64 = 10.0;

/// Helper to create StorageError for vote operations
fn vote_error(
    e: impl std::error::Error + Send + Sync + 'static,
    verb: ErrorVerb,
) -> StorageError<NodeId> {
    StorageError::from_io_error(
        ErrorSubject::Vote,
        verb,
        std::io::Error::other(e.to_string()),
    )
}

/// Helper to create StorageError for log operations
fn logs_error(
    e: impl std::error::Error + Send + Sync + 'static,
    verb: ErrorVerb,
) -> StorageError<NodeId> {
    StorageError::from_io_error(
        ErrorSubject::Logs,
        verb,
        std::io::Error::other(e.to_string()),
    )
}

/// RocksDB-based log store for Raft
pub struct RocksLogStore {
    db: Arc<DB>,
    /// Cached last log ID for performance
    last_log_id: RwLock<Option<LogId<NodeId>>>,
    /// Cached vote
    vote: RwLock<Option<Vote<NodeId>>>,
    /// Cached last purged log ID
    last_purged: RwLock<Option<LogId<NodeId>>>,
}

impl RocksLogStore {
    /// Create a new RocksDB log store
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError<NodeId>> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        // Performance optimizations using named constants
        db_opts.set_write_buffer_size(WRITE_BUFFER_SIZE);
        db_opts.set_max_write_buffer_number(MAX_WRITE_BUFFER_NUMBER);
        db_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

        // Block-based table options with block cache for read performance
        let mut block_opts = BlockBasedOptions::default();
        let cache = rocksdb::Cache::new_lru_cache(BLOCK_CACHE_SIZE);
        block_opts.set_block_cache(&cache);
        block_opts.set_bloom_filter(BLOOM_FILTER_BITS_PER_KEY, false);

        // Column family options with same optimizations
        let mut cf_opts = Options::default();
        cf_opts.set_write_buffer_size(WRITE_BUFFER_SIZE);
        cf_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        cf_opts.set_block_based_table_factory(&block_opts);

        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_LOGS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_STATE, cf_opts),
        ];

        let db = DB::open_cf_descriptors(&db_opts, path, cfs)
            .map_err(|e| logs_error(e, ErrorVerb::Read))?;

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
    async fn load_cached_values(&self) -> Result<(), StorageError<NodeId>> {
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

    /// Serialize an entry
    fn serialize_entry(entry: &Entry<TypeConfig>) -> Result<Vec<u8>, StorageError<NodeId>> {
        serde_json::to_vec(entry).map_err(|e| logs_error(e, ErrorVerb::Write))
    }

    /// Deserialize an entry
    fn deserialize_entry(bytes: &[u8]) -> Result<Entry<TypeConfig>, StorageError<NodeId>> {
        serde_json::from_slice(bytes).map_err(|e| logs_error(e, ErrorVerb::Read))
    }

    /// Load vote from storage
    fn load_vote_internal(&self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        match self.db.get_cf(self.cf_state(), KEY_VOTE) {
            Ok(Some(bytes)) => {
                let vote: Vote<NodeId> =
                    serde_json::from_slice(&bytes).map_err(|e| vote_error(e, ErrorVerb::Read))?;
                Ok(Some(vote))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(vote_error(e, ErrorVerb::Read)),
        }
    }

    /// Load last purged log ID
    fn load_last_purged_internal(&self) -> Result<Option<LogId<NodeId>>, StorageError<NodeId>> {
        match self.db.get_cf(self.cf_state(), KEY_LAST_PURGED) {
            Ok(Some(bytes)) => {
                let log_id: LogId<NodeId> =
                    serde_json::from_slice(&bytes).map_err(|e| logs_error(e, ErrorVerb::Read))?;
                Ok(Some(log_id))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(logs_error(e, ErrorVerb::Read)),
        }
    }

    /// Calculate the last log ID by scanning the log store
    fn calculate_last_log_id(&self) -> Result<Option<LogId<NodeId>>, StorageError<NodeId>> {
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

    /// Get an entry by index
    #[allow(dead_code)]
    fn get_entry(&self, index: u64) -> Result<Option<Entry<TypeConfig>>, StorageError<NodeId>> {
        let key = Self::encode_log_index(index);
        match self.db.get_cf(self.cf_logs(), &key) {
            Ok(Some(bytes)) => {
                let entry = Self::deserialize_entry(&bytes)?;
                Ok(Some(entry))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(logs_error(e, ErrorVerb::Read)),
        }
    }
}

impl RaftLogReader<TypeConfig> for RocksLogStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<TypeConfig>>, StorageError<NodeId>> {
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
}

impl RaftLogStorage<TypeConfig> for RocksLogStore {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, StorageError<NodeId>> {
        let last_purged = *self.last_purged.read().await;
        let last_log_id = *self.last_log_id.read().await;

        Ok(LogState {
            last_purged_log_id: last_purged,
            last_log_id,
        })
    }

    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        let bytes = serde_json::to_vec(vote).map_err(|e| vote_error(e, ErrorVerb::Write))?;
        self.db
            .put_cf(self.cf_state(), KEY_VOTE, &bytes)
            .map_err(|e| vote_error(e, ErrorVerb::Write))?;

        *self.vote.write().await = Some(*vote);
        debug!("Saved vote: {:?}", vote);
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        Ok(*self.vote.read().await)
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
        callback: LogFlushed<TypeConfig>,
    ) -> Result<(), StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let entries: Vec<_> = entries.into_iter().collect();
        if entries.is_empty() {
            callback.log_io_completed(Ok(()));
            return Ok(());
        }

        let mut batch = rocksdb::WriteBatch::default();
        let mut last_log_id = None;

        for entry in &entries {
            let key = Self::encode_log_index(entry.log_id.index);
            let value = Self::serialize_entry(entry)?;
            batch.put_cf(self.cf_logs(), &key, &value);
            last_log_id = Some(entry.log_id);
        }

        self.db
            .write(batch)
            .map_err(|e| logs_error(e, ErrorVerb::Write))?;

        if let Some(log_id) = last_log_id {
            *self.last_log_id.write().await = Some(log_id);
        }

        debug!("Appended {} log entries", entries.len());
        callback.log_io_completed(Ok(()));
        Ok(())
    }

    async fn truncate(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        // Delete all entries with index > log_id.index
        let start_key = Self::encode_log_index(log_id.index + 1);

        let mut batch = rocksdb::WriteBatch::default();
        let mut iter = self.db.raw_iterator_cf(self.cf_logs());
        iter.seek(&start_key);

        while iter.valid() {
            if let Some(key) = iter.key() {
                batch.delete_cf(self.cf_logs(), key);
            }
            iter.next();
        }

        self.db
            .write(batch)
            .map_err(|e| logs_error(e, ErrorVerb::Write))?;

        // Update cached last log ID
        *self.last_log_id.write().await = Some(log_id);

        debug!("Truncated logs after index {}", log_id.index);
        Ok(())
    }

    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        // Delete all entries with index <= log_id.index
        let end_key = Self::encode_log_index(log_id.index + 1);

        let mut batch = rocksdb::WriteBatch::default();
        let mut iter = self.db.raw_iterator_cf(self.cf_logs());
        iter.seek_to_first();

        while iter.valid() {
            if let Some(key) = iter.key() {
                if key >= end_key.as_slice() {
                    break;
                }
                batch.delete_cf(self.cf_logs(), key);
            }
            iter.next();
        }

        // Save last purged log ID
        let last_purged_bytes =
            serde_json::to_vec(&log_id).map_err(|e| logs_error(e, ErrorVerb::Write))?;
        batch.put_cf(self.cf_state(), KEY_LAST_PURGED, &last_purged_bytes);

        self.db
            .write(batch)
            .map_err(|e| logs_error(e, ErrorVerb::Write))?;

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
        let indices = vec![0u64, 1, 100, 1000, u64::MAX];
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
}
