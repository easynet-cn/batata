// Embedded persistence backend using RocksDB
// Provides standalone (single-node) storage without an external database

use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::reader::RocksDbReader;
use batata_consistency::raft::state_machine::{
    CF_CONFIG, CF_CONFIG_GRAY, CF_CONFIG_HISTORY, CF_NAMESPACE, CF_PERMISSIONS, CF_ROLES, CF_USERS,
    RocksStateMachine,
};

use crate::model::{
    CapacityInfo, ConfigGrayStorageData, ConfigHistoryStorageData, ConfigStorageData,
    NamespaceInfo, Page, PermissionInfo, RoleInfo, StorageMode, UserInfo,
};
use crate::traits::PersistenceService;
use crate::traits::auth::AuthPersistence;
use crate::traits::capacity::CapacityPersistence;
use crate::traits::config::ConfigPersistence;
use crate::traits::namespace::NamespacePersistence;

/// Standalone embedded persistence using RocksDB
///
/// Reads via `RocksDbReader`, writes directly to RocksDB (no Raft).
/// Suitable for single-node deployments without an external database.
pub struct EmbeddedPersistService {
    reader: RocksDbReader,
    db: Arc<DB>,
}

impl EmbeddedPersistService {
    /// Create from a RocksStateMachine
    pub fn from_state_machine(sm: &RocksStateMachine) -> Self {
        let db = sm.db();
        Self {
            reader: RocksDbReader::new(db.clone()),
            db,
        }
    }

    /// Create from a raw RocksDB instance
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            reader: RocksDbReader::new(db.clone()),
            db,
        }
    }

    /// Get a column family handle
    fn cf(&self, name: &str) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", name))
    }

    /// Write a JSON value to a column family
    fn put_json(&self, cf_name: &str, key: &str, value: &serde_json::Value) -> anyhow::Result<()> {
        let cf = self.cf(cf_name)?;
        self.db
            .put_cf(cf, key.as_bytes(), value.to_string().as_bytes())
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))
    }

    /// Delete a key from a column family
    fn delete_key(&self, cf_name: &str, key: &str) -> anyhow::Result<()> {
        let cf = self.cf(cf_name)?;
        self.db
            .delete_cf(cf, key.as_bytes())
            .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))
    }

    /// Compute MD5 hash of content
    fn compute_md5(content: &str) -> String {
        format!("{:x}", md5::compute(content.as_bytes()))
    }

    /// Convert a JSON value from RocksDB to ConfigStorageData
    pub fn json_to_config(v: &serde_json::Value) -> ConfigStorageData {
        ConfigStorageData {
            data_id: v["data_id"].as_str().unwrap_or("").to_string(),
            group: v["group"].as_str().unwrap_or("").to_string(),
            tenant: v["tenant"].as_str().unwrap_or("").to_string(),
            content: v["content"].as_str().unwrap_or("").to_string(),
            md5: v["md5"].as_str().unwrap_or("").to_string(),
            app_name: v["app_name"].as_str().unwrap_or("").to_string(),
            config_type: v["config_type"].as_str().unwrap_or("").to_string(),
            desc: v["desc"].as_str().unwrap_or("").to_string(),
            r#use: v["use"].as_str().unwrap_or("").to_string(),
            effect: v["effect"].as_str().unwrap_or("").to_string(),
            schema: v["schema"].as_str().unwrap_or("").to_string(),
            config_tags: v["config_tags"].as_str().unwrap_or("").to_string(),
            encrypted_data_key: v["encrypted_data_key"].as_str().unwrap_or("").to_string(),
            src_user: v["src_user"].as_str().unwrap_or("").to_string(),
            src_ip: v["src_ip"].as_str().unwrap_or("").to_string(),
            created_time: v["created_time"].as_i64().unwrap_or(0),
            modified_time: v["modified_time"].as_i64().unwrap_or(0),
        }
    }

    /// Convert a JSON value to ConfigHistoryStorageData
    pub fn json_to_history(v: &serde_json::Value) -> ConfigHistoryStorageData {
        ConfigHistoryStorageData {
            id: v["id"].as_u64().unwrap_or(0),
            data_id: v["data_id"].as_str().unwrap_or("").to_string(),
            group: v["group"].as_str().unwrap_or("").to_string(),
            tenant: v["tenant"].as_str().unwrap_or("").to_string(),
            content: v["content"].as_str().unwrap_or("").to_string(),
            md5: v["md5"].as_str().unwrap_or("").to_string(),
            app_name: v["app_name"].as_str().unwrap_or("").to_string(),
            src_user: v["src_user"].as_str().unwrap_or("").to_string(),
            src_ip: v["src_ip"].as_str().unwrap_or("").to_string(),
            op_type: v["op_type"].as_str().unwrap_or("").to_string(),
            publish_type: v["publish_type"].as_str().unwrap_or("").to_string(),
            gray_name: v["gray_name"].as_str().unwrap_or("").to_string(),
            ext_info: v["ext_info"].as_str().unwrap_or("").to_string(),
            encrypted_data_key: v["encrypted_data_key"].as_str().unwrap_or("").to_string(),
            created_time: v["created_time"].as_i64().unwrap_or(0),
            modified_time: v["modified_time"].as_i64().unwrap_or(0),
        }
    }

    /// Convert a JSON value to ConfigGrayStorageData
    pub fn json_to_gray(v: &serde_json::Value) -> ConfigGrayStorageData {
        ConfigGrayStorageData {
            data_id: v["data_id"].as_str().unwrap_or("").to_string(),
            group: v["group"].as_str().unwrap_or("").to_string(),
            tenant: v["tenant"].as_str().unwrap_or("").to_string(),
            content: v["content"].as_str().unwrap_or("").to_string(),
            md5: v["md5"].as_str().unwrap_or("").to_string(),
            app_name: v["app_name"].as_str().unwrap_or("").to_string(),
            gray_name: v["gray_name"].as_str().unwrap_or("").to_string(),
            gray_rule: v["gray_rule"].as_str().unwrap_or("").to_string(),
            encrypted_data_key: v["encrypted_data_key"].as_str().unwrap_or("").to_string(),
            src_user: v["src_user"].as_str().unwrap_or("").to_string(),
            src_ip: v["src_ip"].as_str().unwrap_or("").to_string(),
            created_time: v["created_time"].as_i64().unwrap_or(0),
            modified_time: v["modified_time"].as_i64().unwrap_or(0),
        }
    }

    /// Convert JSON to NamespaceInfo
    pub fn json_to_namespace(v: &serde_json::Value, config_count: i32) -> NamespaceInfo {
        NamespaceInfo {
            namespace_id: v["namespace_id"].as_str().unwrap_or("").to_string(),
            namespace_name: v["namespace_name"].as_str().unwrap_or("").to_string(),
            namespace_desc: v["namespace_desc"].as_str().unwrap_or("").to_string(),
            config_count,
            quota: 200,
        }
    }

    /// Convert JSON to UserInfo
    pub fn json_to_user(v: &serde_json::Value) -> UserInfo {
        UserInfo {
            username: v["username"].as_str().unwrap_or("").to_string(),
            password: v["password_hash"].as_str().unwrap_or("").to_string(),
            enabled: v["enabled"].as_bool().unwrap_or(true),
        }
    }

    /// Convert JSON to RoleInfo
    pub fn json_to_role(v: &serde_json::Value) -> RoleInfo {
        RoleInfo {
            role: v["role"].as_str().unwrap_or("").to_string(),
            username: v["username"].as_str().unwrap_or("").to_string(),
        }
    }

    /// Convert JSON to PermissionInfo
    pub fn json_to_permission(v: &serde_json::Value) -> PermissionInfo {
        PermissionInfo {
            role: v["role"].as_str().unwrap_or("").to_string(),
            resource: v["resource"].as_str().unwrap_or("").to_string(),
            action: v["action"].as_str().unwrap_or("").to_string(),
        }
    }

    /// Generate a unique history ID based on timestamp + counter
    fn generate_history_id() -> u64 {
        // Use timestamp in millis as a simple unique ID
        chrono::Utc::now().timestamp_millis() as u64
    }
}

