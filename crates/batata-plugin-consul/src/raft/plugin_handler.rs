/// Consul plugin handler for the unified Raft state machine.
///
/// Implements `RaftPluginHandler` so Consul write operations (KV, Sessions)
/// can be replicated through the core Raft group instead of a separate
/// Consul-only Raft.
///
/// All Consul apply logic is implemented here — the
/// handler receives `ConsulRaftRequest` (deserialized from `PluginWrite.payload`)
/// and applies it to the shared RocksDB.
use std::hash::Hasher;
use std::sync::Arc;

use batata_consistency::RaftNode;
use batata_consistency::raft::plugin::RaftPluginHandler;
use batata_consistency::raft::request::{RaftRequest, RaftResponse};
use rocksdb::{DB, WriteBatch};
use tracing::{debug, error, info, warn};

use super::apply_hook::{SharedConsulApplyHook, new_shared_hook};
use super::request::{ConsulRaftRequest, ConsulRaftResponse};
use crate::constants::{
    CF_CONSUL_ACL, CF_CONSUL_CA_ROOTS, CF_CONSUL_CATALOG, CF_CONSUL_CONFIG_ENTRIES,
    CF_CONSUL_COORDINATES, CF_CONSUL_EVENTS, CF_CONSUL_HEALTH_CHECKS, CF_CONSUL_INTENTIONS,
    CF_CONSUL_KV, CF_CONSUL_NAMESPACES, CF_CONSUL_OPERATOR, CF_CONSUL_PEERING, CF_CONSUL_QUERIES,
    CF_CONSUL_SESSIONS,
};
use crate::index_provider::{ConsulTable, ConsulTableIndex};

/// Plugin identifier used in `PluginWrite { plugin_id, .. }`.
pub const CONSUL_PLUGIN_ID: &str = "consul";

/// Snapshot data: column family name -> list of (key, value) pairs
type SnapshotCfData = std::collections::HashMap<String, Vec<(Vec<u8>, Vec<u8>)>>;

/// Size of the checksum trailer appended to snapshot data (8 bytes for u64)
const SNAPSHOT_CHECKSUM_SIZE: usize = std::mem::size_of::<u64>();

/// Per-variant payload the apply-back hook needs after a successful apply.
/// Stored alongside the original request so the handler can invoke the
/// hook without re-deserializing or cloning the whole request enum.
enum HookDispatch {
    CatalogRegister {
        key: String,
        registration_json: String,
    },
    CatalogDeregister {
        key: String,
    },
    /// Any ACL mutation (token/policy/role/auth-method/binding-rule).
    /// The hook clears all ACL caches — no per-entity dispatch needed.
    AclChange,
}

pub struct ConsulRaftPluginHandler {
    /// Per-table index — updated on every apply to notify blocking queries.
    table_index: ConsulTableIndex,
    /// Apply-back hook slot. When registered, the handler invokes the hook
    /// after writing to RocksDB so follower nodes can refresh their
    /// in-memory caches (notably `ConsulNamingStore`). Shared `Arc<RwLock>`
    /// lets plugin construction install the hook after the handler is
    /// already embedded in the core Raft state machine.
    apply_hook: SharedConsulApplyHook,
}

impl ConsulRaftPluginHandler {
    pub fn new(table_index: ConsulTableIndex) -> Self {
        Self {
            table_index,
            apply_hook: new_shared_hook(),
        }
    }

    pub fn new_arc(table_index: ConsulTableIndex) -> Arc<Self> {
        Arc::new(Self::new(table_index))
    }

    /// Get a shared handle to the apply hook slot so callers can install
    /// their concrete hook implementation after construction.
    pub fn apply_hook(&self) -> SharedConsulApplyHook {
        self.apply_hook.clone()
    }
}

impl RaftPluginHandler for ConsulRaftPluginHandler {
    fn plugin_id(&self) -> &str {
        CONSUL_PLUGIN_ID
    }

    fn column_families(&self) -> Vec<String> {
        vec![
            CF_CONSUL_KV.to_string(),
            CF_CONSUL_SESSIONS.to_string(),
            CF_CONSUL_ACL.to_string(),
            CF_CONSUL_QUERIES.to_string(),
            CF_CONSUL_CONFIG_ENTRIES.to_string(),
            CF_CONSUL_CA_ROOTS.to_string(),
            CF_CONSUL_INTENTIONS.to_string(),
            CF_CONSUL_COORDINATES.to_string(),
            CF_CONSUL_PEERING.to_string(),
            CF_CONSUL_OPERATOR.to_string(),
            CF_CONSUL_EVENTS.to_string(),
            CF_CONSUL_NAMESPACES.to_string(),
            CF_CONSUL_CATALOG.to_string(),
            CF_CONSUL_HEALTH_CHECKS.to_string(),
        ]
    }

