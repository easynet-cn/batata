//! RocksDB reader for embedded storage backends.
//!
//! Provides read-only query operations on the state machine's RocksDB.

use crate::bincode;
use std::sync::Arc;

use super::history::{HistoryTopK, bounded_history_cap, history_id_from_key};
use batata_common::model::clamp_page;
use rocksdb::DB;

/// Hard cap on the number of configs a single export may materialize.
/// Kept in sync with the SQL backend's `MAX_EXPORT_CONFIGS` (10_000).
const MAX_EXPORT_CONFIGS: usize = 10_000;

#[cfg(test)]
mod test_instrumentation {
    use std::cell::Cell;

    thread_local! {
        static HISTORY_DECODES: Cell<usize> = const { Cell::new(0) };
        static AUTH_MATERIALIZATIONS: Cell<usize> = const { Cell::new(0) };
        static HISTORY_RETAINED_KEYS: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn note_decode() {
        HISTORY_DECODES.with(|c| c.set(c.get().saturating_add(1)));
    }

    pub(crate) fn decode_count() -> usize {
        HISTORY_DECODES.with(|c| c.get())
    }

    /// Count records converted to `serde_json::Value` by the auth paging path.
    pub(crate) fn note_auth_materialization() {
        AUTH_MATERIALIZATIONS.with(|c| c.set(c.get().saturating_add(1)));
    }

    pub(crate) fn auth_materialization_count() -> usize {
        AUTH_MATERIALIZATIONS.with(|c| c.get())
    }

    /// Record how many compact keys a bounded history page retained.
    pub(crate) fn note_history_retained_keys(count: usize) {
        HISTORY_RETAINED_KEYS.with(|c| c.set(count));
    }

    pub(crate) fn history_retained_keys() -> usize {
        HISTORY_RETAINED_KEYS.with(|c| c.get())
    }

    pub(crate) fn reset() {
        HISTORY_DECODES.with(|c| c.set(0));
        AUTH_MATERIALIZATIONS.with(|c| c.set(0));
        HISTORY_RETAINED_KEYS.with(|c| c.set(0));
    }
}

/// Decode one stored history record. Test builds count every deserialization
/// so the bounded-scan guards can prove they actually avoid the full decode.
fn decode_history(bytes: &[u8]) -> anyhow::Result<StoredConfigHistory> {
    #[cfg(test)]
    test_instrumentation::note_decode();
    bincode::deserialize(bytes)
        .map_err(|e| anyhow::anyhow!("StoredConfigHistory decode error: {}", e))
}

use super::state_machine::{
    CF_CONFIG, CF_CONFIG_GRAY, CF_CONFIG_HISTORY, CF_INSTANCES, CF_NAMESPACE, CF_PERMISSIONS,
    CF_ROLES, CF_USERS, RocksStateMachine, StoredConfig, StoredConfigGray, StoredConfigHistory,
    StoredInstance, StoredNamespace, StoredPermission, StoredRole, StoredUser,
};

/// Read-only interface for querying data stored in RocksDB
///
/// This struct provides read operations on the state machine's RocksDB,
/// used by both `EmbeddedPersistService` (standalone) and
/// `DistributedPersistService` (Raft cluster) backends.
pub struct RocksDbReader {
    db: Arc<DB>,
}

impl RocksDbReader {
    /// Create a new reader wrapping the given RocksDB instance
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    /// Create a reader from a RocksStateMachine
    pub fn from_state_machine(sm: &RocksStateMachine) -> Self {
        Self::new(sm.db())
    }

    /// Get a reference to the underlying RocksDB instance.
    ///
    /// This is useful for operations that need direct DB access,
    /// such as capacity metadata stored in the default column family.
    pub fn db(&self) -> &Arc<DB> {
        &self.db
    }

    // ==================== Config Operations ====================

    /// Get a single config by data_id, group, and tenant
    pub fn get_config(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let key = RocksStateMachine::config_key(data_id, group, tenant);
        let cf = self.cf_handle(CF_CONFIG)?;
        match self
            .db
            .get_cf(cf, key.as_bytes())
            .map_err(|e| anyhow::anyhow!("RocksDB get error: {}", e))?
        {
            Some(bytes) => {
                let stored: StoredConfig = bincode::deserialize(&bytes)
                    .map_err(|e| anyhow::anyhow!("StoredConfig decode error: {}", e))?;
                Ok(Some(serde_json::to_value(&stored)?))
            }
            None => Ok(None),
        }
    }

    /// List all configs, optionally filtered by tenant prefix
    pub fn list_configs(&self, tenant: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        self.collect_configs(tenant, None)
    }

    /// List configs for export, rejecting namespaces larger than
    /// [`MAX_EXPORT_CONFIGS`] instead of materializing the whole namespace.
    ///
    /// Matches the SQL backend's `enforce_export_cap`: read one extra row to
    /// detect an over-cap result and return a clear error rather than silently
    /// truncating the export.
    pub fn list_configs_for_export(&self, tenant: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        self.list_configs_capped(tenant, MAX_EXPORT_CONFIGS)
    }

    fn list_configs_capped(
        &self,
        tenant: &str,
        cap: usize,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let values = self.collect_configs(tenant, Some(cap.saturating_add(1)))?;
        if values.len() > cap {
            anyhow::bail!(
                "config export would return more than {} configs; narrow the query (group, dataId, appName)",
                cap
            );
        }
        Ok(values)
    }

    fn collect_configs(
        &self,
        tenant: &str,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let cf = self.cf_handle(CF_CONFIG)?;
        let mut values = Vec::new();
        if tenant.is_empty() {
            let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            for item in iter {
                let (_, v) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
                let stored: StoredConfig = bincode::deserialize(&v)
                    .map_err(|e| anyhow::anyhow!("StoredConfig decode error: {}", e))?;
                values.push(serde_json::to_value(&stored)?);
                if limit.is_some_and(|n| values.len() >= n) {
                    break;
                }
            }
        } else {
            let prefix = format!("{}@@", tenant);
            let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
            for item in iter {
                let (k, v) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
                if !k.starts_with(prefix.as_bytes()) {
                    break;
                }
                let stored: StoredConfig = bincode::deserialize(&v)
                    .map_err(|e| anyhow::anyhow!("StoredConfig decode error: {}", e))?;
                values.push(serde_json::to_value(&stored)?);
                if limit.is_some_and(|n| values.len() >= n) {
                    break;
                }
            }
        }
        Ok(values)
    }

    /// Count configs in a namespace (tenant)
    pub fn count_configs(&self, tenant: &str) -> anyhow::Result<i32> {
        let cf = self.cf_handle(CF_CONFIG)?;
        let mut count = 0i32;

        if tenant.is_empty() {
            let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            for item in iter {
                let _ = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
                count += 1;
            }
        } else {
            let prefix = format!("{}@@", tenant);
            let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
            for item in iter {
                let (key, _) =
                    item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
                let key_str = String::from_utf8_lossy(&key);
                if !key_str.starts_with(&prefix) {
                    break;
                }
                count += 1;
            }
        }

        Ok(count)
    }

    /// Find all distinct group names in a namespace
    pub fn find_all_group_names(&self, tenant: &str) -> anyhow::Result<Vec<String>> {
        let cf = self.cf_handle(CF_CONFIG)?;
        let mut groups = std::collections::HashSet::new();
        let prefix = format!("{}@@", tenant);

        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, _) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            // Key format: tenant@@group@@data_id
            let rest = &key_str[prefix.len()..];
            if let Some(sep_pos) = rest.find("@@") {
                groups.insert(rest[..sep_pos].to_string());
            }
        }

