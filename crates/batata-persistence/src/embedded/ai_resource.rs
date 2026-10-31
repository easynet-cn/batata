// AI resource persistence for the embedded (standalone) backend
// Uses RocksDB column families for ai_resource, ai_resource_version, and pipeline_execution

use async_trait::async_trait;

use batata_consistency::raft::state_machine::{
    CF_AI_RESOURCE, CF_AI_RESOURCE_SEARCH_CHUNK, CF_AI_RESOURCE_SEARCH_DOCUMENT,
    CF_AI_RESOURCE_TASK, CF_AI_RESOURCE_VERSION, CF_PIPELINE_EXECUTION,
};

use crate::model::{
    AiResourceInfo, AiResourceListFilter, AiResourceSearchChunkInfo, AiResourceSearchDocumentInfo,
    AiResourceSearchHitInfo, AiResourceTaskInfo, AiResourceVersionInfo, Page,
    PipelineExecutionInfo,
};
use crate::traits::ai_resource::{AiResourcePersistence, TASK_STATUS_PROCESSING, task_is_due};

use super::EmbeddedPersistService;

/// Take the lock that serializes index-task mutations.
///
/// Panics are recovered on unlock so a poisoned lock (a task mutation that
/// panicked) cannot wedge the whole queue.
fn lock_tasks(service: &EmbeddedPersistService) -> std::sync::MutexGuard<'_, ()> {
    service
        .task_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Current time as an RFC 3339 string, matching the other embedded writers.
fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Key for the auto-increment counter stored in the ai_resource CF
const KEY_AI_RESOURCE_NEXT_ID: &str = "__ai_resource_next_id__";
/// Key for the auto-increment counter stored in the ai_resource_version CF
const KEY_AI_RESOURCE_VERSION_NEXT_ID: &str = "__ai_resource_version_next_id__";
/// Key for the auto-increment counter stored in the search document CF
const KEY_SEARCH_DOCUMENT_NEXT_ID: &str = "__search_document_next_id__";
/// Key for the auto-increment counter stored in the search chunk CF
const KEY_SEARCH_CHUNK_NEXT_ID: &str = "__search_chunk_next_id__";

impl EmbeddedPersistService {
    /// Build the key for an ai_resource entry
    fn ai_resource_key(namespace_id: &str, resource_type: &str, name: &str) -> String {
        format!("{}:{}:{}", namespace_id, resource_type, name)
    }

    /// Build the key for an ai_resource_version entry
    fn ai_resource_version_key(
        namespace_id: &str,
        resource_type: &str,
        name: &str,
        version: &str,
    ) -> String {
        format!("{}:{}:{}:{}", namespace_id, resource_type, name, version)
    }

    /// Build the key prefix for listing versions of a specific resource
    fn ai_resource_version_prefix(namespace_id: &str, resource_type: &str, name: &str) -> String {
        format!("{}:{}:{}:", namespace_id, resource_type, name)
    }

    /// Get the next auto-increment ID for ai_resource
    fn next_ai_resource_id(&self) -> anyhow::Result<i64> {
        let cf = self.cf(CF_AI_RESOURCE)?;
        let current = match self.db.get_cf(cf, KEY_AI_RESOURCE_NEXT_ID.as_bytes())? {
            Some(bytes) => {
                let s = String::from_utf8(bytes.to_vec())?;
                s.parse::<i64>()?
            }
            None => 0,
        };
        let next = current + 1;
        self.db.put_cf(
            cf,
            KEY_AI_RESOURCE_NEXT_ID.as_bytes(),
            next.to_string().as_bytes(),
        )?;
        Ok(next)
    }

    /// Get the next auto-increment ID for ai_resource_version
    fn next_ai_resource_version_id(&self) -> anyhow::Result<i64> {
        let cf = self.cf(CF_AI_RESOURCE_VERSION)?;
        let current = match self
            .db
            .get_cf(cf, KEY_AI_RESOURCE_VERSION_NEXT_ID.as_bytes())?
        {
            Some(bytes) => {
                let s = String::from_utf8(bytes.to_vec())?;
                s.parse::<i64>()?
            }
            None => 0,
        };
        let next = current + 1;
        self.db.put_cf(
            cf,
            KEY_AI_RESOURCE_VERSION_NEXT_ID.as_bytes(),
            next.to_string().as_bytes(),
        )?;
        Ok(next)
    }

    /// Read an AiResourceInfo from the ai_resource CF
    fn get_ai_resource(&self, key: &str) -> anyhow::Result<Option<AiResourceInfo>> {
        let cf = self.cf(CF_AI_RESOURCE)?;
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(bytes) => {
                let info: AiResourceInfo = serde_json::from_slice(&bytes)?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    /// Write an AiResourceInfo to the ai_resource CF
    fn put_ai_resource(&self, key: &str, info: &AiResourceInfo) -> anyhow::Result<()> {
        let cf = self.cf(CF_AI_RESOURCE)?;
        let json = serde_json::to_vec(info)?;
        self.db
            .put_cf(cf, key.as_bytes(), &json)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))
    }

    /// Read an AiResourceVersionInfo from the ai_resource_version CF
    fn get_ai_resource_version(&self, key: &str) -> anyhow::Result<Option<AiResourceVersionInfo>> {
        let cf = self.cf(CF_AI_RESOURCE_VERSION)?;
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(bytes) => {
                let info: AiResourceVersionInfo = serde_json::from_slice(&bytes)?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    /// Write an AiResourceVersionInfo to the ai_resource_version CF
    fn put_ai_resource_version(
        &self,
        key: &str,
        info: &AiResourceVersionInfo,
    ) -> anyhow::Result<()> {
        let cf = self.cf(CF_AI_RESOURCE_VERSION)?;
        let json = serde_json::to_vec(info)?;
        self.db
            .put_cf(cf, key.as_bytes(), &json)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))
    }