    fn apply(&self, db: &DB, op_type: &str, payload: &[u8], log_index: u64) -> RaftResponse {
        let request: ConsulRaftRequest = match serde_json::from_slice(payload) {
            Ok(r) => r,
            Err(e) => {
                error!(op_type = %op_type, error = %e, "Failed to deserialize ConsulRaftRequest");
                return RaftResponse::failure(format!("deserialize error: {}", e));
            }
        };

        // Snapshot request metadata the hook needs BEFORE the request is
        // moved into `apply_consul_request`. Variants that don't carry
        // hook-relevant data leave this as None.
        let hook_dispatch: Option<HookDispatch> = match &request {
            ConsulRaftRequest::CatalogRegister {
                key,
                registration_json,
            } => Some(HookDispatch::CatalogRegister {
                key: key.clone(),
                registration_json: registration_json.clone(),
            }),
            ConsulRaftRequest::CatalogDeregister { key } => {
                Some(HookDispatch::CatalogDeregister { key: key.clone() })
            }
            // All ACL mutations flow through a single hook dispatch —
            // the concrete hook impl invalidates every cached entry on
            // every ACL change, so per-variant discrimination would buy
            // nothing.
            ConsulRaftRequest::ACLTokenSet { .. }
            | ConsulRaftRequest::ACLTokenDelete { .. }
            | ConsulRaftRequest::ACLPolicySet { .. }
            | ConsulRaftRequest::ACLPolicyDelete { .. }
            | ConsulRaftRequest::ACLRoleSet { .. }
            | ConsulRaftRequest::ACLRoleDelete { .. }
            | ConsulRaftRequest::ACLAuthMethodSet { .. }
            | ConsulRaftRequest::ACLAuthMethodDelete { .. }
            | ConsulRaftRequest::ACLBindingRuleSet { .. }
            | ConsulRaftRequest::ACLBindingRuleDelete { .. }
            | ConsulRaftRequest::ACLBootstrap { .. } => Some(HookDispatch::AclChange),
            _ => None,
        };

        let consul_resp = apply_consul_request(db, &self.table_index, request, log_index);

        // Fire the apply-back hook only on success. Uses `try_read` to
        // avoid blocking the apply path when the hook slot is being
        // installed; a missed notification is preferable to a stall, and
        // cold-start recovery refreshes the view anyway.
        if consul_resp.success {
            if let Some(dispatch) = hook_dispatch {
                if let Ok(guard) = self.apply_hook.try_read() {
                    if let Some(hook) = guard.as_ref() {
                        match &dispatch {
                            HookDispatch::CatalogRegister {
                                key,
                                registration_json,
                            } => hook.on_catalog_register(key, registration_json),
                            HookDispatch::CatalogDeregister { key } => {
                                hook.on_catalog_deregister(key)
                            }
                            HookDispatch::AclChange => hook.on_acl_change(),
                        }
                    }
                }
            }
        }

        // Convert ConsulRaftResponse -> RaftResponse
        if consul_resp.success {
            RaftResponse::success()
        } else {
            RaftResponse::failure(consul_resp.message.unwrap_or_default())
        }
    }

    fn build_snapshot(&self, db: &DB) -> Result<Vec<u8>, String> {
        let mut snapshot_data: SnapshotCfData = std::collections::HashMap::new();

        for cf_name in self.column_families() {
            let cf_name = cf_name.as_str();
            if let Some(cf) = db.cf_handle(cf_name) {
                let mut cf_data = Vec::new();
                let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
                for (key, value) in iter.flatten() {
                    cf_data.push((key.to_vec(), value.to_vec()));
                }
                snapshot_data.insert(cf_name.to_string(), cf_data);
            }
        }

        let payload = serde_json::to_vec(&snapshot_data)
            .map_err(|e| format!("snapshot serialize error: {}", e))?;

        let checksum = compute_checksum(&payload);
        let mut data = payload;
        data.extend_from_slice(&checksum.to_le_bytes());

        info!("Built Consul plugin snapshot with {} bytes", data.len());
        Ok(data)
    }

    fn install_snapshot(&self, db: &DB, data: &[u8]) -> Result<(), String> {
        if data.is_empty() {
            return Ok(());
        }

        // Validate checksum
        let payload = if data.len() > SNAPSHOT_CHECKSUM_SIZE {
            let (payload, checksum_bytes) = data.split_at(data.len() - SNAPSHOT_CHECKSUM_SIZE);
            let expected = u64::from_le_bytes(
                checksum_bytes
                    .try_into()
                    .map_err(|_| "Invalid checksum trailer size".to_string())?,
            );
            let actual = compute_checksum(payload);
            if actual != expected {
                return Err(format!(
                    "Consul snapshot checksum mismatch: expected {:#x}, got {:#x}",
                    expected, actual
                ));
            }
            info!("Consul plugin snapshot checksum verified ({:#x})", actual);
            payload
        } else {
            warn!("Consul plugin snapshot has no checksum trailer, skipping integrity check");
            data
        };

        let snapshot_data: SnapshotCfData =
            serde_json::from_slice(payload).map_err(|e| format!("snapshot deserialize: {}", e))?;

        for (cf_name, cf_data) in snapshot_data {
            if let Some(cf) = db.cf_handle(&cf_name) {
                let mut batch = WriteBatch::default();

                // Clear existing data
                let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
                for (key, _) in iter.flatten() {
                    batch.delete_cf(cf, &key);
                }

                // Restore snapshot data
                for (key, value) in cf_data {
                    batch.put_cf(cf, &key, &value);
                }

                db.write(batch)
                    .map_err(|e| format!("snapshot restore CF {}: {}", cf_name, e))?;
            }
        }

        info!("Consul plugin snapshot installed");
        Ok(())
    }
}

