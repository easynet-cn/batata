//! Raft state machine implementation.
//!
//! Applies committed log entries to the application state using RocksDB.

// Allow many arguments for Raft apply operations - parameters are logically coupled
#![allow(clippy::too_many_arguments)]
// Allow complex types for snapshot data structures
#![allow(clippy::type_complexity)]

use crate::bincode;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::TryStreamExt;
use openraft::storage::{EntryResponder, RaftSnapshotBuilder, RaftStateMachine};
use openraft::{EntryPayload, OptionalSend};
use rocksdb::{BlockBasedOptions, ColumnFamily, ColumnFamilyDescriptor, DB, Options};
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::hooks::{
    HookSet, SharedHooks, install_naming_hook, new_shared_hooks, register_plugin_handler,
};
use super::naming_hook::NamingApplyHook;
use super::plugin::RaftPluginHandler;
use super::request::{RaftRequest, RaftResponse};
use super::snapshot_file::SnapshotFile;
use super::snapshot_format::{self, SnapshotReader, SnapshotWriter};
use super::types::{LogId, NodeId, Snapshot, SnapshotMeta, StoredMembership, TypeConfig};

/// On-disk snapshot layout, mirroring JRaft's `LocalSnapshotStorage`:
///
///   {snapshot_dir}/temp/                  snapshot being written
///   {snapshot_dir}/incoming/              snapshot streamed in from the leader
///   {snapshot_dir}/snapshot_{index}/      committed snapshot (latest one wins)
///
/// Each of those directories holds `__raft_snapshot_meta` (serialized
/// `SnapshotMeta`) plus `data` (the snapshot body). Writing goes through `temp`
/// and is published with a single `rename`, so a crash mid-write can never
/// expose a half-written snapshot.
/// Directory a snapshot is staged in before it is published by rename.
const SNAPSHOT_TEMP_DIR: &str = "temp";
/// Directory a snapshot streamed in from the leader is written to.
pub(crate) const SNAPSHOT_INCOMING_DIR: &str = "incoming";
/// Prefix of a committed snapshot directory, suffixed with the last applied index.
const SNAPSHOT_DIR_PREFIX: &str = "snapshot_";
/// File holding the serialized `SnapshotMeta` of a snapshot.
const SNAPSHOT_META_FILE: &str = "__raft_snapshot_meta";
/// File holding the snapshot body.
const SNAPSHOT_DATA_FILE: &str = "data";
/// Flush threshold for the restore `WriteBatch`: a snapshot is applied in
/// batches so memory stays flat no matter how large the snapshot is.
const RESTORE_BATCH_BYTES: usize = 8 * 1024 * 1024;

/// Bounded number of snapshot records in flight between the blocking RocksDB
/// iterator and the async snapshot writer.
///
/// A small capacity keeps peak memory flat while still allowing the blocking
/// producer to run ahead of the file writer. Each record is length-capped by
/// `snapshot_format::MAX_SNAPSHOT_RECORD_LEN`.
const SNAPSHOT_CHANNEL_CAPACITY: usize = 8;

/// One record produced by the blocking RocksDB iterator and consumed by the
/// async snapshot writer.
enum SnapshotRecord {
    StartCf(String),
    Entry(Vec<u8>, Vec<u8>),
}

/// Run synchronous snapshot RocksDB work on Tokio's blocking pool.
///
/// Snapshot build, restore and post-install naming reconciliation used to
/// iterate or write RocksDB inline on the actix `current_thread` runtime. A
/// large snapshot therefore froze `main`, the same thread that polls
/// RaftCore, the heartbeat worker and the election deadline notifier. Awaiting
/// the blocking task keeps the async thread free while preserving streaming
/// and ordering.
async fn blocking_snapshot_io<F, T>(f: F) -> Result<T, std::io::Error>
where
    F: FnOnce() -> Result<T, std::io::Error> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| io_error(format!("RocksDB snapshot task failed: {e}")))?
}

/// Apply one bounded batch of snapshot entries to RocksDB.
///
/// Synchronous by design: called from inside `blocking_snapshot_io`, so the
/// `WriteBatch` never occupies the async runtime thread.
fn flush_restore_batch(
    db: Arc<DB>,
    cf_name: String,
    entries: Vec<(Vec<u8>, Vec<u8>)>,
    include_range_delete: bool,
) -> Result<(), std::io::Error> {
    let cf = db
        .cf_handle(&cf_name)
        .ok_or_else(|| io_error("snapshot entry before any column family"))?;

    let mut batch = rocksdb::WriteBatch::default();
    if include_range_delete {
        // Keys are UTF-8 strings, so 0xFF as exclusive end covers all keys.
        batch.delete_range_cf(cf, vec![0u8], vec![0xFFu8, 0xFF, 0xFF, 0xFF]);
    }
    for (key, value) in entries {
        batch.put_cf(cf, &key, &value);
    }
    db.write(batch).map_err(io_error)?;
    Ok(())
}

/// Flush the accumulated entries for one column family on the blocking pool.
async fn flush_restore_batch_async(
    db: Arc<DB>,
    cf_name: String,
    pending: &mut Vec<(Vec<u8>, Vec<u8>)>,
    batch_bytes: &mut usize,
    include_range_delete: bool,
) -> Result<(), std::io::Error> {
    if pending.is_empty() {
        *batch_bytes = 0;
        return Ok(());
    }

    let entries = std::mem::take(pending);
    *batch_bytes = 0;
    blocking_snapshot_io(move || flush_restore_batch(db, cf_name, entries, include_range_delete))
        .await
}

/// What a snapshot restore applied.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RestoreStats {
    /// Number of column families the snapshot touched.
    pub column_families: usize,
    /// Number of key/value pairs written.
    pub entries: usize,
    /// Sum of key and value bytes written.
    pub bytes: u64,
}

/// Column families owned by the core, in the order they are streamed into a
/// snapshot.
///
/// Plugin-owned CFs are *not* listed here — plugins declare them through
/// [`RaftPluginHandler::snapshot_column_families`] and the builder appends
/// them, so a plugin's data can never be silently omitted from snapshots.
pub(super) const CORE_SNAPSHOT_CF_NAMES: &[&str] = &[
    CF_CONFIG,
    CF_CONFIG_HISTORY,
    CF_CONFIG_GRAY,
    CF_NAMESPACE,
    CF_USERS,
    CF_ROLES,
    CF_PERMISSIONS,
    CF_INSTANCES,
    CF_LOCKS,
    CF_AI_RESOURCE,
    CF_AI_RESOURCE_VERSION,
    CF_PIPELINE_EXECUTION,
    CF_AI_RESOURCE_SEARCH_DOCUMENT,
    CF_AI_RESOURCE_SEARCH_CHUNK,
    CF_AI_RESOURCE_TASK,
];

/// Pick a stable scratch file for nodes without a snapshot directory.
///
/// A snapshot body has to live somewhere, and openraft's `SnapshotData` is now
/// a file handle. Without a configured directory we fall back to the system
/// temp directory with a name derived from the DB path, so repeated builds
/// overwrite one file instead of leaking a new one each time.
fn transient_snapshot_path(db_path: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    db_path.hash(&mut hasher);
    std::env::temp_dir().join(format!("batata-raft-{:016x}.snapshot", hasher.finish()))
}

/// Copy `src` to `dst` through a fixed-size buffer.
///
/// Used to publish a snapshot body: the copy streams, so a multi-gigabyte
/// snapshot costs constant memory on both sides.
async fn copy_file_streaming(src: &Path, dst: &Path) -> std::io::Result<u64> {
    let mut reader = BufReader::with_capacity(
        snapshot_format::IO_BUFFER_SIZE,
        tokio::fs::File::open(src).await?,
    );
    let mut writer = BufWriter::with_capacity(
        snapshot_format::IO_BUFFER_SIZE,
        tokio::fs::File::create(dst).await?,
    );

    let copied = tokio::io::copy(&mut reader, &mut writer).await?;
    writer.flush().await?;
    writer.into_inner().sync_all().await?;
    Ok(copied)
}

/// openraft 0.10 state machine methods return `io::Error`; the storage
/// context is attached by openraft itself.
fn io_error(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

/// Write options for state-machine bookkeeping (last applied log id and
/// membership).
///
/// RocksDB appends every write to a single WAL, so syncing the last-applied
/// marker also flushes the entry data written just before it. Without the
/// sync, a power failure can leave the marker ahead of the data, and replay
/// would then skip entries that were never applied.
fn sync_write_options() -> rocksdb::WriteOptions {
    let mut opts = rocksdb::WriteOptions::default();
    opts.set_sync(true);
    opts
}

/// Owned snapshot of the shared handles an apply may dispatch to.
///
/// `apply_request` runs on Tokio's blocking pool, where a
/// `tokio::sync::RwLock` cannot be awaited (and blocking on it could deadlock
/// against the single-threaded async worker). Resolve the whole handle set
/// once per entry on the async side instead. The set is a single immutable
/// `Arc`, so one read lock yields an atomically consistent generation and
/// nothing is held across the blocking hop.
type ApplyHooks = Arc<HookSet>;

/// Run synchronous state-machine work on Tokio's blocking pool.
///
/// `apply_request` performs RocksDB reads and writes inline; on the actix
/// `current_thread` runtime those used to occupy `main`, the same thread that
/// polls RaftCore, the Raft gRPC server and the election deadline notifier.
/// Awaiting the blocking task keeps apply strictly serialised (one entry at a
/// time, in log order) while the runtime thread stays free.
async fn blocking_apply_io<F, T>(f: F) -> Result<T, std::io::Error>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| io_error(format!("RocksDB state-machine task failed: {e}")))
}

// Column family names for state machine
/// RocksDB column family holding published configuration entries.
pub const CF_CONFIG: &str = "batata_config";
/// RocksDB column family holding configuration change history entries.
pub const CF_CONFIG_HISTORY: &str = "batata_config_history";
/// RocksDB column family holding gray (beta) configuration entries.
pub const CF_CONFIG_GRAY: &str = "batata_config_gray";
/// RocksDB column family holding namespace entries.
pub const CF_NAMESPACE: &str = "batata_namespace";
/// RocksDB column family holding user entries.
pub const CF_USERS: &str = "batata_users";
/// RocksDB column family holding role assignment entries.
pub const CF_ROLES: &str = "batata_roles";
/// RocksDB column family holding permission entries.
pub const CF_PERMISSIONS: &str = "batata_permissions";
/// RocksDB column family holding persistent service instance entries.
pub const CF_INSTANCES: &str = "batata_instances";
/// RocksDB column family holding distributed lock entries.
pub const CF_LOCKS: &str = "batata_locks";
/// RocksDB column family holding AI resource entries.
pub const CF_AI_RESOURCE: &str = "batata_ai_resource";
/// RocksDB column family holding AI resource version entries.
pub const CF_AI_RESOURCE_VERSION: &str = "batata_ai_resource_version";
/// RocksDB column family holding pipeline execution entries.
pub const CF_PIPELINE_EXECUTION: &str = "batata_pipeline_execution";
/// RocksDB column family holding AI resource search documents.
pub const CF_AI_RESOURCE_SEARCH_DOCUMENT: &str = "batata_ai_resource_search_document";
/// RocksDB column family holding AI resource search chunks.
pub const CF_AI_RESOURCE_SEARCH_CHUNK: &str = "batata_ai_resource_search_chunk";
/// RocksDB column family holding AI resource async tasks.
pub const CF_AI_RESOURCE_TASK: &str = "batata_ai_resource_task";
const CF_META: &str = "batata_meta";

