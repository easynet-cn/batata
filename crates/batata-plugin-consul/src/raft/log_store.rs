/// RocksDB-based log storage for the Consul Raft group.
///
/// This is a copy of `batata_consistency::raft::log_store::RocksLogStore`
/// adapted for `ConsulTypeConfig`. The logic is identical; only the
/// type parameters differ.
use std::fmt::Debug;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

use byteorder::{BigEndian, WriteBytesExt};
use openraft::storage::{LogFlushed, LogState, RaftLogStorage};
use openraft::{ErrorSubject, ErrorVerb, OptionalSend, RaftLogReader, StorageError};
use rocksdb::{BlockBasedOptions, ColumnFamily, ColumnFamilyDescriptor, DB, Options};
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::types::*;

const CF_LOGS: &str = "logs";
const CF_STATE: &str = "state";
const KEY_VOTE: &[u8] = b"vote";
const KEY_LAST_PURGED: &[u8] = b"last_purged";

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

pub struct ConsulLogStore {
    db: Arc<DB>,
    last_log_id: RwLock<Option<ConsulLogId>>,
    vote: RwLock<Option<ConsulVote>>,
    last_purged: RwLock<Option<ConsulLogId>>,
}

impl ConsulLogStore {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StorageError<NodeId>> {
        let path = path.as_ref();
        std::fs::create_dir_all(path).map_err(|e| logs_error(e, ErrorVerb::Read))?;

        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

        let mut cf_opts = Options::default();
        cf_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        let mut block_opts = BlockBasedOptions::default();
        let cache = rocksdb::Cache::new_lru_cache(64 * 1024 * 1024);
        block_opts.set_block_cache(&cache);
        block_opts.set_bloom_filter(10.0, false);
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

        store.load_cached_values().await?;
        info!("Consul Raft log store initialized at {:?}", path);
        Ok(store)
    }

    async fn load_cached_values(&self) -> Result<(), StorageError<NodeId>> {
        if let Ok(Some(bytes)) = self.db.get_cf(self.cf_state(), KEY_VOTE)
            && let Ok(v) = serde_json::from_slice(&bytes)
        {
            *self.vote.write().await = Some(v);
        }
        if let Ok(Some(bytes)) = self.db.get_cf(self.cf_state(), KEY_LAST_PURGED)
            && let Ok(v) = serde_json::from_slice(&bytes)
        {
            *self.last_purged.write().await = Some(v);
        }
        let mut iter = self.db.raw_iterator_cf(self.cf_logs());
        iter.seek_to_last();
        if iter.valid()
            && let Some(value) = iter.value()
            && let Ok(entry) = serde_json::from_slice::<ConsulEntry>(value)
        {
            *self.last_log_id.write().await = Some(entry.log_id);
        }
        Ok(())
    }

    fn cf_logs(&self) -> &ColumnFamily {
        self.db.cf_handle(CF_LOGS).expect("CF logs must exist")
    }

    fn cf_state(&self) -> &ColumnFamily {
        self.db.cf_handle(CF_STATE).expect("CF state must exist")
    }

    fn encode_index(index: u64) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8);
        buf.write_u64::<BigEndian>(index)
            .expect("Vec write infallible");
        buf
    }
}

impl RaftLogReader<ConsulTypeConfig> for ConsulLogStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<ConsulEntry>, StorageError<NodeId>> {
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
        let start_key = Self::encode_index(start);
        let end_key = Self::encode_index(end);

        let mut iter = self.db.raw_iterator_cf(self.cf_logs());
        iter.seek(&start_key);

        while iter.valid() {
            if let (Some(key), Some(value)) = (iter.key(), iter.value()) {
                if key >= end_key.as_slice() {
                    break;
                }
                let entry: ConsulEntry =
                    serde_json::from_slice(value).map_err(|e| logs_error(e, ErrorVerb::Read))?;
                entries.push(entry);
            }
            iter.next();
        }

        debug!(
            "Consul log: read {} entries from {:?}",
            entries.len(),
            range
        );
        Ok(entries)
    }
}

impl RaftLogStorage<ConsulTypeConfig> for ConsulLogStore {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> Result<LogState<ConsulTypeConfig>, StorageError<NodeId>> {
        Ok(LogState {
            last_purged_log_id: *self.last_purged.read().await,
            last_log_id: *self.last_log_id.read().await,
        })
    }

    async fn save_vote(&mut self, vote: &ConsulVote) -> Result<(), StorageError<NodeId>> {
        let bytes = serde_json::to_vec(vote).map_err(|e| vote_error(e, ErrorVerb::Write))?;
        self.db
            .put_cf(self.cf_state(), KEY_VOTE, &bytes)
            .map_err(|e| vote_error(e, ErrorVerb::Write))?;
        *self.vote.write().await = Some(*vote);
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<ConsulVote>, StorageError<NodeId>> {
        Ok(*self.vote.read().await)
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        ConsulLogStore {
            db: self.db.clone(),
            last_log_id: RwLock::new(*self.last_log_id.read().await),
            vote: RwLock::new(*self.vote.read().await),
            last_purged: RwLock::new(*self.last_purged.read().await),
        }
    }

    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<ConsulTypeConfig>,
    ) -> Result<(), StorageError<NodeId>>
    where
        I: IntoIterator<Item = ConsulEntry> + OptionalSend,
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
            let key = Self::encode_index(entry.log_id.index);
            let value = serde_json::to_vec(entry).map_err(|e| logs_error(e, ErrorVerb::Write))?;
            batch.put_cf(self.cf_logs(), &key, &value);
            last_log_id = Some(entry.log_id);
        }

        self.db
            .write(batch)
            .map_err(|e| logs_error(e, ErrorVerb::Write))?;

        if let Some(log_id) = last_log_id {
            *self.last_log_id.write().await = Some(log_id);
        }

        callback.log_io_completed(Ok(()));
        Ok(())
    }

    async fn truncate(&mut self, log_id: ConsulLogId) -> Result<(), StorageError<NodeId>> {
        let start_key = Self::encode_index(log_id.index + 1);
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
        *self.last_log_id.write().await = Some(log_id);
        Ok(())
    }

    async fn purge(&mut self, log_id: ConsulLogId) -> Result<(), StorageError<NodeId>> {
        let end_key = Self::encode_index(log_id.index + 1);
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

        let bytes = serde_json::to_vec(&log_id).map_err(|e| logs_error(e, ErrorVerb::Write))?;
        batch.put_cf(self.cf_state(), KEY_LAST_PURGED, &bytes);

        self.db
            .write(batch)
            .map_err(|e| logs_error(e, ErrorVerb::Write))?;
        *self.last_purged.write().await = Some(log_id);
        Ok(())
    }
}