// ============================================================================
// Apply logic for Consul Raft operations
// ============================================================================

fn apply_consul_request(
    db: &DB,
    table_index: &ConsulTableIndex,
    request: ConsulRaftRequest,
    log_index: u64,
) -> ConsulRaftResponse {
    let table =
        match &request {
            // KV
            ConsulRaftRequest::KVPut { .. }
            | ConsulRaftRequest::KVDelete { .. }
            | ConsulRaftRequest::KVDeletePrefix { .. }
            | ConsulRaftRequest::KVAcquireSession { .. }
            | ConsulRaftRequest::KVReleaseSessionKey { .. }
            | ConsulRaftRequest::KVReleaseSession { .. }
            | ConsulRaftRequest::KVCas { .. }
            | ConsulRaftRequest::KVTransaction { .. } => ConsulTable::KVS,
            // Session
            ConsulRaftRequest::SessionCreate { .. }
            | ConsulRaftRequest::SessionDestroy { .. }
            | ConsulRaftRequest::SessionRenew { .. }
            | ConsulRaftRequest::SessionCleanupExpired { .. } => ConsulTable::Sessions,
            // ACL
            ConsulRaftRequest::ACLTokenSet { .. }
            | ConsulRaftRequest::ACLTokenDelete { .. }
            | ConsulRaftRequest::ACLPolicySet { .. }
            | ConsulRaftRequest::ACLPolicyDelete { .. }
            | ConsulRaftRequest::ACLRoleSet { .. }
            | ConsulRaftRequest::ACLRoleDelete { .. }
            | ConsulRaftRequest::ACLAuthMethodSet { .. }
            | ConsulRaftRequest::ACLAuthMethodDelete { .. }
            | ConsulRaftRequest::ACLBootstrap { .. }
            | ConsulRaftRequest::ACLBindingRuleSet { .. }
            | ConsulRaftRequest::ACLBindingRuleDelete { .. } => ConsulTable::ACL,
            // Query
            ConsulRaftRequest::QueryCreate { .. }
            | ConsulRaftRequest::QueryUpdate { .. }
            | ConsulRaftRequest::QueryDelete { .. } => ConsulTable::Queries,
            // ConfigEntry
            ConsulRaftRequest::ConfigEntryApply { .. }
            | ConsulRaftRequest::ConfigEntryDelete { .. } => ConsulTable::ConfigEntries,
            // ConnectCA
            ConsulRaftRequest::CARootSet { .. }
            | ConsulRaftRequest::CAConfigUpdate { .. }
            | ConsulRaftRequest::IntentionUpsert { .. }
            | ConsulRaftRequest::IntentionDelete { .. }
            | ConsulRaftRequest::IntentionUpsertExact { .. } => ConsulTable::ConnectCA,
            // Coordinate
            ConsulRaftRequest::CoordinateBatchUpdate { .. } => ConsulTable::Coordinates,
            // Peering
            ConsulRaftRequest::PeeringWrite { .. } | ConsulRaftRequest::PeeringDelete { .. } => {
                ConsulTable::Peering
            }
            // Operator
            ConsulRaftRequest::OperatorRemovePeer { .. }
            | ConsulRaftRequest::OperatorAutopilotUpdate { .. } => ConsulTable::Operator,
            // Namespace
            ConsulRaftRequest::NamespaceUpsert { .. }
            | ConsulRaftRequest::NamespaceDelete { .. } => ConsulTable::Namespaces,
            // Catalog
            ConsulRaftRequest::CatalogRegister { .. }
            | ConsulRaftRequest::CatalogDeregister { .. } => ConsulTable::Catalog,
            // HealthCheck
            ConsulRaftRequest::HealthCheckRegister { .. }
            | ConsulRaftRequest::HealthCheckDeregister { .. } => ConsulTable::Catalog,
            // Internal
            ConsulRaftRequest::Noop => ConsulTable::KVS,
        };

    let resp = match request {
        ConsulRaftRequest::KVPut {
            key,
            stored_kv_json,
            session_index_key,
        } => apply_kv_put(
            db,
            &key,
            &stored_kv_json,
            session_index_key.as_deref(),
            log_index,
        ),
        ConsulRaftRequest::KVDelete {
            key,
            session_index_cleanup,
        } => apply_kv_delete(db, &key, session_index_cleanup.as_deref(), log_index),
        ConsulRaftRequest::KVDeletePrefix { prefix } => {
            apply_kv_delete_prefix(db, &prefix, log_index)
        }
        ConsulRaftRequest::KVAcquireSession {
            key,
            session_id,
            stored_kv_json,
        } => apply_kv_acquire_session(db, &key, &session_id, &stored_kv_json, log_index),
        ConsulRaftRequest::KVReleaseSessionKey {
            key,
            session_id,
            stored_kv_json,
        } => apply_kv_release_session_key(db, &key, &session_id, &stored_kv_json, log_index),
        ConsulRaftRequest::KVReleaseSession {
            session_id,
            updates,
            index_keys_to_delete,
        } => apply_kv_release_session(db, &session_id, updates, index_keys_to_delete, log_index),
        ConsulRaftRequest::KVCas {
            key,
            stored_kv_json,
            expected_modify_index,
        } => apply_kv_cas(db, &key, &stored_kv_json, expected_modify_index, log_index),
        ConsulRaftRequest::KVTransaction {
            puts,
            deletes,
            session_index_puts,
            session_index_deletes,
        } => apply_kv_transaction(
            db,
            puts,
            deletes,
            session_index_puts,
            session_index_deletes,
            log_index,
        ),
        ConsulRaftRequest::SessionCreate {
            session_id,
            stored_session_json,
        } => apply_session_create(db, &session_id, &stored_session_json, log_index),
        ConsulRaftRequest::SessionDestroy { session_id } => {
            apply_session_destroy(db, &session_id, log_index)
        }
        ConsulRaftRequest::SessionRenew {
            session_id,
            stored_session_json,
        } => apply_session_renew(db, &session_id, &stored_session_json, log_index),
        ConsulRaftRequest::SessionCleanupExpired {
            expired_session_ids,
        } => apply_session_cleanup_expired(db, expired_session_ids, log_index),

        // === ACL: write to CF_CONSUL_ACL ===
        ConsulRaftRequest::ACLTokenSet {
            accessor_id,
            token_json,
        } => apply_generic_put(
            db,
            CF_CONSUL_ACL,
            &format!("token::{}", accessor_id),
            &token_json,
            log_index,
        ),
        ConsulRaftRequest::ACLBootstrap { token_json } => {
            // Bootstrap token: accessor_id is embedded in the JSON
            apply_generic_put(
                db,
                CF_CONSUL_ACL,
                "token::bootstrap",
                &token_json,
                log_index,
            )
        }
        ConsulRaftRequest::ACLTokenDelete { accessor_id } => apply_generic_delete(
            db,
            CF_CONSUL_ACL,
            &format!("token::{}", accessor_id),
            log_index,
        ),
        ConsulRaftRequest::ACLPolicySet { id, policy_json } => apply_generic_put(
            db,
            CF_CONSUL_ACL,
            &format!("policy::{}", id),
            &policy_json,
            log_index,
        ),
        ConsulRaftRequest::ACLPolicyDelete { id } => {
            apply_generic_delete(db, CF_CONSUL_ACL, &format!("policy::{}", id), log_index)
        }
        ConsulRaftRequest::ACLRoleSet { id, role_json } => apply_generic_put(
            db,
            CF_CONSUL_ACL,
            &format!("role::{}", id),
            &role_json,
            log_index,
        ),
        ConsulRaftRequest::ACLRoleDelete { id } => {
            apply_generic_delete(db, CF_CONSUL_ACL, &format!("role::{}", id), log_index)
        }
        ConsulRaftRequest::ACLAuthMethodSet { name, method_json } => apply_generic_put(
            db,
            CF_CONSUL_ACL,
            &format!("auth_method::{}", name),
            &method_json,
            log_index,
        ),
        ConsulRaftRequest::ACLAuthMethodDelete { name } => apply_generic_delete(
            db,
            CF_CONSUL_ACL,
            &format!("auth_method::{}", name),
            log_index,
        ),
        ConsulRaftRequest::ACLBindingRuleSet { id, rule_json } => apply_generic_put(
            db,
            CF_CONSUL_ACL,
            &format!("binding_rule::{}", id),
            &rule_json,
            log_index,
        ),
        ConsulRaftRequest::ACLBindingRuleDelete { id } => apply_generic_delete(
            db,
            CF_CONSUL_ACL,
            &format!("binding_rule::{}", id),
            log_index,
        ),

        // === Query: write to CF_CONSUL_QUERIES ===
        ConsulRaftRequest::QueryCreate { id, query_json }
        | ConsulRaftRequest::QueryUpdate { id, query_json } => {
            apply_generic_put(db, CF_CONSUL_QUERIES, &id, &query_json, log_index)
        }
        ConsulRaftRequest::QueryDelete { id } => {
            apply_generic_delete(db, CF_CONSUL_QUERIES, &id, log_index)
        }

        // === ConfigEntry: write to CF_CONSUL_CONFIG_ENTRIES ===
        ConsulRaftRequest::ConfigEntryApply { key, entry_json } => {
            apply_generic_put(db, CF_CONSUL_CONFIG_ENTRIES, &key, &entry_json, log_index)
        }
        ConsulRaftRequest::ConfigEntryDelete { key } => {
            apply_generic_delete(db, CF_CONSUL_CONFIG_ENTRIES, &key, log_index)
        }

        // === ConnectCA: write to CF_CONSUL_CA_ROOTS / CF_CONSUL_INTENTIONS ===
        ConsulRaftRequest::CARootSet { id, root_json } => {
            apply_generic_put(db, CF_CONSUL_CA_ROOTS, &id, &root_json, log_index)
        }
        ConsulRaftRequest::CAConfigUpdate { config_json } => apply_generic_put(
            db,
            CF_CONSUL_CA_ROOTS,
            "__ca_config__",
            &config_json,
            log_index,
        ),
        ConsulRaftRequest::IntentionUpsert { id, intention_json } => {
            apply_generic_put(db, CF_CONSUL_INTENTIONS, &id, &intention_json, log_index)
        }
        ConsulRaftRequest::IntentionDelete { id } => {
            apply_generic_delete(db, CF_CONSUL_INTENTIONS, &id, log_index)
        }
        ConsulRaftRequest::IntentionUpsertExact {
            source,
            destination,
            intention_json,
        } => {
            let key = format!("{}:{}", source, destination);
            apply_generic_put(db, CF_CONSUL_INTENTIONS, &key, &intention_json, log_index)
        }

        // === Coordinate: write to CF_CONSUL_COORDINATES ===
        ConsulRaftRequest::CoordinateBatchUpdate { key, entry_json } => {
            apply_generic_put(db, CF_CONSUL_COORDINATES, &key, &entry_json, log_index)
        }

        // === Peering: write to CF_CONSUL_PEERING ===
        ConsulRaftRequest::PeeringWrite { name, peering_json } => {
            apply_generic_put(db, CF_CONSUL_PEERING, &name, &peering_json, log_index)
        }
        ConsulRaftRequest::PeeringDelete { name } => {
            apply_generic_delete(db, CF_CONSUL_PEERING, &name, log_index)
        }

        // === Operator: write to CF_CONSUL_OPERATOR ===
        ConsulRaftRequest::OperatorRemovePeer { server_key } => apply_generic_delete(
            db,
            CF_CONSUL_OPERATOR,
            &format!("server:{}", server_key),
            log_index,
        ),
        ConsulRaftRequest::OperatorAutopilotUpdate { config_json } => apply_generic_put(
            db,
            CF_CONSUL_OPERATOR,
            "autopilot_config",
            &config_json,
            log_index,
        ),

        // === Namespace: write to CF_CONSUL_NAMESPACES ===
        ConsulRaftRequest::NamespaceUpsert {
            name,
            namespace_json,
        } => apply_generic_put(db, CF_CONSUL_NAMESPACES, &name, &namespace_json, log_index),
        ConsulRaftRequest::NamespaceDelete { name } => {
            apply_generic_delete(db, CF_CONSUL_NAMESPACES, &name, log_index)
        }

        // === Catalog: write to CF_CONSUL_CATALOG ===
        ConsulRaftRequest::CatalogRegister {
            key,
            registration_json,
        } => apply_generic_put(db, CF_CONSUL_CATALOG, &key, &registration_json, log_index),
        ConsulRaftRequest::CatalogDeregister { key } => {
            apply_generic_delete(db, CF_CONSUL_CATALOG, &key, log_index)
        }

        // === HealthCheck: write to CF_CONSUL_HEALTH_CHECKS ===
        ConsulRaftRequest::HealthCheckRegister {
            check_id,
            config_json,
        } => apply_generic_put(
            db,
            CF_CONSUL_HEALTH_CHECKS,
            &check_id,
            &config_json,
            log_index,
        ),
        ConsulRaftRequest::HealthCheckDeregister { check_id } => {
            apply_generic_delete(db, CF_CONSUL_HEALTH_CHECKS, &check_id, log_index)
        }

        ConsulRaftRequest::Noop => ConsulRaftResponse::success(),
    };

    // Update table index AFTER apply
    table_index.update(table, log_index);

    resp
}