// Apollo plugin column families
/// Apollo plugin column family for applications.
pub const CF_APOLLO_APP: &str = "apollo_app";
/// Apollo plugin column family for clusters.
pub const CF_APOLLO_CLUSTER: &str = "apollo_cluster";
/// Apollo plugin column family for namespaces.
pub const CF_APOLLO_NAMESPACE: &str = "apollo_namespace";
/// Apollo plugin column family for configuration items.
pub const CF_APOLLO_ITEM: &str = "apollo_item";
/// Apollo plugin column family for releases.
pub const CF_APOLLO_RELEASE: &str = "apollo_release";
/// Apollo plugin column family for commits.
pub const CF_APOLLO_COMMIT: &str = "apollo_commit";
/// Apollo plugin column family for gray rules.
pub const CF_APOLLO_GRAY_RULE: &str = "apollo_gray_rule";
/// Apollo plugin column family for instances.
pub const CF_APOLLO_INSTANCE: &str = "apollo_instance";
/// Apollo plugin column family for access keys.
pub const CF_APOLLO_ACCESS_KEY: &str = "apollo_access_key";
/// Apollo plugin column family for release messages.
pub const CF_APOLLO_RELEASE_MSG: &str = "apollo_release_msg";
/// Apollo plugin column family for namespace locks.
pub const CF_APOLLO_NAMESPACE_LOCK: &str = "apollo_namespace_lock";
/// Apollo plugin column family for release history.
pub const CF_APOLLO_RELEASE_HISTORY: &str = "apollo_release_history";
/// Apollo plugin column family for application-namespace mappings.
pub const CF_APOLLO_APP_NAMESPACE: &str = "apollo_app_namespace";
/// Apollo plugin column family for audit records.
pub const CF_APOLLO_AUDIT: &str = "apollo_audit";
/// Apollo plugin column family for consumers.
pub const CF_APOLLO_CONSUMER: &str = "apollo_consumer";
/// Apollo plugin column family for consumer tokens.
pub const CF_APOLLO_CONSUMER_TOKEN: &str = "apollo_consumer_token";
/// Apollo plugin column family for consumer audit records.
pub const CF_APOLLO_CONSUMER_AUDIT: &str = "apollo_consumer_audit";
/// Apollo plugin column family for consumer-role mappings.
pub const CF_APOLLO_CONSUMER_ROLE: &str = "apollo_consumer_role";
/// Apollo plugin column family for permissions.
pub const CF_APOLLO_PERMISSION: &str = "apollo_permission";
/// Apollo plugin column family for roles.
pub const CF_APOLLO_ROLE: &str = "apollo_role";
/// Apollo plugin column family for role-permission mappings.
pub const CF_APOLLO_ROLE_PERMISSION: &str = "apollo_role_permission";
/// Apollo plugin column family for user-role mappings.
pub const CF_APOLLO_USER_ROLE: &str = "apollo_user_role";
/// Apollo plugin column family for users.
pub const CF_APOLLO_USERS: &str = "apollo_users";
/// Apollo plugin column family for favorites.
pub const CF_APOLLO_FAVORITE: &str = "apollo_favorite";
/// Apollo plugin column family for server config.
pub const CF_APOLLO_SERVER_CONFIG: &str = "apollo_server_config";
/// Apollo plugin column family for service registry.
pub const CF_APOLLO_SERVICE_REGISTRY: &str = "apollo_service_registry";
/// Apollo plugin column family for instance config.
pub const CF_APOLLO_INSTANCE_CONFIG: &str = "apollo_instance_config";
/// Apollo plugin column family for user tokens (portal session / openapi user token).
pub const CF_APOLLO_USER_TOKEN: &str = "apollo_user_token";
/// Apollo plugin column family for Spring Security authorities.
pub const CF_APOLLO_AUTHORITIES: &str = "apollo_authorities";
/// Apollo plugin column family for user-token request audit records.
pub const CF_APOLLO_USER_TOKEN_AUDIT: &str = "apollo_user_token_audit";
/// Apollo plugin column family for distributed audit logs.
pub const CF_APOLLO_AUDIT_LOG: &str = "apollo_audit_log";
/// Apollo plugin column family for audit-log data influences.
pub const CF_APOLLO_AUDIT_LOG_DATA_INFLUENCE: &str = "apollo_audit_log_data_influence";

// Meta keys
const KEY_LAST_APPLIED: &[u8] = b"last_applied";
const KEY_LAST_MEMBERSHIP: &[u8] = b"last_membership";

/// Shared state behind [`RocksStateMachine`].
///
/// openraft's `apply()` is polled on the actix `current_thread` runtime, the
/// same `main` thread that runs RaftCore, the heartbeat worker and the
/// election deadline notifier. Moving the synchronous RocksDB work into
/// `spawn_blocking` requires an owned `'static` handle to this state, so it
/// lives behind an `Arc`. `Deref` keeps every existing `self.field` and
/// `self.method()` expression unchanged.
pub struct StateMachineInner {
    pub(super) db: Arc<DB>,
    /// Last applied log ID
    last_applied: RwLock<Option<LogId>>,
    /// Last membership configuration
    last_membership: RwLock<StoredMembership>,
    /// Every handle an apply may dispatch to: plugin handlers, the naming
    /// apply-back hook and the health-check apply-back hook. Registration
    /// replaces the inner `Arc<HookSet>` atomically, so the apply path takes
    /// one read and clones one `Arc` per entry. See `raft/hooks.rs`.
    hooks: SharedHooks,
    /// Pre-configured WriteOptions for state-machine writes (sync, WAL settings).
    pub(super) write_opts: rocksdb::WriteOptions,
    /// Directory snapshots are persisted to, using JRaft's `LocalSnapshotStorage`
    /// layout. `None` keeps snapshots memory-only (no restart recovery).
    snapshot_dir: Option<PathBuf>,

    /// Scratch file used when no snapshot directory is configured.
    transient_snapshot_path: PathBuf,
}

/// RocksDB-based state machine for Raft.
///
/// A thin `Arc` wrapper: the state itself is in [`StateMachineInner`] so the
/// apply path can hand an owned handle to a blocking task. Fields are exposed
/// to the apply-handler submodules through `Deref`.
pub struct RocksStateMachine {
    inner: Arc<StateMachineInner>,
}

impl std::ops::Deref for RocksStateMachine {
    type Target = StateMachineInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl RocksStateMachine {
    /// Build a cheap owned handle to the same state for `spawn_blocking`.
    fn handle(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Serialize a JSON value directly to bytes, avoiding intermediate String allocation.
///
/// `pub(super)` so apply-handler submodules can share this helper.
#[inline]
pub(super) fn json_to_bytes(value: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_default()
}

mod apply_ai;
mod apply_config;
mod apply_lock;
mod keys;
mod records;
pub use records::{
    StoredConfig, StoredConfigGray, StoredConfigHistory, StoredInstance, StoredNamespace,
    StoredPermission, StoredRole, StoredUser,
};

impl RocksStateMachine {
    /// Index a snapshot is filed under: the last applied log index, or 0 for a
    /// state machine that has not applied anything yet.
    fn snapshot_index(meta: &SnapshotMeta) -> u64 {
        meta.last_log_id.map(|l| l.index).unwrap_or(0)
    }

    /// Publish the snapshot body at `src` as this node's current snapshot.
    ///
    /// Stages everything under `temp/` and publishes with a single `rename`, so
    /// a crash mid-write never leaves a readable but incomplete snapshot. This
    /// is the same staging policy JRaft's `LocalSnapshotStorage` uses. The body
    /// is copied in fixed-size chunks, so publishing a large snapshot costs
    /// constant memory.
    ///
    /// Returns the path of the published body, or `None` when no snapshot
    /// directory is configured.
    /// Create an empty `temp/` staging directory for the next snapshot.
    ///
    /// Returns `None` when no snapshot directory is configured, meaning the
    /// snapshot stays in the scratch file instead of being published.
    async fn prepare_staging_dir(&self) -> Result<Option<PathBuf>, std::io::Error> {
        let Some(dir) = self.snapshot_dir.as_ref() else {
            return Ok(None);
        };

        tokio::fs::create_dir_all(dir).await.map_err(io_error)?;

        let temp = dir.join(SNAPSHOT_TEMP_DIR);
        if temp.exists() {
            tokio::fs::remove_dir_all(&temp).await.map_err(io_error)?;
        }
        tokio::fs::create_dir_all(&temp).await.map_err(io_error)?;

        Ok(Some(temp))
    }

    /// Write `meta` into the staging directory and publish it by rename.
    ///
    /// Returns the path of the published snapshot body.
    async fn commit_staging_dir(
        &self,
        temp: &Path,
        meta: &SnapshotMeta,
    ) -> Result<PathBuf, std::io::Error> {
        let dir = self
            .snapshot_dir
            .as_ref()
            .ok_or_else(|| io_error(std::io::Error::other("no snapshot directory")))?;

        let meta_bytes = bincode::serialize(meta).map_err(io_error)?;
        tokio::fs::write(temp.join(SNAPSHOT_META_FILE), &meta_bytes)
            .await
            .map_err(io_error)?;

        let target = dir.join(format!(
            "{}{}",
            SNAPSHOT_DIR_PREFIX,
            Self::snapshot_index(meta)
        ));
        if target.exists() {
            tokio::fs::remove_dir_all(&target).await.map_err(io_error)?;
        }
        tokio::fs::rename(temp, &target).await.map_err(io_error)?;

        self.prune_snapshots(&target).await?;
        Ok(target.join(SNAPSHOT_DATA_FILE))
    }

    async fn publish_snapshot_file(
        &self,
        meta: &SnapshotMeta,
        src: &Path,
    ) -> Result<Option<PathBuf>, std::io::Error> {
        let Some(temp) = self.prepare_staging_dir().await? else {
            return Ok(None);
        };

        copy_file_streaming(src, &temp.join(SNAPSHOT_DATA_FILE))
            .await
            .map_err(io_error)?;

        Ok(Some(self.commit_staging_dir(&temp, meta).await?))
    }

    /// Apply the snapshot body at `path` to RocksDB, streaming it record by
    /// record.
    ///
    /// Each column family is cleared with a range delete the first time it
    /// appears, then filled from the snapshot. Batches are flushed once they
    /// reach [`RESTORE_BATCH_BYTES`], so peak memory is bounded by that
    /// threshold rather than by the size of the snapshot. All RocksDB writes
    /// run on the blocking pool through `blocking_snapshot_io`; only the async
    /// file reader stays on the runtime thread.
    async fn restore_from_file(&self, path: &Path) -> Result<RestoreStats, std::io::Error> {
        let file = tokio::fs::File::open(path).await.map_err(io_error)?;

        // An empty body is a valid snapshot: a node with no state at all.
        let total_len = file.metadata().await.map_err(io_error)?.len();
        if total_len == 0 {
            return Ok(RestoreStats::default());
        }

        let mut reader = SnapshotReader::new(BufReader::with_capacity(
            snapshot_format::IO_BUFFER_SIZE,
            file,
        ))
        .with_total_len(total_len);
        reader.read_header().await.map_err(io_error)?;

        let mut stats = RestoreStats::default();
        let mut cf_name = String::new();
        let mut key = Vec::new();
        let mut value = Vec::new();

        let db = self.db.clone();
        let mut current_cf: Option<String> = None;
        let mut cleared = false;
        let mut pending: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        let mut batch_bytes = 0usize;

        while let Some(kind) = reader
            .next_record(&mut cf_name, &mut key, &mut value)
            .await
            .map_err(io_error)?
        {
            match kind {
                snapshot_format::RecordKind::ColumnFamily => {
                    // Flush any pending writes for the previous CF first.
                    if let Some(cf) = current_cf.clone() {
                        flush_restore_batch_async(
                            db.clone(),
                            cf,
                            &mut pending,
                            &mut batch_bytes,
                            !cleared,
                        )
                        .await?;
                    }
                    current_cf = Some(cf_name.clone());
                    cleared = false;
                    stats.column_families += 1;
                }
                snapshot_format::RecordKind::Entry => {
                    if current_cf.is_none() {
                        return Err(io_error("snapshot entry before any column family"));
                    }

                    let len = key.len() + value.len();
                    // `next_record` reuses its buffers; take ownership of the
                    // current pair so the bounded batch stays intact.
                    pending.push((std::mem::take(&mut key), std::mem::take(&mut value)));
                    batch_bytes += len;
                    stats.entries += 1;
                    stats.bytes += len as u64;

                    if batch_bytes >= RESTORE_BATCH_BYTES {
                        let cf = current_cf.clone().expect("checked above");
                        flush_restore_batch_async(
                            db.clone(),
                            cf,
                            &mut pending,
                            &mut batch_bytes,
                            !cleared,
                        )
                        .await?;
                        // The range delete is already durable, do not repeat it.
                        cleared = true;
                    }
                }
                snapshot_format::RecordKind::End => break,
            }
        }

        if let Some(cf) = current_cf.clone() {
            flush_restore_batch_async(db.clone(), cf, &mut pending, &mut batch_bytes, !cleared)
                .await?;
        }

        Ok(stats)
    }

    /// Drop every committed snapshot except `keep`.
    ///
    /// JRaft keeps only the newest snapshot too -- older ones are pure disk
    /// cost once the corresponding log entries have been purged.
    async fn prune_snapshots(&self, keep: &Path) -> Result<(), std::io::Error> {
        let Some(dir) = self.snapshot_dir.as_ref() else {
            return Ok(());
        };

        let mut entries = tokio::fs::read_dir(dir).await.map_err(io_error)?;

        while let Some(entry) = entries.next_entry().await.map_err(io_error)? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with(SNAPSHOT_DIR_PREFIX) {
                continue;
            }

            let path = entry.path();
            if path.as_path() == keep || !path.is_dir() {
                continue;
            }

            if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                warn!("Failed to remove stale snapshot {}: {}", name, e);
            } else {
                info!("Removed stale snapshot {}", name);
            }
        }

        Ok(())
    }

    /// Locate the newest committed snapshot directory, if any.
    async fn latest_snapshot_dir(&self) -> Option<PathBuf> {
        let dir = self.snapshot_dir.as_ref()?;
        let mut entries = tokio::fs::read_dir(dir).await.ok()?;
        let mut best: Option<(u64, PathBuf)> = None;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(index) = name
                .strip_prefix(SNAPSHOT_DIR_PREFIX)
                .and_then(|suffix| suffix.parse::<u64>().ok())
            else {
                continue;
            };
            if best.as_ref().map(|(b, _)| index > *b).unwrap_or(true) {
                best = Some((index, entry.path()));
            }
        }

        best.map(|(_, path)| path)
    }