#[async_trait]
impl ConfigPersistence for EmbeddedPersistService {
    async fn config_find_one(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigStorageData>> {
        let json = self.reader.get_config(data_id, group, namespace_id)?;
        Ok(json.as_ref().map(Self::json_to_config))
    }

    async fn config_search_page(
        &self,
        page_no: u64,
        page_size: u64,
        namespace_id: &str,
        data_id: &str,
        group_id: &str,
        app_name: &str,
        tags: Vec<String>,
        types: Vec<String>,
        content: &str,
    ) -> anyhow::Result<Page<ConfigStorageData>> {
        let (items, total) = self.reader.search_configs(
            namespace_id,
            data_id,
            group_id,
            app_name,
            &tags,
            &types,
            content,
            page_no,
            page_size,
        )?;

        let page_items = items.iter().map(Self::json_to_config).collect();
        Ok(Page::new(total, page_no, page_size, page_items))
    }

    async fn config_create_or_update(
        &self,
        data_id: &str,
        group_id: &str,
        tenant_id: &str,
        content: &str,
        app_name: &str,
        src_user: &str,
        src_ip: &str,
        config_tags: &str,
        desc: &str,
        r#use: &str,
        effect: &str,
        r#type: &str,
        schema: &str,
        encrypted_data_key: &str,
    ) -> anyhow::Result<bool> {
        let key = RocksStateMachine::config_key(data_id, group_id, tenant_id);
        let now = chrono::Utc::now().timestamp_millis();
        let md5_val = Self::compute_md5(content);

        // Check if config already exists
        let existing = self.reader.get_config(data_id, group_id, tenant_id)?;
        let is_update = existing.is_some();
        let created_time = if let Some(ref ex) = existing {
            ex["created_time"].as_i64().unwrap_or(now)
        } else {
            now
        };

        let value = serde_json::json!({
            "data_id": data_id,
            "group": group_id,
            "tenant": tenant_id,
            "content": content,
            "md5": md5_val,
            "app_name": app_name,
            "config_type": r#type,
            "desc": desc,
            "use": r#use,
            "effect": effect,
            "schema": schema,
            "config_tags": config_tags,
            "encrypted_data_key": encrypted_data_key,
            "src_user": src_user,
            "src_ip": src_ip,
            "created_time": created_time,
            "modified_time": now,
        });

        self.put_json(CF_CONFIG, &key, &value)?;

        // Insert history entry
        let history_id = Self::generate_history_id();
        let op_type = if is_update { "U" } else { "I" };
        let history_key =
            RocksStateMachine::config_history_key(data_id, group_id, tenant_id, history_id);
        let history_value = serde_json::json!({
            "id": history_id,
            "data_id": data_id,
            "group": group_id,
            "tenant": tenant_id,
            "content": content,
            "md5": md5_val,
            "app_name": app_name,
            "src_user": src_user,
            "src_ip": src_ip,
            "op_type": op_type,
            "publish_type": "",
            "gray_name": "",
            "ext_info": "",
            "encrypted_data_key": encrypted_data_key,
            "created_time": now,
            "modified_time": now,
        });
        self.put_json(CF_CONFIG_HISTORY, &history_key, &history_value)?;

        Ok(true)
    }

    async fn config_delete(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        _gray_name: &str,
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<bool> {
        let key = RocksStateMachine::config_key(data_id, group, namespace_id);

        // Get existing config for history
        let existing = self.reader.get_config(data_id, group, namespace_id)?;
        if existing.is_none() {
            return Ok(false);
        }
        let existing = existing.unwrap();

        // Delete the config
        self.delete_key(CF_CONFIG, &key)?;

        // Insert delete history
        let history_id = Self::generate_history_id();
        let now = chrono::Utc::now().timestamp_millis();
        let history_key =
            RocksStateMachine::config_history_key(data_id, group, namespace_id, history_id);
        let history_value = serde_json::json!({
            "id": history_id,
            "data_id": data_id,
            "group": group,
            "tenant": namespace_id,
            "content": existing["content"],
            "md5": existing["md5"],
            "app_name": existing["app_name"],
            "src_user": src_user,
            "src_ip": client_ip,
            "op_type": "D",
            "publish_type": "",
            "gray_name": "",
            "ext_info": "",
            "encrypted_data_key": existing["encrypted_data_key"],
            "created_time": now,
            "modified_time": now,
        });
        self.put_json(CF_CONFIG_HISTORY, &history_key, &history_value)?;

        Ok(true)
    }

    async fn config_find_gray_one(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigGrayStorageData>> {
        let json = self.reader.get_config_gray(data_id, group, namespace_id)?;
        Ok(json.as_ref().map(Self::json_to_gray))
    }

    async fn config_create_or_update_gray(
        &self,
        data_id: &str,
        group_id: &str,
        tenant_id: &str,
        content: &str,
        gray_name: &str,
        gray_rule: &str,
        src_user: &str,
        src_ip: &str,
        app_name: &str,
        encrypted_data_key: &str,
    ) -> anyhow::Result<bool> {
        let key = RocksStateMachine::config_gray_key(data_id, group_id, tenant_id, gray_name);
        let now = chrono::Utc::now().timestamp_millis();
        let md5_val = Self::compute_md5(content);

        let value = serde_json::json!({
            "data_id": data_id,
            "group": group_id,
            "tenant": tenant_id,
            "content": content,
            "md5": md5_val,
            "app_name": app_name,
            "gray_name": gray_name,
            "gray_rule": gray_rule,
            "encrypted_data_key": encrypted_data_key,
            "src_user": src_user,
            "src_ip": src_ip,
            "created_time": now,
            "modified_time": now,
        });

        self.put_json(CF_CONFIG_GRAY, &key, &value)?;
        Ok(true)
    }

    async fn config_delete_gray(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        _client_ip: &str,
        _src_user: &str,
    ) -> anyhow::Result<bool> {
        // Scan and delete all gray configs for this data_id/group/namespace
        let prefix = format!("{}@@{}@@{}@@", namespace_id, group, data_id);
        let cf = self.cf(CF_CONFIG_GRAY)?;
        let mut keys_to_delete = Vec::new();

        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, _) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            keys_to_delete.push(key.to_vec());
        }

        if keys_to_delete.is_empty() {
            return Ok(false);
        }

        let mut batch = rocksdb::WriteBatch::default();
        for key in &keys_to_delete {
            batch.delete_cf(cf, key);
        }
        self.db
            .write(batch)
            .map_err(|e| anyhow::anyhow!("RocksDB batch delete error: {}", e))?;

        Ok(true)
    }

    async fn config_batch_delete(
        &self,
        _ids: &[i64],
        _client_ip: &str,
        _src_user: &str,
    ) -> anyhow::Result<usize> {
        // RocksDB configs are keyed by tenant@@group@@data_id, not by integer IDs.
        // Batch delete by SQL integer ID is not applicable in embedded mode.
        Err(anyhow::anyhow!(
            "Batch delete by integer ID is not supported in standalone embedded mode"
        ))
    }

    async fn config_history_find_by_id(
        &self,
        nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryStorageData>> {
        let json = self.reader.get_config_history_by_id(nid)?;
        Ok(json.as_ref().map(Self::json_to_history))
    }

    async fn config_history_search_page(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryStorageData>> {
        let (items, total) =
            self.reader
                .search_config_history(data_id, group, namespace_id, page_no, page_size)?;

        let page_items = items.iter().map(Self::json_to_history).collect();
        Ok(Page::new(total, page_no, page_size, page_items))
    }

    async fn config_count_by_namespace(&self, namespace_id: &str) -> anyhow::Result<i32> {
        self.reader.count_configs(namespace_id)
    }

    async fn config_find_all_group_names(&self, namespace_id: &str) -> anyhow::Result<Vec<String>> {
        self.reader.find_all_group_names(namespace_id)
    }

    async fn config_history_get_previous(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        current_nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryStorageData>> {
        let prefix = format!("{}@@{}@@{}@@", namespace_id, group, data_id);
        let cf = self.cf(CF_CONFIG_HISTORY)?;

        let mut best: Option<serde_json::Value> = None;
        let mut best_id: u64 = 0;

        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            let id = json["id"].as_u64().unwrap_or(0);
            if id < current_nid && id > best_id {
                best_id = id;
                best = Some(json);
            }
        }

        Ok(best.as_ref().map(Self::json_to_history))
    }

    async fn config_history_search_with_filters(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        op_type: Option<&str>,
        src_user: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryStorageData>> {
        // Build prefix for history entries matching data_id/group/namespace
        let prefix = format!("{}@@{}@@{}@@", namespace_id, group, data_id);
        let cf = self.cf(CF_CONFIG_HISTORY)?;

        let mut all_items = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            let json: serde_json::Value = serde_json::from_slice(&value)?;

            // Apply filters
            if let Some(op) = op_type {
                let entry_op = json["op_type"].as_str().unwrap_or("");
                if entry_op != op {
                    continue;
                }
            }
            if let Some(user) = src_user {
                let entry_user = json["src_user"].as_str().unwrap_or("");
                if !entry_user.contains(user) {
                    continue;
                }
            }
            if let Some(start) = start_time {
                let entry_time = json["modified_time"].as_i64().unwrap_or(0);
                if entry_time < start {
                    continue;
                }
            }
            if let Some(end) = end_time {
                let entry_time = json["modified_time"].as_i64().unwrap_or(0);
                if entry_time > end {
                    continue;
                }
            }

            all_items.push(json);
        }

        // Sort by id descending (newest first)
        all_items.sort_by(|a, b| {
            let id_a = a["id"].as_u64().unwrap_or(0);
            let id_b = b["id"].as_u64().unwrap_or(0);
            id_b.cmp(&id_a)
        });

        let total = all_items.len() as u64;
        let offset = (page_no.saturating_sub(1)) * page_size;
        let page_items: Vec<ConfigHistoryStorageData> = all_items
            .iter()
            .skip(offset as usize)
            .take(page_size as usize)
            .map(Self::json_to_history)
            .collect();

        Ok(Page::new(total, page_no, page_size, page_items))
    }

    async fn config_find_by_namespace(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigStorageData>> {
        let prefix = format!("{}@@", namespace_id);
        let cf = self.cf(CF_CONFIG)?;
        let mut results = Vec::new();

        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            results.push(Self::json_to_config(&json));
        }

        Ok(results)
    }

    async fn config_find_for_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<Vec<String>>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<ConfigStorageData>> {
        let all_configs = self.reader.list_configs(namespace_id)?;
        let mut results = Vec::new();

        for json in &all_configs {
            // Filter by group if provided
            if let Some(g) = group {
                let config_group = json["group"].as_str().unwrap_or("");
                if config_group != g {
                    continue;
                }
            }

            // Filter by data_ids if provided
            if let Some(ref ids) = data_ids {
                let config_data_id = json["data_id"].as_str().unwrap_or("");
                if !ids.iter().any(|id| id == config_data_id) {
                    continue;
                }
            }

            // Filter by app_name if provided
            if let Some(app) = app_name {
                let config_app_name = json["app_name"].as_str().unwrap_or("");
                if config_app_name != app {
                    continue;
                }
            }

            results.push(Self::json_to_config(json));
        }

        Ok(results)
    }
}

#[async_trait]
impl NamespacePersistence for EmbeddedPersistService {
    async fn namespace_find_all(&self) -> anyhow::Result<Vec<NamespaceInfo>> {
        let namespaces = self.reader.list_namespaces()?;
        let mut result = Vec::with_capacity(namespaces.len());

        for ns in &namespaces {
            let ns_id = ns["namespace_id"].as_str().unwrap_or("");
            let config_count = self.reader.count_configs(ns_id)?;
            result.push(Self::json_to_namespace(ns, config_count));
        }

        Ok(result)
    }

    async fn namespace_get_by_id(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Option<NamespaceInfo>> {
        let json = self.reader.get_namespace(namespace_id)?;
        match json {
            Some(ref v) => {
                let config_count = self.reader.count_configs(namespace_id)?;
                Ok(Some(Self::json_to_namespace(v, config_count)))
            }
            None => Ok(None),
        }
    }

    async fn namespace_create(
        &self,
        namespace_id: &str,
        name: &str,
        desc: &str,
    ) -> anyhow::Result<()> {
        let key = RocksStateMachine::namespace_key(namespace_id);
        let now = chrono::Utc::now().timestamp_millis();

        let value = serde_json::json!({
            "namespace_id": namespace_id,
            "namespace_name": name,
            "namespace_desc": desc,
            "created_time": now,
        });

        self.put_json(CF_NAMESPACE, &key, &value)
    }

    async fn namespace_update(
        &self,
        namespace_id: &str,
        name: &str,
        desc: &str,
    ) -> anyhow::Result<bool> {
        let key = RocksStateMachine::namespace_key(namespace_id);

        // Check existence
        if self.reader.get_namespace(namespace_id)?.is_none() {
            return Ok(false);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let value = serde_json::json!({
            "namespace_id": namespace_id,
            "namespace_name": name,
            "namespace_desc": desc,
            "modified_time": now,
        });

        self.put_json(CF_NAMESPACE, &key, &value)?;
        Ok(true)
    }

    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool> {
        let key = RocksStateMachine::namespace_key(namespace_id);

        if self.reader.get_namespace(namespace_id)?.is_none() {
            return Ok(false);
        }

        self.delete_key(CF_NAMESPACE, &key)?;
        Ok(true)
    }

    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool> {
        Ok(self.reader.get_namespace(namespace_id)?.is_some())
    }
}

#[async_trait]
impl AuthPersistence for EmbeddedPersistService {
    async fn user_find_by_username(&self, username: &str) -> anyhow::Result<Option<UserInfo>> {
        let json = self.reader.get_user(username)?;
        Ok(json.as_ref().map(Self::json_to_user))
    }

    async fn user_find_page(
        &self,
        username: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<UserInfo>> {
        let (items, total) = self
            .reader
            .search_users(username, accurate, page_no, page_size)?;
        let page_items = items.iter().map(Self::json_to_user).collect();
        Ok(Page::new(total, page_no, page_size, page_items))
    }

    async fn user_create(
        &self,
        username: &str,
        password_hash: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
        let key = RocksStateMachine::user_key(username);
        let now = chrono::Utc::now().timestamp_millis();

        let value = serde_json::json!({
            "username": username,
            "password_hash": password_hash,
            "enabled": enabled,
            "created_time": now,
        });

        self.put_json(CF_USERS, &key, &value)
    }

    async fn user_update_password(
        &self,
        username: &str,
        password_hash: &str,
    ) -> anyhow::Result<()> {
        let key = RocksStateMachine::user_key(username);
        let now = chrono::Utc::now().timestamp_millis();

        let existing = self
            .reader
            .get_user(username)?
            .ok_or_else(|| anyhow::anyhow!("User not found: {}", username))?;

        let mut existing = existing;
        existing["password_hash"] = serde_json::json!(password_hash);
        existing["modified_time"] = serde_json::json!(now);

        self.put_json(CF_USERS, &key, &existing)
    }

    async fn user_delete(&self, username: &str) -> anyhow::Result<()> {
        let key = RocksStateMachine::user_key(username);
        self.delete_key(CF_USERS, &key)
    }

    async fn user_search(&self, username: &str) -> anyhow::Result<Vec<String>> {
        let cf = self.cf(CF_USERS)?;
        let mut results = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let json: serde_json::Value = serde_json::from_slice(&value)?;
            let u = json["username"].as_str().unwrap_or("");
            if username.is_empty() || u.contains(username) {
                results.push(u.to_string());
            }
        }

        Ok(results)
    }

    async fn role_find_by_username(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        let items = self.reader.get_roles_by_username(username)?;
        Ok(items.iter().map(Self::json_to_role).collect())
    }

    async fn role_find_page(
        &self,
        username: &str,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<RoleInfo>> {
        let (items, total) = self
            .reader
            .search_roles(username, role, accurate, page_no, page_size)?;
        let page_items = items.iter().map(Self::json_to_role).collect();
        Ok(Page::new(total, page_no, page_size, page_items))
    }

    async fn role_create(&self, role: &str, username: &str) -> anyhow::Result<()> {
        let key = RocksStateMachine::role_key(role, username);
        let now = chrono::Utc::now().timestamp_millis();

        let value = serde_json::json!({
            "role": role,
            "username": username,
            "created_time": now,
        });

        self.put_json(CF_ROLES, &key, &value)
    }

    async fn role_delete(&self, role: &str, username: &str) -> anyhow::Result<()> {
        let key = RocksStateMachine::role_key(role, username);
        self.delete_key(CF_ROLES, &key)
    }

    async fn role_has_global_admin(&self) -> anyhow::Result<bool> {
        self.reader.has_global_admin()
    }

    async fn role_has_global_admin_by_username(&self, username: &str) -> anyhow::Result<bool> {
        self.reader.has_global_admin_by_username(username)
    }

    async fn role_search(&self, role: &str) -> anyhow::Result<Vec<String>> {
        // Use search_roles with empty username pattern, non-accurate, and large page to get all matches
        let (items, _total) = self.reader.search_roles("", role, false, 1, u64::MAX)?;
        let role_names: Vec<String> = items
            .iter()
            .filter_map(|v| {
                v.get("role")
                    .and_then(|r| r.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        Ok(role_names)
    }

    async fn permission_find_by_role(&self, role: &str) -> anyhow::Result<Vec<PermissionInfo>> {
        let items = self.reader.get_permissions_by_role(role)?;
        Ok(items.iter().map(Self::json_to_permission).collect())
    }

    async fn permission_find_by_roles(
        &self,
        roles: Vec<String>,
    ) -> anyhow::Result<Vec<PermissionInfo>> {
        let items = self.reader.get_permissions_by_roles(&roles)?;
        Ok(items.iter().map(Self::json_to_permission).collect())
    }

    async fn permission_find_page(
        &self,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<PermissionInfo>> {
        let (items, total) = self
            .reader
            .search_permissions(role, accurate, page_no, page_size)?;
        let page_items = items.iter().map(Self::json_to_permission).collect();
        Ok(Page::new(total, page_no, page_size, page_items))
    }

    async fn permission_find_by_id(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<Option<PermissionInfo>> {
        let key = RocksStateMachine::permission_key(role, resource, action);
        let cf = self.cf(CF_PERMISSIONS)?;

        match self.db.get_cf(cf, key.as_bytes())? {
            Some(data) => {
                let v: serde_json::Value = serde_json::from_slice(&data)?;
                Ok(Some(Self::json_to_permission(&v)))
            }
            None => Ok(None),
        }
    }

    async fn permission_grant(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        let key = RocksStateMachine::permission_key(role, resource, action);
        let now = chrono::Utc::now().timestamp_millis();

        let value = serde_json::json!({
            "role": role,
            "resource": resource,
            "action": action,
            "created_time": now,
        });

        self.put_json(CF_PERMISSIONS, &key, &value)
    }

    async fn permission_revoke(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        let key = RocksStateMachine::permission_key(role, resource, action);
        self.delete_key(CF_PERMISSIONS, &key)
    }
}

#[async_trait]
impl CapacityPersistence for EmbeddedPersistService {
    async fn capacity_get_tenant(&self, tenant_id: &str) -> anyhow::Result<Option<CapacityInfo>> {
        let key = format!("capacity:tenant:{}", tenant_id);
        match self.db.get(key.as_bytes())? {
            Some(data) => {
                let info: CapacityInfo = serde_json::from_slice(&data)?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn capacity_upsert_tenant(
        &self,
        tenant_id: &str,
        quota: Option<u32>,
        max_size: Option<u32>,
        max_aggr_count: Option<u32>,
        max_aggr_size: Option<u32>,
        max_history_count: Option<u32>,
    ) -> anyhow::Result<CapacityInfo> {
        let key = format!("capacity:tenant:{}", tenant_id);

        // Read existing or create defaults
        let mut info = match self.db.get(key.as_bytes())? {
            Some(data) => serde_json::from_slice::<CapacityInfo>(&data)?,
            None => CapacityInfo {
                id: None,
                identifier: tenant_id.to_string(),
                quota: 200,
                usage: 0,
                max_size: 102400,
                max_aggr_count: 10000,
                max_aggr_size: 2097152,
                max_history_count: 24,
            },
        };

        // Merge provided fields
        if let Some(v) = quota {
            info.quota = v;
        }
        if let Some(v) = max_size {
            info.max_size = v;
        }
        if let Some(v) = max_aggr_count {
            info.max_aggr_count = v;
        }
        if let Some(v) = max_aggr_size {
            info.max_aggr_size = v;
        }
        if let Some(v) = max_history_count {
            info.max_history_count = v;
        }

        // Compute usage by counting configs in this namespace
        info.usage = self.reader.count_configs(tenant_id)? as u32;

        // Write back
        let json = serde_json::to_vec(&info)?;
        self.db.put(key.as_bytes(), &json)?;

        Ok(info)
    }

    async fn capacity_delete_tenant(&self, tenant_id: &str) -> anyhow::Result<bool> {
        let key = format!("capacity:tenant:{}", tenant_id);
        let exists = self.db.get(key.as_bytes())?.is_some();
        if exists {
            self.db.delete(key.as_bytes())?;
        }
        Ok(exists)
    }

    async fn capacity_get_group(&self, group_id: &str) -> anyhow::Result<Option<CapacityInfo>> {
        let key = format!("capacity:group:{}", group_id);
        match self.db.get(key.as_bytes())? {
            Some(data) => {
                let info: CapacityInfo = serde_json::from_slice(&data)?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn capacity_upsert_group(
        &self,
        group_id: &str,
        quota: Option<u32>,
        max_size: Option<u32>,
        max_aggr_count: Option<u32>,
        max_aggr_size: Option<u32>,
        max_history_count: Option<u32>,
    ) -> anyhow::Result<CapacityInfo> {
        let key = format!("capacity:group:{}", group_id);

        // Read existing or create defaults
        let mut info = match self.db.get(key.as_bytes())? {
            Some(data) => serde_json::from_slice::<CapacityInfo>(&data)?,
            None => CapacityInfo {
                id: None,
                identifier: group_id.to_string(),
                quota: 200,
                usage: 0,
                max_size: 102400,
                max_aggr_count: 10000,
                max_aggr_size: 2097152,
                max_history_count: 24,
            },
        };

        // Merge provided fields
        if let Some(v) = quota {
            info.quota = v;
        }
        if let Some(v) = max_size {
            info.max_size = v;
        }
        if let Some(v) = max_aggr_count {
            info.max_aggr_count = v;
        }
        if let Some(v) = max_aggr_size {
            info.max_aggr_size = v;
        }
        if let Some(v) = max_history_count {
            info.max_history_count = v;
        }

        // Write back (group capacity uses the stored usage value, no recount)
        let json = serde_json::to_vec(&info)?;
        self.db.put(key.as_bytes(), &json)?;

        Ok(info)
    }

    async fn capacity_delete_group(&self, group_id: &str) -> anyhow::Result<bool> {
        let key = format!("capacity:group:{}", group_id);
        let exists = self.db.get(key.as_bytes())?.is_some();
        if exists {
            self.db.delete(key.as_bytes())?;
        }
        Ok(exists)
    }
}

#[async_trait]
impl PersistenceService for EmbeddedPersistService {
    fn storage_mode(&self) -> StorageMode {
        StorageMode::StandaloneEmbedded
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        // Verify we can access column families
        self.cf(CF_CONFIG)?;
        self.cf(CF_NAMESPACE)?;
        self.cf(CF_USERS)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_service() -> (EmbeddedPersistService, TempDir) {
        let tmp_dir = TempDir::new().unwrap();
        let sm = RocksStateMachine::new(tmp_dir.path()).await.unwrap();
        let service = EmbeddedPersistService::from_state_machine(&sm);
        (service, tmp_dir)
    }

    // ==================== Config Tests ====================

    #[tokio::test]
    async fn test_config_create_and_find() {
        let (svc, _tmp) = create_test_service().await;

        // Create a config
        let result = svc
            .config_create_or_update(
                "app.properties",
                "DEFAULT_GROUP",
                "public",
                "server.port=8080",
                "my-app",
                "admin",
                "127.0.0.1",
                "",
                "Main config",
                "",
                "",
                "properties",
                "",
                "",
            )
            .await
            .unwrap();
        assert!(result);

        // Find the config
        let found = svc
            .config_find_one("app.properties", "DEFAULT_GROUP", "public")
            .await
            .unwrap();
        assert!(found.is_some());
        let config = found.unwrap();
        assert_eq!(config.data_id, "app.properties");
        assert_eq!(config.group, "DEFAULT_GROUP");
        assert_eq!(config.tenant, "public");
        assert_eq!(config.content, "server.port=8080");
        assert_eq!(config.config_type, "properties");
        assert_eq!(config.app_name, "my-app");
        assert_eq!(config.desc, "Main config");
        assert!(!config.md5.is_empty());

        // Not found
        let not_found = svc
            .config_find_one("nonexistent", "DEFAULT_GROUP", "public")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_config_update_preserves_created_time() {
        let (svc, _tmp) = create_test_service().await;

        // Create
        svc.config_create_or_update(
            "test",
            "G",
            "T",
            "v1",
            "",
            "admin",
            "127.0.0.1",
            "",
            "",
            "",
            "",
            "text",
            "",
            "",
        )
        .await
        .unwrap();

        let first = svc
            .config_find_one("test", "G", "T")
            .await
            .unwrap()
            .unwrap();
        let created_time = first.created_time;
        assert!(created_time > 0);

        // Update
        svc.config_create_or_update(
            "test",
            "G",
            "T",
            "v2",
            "",
            "admin",
            "127.0.0.1",
            "",
            "",
            "",
            "",
            "text",
            "",
            "",
        )
        .await
        .unwrap();

        let second = svc
            .config_find_one("test", "G", "T")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.content, "v2");
        assert_eq!(second.created_time, created_time); // created_time preserved
        assert!(second.modified_time >= first.modified_time);
    }

    #[tokio::test]
    async fn test_config_delete() {
        let (svc, _tmp) = create_test_service().await;

        // Create
        svc.config_create_or_update(
            "del-test",
            "G",
            "T",
            "content",
            "",
            "admin",
            "127.0.0.1",
            "",
            "",
            "",
            "",
            "text",
            "",
            "",
        )
        .await
        .unwrap();

        // Delete
        let deleted = svc
            .config_delete("del-test", "G", "T", "", "127.0.0.1", "admin")
            .await
            .unwrap();
        assert!(deleted);

        // Verify gone
        let found = svc.config_find_one("del-test", "G", "T").await.unwrap();
        assert!(found.is_none());

        // Delete non-existent returns false
        let deleted_again = svc
            .config_delete("del-test", "G", "T", "", "127.0.0.1", "admin")
            .await
            .unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_config_search_page() {
        let (svc, _tmp) = create_test_service().await;

        // Create multiple configs
        for i in 0..5 {
            svc.config_create_or_update(
                &format!("data-{}", i),
                "DEFAULT_GROUP",
                "public",
                &format!("content-{}", i),
                "my-app",
                "admin",
                "127.0.0.1",
                "",
                "",
                "",
                "",
                "properties",
                "",
                "",
            )
            .await
            .unwrap();
        }

        // Search all
        let page = svc
            .config_search_page(1, 10, "public", "", "", "", vec![], vec![], "")
            .await
            .unwrap();
        assert_eq!(page.total_count, 5);
        assert_eq!(page.page_items.len(), 5);

        // Search with data_id pattern
        let page = svc
            .config_search_page(1, 10, "public", "data-3", "", "", vec![], vec![], "")
            .await
            .unwrap();
        assert_eq!(page.total_count, 1);
        assert_eq!(page.page_items[0].data_id, "data-3");

        // Search with pagination
        let page = svc
            .config_search_page(1, 2, "public", "", "", "", vec![], vec![], "")
            .await
            .unwrap();
        assert_eq!(page.total_count, 5);
        assert_eq!(page.page_items.len(), 2);

        // Page 2
        let page2 = svc
            .config_search_page(2, 2, "public", "", "", "", vec![], vec![], "")
            .await
            .unwrap();
        assert_eq!(page2.total_count, 5);
        assert_eq!(page2.page_items.len(), 2);
    }

    #[tokio::test]
    async fn test_config_count_and_group_names() {
        let (svc, _tmp) = create_test_service().await;

        svc.config_create_or_update(
            "d1", "GROUP_A", "ns1", "c1", "", "", "", "", "", "", "", "text", "", "",
        )
        .await
        .unwrap();
        svc.config_create_or_update(
            "d2", "GROUP_A", "ns1", "c2", "", "", "", "", "", "", "", "text", "", "",
        )
        .await
        .unwrap();
        svc.config_create_or_update(
            "d3", "GROUP_B", "ns1", "c3", "", "", "", "", "", "", "", "text", "", "",
        )
        .await
        .unwrap();

        assert_eq!(svc.config_count_by_namespace("ns1").await.unwrap(), 3);
        assert_eq!(svc.config_count_by_namespace("ns2").await.unwrap(), 0);

        let mut groups = svc.config_find_all_group_names("ns1").await.unwrap();
        groups.sort();
        assert_eq!(groups, vec!["GROUP_A", "GROUP_B"]);
    }

    // ==================== Config History Tests ====================

    #[tokio::test]
    async fn test_config_history_tracking() {
        let (svc, _tmp) = create_test_service().await;

        // Create config (generates history entry with op_type "I")
        svc.config_create_or_update(
            "hist-test",
            "G",
            "T",
            "v1",
            "",
            "admin",
            "127.0.0.1",
            "",
            "",
            "",
            "",
            "text",
            "",
            "",
        )
        .await
        .unwrap();

        // Update config (generates history entry with op_type "U")
        // Small delay to ensure different timestamp
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        svc.config_create_or_update(
            "hist-test",
            "G",
            "T",
            "v2",
            "",
            "admin",
            "127.0.0.1",
            "",
            "",
            "",
            "",
            "text",
            "",
            "",
        )
        .await
        .unwrap();

        // Delete config (generates history entry with op_type "D")
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        svc.config_delete("hist-test", "G", "T", "", "127.0.0.1", "admin")
            .await
            .unwrap();

        // Search history
        let page = svc
            .config_history_search_page("hist-test", "G", "T", 1, 10)
            .await
            .unwrap();
        assert_eq!(page.total_count, 3);

        // Verify order (most recent first)
        assert_eq!(page.page_items[0].op_type, "D");
        assert_eq!(page.page_items[1].op_type, "U");
        assert_eq!(page.page_items[2].op_type, "I");

        // Find by ID
        let history_id = page.page_items[0].id;
        let found = svc.config_history_find_by_id(history_id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().op_type, "D");
    }

    // ==================== Config Gray Tests ====================

    #[tokio::test]
    async fn test_config_gray_crud() {
        let (svc, _tmp) = create_test_service().await;

        // Create gray config
        let result = svc
            .config_create_or_update_gray(
                "gray-test",
                "G",
                "T",
                "gray content",
                "gray-1",
                r#"{"type":"tag","expr":"tag=gray"}"#,
                "admin",
                "127.0.0.1",
                "my-app",
                "",
            )
            .await
            .unwrap();
        assert!(result);

        // Find gray config
        let found = svc
            .config_find_gray_one("gray-test", "G", "T")
            .await
            .unwrap();
        assert!(found.is_some());
        let gray = found.unwrap();
        assert_eq!(gray.content, "gray content");
        assert_eq!(gray.gray_name, "gray-1");

        // Delete gray config
        let deleted = svc
            .config_delete_gray("gray-test", "G", "T", "127.0.0.1", "admin")
            .await
            .unwrap();
        assert!(deleted);

        // Verify gone
        let found = svc
            .config_find_gray_one("gray-test", "G", "T")
            .await
            .unwrap();
        assert!(found.is_none());
    }

    // ==================== Namespace Tests ====================

    #[tokio::test]
    async fn test_namespace_crud() {
        let (svc, _tmp) = create_test_service().await;

        // Create namespace
        svc.namespace_create("dev", "Development", "Dev environment")
            .await
            .unwrap();

        // Check existence
        assert!(svc.namespace_check("dev").await.unwrap());
        assert!(!svc.namespace_check("nonexistent").await.unwrap());

        // Get by ID
        let ns = svc.namespace_get_by_id("dev").await.unwrap().unwrap();
        assert_eq!(ns.namespace_id, "dev");
        assert_eq!(ns.namespace_name, "Development");
        assert_eq!(ns.namespace_desc, "Dev environment");
        assert_eq!(ns.config_count, 0);

        // Update
        let updated = svc
            .namespace_update("dev", "Dev Updated", "Updated desc")
            .await
            .unwrap();
        assert!(updated);

        let ns = svc.namespace_get_by_id("dev").await.unwrap().unwrap();
        assert_eq!(ns.namespace_name, "Dev Updated");

        // Update non-existent
        let updated = svc.namespace_update("nonexistent", "X", "Y").await.unwrap();
        assert!(!updated);

        // List all
        svc.namespace_create("prod", "Production", "Prod environment")
            .await
            .unwrap();
        let all = svc.namespace_find_all().await.unwrap();
        assert_eq!(all.len(), 2);

        // Delete
        let deleted = svc.namespace_delete("dev").await.unwrap();
        assert!(deleted);
        assert!(!svc.namespace_check("dev").await.unwrap());

        // Delete non-existent
        let deleted = svc.namespace_delete("dev").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_namespace_config_count() {
        let (svc, _tmp) = create_test_service().await;

        svc.namespace_create("ns1", "NS1", "").await.unwrap();

        // Add configs to namespace
        svc.config_create_or_update(
            "d1", "G", "ns1", "c", "", "", "", "", "", "", "", "text", "", "",
        )
        .await
        .unwrap();
        svc.config_create_or_update(
            "d2", "G", "ns1", "c", "", "", "", "", "", "", "", "text", "", "",
        )
        .await
        .unwrap();

        let ns = svc.namespace_get_by_id("ns1").await.unwrap().unwrap();
        assert_eq!(ns.config_count, 2);
    }

    // ==================== Auth: User Tests ====================

    #[tokio::test]
    async fn test_user_crud() {
        let (svc, _tmp) = create_test_service().await;

        // Create user
        svc.user_create("alice", "hash123", true).await.unwrap();

        // Find by username
        let user = svc.user_find_by_username("alice").await.unwrap().unwrap();
        assert_eq!(user.username, "alice");
        assert_eq!(user.password, "hash123");
        assert!(user.enabled);

        // Not found
        let not_found = svc.user_find_by_username("bob").await.unwrap();
        assert!(not_found.is_none());

        // Update password
        svc.user_update_password("alice", "newhash").await.unwrap();
        let user = svc.user_find_by_username("alice").await.unwrap().unwrap();
        assert_eq!(user.password, "newhash");

        // Delete
        svc.user_delete("alice").await.unwrap();
        let not_found = svc.user_find_by_username("alice").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_user_find_page() {
        let (svc, _tmp) = create_test_service().await;

        svc.user_create("alice", "h", true).await.unwrap();
        svc.user_create("alice_admin", "h", true).await.unwrap();
        svc.user_create("bob", "h", true).await.unwrap();

        // Search with fuzzy match
        let page = svc.user_find_page("alice", 1, 10, false).await.unwrap();
        assert_eq!(page.total_count, 2);

        // Accurate match
        let page = svc.user_find_page("alice", 1, 10, true).await.unwrap();
        assert_eq!(page.total_count, 1);
        assert_eq!(page.page_items[0].username, "alice");

        // All users
        let page = svc.user_find_page("", 1, 10, false).await.unwrap();
        assert_eq!(page.total_count, 3);

        // Pagination
        let page = svc.user_find_page("", 1, 2, false).await.unwrap();
        assert_eq!(page.total_count, 3);
        assert_eq!(page.page_items.len(), 2);
    }

    // ==================== Auth: Role Tests ====================

    #[tokio::test]
    async fn test_role_crud() {
        let (svc, _tmp) = create_test_service().await;

        // Create roles
        svc.role_create("ROLE_ADMIN", "admin").await.unwrap();
        svc.role_create("ROLE_USER", "alice").await.unwrap();

        // Find by username
        let roles = svc.role_find_by_username("admin").await.unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].role, "ROLE_ADMIN");

        // Global admin check
        assert!(svc.role_has_global_admin().await.unwrap());
        assert!(
            svc.role_has_global_admin_by_username("admin")
                .await
                .unwrap()
        );
        assert!(
            !svc.role_has_global_admin_by_username("alice")
                .await
                .unwrap()
        );

        // Search roles
        let page = svc.role_find_page("", "", 1, 10, false).await.unwrap();
        assert_eq!(page.total_count, 2);

        // Delete
        svc.role_delete("ROLE_ADMIN", "admin").await.unwrap();
        assert!(!svc.role_has_global_admin().await.unwrap());
    }

    // ==================== Auth: Permission Tests ====================

    #[tokio::test]
    async fn test_permission_crud() {
        let (svc, _tmp) = create_test_service().await;

        // Grant permissions
        svc.permission_grant("ROLE_ADMIN", "*:*:*", "rw")
            .await
            .unwrap();
        svc.permission_grant("ROLE_USER", "public:*:config/*", "r")
            .await
            .unwrap();

        // Find by role
        let perms = svc.permission_find_by_role("ROLE_ADMIN").await.unwrap();
        assert_eq!(perms.len(), 1);
        assert_eq!(perms[0].resource, "*:*:*");
        assert_eq!(perms[0].action, "rw");

        // Find by multiple roles
        let perms = svc
            .permission_find_by_roles(vec!["ROLE_ADMIN".to_string(), "ROLE_USER".to_string()])
            .await
            .unwrap();
        assert_eq!(perms.len(), 2);

        // Search permissions
        let page = svc
            .permission_find_page("ROLE_ADMIN", 1, 10, true)
            .await
            .unwrap();
        assert_eq!(page.total_count, 1);

        // Revoke
        svc.permission_revoke("ROLE_ADMIN", "*:*:*", "rw")
            .await
            .unwrap();
        let perms = svc.permission_find_by_role("ROLE_ADMIN").await.unwrap();
        assert_eq!(perms.len(), 0);
    }

    // ==================== Health Check & Storage Mode ====================

    #[tokio::test]
    async fn test_health_check_and_storage_mode() {
        let (svc, _tmp) = create_test_service().await;

        assert_eq!(svc.storage_mode(), StorageMode::StandaloneEmbedded);
        assert!(svc.health_check().await.is_ok());
    }
}