// ============================================================================
// KV apply methods
// ============================================================================

fn cf_kv(db: &DB) -> &rocksdb::ColumnFamily {
    db.cf_handle(CF_CONSUL_KV).expect("CF consul_kv must exist")
}

fn cf_sessions(db: &DB) -> &rocksdb::ColumnFamily {
    db.cf_handle(CF_CONSUL_SESSIONS)
        .expect("CF consul_sessions must exist")
}

fn apply_kv_put(
    db: &DB,
    key: &str,
    stored_kv_json: &str,
    session_index_key: Option<&str>,
    log_index: u64,
) -> ConsulRaftResponse {
    let json = rewrite_kv_indexes(db, key, stored_kv_json, log_index);

    if let Some(idx_key) = session_index_key {
        let mut batch = WriteBatch::default();
        batch.put_cf(cf_kv(db), key.as_bytes(), json.as_bytes());
        batch.put_cf(cf_sessions(db), idx_key.as_bytes(), b"");
        write_batch(db, batch, "KV put")
    } else {
        match db.put_cf(cf_kv(db), key.as_bytes(), json.as_bytes()) {
            Ok(_) => ConsulRaftResponse::success(),
            Err(e) => ConsulRaftResponse::failure(format!("KV put failed: {}", e)),
        }
    }
}

