// Distributed persistence backend using Raft + RocksDB
// Provides multi-node consistency through Raft consensus

use std::sync::Arc;

use async_trait::async_trait;

use batata_consistency::raft::node::RaftNode;
use batata_consistency::raft::reader::RocksDbReader;
use batata_consistency::raft::request::RaftRequest;

use crate::model::{
    CapacityInfo, ConfigGrayStorageData, ConfigHistoryStorageData, ConfigStorageData,
    NamespaceInfo, Page, PermissionInfo, RoleInfo, StorageMode, UserInfo,
};
use crate::traits::PersistenceService;
use crate::traits::auth::AuthPersistence;
use crate::traits::capacity::CapacityPersistence;
use crate::traits::config::ConfigPersistence;
use crate::traits::namespace::NamespacePersistence;

// Re-use conversion helpers from embedded backend
use crate::embedded::EmbeddedPersistService;

/// Distributed persistence using Raft consensus + RocksDB
///
/// - Writes go through Raft consensus (RaftNode.write())
/// - Reads use linearizable_read() then read from local RocksDB
pub struct DistributedPersistService {
    reader: RocksDbReader,
    raft_node: Arc<RaftNode>,
}

impl DistributedPersistService {
    /// Create from a RaftNode
    ///
    /// Note: The caller must ensure the RaftNode's state machine uses the same
    /// RocksDB instance that the reader will query.
    pub fn new(raft_node: Arc<RaftNode>, reader: RocksDbReader) -> Self {
        Self { reader, raft_node }
    }

    /// Ensure linearizable read consistency
    async fn ensure_consistent_read(&self) -> anyhow::Result<()> {
        self.raft_node
            .linearizable_read()
            .await
            .map_err(|e| anyhow::anyhow!("Linearizable read failed: {}", e))
    }

    /// Write through Raft consensus
    async fn raft_write(&self, request: RaftRequest) -> anyhow::Result<()> {
        let response = self
            .raft_node
            .write(request)
            .await
            .map_err(|e| anyhow::anyhow!("Raft write failed: {}", e))?;

        if response.success {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Raft write rejected: {}",
                response.message.unwrap_or_default()
            ))
        }
    }

    /// Compute MD5 hash
    fn compute_md5(content: &str) -> String {
        format!("{:x}", md5::compute(content.as_bytes()))
    }
}

