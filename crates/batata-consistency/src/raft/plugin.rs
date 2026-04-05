// Plugin handler trait for extending Raft state machine with plugin-specific operations.
//
// Plugins register handlers that receive PluginWrite requests after Raft consensus.
// The core state machine routes requests by plugin_id — it never interprets the payload.

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

    /// Apply a committed write operation to the plugin's state.
    ///
    /// - `db`: shared RocksDB instance (use `db.cf_handle(name)` to access CFs)
    /// - `op_type`: plugin-defined operation type (e.g., "kv_put", "session_create")
    /// - `payload`: serialized plugin-specific request data
    /// - `log_index`: Raft log index of this entry (useful for CreateIndex/ModifyIndex)
    fn apply(&self, db: &DB, op_type: &str, payload: &[u8], log_index: u64) -> RaftResponse;

    /// Build a snapshot of this plugin's data.
    ///
    /// Called during Raft snapshot creation. The returned bytes are included in
    /// the snapshot alongside core state machine data.
    fn build_snapshot(&self, db: &DB) -> Result<Vec<u8>, String>;

    /// Restore plugin state from a snapshot.
    ///
    /// Called during Raft snapshot installation. The plugin should replace its
    /// current state with the data from the snapshot.
    fn install_snapshot(&self, db: &DB, data: &[u8]) -> Result<(), String>;
}

/// Registry of plugin handlers, keyed by plugin_id.
///
/// Thread-safe: handlers are stored behind `Arc` and the registry itself is
/// wrapped in `Arc` when shared across the state machine.
pub struct PluginRegistry {
    handlers: std::collections::HashMap<String, Arc<dyn RaftPluginHandler>>,
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            handlers: std::collections::HashMap::new(),
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
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