fn apply_kv_delete(
    db: &DB,
    key: &str,
    session_index_cleanup: Option<&str>,
    _log_index: u64,
) -> ConsulRaftResponse {
    if let Some(idx_key) = session_index_cleanup {
        let mut batch = WriteBatch::default();
        batch.delete_cf(cf_kv(db), key.as_bytes());
        batch.delete_cf(cf_sessions(db), idx_key.as_bytes());
        write_batch(db, batch, "KV delete")
    } else {
        match db.delete_cf(cf_kv(db), key.as_bytes()) {
            Ok(_) => ConsulRaftResponse::success(),
            Err(e) => ConsulRaftResponse::failure(format!("KV delete failed: {}", e)),
        }
    }
}

fn apply_kv_delete_prefix(db: &DB, prefix: &str, _log_index: u64) -> ConsulRaftResponse {
    let cf_k = cf_kv(db);
    let cf_s = cf_sessions(db);

    let mode = if prefix.is_empty() {
        rocksdb::IteratorMode::Start
    } else {
        rocksdb::IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward)
    };

    let mut batch = WriteBatch::default();
    let mut count = 0u32;

    for item in db.iterator_cf(cf_k, mode).flatten() {
        let (key_bytes, value_bytes) = item;
        let key = match String::from_utf8(key_bytes.to_vec()) {
            Ok(k) => k,
            Err(_) => continue,
        };
        if !prefix.is_empty() && !key.starts_with(prefix) {
            break;
        }

        batch.delete_cf(cf_k, key.as_bytes());

        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&value_bytes)
            && let Some(pair) = val.get("pair")
            && let Some(session_id) = pair.get("Session").and_then(|s| s.as_str())
        {
            let idx_key = format!("kidx:{}:{}", session_id, key);
            batch.delete_cf(cf_s, idx_key.as_bytes());
        }

        count += 1;
    }

    if count > 0 {
        write_batch(db, batch, "KV delete prefix")
    } else {
        ConsulRaftResponse::success()
    }
}