    /// Scan all ai_resource entries matching a prefix, deserialize, and collect
    fn scan_ai_resources(&self, prefix: &str) -> anyhow::Result<Vec<AiResourceInfo>> {
        let cf = self.cf(CF_AI_RESOURCE)?;
        let mut results = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(prefix) {
                break;
            }
            // Skip internal counter keys
            if key_str.starts_with("__") {
                continue;
            }
            let info: AiResourceInfo = serde_json::from_slice(&value)?;
            results.push(info);
        }
        Ok(results)
    }

    /// Scan all ai_resource_version entries matching a prefix
    fn scan_ai_resource_versions(
        &self,
        prefix: &str,
    ) -> anyhow::Result<Vec<AiResourceVersionInfo>> {
        let cf = self.cf(CF_AI_RESOURCE_VERSION)?;
        let mut results = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(prefix) {
                break;
            }
            // Skip internal counter keys
            if key_str.starts_with("__") {
                continue;
            }
            let info: AiResourceVersionInfo = serde_json::from_slice(&value)?;
            results.push(info);
        }
        Ok(results)
    }

    // ========================================================================
    // ai_resource_search_document / _chunk / _task helpers
    // ========================================================================

    /// Build the key for an ai_resource_search_document entry
    fn search_document_key(
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> String {
        format!(
            "{}:{}:{}:{}",
            namespace_id, resource_type, resource_name, resource_version
        )
    }

    /// Build the key prefix for the chunks of one resource version
    fn search_chunk_prefix(
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> String {
        format!(
            "{}:{}:{}:{}:",
            namespace_id, resource_type, resource_name, resource_version
        )
    }

    /// Get the next auto-increment ID for ai_resource_search_document
    fn next_search_document_id(&self) -> anyhow::Result<i64> {
        let cf = self.cf(CF_AI_RESOURCE_SEARCH_DOCUMENT)?;
        let current = match self.db.get_cf(cf, KEY_SEARCH_DOCUMENT_NEXT_ID.as_bytes())? {
            Some(bytes) => String::from_utf8(bytes.to_vec())?.parse::<i64>()?,
            None => 0,
        };
        let next = current + 1;
        self.db.put_cf(
            cf,
            KEY_SEARCH_DOCUMENT_NEXT_ID.as_bytes(),
            next.to_string().as_bytes(),
        )?;
        Ok(next)
    }

    /// Get the next auto-increment ID for ai_resource_search_chunk
    fn next_search_chunk_id(&self) -> anyhow::Result<i64> {
        let cf = self.cf(CF_AI_RESOURCE_SEARCH_CHUNK)?;
        let current = match self.db.get_cf(cf, KEY_SEARCH_CHUNK_NEXT_ID.as_bytes())? {
            Some(bytes) => String::from_utf8(bytes.to_vec())?.parse::<i64>()?,
            None => 0,
        };
        let next = current + 1;
        self.db.put_cf(
            cf,
            KEY_SEARCH_CHUNK_NEXT_ID.as_bytes(),
            next.to_string().as_bytes(),
        )?;
        Ok(next)
    }

    /// Read one ai_resource_search_document entry
    fn get_search_document(
        &self,
        key: &str,
    ) -> anyhow::Result<Option<AiResourceSearchDocumentInfo>> {
        let cf = self.cf(CF_AI_RESOURCE_SEARCH_DOCUMENT)?;
        match self.db.get_cf(cf, key.as_bytes())? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Write one ai_resource_search_document entry
    fn put_search_document(
        &self,
        key: &str,
        info: &AiResourceSearchDocumentInfo,
    ) -> anyhow::Result<()> {
        let cf = self.cf(CF_AI_RESOURCE_SEARCH_DOCUMENT)?;
        let json = serde_json::to_vec(info)?;
        self.db
            .put_cf(cf, key.as_bytes(), &json)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))
    }

    /// Scan the chunks stored under a resource-version prefix
    fn scan_search_chunks(&self, prefix: &str) -> anyhow::Result<Vec<AiResourceSearchChunkInfo>> {
        let cf = self.cf(CF_AI_RESOURCE_SEARCH_CHUNK)?;
        let mut results = Vec::new();
        for item in self.db.prefix_iterator_cf(cf, prefix.as_bytes()) {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(prefix) {
                break;
            }
            if key_str.starts_with("__") {
                continue;
            }
            results.push(serde_json::from_slice(&value)?);
        }
        Ok(results)
    }

    /// Delete every chunk stored under a resource-version prefix
    fn delete_search_chunks(&self, prefix: &str) -> anyhow::Result<u64> {
        let cf = self.cf(CF_AI_RESOURCE_SEARCH_CHUNK)?;
        let mut deleted = 0u64;
        for item in self.db.prefix_iterator_cf(cf, prefix.as_bytes()) {
            let (key, _) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(prefix) {
                break;
            }
            if key_str.starts_with("__") {
                continue;
            }
            self.db.delete_cf(cf, &key)?;
            deleted += 1;
        }
        Ok(deleted)
    }

    /// Read one ai_resource_task entry
    fn get_task(&self, task_key: &str) -> anyhow::Result<Option<AiResourceTaskInfo>> {
        let cf = self.cf(CF_AI_RESOURCE_TASK)?;
        match self.db.get_cf(cf, task_key.as_bytes())? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Write one ai_resource_task entry
    fn put_task(&self, task: &AiResourceTaskInfo) -> anyhow::Result<()> {
        let cf = self.cf(CF_AI_RESOURCE_TASK)?;
        let json = serde_json::to_vec(task)?;
        self.db
            .put_cf(cf, task.task_key.as_bytes(), &json)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))
    }

    /// Scan every stored task (empty prefix iterates the whole column family)
    fn scan_tasks(&self) -> anyhow::Result<Vec<AiResourceTaskInfo>> {
        let cf = self.cf(CF_AI_RESOURCE_TASK)?;
        let mut results = Vec::new();
        for item in self.db.prefix_iterator_cf(cf, b"") {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if key_str.starts_with("__") {
                continue;
            }
            results.push(serde_json::from_slice(&value)?);
        }
        Ok(results)
    }
}