    /// Hand the post-install set of persistent instances to the naming hook.
    ///
    /// Snapshot install bypasses the normal apply path, so no `on_register` /
    /// `on_deregister` ever fires for the replaced entries. Without this the
    /// in-memory naming view diverges from `CF_INSTANCES` in both directions:
    /// it keeps serving instances the cluster deleted, and it misses the ones
    /// the snapshot added.
    ///
    /// The hook is invoked with the full authoritative set rather than a diff
    /// because a snapshot replaces state wholesale -- there is no cheap way to
    /// know what changed, and the receiving side owns the in-memory view so it
    /// can compute the difference itself.
    async fn reconcile_naming_view(&self) {
        let hook = {
            let guard = self.hooks.read().await;
            guard.naming.clone()
        };
        let Some(hook) = hook else {
            return;
        };

        let db = self.db.clone();
        // Both the RocksDB iteration and the hook dispatch are synchronous.
        // Run them on the blocking pool so a large instance set cannot freeze
        // the single-threaded async runtime during snapshot install.
        if let Err(e) = blocking_snapshot_io(move || {
            let Some(cf) = db.cf_handle(CF_INSTANCES) else {
                return Ok(());
            };

            let mut instances = Vec::new();
            let mut malformed = 0usize;
            for item in db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
                match item {
                    Ok((_key, value)) => match bincode::deserialize::<StoredInstance>(&value) {
                        Ok(stored) => instances.push(stored),
                        Err(e) => {
                            malformed += 1;
                            warn!("Undecodable instance after snapshot install: {}", e);
                        }
                    },
                    Err(e) => warn!("Unreadable instance after snapshot install: {}", e),
                }
            }

            info!(
                "Reconciling {} persistent instances after snapshot install ({} undecodable)",
                instances.len(),
                malformed
            );
            hook.on_snapshot_loaded(instances);
            Ok(())
        })
        .await
        {
            warn!("Snapshot naming reconciliation task failed: {}", e);
        }
    }

    /// Enable on-disk snapshot persistence under `dir`.
    ///
    /// Without this, snapshots stay memory-only: a node whose log has been
    /// purged cannot rebuild its state machine after a restart. The on-disk
    /// layout mirrors JRaft's `LocalSnapshotStorage`.
    pub fn with_snapshot_dir(mut self, dir: PathBuf) -> Self {
        // Called once during startup, before any handle is cloned for the
        // blocking pool, so the Arc is still uniquely owned.
        Arc::get_mut(&mut self.inner)
            .expect("snapshot dir must be set before the state machine is shared")
            .snapshot_dir = Some(dir);
        self
    }

    /// Get a reference to the underlying RocksDB instance
    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    /// Get a shared handle to the atomic hook set.
    ///
    /// `RaftNode` holds the same handle so plugins and apply-back hooks can
    /// be installed after the state machine has been moved into OpenRaft.
    pub fn hooks(&self) -> SharedHooks {
        self.hooks.clone()
    }

    /// Install the naming apply-back hook through the atomic set.
    ///
    /// Convenience entry point for tests and snapshot-install setups that do
    /// not go through `RaftNode`; production uses
    /// `RaftNode::register_naming_hook` on the very same shared handle.
    pub async fn install_naming_hook(&self, hook: Arc<dyn NamingApplyHook>) {
        install_naming_hook(&self.hooks, hook).await;
    }

    /// Register a plugin handler for processing PluginWrite operations.
    ///
    /// The plugin's column families must already exist in RocksDB — pass them
    /// via `extra_cf_names` when creating the state machine, or via
    /// `PluginContext::get("extra_cf_names")` before Raft startup.
    pub async fn register_plugin(&self, handler: Arc<dyn RaftPluginHandler>) -> Result<(), String> {
        // Verify all required CFs exist
        for cf_name in handler.column_families() {
            if self.db.cf_handle(&cf_name).is_none() {
                return Err(format!(
                    "Column family '{}' required by plugin '{}' does not exist in RocksDB. \
                     Pass it via extra_cf_names when creating the state machine.",
                    cf_name,
                    handler.plugin_id()
                ));
            }
        }
        register_plugin_handler(&self.hooks, handler).await
    }

    /// Create a new RocksDB state machine with default configuration
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        Self::with_options(path, None, None).await
    }

    /// Create a new RocksDB state machine with pre-configured RocksDB options.
    ///
    /// Pass `None` for either parameter to use sensible defaults.
    /// This allows callers to customize RocksDB tuning via application configuration
    /// without introducing a dependency on the configuration crate.
    pub async fn with_options<P: AsRef<Path>>(
        path: P,
        custom_db_opts: Option<Options>,
        custom_cf_opts: Option<Options>,
    ) -> Result<Self, std::io::Error> {
        Self::with_options_and_cfs(path, custom_db_opts, custom_cf_opts, &[]).await
    }

    /// Create a new RocksDB state machine with custom options and extra column families.
    ///
    /// `extra_cf_names` are additional column families (e.g., from plugins) that
    /// will be created alongside the core CFs. The same `cf_opts` are applied to all.
    /// `custom_history_cf_opts` allows separate tuning for history CF (append-only workload).
    /// `custom_write_opts` configures sync/WAL behavior for state-machine writes.
    pub async fn with_options_and_cfs<P: AsRef<Path>>(
        path: P,
        custom_db_opts: Option<Options>,
        custom_cf_opts: Option<Options>,
        extra_cf_names: &[String],
    ) -> Result<Self, std::io::Error> {
        Self::with_full_options(
            path,
            custom_db_opts,
            custom_cf_opts,
            None,
            None,
            extra_cf_names,
        )
        .await
    }

    /// Create a new RocksDB state machine with full customization including
    /// separate history CF options and WriteOptions.
    pub async fn with_full_options<P: AsRef<Path>>(
        path: P,
        custom_db_opts: Option<Options>,
        custom_cf_opts: Option<Options>,
        custom_history_cf_opts: Option<Options>,
        custom_write_opts: Option<rocksdb::WriteOptions>,
        extra_cf_names: &[String],
    ) -> Result<Self, std::io::Error> {
        let db_opts = custom_db_opts.unwrap_or_else(|| {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.create_missing_column_families(true);
            opts.set_write_buffer_size(128 * 1024 * 1024);
            opts.set_max_write_buffer_number(4);
            opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
            opts.set_bottommost_compression_type(rocksdb::DBCompressionType::Zstd);
            opts.set_level_compaction_dynamic_level_bytes(true);
            opts.increase_parallelism(std::cmp::max(
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4) as i32
                    / 2,
                2,
            ));
            opts.set_max_background_jobs(4);
            opts
        });

        let cf_opts = custom_cf_opts.unwrap_or_else(|| {
            let mut opts = Options::default();
            opts.set_write_buffer_size(128 * 1024 * 1024);
            opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
            opts.set_bottommost_compression_type(rocksdb::DBCompressionType::Zstd);
            let mut block_opts = BlockBasedOptions::default();
            let cache = rocksdb::Cache::new_lru_cache(256 * 1024 * 1024);
            block_opts.set_block_cache(&cache);
            block_opts.set_bloom_filter(10.0, false);
            opts.set_block_based_table_factory(&block_opts);
            opts
        });

        let history_cf_opts = custom_history_cf_opts.unwrap_or_else(|| cf_opts.clone());

        let mut cfs = vec![
            ColumnFamilyDescriptor::new(CF_CONFIG, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_CONFIG_HISTORY, history_cf_opts),
            ColumnFamilyDescriptor::new(CF_CONFIG_GRAY, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_NAMESPACE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_USERS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_ROLES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_PERMISSIONS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_INSTANCES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_LOCKS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_AI_RESOURCE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_AI_RESOURCE_VERSION, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_PIPELINE_EXECUTION, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_AI_RESOURCE_SEARCH_DOCUMENT, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_AI_RESOURCE_SEARCH_CHUNK, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_AI_RESOURCE_TASK, cf_opts.clone()),
            // Apollo plugin column families
            ColumnFamilyDescriptor::new(CF_APOLLO_APP, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_CLUSTER, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_NAMESPACE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_ITEM, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_RELEASE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_COMMIT, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_GRAY_RULE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_INSTANCE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_ACCESS_KEY, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_RELEASE_MSG, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_NAMESPACE_LOCK, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_RELEASE_HISTORY, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_APP_NAMESPACE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_AUDIT, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_CONSUMER, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_CONSUMER_TOKEN, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_CONSUMER_AUDIT, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_CONSUMER_ROLE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_PERMISSION, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_ROLE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_ROLE_PERMISSION, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_USER_ROLE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_USERS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_FAVORITE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_SERVER_CONFIG, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_INSTANCE_CONFIG, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_USER_TOKEN, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_AUTHORITIES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_USER_TOKEN_AUDIT, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_AUDIT_LOG, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_AUDIT_LOG_DATA_INFLUENCE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_APOLLO_SERVICE_REGISTRY, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_META, cf_opts.clone()),
        ];

        // Add plugin column families
        for cf_name in extra_cf_names {
            cfs.push(ColumnFamilyDescriptor::new(cf_name, cf_opts.clone()));
        }

        let db_path = path.as_ref().to_path_buf();
        let db = DB::open_cf_descriptors(&db_opts, path, cfs).map_err(io_error)?;

        let sm = Self {
            inner: Arc::new(StateMachineInner {
                db: Arc::new(db),
                last_applied: RwLock::new(None),
                last_membership: RwLock::new(StoredMembership::default()),
                hooks: new_shared_hooks(),
                write_opts: custom_write_opts.unwrap_or_default(),
                snapshot_dir: None,
                transient_snapshot_path: transient_snapshot_path(&db_path),
            }),
        };

        // Load cached values asynchronously
        sm.load_cached_values().await?;

        info!("RocksDB state machine initialized");
        Ok(sm)
    }

    /// Load cached values from storage.
    async fn load_cached_values(&self) -> Result<(), std::io::Error> {
        // Load last applied
        if let Some(bytes) = self
            .db
            .get_cf(self.cf_meta(), KEY_LAST_APPLIED)
            .map_err(io_error)?
        {
            let log_id: LogId = bincode::deserialize(&bytes).map_err(io_error)?;
            *self.last_applied.write().await = Some(log_id);
        }

        // Load last membership
        if let Some(bytes) = self
            .db
            .get_cf(self.cf_meta(), KEY_LAST_MEMBERSHIP)
            .map_err(io_error)?
        {
            let membership: StoredMembership = bincode::deserialize(&bytes).map_err(io_error)?;
            *self.last_membership.write().await = membership;
        }

        Ok(())
    }

    /// Voter ids from the last membership persisted in RocksDB.
    ///
    /// Loaded during construction from `KEY_LAST_MEMBERSHIP`, so this reflects
    /// the identity generation the data directory belongs to. Used at startup
    /// to reject reuse of a directory created with a different node-id scheme
    /// before OpenRaft skips `initialize()` and silently stalls all writes.
    /// Returns an empty set for a never-initialized store.
    pub async fn stored_voter_ids(&self) -> BTreeSet<NodeId> {
        self.last_membership.read().await.voter_ids().collect()
    }

    /// Get column family handles
    /// These expect() calls are acceptable because:
    /// 1. Column families are created during DB initialization
    /// 2. If they don't exist, it indicates severe DB corruption that cannot be recovered
    /// 3. The application cannot function without these column families
    pub(super) fn cf_config(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONFIG)
            .expect("CF_CONFIG must exist - database may be corrupted")
    }

    pub(super) fn cf_namespace(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_NAMESPACE)
            .expect("CF_NAMESPACE must exist - database may be corrupted")
    }

    pub(super) fn cf_users(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_USERS)
            .expect("CF_USERS must exist - database may be corrupted")
    }

    pub(super) fn cf_roles(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_ROLES)
            .expect("CF_ROLES must exist - database may be corrupted")
    }

    pub(super) fn cf_permissions(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_PERMISSIONS)
            .expect("CF_PERMISSIONS must exist - database may be corrupted")
    }

    pub(super) fn cf_instances(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_INSTANCES)
            .expect("CF_INSTANCES must exist - database may be corrupted")
    }

    pub(super) fn cf_locks(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_LOCKS)
            .expect("CF_LOCKS must exist - database may be corrupted")
    }

    pub(super) fn cf_config_history(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONFIG_HISTORY)
            .expect("CF_CONFIG_HISTORY must exist - database may be corrupted")
    }

    pub(super) fn cf_config_gray(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONFIG_GRAY)
            .expect("CF_CONFIG_GRAY must exist - database may be corrupted")
    }

    fn cf_meta(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_META)
            .expect("CF_META must exist - database may be corrupted")
    }

    /// Resolve the shared handles one apply may dispatch to.
    ///
    /// Called on the async thread before `spawn_blocking`; the returned
    /// `Arc<HookSet>` owns everything the blocking closure needs. One read
    /// lock yields one immutable generation of all three handles, and the
    /// guard is dropped before the next `.await`, so no
    /// `tokio::sync::RwLock` guard is ever held across an await point.
    async fn apply_hooks(&self) -> ApplyHooks {
        let guard = self.hooks.read().await;
        Arc::clone(&guard)
    }

    /// Apply a single request to the state machine.
    ///
    /// Synchronous by design: `apply()` runs it inside `spawn_blocking`, so
    /// every RocksDB read/write below stays off the single-threaded runtime.
    fn apply_request(
        &self,
        request: RaftRequest,
        log_index: u64,
        hooks: &ApplyHooks,
    ) -> RaftResponse {
        match request {
            RaftRequest::ConfigPublish(payload) => {
                let crate::raft::request::ConfigPublishPayload {
                    data_id,
                    group,
                    tenant,
                    content,
                    md5,
                    config_type,
                    app_name,
                    tag,
                    desc,
                    src_user,
                    src_ip,
                    r#use,
                    effect,
                    schema,
                    encrypted_data_key,
                    cas_md5,
                    history,
                    now_ms,
                } = *payload;
                self.apply_config_publish_batched(
                    &data_id,
                    &group,
                    &tenant,
                    &content,
                    &md5,
                    log_index as i64,
                    config_type,
                    app_name,
                    tag,
                    desc,
                    src_user,
                    src_ip,
                    r#use,
                    effect,
                    schema,
                    encrypted_data_key,
                    cas_md5.as_deref(),
                    history,
                    now_ms,
                )
            }

            RaftRequest::ConfigRemove {
                data_id,
                group,
                tenant,
                history,
                now_ms,
            } => self.apply_config_remove_batched(
                &data_id,
                &group,
                &tenant,
                history,
                log_index as i64,
                now_ms,
            ),

            RaftRequest::ConfigGrayPublish(payload) => {
                let crate::raft::request::ConfigGrayPublishPayload {
                    data_id,
                    group,
                    tenant,
                    content,
                    gray_name,
                    gray_rule,
                    app_name,
                    encrypted_data_key,
                    src_user,
                    src_ip,
                    cas_md5,
                    now_ms,
                } = *payload;
                self.apply_config_gray_publish(
                    &data_id,
                    &group,
                    &tenant,
                    &content,
                    &gray_name,
                    &gray_rule,
                    app_name,
                    encrypted_data_key,
                    src_user,
                    src_ip,
                    cas_md5,
                    now_ms,
                )
            }

            RaftRequest::ConfigGrayRemove {
                data_id,
                group,
                tenant,
                gray_name,
            } => self.apply_config_gray_remove(&data_id, &group, &tenant, &gray_name),

            RaftRequest::NamespaceCreate {
                namespace_id,
                namespace_name,
                namespace_desc,
                now_ms,
            } => {
                self.apply_namespace_create(&namespace_id, &namespace_name, namespace_desc, now_ms)
            }

            RaftRequest::NamespaceUpdate {
                namespace_id,
                namespace_name,
                namespace_desc,
                now_ms,
            } => {
                self.apply_namespace_update(&namespace_id, &namespace_name, namespace_desc, now_ms)
            }

            RaftRequest::NamespaceDelete { namespace_id } => {
                self.apply_namespace_delete(&namespace_id)
            }

            RaftRequest::UserCreate {
                username,
                password_hash,
                enabled,
                source,
                now_ms,
            } => self.apply_user_create(&username, &password_hash, enabled, &source, now_ms),

            RaftRequest::UserUpdate {
                username,
                password_hash,
                enabled,
                now_ms,
            } => self.apply_user_update(&username, password_hash, enabled, now_ms),

            RaftRequest::UserDelete { username } => self.apply_user_delete(&username),

            RaftRequest::RoleCreate {
                role,
                username,
                now_ms,
            } => self.apply_role_create(&role, &username, now_ms),

            RaftRequest::RoleDelete { role, username } => self.apply_role_delete(&role, &username),

            RaftRequest::PermissionGrant {
                role,
                resource,
                action,
                now_ms,
            } => self.apply_permission_grant(&role, &resource, &action, now_ms),

            RaftRequest::PermissionRevoke {
                role,
                resource,
                action,
            } => self.apply_permission_revoke(&role, &resource, &action),

            RaftRequest::ConfigHistoryInsert(payload) => {
                let crate::raft::request::ConfigHistoryInsertPayload {
                    id,
                    data_id,
                    group,
                    tenant,
                    content,
                    md5,
                    app_name,
                    src_user,
                    src_ip,
                    op_type,
                    publish_type,
                    gray_name,
                    ext_info,
                    encrypted_data_key,
                    created_time,
                    last_modified_time,
                } = *payload;
                self.apply_config_history_insert(
                    id,
                    &data_id,
                    &group,
                    &tenant,
                    &content,
                    &md5,
                    app_name,
                    src_user,
                    src_ip,
                    &op_type,
                    publish_type,
                    gray_name,
                    ext_info,
                    encrypted_data_key,
                    created_time,
                    last_modified_time,
                )
            }

            RaftRequest::ConfigTagsUpdate {
                data_id,
                group,
                tenant,
                tag,
                now_ms,
                ..
            } => self.apply_config_tags_update(&data_id, &group, &tenant, &tag, now_ms),

            RaftRequest::ConfigTagsDelete {
                data_id,
                group,
                tenant,
                tag,
                now_ms,
            } => self.apply_config_tags_delete(&data_id, &group, &tenant, &tag, now_ms),

            RaftRequest::PersistentInstanceRegister(payload) => {
                let crate::raft::request::PersistentInstanceRegisterPayload {
                    namespace_id,
                    group_name,
                    service_name,
                    instance_id,
                    ip,
                    port,
                    weight,
                    healthy,
                    enabled,
                    metadata,
                    cluster_name,
                    now_ms,
                } = *payload;
                let resp = self.apply_instance_register(
                    &namespace_id,
                    &group_name,
                    &service_name,
                    &instance_id,
                    &ip,
                    port,
                    weight,
                    healthy,
                    enabled,
                    &metadata,
                    &cluster_name,
                    now_ms,
                );
                if resp.success
                    && let Some(hook) = hooks.naming.as_ref()
                {
                    hook.on_register(
                        &namespace_id,
                        &group_name,
                        &service_name,
                        &instance_id,
                        &ip,
                        port,
                        weight,
                        healthy,
                        enabled,
                        &metadata,
                        &cluster_name,
                    );
                }
                resp
            }

            RaftRequest::PersistentInstanceDeregister {
                namespace_id,
                group_name,
                service_name,
                instance_id,
            } => {
                let resp = self.apply_instance_deregister(
                    &namespace_id,
                    &group_name,
                    &service_name,
                    &instance_id,
                );
                if resp.success
                    && let Some(hook) = hooks.naming.as_ref()
                {
                    hook.on_deregister(&namespace_id, &group_name, &service_name, &instance_id);
                }
                resp
            }

            RaftRequest::PersistentInstanceUpdate(payload) => {
                let crate::raft::request::PersistentInstanceUpdatePayload {
                    namespace_id,
                    group_name,
                    service_name,
                    instance_id,
                    ip,
                    port,
                    weight,
                    healthy,
                    enabled,
                    metadata,
                    now_ms,
                } = *payload;
                let ip_c = ip.clone();
                let metadata_c = metadata.clone();
                let resp = self.apply_instance_update(
                    &namespace_id,
                    &group_name,
                    &service_name,
                    &instance_id,
                    ip,
                    port,
                    weight,
                    healthy,
                    enabled,
                    metadata,
                    now_ms,
                );
                if resp.success
                    && let Some(hook) = hooks.naming.as_ref()
                {
                    hook.on_update(
                        &namespace_id,
                        &group_name,
                        &service_name,
                        &instance_id,
                        ip_c.as_deref(),
                        port,
                        weight,
                        healthy,
                        enabled,
                        metadata_c.as_deref(),
                    );
                }
                resp
            }

            RaftRequest::AiWrite { ops } => self.apply_ai_write(&ops),

            RaftRequest::AiAllocateIds {
                cf,
                counter_key,
                count,
            } => self.apply_ai_allocate_ids(&cf, &counter_key, count),

            RaftRequest::Noop => RaftResponse::success(),

            // Lock operations (ADV-005)
            RaftRequest::LockAcquire {
                namespace,
                name,
                owner,
                ttl_ms,
                fence_token,
                owner_metadata,
                now_ms,
            } => self.apply_lock_acquire(
                &namespace,
                &name,
                &owner,
                ttl_ms,
                fence_token,
                owner_metadata,
                now_ms,
            ),

            RaftRequest::LockRelease {
                namespace,
                name,
                owner,
                fence_token,
            } => self.apply_lock_release(&namespace, &name, &owner, fence_token),

            RaftRequest::LockRenew {
                namespace,
                name,
                owner,
                ttl_ms,
                now_ms,
            } => self.apply_lock_renew(&namespace, &name, &owner, ttl_ms, now_ms),

            RaftRequest::LockForceRelease { namespace, name } => {
                self.apply_lock_force_release(&namespace, &name)
            }

            RaftRequest::LockExpire { namespace, name } => {
                self.apply_lock_expire(&namespace, &name)
            }

            RaftRequest::PluginWrite {
                plugin_id,
                op_type,
                payload,
            } => {
                if let Some(handler) = hooks.plugins.get(&plugin_id) {
                    handler.apply(&self.db, &op_type, &payload, log_index)
                } else {
                    warn!(
                        plugin_id = %plugin_id,
                        op_type = %op_type,
                        "No plugin handler registered — PluginWrite ignored"
                    );
                    RaftResponse::success()
                }
            }

            RaftRequest::HealthCheckStatusUpdate {
                check_key,
                status,
                output,
                response_time_ms,
                timestamp_ms,
            } => {
                // status arrives as "passing"/"warning"/"critical"; the
                // active-probe path encodes only success vs failure, but we
                // accept the same string format for symmetry with TTL updates
                // and forward-compat. Map to (success, status_label) for the
                // hook.
                let success = matches!(status.as_str(), "passing" | "warning");
                if let Some(hook) = hooks.health_check.as_ref() {
                    hook.on_status_update(
                        &check_key,
                        success,
                        &output,
                        response_time_ms,
                        timestamp_ms,
                    );
                }
                RaftResponse::success()
            }

            RaftRequest::HealthCheckTtlUpdate {
                check_key,
                status,
                output,
                timestamp_ms,
            } => {
                if let Some(hook) = hooks.health_check.as_ref() {
                    hook.on_ttl_update(&check_key, &status, output.as_deref(), timestamp_ms);
                }
                RaftResponse::success()
            }
        }
    }

    // Namespace operations
    fn apply_namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: Option<String>,
        now_ms: i64,
    ) -> RaftResponse {
        let key = Self::namespace_key(namespace_id);
        let now = now_ms;
        let stored = StoredNamespace {
            namespace_id: namespace_id.to_string(),
            namespace_name: namespace_name.to_string(),
            namespace_desc,
            created_time: now,
            modified_time: now,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => {
                return RaftResponse::failure(format!("Failed to encode namespace: {}", e));
            }
        };
        match self.db.put_cf_opt(
            self.cf_namespace(),
            key.as_bytes(),
            &encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Namespace created: {}", namespace_id);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to create namespace: {}", e);
                RaftResponse::failure(format!("Failed to create namespace: {}", e))
            }
        }
    }

    fn apply_namespace_update(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: Option<String>,
        now_ms: i64,
    ) -> RaftResponse {
        let key = Self::namespace_key(namespace_id);
        let now = now_ms;

        // Preserve created_time from existing entry if present.
        let created_time = match self.db.get_cf(self.cf_namespace(), key.as_bytes()) {
            Ok(Some(bytes)) => bincode::deserialize::<StoredNamespace>(&bytes)
                .map(|s| s.created_time)
                .unwrap_or(now),
            _ => now,
        };

        let stored = StoredNamespace {
            namespace_id: namespace_id.to_string(),
            namespace_name: namespace_name.to_string(),
            namespace_desc,
            created_time,
            modified_time: now,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => {
                return RaftResponse::failure(format!("Failed to encode namespace: {}", e));
            }
        };
        match self.db.put_cf_opt(
            self.cf_namespace(),
            key.as_bytes(),
            &encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Namespace updated: {}", namespace_id);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to update namespace: {}", e);
                RaftResponse::failure(format!("Failed to update namespace: {}", e))
            }
        }
    }

    fn apply_namespace_delete(&self, namespace_id: &str) -> RaftResponse {
        let key = Self::namespace_key(namespace_id);

        match self
            .db
            .delete_cf_opt(self.cf_namespace(), key.as_bytes(), &self.write_opts)
        {
            Ok(_) => {
                debug!("Namespace deleted: {}", namespace_id);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to delete namespace: {}", e);
                RaftResponse::failure(format!("Failed to delete namespace: {}", e))
            }
        }
    }

    // User operations
    fn apply_user_create(
        &self,
        username: &str,
        password_hash: &str,
        enabled: bool,
        source: &str,
        now_ms: i64,
    ) -> RaftResponse {
        let key = Self::user_key(username);
        let now = now_ms;
        let stored = StoredUser {
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            enabled,
            created_time: now,
            modified_time: now,
            source: source.to_string(),
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("Failed to encode user: {}", e)),
        };
        match self
            .db
            .put_cf_opt(self.cf_users(), key.as_bytes(), &encoded, &self.write_opts)
        {
            Ok(_) => {
                debug!("User created: {}", username);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to create user: {}", e);
                RaftResponse::failure(format!("Failed to create user: {}", e))
            }
        }
    }

    fn apply_user_update(
        &self,
        username: &str,
        password_hash: Option<String>,
        enabled: Option<bool>,
        now_ms: i64,
    ) -> RaftResponse {
        let key = Self::user_key(username);

        let mut stored: StoredUser = match self.db.get_cf(self.cf_users(), key.as_bytes()) {
            Ok(Some(bytes)) => match bincode::deserialize(&bytes) {
                Ok(s) => s,
                Err(e) => {
                    return RaftResponse::failure(format!("Failed to decode user: {}", e));
                }
            },
            _ => return RaftResponse::failure("User not found"),
        };

        if let Some(hash) = password_hash {
            stored.password_hash = hash;
        }
        if let Some(en) = enabled {
            stored.enabled = en;
        }
        stored.modified_time = now_ms;

        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("Failed to encode user: {}", e)),
        };
        match self
            .db
            .put_cf_opt(self.cf_users(), key.as_bytes(), &encoded, &self.write_opts)
        {
            Ok(_) => {
                debug!("User updated: {}", username);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to update user: {}", e);
                RaftResponse::failure(format!("Failed to update user: {}", e))
            }
        }
    }

    fn apply_user_delete(&self, username: &str) -> RaftResponse {
        let key = Self::user_key(username);

        match self
            .db
            .delete_cf_opt(self.cf_users(), key.as_bytes(), &self.write_opts)
        {
            Ok(_) => {
                debug!("User deleted: {}", username);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to delete user: {}", e);
                RaftResponse::failure(format!("Failed to delete user: {}", e))
            }
        }
    }

    // Role operations
    fn apply_role_create(&self, role: &str, username: &str, now_ms: i64) -> RaftResponse {
        let key = Self::role_key(role, username);
        let stored = StoredRole {
            role: role.to_string(),
            username: username.to_string(),
            created_time: now_ms,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("Failed to encode role: {}", e)),
        };
        match self
            .db
            .put_cf_opt(self.cf_roles(), key.as_bytes(), &encoded, &self.write_opts)
        {
            Ok(_) => {
                debug!("Role assigned: {} -> {}", role, username);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to assign role: {}", e);
                RaftResponse::failure(format!("Failed to assign role: {}", e))
            }
        }
    }

    fn apply_role_delete(&self, role: &str, username: &str) -> RaftResponse {
        let key = Self::role_key(role, username);

        match self
            .db
            .delete_cf_opt(self.cf_roles(), key.as_bytes(), &self.write_opts)
        {
            Ok(_) => {
                debug!("Role removed: {} -> {}", role, username);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to remove role: {}", e);
                RaftResponse::failure(format!("Failed to remove role: {}", e))
            }
        }
    }

    // Permission operations
    fn apply_permission_grant(
        &self,
        role: &str,
        resource: &str,
        action: &str,
        now_ms: i64,
    ) -> RaftResponse {
        let key = Self::permission_key(role, resource, action);
        let stored = StoredPermission {
            role: role.to_string(),
            resource: resource.to_string(),
            action: action.to_string(),
            created_time: now_ms,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("Failed to encode permission: {}", e)),
        };
        match self.db.put_cf_opt(
            self.cf_permissions(),
            key.as_bytes(),
            &encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Permission granted: {} -> {}:{}", role, resource, action);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to grant permission: {}", e);
                RaftResponse::failure(format!("Failed to grant permission: {}", e))
            }
        }
    }

    fn apply_permission_revoke(&self, role: &str, resource: &str, action: &str) -> RaftResponse {
        let key = Self::permission_key(role, resource, action);

        match self
            .db
            .delete_cf_opt(self.cf_permissions(), key.as_bytes(), &self.write_opts)
        {
            Ok(_) => {
                debug!("Permission revoked: {} -> {}:{}", role, resource, action);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to revoke permission: {}", e);
                RaftResponse::failure(format!("Failed to revoke permission: {}", e))
            }
        }
    }

    // Instance operations
    fn apply_instance_register(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
        ip: &str,
        port: u16,
        weight: f64,
        healthy: bool,
        enabled: bool,
        metadata: &str,
        cluster_name: &str,
        now_ms: i64,
    ) -> RaftResponse {
        let key = Self::instance_key(namespace_id, group_name, service_name, instance_id);
        let now = now_ms;
        let stored = StoredInstance {
            namespace_id: namespace_id.to_string(),
            group_name: group_name.to_string(),
            service_name: service_name.to_string(),
            instance_id: instance_id.to_string(),
            ip: ip.to_string(),
            port,
            weight,
            healthy,
            enabled,
            metadata: metadata.to_string(),
            cluster_name: cluster_name.to_string(),
            registered_time: now,
            modified_time: now,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to bincode-encode StoredInstance: {}", e);
                return RaftResponse::failure(format!("encode error: {}", e));
            }
        };

        match self.db.put_cf_opt(
            self.cf_instances(),
            key.as_bytes(),
            encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Instance registered: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to register instance: {}", e);
                RaftResponse::failure(format!("Failed to register instance: {}", e))
            }
        }
    }

    fn apply_instance_deregister(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
    ) -> RaftResponse {
        let key = Self::instance_key(namespace_id, group_name, service_name, instance_id);

        match self
            .db
            .delete_cf_opt(self.cf_instances(), key.as_bytes(), &self.write_opts)
        {
            Ok(_) => {
                debug!("Instance deregistered: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to deregister instance: {}", e);
                RaftResponse::failure(format!("Failed to deregister instance: {}", e))
            }
        }
    }

    fn apply_instance_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
        ip: Option<String>,
        port: Option<u16>,
        weight: Option<f64>,
        healthy: Option<bool>,
        enabled: Option<bool>,
        metadata: Option<String>,
        now_ms: i64,
    ) -> RaftResponse {
        let key = Self::instance_key(namespace_id, group_name, service_name, instance_id);

        // Read existing instance as typed StoredInstance (bincode)
        let mut stored: StoredInstance = match self.db.get_cf(self.cf_instances(), key.as_bytes()) {
            Ok(Some(bytes)) => match bincode::deserialize(&bytes) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to decode StoredInstance: {}", e);
                    return RaftResponse::failure(format!("decode error: {}", e));
                }
            },
            _ => return RaftResponse::failure("Instance not found"),
        };

        if let Some(ip) = ip {
            stored.ip = ip;
        }
        if let Some(port) = port {
            stored.port = port;
        }
        if let Some(weight) = weight {
            stored.weight = weight;
        }
        if let Some(healthy) = healthy {
            stored.healthy = healthy;
        }
        if let Some(enabled) = enabled {
            stored.enabled = enabled;
        }
        if let Some(metadata) = metadata {
            stored.metadata = metadata;
        }
        stored.modified_time = now_ms;

        let encoded = match bincode::serialize(&stored) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to encode StoredInstance: {}", e);
                return RaftResponse::failure(format!("encode error: {}", e));
            }
        };

        match self.db.put_cf_opt(
            self.cf_instances(),
            key.as_bytes(),
            encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Instance updated: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to update instance: {}", e);
                RaftResponse::failure(format!("Failed to update instance: {}", e))
            }
        }
    }

    /// Durable meta write, executed on the blocking pool.
    fn persist_meta_sync(&self, key: &'static [u8], bytes: &[u8]) -> Result<(), std::io::Error> {
        self.db
            .put_cf_opt(self.cf_meta(), key, bytes, &sync_write_options())
            .map_err(io_error)
    }

    /// Persist a state-machine marker without occupying the async worker.
    async fn persist_meta(&self, key: &'static [u8], bytes: Vec<u8>) -> Result<(), std::io::Error> {
        let handle = self.handle();
        blocking_apply_io(move || handle.persist_meta_sync(key, &bytes)).await?
    }

    /// Save last applied log ID
    async fn save_last_applied(&self, log_id: LogId) -> Result<(), std::io::Error> {
        let bytes = bincode::serialize(&log_id).map_err(io_error)?;
        self.persist_meta(KEY_LAST_APPLIED, bytes).await?;

        *self.last_applied.write().await = Some(log_id);
        Ok(())
    }

    /// Save membership
    async fn save_membership(&self, membership: StoredMembership) -> Result<(), std::io::Error> {
        let bytes = bincode::serialize(&membership).map_err(io_error)?;
        self.persist_meta(KEY_LAST_MEMBERSHIP, bytes).await?;

        *self.last_membership.write().await = membership;
        Ok(())
    }

    /// Column families to stream into a snapshot, in the order they are
    /// written.
    ///
    /// The ordered, de-duplicated list is computed once per registration and
    /// stored inside the immutable `HookSet`, so enumerating it here is a
    /// single `Arc` clone. Core CFs come first, then the CFs each registered
    /// plugin declares (plugins are iterated by id), which keeps every
    /// replica's snapshot byte-identical for the same state. The list and the
    /// registry it describes are replaced atomically together.
    async fn snapshot_cf_names(&self) -> Arc<Vec<String>> {
        let guard = self.hooks.read().await;
        Arc::clone(&guard.snapshot_cf_names)
    }
}