fn apply_kv_acquire_session(
    db: &DB,
    key: &str,
    session_id: &str,
    stored_kv_json: &str,
    log_index: u64,
) -> ConsulRaftResponse {
    let json = rewrite_kv_indexes(db, key, stored_kv_json, log_index);

    let mut batch = WriteBatch::default();
    batch.put_cf(cf_kv(db), key.as_bytes(), json.as_bytes());
    let idx_key = format!("kidx:{}:{}", session_id, key);
    batch.put_cf(cf_sessions(db), idx_key.as_bytes(), b"");
    write_batch(db, batch, "KV acquire session")
}

fn apply_kv_release_session_key(
    db: &DB,
    key: &str,
    session_id: &str,
    stored_kv_json: &str,
    log_index: u64,
) -> ConsulRaftResponse {
    let json = rewrite_kv_indexes(db, key, stored_kv_json, log_index);

    let mut batch = WriteBatch::default();
    batch.put_cf(cf_kv(db), key.as_bytes(), json.as_bytes());
    let idx_key = format!("kidx:{}:{}", session_id, key);
    batch.delete_cf(cf_sessions(db), idx_key.as_bytes());
    write_batch(db, batch, "KV release session key")
}

fn apply_kv_release_session(
    db: &DB,
    _session_id: &str,
    updates: Vec<(String, String)>,
    index_keys_to_delete: Vec<String>,
    log_index: u64,
) -> ConsulRaftResponse {
    let mut batch = WriteBatch::default();

    for (kv_key, stored_kv_json) in &updates {
        let json = rewrite_kv_indexes(db, kv_key, stored_kv_json, log_index);
        batch.put_cf(cf_kv(db), kv_key.as_bytes(), json.as_bytes());
    }

    for idx_key in &index_keys_to_delete {
        batch.delete_cf(cf_sessions(db), idx_key.as_bytes());
    }

    write_batch(db, batch, "KV release session")
}

fn apply_kv_cas(
    db: &DB,
    key: &str,
    stored_kv_json: &str,
    expected_modify_index: u64,
    log_index: u64,
) -> ConsulRaftResponse {
    let cf_k = cf_kv(db);

    if let Ok(Some(existing_bytes)) = db.get_cf(cf_k, key.as_bytes()) {
        if let Ok(existing) = serde_json::from_slice::<serde_json::Value>(&existing_bytes) {
            let current_index = existing
                .get("pair")
                .and_then(|p| p.get("ModifyIndex"))
                .and_then(|i| i.as_u64())
                .unwrap_or(0);

            if current_index != expected_modify_index {
                return ConsulRaftResponse::failure("CAS failed: index mismatch".into());
            }
        }
    } else if expected_modify_index != 0 {
        return ConsulRaftResponse::failure("CAS failed: key not found".into());
    }

    let json = rewrite_kv_indexes(db, key, stored_kv_json, log_index);
    match db.put_cf(cf_k, key.as_bytes(), json.as_bytes()) {
        Ok(_) => ConsulRaftResponse::success(),
        Err(e) => ConsulRaftResponse::failure(format!("CAS write failed: {}", e)),
    }
}