        Ok(groups.into_iter().collect())
    }

    /// Search configs with pagination and filters.
    ///
    /// Streams through the RocksDB iterator with inline filtering and pagination.
    /// Only deserializes and collects the entries needed for the requested page,
    /// avoiding loading the entire namespace into memory.
    #[allow(clippy::too_many_arguments)]
    pub fn search_configs(
        &self,
        namespace_id: &str,
        data_id_pattern: &str,
        group_pattern: &str,
        app_name: &str,
        tags: &[String],
        types: &[String],
        content_pattern: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        let cf = self.cf_handle(CF_CONFIG)?;
        let (offset, page_size) = clamp_page(page_no, page_size);

        let mut total: u64 = 0;
        let mut page_items: Vec<serde_json::Value> = Vec::new();

        // Use prefix iterator for namespace, or full scan for empty namespace
        let prefix = if namespace_id.is_empty() {
            String::new()
        } else {
            format!("{}@@", namespace_id)
        };

        let iter = if prefix.is_empty() {
            self.db.iterator_cf(cf, rocksdb::IteratorMode::Start)
        } else {
            self.db.prefix_iterator_cf(cf, prefix.as_bytes())
        };

        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            // Stop when prefix no longer matches (prefix iterator may overshoot)
            if !prefix.is_empty() {
                let key_str = String::from_utf8_lossy(&key);
                if !key_str.starts_with(&prefix) {
                    break;
                }
            }
            let stored: StoredConfig = match bincode::deserialize(&value) {
                Ok(s) => s,
                Err(_) => continue,
            };

            // Apply filters inline (typed fields — no Value indexing)
            if !data_id_pattern.is_empty() && !glob_match(data_id_pattern, &stored.data_id) {
                continue;
            }
            if !group_pattern.is_empty() && !glob_match(group_pattern, &stored.group) {
                continue;
            }
            if !app_name.is_empty() {
                let an = stored.app_name.as_deref().unwrap_or("");
                if an != app_name {
                    continue;
                }
            }
            if !tags.is_empty() {
                let config_tags = stored.config_tags.as_deref().unwrap_or("");
                if !tags.iter().all(|tag| config_tags.contains(tag.as_str())) {
                    continue;
                }
            }
            if !types.is_empty() {
                let config_type = stored.config_type.as_deref().unwrap_or("");
                if !types.iter().any(|t| t == config_type) {
                    continue;
                }
            }
            if !content_pattern.is_empty() && !stored.content.contains(content_pattern) {
                continue;
            }

            // Passed all filters — count toward total and collect if in page window
            if total >= offset && page_items.len() < page_size as usize {
                page_items.push(serde_json::to_value(&stored)?);
            }
            total += 1;
        }

        Ok((page_items, total))
    }
}

/// Simple glob matching supporting `*` wildcards.
/// Splits pattern on `*` and checks that all segments appear in order in the value.
fn glob_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    // No wildcard → exact or contains match
    if !pattern.contains('*') {
        return value.contains(pattern);
    }

    let segments: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0;

    // First segment must match at the start (if non-empty)
    if let Some(first) = segments.first()
        && !first.is_empty()
    {
        if !value.starts_with(first) {
            return false;
        }
        pos = first.len();
    }

    // Last segment must match at the end (if non-empty)
    if let Some(last) = segments.last()
        && !last.is_empty()
        && !value.ends_with(last)
    {
        return false;
    }

    // Middle segments must appear in order
    for seg in &segments[1..segments.len().saturating_sub(1)] {
        if seg.is_empty() {
            continue;
        }
        match value[pos..].find(seg) {
            Some(idx) => pos += idx + seg.len(),
            None => return false,
        }
    }

    true
}

impl RocksDbReader {
    // ==================== Config Gray Operations ====================

    /// Get gray config
    pub fn get_config_gray(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        // Gray configs use a prefix scan since gray_name is part of the key
        let prefix = format!("{}@@{}@@{}@@", tenant, group, data_id);
        let cf = self.cf_handle(CF_CONFIG_GRAY)?;

        let mut iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        if let Some(item) = iter.next() {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            if key.starts_with(prefix.as_bytes()) {
                let stored: StoredConfigGray = bincode::deserialize(&value)
                    .map_err(|e| anyhow::anyhow!("StoredConfigGray decode error: {}", e))?;
                return Ok(Some(serde_json::to_value(&stored)?));
            }
        }
        Ok(None)
    }