impl RaftSnapshotBuilder<TypeConfig> for RocksStateMachine {
    type SnapshotData = SnapshotFile;

    async fn build_snapshot(&mut self) -> Result<Snapshot, std::io::Error> {
        let cf_names = self.snapshot_cf_names().await;
        let last_applied = *self.last_applied.read().await;
        let last_membership = self.last_membership.read().await.clone();

        // openraft 0.10 identifies a snapshot by the log position it covers;
        // the metadata no longer carries a transfer id.
        let meta = SnapshotMeta {
            last_log_id: last_applied,
            last_membership,
        };

        // Stream every column family straight into the snapshot file. Encoding
        // while iterating keeps peak memory flat; the previous implementation
        // collected every CF into a HashMap first, which cost as much memory as
        // the snapshot itself on top of the encoded bytes.
        let staged = match self.prepare_staging_dir().await? {
            Some(temp) => temp.join(SNAPSHOT_DATA_FILE),
            None => self.transient_snapshot_path.clone(),
        };
        if let Some(parent) = staged.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(io_error)?;
        }

        let file = tokio::fs::File::create(&staged).await.map_err(io_error)?;
        let mut writer = SnapshotWriter::new(BufWriter::with_capacity(
            snapshot_format::IO_BUFFER_SIZE,
            file,
        ));
        writer.write_header().await.map_err(io_error)?;

