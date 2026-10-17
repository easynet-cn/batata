//! Plugin handler trait for extending Raft state machine with plugin-specific operations.
//!
//! Plugins register handlers that receive `PluginWrite` requests after Raft consensus.
//! The core state machine routes requests by `plugin_id` — it never interprets the payload.

use std::collections::BTreeMap;
use std::sync::Arc;

use rocksdb::DB;

use super::request::RaftResponse;

/// Trait that plugins implement to handle Raft-replicated write operations.
///
/// Each plugin registers a handler with a unique `plugin_id`. When a `PluginWrite`
/// entry is committed, the state machine looks up the handler by `plugin_id` and
/// calls `apply`. The core Raft layer does not interpret `op_type` or `payload` —
/// those are opaque to it.
pub trait RaftPluginHandler: Send + Sync + 'static {
    /// Unique identifier for this plugin (e.g., "consul", "etcd").
    fn plugin_id(&self) -> &str;

    /// Column family names this plugin requires in RocksDB.
    ///
    /// These are created when the plugin is registered with the state machine.
    /// The plugin is responsible for defining its own key/value schema within
    /// these column families.
    fn column_families(&self) -> Vec<String>;

    /// Column families this plugin wants streamed into Raft snapshots.
    ///
    /// Defaults to every CF the plugin owns, which is the safe choice: a CF
    /// left out of snapshots only survives on a lagging follower if the log
    /// entries that produced it are still there, and log purging after a
    /// snapshot removes exactly those entries. Override only when a CF is
    /// genuinely rebuildable from the log or is explicitly node-local.
    ///
    /// The core state machine never hardcodes plugin CF names — plugins
    /// declare their own, so adding a plugin cannot silently leave its data
    /// out of the snapshot path.
    fn snapshot_column_families(&self) -> Vec<String> {
        self.column_families()
    }

    /// Apply a committed write operation to the plugin's state.
    ///
    /// - `db`: shared RocksDB instance (use `db.cf_handle(name)` to access CFs)
    /// - `op_type`: plugin-defined operation type (e.g., "kv_put", "session_create")
    /// - `payload`: serialized plugin-specific request data
    /// - `log_index`: Raft log index of this entry (useful for CreateIndex/ModifyIndex)
    fn apply(&self, db: &DB, op_type: &str, payload: &[u8], log_index: u64) -> RaftResponse;

    /// Build a snapshot of this plugin's data as a standalone byte buffer.
    ///
    /// Legacy path: plugin data now travels in the unified snapshot body via
    /// [`Self::snapshot_column_families`], which streams straight to disk
    /// instead of materializing a byte vector. The core state machine does not
    /// call this — it is kept so plugins can still exercise their own
    /// encode/decode round trip in tests.
    fn build_snapshot(&self, db: &DB) -> Result<Vec<u8>, String>;

    /// Restore plugin state from a snapshot produced by [`Self::build_snapshot`].
    ///
    /// See [`Self::build_snapshot`] — this is the legacy byte-buffer path and
    /// is not invoked during Raft snapshot installation.
    fn install_snapshot(&self, db: &DB, data: &[u8]) -> Result<(), String>;
}

/// Registry of plugin handlers, keyed by plugin_id.
///
/// Handlers live in a `BTreeMap` so iteration is ordered by plugin id: the
/// snapshot builder walks the registry, and every replica has to stream the
/// same column family sequence to produce identical snapshot bytes.
///
/// Thread-safe: handlers are stored behind `Arc` and the registry itself is
/// wrapped in `Arc` when shared across the state machine.
pub struct PluginRegistry {
    handlers: BTreeMap<String, Arc<dyn RaftPluginHandler>>,
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
        }
    }

    /// Register a plugin handler. Returns an error if a handler with the same
    /// plugin_id is already registered.
    pub fn register(&mut self, handler: Arc<dyn RaftPluginHandler>) -> Result<(), String> {
        let id = handler.plugin_id().to_string();
        if self.handlers.contains_key(&id) {
            return Err(format!("plugin '{}' is already registered", id));
        }
        tracing::info!(plugin_id = %id, "Registered Raft plugin handler");
        self.handlers.insert(id, handler);
        Ok(())
    }

    /// Look up a handler by plugin_id.
    pub fn get(&self, plugin_id: &str) -> Option<&Arc<dyn RaftPluginHandler>> {
        self.handlers.get(plugin_id)
    }

    /// Iterate over all registered handlers.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Arc<dyn RaftPluginHandler>)> {
        self.handlers.iter()
    }

    /// Collect all column families required by registered plugins.
    pub fn all_column_families(&self) -> Vec<String> {
        self.handlers
            .values()
            .flat_map(|h| h.column_families())
            .collect()
    }

    /// Collect the column families registered plugins want in snapshots.
    ///
    /// Ordered by plugin id (the registry is a `BTreeMap`), so every replica
    /// streams the same CF sequence and produces byte-identical snapshot
    /// bodies for the same state.
    pub fn all_snapshot_column_families(&self) -> Vec<String> {
        self.handlers
            .values()
            .flat_map(|h| h.snapshot_column_families())
            .collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