#[async_trait]
impl AiResourcePersistence for EmbeddedPersistService {
    // ========================================================================
    // ai_resource operations
    // ========================================================================

    async fn ai_resource_find(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<Option<AiResourceInfo>> {
        let key = Self::ai_resource_key(namespace_id, resource_type, name);
        self.get_ai_resource(&key)
    }

    async fn ai_resource_insert(&self, resource: &AiResourceInfo) -> anyhow::Result<i64> {
        let id = self.next_ai_resource_id()?;
        let mut info = resource.clone();
        info.id = id;
        let now = chrono::Utc::now().to_rfc3339();
        if info.gmt_create.is_none() {
            info.gmt_create = Some(now.clone());
        }
        if info.gmt_modified.is_none() {
            info.gmt_modified = Some(now);
        }
        let key = Self::ai_resource_key(&info.namespace_id, &info.resource_type, &info.name);
        self.put_ai_resource(&key, &info)?;
        Ok(id)
    }

    async fn ai_resource_update_version_info_cas(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        expected_meta_version: i64,
        version_info: &str,
        new_meta_version: i64,
    ) -> anyhow::Result<bool> {
        let key = Self::ai_resource_key(namespace_id, resource_type, name);
        match self.get_ai_resource(&key)? {
            Some(mut info) => {
                if info.meta_version != expected_meta_version {
                    return Ok(false);
                }
                info.version_info = Some(version_info.to_string());
                info.meta_version = new_meta_version;
                info.gmt_modified = Some(chrono::Utc::now().to_rfc3339());
                self.put_ai_resource(&key, &info)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    async fn ai_resource_update_biz_tags(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        biz_tags: &str,
    ) -> anyhow::Result<()> {
        let key = Self::ai_resource_key(namespace_id, resource_type, name);
        match self.get_ai_resource(&key)? {
            Some(mut info) => {
                info.biz_tags = Some(biz_tags.to_string());
                info.gmt_modified = Some(chrono::Utc::now().to_rfc3339());
                self.put_ai_resource(&key, &info)
            }
            None => Err(anyhow::anyhow!(
                "AI resource not found: {}:{}:{}",
                namespace_id,
                resource_type,
                name
            )),
        }
    }

    async fn ai_resource_update_description(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        description: &str,
    ) -> anyhow::Result<()> {
        let key = Self::ai_resource_key(namespace_id, resource_type, name);
        match self.get_ai_resource(&key)? {
            Some(mut info) => {
                info.description = Some(description.to_string());
                info.gmt_modified = Some(chrono::Utc::now().to_rfc3339());
                self.put_ai_resource(&key, &info)
            }
            None => Err(anyhow::anyhow!(
                "AI resource not found: {}:{}:{}",
                namespace_id,
                resource_type,
                name
            )),
        }
    }

    async fn ai_resource_update_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        let key = Self::ai_resource_key(namespace_id, resource_type, name);
        match self.get_ai_resource(&key)? {
            Some(mut info) => {
                info.status = Some(status.to_string());
                info.gmt_modified = Some(chrono::Utc::now().to_rfc3339());
                self.put_ai_resource(&key, &info)
            }
            None => Err(anyhow::anyhow!(
                "AI resource not found: {}:{}:{}",
                namespace_id,
                resource_type,
                name
            )),
        }
    }

    async fn ai_resource_update_scope(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        scope: &str,
    ) -> anyhow::Result<()> {
        let key = Self::ai_resource_key(namespace_id, resource_type, name);
        match self.get_ai_resource(&key)? {
            Some(mut info) => {
                info.scope = scope.to_string();
                info.gmt_modified = Some(chrono::Utc::now().to_rfc3339());
                self.put_ai_resource(&key, &info)
            }
            None => Err(anyhow::anyhow!(
                "AI resource not found: {}:{}:{}",
                namespace_id,
                resource_type,
                name
            )),
        }
    }

    async fn ai_resource_increment_download_count(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        increment: i64,
    ) -> anyhow::Result<()> {
        let key = Self::ai_resource_key(namespace_id, resource_type, name);
        match self.get_ai_resource(&key)? {
            Some(mut info) => {
                info.download_count += increment;
                info.gmt_modified = Some(chrono::Utc::now().to_rfc3339());
                self.put_ai_resource(&key, &info)
            }
            None => Err(anyhow::anyhow!(
                "AI resource not found: {}:{}:{}",
                namespace_id,
                resource_type,
                name
            )),
        }
    }

    async fn ai_resource_delete(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<u64> {
        let key = Self::ai_resource_key(namespace_id, resource_type, name);
        let cf = self.cf(CF_AI_RESOURCE)?;
        if self.db.get_cf(cf, key.as_bytes())?.is_some() {
            self.db
                .delete_cf(cf, key.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            Ok(1)
        } else {
            Ok(0)
        }
    }

    async fn ai_resource_list(
        &self,
        namespace_id: &str,
        resource_type: &str,
        filter: &AiResourceListFilter<'_>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AiResourceInfo>> {
        let prefix = format!("{}:{}:", namespace_id, resource_type);
        let mut items = self.scan_ai_resources(&prefix)?;

        // Apply name filter
        if let Some(name) = filter.name_filter
            && !name.is_empty()
        {
            items.retain(|item| {
                if filter.search_accurate {
                    item.name == name
                } else {
                    item.name.contains(name)
                }
            });
        }

        // Apply scope filter
        if let Some(scope) = filter.scope_filter {
            items.retain(|item| item.scope == scope);
        }

        // Apply owner filter (with optional PUBLIC inclusion)
        if let Some(owner) = filter.owner_filter {
            if filter.include_public_for_owner {
                items.retain(|item| item.owner == owner || item.scope == "PUBLIC");
            } else {
                items.retain(|item| item.owner == owner);
            }
        }

        // Sort
        if filter.order_by_downloads {
            items.sort_by(|a, b| b.download_count.cmp(&a.download_count));
        } else {
            items.sort_by(|a, b| a.name.cmp(&b.name));
        }

        // Paginate
        let total_count = items.len() as u64;
        let start = ((page_no.saturating_sub(1)) * page_size) as usize;
        let page_items: Vec<AiResourceInfo> = items
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();

        Ok(Page::new(total_count, page_no, page_size, page_items))
    }

    async fn ai_resource_find_all(
        &self,
        namespace_id: &str,
        resource_type: &str,
    ) -> anyhow::Result<Vec<AiResourceInfo>> {
        let prefix = format!("{}:{}:", namespace_id, resource_type);
        self.scan_ai_resources(&prefix)
    }

    // ========================================================================
    // ai_resource_version operations
    // ========================================================================

    async fn ai_resource_version_find(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
    ) -> anyhow::Result<Option<AiResourceVersionInfo>> {
        let key = Self::ai_resource_version_key(namespace_id, resource_type, name, version);
        self.get_ai_resource_version(&key)
    }

    async fn ai_resource_version_insert(
        &self,
        version: &AiResourceVersionInfo,
    ) -> anyhow::Result<i64> {
        let id = self.next_ai_resource_version_id()?;
        let mut info = version.clone();
        info.id = id;
        let now = chrono::Utc::now().to_rfc3339();
        if info.gmt_create.is_none() {
            info.gmt_create = Some(now.clone());
        }
        if info.gmt_modified.is_none() {
            info.gmt_modified = Some(now);
        }
        let key = Self::ai_resource_version_key(
            &info.namespace_id,
            &info.resource_type,
            &info.name,
            &info.version,
        );
        self.put_ai_resource_version(&key, &info)?;
        Ok(id)
    }

    async fn ai_resource_version_update_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        let key = Self::ai_resource_version_key(namespace_id, resource_type, name, version);
        match self.get_ai_resource_version(&key)? {
            Some(mut info) => {
                info.status = status.to_string();
                info.gmt_modified = Some(chrono::Utc::now().to_rfc3339());
                self.put_ai_resource_version(&key, &info)
            }
            None => Err(anyhow::anyhow!(
                "AI resource version not found: {}:{}:{}:{}",
                namespace_id,
                resource_type,
                name,
                version
            )),
        }
    }

    async fn ai_resource_version_update_storage(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
        storage: &str,
        description: Option<&str>,
    ) -> anyhow::Result<()> {
        let key = Self::ai_resource_version_key(namespace_id, resource_type, name, version);
        match self.get_ai_resource_version(&key)? {
            Some(mut info) => {
                info.storage = Some(storage.to_string());
                if let Some(desc) = description {
                    info.description = Some(desc.to_string());
                }
                info.gmt_modified = Some(chrono::Utc::now().to_rfc3339());
                self.put_ai_resource_version(&key, &info)
            }
            None => Err(anyhow::anyhow!(
                "AI resource version not found: {}:{}:{}:{}",
                namespace_id,
                resource_type,
                name,
                version
            )),
        }
    }

    async fn ai_resource_version_increment_download_count(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
        increment: i64,
    ) -> anyhow::Result<()> {
        let key = Self::ai_resource_version_key(namespace_id, resource_type, name, version);
        match self.get_ai_resource_version(&key)? {
            Some(mut info) => {
                info.download_count += increment;
                info.gmt_modified = Some(chrono::Utc::now().to_rfc3339());
                self.put_ai_resource_version(&key, &info)
            }
            None => Err(anyhow::anyhow!(
                "AI resource version not found: {}:{}:{}:{}",
                namespace_id,
                resource_type,
                name,
                version
            )),
        }
    }

    async fn ai_resource_version_list(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<Vec<AiResourceVersionInfo>> {
        let prefix = Self::ai_resource_version_prefix(namespace_id, resource_type, name);
        self.scan_ai_resource_versions(&prefix)
    }

    async fn ai_resource_version_count_by_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        status: &str,
    ) -> anyhow::Result<u64> {
        let prefix = Self::ai_resource_version_prefix(namespace_id, resource_type, name);
        let versions = self.scan_ai_resource_versions(&prefix)?;
        Ok(versions.iter().filter(|v| v.status == status).count() as u64)
    }

    async fn ai_resource_version_delete(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        version: &str,
    ) -> anyhow::Result<u64> {
        let key = Self::ai_resource_version_key(namespace_id, resource_type, name, version);
        let cf = self.cf(CF_AI_RESOURCE_VERSION)?;
        if self.db.get_cf(cf, key.as_bytes())?.is_some() {
            self.db
                .delete_cf(cf, key.as_bytes())
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            Ok(1)
        } else {
            Ok(0)
        }
    }

    async fn ai_resource_version_delete_all(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<u64> {
        let prefix = Self::ai_resource_version_prefix(namespace_id, resource_type, name);
        let cf = self.cf(CF_AI_RESOURCE_VERSION)?;
        let mut count = 0u64;
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        let mut keys_to_delete = Vec::new();
        for item in iter {
            let (key, _) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            if key_str.starts_with("__") {
                continue;
            }
            keys_to_delete.push(key.to_vec());
        }
        for key in &keys_to_delete {
            self.db
                .delete_cf(cf, key)
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            count += 1;
        }
        Ok(count)
    }

    async fn ai_resource_version_delete_by_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        status: &str,
    ) -> anyhow::Result<u64> {
        let prefix = Self::ai_resource_version_prefix(namespace_id, resource_type, name);
        let cf = self.cf(CF_AI_RESOURCE_VERSION)?;
        let mut count = 0u64;
        let mut keys_to_delete = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            let (key, value) =
                item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(&prefix) {
                break;
            }
            if key_str.starts_with("__") {
                continue;
            }
            let info: AiResourceVersionInfo = serde_json::from_slice(&value)?;
            if info.status == status {
                keys_to_delete.push(key.to_vec());
            }
        }
        for key in &keys_to_delete {
            self.db
                .delete_cf(cf, key)
                .map_err(|e| anyhow::anyhow!("RocksDB delete error: {}", e))?;
            count += 1;
        }
        Ok(count)
    }

    // ========================================================================
    // pipeline_execution operations
    // ========================================================================

    async fn pipeline_execution_find(
        &self,
        execution_id: &str,
    ) -> anyhow::Result<Option<PipelineExecutionInfo>> {
        let cf = self.cf(CF_PIPELINE_EXECUTION)?;
        match self.db.get_cf(cf, execution_id.as_bytes())? {
            Some(bytes) => {
                let info: PipelineExecutionInfo = serde_json::from_slice(&bytes)?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    async fn pipeline_execution_list(
        &self,
        resource_type: &str,
        resource_name: Option<&str>,
        namespace_id: Option<&str>,
        version: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<PipelineExecutionInfo>> {
        let cf = self.cf(CF_PIPELINE_EXECUTION)?;
        let mut items = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, value) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let info: PipelineExecutionInfo = serde_json::from_slice(&value)?;

            // Apply filters
            if info.resource_type != resource_type {
                continue;
            }
            if let Some(rn) = resource_name
                && info.resource_name != rn
            {
                continue;
            }
            if let Some(ns) = namespace_id
                && info.namespace_id.as_deref() != Some(ns)
            {
                continue;
            }
            if let Some(v) = version
                && info.version.as_deref() != Some(v)
            {
                continue;
            }
            items.push(info);
        }

        // Sort by create_time descending (newest first)
        items.sort_by(|a, b| b.create_time.cmp(&a.create_time));

        // Paginate
        let total_count = items.len() as u64;
        let start = ((page_no.saturating_sub(1)) * page_size) as usize;
        let page_items: Vec<PipelineExecutionInfo> = items
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();

        Ok(Page::new(total_count, page_no, page_size, page_items))
    }

    // ========================================================================
    // ai_resource_search_document operations
    // ========================================================================

    async fn search_document_find(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<Option<AiResourceSearchDocumentInfo>> {
        let key =
            Self::search_document_key(namespace_id, resource_type, resource_name, resource_version);
        self.get_search_document(&key)
    }

    async fn search_document_upsert(
        &self,
        document: &AiResourceSearchDocumentInfo,
    ) -> anyhow::Result<i64> {
        let key = Self::search_document_key(
            &document.namespace_id,
            &document.resource_type,
            &document.resource_name,
            &document.resource_version,
        );
        let now = chrono::Utc::now().to_rfc3339();

        match self.get_search_document(&key)? {
            Some(existing) => {
                let mut updated = document.clone();
                updated.id = existing.id;
                updated.gmt_create = existing.gmt_create;
                updated.gmt_modified = Some(now);
                self.put_search_document(&key, &updated)?;
                Ok(existing.id)
            }
            None => {
                let id = self.next_search_document_id()?;
                let mut created = document.clone();
                created.id = id;
                created.gmt_create = Some(now.clone());
                created.gmt_modified = Some(now);
                self.put_search_document(&key, &created)?;
                Ok(id)
            }
        }
    }

    async fn search_document_delete(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<u64> {
        let key =
            Self::search_document_key(namespace_id, resource_type, resource_name, resource_version);
        if self.get_search_document(&key)?.is_none() {
            return Ok(0);
        }
        let cf = self.cf(CF_AI_RESOURCE_SEARCH_DOCUMENT)?;
        self.db.delete_cf(cf, key.as_bytes())?;
        Ok(1)
    }

    // ========================================================================
    // ai_resource_search_chunk operations
    // ========================================================================

    async fn search_chunk_replace(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
        chunks: &[AiResourceSearchChunkInfo],
    ) -> anyhow::Result<u64> {
        let prefix =
            Self::search_chunk_prefix(namespace_id, resource_type, resource_name, resource_version);
        self.delete_search_chunks(&prefix)?;

        if chunks.is_empty() {
            return Ok(0);
        }

        let document_id = self
            .search_document_find(namespace_id, resource_type, resource_name, resource_version)
            .await?
            .map(|d| d.id)
            .unwrap_or_default();

        let now = chrono::Utc::now().to_rfc3339();
        let cf = self.cf(CF_AI_RESOURCE_SEARCH_CHUNK)?;
        for chunk in chunks {
            let id = self.next_search_chunk_id()?;
            let mut stored = chunk.clone();
            stored.id = id;
            stored.document_id = document_id;
            stored.gmt_create = Some(now.clone());
            stored.gmt_modified = Some(now.clone());
            let json = serde_json::to_vec(&stored)?;
            self.db
                .put_cf(cf, format!("{prefix}{id}").as_bytes(), &json)
                .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))?;
        }
        Ok(chunks.len() as u64)
    }

    async fn search_chunk_list(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<Vec<AiResourceSearchChunkInfo>> {
        let prefix =
            Self::search_chunk_prefix(namespace_id, resource_type, resource_name, resource_version);
        self.scan_search_chunks(&prefix)
    }

    async fn search_chunk_delete(
        &self,
        namespace_id: &str,
        resource_type: &str,
        resource_name: &str,
        resource_version: &str,
    ) -> anyhow::Result<u64> {
        let prefix =
            Self::search_chunk_prefix(namespace_id, resource_type, resource_name, resource_version);
        self.delete_search_chunks(&prefix)
    }

    async fn search_chunk_search(
        &self,
        namespace_id: &str,
        text: &str,
        resource_types: &[&str],
        limit: u64,
    ) -> anyhow::Result<Vec<AiResourceSearchHitInfo>> {
        Ok(crate::search_util::keyword_hits(
            self.scan_search_chunks(&format!("{}:", namespace_id))?,
            text,
            resource_types,
            limit,
        ))
    }

    // ========================================================================
    // ai_resource_task operations
    // ========================================================================

    async fn task_upsert(&self, task: &AiResourceTaskInfo) -> anyhow::Result<()> {
        let _guard = lock_tasks(self);
        let now = now_rfc3339();
        let mut stored = task.clone();
        match self.get_task(&task.task_key)? {
            Some(existing) => {
                stored.gmt_create = existing.gmt_create;
                stored.gmt_modified = Some(now);
            }
            None => {
                stored.gmt_create = Some(now.clone());
                stored.gmt_modified = Some(now);
            }
        }
        self.put_task(&stored)
    }

    async fn task_find(&self, task_key: &str) -> anyhow::Result<Option<AiResourceTaskInfo>> {
        self.get_task(task_key)
    }

    async fn task_find_due(
        &self,
        task_type: &str,
        now_millis: i64,
        limit: u64,
    ) -> anyhow::Result<Vec<AiResourceTaskInfo>> {
        let mut due: Vec<AiResourceTaskInfo> = self
            .scan_tasks()?
            .into_iter()
            .filter(|t| {
                t.task_type == task_type
                    && t.next_execute_at <= now_millis
                    && match t.lease_expire_at {
                        Some(expiry) => expiry <= now_millis,
                        None => true,
                    }
            })
            .collect();
        due.sort_by(|a, b| a.next_execute_at.cmp(&b.next_execute_at));
        due.truncate(limit as usize);
        Ok(due)
    }

    async fn task_delete(&self, task_key: &str) -> anyhow::Result<u64> {
        let _guard = lock_tasks(self);
        if self.get_task(task_key)?.is_none() {
            return Ok(0);
        }
        let cf = self.cf(CF_AI_RESOURCE_TASK)?;
        self.db.delete_cf(cf, task_key.as_bytes())?;
        Ok(1)
    }

    async fn task_claim(
        &self,
        task: &AiResourceTaskInfo,
        lease_millis: i64,
        now_millis: i64,
    ) -> anyhow::Result<Option<AiResourceTaskInfo>> {
        let _guard = lock_tasks(self);
        let Some(stored) = self.get_task(&task.task_key)? else {
            return Ok(None);
        };
        if stored.revision != task.revision || stored.lease_token != task.lease_token {
            return Ok(None);
        }
        if !task_is_due(&stored, now_millis) {
            return Ok(None);
        }
        let mut claimed = stored.clone();
        claimed.status = TASK_STATUS_PROCESSING.to_string();
        claimed.lease_token = stored.lease_token + 1;
        claimed.lease_expire_at = Some(now_millis + lease_millis);
        claimed.gmt_modified = Some(now_rfc3339());
        self.put_task(&claimed)?;
        Ok(Some(claimed))
    }

    async fn task_renew_lease(
        &self,
        task: &AiResourceTaskInfo,
        lease_millis: i64,
        now_millis: i64,
    ) -> anyhow::Result<bool> {
        let _guard = lock_tasks(self);
        let Some(stored) = self.get_task(&task.task_key)? else {
            return Ok(false);
        };
        // The token still being ours is not enough: once the expiry has passed
        // another worker may already have taken over, and renewing then would
        // hand the lease back to a worker that no longer owns it.
        if stored.lease_token != task.lease_token {
            return Ok(false);
        }
        if !stored
            .lease_expire_at
            .is_some_and(|expiry| expiry >= now_millis)
        {
            return Ok(false);
        }
        let mut renewed = stored.clone();
        renewed.lease_expire_at = Some(now_millis + lease_millis);
        renewed.gmt_modified = Some(now_rfc3339());
        self.put_task(&renewed)?;
        Ok(true)
    }

    async fn task_update_leased(
        &self,
        task: &AiResourceTaskInfo,
        lease_token: i64,
    ) -> anyhow::Result<bool> {
        let _guard = lock_tasks(self);
        let Some(stored) = self.get_task(&task.task_key)? else {
            return Ok(false);
        };
        if stored.revision != task.revision
            || stored.lease_token != lease_token
            || stored.status != TASK_STATUS_PROCESSING
        {
            return Ok(false);
        }
        let mut updated = task.clone();
        updated.gmt_create = stored.gmt_create;
        updated.gmt_modified = Some(now_rfc3339());
        self.put_task(&updated)?;
        Ok(true)
    }

    async fn task_delete_leased(&self, task_key: &str, lease_token: i64) -> anyhow::Result<bool> {
        let _guard = lock_tasks(self);
        let Some(stored) = self.get_task(task_key)? else {
            return Ok(false);
        };
        if stored.lease_token != lease_token || stored.status != TASK_STATUS_PROCESSING {
            return Ok(false);
        }
        let cf = self.cf(CF_AI_RESOURCE_TASK)?;
        self.db.delete_cf(cf, task_key.as_bytes())?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_consistency::raft::state_machine::RocksStateMachine;
    use tempfile::TempDir;

    async fn create_test_service() -> (EmbeddedPersistService, TempDir) {
        let tmp_dir = TempDir::new().unwrap();
        let sm = RocksStateMachine::new(tmp_dir.path()).await.unwrap();
        let service = EmbeddedPersistService::from_state_machine(&sm);
        (service, tmp_dir)
    }

    fn document(digest: &str) -> AiResourceSearchDocumentInfo {
        AiResourceSearchDocumentInfo {
            id: 0,
            namespace_id: "public".to_string(),
            resource_type: "skill".to_string(),
            resource_name: "probe-skill".to_string(),
            resource_version: "1.0.0".to_string(),
            display_name: "Probe".to_string(),
            description: None,
            tags: None,
            capabilities: None,
            representative_queries: None,
            metadata: None,
            source_digest: digest.to_string(),
            status: "pending".to_string(),
            generate_mode: "auto".to_string(),
            gmt_create: None,
            gmt_modified: None,
        }
    }

    fn chunk(suffix: &str) -> AiResourceSearchChunkInfo {
        AiResourceSearchChunkInfo {
            id: 0,
            document_id: 0,
            namespace_id: "public".to_string(),
            resource_type: "skill".to_string(),
            resource_name: "probe-skill".to_string(),
            resource_version: "1.0.0".to_string(),
            chunk_type: "description".to_string(),
            chunk_text: format!("text-{suffix}"),
            canonical_text: format!("canonical-{suffix}"),
            language: None,
            chunk_hash: format!("hash-{suffix}"),
            metadata: None,
            status: "enabled".to_string(),
            gmt_create: None,
            gmt_modified: None,
        }
    }

    fn task(key: &str, next_execute_at: i64) -> AiResourceTaskInfo {
        AiResourceTaskInfo {
            task_key: key.to_string(),
            namespace_id: "public".to_string(),
            task_type: "search_index".to_string(),
            task_stage: "base_index".to_string(),
            status: "pending".to_string(),
            task_payload: "{}".to_string(),
            task_result: None,
            retry_count: 0,
            revision: 1,
            lease_token: 0,
            next_execute_at,
            lease_expire_at: None,
            last_error: None,
            gmt_create: None,
            gmt_modified: None,
        }
    }

    #[tokio::test]
    async fn test_embedded_search_index_round_trip() {
        let (svc, _tmp) = create_test_service().await;

        // Document insert then upsert-update keeps a stable id.
        let id = svc
            .search_document_upsert(&document("digest-1"))
            .await
            .unwrap();
        assert!(id > 0);
        let id2 = svc
            .search_document_upsert(&document("digest-2"))
            .await
            .unwrap();
        assert_eq!(id, id2, "upsert must not create a second entry");

        let found = svc
            .search_document_find("public", "skill", "probe-skill", "1.0.0")
            .await
            .unwrap()
            .expect("document must exist");
        assert_eq!(found.source_digest, "digest-2");

        // Chunks are replaced, not appended.
        let written = svc
            .search_chunk_replace(
                "public",
                "skill",
                "probe-skill",
                "1.0.0",
                &[chunk("a"), chunk("b")],
            )
            .await
            .unwrap();
        assert_eq!(written, 2);

        let chunks = svc
            .search_chunk_list("public", "skill", "probe-skill", "1.0.0")
            .await
            .unwrap();
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|c| c.document_id == id));

        svc.search_chunk_replace("public", "skill", "probe-skill", "1.0.0", &[chunk("c")])
            .await
            .unwrap();
        let chunks = svc
            .search_chunk_list("public", "skill", "probe-skill", "1.0.0")
            .await
            .unwrap();
        assert_eq!(chunks.len(), 1, "replace must not append");
        assert_eq!(chunks[0].chunk_hash, "hash-c");

        // Tasks: upsert, update in place, find due, delete.
        let now = chrono::Utc::now().timestamp_millis();
        svc.task_upsert(&task("due", now - 1_000)).await.unwrap();
        svc.task_upsert(&task("future", now + 60_000))
            .await
            .unwrap();

        let stored = svc.task_find("due").await.unwrap().expect("task exists");
        assert_eq!(stored.task_stage, "base_index");

        let mut advanced = task("due", now - 1_000);
        advanced.task_stage = "llm_enhancement".to_string();
        svc.task_upsert(&advanced).await.unwrap();
        let stored = svc.task_find("due").await.unwrap().expect("task exists");
        assert_eq!(stored.task_stage, "llm_enhancement", "upsert must update");

        let due = svc.task_find_due("search_index", now, 100).await.unwrap();
        assert_eq!(due.len(), 1, "only the due task should be returned");
        assert_eq!(due[0].task_key, "due");

        assert_eq!(svc.task_delete("due").await.unwrap(), 1);
        assert!(svc.task_find("due").await.unwrap().is_none());

        // Delete the document.
        assert_eq!(
            svc.search_document_delete("public", "skill", "probe-skill", "1.0.0")
                .await
                .unwrap(),
            1
        );
        assert!(
            svc.search_document_find("public", "skill", "probe-skill", "1.0.0")
                .await
                .unwrap()
                .is_none()
        );
    }

    /// Two workers holding the same snapshot must not both take the lease.
    ///
    /// Within one process the read-modify-write lock and the due check would
    /// also reject the second claim; the cross-node case, where both nodes see
    /// the same due task and only the compare-and-set separates them, is
    /// covered by `ai_resource_replication::two_nodes_cannot_both_claim_one_index_task`.
    #[tokio::test]
    async fn test_embedded_task_claim_is_exclusive() {
        let (svc, _tmp) = create_test_service().await;
        let now = chrono::Utc::now().timestamp_millis();
        svc.task_upsert(&task("claimed", now - 1_000))
            .await
            .unwrap();
        let snapshot = svc.task_find("claimed").await.unwrap().expect("task");

        let first = svc.task_claim(&snapshot, 60_000, now).await.unwrap();
        let first = first.expect("the first claim must win");
        assert_eq!(first.status, "processing");
        assert_eq!(first.lease_token, snapshot.lease_token + 1);
        assert_eq!(first.lease_expire_at, Some(now + 60_000));

        // The loser still holds the pre-claim snapshot, which no longer matches.
        let second = svc.task_claim(&snapshot, 60_000, now).await.unwrap();
        assert!(
            second.is_none(),
            "a stale snapshot must not take the lease from its owner"
        );
    }

    /// A task that is not due yet cannot be claimed ahead of time.
    #[tokio::test]
    async fn test_embedded_task_claim_respects_the_schedule() {
        let (svc, _tmp) = create_test_service().await;
        let now = chrono::Utc::now().timestamp_millis();
        svc.task_upsert(&task("future", now + 60_000))
            .await
            .unwrap();
        let snapshot = svc.task_find("future").await.unwrap().expect("task");

        assert!(
            svc.task_claim(&snapshot, 60_000, now)
                .await
                .unwrap()
                .is_none()
        );

        // Once it comes due it is claimable.
        let later = now + 60_001;
        let claimed = svc
            .task_claim(&snapshot, 60_000, later)
            .await
            .unwrap()
            .expect("due now");
        assert_eq!(claimed.status, "processing");
    }

    /// An expired lease is taken over, which is how a crashed worker's task
    /// gets picked up again.
    #[tokio::test]
    async fn test_embedded_task_claim_takes_over_an_expired_lease() {
        let (svc, _tmp) = create_test_service().await;
        let now = chrono::Utc::now().timestamp_millis();
        svc.task_upsert(&task("leased", now - 1_000)).await.unwrap();
        let snapshot = svc.task_find("leased").await.unwrap().expect("task");
        let leased = svc
            .task_claim(&snapshot, 60_000, now)
            .await
            .unwrap()
            .expect("claim");

        // Still inside the lease: the owner keeps it.
        assert!(
            svc.task_claim(&leased, 60_000, now + 1_000)
                .await
                .unwrap()
                .is_none()
        );

        // After the lease expires: takeover.
        let taken = svc
            .task_claim(&leased, 60_000, now + 60_001)
            .await
            .unwrap()
            .expect("expired lease must be taken over");
        assert_eq!(taken.lease_token, leased.lease_token + 1);
    }

    /// Renewing pushes the expiry forward so a long stage keeps the task.
    #[tokio::test]
    async fn test_embedded_task_lease_renewal_extends_the_expiry() {
        let (svc, _tmp) = create_test_service().await;
        let now = chrono::Utc::now().timestamp_millis();
        svc.task_upsert(&task("renew", now - 1_000)).await.unwrap();
        let snapshot = svc.task_find("renew").await.unwrap().expect("task");
        let claimed = svc
            .task_claim(&snapshot, 60_000, now)
            .await
            .unwrap()
            .expect("claim");

        let later = now + 20_000;
        assert!(svc.task_renew_lease(&claimed, 60_000, later).await.unwrap());
        let stored = svc.task_find("renew").await.unwrap().expect("task");
        assert_eq!(stored.lease_expire_at, Some(later + 60_000));

        // Without the renewal the task would have come due again by now.
        assert!(
            svc.task_claim(&claimed, 60_000, later + 30_000)
                .await
                .unwrap()
                .is_none(),
            "a renewed lease must not be claimable"
        );
    }

    /// A lease that has already lapsed cannot be renewed back.
    ///
    /// This is the case that makes renewal safe: once the expiry has passed
    /// another worker may own the task, so a renewal arriving late has to fail
    /// instead of stealing the task back.
    #[tokio::test]
    async fn test_embedded_task_lease_renewal_cannot_resurrect_a_lapsed_lease() {
        let (svc, _tmp) = create_test_service().await;
        let now = chrono::Utc::now().timestamp_millis();
        svc.task_upsert(&task("lapsed", now - 1_000)).await.unwrap();
        let snapshot = svc.task_find("lapsed").await.unwrap().expect("task");
        let first = svc
            .task_claim(&snapshot, 60_000, now)
            .await
            .unwrap()
            .expect("claim");

        // The lease simply runs out — nobody has taken the task over yet, so
        // the token still matches. Renewal has to fail anyway: the task is due
        // again and another worker may be about to claim it.
        assert!(
            !svc.task_renew_lease(&first, 60_000, now + 60_001)
                .await
                .unwrap(),
            "a lapsed lease must not be renewable even before anyone takes over"
        );

        // Once a second worker has taken over, the token no longer matches.
        let second = svc
            .task_claim(&first, 60_000, now + 60_001)
            .await
            .unwrap()
            .expect("expired lease must be taken over");

        assert!(
            !svc.task_renew_lease(&first, 60_000, now + 61_000)
                .await
                .unwrap(),
            "the previous owner must not renew a lease it no longer holds"
        );

        // And the takeover survives: the successor still owns it.
        assert!(
            svc.task_renew_lease(&second, 60_000, now + 61_000)
                .await
                .unwrap()
        );
        let stored = svc.task_find("lapsed").await.unwrap().expect("task");
        assert_eq!(stored.lease_token, second.lease_token);
    }

    /// A worker that lost the lease must not write over its successor.
    #[tokio::test]
    async fn test_embedded_task_writes_are_fenced_by_the_lease_token() {
        let (svc, _tmp) = create_test_service().await;
        let now = chrono::Utc::now().timestamp_millis();
        svc.task_upsert(&task("fenced", now - 1_000)).await.unwrap();
        let snapshot = svc.task_find("fenced").await.unwrap().expect("task");
        let claimed = svc
            .task_claim(&snapshot, 60_000, now)
            .await
            .unwrap()
            .expect("claim");

        let mut done = claimed.clone();
        done.status = "completed".to_string();
        done.lease_token = 0;
        done.lease_expire_at = None;

        // A stale token is rejected and nothing is written.
        assert!(
            !svc.task_update_leased(&done, claimed.lease_token - 1)
                .await
                .unwrap()
        );
        let stored = svc.task_find("fenced").await.unwrap().expect("task");
        assert_eq!(stored.status, "processing", "the stale write must not land");

        // The token the claim handed out is accepted.
        assert!(
            svc.task_update_leased(&done, claimed.lease_token)
                .await
                .unwrap()
        );
        let stored = svc.task_find("fenced").await.unwrap().expect("task");
        assert_eq!(stored.status, "completed");
    }

    /// Deleting a task is fenced the same way an update is.
    #[tokio::test]
    async fn test_embedded_task_delete_is_fenced_by_the_lease_token() {
        let (svc, _tmp) = create_test_service().await;
        let now = chrono::Utc::now().timestamp_millis();
        svc.task_upsert(&task("removable", now - 1_000))
            .await
            .unwrap();
        let snapshot = svc.task_find("removable").await.unwrap().expect("task");
        let claimed = svc
            .task_claim(&snapshot, 60_000, now)
            .await
            .unwrap()
            .expect("claim");

        assert!(
            !svc.task_delete_leased("removable", claimed.lease_token - 1)
                .await
                .unwrap()
        );
        assert!(svc.task_find("removable").await.unwrap().is_some());

        assert!(
            svc.task_delete_leased("removable", claimed.lease_token)
                .await
                .unwrap()
        );
        assert!(svc.task_find("removable").await.unwrap().is_none());
    }
}