        let mut entries = 0usize;
        let mut bytes = 0u64;

        // The RocksDB iterator is synchronous and used to run on the actix
        // current_thread runtime. Move it to the blocking pool and stream
        // bounded owned records back to the async writer through a small
        // channel, so neither the iterator nor a full CF ever occupies `main`
        // or has to be materialised in memory.
        let (tx, mut rx) = tokio::sync::mpsc::channel::<SnapshotRecord>(SNAPSHOT_CHANNEL_CAPACITY);
        let producer_db = self.db.clone();
        let producer_cf_names = cf_names;
        let producer = tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
            for cf_name in producer_cf_names.iter() {
                let Some(cf) = producer_db.cf_handle(cf_name) else {
                    continue;
                };
                if tx
                    .blocking_send(SnapshotRecord::StartCf(cf_name.clone()))
                    .is_err()
                {
                    // The async writer stopped; its own error is returned.
                    return Ok(());
                }

                let iter = producer_db.iterator_cf(cf, rocksdb::IteratorMode::Start);
                for item in iter {
                    let (key, value) = item.map_err(io_error)?;
                    if tx
                        .blocking_send(SnapshotRecord::Entry(key.to_vec(), value.to_vec()))
                        .is_err()
                    {
                        return Ok(());
                    }
                }
            }
            Ok(())
        });

        let mut write_error: Option<std::io::Error> = None;
        while let Some(record) = rx.recv().await {
            let result = match record {
                SnapshotRecord::StartCf(name) => {
                    writer.start_column_family(&name).await.map_err(io_error)
                }
                SnapshotRecord::Entry(key, value) => {
                    let len = (key.len() + value.len()) as u64;
                    let result = writer.write_entry(&key, &value).await.map_err(io_error);
                    if result.is_ok() {
                        entries += 1;
                        bytes += len;
                    }
                    result
                }
            };
            if let Err(e) = result {
                write_error = Some(e);
                break;
            }
        }

        // Dropping the receiver wakes a producer blocked on a full channel.
        drop(rx);
        let producer_result = producer
            .await
            .map_err(|e| io_error(format!("snapshot build task failed: {e}")))?;
        if let Some(e) = write_error {
            return Err(e);
        }
        producer_result?;

        writer.finish().await.map_err(io_error)?;
        writer
            .into_inner()
            .into_inner()
            .sync_all()
            .await
            .map_err(io_error)?;

        // Publish under snapshot_{index} so the node can restart from this
        // snapshot instead of replaying a log that may already be purged.
        let published = match self.snapshot_dir.as_ref() {
            Some(_) => {
                let temp = self
                    .snapshot_dir
                    .as_ref()
                    .expect("checked above")
                    .join(SNAPSHOT_TEMP_DIR);
                self.commit_staging_dir(&temp, &meta).await?
            }
            None => staged,
        };

        info!(
            "Built snapshot at log index {} with {} entries ({} bytes)",
            last_applied.map(|l| l.index).unwrap_or(0),
            entries,
            bytes
        );

        Ok(Snapshot {
            meta,
            snapshot: SnapshotFile::open(&published).await.map_err(io_error)?,
        })
    }
}