#[async_trait]
impl ConfigPersistence for DistributedPersistService {
    async fn config_find_one(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigStorageData>> {
        self.ensure_consistent_read().await?;
        let json = self.reader.get_config(data_id, group, namespace_id)?;
        Ok(json.as_ref().map(EmbeddedPersistService::json_to_config))
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
        self.ensure_consistent_read().await?;
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

        let page_items = items
            .iter()
            .map(EmbeddedPersistService::json_to_config)
            .collect();
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
        let _ = (r#use, effect, schema, encrypted_data_key);
        let request = RaftRequest::ConfigPublish {
            data_id: data_id.to_string(),
            group: group_id.to_string(),
            tenant: tenant_id.to_string(),
            content: content.to_string(),
            config_type: Some(r#type.to_string()),
            app_name: Some(app_name.to_string()),
            tag: if config_tags.is_empty() {
                None
            } else {
                Some(config_tags.to_string())
            },
            desc: Some(desc.to_string()),
            src_user: Some(src_user.to_string()),
        };

        self.raft_write(request).await?;

        // Also insert history through Raft
        let md5_val = Self::compute_md5(content);
        let now = chrono::Utc::now().timestamp_millis();
        let history_request = RaftRequest::ConfigHistoryInsert {
            id: now,
            data_id: data_id.to_string(),
            group: group_id.to_string(),
            tenant: tenant_id.to_string(),
            content: content.to_string(),
            md5: md5_val,
            src_user: Some(src_user.to_string()),
            src_ip: Some(src_ip.to_string()),
            op_type: "I".to_string(), // simplified: always "I" for now
            created_time: now,
            last_modified_time: now,
        };
        // History insert is best-effort
        let _ = self.raft_write(history_request).await;

        Ok(true)
    }

    async fn config_delete(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        _gray_name: &str,
        _client_ip: &str,
        _src_user: &str,
    ) -> anyhow::Result<bool> {
        let request = RaftRequest::ConfigRemove {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: namespace_id.to_string(),
        };

        self.raft_write(request).await?;
        Ok(true)
    }

    async fn config_find_gray_one(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigGrayStorageData>> {
        self.ensure_consistent_read().await?;
        let json = self.reader.get_config_gray(data_id, group, namespace_id)?;
        Ok(json.as_ref().map(EmbeddedPersistService::json_to_gray))
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
        let request = RaftRequest::ConfigGrayPublish {
            data_id: data_id.to_string(),
            group: group_id.to_string(),
            tenant: tenant_id.to_string(),
            content: content.to_string(),
            gray_name: gray_name.to_string(),
            gray_rule: gray_rule.to_string(),
            app_name: Some(app_name.to_string()),
            encrypted_data_key: Some(encrypted_data_key.to_string()),
            src_user: Some(src_user.to_string()),
            src_ip: Some(src_ip.to_string()),
        };
        self.raft_write(request).await?;
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
        let request = RaftRequest::ConfigGrayRemove {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: namespace_id.to_string(),
        };
        self.raft_write(request).await?;
        Ok(true)
    }

    async fn config_batch_delete(
        &self,
        _ids: &[i64],
        _client_ip: &str,
        _src_user: &str,
    ) -> anyhow::Result<usize> {
        // RocksDB configs are keyed by tenant@@group@@data_id, not by integer IDs.
        // Batch delete by SQL integer ID is not applicable in embedded/distributed mode.
        Err(anyhow::anyhow!(
            "Batch delete by integer ID is not supported in distributed embedded mode"
        ))
    }

    async fn config_history_find_by_id(
        &self,
        nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryStorageData>> {
        self.ensure_consistent_read().await?;
        let json = self.reader.get_config_history_by_id(nid)?;
        Ok(json.as_ref().map(EmbeddedPersistService::json_to_history))
    }

    async fn config_history_search_page(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryStorageData>> {
        self.ensure_consistent_read().await?;
        let (items, total) =
            self.reader
                .search_config_history(data_id, group, namespace_id, page_no, page_size)?;

        let page_items = items
            .iter()
            .map(EmbeddedPersistService::json_to_history)
            .collect();
        Ok(Page::new(total, page_no, page_size, page_items))
    }

    async fn config_count_by_namespace(&self, namespace_id: &str) -> anyhow::Result<i32> {
        self.ensure_consistent_read().await?;
        self.reader.count_configs(namespace_id)
    }

    async fn config_find_all_group_names(&self, namespace_id: &str) -> anyhow::Result<Vec<String>> {
        self.ensure_consistent_read().await?;
        self.reader.find_all_group_names(namespace_id)
    }

    async fn config_history_get_previous(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        current_nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryStorageData>> {
        self.ensure_consistent_read().await?;
        let (entries, _) =
            self.reader
                .search_config_history(data_id, group, namespace_id, 1, u64::MAX)?;

        // Find the entry with the highest id that is still less than current_nid
        let mut best: Option<&serde_json::Value> = None;
        let mut best_id: u64 = 0;

        for entry in &entries {
            let id = entry["id"].as_u64().unwrap_or(0);
            if id < current_nid && id > best_id {
                best_id = id;
                best = Some(entry);
            }
        }

        Ok(best.map(EmbeddedPersistService::json_to_history))
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
        self.ensure_consistent_read().await?;

        let prefix = format!("{}@@{}@@{}@@", namespace_id, group, data_id);
        let (items, total) = self.reader.search_config_history_with_filters(
            &prefix, op_type, src_user, start_time, end_time, page_no, page_size,
        )?;

        let page_items = items
            .iter()
            .map(EmbeddedPersistService::json_to_history)
            .collect();
        Ok(Page::new(total, page_no, page_size, page_items))
    }

    async fn config_find_by_namespace(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigStorageData>> {
        self.ensure_consistent_read().await?;
        let configs = self.reader.list_configs(namespace_id)?;
        Ok(configs
            .iter()
            .map(EmbeddedPersistService::json_to_config)
            .collect())
    }

    async fn config_find_for_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<Vec<String>>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<ConfigStorageData>> {
        self.ensure_consistent_read().await?;
        let all_configs = self.reader.list_configs(namespace_id)?;

        let filtered: Vec<ConfigStorageData> = all_configs
            .iter()
            .map(EmbeddedPersistService::json_to_config)
            .filter(|cfg| {
                // Filter by group if specified
                if let Some(g) = group
                    && !g.is_empty()
                    && cfg.group != g
                {
                    return false;
                }
                // Filter by data_ids if specified
                if let Some(ref ids) = data_ids
                    && !ids.is_empty()
                    && !ids.contains(&cfg.data_id)
                {
                    return false;
                }
                // Filter by app_name if specified
                if let Some(app) = app_name
                    && !app.is_empty()
                    && cfg.app_name != app
                {
                    return false;
                }
                true
            })
            .collect();

        Ok(filtered)
    }
}

#[async_trait]
impl NamespacePersistence for DistributedPersistService {
    async fn namespace_find_all(&self) -> anyhow::Result<Vec<NamespaceInfo>> {
        self.ensure_consistent_read().await?;
        let namespaces = self.reader.list_namespaces()?;
        let mut result = Vec::with_capacity(namespaces.len());

        for ns in &namespaces {
            let ns_id = ns["namespace_id"].as_str().unwrap_or("");
            let config_count = self.reader.count_configs(ns_id)?;
            result.push(EmbeddedPersistService::json_to_namespace(ns, config_count));
        }

        Ok(result)
    }

    async fn namespace_get_by_id(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Option<NamespaceInfo>> {
        self.ensure_consistent_read().await?;
        let json = self.reader.get_namespace(namespace_id)?;
        match json {
            Some(ref v) => {
                let config_count = self.reader.count_configs(namespace_id)?;
                Ok(Some(EmbeddedPersistService::json_to_namespace(
                    v,
                    config_count,
                )))
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
        let request = RaftRequest::NamespaceCreate {
            namespace_id: namespace_id.to_string(),
            namespace_name: name.to_string(),
            namespace_desc: Some(desc.to_string()),
        };
        self.raft_write(request).await
    }

    async fn namespace_update(
        &self,
        namespace_id: &str,
        name: &str,
        desc: &str,
    ) -> anyhow::Result<bool> {
        let request = RaftRequest::NamespaceUpdate {
            namespace_id: namespace_id.to_string(),
            namespace_name: name.to_string(),
            namespace_desc: Some(desc.to_string()),
        };
        self.raft_write(request).await?;
        Ok(true)
    }

    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool> {
        let request = RaftRequest::NamespaceDelete {
            namespace_id: namespace_id.to_string(),
        };
        self.raft_write(request).await?;
        Ok(true)
    }

    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool> {
        self.ensure_consistent_read().await?;
        Ok(self.reader.get_namespace(namespace_id)?.is_some())
    }
}

#[async_trait]
impl AuthPersistence for DistributedPersistService {
    async fn user_find_by_username(&self, username: &str) -> anyhow::Result<Option<UserInfo>> {
        self.ensure_consistent_read().await?;
        let json = self.reader.get_user(username)?;
        Ok(json.as_ref().map(EmbeddedPersistService::json_to_user))
    }

    async fn user_find_page(
        &self,
        username: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<UserInfo>> {
        self.ensure_consistent_read().await?;
        let (items, total) = self
            .reader
            .search_users(username, accurate, page_no, page_size)?;
        let page_items = items
            .iter()
            .map(EmbeddedPersistService::json_to_user)
            .collect();
        Ok(Page::new(total, page_no, page_size, page_items))
    }

    async fn user_create(
        &self,
        username: &str,
        password_hash: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
        let request = RaftRequest::UserCreate {
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            enabled,
        };
        self.raft_write(request).await
    }

    async fn user_update_password(
        &self,
        username: &str,
        password_hash: &str,
    ) -> anyhow::Result<()> {
        let request = RaftRequest::UserUpdate {
            username: username.to_string(),
            password_hash: Some(password_hash.to_string()),
            enabled: None,
        };
        self.raft_write(request).await
    }

    async fn user_delete(&self, username: &str) -> anyhow::Result<()> {
        let request = RaftRequest::UserDelete {
            username: username.to_string(),
        };
        self.raft_write(request).await
    }

    async fn user_search(&self, username: &str) -> anyhow::Result<Vec<String>> {
        self.ensure_consistent_read().await?;
        let (users, _) = self.reader.search_users(username, false, 1, u64::MAX)?;
        Ok(users
            .iter()
            .map(|v| v["username"].as_str().unwrap_or("").to_string())
            .collect())
    }

    async fn role_find_by_username(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        self.ensure_consistent_read().await?;
        let items = self.reader.get_roles_by_username(username)?;
        Ok(items
            .iter()
            .map(EmbeddedPersistService::json_to_role)
            .collect())
    }

    async fn role_find_page(
        &self,
        username: &str,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<RoleInfo>> {
        self.ensure_consistent_read().await?;
        let (items, total) = self
            .reader
            .search_roles(username, role, accurate, page_no, page_size)?;
        let page_items = items
            .iter()
            .map(EmbeddedPersistService::json_to_role)
            .collect();
        Ok(Page::new(total, page_no, page_size, page_items))
    }

    async fn role_create(&self, role: &str, username: &str) -> anyhow::Result<()> {
        let request = RaftRequest::RoleCreate {
            role: role.to_string(),
            username: username.to_string(),
        };
        self.raft_write(request).await
    }

    async fn role_delete(&self, role: &str, username: &str) -> anyhow::Result<()> {
        let request = RaftRequest::RoleDelete {
            role: role.to_string(),
            username: username.to_string(),
        };
        self.raft_write(request).await
    }

    async fn role_has_global_admin(&self) -> anyhow::Result<bool> {
        self.ensure_consistent_read().await?;
        self.reader.has_global_admin()
    }

    async fn role_has_global_admin_by_username(&self, username: &str) -> anyhow::Result<bool> {
        self.ensure_consistent_read().await?;
        self.reader.has_global_admin_by_username(username)
    }

    async fn role_search(&self, role: &str) -> anyhow::Result<Vec<String>> {
        self.ensure_consistent_read().await?;
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
        self.ensure_consistent_read().await?;
        let items = self.reader.get_permissions_by_role(role)?;
        Ok(items
            .iter()
            .map(EmbeddedPersistService::json_to_permission)
            .collect())
    }

    async fn permission_find_by_roles(
        &self,
        roles: Vec<String>,
    ) -> anyhow::Result<Vec<PermissionInfo>> {
        self.ensure_consistent_read().await?;
        let items = self.reader.get_permissions_by_roles(&roles)?;
        Ok(items
            .iter()
            .map(EmbeddedPersistService::json_to_permission)
            .collect())
    }

    async fn permission_find_page(
        &self,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<PermissionInfo>> {
        self.ensure_consistent_read().await?;
        let (items, total) = self
            .reader
            .search_permissions(role, accurate, page_no, page_size)?;
        let page_items = items
            .iter()
            .map(EmbeddedPersistService::json_to_permission)
            .collect();
        Ok(Page::new(total, page_no, page_size, page_items))
    }

    async fn permission_find_by_id(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<Option<PermissionInfo>> {
        self.ensure_consistent_read().await?;
        match self.reader.get_permission(role, resource, action)? {
            Some(v) => Ok(Some(EmbeddedPersistService::json_to_permission(&v))),
            None => Ok(None),
        }
    }

    async fn permission_grant(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        let request = RaftRequest::PermissionGrant {
            role: role.to_string(),
            resource: resource.to_string(),
            action: action.to_string(),
        };
        self.raft_write(request).await
    }

    async fn permission_revoke(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        let request = RaftRequest::PermissionRevoke {
            role: role.to_string(),
            resource: resource.to_string(),
            action: action.to_string(),
        };
        self.raft_write(request).await
    }
}

#[async_trait]
impl CapacityPersistence for DistributedPersistService {
    async fn capacity_get_tenant(&self, tenant_id: &str) -> anyhow::Result<Option<CapacityInfo>> {
        let key = format!("capacity:tenant:{}", tenant_id);
        match self.reader.db().get(key.as_bytes())? {
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
        let db = self.reader.db();

        // Read existing or create defaults
        let mut info = match db.get(key.as_bytes())? {
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
        db.put(key.as_bytes(), &json)?;

        Ok(info)
    }

    async fn capacity_delete_tenant(&self, tenant_id: &str) -> anyhow::Result<bool> {
        let key = format!("capacity:tenant:{}", tenant_id);
        let db = self.reader.db();
        let exists = db.get(key.as_bytes())?.is_some();
        if exists {
            db.delete(key.as_bytes())?;
        }
        Ok(exists)
    }

    async fn capacity_get_group(&self, group_id: &str) -> anyhow::Result<Option<CapacityInfo>> {
        let key = format!("capacity:group:{}", group_id);
        match self.reader.db().get(key.as_bytes())? {
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
        let db = self.reader.db();

        // Read existing or create defaults
        let mut info = match db.get(key.as_bytes())? {
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

        // Write back
        let json = serde_json::to_vec(&info)?;
        db.put(key.as_bytes(), &json)?;

        Ok(info)
    }

    async fn capacity_delete_group(&self, group_id: &str) -> anyhow::Result<bool> {
        let key = format!("capacity:group:{}", group_id);
        let db = self.reader.db();
        let exists = db.get(key.as_bytes())?.is_some();
        if exists {
            db.delete(key.as_bytes())?;
        }
        Ok(exists)
    }
}

#[async_trait]
impl PersistenceService for DistributedPersistService {
    fn storage_mode(&self) -> StorageMode {
        StorageMode::DistributedEmbedded
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        // Verify Raft is working by checking if there's a leader
        if self.raft_node.leader_id().is_some() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("No Raft leader elected"))
        }
    }
}