    /// Get all gray configs for a data_id/group/tenant
    pub fn get_all_config_grays(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let prefix = format!("{}@@{}@@{}@@", tenant, group, data_id);
        let cf = self.cf_handle(CF_CONFIG_GRAY)?;
        let mut values = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (k, v) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            if !k.starts_with(prefix.as_bytes()) {
                break;
            }
            let stored: StoredConfigGray = bincode::deserialize(&v)
                .map_err(|e| anyhow::anyhow!("StoredConfigGray decode error: {}", e))?;
            values.push(serde_json::to_value(&stored)?);
        }
        Ok(values)
    }

    // ==================== Config History Operations ====================

    /// Get a config history entry by ID.
    ///
    /// The lookup only receives `nid`; there is no tenant/group/dataId to build
    /// the compound history key from, so a point read is impossible without a
    /// second on-disk index (which would change the storage/snapshot format).
    /// The trailing `@@{id}` segment still lets us rule out almost every entry
    /// *before* deserializing it: only a matching (or unparsable) key is
    /// decoded, and the stored id is still checked afterwards so the result is
    /// identical to the old full decode-and-compare scan.
    pub fn get_config_history_by_id(&self, nid: i64) -> anyhow::Result<Option<serde_json::Value>> {
        let cf = self.cf_handle(CF_CONFIG_HISTORY)?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            if history_id_from_key(&key).is_some_and(|id| id != nid) {
                continue;
            }
            let stored = decode_history(&value)?;
            if stored.id == nid {
                return Ok(Some(serde_json::to_value(&stored)?));
            }
        }
        Ok(None)
    }

    /// Search config history with pagination.
    ///
    /// A reverse RocksDB iterator cannot be used here: the key trailing id is
    /// a decimal string, so `"9"` sorts after `"100"`. Instead we rank the
    /// matching keys by their *numeric* id with a bounded top-K selector and
    /// deserialize only the `offset + page_size` candidates that can reach the
    /// page. Any key that does not carry a parseable id (or whose stored id
    /// disagrees with the key) falls back to the streaming full-scan path, so
    /// the page order and totals never depend on the key layout being intact.
    pub fn search_config_history(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        let prefix = format!("{}@@{}@@{}@@", tenant, group, data_id);
        let (offset, page_size) = clamp_page(page_no, page_size);
        let cap = bounded_history_cap(offset, page_size);
        if cap.is_none() {
            return self.history_bounded_page(&prefix, page_no, page_size, |_| true);
        }
        let cap = cap.unwrap_or(0);

        let cf = self.cf_handle(CF_CONFIG_HISTORY)?;
        let mut top = HistoryTopK::new(cap);
        let mut total: u64 = 0;
        for item in self.db.prefix_iterator_cf(cf, prefix.as_bytes()) {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            if !key.starts_with(prefix.as_bytes()) {
                break;
            }
            total += 1;
            let Some(id) = history_id_from_key(&key) else {
                return self.history_bounded_page(&prefix, page_no, page_size, |_| true);
            };
            top.push(id, key.to_vec(), value.to_vec());
        }

        let start = offset as usize;
        let end = start.saturating_add(page_size as usize);
        let mut page_items = Vec::new();
        for (index, (id, bytes)) in top.into_sorted().into_iter().enumerate() {
            let stored = decode_history(&bytes)?;
            if stored.id != id {
                return self.history_bounded_page(&prefix, page_no, page_size, |_| true);
            }
            if index >= start && index < end {
                page_items.push(serde_json::to_value(&stored).unwrap_or(serde_json::Value::Null));
            }
        }
        Ok((page_items, total))
    }

    /// Test-only reference: deserialize every matching record, stable-sort by
    /// numeric id descending, then take the requested page. This is the
    /// implementation of record that all bounded paths must be equivalent to;
    /// production no longer calls it, so it is compiled only for tests.
    #[cfg(test)]
    fn history_full_page<F>(
        &self,
        prefix: &str,
        page_no: u64,
        page_size: u64,
        keep: F,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)>
    where
        F: Fn(&StoredConfigHistory) -> bool,
    {
        let cf = self.cf_handle(CF_CONFIG_HISTORY)?;
        let mut entries: Vec<StoredConfigHistory> = Vec::new();
        for item in self.db.prefix_iterator_cf(cf, prefix.as_bytes()) {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            if !key.starts_with(prefix.as_bytes()) {
                break;
            }
            let stored = decode_history(&value)?;
            if keep(&stored) {
                entries.push(stored);
            }
        }

        // Stable sort by id descending (most recent first). `id` is the
        // canonical, strictly-monotonic history sequence, so this is stable
        // even when two entries share the same millisecond `created_time`.
        entries.sort_by(|a, b| b.id.cmp(&a.id));

        let total = entries.len() as u64;
        let (offset, page_size) = clamp_page(page_no, page_size);
        let page_items: Vec<serde_json::Value> = entries
            .into_iter()
            .skip(offset as usize)
            .take(page_size as usize)
            .map(|s| serde_json::to_value(&s).unwrap_or(serde_json::Value::Null))
            .collect();
        Ok((page_items, total))
    }

    /// Bounded-memory fallback for history pages whose keys cannot be trusted
    /// (unparsable id, stored id disagreeing with the key) or whose requested
    /// page is too deep for the key-only ranking path.
    ///
    /// The authoritative rank is `stored.id`, so every candidate is decoded
    /// once. But the selector retains only compact `(id, key)` entries — never
    /// a full `StoredConfigHistory` per row — and ranks at most
    /// `offset + page_size` of them. The requested window is then read back by
    /// exact key, so only `page_size` records are materialized as JSON. For
    /// pages deeper than `MAX_BOUNDED_HISTORY_KEYS` the retention is O(number
    /// of matching keys) compact entries instead of O(number of matching
    /// records) full records; exact `total` and ordering are unchanged.
    fn history_bounded_page<F>(
        &self,
        prefix: &str,
        page_no: u64,
        page_size: u64,
        keep: F,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)>
    where
        F: Fn(&StoredConfigHistory) -> bool,
    {
        let (offset, page_size) = clamp_page(page_no, page_size);
        let cap = bounded_history_cap(offset, page_size).unwrap_or(usize::MAX);

        let cf = self.cf_handle(CF_CONFIG_HISTORY)?;
        let mut top = HistoryTopK::new(cap);
        let mut total: u64 = 0;
        for item in self.db.prefix_iterator_cf(cf, prefix.as_bytes()) {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            if !key.starts_with(prefix.as_bytes()) {
                break;
            }
            let stored = decode_history(&value)?;
            if !keep(&stored) {
                continue;
            }
            total += 1;
            top.push(stored.id, key.to_vec(), ());
        }

        #[cfg(test)]
        test_instrumentation::note_history_retained_keys(top.len());

        let start = offset as usize;
        let end = start.saturating_add(page_size as usize);
        let mut page_items = Vec::new();
        for (index, (_id, key, ())) in top.into_sorted_entries().into_iter().enumerate() {
            if index >= start && index < end {
                let bytes = self
                    .db
                    .get_cf(cf, &key)
                    .map_err(|e| anyhow::anyhow!("RocksDB get error: {}", e))?
                    .ok_or_else(|| anyhow::anyhow!("history key disappeared during paging"))?;
                let stored = decode_history(&bytes)?;
                page_items.push(serde_json::to_value(&stored).unwrap_or(serde_json::Value::Null));
            }
        }
        Ok((page_items, total))
    }

    /// Search config history with advanced filters.
    ///
    /// Filters need fields from the record, so every candidate is still
    /// deserialized. The bounded top-K selector replaces the full vector plus
    /// full stable sort, keeping only `offset + page_size` decoded records in
    /// memory and ranking them in O(n log cap).
    #[allow(clippy::too_many_arguments)]
    pub fn search_config_history_with_filters(
        &self,
        prefix: &str,
        op_type: Option<&str>,
        src_user: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        let (offset, page_size) = clamp_page(page_no, page_size);
        let matches = |stored: &StoredConfigHistory| -> bool {
            if let Some(op) = op_type
                && stored.op_type != op
            {
                return false;
            }
            if let Some(user) = src_user {
                let entry_user = stored.src_user.as_deref().unwrap_or("");
                if !entry_user.contains(user) {
                    return false;
                }
            }
            if let Some(start) = start_time
                && stored.modified_time < start
            {
                return false;
            }
            if let Some(end) = end_time
                && stored.modified_time > end
            {
                return false;
            }
            true
        };

        let Some(cap) = bounded_history_cap(offset, page_size) else {
            return self.history_bounded_page(prefix, page_no, page_size, matches);
        };

        let cf = self.cf_handle(CF_CONFIG_HISTORY)?;
        let mut top = HistoryTopK::new(cap);
        let mut total: u64 = 0;
        for item in self.db.prefix_iterator_cf(cf, prefix.as_bytes()) {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            if !key.starts_with(prefix.as_bytes()) {
                break;
            }
            let stored = decode_history(&value)?;
            if !matches(&stored) {
                continue;
            }
            total += 1;
            top.push(stored.id, key.to_vec(), stored);
        }

        let start = offset as usize;
        let end = start.saturating_add(page_size as usize);
        let page_items = top
            .into_sorted()
            .into_iter()
            .enumerate()
            .filter(|(index, _)| *index >= start && *index < end)
            .map(|(_, (_, stored))| {
                serde_json::to_value(&stored).unwrap_or(serde_json::Value::Null)
            })
            .collect();
        Ok((page_items, total))
    }

    // ==================== Namespace Operations ====================

    /// Get a namespace by ID
    pub fn get_namespace(&self, namespace_id: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let key = RocksStateMachine::namespace_key(namespace_id);
        self.get_bincode::<StoredNamespace>(CF_NAMESPACE, &key)
    }

    /// List all namespaces
    pub fn list_namespaces(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.iterate_cf_bincode::<StoredNamespace>(CF_NAMESPACE)
    }

    // ==================== User Operations ====================

    /// Get a user by username
    pub fn get_user(&self, username: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let key = RocksStateMachine::user_key(username);
        self.get_bincode::<StoredUser>(CF_USERS, &key)
    }

    /// List all users
    pub fn list_users(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.iterate_cf_bincode::<StoredUser>(CF_USERS)
    }

    /// Search users with optional username filter and pagination.
    ///
    /// Streams the column family so only the requested page is materialized as
    /// JSON; `total` still counts every matching row.
    pub fn search_users(
        &self,
        username_pattern: &str,
        accurate: bool,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        self.paginate_bincode_cf::<StoredUser, _>(
            CF_USERS,
            |stored| {
                if username_pattern.is_empty() {
                    return true;
                }
                if accurate {
                    stored.username == username_pattern
                } else {
                    stored.username.contains(username_pattern)
                }
            },
            page_no,
            page_size,
        )
    }

    // ==================== Role Operations ====================

    /// Get roles by username
    pub fn get_roles_by_username(&self, username: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let cf = self.cf_handle(CF_ROLES)?;
        let mut roles = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let stored: StoredRole = bincode::deserialize(&value)
                .map_err(|e| anyhow::anyhow!("StoredRole decode error: {}", e))?;
            if stored.username == username {
                roles.push(serde_json::to_value(&stored)?);
            }
        }

        Ok(roles)
    }

    /// Search roles with pagination. Streams the column family instead of
    /// materializing every row before filtering.
    pub fn search_roles(
        &self,
        username_pattern: &str,
        role_pattern: &str,
        accurate: bool,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        self.paginate_bincode_cf::<StoredRole, _>(
            CF_ROLES,
            |stored| {
                let username_match = if username_pattern.is_empty() {
                    true
                } else if accurate {
                    stored.username == username_pattern
                } else {
                    stored.username.contains(username_pattern)
                };
                let role_match = if role_pattern.is_empty() {
                    true
                } else if accurate {
                    stored.role == role_pattern
                } else {
                    stored.role.contains(role_pattern)
                };
                username_match && role_match
            },
            page_no,
            page_size,
        )
    }

    /// Check if any global admin role exists
    pub fn has_global_admin(&self) -> anyhow::Result<bool> {
        let cf = self.cf_handle(CF_ROLES)?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let stored: StoredRole = bincode::deserialize(&value)
                .map_err(|e| anyhow::anyhow!("StoredRole decode error: {}", e))?;
            if stored.role == "ROLE_ADMIN" {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Check if a specific user has the global admin role
    pub fn has_global_admin_by_username(&self, username: &str) -> anyhow::Result<bool> {
        let key = RocksStateMachine::role_key("ROLE_ADMIN", username);
        let cf = self.cf_handle(CF_ROLES)?;
        Ok(self
            .db
            .get_cf(cf, key.as_bytes())
            .map_err(|e| anyhow::anyhow!("RocksDB get error: {}", e))?
            .is_some())
    }

    // ==================== Permission Operations ====================

    /// Get a single permission by role, resource, and action
    pub fn get_permission(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let key = RocksStateMachine::permission_key(role, resource, action);
        self.get_bincode::<StoredPermission>(CF_PERMISSIONS, &key)
    }

    /// Get permissions by role
    pub fn get_permissions_by_role(&self, role: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let cf = self.cf_handle(CF_PERMISSIONS)?;
        let prefix = format!("{}@@", role);
        let mut permissions = Vec::new();

        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            if !key.starts_with(prefix.as_bytes()) {
                break;
            }
            let stored: StoredPermission = bincode::deserialize(&value)
                .map_err(|e| anyhow::anyhow!("StoredPermission decode error: {}", e))?;
            permissions.push(serde_json::to_value(&stored)?);
        }

        Ok(permissions)
    }

    /// Get permissions by multiple roles
    pub fn get_permissions_by_roles(
        &self,
        roles: &[String],
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let mut permissions = Vec::new();
        for role in roles {
            permissions.extend(self.get_permissions_by_role(role)?);
        }
        Ok(permissions)
    }

    /// Search permissions with pagination. Streams the column family instead
    /// of materializing every row before filtering.
    pub fn search_permissions(
        &self,
        role_pattern: &str,
        accurate: bool,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)> {
        self.paginate_bincode_cf::<StoredPermission, _>(
            CF_PERMISSIONS,
            |stored| {
                if role_pattern.is_empty() {
                    return true;
                }
                if accurate {
                    stored.role == role_pattern
                } else {
                    stored.role.contains(role_pattern)
                }
            },
            page_no,
            page_size,
        )
    }

    // ==================== Instance Operations ====================

    /// Get an instance
    pub fn get_instance(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let key =
            RocksStateMachine::instance_key(namespace_id, group_name, service_name, instance_id);
        let cf = self.cf_handle(CF_INSTANCES)?;
        match self
            .db
            .get_cf(cf, key.as_bytes())
            .map_err(|e| anyhow::anyhow!("RocksDB get error: {}", e))?
        {
            Some(bytes) => {
                let stored: StoredInstance = bincode::deserialize(&bytes)
                    .map_err(|e| anyhow::anyhow!("StoredInstance decode error: {}", e))?;
                Ok(Some(serde_json::to_value(&stored)?))
            }
            None => Ok(None),
        }
    }

    /// List instances for a service
    pub fn list_instances(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let prefix = format!("{}@@{}@@{}@@", namespace_id, group_name, service_name);
        let cf = self.cf_handle(CF_INSTANCES)?;
        let mut values = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (k, v) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            if !k.starts_with(prefix.as_bytes()) {
                break;
            }
            let stored: StoredInstance = bincode::deserialize(&v)
                .map_err(|e| anyhow::anyhow!("StoredInstance decode error: {}", e))?;
            values.push(serde_json::to_value(&stored)?);
        }
        Ok(values)
    }

    // ==================== Internal Helpers ====================

    /// Get a column family handle
    fn cf_handle(&self, cf_name: &str) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(cf_name)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", cf_name))
    }

    /// Get a bincode-decoded typed value from a CF by key, emitted as `serde_json::Value`.
    ///
    /// Used by cold-path CF readers (namespace/user/role/permission) where the
    /// reader API contract is still `serde_json::Value` for backward compat with
    /// persistence callers, but on-disk format is bincode.
    fn get_bincode<T>(&self, cf_name: &str, key: &str) -> anyhow::Result<Option<serde_json::Value>>
    where
        T: serde::de::DeserializeOwned + serde::Serialize,
    {
        let cf = self.cf_handle(cf_name)?;
        match self
            .db
            .get_cf(cf, key.as_bytes())
            .map_err(|e| anyhow::anyhow!("RocksDB get error: {}", e))?
        {
            Some(bytes) => {
                let stored: T = bincode::deserialize(&bytes)
                    .map_err(|e| anyhow::anyhow!("bincode decode error: {}", e))?;
                Ok(Some(serde_json::to_value(&stored)?))
            }
            None => Ok(None),
        }
    }

    /// Stream one bincode-encoded column family into a filtered page.
    ///
    /// The previous auth searches deserialized every row into a `Vec<Value>`,
    /// filtered that vector, then applied `skip`/`take`. This keeps only the
    /// requested window in memory while still counting every matching row for
    /// `total`. Ordering matches `iterate_cf_bincode` (RocksDB key order), and
    /// the emitted JSON is produced by the same `serde_json::to_value(&T)`.
    /// Corrupt rows still fail the whole query rather than being skipped.
    fn paginate_bincode_cf<T, F>(
        &self,
        cf_name: &str,
        matches: F,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<(Vec<serde_json::Value>, u64)>
    where
        T: serde::de::DeserializeOwned + serde::Serialize,
        F: Fn(&T) -> bool,
    {
        let cf = self.cf_handle(cf_name)?;
        let (offset, page_size) = clamp_page(page_no, page_size);
        let mut total: u64 = 0;
        let mut page_items: Vec<serde_json::Value> = Vec::new();

        for item in self.db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let stored: T = bincode::deserialize(&value)
                .map_err(|e| anyhow::anyhow!("bincode decode error: {}", e))?;
            if !matches(&stored) {
                continue;
            }
            if total >= offset && page_items.len() < page_size as usize {
                #[cfg(test)]
                test_instrumentation::note_auth_materialization();
                page_items.push(serde_json::to_value(&stored)?);
            }
            total += 1;
        }

        Ok((page_items, total))
    }

    /// Iterate all bincode-decoded values in a CF, emitted as `serde_json::Value`.
    fn iterate_cf_bincode<T>(&self, cf_name: &str) -> anyhow::Result<Vec<serde_json::Value>>
    where
        T: serde::de::DeserializeOwned + serde::Serialize,
    {
        let cf = self.cf_handle(cf_name)?;
        let mut values = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, v) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let stored: T = bincode::deserialize(&v)
                .map_err(|e| anyhow::anyhow!("bincode decode error: {}", e))?;
            values.push(serde_json::to_value(&stored)?);
        }
        Ok(values)
    }

    /// Get a JSON value from a column family by key
    #[allow(dead_code)]
    fn get_json(&self, cf_name: &str, key: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let cf = self.cf_handle(cf_name)?;
        match self.db.get_cf(cf, key.as_bytes()) {
            Ok(Some(bytes)) => {
                let json: serde_json::Value = serde_json::from_slice(&bytes)?;
                Ok(Some(json))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("RocksDB get error: {}", e)),
        }
    }

    /// Iterate all values in a column family
    #[allow(dead_code)]
    fn iterate_cf_values(&self, cf_name: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let cf = self.cf_handle(cf_name)?;
        let mut values = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            values.push(json);
        }

        Ok(values)
    }

    /// Iterate values in a column family with a key prefix
    #[allow(dead_code)]
    fn iterate_cf_values_with_prefix(
        &self,
        cf_name: &str,
        prefix: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let cf = self.cf_handle(cf_name)?;
        let mut values = Vec::new();

        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(prefix) {
                break;
            }
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            values.push(json);
        }

        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_db() -> (Arc<DB>, TempDir) {
        let tmp_dir = TempDir::new().unwrap();
        let sm: RocksStateMachine = RocksStateMachine::new(tmp_dir.path()).await.unwrap();
        (sm.db(), tmp_dir)
    }

    #[tokio::test]
    async fn test_reader_config_crud() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;

        // Write a config directly (bincode-encoded StoredConfig)
        let key = RocksStateMachine::config_key("test-data", "DEFAULT_GROUP", "public");
        let stored = StoredConfig {
            data_id: "test-data".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            tenant: "public".to_string(),
            content: "key=value".to_string(),
            md5: "abc123".to_string(),
            config_type: Some("properties".to_string()),
            app_name: Some(String::new()),
            config_tags: Some(String::new()),
            ..Default::default()
        };
        let cf = db.cf_handle("batata_config").unwrap();
        db.put_cf(cf, key.as_bytes(), bincode::serialize(&stored).unwrap())
            .unwrap();

        let reader = RocksDbReader::new(db);

        // Test get_config
        let result = reader
            .get_config("test-data", "DEFAULT_GROUP", "public")
            .unwrap();
        assert!(result.is_some());
        let config = result.unwrap();
        assert_eq!(config["data_id"].as_str(), Some("test-data"));
        assert_eq!(config["content"].as_str(), Some("key=value"));

        // Test get_config not found
        let result = reader
            .get_config("nonexistent", "DEFAULT_GROUP", "public")
            .unwrap();
        assert!(result.is_none());

        // Test list_configs
        let configs = reader.list_configs("public").unwrap();
        assert_eq!(configs.len(), 1);

        // Test count_configs
        let count = reader.count_configs("public").unwrap();
        assert_eq!(count, 1);

        let count = reader.count_configs("other").unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_reader_namespace_crud() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;

        // Write a namespace (bincode StoredNamespace)
        let key = RocksStateMachine::namespace_key("test-ns");
        let stored = StoredNamespace {
            namespace_id: "test-ns".to_string(),
            namespace_name: "Test Namespace".to_string(),
            namespace_desc: Some("A test namespace".to_string()),
            ..Default::default()
        };
        let cf = db.cf_handle("batata_namespace").unwrap();
        db.put_cf(cf, key.as_bytes(), bincode::serialize(&stored).unwrap())
            .unwrap();

        let reader = RocksDbReader::new(db);

        let result = reader.get_namespace("test-ns").unwrap();
        assert!(result.is_some());
        assert_eq!(
            result.unwrap()["namespace_name"].as_str(),
            Some("Test Namespace")
        );

        let namespaces = reader.list_namespaces().unwrap();
        assert_eq!(namespaces.len(), 1);
    }

    #[tokio::test]
    async fn test_reader_user_search() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;

        let cf = db.cf_handle("batata_users").unwrap();
        for name in &["alice", "bob", "alice_admin"] {
            let key = RocksStateMachine::user_key(name);
            let stored = StoredUser {
                username: name.to_string(),
                password_hash: "hash".to_string(),
                enabled: true,
                ..Default::default()
            };
            db.put_cf(cf, key.as_bytes(), bincode::serialize(&stored).unwrap())
                .unwrap();
        }

        let reader = RocksDbReader::new(db);

        // Search with pattern
        let (users, total) = reader.search_users("alice", false, 1, 10).unwrap();
        assert_eq!(total, 2); // alice and alice_admin
        assert_eq!(users.len(), 2);

        // Accurate search
        let (users, total) = reader.search_users("alice", true, 1, 10).unwrap();
        assert_eq!(total, 1);
        assert_eq!(users.len(), 1);

        // All users
        let (users, total) = reader.search_users("", false, 1, 10).unwrap();
        assert_eq!(total, 3);
        assert_eq!(users.len(), 3);
    }

    #[tokio::test]
    async fn test_reader_roles_and_permissions() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;

        // Write roles (bincode StoredRole)
        let role_cf = db.cf_handle("batata_roles").unwrap();
        let key1 = RocksStateMachine::role_key("ROLE_ADMIN", "admin");
        let role1 = StoredRole {
            role: "ROLE_ADMIN".to_string(),
            username: "admin".to_string(),
            ..Default::default()
        };
        db.put_cf(
            role_cf,
            key1.as_bytes(),
            bincode::serialize(&role1).unwrap(),
        )
        .unwrap();

        let key2 = RocksStateMachine::role_key("ROLE_USER", "alice");
        let role2 = StoredRole {
            role: "ROLE_USER".to_string(),
            username: "alice".to_string(),
            ..Default::default()
        };
        db.put_cf(
            role_cf,
            key2.as_bytes(),
            bincode::serialize(&role2).unwrap(),
        )
        .unwrap();

        // Write permissions (bincode StoredPermission)
        let perm_cf = db.cf_handle("batata_permissions").unwrap();
        let pkey = RocksStateMachine::permission_key("ROLE_ADMIN", "public::*", "rw");
        let perm = StoredPermission {
            role: "ROLE_ADMIN".to_string(),
            resource: "public::*".to_string(),
            action: "rw".to_string(),
            ..Default::default()
        };
        db.put_cf(perm_cf, pkey.as_bytes(), bincode::serialize(&perm).unwrap())
            .unwrap();

        let reader = RocksDbReader::new(db);

        // Test roles
        let roles = reader.get_roles_by_username("admin").unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0]["role"].as_str(), Some("ROLE_ADMIN"));

        // Test has_global_admin
        assert!(reader.has_global_admin().unwrap());
        assert!(reader.has_global_admin_by_username("admin").unwrap());
        assert!(!reader.has_global_admin_by_username("alice").unwrap());

        // Test permissions
        let perms = reader.get_permissions_by_role("ROLE_ADMIN").unwrap();
        assert_eq!(perms.len(), 1);
        assert_eq!(perms[0]["resource"].as_str(), Some("public::*"));

        // Test permissions by roles
        let perms = reader
            .get_permissions_by_roles(&["ROLE_ADMIN".to_string(), "ROLE_USER".to_string()])
            .unwrap();
        assert_eq!(perms.len(), 1); // Only ROLE_ADMIN has permissions
    }

    #[tokio::test]
    async fn test_reader_group_names() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;

        let cf = db.cf_handle("batata_config").unwrap();
        for (data_id, group) in &[("d1", "GROUP_A"), ("d2", "GROUP_A"), ("d3", "GROUP_B")] {
            let key = RocksStateMachine::config_key(data_id, group, "public");
            let stored = StoredConfig {
                data_id: data_id.to_string(),
                group: group.to_string(),
                tenant: "public".to_string(),
                content: "test".to_string(),
                ..Default::default()
            };
            db.put_cf(cf, key.as_bytes(), bincode::serialize(&stored).unwrap())
                .unwrap();
        }

        let reader = RocksDbReader::new(db);
        let mut groups = reader.find_all_group_names("public").unwrap();
        groups.sort();
        assert_eq!(groups, vec!["GROUP_A", "GROUP_B"]);
    }

    /// Seed `count` configs in the `public` namespace.
    fn seed_configs(db: &DB, count: u64) {
        let cf = db.cf_handle("batata_config").unwrap();
        for i in 0..count {
            let data_id = format!("d{i:05}");
            let key = RocksStateMachine::config_key(&data_id, "DEFAULT_GROUP", "public");
            let stored = StoredConfig {
                data_id,
                group: "DEFAULT_GROUP".to_string(),
                tenant: "public".to_string(),
                content: "v".to_string(),
                ..Default::default()
            };
            db.put_cf(cf, key.as_bytes(), bincode::serialize(&stored).unwrap())
                .unwrap();
        }
    }

    /// A caller requesting `pageSize = u64::MAX` must not materialize the whole
    /// column family. Mutant check: removing `clamp_page` from `search_configs`
    /// makes `page.len()` equal `SEED` instead of `MAX_PAGE_SIZE`.
    #[tokio::test]
    async fn search_configs_clamps_unbounded_page_size() {
        use batata_common::model::MAX_PAGE_SIZE;

        const SEED: u64 = MAX_PAGE_SIZE + 5;
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;
        seed_configs(&db, SEED);
        let reader = RocksDbReader::new(db);

        let (page, total) = reader
            .search_configs("public", "", "", "", &[], &[], "", 1, u64::MAX)
            .unwrap();
        assert_eq!(total, SEED);
        assert_eq!(page.len() as u64, MAX_PAGE_SIZE);
    }

    /// `page_no = u64::MAX` used to overflow `(page_no - 1) * page_size` in
    /// debug builds. Mutant check: restoring the plain multiplication panics.
    #[tokio::test]
    async fn search_configs_does_not_overflow_on_max_page_no() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;
        seed_configs(&db, 3);
        let reader = RocksDbReader::new(db);

        let (page, total) = reader
            .search_configs("public", "", "", "", &[], &[], "", u64::MAX, u64::MAX)
            .unwrap();
        assert_eq!(total, 3);
        assert!(page.is_empty(), "the saturated offset is past the last row");
    }

    /// Export must bound the read and reject an over-cap namespace instead of
    /// silently truncating. Mutant check: removing the `len() > cap` guard
    /// makes the `is_err()` assertion fail.
    #[tokio::test]
    async fn export_list_is_bounded_and_rejects_over_cap() {
        let (db, _tmp_dir): (Arc<DB>, TempDir) = create_test_db().await;
        seed_configs(&db, 3);
        let reader = RocksDbReader::new(db);

        // Bounded read stops after cap + 1 rows.
        assert_eq!(reader.collect_configs("public", Some(2)).unwrap().len(), 2);
        // Success path (cap >= rows) and failure path (cap < rows).
        assert_eq!(reader.list_configs_capped("public", 3).unwrap().len(), 3);
        assert!(reader.list_configs_capped("public", 2).is_err());
    }

    const HISTORY_PREFIX: &str = "public@@DEFAULT_GROUP@@app.yml@@";

    fn seed_history(db: &Arc<DB>, ids: &[i64]) {
        let cf = db.cf_handle(CF_CONFIG_HISTORY).unwrap();
        for (n, id) in ids.iter().enumerate() {
            let key = RocksStateMachine::config_history_key(
                "app.yml",
                "DEFAULT_GROUP",
                "public",
                *id as u64,
            );
            let stored = StoredConfigHistory {
                id: *id,
                data_id: "app.yml".to_string(),
                group: "DEFAULT_GROUP".to_string(),
                tenant: "public".to_string(),
                content: "x".repeat(512),
                md5: format!("md5-{}", id),
                op_type: if n % 2 == 0 { "U" } else { "I" }.to_string(),
                src_user: Some(if n % 3 == 0 { "alice" } else { "bob" }.to_string()),
                modified_time: *id * 10,
                ..Default::default()
            };
            db.put_cf(cf, key.as_bytes(), bincode::serialize(&stored).unwrap())
                .unwrap();
        }
    }

    fn write_history_with_key(db: &Arc<DB>, key: &str, id: i64) {
        let cf = db.cf_handle(CF_CONFIG_HISTORY).unwrap();
        let stored = StoredConfigHistory {
            id,
            data_id: "app.yml".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            tenant: "public".to_string(),
            content: "x".repeat(512),
            ..Default::default()
        };
        db.put_cf(cf, key.as_bytes(), bincode::serialize(&stored).unwrap())
            .unwrap();
    }

    /// The bounded numeric-id ranking must be indistinguishable from the old
    /// full decode + stable sort on every page, including non-padded ids where
    /// key lexicographic order differs from numeric order.
    #[tokio::test]
    async fn bounded_history_pagination_matches_full_scan_on_every_page() {
        let (db, _tmp_dir) = create_test_db().await;
        seed_history(&db, &[1, 2, 9, 10, 99, 100, 101, 1000]);
        let reader = RocksDbReader::new(db);

        for (page_no, page_size) in [
            (1, 3),
            (2, 3),
            (3, 3),
            (4, 3),
            (1, 1000),
            (0, 3),
            (u64::MAX, 3),
        ] {
            let (bounded, bounded_total) = reader
                .search_config_history("app.yml", "DEFAULT_GROUP", "public", page_no, page_size)
                .unwrap();
            let (reference, reference_total) = reader
                .history_full_page(HISTORY_PREFIX, page_no, page_size, |_| true)
                .unwrap();
            assert_eq!(bounded_total, reference_total, "page_no={page_no}");
            assert_eq!(
                bounded, reference,
                "page_no={page_no} page_size={page_size}"
            );
        }
    }

    /// Guard for the optimisation itself: if the bounded path regresses to a
    /// full scan (or to a lexicographic reverse iterator) this test turns red.
    #[tokio::test]
    async fn bounded_history_page_decodes_only_the_requested_window() {
        let (db, _tmp_dir) = create_test_db().await;
        let ids: Vec<i64> = (1..=200).collect();
        seed_history(&db, &ids);
        let reader = RocksDbReader::new(db);

        test_instrumentation::reset();
        let (page, total) = reader
            .search_config_history("app.yml", "DEFAULT_GROUP", "public", 1, 5)
            .unwrap();
        let bounded_decodes = test_instrumentation::decode_count();
        assert_eq!(total, 200);
        assert_eq!(page.len(), 5);
        assert_eq!(page[0]["id"].as_i64(), Some(200));
        assert_eq!(page[4]["id"].as_i64(), Some(196));
        assert!(
            bounded_decodes <= 5,
            "bounded path deserialized {bounded_decodes} records for a 5-row page"
        );

        test_instrumentation::reset();
        let (reference, _) = reader
            .history_full_page(HISTORY_PREFIX, 1, 5, |_| true)
            .unwrap();
        assert_eq!(reference, page, "full scan must return the same page");
        assert_eq!(test_instrumentation::decode_count(), 200);
    }

    /// A key whose trailing id is not decimal must send the query down the
    /// full-scan fallback: its stored id still has to appear, in numeric order.
    #[tokio::test]
    async fn bounded_history_falls_back_for_non_decimal_key() {
        let (db, _tmp_dir) = create_test_db().await;
        let ids: Vec<i64> = (1..=200).collect();
        seed_history(&db, &ids);
        write_history_with_key(&db, "public@@DEFAULT_GROUP@@app.yml@@legacy", 5000);
        let reader = RocksDbReader::new(db);

        test_instrumentation::reset();
        let (page, total) = reader
            .search_config_history("app.yml", "DEFAULT_GROUP", "public", 1, 5)
            .unwrap();
        assert_eq!(total, 201);
        assert_eq!(page.len(), 5);
        assert_eq!(page[0]["id"].as_i64(), Some(5000));
        assert_eq!(page[1]["id"].as_i64(), Some(200));
        assert!(
            test_instrumentation::decode_count() >= 201,
            "fallback must deserialize every record"
        );
    }

    /// Legacy data whose stored id disagrees with the key must also fall back,
    /// so the page is ordered by the authoritative stored id.
    #[tokio::test]
    async fn bounded_history_falls_back_for_mismatched_stored_id() {
        let (db, _tmp_dir) = create_test_db().await;
        seed_history(&db, &[1, 2, 3]);
        // Key id 0 sorts last, stored id 7000 sorts first. Keep page_size
        // larger than the dataset so the odd record is inside the top-K and
        // actually gets decoded (that is what exposes the mismatch guard).
        write_history_with_key(&db, "public@@DEFAULT_GROUP@@app.yml@@0", 7000);
        let reader = RocksDbReader::new(db);

        test_instrumentation::reset();
        let (page, total) = reader
            .search_config_history("app.yml", "DEFAULT_GROUP", "public", 1, 10)
            .unwrap();
        assert_eq!(total, 4);
        assert_eq!(page[0]["id"].as_i64(), Some(7000));
        assert_eq!(page[1]["id"].as_i64(), Some(3));
        assert!(
            test_instrumentation::decode_count() >= 4,
            "mismatched id must trigger the full-scan fallback"
        );
    }

    /// `get_config_history_by_id` has only the id, so it still scans keys —
    /// but it must decode only the candidate(s) and must not skip legacy keys.
    #[tokio::test]
    async fn history_find_by_id_is_exact_and_skips_unrelated_decodes() {
        let (db, _tmp_dir) = create_test_db().await;
        let ids: Vec<i64> = (1..=200).collect();
        seed_history(&db, &ids);
        write_history_with_key(&db, "public@@DEFAULT_GROUP@@app.yml@@legacy", 7777);
        let reader = RocksDbReader::new(db);

        test_instrumentation::reset();
        let found = reader.get_config_history_by_id(7777).unwrap().unwrap();
        assert_eq!(found["id"].as_i64(), Some(7777));
        assert!(
            test_instrumentation::decode_count() <= 2,
            "unrelated history records must not be deserialized"
        );

        test_instrumentation::reset();
        assert!(reader.get_config_history_by_id(424_242).unwrap().is_none());
        assert!(test_instrumentation::decode_count() <= 2);
    }

    fn reference_filter(
        stored: &StoredConfigHistory,
        op_type: Option<&str>,
        src_user: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> bool {
        if let Some(op) = op_type
            && stored.op_type != op
        {
            return false;
        }
        if let Some(user) = src_user
            && !stored.src_user.as_deref().unwrap_or("").contains(user)
        {
            return false;
        }
        if let Some(start) = start_time
            && stored.modified_time < start
        {
            return false;
        }
        if let Some(end) = end_time
            && stored.modified_time > end
        {
            return false;
        }
        true
    }

    /// The filtered path still has to deserialize every record (filters live
    /// in the value), but its bounded ranking must match the old full sort.
    #[tokio::test]
    async fn filtered_history_pagination_matches_full_scan() {
        let (db, _tmp_dir) = create_test_db().await;
        let ids: Vec<i64> = (1..=40).collect();
        seed_history(&db, &ids);
        let reader = RocksDbReader::new(db);

        let cases: [(Option<&str>, Option<&str>, Option<i64>, Option<i64>); 4] = [
            (None, None, None, None),
            (Some("U"), None, None, None),
            (None, Some("alice"), None, None),
            (Some("I"), Some("bob"), Some(50), Some(350)),
        ];
        for (op, user, start, end) in cases {
            for (page_no, page_size) in [(1, 7), (2, 7), (3, 7), (1, 1000)] {
                let (bounded, bounded_total) = reader
                    .search_config_history_with_filters(
                        HISTORY_PREFIX,
                        op,
                        user,
                        start,
                        end,
                        page_no,
                        page_size,
                    )
                    .unwrap();
                let (reference, reference_total) = reader
                    .history_full_page(HISTORY_PREFIX, page_no, page_size, |stored| {
                        reference_filter(stored, op, user, start, end)
                    })
                    .unwrap();
                assert_eq!(bounded_total, reference_total, "filter={op:?}/{user:?}");
                assert_eq!(bounded, reference, "filter={op:?}/{user:?} page={page_no}");
            }
        }
    }

    /// Same-binary / same-session A/B diagnostic. Absolute RPS is meaningless
    /// on this host; only the full-scan vs bounded ratio in one process counts.
    #[tokio::test]
    async fn history_pagination_ab_same_binary() {
        use std::time::Instant;

        let (db, _tmp_dir) = create_test_db().await;
        let ids: Vec<i64> = (1..=1000).collect();
        seed_history(&db, &ids);
        let reader = RocksDbReader::new(db);

        let (fast, fast_total) = reader
            .search_config_history("app.yml", "DEFAULT_GROUP", "public", 1, 20)
            .unwrap();
        let (slow, slow_total) = reader
            .history_full_page(HISTORY_PREFIX, 1, 20, |_| true)
            .unwrap();
        assert_eq!(fast, slow);
        assert_eq!(fast_total, slow_total);

        let loops = 8;
        let t0 = Instant::now();
        for _ in 0..loops {
            reader
                .history_full_page(HISTORY_PREFIX, 1, 20, |_| true)
                .unwrap();
        }
        let full_scan = t0.elapsed();
        let t1 = Instant::now();
        for _ in 0..loops {
            reader
                .search_config_history("app.yml", "DEFAULT_GROUP", "public", 1, 20)
                .unwrap();
        }
        let bounded = t1.elapsed();
        println!(
            "history A/B ({} rows x {} loops): full_scan={:?} bounded={:?} ratio={:.2}x",
            ids.len(),
            loops,
            full_scan,
            bounded,
            full_scan.as_secs_f64() / bounded.as_secs_f64().max(f64::EPSILON)
        );
    }

    // ==================== Leftover: bounded auth / history scans ==========

    /// One user, one role and one permission per row. Two role names keep both
    /// exact and fuzzy filters meaningful on every surface.
    fn seed_auth_rows(db: &Arc<DB>, count: u64) {
        let users_cf = db.cf_handle(CF_USERS).unwrap();
        let roles_cf = db.cf_handle(CF_ROLES).unwrap();
        let perms_cf = db.cf_handle(CF_PERMISSIONS).unwrap();
        for i in 0..count {
            let username = format!("user{i:04}");
            let role = if i % 3 == 0 {
                "ROLE_ADMIN"
            } else {
                "ROLE_USER"
            };

            let user = StoredUser {
                username: username.clone(),
                password_hash: "h".repeat(64),
                enabled: true,
                ..Default::default()
            };
            db.put_cf(
                users_cf,
                RocksStateMachine::user_key(&username).as_bytes(),
                bincode::serialize(&user).unwrap(),
            )
            .unwrap();

            let stored_role = StoredRole {
                role: role.to_string(),
                username: username.clone(),
                ..Default::default()
            };
            db.put_cf(
                roles_cf,
                RocksStateMachine::role_key(role, &username).as_bytes(),
                bincode::serialize(&stored_role).unwrap(),
            )
            .unwrap();

            let resource = format!("public::{i:04}");
            let permission = StoredPermission {
                role: role.to_string(),
                resource: resource.clone(),
                action: "rw".to_string(),
                ..Default::default()
            };
            db.put_cf(
                perms_cf,
                RocksStateMachine::permission_key(role, &resource, "rw").as_bytes(),
                bincode::serialize(&permission).unwrap(),
            )
            .unwrap();
        }
    }

    /// Old implementation of record: materialize the whole CF as JSON, filter
    /// it, then `skip`/`take`. The streaming path must be wire-for-wire
    /// identical to this on every filter / page combination.
    fn legacy_paginate(
        all: Vec<serde_json::Value>,
        keep: impl Fn(&serde_json::Value) -> bool,
        page_no: u64,
        page_size: u64,
    ) -> (Vec<serde_json::Value>, u64) {
        let filtered: Vec<serde_json::Value> = all.into_iter().filter(keep).collect();
        let total = filtered.len() as u64;
        let (offset, page_size) = clamp_page(page_no, page_size);
        let page = filtered
            .into_iter()
            .skip(offset as usize)
            .take(page_size as usize)
            .collect();
        (page, total)
    }

    fn legacy_user_page(
        reader: &RocksDbReader,
        pattern: &str,
        accurate: bool,
        page_no: u64,
        page_size: u64,
    ) -> (Vec<serde_json::Value>, u64) {
        legacy_paginate(
            reader.list_users().unwrap(),
            |v| {
                if pattern.is_empty() {
                    return true;
                }
                let username = v["username"].as_str().unwrap_or("");
                if accurate {
                    username == pattern
                } else {
                    username.contains(pattern)
                }
            },
            page_no,
            page_size,
        )
    }

    fn legacy_role_page(
        reader: &RocksDbReader,
        user_pattern: &str,
        role_pattern: &str,
        accurate: bool,
        page_no: u64,
        page_size: u64,
    ) -> (Vec<serde_json::Value>, u64) {
        legacy_paginate(
            reader.iterate_cf_bincode::<StoredRole>(CF_ROLES).unwrap(),
            |v| {
                let username = v["username"].as_str().unwrap_or("");
                let role = v["role"].as_str().unwrap_or("");
                let username_match = if user_pattern.is_empty() {
                    true
                } else if accurate {
                    username == user_pattern
                } else {
                    username.contains(user_pattern)
                };
                let role_match = if role_pattern.is_empty() {
                    true
                } else if accurate {
                    role == role_pattern
                } else {
                    role.contains(role_pattern)
                };
                username_match && role_match
            },
            page_no,
            page_size,
        )
    }

    fn legacy_permission_page(
        reader: &RocksDbReader,
        role_pattern: &str,
        accurate: bool,
        page_no: u64,
        page_size: u64,
    ) -> (Vec<serde_json::Value>, u64) {
        legacy_paginate(
            reader
                .iterate_cf_bincode::<StoredPermission>(CF_PERMISSIONS)
                .unwrap(),
            |v| {
                if role_pattern.is_empty() {
                    return true;
                }
                let role = v["role"].as_str().unwrap_or("");
                if accurate {
                    role == role_pattern
                } else {
                    role.contains(role_pattern)
                }
            },
            page_no,
            page_size,
        )
    }

    /// Correctness guard for `paginate_bincode_cf`: replacing a full-vector
    /// scan with a streaming scan must not change the returned page, the exact
    /// `total`, the ordering, or the behaviour of edge pages.
    #[tokio::test]
    async fn streaming_auth_paging_matches_legacy_full_scan() {
        const N: u64 = 60;
        let (db, _tmp_dir) = create_test_db().await;
        seed_auth_rows(&db, N);
        let reader = RocksDbReader::new(db);

        let pages = [
            (1, 7),
            (2, 7),
            (9, 7),
            (1, 1000),
            (1, 0),
            (0, 7),
            (u64::MAX, 7),
        ];

        for (pattern, accurate) in [
            ("", false),
            ("user", false),
            // "user00" is a prefix of every username but matches none exactly:
            // these two cases make accurate vs fuzzy observably different.
            ("user00", true),
            ("user00", false),
            ("user0001", true),
            ("user0001", false),
            ("no-such-user", false),
        ] {
            for (page_no, page_size) in pages {
                assert_eq!(
                    reader
                        .search_users(pattern, accurate, page_no, page_size)
                        .unwrap(),
                    legacy_user_page(&reader, pattern, accurate, page_no, page_size),
                    "users pattern={pattern:?} accurate={accurate} page={page_no}/{page_size}"
                );
            }
        }

        for (user_pattern, role_pattern, accurate) in [
            ("", "", false),
            ("user00", "", true),
            ("user00", "ROLE_ADMIN", false),
            ("", "ROLE_USER", false),
            ("", "ROLE_ADMIN", true),
            ("user0001", "ROLE_ADMIN", true),
        ] {
            for (page_no, page_size) in pages {
                assert_eq!(
                    reader
                        .search_roles(user_pattern, role_pattern, accurate, page_no, page_size)
                        .unwrap(),
                    legacy_role_page(
                        &reader,
                        user_pattern,
                        role_pattern,
                        accurate,
                        page_no,
                        page_size
                    ),
                    "roles user={user_pattern:?} role={role_pattern:?} accurate={accurate} page={page_no}/{page_size}"
                );
            }
        }

        for (role_pattern, accurate) in [
            ("", false),
            ("ROLE_ADMIN", false),
            ("ROLE", false),
            ("ROLE", true),
            ("ROLE_ADMIN", true),
        ] {
            for (page_no, page_size) in pages {
                assert_eq!(
                    reader
                        .search_permissions(role_pattern, accurate, page_no, page_size)
                        .unwrap(),
                    legacy_permission_page(&reader, role_pattern, accurate, page_no, page_size),
                    "permissions role={role_pattern:?} accurate={accurate} page={page_no}/{page_size}"
                );
            }
        }
    }

    /// Guard for the optimisation itself: only the returned rows are turned
    /// into `serde_json::Value`. Reverting to the old "materialize all rows"
    /// implementation (or dropping the offset/window guards) turns this red.
    #[tokio::test]
    async fn streaming_auth_paging_materializes_only_the_requested_window() {
        const N: u64 = 60;
        let (db, _tmp_dir) = create_test_db().await;
        seed_auth_rows(&db, N);
        let reader = RocksDbReader::new(db);

        for (page_no, page_size, expected) in [
            (1, 5, 5),
            (3, 5, 5),
            (13, 5, 0),
            (1, u64::MAX, N as usize),
            (2, u64::MAX, 0),
        ] {
            test_instrumentation::reset();
            let (page, total) = reader.search_users("", false, page_no, page_size).unwrap();
            assert_eq!(total, N);
            assert_eq!(page.len(), expected, "page={page_no}/{page_size}");
            assert_eq!(
                test_instrumentation::auth_materialization_count(),
                expected,
                "only returned rows may become Values (page={page_no}/{page_size})"
            );
        }

        test_instrumentation::reset();
        // "user001" matches user0010..user0019 only (not user0001).
        let (page, total) = reader.search_users("user001", false, 1, 3).unwrap();
        assert_eq!(total, 10);
        assert_eq!(page.len(), 3);
        assert_eq!(test_instrumentation::auth_materialization_count(), 3);

        test_instrumentation::reset();
        let (page, total) = reader
            .search_roles("user", "ROLE_ADMIN", false, 1, 4)
            .unwrap();
        assert_eq!(total, 20);
        assert_eq!(page.len(), 4);
        assert_eq!(test_instrumentation::auth_materialization_count(), 4);

        test_instrumentation::reset();
        let (page, total) = reader
            .search_permissions("ROLE_ADMIN", false, 2, 4)
            .unwrap();
        assert_eq!(total, 20);
        assert_eq!(page.len(), 4);
        assert_eq!(test_instrumentation::auth_materialization_count(), 4);
    }

    /// `history_bounded_page` retains compact `(id, key)` entries and re-reads
    /// only the requested window by exact key. Mutant check: reverting to the
    /// full-vector reference implementation drops both counters.
    #[tokio::test]
    async fn bounded_history_fallback_retains_keys_not_records() {
        let (db, _tmp_dir) = create_test_db().await;
        let ids: Vec<i64> = (1..=200).collect();
        seed_history(&db, &ids);
        let reader = RocksDbReader::new(db);

        for (page_no, page_size, expected_retained) in [(1, 5, 5), (3, 5, 15)] {
            test_instrumentation::reset();
            let (page, total) = reader
                .history_bounded_page(HISTORY_PREFIX, page_no, page_size, |_| true)
                .unwrap();
            let decodes = test_instrumentation::decode_count();
            let retained = test_instrumentation::history_retained_keys();
            assert_eq!(total, 200);
            assert_eq!(page.len(), 5);
            assert_eq!(retained, expected_retained, "page={page_no}/{page_size}");
            assert_eq!(
                decodes,
                200 + 5,
                "page={page_no}/{page_size}: one scan decode per row plus exact-key re-reads for the window"
            );

            let (reference, reference_total) = reader
                .history_full_page(HISTORY_PREFIX, page_no, page_size, |_| true)
                .unwrap();
            assert_eq!(reference_total, total);
            assert_eq!(reference, page, "page={page_no}/{page_size}");
        }
    }

    /// The public API must route untrusted-key fallbacks through the bounded
    /// page rather than the old full-vector reference implementation.
    #[tokio::test]
    async fn history_public_fallback_uses_bounded_page() {
        let (db, _tmp_dir) = create_test_db().await;
        let ids: Vec<i64> = (1..=200).collect();
        seed_history(&db, &ids);
        write_history_with_key(&db, "public@@DEFAULT_GROUP@@app.yml@@legacy", 5000);
        let reader = RocksDbReader::new(db);

        test_instrumentation::reset();
        let (page, total) = reader
            .search_config_history("app.yml", "DEFAULT_GROUP", "public", 1, 5)
            .unwrap();
        let decodes = test_instrumentation::decode_count();
        let retained = test_instrumentation::history_retained_keys();
        assert_eq!(total, 201);
        assert_eq!(page.len(), 5);
        assert_eq!(page[0]["id"].as_i64(), Some(5000));
        assert_eq!(retained, 5);
        assert_eq!(decodes, 201 + 5);
    }

    /// A page deeper than `MAX_BOUNDED_HISTORY_KEYS` takes the `cap = None`
    /// branch; it must still return the exact window and the exact total.
    #[tokio::test]
    async fn deep_history_page_returns_exact_window_without_full_record_vector() {
        use crate::raft::history::MAX_BOUNDED_HISTORY_KEYS;

        const N: i64 = (MAX_BOUNDED_HISTORY_KEYS + 5) as i64;
        let page_no: u64 = 11;
        let page_size: u64 = 1000;
        assert!(bounded_history_cap((page_no - 1) * page_size, page_size).is_none());

        let (db, _tmp_dir) = create_test_db().await;
        let ids: Vec<i64> = (1..=N).collect();
        seed_history(&db, &ids);
        let reader = RocksDbReader::new(db);

        test_instrumentation::reset();
        let (page, total) = reader
            .search_config_history("app.yml", "DEFAULT_GROUP", "public", page_no, page_size)
            .unwrap();
        assert_eq!(total, N as u64);
        assert_eq!(page.len(), 5);
        assert_eq!(page[0]["id"].as_i64(), Some(5));
        assert_eq!(page[4]["id"].as_i64(), Some(1));
        assert_eq!(test_instrumentation::history_retained_keys(), N as usize);
        assert_eq!(test_instrumentation::decode_count(), N as usize + 5);
    }
}