impl RaftStateMachine<TypeConfig> for RocksStateMachine {
    type SnapshotData = SnapshotFile;
    type SnapshotBuilder = Self;

    async fn applied_state(&mut self) -> Result<(Option<LogId>, StoredMembership), std::io::Error> {
        let last_applied = *self.last_applied.read().await;
        let last_membership = self.last_membership.read().await.clone();
        Ok((last_applied, last_membership))
    }

    async fn apply<Strm>(&mut self, mut entries: Strm) -> Result<(), std::io::Error>
    where
        Strm: futures::Stream<Item = Result<EntryResponder<TypeConfig>, std::io::Error>>
            + Unpin
            + OptionalSend,
    {
        let mut last_log_id: Option<LogId> = None;

        // openraft 0.10 streams entries paired with a responder, so a client
        // write is answered as soon as its own entry has been applied.
        while let Some((entry, responder)) = entries.try_next().await? {
            let log_id = entry.log_id;

            let response = match entry.payload {
                EntryPayload::Normal(request) => {
                    // Resolve shared handles on the async thread, then move every
                    // RocksDB read/write to the blocking pool. The join is awaited
                    // before the next entry is pulled, so apply order, responder
                    // semantics and durability are unchanged.
                    let hooks = self.apply_hooks().await;
                    let handle = self.handle();
                    blocking_apply_io(move || handle.apply_request(request, log_id.index, &hooks))
                        .await?
                }
                EntryPayload::Membership(membership) => {
                    let stored = StoredMembership::new(Some(log_id), membership);
                    self.save_membership(stored).await?;
                    RaftResponse::success()
                }
                EntryPayload::Blank => RaftResponse::success(),
            };

            last_log_id = Some(log_id);
            if let Some(responder) = responder {
                responder.send(response);
            }
        }

        // Persist last_applied once after the whole stream, not per entry.
        if let Some(log_id) = last_log_id {
            self.save_last_applied(log_id).await?;
        }

        Ok(())
    }

    async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot>, std::io::Error> {
        let Some(path) = self.latest_snapshot_dir().await else {
            return Ok(None);
        };

        let meta_bytes = match tokio::fs::read(path.join(SNAPSHOT_META_FILE)).await {
            Ok(bytes) => bytes,
            Err(e) => {
                warn!("Unreadable snapshot meta at {:?}: {}", path, e);
                return Ok(None);
            }
        };
        let meta: SnapshotMeta = match bincode::deserialize(&meta_bytes) {
            Ok(meta) => meta,
            Err(e) => {
                warn!("Corrupt snapshot meta at {:?}: {}", path, e);
                return Ok(None);
            }
        };

        let data_path = path.join(SNAPSHOT_DATA_FILE);
        let body = match SnapshotFile::open(&data_path).await {
            Ok(body) => body,
            Err(e) => {
                warn!("Unreadable snapshot data at {:?}: {}", data_path, e);
                return Ok(None);
            }
        };
        let bytes = body.len().await.unwrap_or(0);

        info!(
            "Loaded snapshot at {:?} from disk ({} bytes)",
            meta.last_log_id, bytes
        );

        Ok(Some(Snapshot {
            meta,
            snapshot: body,
        }))
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        // Snapshot builder uses default WriteOptions (snapshots don't need the same tuning)
        RocksStateMachine {
            inner: Arc::new(StateMachineInner {
                db: self.db.clone(),
                last_applied: RwLock::new(*self.last_applied.read().await),
                last_membership: RwLock::new(self.last_membership.read().await.clone()),
                // Clone the LIVE hook set, not a fresh empty one. The snapshot
                // builder's `snapshot_cf_names()` reads the plugin registry in
                // this set to enumerate plugin column families
                // (Consul/Apollo/visibility etc.). Building from an empty set
                // would silently drop every plugin CF from the snapshot, so a
                // follower restored from it would lose all plugin data.
                hooks: self.hooks.clone(),
                write_opts: rocksdb::WriteOptions::default(),
                snapshot_dir: self.snapshot_dir.clone(),
                transient_snapshot_path: self.transient_snapshot_path.clone(),
            }),
        }
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta,
        snapshot: SnapshotFile,
    ) -> Result<(), std::io::Error> {
        let source = snapshot.path().to_path_buf();

        // Verify before publishing or applying: a corrupt snapshot must never
        // become this node's recovery point, and must never half-restore.
        snapshot_format::verify_file(&source)
            .await
            .map_err(io_error)?;

        // A snapshot received from the leader is just as good as one we built,
        // so publish it under snapshot_{index}: without it this node would have
        // nothing to recover from until it builds one of its own.
        let published = self.publish_snapshot_file(meta, &source).await?;
        let body = published.unwrap_or(source);

        let stats = self.restore_from_file(&body).await?;

        // Update metadata
        if let Some(log_id) = meta.last_log_id {
            self.save_last_applied(log_id).await?;
        }
        self.save_membership(meta.last_membership.clone()).await?;

        // The install rewrote every column family directly, so the apply-back
        // hook never ran for the replaced entries. Hand the new authoritative
        // set to the hook so the in-memory view cannot keep serving instances
        // the rest of the cluster already deleted.
        self.reconcile_naming_view().await;

        info!(
            "Snapshot installed at {:?} ({} column families, {} entries, {} bytes)",
            meta.last_log_id, stats.column_families, stats.entries, stats.bytes
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raft::reader::RocksDbReader;

    /// Fixed leader clock for lock tests: assertions must not depend on the
    /// wall clock, and two replicas applying the same entry must see the
    /// same value.
    const TEST_NOW_MS: i64 = 1_700_000_000_000;

    /// Apply work must not run on the async worker: on the actix
    /// `current_thread` runtime that thread also polls heartbeats and the
    /// election deadline notifier (see `docs/raft-timing.md` section 6).
    #[tokio::test(flavor = "current_thread")]
    async fn blocking_apply_io_runs_off_the_async_worker_thread() {
        let async_thread = std::thread::current().id();
        let blocking_thread = blocking_apply_io(|| std::thread::current().id())
            .await
            .expect("blocking apply task must succeed");
        assert_ne!(
            async_thread, blocking_thread,
            "state-machine RocksDB work must not occupy the async worker thread"
        );
    }

    /// Architecture guard. Reverting `apply()` to an inline `apply_request`
    /// call, making `apply_request` async again, or dropping the blocking hop
    /// from the helper must all turn this test red.
    #[test]
    fn apply_path_runs_on_the_blocking_pool() {
        let src = include_str!("mod.rs");
        // Exclude this test module: otherwise the guard's own literal strings
        // would be found in the source and mask a real regression.
        let tests_start = src
            .find("#[cfg(test)]\nmod tests")
            .expect("tests module must exist");
        let production = &src[..tests_start];
        assert!(
            production.contains("fn apply_request("),
            "apply_request must exist"
        );
        assert!(
            !production.contains("async fn apply_request("),
            "apply_request must stay synchronous so a blocking task can call it"
        );

        let start = production
            .find("async fn apply<Strm>")
            .expect("apply must exist");
        let end = production
            .find("async fn get_current_snapshot")
            .expect("get_current_snapshot must exist");
        let body = &production[start..end];
        assert!(
            body.contains("blocking_apply_io("),
            "apply must dispatch RocksDB work through the blocking pool"
        );
        assert!(
            body.contains(".await?"),
            "apply must await the blocking task before pulling the next entry"
        );

        let helper = production
            .find("async fn blocking_apply_io")
            .expect("blocking helper must exist");
        assert!(
            production[helper..helper + 700].contains("spawn_blocking"),
            "the helper must actually use spawn_blocking"
        );
    }

    /// Snapshot RocksDB work must not run on the async worker either.
    #[tokio::test(flavor = "current_thread")]
    async fn blocking_snapshot_io_runs_off_the_async_worker_thread() {
        let async_thread = std::thread::current().id();
        let blocking_thread =
            blocking_snapshot_io(|| Ok::<_, std::io::Error>(std::thread::current().id()))
                .await
                .expect("blocking snapshot task must succeed");
        assert_ne!(
            async_thread, blocking_thread,
            "snapshot RocksDB work must not occupy the async worker thread"
        );
    }

    #[tokio::test]
    async fn blocking_snapshot_io_propagates_io_errors() {
        let err = blocking_snapshot_io(|| Err::<(), _>(io_error("snapshot io failed")))
            .await
            .expect_err("snapshot IO failure must be propagated");
        assert!(err.to_string().contains("snapshot io failed"));
    }

    /// `spawn_blocking` must be able to feed the bounded snapshot channel.
    /// This guards the producer/consumer wiring used by `build_snapshot`.
    #[tokio::test(flavor = "current_thread")]
    async fn snapshot_record_channel_works_from_the_blocking_pool() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<SnapshotRecord>(1);
        let producer = tokio::task::spawn_blocking(move || {
            tx.blocking_send(SnapshotRecord::StartCf("cf_config".to_string()))
                .expect("receiver must be alive");
        });
        assert!(matches!(
            rx.recv().await,
            Some(SnapshotRecord::StartCf(name)) if name == "cf_config"
        ));
        producer.await.expect("blocking producer must finish");
    }

    /// Architecture guard: reverting any snapshot path to inline RocksDB work
    /// must turn this test red.
    #[test]
    fn snapshot_paths_use_the_blocking_pool() {
        let src = include_str!("mod.rs");
        let tests_start = src
            .find("#[cfg(test)]\nmod tests")
            .expect("tests module must exist");
        let production = &src[..tests_start];

        let restore = production
            .find("async fn restore_from_file")
            .expect("restore_from_file must exist");
        let restore_end = production
            .find("async fn prune_snapshots")
            .expect("prune_snapshots must exist");
        let restore_body = &production[restore..restore_end];
        assert!(
            restore_body.contains("flush_restore_batch_async("),
            "restore must dispatch WriteBatch writes through the blocking pool"
        );
        assert!(
            !restore_body.contains("self.db.write("),
            "restore must not write RocksDB inline"
        );
        assert!(
            !restore_body.contains("self.db.cf_handle("),
            "restore must not look up column families inline"
        );

        let flush = production
            .find("async fn flush_restore_batch_async")
            .expect("restore flush helper must exist");
        assert!(
            production[flush..flush + 600].contains("blocking_snapshot_io("),
            "the restore flush helper must use the blocking pool"
        );

        let reconcile = production
            .find("async fn reconcile_naming_view")
            .expect("reconcile_naming_view must exist");
        let reconcile_end = production
            .find("/// Enable on-disk snapshot persistence")
            .expect("with_snapshot_dir doc must exist");
        let reconcile_body = &production[reconcile..reconcile_end];
        assert!(
            reconcile_body.contains("blocking_snapshot_io("),
            "naming reconciliation must run RocksDB work on the blocking pool"
        );
        assert!(
            !reconcile_body.contains("self.db.iterator_cf("),
            "naming reconciliation must not iterate RocksDB inline"
        );

        let build = production
            .find("async fn build_snapshot")
            .expect("build_snapshot must exist");
        let build_end = production
            .find("async fn applied_state")
            .expect("applied_state must exist");
        let build_body = &production[build..build_end];
        assert!(
            build_body.contains("spawn_blocking("),
            "snapshot build must iterate RocksDB on the blocking pool"
        );
        assert!(
            build_body.contains("mpsc::channel"),
            "snapshot build must stream records through a bounded channel"
        );
        assert!(
            !build_body.contains("self.db.iterator_cf("),
            "snapshot build must not iterate RocksDB inline"
        );
        assert!(
            !build_body.contains("self.db.cf_handle("),
            "snapshot build must not look up column families inline"
        );

        let helper = production
            .find("async fn blocking_snapshot_io")
            .expect("blocking snapshot helper must exist");
        assert!(
            production[helper..helper + 700].contains("spawn_blocking"),
            "the helper must actually use spawn_blocking"
        );
    }

    /// Success path: the migrated durable marker still reaches RocksDB and
    /// updates the in-memory view.
    #[tokio::test]
    async fn save_last_applied_still_persists_after_blocking_migration() {
        use crate::raft::types::CommittedLeaderId;

        let dir = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(dir.path())
            .await
            .expect("state machine");
        let log_id = LogId::new(
            CommittedLeaderId {
                term: 3,
                node_id: 1,
            },
            42,
        );

        sm.save_last_applied(log_id)
            .await
            .expect("persist last applied");
        assert_eq!(*sm.last_applied.read().await, Some(log_id));

        let bytes = sm
            .db
            .get_cf(sm.cf_meta(), KEY_LAST_APPLIED)
            .expect("meta read")
            .expect("last applied marker");
        let decoded: LogId = bincode::deserialize(&bytes).expect("decode marker");
        assert_eq!(decoded, log_id);
    }

    /// The blocking apply closure owns all shared handles it may dispatch to.
    #[tokio::test]
    async fn apply_hooks_snapshot_covers_plugins_and_hook_slots() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(dir.path())
            .await
            .expect("state machine");
        let hooks = sm.apply_hooks().await;
        assert!(hooks.naming.is_none());
        assert!(hooks.health_check.is_none());
        assert!(hooks.plugins.get("missing").is_none());
    }