fn apply_kv_transaction(
    db: &DB,
    puts: Vec<(String, String)>,
    deletes: Vec<String>,
    session_index_puts: Vec<String>,
    session_index_deletes: Vec<String>,
    log_index: u64,
) -> ConsulRaftResponse {
    let cf_k = cf_kv(db);
    let cf_s = cf_sessions(db);
    let mut batch = WriteBatch::default();

    for (key, stored_kv_json) in &puts {
        let json = rewrite_kv_indexes(db, key, stored_kv_json, log_index);
        batch.put_cf(cf_k, key.as_bytes(), json.as_bytes());
    }

    for key in &deletes {
        batch.delete_cf(cf_k, key.as_bytes());
    }

    for idx_key in &session_index_puts {
        batch.put_cf(cf_s, idx_key.as_bytes(), b"");
    }

    for idx_key in &session_index_deletes {
        batch.delete_cf(cf_s, idx_key.as_bytes());
    }

    write_batch(db, batch, "KV transaction")
}

// ============================================================================
// Session apply methods
// ============================================================================

fn apply_session_create(
    db: &DB,
    session_id: &str,
    stored_session_json: &str,
    log_index: u64,
) -> ConsulRaftResponse {
    let json = rewrite_json_index(stored_session_json, log_index, true);
    match db.put_cf(cf_sessions(db), session_id.as_bytes(), json.as_bytes()) {
        Ok(_) => ConsulRaftResponse::success(),
        Err(e) => ConsulRaftResponse::failure(format!("Session create failed: {}", e)),
    }
}

fn apply_session_destroy(db: &DB, session_id: &str, _log_index: u64) -> ConsulRaftResponse {
    match db.delete_cf(cf_sessions(db), session_id.as_bytes()) {
        Ok(_) => ConsulRaftResponse::success(),
        Err(e) => ConsulRaftResponse::failure(format!("Session destroy failed: {}", e)),
    }
}

fn apply_session_renew(
    db: &DB,
    session_id: &str,
    stored_session_json: &str,
    log_index: u64,
) -> ConsulRaftResponse {
    let json = rewrite_json_index(stored_session_json, log_index, false);
    match db.put_cf(cf_sessions(db), session_id.as_bytes(), json.as_bytes()) {
        Ok(_) => ConsulRaftResponse::success(),
        Err(e) => ConsulRaftResponse::failure(format!("Session renew failed: {}", e)),
    }
}

fn apply_session_cleanup_expired(
    db: &DB,
    expired_session_ids: Vec<String>,
    _log_index: u64,
) -> ConsulRaftResponse {
    let cf = cf_sessions(db);
    let mut batch = WriteBatch::default();
    for id in &expired_session_ids {
        batch.delete_cf(cf, id.as_bytes());
    }
    write_batch(db, batch, "Session cleanup expired")
}

// ============================================================================
// Helpers
// ============================================================================

/// Rewrite CreateIndex/ModifyIndex on a StoredKV JSON entry.
fn rewrite_kv_indexes(db: &DB, key: &str, stored_kv_json: &str, log_index: u64) -> String {
    let mut val: serde_json::Value = match serde_json::from_str(stored_kv_json) {
        Ok(v) => v,
        Err(_) => return stored_kv_json.to_string(),
    };

    if let Some(pair) = val.get_mut("pair") {
        pair["ModifyIndex"] = serde_json::json!(log_index);

        let existing_create_index = get_existing_create_index(db, key);
        if let Some(ci) = existing_create_index {
            pair["CreateIndex"] = serde_json::json!(ci);
        } else {
            pair["CreateIndex"] = serde_json::json!(log_index);
        }
    }

    serde_json::to_string(&val).unwrap_or_else(|_| stored_kv_json.to_string())
}

fn get_existing_create_index(db: &DB, key: &str) -> Option<u64> {
    let cf = cf_kv(db);
    let bytes = db.get_cf(cf, key.as_bytes()).ok()??;
    let val: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    val.get("pair")?.get("CreateIndex")?.as_u64()
}

/// Rewrite ModifyIndex (and optionally CreateIndex) on a generic JSON entry.
fn rewrite_json_index(json_str: &str, log_index: u64, set_create_index: bool) -> String {
    if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(json_str) {
        val["ModifyIndex"] = serde_json::json!(log_index);
        if set_create_index {
            val["CreateIndex"] = serde_json::json!(log_index);
        }
        serde_json::to_string(&val).unwrap_or_else(|_| json_str.to_string())
    } else {
        json_str.to_string()
    }
}