    struct NoopPluginHandler;

    impl RaftPluginHandler for NoopPluginHandler {
        fn plugin_id(&self) -> &str {
            "noop"
        }

        fn column_families(&self) -> Vec<String> {
            Vec::new()
        }

        fn apply(
            &self,
            _db: &DB,
            _op_type: &str,
            _payload: &[u8],
            _log_index: u64,
        ) -> RaftResponse {
            RaftResponse::success()
        }

        fn build_snapshot(&self, _db: &DB) -> Result<Vec<u8>, String> {
            Ok(Vec::new())
        }

        fn install_snapshot(&self, _db: &DB, _data: &[u8]) -> Result<(), String> {
            Ok(())
        }
    }

    /// The per-apply handle set is one O(1) `Arc` clone, and registration
    /// replaces the inner `Arc` without mutating a snapshot already handed to
    /// a blocking apply.
    #[tokio::test]
    async fn apply_hooks_clones_the_inner_hook_set_arc() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(dir.path())
            .await
            .expect("state machine");
        let shared = sm.hooks();
        let before = Arc::clone(&*shared.read().await);
        let hooks = sm.apply_hooks().await;
        assert!(
            Arc::ptr_eq(&before, &hooks),
            "apply_hooks must clone the inner Arc<HookSet>, not rebuild it"
        );

        sm.register_plugin(Arc::new(NoopPluginHandler))
            .await
            .expect("register plugin");
        let after = Arc::clone(&*shared.read().await);
        assert!(
            !Arc::ptr_eq(&before, &after),
            "registration must replace the inner Arc so subsequent applies see it"
        );
        assert!(after.plugins.get("noop").is_some());
        assert!(
            before.plugins.get("noop").is_none(),
            "a previous apply snapshot must stay immutable"
        );
        assert!(sm.apply_hooks().await.plugins.get("noop").is_some());
    }

    /// Architecture guard: reverting to three independent hook slots, cloning
    /// the registry `BTreeMap` per apply, or giving the snapshot builder a
    /// fresh empty set must all turn this test red.
    #[test]
    fn apply_hooks_uses_one_atomic_shared_hook_set() {
        let src = include_str!("mod.rs");
        let tests_start = src
            .find("#[cfg(test)]\nmod tests")
            .expect("tests module must exist");
        let production = &src[..tests_start];

        let start = production
            .find("async fn apply_hooks(&self) -> ApplyHooks")
            .expect("apply_hooks must exist");
        let end = production[start..]
            .find("    fn apply_request(")
            .expect("apply_request must exist");
        let body = &production[start..start + end];
        assert_eq!(
            body.matches("read().await").count(),
            1,
            "apply_hooks must take exactly one read lock"
        );
        assert!(
            body.contains("self.hooks.read().await") && body.contains("Arc::clone(&guard)"),
            "apply_hooks must clone the single inner Arc<HookSet>"
        );
        assert!(
            !body.contains("PluginRegistry::clone("),
            "apply_hooks must never clone the registry BTreeMap"
        );
        assert!(
            !body.contains("self.naming_hook") && !body.contains("self.health_check_hook"),
            "no per-slot hook lock may remain on the apply path"
        );

        assert!(
            production.contains("hooks: SharedHooks"),
            "the state machine must hold exactly one shared hook set"
        );
        assert!(
            !production.contains("plugin_registry: SharedPluginRegistry")
                && !production.contains("naming_hook: SharedNamingHook")
                && !production.contains("health_check_hook: SharedHealthCheckHook"),
            "the three legacy slots must not come back"
        );

        let snap = production
            .find("async fn snapshot_cf_names")
            .expect("snapshot_cf_names must exist");
        let snap_body = &production[snap..snap + 500];
        assert!(
            snap_body.contains("Arc::clone(&guard.snapshot_cf_names)"),
            "snapshot CF enumeration must read the cached list"
        );
        assert!(
            !snap_body.contains("all_snapshot_column_families()"),
            "snapshot CF enumeration must not recompute the list per snapshot"
        );

        let builder = production
            .find("async fn get_snapshot_builder")
            .expect("get_snapshot_builder must exist");
        assert!(
            production[builder..builder + 1200].contains("hooks: self.hooks.clone()"),
            "the snapshot builder must share the LIVE hook set"
        );

        let reg = production
            .find("pub async fn register_plugin")
            .expect("register_plugin must exist");
        let reg_end = production
            .find("/// Create a new RocksDB state machine with default configuration")
            .expect("new doc must exist");
        assert!(
            production[reg..reg_end].contains("register_plugin_handler(&self.hooks, handler)"),
            "registration must go through the atomic shared installer"
        );
    }

    /// Publish the same config entry on two independent replicas and compare
    /// the raw RocksDB bytes. A local `Utc::now()` in apply() would make the
    /// config and its history row differ from node to node.
    #[tokio::test]
    async fn config_publish_is_replica_independent() {
        use crate::raft::request::ConfigHistoryInfo;

        let a_dir = tempfile::tempdir().expect("tempdir");
        let b_dir = tempfile::tempdir().expect("tempdir");
        let a = RocksStateMachine::new(a_dir.path())
            .await
            .expect("state machine a");
        let b = RocksStateMachine::new(b_dir.path())
            .await
            .expect("state machine b");

        let publish = |sm: &RocksStateMachine| {
            sm.apply_config_publish_batched(
                "app.properties",
                "DEFAULT_GROUP",
                "public",
                "key=value",
                "md5-test",
                42,
                Some("properties".to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(ConfigHistoryInfo {
                    op_type: String::new(),
                    publish_type: Some("formal".to_string()),
                    ext_info: None,
                }),
                TEST_NOW_MS,
            )
        };

        let ra = publish(&a);
        let rb = publish(&b);
        assert!(ra.success && rb.success, "both replicas must publish");

        let key = RocksStateMachine::config_key("app.properties", "DEFAULT_GROUP", "public");
        let va =
            a.db.get_cf(a.cf_config(), key.as_bytes())
                .expect("read a")
                .expect("config present on a");
        let vb =
            b.db.get_cf(b.cf_config(), key.as_bytes())
                .expect("read b")
                .expect("config present on b");
        assert_eq!(va, vb, "replicas must store byte-identical config state");

        let stored: StoredConfig = bincode::deserialize(&va).expect("decode config");
        assert_eq!(stored.created_time, TEST_NOW_MS);
        assert_eq!(stored.modified_time, TEST_NOW_MS);

        let history_key =
            RocksStateMachine::config_history_key("app.properties", "DEFAULT_GROUP", "public", 42);
        let ha =
            a.db.get_cf(a.cf_config_history(), history_key.as_bytes())
                .expect("read history a")
                .expect("history present on a");
        let hb =
            b.db.get_cf(b.cf_config_history(), history_key.as_bytes())
                .expect("read history b")
                .expect("history present on b");
        assert_eq!(ha, hb, "replicas must store byte-identical history state");

        let history: StoredConfigHistory = bincode::deserialize(&ha).expect("decode history");
        assert_eq!(history.created_time, TEST_NOW_MS);
        assert_eq!(history.modified_time, TEST_NOW_MS);
    }

    /// A corrupt persisted value must not be treated as "config absent":
    /// otherwise CAS would be bypassed, op_type misreported as Insert and
    /// created_time silently reset.
    #[tokio::test]
    async fn config_publish_rejects_corrupt_existing_value() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(dir.path())
            .await
            .expect("state machine");
        let key = RocksStateMachine::config_key("app.properties", "DEFAULT_GROUP", "public");
        let corrupt = vec![0xff_u8; 16];
        sm.db
            .put_cf(sm.cf_config(), key.as_bytes(), &corrupt)
            .expect("seed corrupt row");

        let resp = sm.apply_config_publish_batched(
            "app.properties",
            "DEFAULT_GROUP",
            "public",
            "key=value",
            "md5-test",
            43,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            TEST_NOW_MS,
        );
        assert!(!resp.success, "corrupt row must abort the publish");
        let after = sm
            .db
            .get_cf(sm.cf_config(), key.as_bytes())
            .expect("read")
            .expect("row still present");
        assert_eq!(after, corrupt, "publish must not overwrite the row");
    }

    /// A corrupt persisted lock record must NOT be treated as "lock free":
    /// doing so lets a second owner acquire a lock that is actually held,
    /// which silently breaks mutual exclusion. Fail closed instead.
    ///
    /// Mutant check: dropping the `Err(e) => return failure` branch in
    /// `apply_lock_acquire`'s read/decode makes a corrupt record look like an
    /// absent one, so `acquire` would return success and overwrite the row —
    /// this assertion (`!success` + row untouched) would then fail.
    #[tokio::test]
    async fn lock_acquire_rejects_corrupt_existing_record() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(dir.path())
            .await
            .expect("state machine");
        let key = RocksStateMachine::lock_key("public", "k1");
        let corrupt = vec![0xff_u8; 16];
        sm.db
            .put_cf(sm.cf_locks(), key.as_bytes(), &corrupt)
            .expect("seed corrupt lock row");

        let resp = sm.apply_lock_acquire("public", "k1", "owner-b", 60_000, 7, None, TEST_NOW_MS);
        assert!(
            !resp.success,
            "decode failure on an existing lock must abort the acquire"
        );
        let after = sm
            .db
            .get_cf(sm.cf_locks(), key.as_bytes())
            .expect("read")
            .expect("row still present");
        assert_eq!(after, corrupt, "acquire must not overwrite the corrupt row");
    }

    /// Success paths for `apply_lock_expire`: an absent lock is a no-op, an
    /// existing lock is written as `Expired`. Together with the pure
    /// `expire_check` tests this also catches an "always fail" mutant.
    #[tokio::test]
    async fn lock_expire_handles_absent_and_existing_locks() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(dir.path())
            .await
            .expect("state machine");

        // Absent: success, no row created.
        let resp = sm.apply_lock_expire("public", "missing");
        assert!(resp.success, "expiring an absent lock must be a no-op");
        let key = RocksStateMachine::lock_key("public", "missing");
        assert_eq!(
            sm.db
                .get_cf(sm.cf_locks(), key.as_bytes())
                .expect("read")
                .map(|b| b.to_vec()),
            None
        );

        // Existing: succeeds and writes the Expired marker.
        let acquired =
            sm.apply_lock_acquire("public", "k1", "owner-a", 60_000, 1, None, TEST_NOW_MS);
        assert!(acquired.success, "acquire must succeed");
        let resp = sm.apply_lock_expire("public", "k1");
        assert!(resp.success, "expiring an existing lock must succeed");
        let row = sm
            .db
            .get_cf(
                sm.cf_locks(),
                RocksStateMachine::lock_key("public", "k1").as_bytes(),
            )
            .expect("read")
            .expect("expired row exists");
        let value: serde_json::Value = serde_json::from_slice(&row).expect("valid JSON");
        assert_eq!(value["state"], "Expired");
        assert_eq!(value["fence_token"], 0);
    }

    /// A corrupt persisted gray config record must NOT be treated as a passing
    /// CAS check: that would let a write overwrite real gray config data. Fail
    /// closed, matching the non-gray `apply_config_publish` path.
    ///
    /// Mutant check: removing the `Err(e) => return failure` branch in the
    /// gray CAS decode makes a corrupt record fall through as "no conflict",
    /// so the publish would succeed and overwrite the row — caught by the
    /// `!success` + row-untouched assertions.
    #[tokio::test]
    async fn gray_publish_rejects_corrupt_existing_record() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(dir.path())
            .await
            .expect("state machine");
        let key = RocksStateMachine::config_gray_key("d", "g", "t", "beta");
        let corrupt = vec![0xff_u8; 16];
        sm.db
            .put_cf(sm.cf_config_gray(), key.as_bytes(), &corrupt)
            .expect("seed corrupt gray row");

        let resp = sm.apply_config_gray_publish(
            "d",
            "g",
            "t",
            "content",
            "beta",
            "rule",
            None,
            None,
            None,
            None,
            Some("anything".to_string()), // cas_md5 triggers the read+decode path
            TEST_NOW_MS,
        );
        assert!(
            !resp.success,
            "decode failure on an existing gray config must abort the publish"
        );
        let after = sm
            .db
            .get_cf(sm.cf_config_gray(), key.as_bytes())
            .expect("read")
            .expect("row still present");
        assert_eq!(after, corrupt, "publish must not overwrite the corrupt row");
    }

    /// History pagination must clamp `page_size` at `MAX_PAGE_SIZE` (1000)
    /// via the shared `clamp_page` helper. Without the clamp a caller
    /// requesting `u64::MAX` would serialize the entire matched history into
    /// one Vec of JSON values.
    ///
    /// Mutant check: removing `clamp_page` makes `take(u64::MAX)` return every
    /// matched row, so `page.len()` would be `SEED` instead of the cap — this
    /// assertion would fail.
    #[tokio::test]
    async fn history_page_size_is_capped() {
        const CAP: u64 = 1000;
        const SEED: u64 = CAP + 500; // 1500 rows, well above the cap
        let dir = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(dir.path())
            .await
            .expect("state machine");
        let cf = sm.cf_config_history();
        for i in 0..SEED {
            let h = StoredConfigHistory {
                id: i as i64,
                data_id: "d".to_string(),
                group: "g".to_string(),
                tenant: "t".to_string(),
                content: "c".to_string(),
                md5: "m".to_string(),
                app_name: String::new(),
                src_user: None,
                src_ip: None,
                op_type: "U".to_string(),
                publish_type: "formal".to_string(),
                gray_name: String::new(),
                ext_info: String::new(),
                encrypted_data_key: String::new(),
                created_time: 0,
                modified_time: 0,
            };
            let key = RocksStateMachine::config_history_key("d", "g", "t", i);
            let encoded = bincode::serialize(&h).expect("encode history");
            sm.db
                .put_cf(&cf, key.as_bytes(), &encoded)
                .expect("seed history row");
        }

        let reader = RocksDbReader::from_state_machine(&sm);
        let (page, total) = reader
            .search_config_history("d", "g", "t", 1, u64::MAX)
            .expect("search");
        assert_eq!(total, SEED, "total must report every matched row");
        assert_eq!(
            page.len() as u64,
            CAP,
            "page_size must be clamped to the cap even when the caller asks for u64::MAX"
        );
    }

    #[test]
    fn test_key_generation() {
        let config_key = RocksStateMachine::config_key("test", "DEFAULT_GROUP", "public");
        assert_eq!(config_key, "public@@DEFAULT_GROUP@@test");

        let permission_key = RocksStateMachine::permission_key("admin", "public::*", "rw");
        assert_eq!(permission_key, "admin@@public::*@@rw");
    }

    #[test]
    fn test_config_key_format() {
        // Test edge cases
        let key = RocksStateMachine::config_key("", "", "");
        assert_eq!(key, "@@@@");

        let key_with_special = RocksStateMachine::config_key("data.id", "group-1", "ns:test");
        assert_eq!(key_with_special, "ns:test@@group-1@@data.id");
    }

    #[test]
    fn test_namespace_key() {
        let key = RocksStateMachine::namespace_key("public");
        assert_eq!(key, "public");

        let key_empty = RocksStateMachine::namespace_key("");
        assert_eq!(key_empty, "");
    }

    /// Reentrant lock semantics (P0-2 fix): same owner acquiring the same
    /// lock twice must increment `lock_count` and require matching releases
    /// to fully unlock. Previously acquire+acquire+release would drop the
    /// lock — incompatible with Nacos AtomicLockService semantics.
    /// Two replicas applying the same acquire entry must derive the same
    /// `expires_at`. Each replica used to call `Utc::now()` during apply, so
    /// the lock expired at different instants on different nodes and mutual
    /// exclusion was silently lost.
    #[tokio::test]
    async fn lock_acquire_expires_at_is_replica_independent() {
        let a_dir = tempfile::tempdir().expect("tempdir");
        let b_dir = tempfile::tempdir().expect("tempdir");
        let a = RocksStateMachine::new(a_dir.path())
            .await
            .expect("state machine a");
        let b = RocksStateMachine::new(b_dir.path())
            .await
            .expect("state machine b");

        let ra = a.apply_lock_acquire("public", "k1", "owner-a", 60_000, 1, None, TEST_NOW_MS);
        let rb = b.apply_lock_acquire("public", "k1", "owner-a", 60_000, 1, None, TEST_NOW_MS);
        assert!(ra.success && rb.success, "both replicas must acquire");

        let key = RocksStateMachine::lock_key("public", "k1");
        let va =
            a.db.get_cf(a.cf_locks(), key.as_bytes())
                .expect("read a")
                .expect("lock present on a");
        let vb =
            b.db.get_cf(b.cf_locks(), key.as_bytes())
                .expect("read b")
                .expect("lock present on b");
        assert_eq!(va, vb, "replicas must store byte-identical lock state");

        let v: serde_json::Value = serde_json::from_slice(&va).expect("lock json");
        assert_eq!(
            v["expires_at"].as_i64(),
            Some(TEST_NOW_MS + 60_000),
            "expires_at must come from the leader clock carried in the log entry"
        );
    }

    /// Two replicas applying the same namespace create entry must store
    /// byte-identical state. Each replica used to stamp its own `Utc::now()`,
    /// so `created_time`/`modified_time` differed from node to node.
    #[tokio::test]
    async fn namespace_create_is_replica_independent() {
        let a_dir = tempfile::tempdir().expect("tempdir");
        let b_dir = tempfile::tempdir().expect("tempdir");
        let a = RocksStateMachine::new(a_dir.path())
            .await
            .expect("state machine a");
        let b = RocksStateMachine::new(b_dir.path())
            .await
            .expect("state machine b");

        let ra = a.apply_namespace_create("ns", "name", None, TEST_NOW_MS);
        let rb = b.apply_namespace_create("ns", "name", None, TEST_NOW_MS);
        assert!(ra.success && rb.success, "both replicas must create");

        let key = RocksStateMachine::namespace_key("ns");
        let va =
            a.db.get_cf(a.cf_namespace(), key.as_bytes())
                .expect("read a")
                .expect("namespace present on a");
        let vb =
            b.db.get_cf(b.cf_namespace(), key.as_bytes())
                .expect("read b")
                .expect("namespace present on b");
        assert_eq!(va, vb, "replicas must store byte-identical namespace state");

        let stored: StoredNamespace = bincode::deserialize(&va).expect("decode namespace");
        assert_eq!(stored.created_time, TEST_NOW_MS);
        assert_eq!(stored.modified_time, TEST_NOW_MS);
    }

    #[tokio::test]
    async fn test_lock_reentrant_semantics() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(tmp.path())
            .await
            .expect("state machine");

        // First acquire: fresh lock, depth = 1
        let resp = sm.apply_lock_acquire("public", "k1", "owner-a", 60_000, 1, None, TEST_NOW_MS);
        assert!(resp.success, "first acquire must succeed");
        let v: serde_json::Value = serde_json::from_slice(&resp.data.unwrap()).unwrap();
        assert_eq!(v["lock_count"].as_u64(), Some(1));
        assert_eq!(v["fence_token"].as_u64(), Some(1));
        assert_eq!(v["owner"].as_str(), Some("owner-a"));

        // Second acquire by same owner: depth = 2, same fence token
        let resp = sm.apply_lock_acquire("public", "k1", "owner-a", 60_000, 99, None, TEST_NOW_MS);
        assert!(resp.success, "reentrant acquire must succeed");
        let v: serde_json::Value = serde_json::from_slice(&resp.data.unwrap()).unwrap();
        assert_eq!(v["lock_count"].as_u64(), Some(2));
        assert_eq!(
            v["fence_token"].as_u64(),
            Some(1),
            "fence token must remain stable across reentrant acquires"
        );

        // Third acquire by same owner: depth = 3
        let resp = sm.apply_lock_acquire("public", "k1", "owner-a", 60_000, 77, None, TEST_NOW_MS);
        assert!(resp.success);
        let v: serde_json::Value = serde_json::from_slice(&resp.data.unwrap()).unwrap();
        assert_eq!(v["lock_count"].as_u64(), Some(3));

        // Different owner is rejected while held
        let resp = sm.apply_lock_acquire("public", "k1", "owner-b", 60_000, 5, None, TEST_NOW_MS);
        assert!(!resp.success, "different owner must fail to acquire");

        // Release once: depth 3 -> 2, still locked
        let resp = sm.apply_lock_release("public", "k1", "owner-a", None);
        assert!(resp.success);
        let v: serde_json::Value = serde_json::from_slice(&resp.data.unwrap()).unwrap();
        assert_eq!(v["lock_count"].as_u64(), Some(2));
        assert_eq!(v["state"].as_str(), Some("Locked"));

        // Different owner still rejected
        let resp = sm.apply_lock_acquire("public", "k1", "owner-b", 60_000, 5, None, TEST_NOW_MS);
        assert!(!resp.success);

        // Release again: depth 2 -> 1, still locked
        let resp = sm.apply_lock_release("public", "k1", "owner-a", None);
        assert!(resp.success);

        // Final release: depth 1 -> 0, lock becomes Free
        let resp = sm.apply_lock_release("public", "k1", "owner-a", None);
        assert!(resp.success);

        // Now another owner can acquire
        let resp = sm.apply_lock_acquire("public", "k1", "owner-b", 60_000, 42, None, TEST_NOW_MS);
        assert!(resp.success, "new owner must acquire after full release");
        let v: serde_json::Value = serde_json::from_slice(&resp.data.unwrap()).unwrap();
        assert_eq!(v["owner"].as_str(), Some("owner-b"));
        assert_eq!(v["lock_count"].as_u64(), Some(1));
        assert_eq!(
            v["fence_token"].as_u64(),
            Some(42),
            "new owner gets its own fence token"
        );
    }

    /// Legacy entries without `lock_count` field must be treated as depth=1
    /// so existing on-disk Raft logs upgrade cleanly.
    #[tokio::test]
    async fn test_lock_legacy_entry_depth_one() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(tmp.path())
            .await
            .expect("state machine");

        // Write a legacy-shaped value directly (no lock_count field).
        let key = RocksStateMachine::lock_key("public", "legacy");
        let now = chrono::Utc::now().timestamp_millis();
        let legacy = serde_json::json!({
            "namespace": "public",
            "name": "legacy",
            "owner": "owner-a",
            "state": "Locked",
            "fence_token": 7,
            "ttl_ms": 60000,
            "acquired_at": now,
            "expires_at": now + 60000,
            "renewal_count": 0,
            "owner_metadata": null,
        });
        sm.db
            .put_cf_opt(
                sm.cf_locks(),
                key.as_bytes(),
                serde_json::to_vec(&legacy).unwrap(),
                &sm.write_opts,
            )
            .unwrap();

        // Single release must fully free it (legacy depth is 1).
        let resp = sm.apply_lock_release("public", "legacy", "owner-a", None);
        assert!(resp.success);
        let resp =
            sm.apply_lock_acquire("public", "legacy", "owner-b", 60_000, 8, None, TEST_NOW_MS);
        assert!(resp.success, "legacy entry must fully release in one call");
    }
}