fn write_batch(db: &DB, batch: WriteBatch, op: &str) -> ConsulRaftResponse {
    match db.write(batch) {
        Ok(_) => {
            debug!("Consul {}", op);
            ConsulRaftResponse::success()
        }
        Err(e) => {
            error!("Consul {} failed: {}", op, e);
            ConsulRaftResponse::failure(format!("{} failed: {}", op, e))
        }
    }
}

// ============================================================================
// Generic apply helpers for non-KV/Session column families
// ============================================================================

/// Write a JSON value to a column family, rewriting ModifyIndex = log_index.
fn apply_generic_put(
    db: &DB,
    cf_name: &str,
    key: &str,
    json_str: &str,
    log_index: u64,
) -> ConsulRaftResponse {
    let json = rewrite_json_index(json_str, log_index, false);
    let Some(cf) = db.cf_handle(cf_name) else {
        return ConsulRaftResponse::failure(format!("CF '{}' not found", cf_name));
    };
    match db.put_cf(cf, key.as_bytes(), json.as_bytes()) {
        Ok(_) => {
            debug!("Consul put {}:{}", cf_name, key);
            ConsulRaftResponse::success()
        }
        Err(e) => ConsulRaftResponse::failure(format!("put {}:{} failed: {}", cf_name, key, e)),
    }
}

/// Delete a key from a column family.
fn apply_generic_delete(db: &DB, cf_name: &str, key: &str, _log_index: u64) -> ConsulRaftResponse {
    let Some(cf) = db.cf_handle(cf_name) else {
        return ConsulRaftResponse::failure(format!("CF '{}' not found", cf_name));
    };
    match db.delete_cf(cf, key.as_bytes()) {
        Ok(_) => {
            debug!("Consul delete {}:{}", cf_name, key);
            ConsulRaftResponse::success()
        }
        Err(e) => ConsulRaftResponse::failure(format!("delete {}:{} failed: {}", cf_name, key, e)),
    }
}

fn compute_checksum(data: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(data);
    hasher.finish()
}

// ============================================================================
// ConsulRaftWriter — adapter from core RaftNode to Consul write interface
// ============================================================================

/// Thin adapter that wraps the core `RaftNode` and exposes the same
/// `write()` / `write_with_index()` interface for Consul write operations.
///
/// Internally converts `ConsulRaftRequest` → `RaftRequest::PluginWrite` and
/// sends it through the unified Raft group.
#[derive(Clone)]
pub struct ConsulRaftWriter {
    core_raft: Arc<RaftNode>,
}

impl ConsulRaftWriter {
    pub fn new(core_raft: Arc<RaftNode>) -> Self {
        Self { core_raft }
    }

    /// Write a Consul request through the core Raft and return the response
    /// along with the Raft log index.
    pub async fn write_with_index(
        &self,
        request: ConsulRaftRequest,
    ) -> Result<(ConsulRaftResponse, u64), Box<dyn std::error::Error + Send + Sync>> {
        let op_type = request.op_type().to_string();
        let payload = serde_json::to_vec(&request)?;

        let raft_request = RaftRequest::PluginWrite {
            plugin_id: CONSUL_PLUGIN_ID.to_string(),
            op_type,
            payload,
        };

        let (resp, log_index) = self.core_raft.write_with_index(raft_request).await?;

        let consul_resp = if resp.success {
            ConsulRaftResponse::success()
        } else {
            ConsulRaftResponse::failure(resp.message.unwrap_or_default())
        };

        Ok((consul_resp, log_index))
    }

    /// Write without returning the log index (convenience method).
    pub async fn write(
        &self,
        request: ConsulRaftRequest,
    ) -> Result<ConsulRaftResponse, Box<dyn std::error::Error + Send + Sync>> {
        let (resp, _) = self.write_with_index(request).await?;
        Ok(resp)
    }

    /// Check whether this node is currently the Raft leader.
    ///
    /// Callers use this to gate leader-only background tasks (e.g. session
    /// TTL expiration sweep) so that exactly one node decides when a
    /// committed state change should happen. The actual mutation still
    /// flows through `write`, which replicates the decision to followers.
    pub fn is_leader(&self) -> bool {
        self.core_raft.is_leader()
    }

    /// Satisfy a linearizable read — wait until this node has applied all
    /// entries committed at the time of the call.
    ///
    /// Used by Consul `?consistent` query mode. Only succeeds on the leader;
    /// followers get a `ForwardToLeader` error because openraft's
    /// `ensure_linearizable` can only be invoked on the leader. Callers
    /// should detect the error and either forward the HTTP request or
    /// emit an HTTP 307 redirect to the leader's address.
    pub async fn linearizable_read(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.core_raft.linearizable_read().await
    }

    /// Get the current Raft leader's address, if known.
    /// Used by consistent-read redirect path.
    pub fn leader_addr(&self) -> Option<String> {
        self.core_raft.leader_addr()
    }
}
