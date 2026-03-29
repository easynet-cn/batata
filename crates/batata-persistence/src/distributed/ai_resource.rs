// AI resource persistence for the distributed (Raft cluster) backend
//
// Reads from local RocksDB state machine via self.reader.
// Writes go directly to the local RocksDB (same pattern as CapacityPersistence)
// because AI resource operations are not yet part of the RaftRequest enum.
// When Raft-replicated AI resource writes are needed, add corresponding
// RaftRequest variants and state machine apply methods, then switch writes
// to self.raft_write().

use async_trait::async_trait;

use batata_consistency::raft::state_machine::{
    CF_AI_RESOURCE, CF_AI_RESOURCE_VERSION, CF_PIPELINE_EXECUTION,
};

use crate::model::{AiResourceInfo, AiResourceVersionInfo, Page, PipelineExecutionInfo};
use crate::traits::ai_resource::AiResourcePersistence;

use super::DistributedPersistService;

/// Key for the auto-increment counter stored in the ai_resource CF
const KEY_AI_RESOURCE_NEXT_ID: &str = "__ai_resource_next_id__";
/// Key for the auto-increment counter stored in the ai_resource_version CF
const KEY_AI_RESOURCE_VERSION_NEXT_ID: &str = "__ai_resource_version_next_id__";

impl DistributedPersistService {
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
        let db = self.reader.db();
        let cf = db
            .cf_handle(CF_AI_RESOURCE)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE))?;
        let current = match db.get_cf(cf, KEY_AI_RESOURCE_NEXT_ID.as_bytes())? {
            Some(bytes) => {
                let s = String::from_utf8(bytes.to_vec())?;
                s.parse::<i64>()?
            }
            None => 0,
        };
        let next = current + 1;
        db.put_cf(
            cf,
            KEY_AI_RESOURCE_NEXT_ID.as_bytes(),
            next.to_string().as_bytes(),
        )?;
        Ok(next)
    }

    /// Get the next auto-increment ID for ai_resource_version
    fn next_ai_resource_version_id(&self) -> anyhow::Result<i64> {
        let db = self.reader.db();
        let cf = db.cf_handle(CF_AI_RESOURCE_VERSION).ok_or_else(|| {
            anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE_VERSION)
        })?;
        let current = match db.get_cf(cf, KEY_AI_RESOURCE_VERSION_NEXT_ID.as_bytes())? {
            Some(bytes) => {
                let s = String::from_utf8(bytes.to_vec())?;
                s.parse::<i64>()?
            }
            None => 0,
        };
        let next = current + 1;
        db.put_cf(
            cf,
            KEY_AI_RESOURCE_VERSION_NEXT_ID.as_bytes(),
            next.to_string().as_bytes(),
        )?;
        Ok(next)
    }

    /// Read an AiResourceInfo from the ai_resource CF
    fn get_ai_resource(&self, key: &str) -> anyhow::Result<Option<AiResourceInfo>> {
        let db = self.reader.db();
        let cf = db
            .cf_handle(CF_AI_RESOURCE)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE))?;
        match db.get_cf(cf, key.as_bytes())? {
            Some(bytes) => {
                let info: AiResourceInfo = serde_json::from_slice(&bytes)?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    /// Write an AiResourceInfo to the ai_resource CF
    fn put_ai_resource(&self, key: &str, info: &AiResourceInfo) -> anyhow::Result<()> {
        let db = self.reader.db();
        let cf = db
            .cf_handle(CF_AI_RESOURCE)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE))?;
        let json = serde_json::to_vec(info)?;
        db.put_cf(cf, key.as_bytes(), &json)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))
    }

    /// Read an AiResourceVersionInfo from the ai_resource_version CF
    fn get_ai_resource_version(&self, key: &str) -> anyhow::Result<Option<AiResourceVersionInfo>> {
        let db = self.reader.db();
        let cf = db.cf_handle(CF_AI_RESOURCE_VERSION).ok_or_else(|| {
            anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE_VERSION)
        })?;
        match db.get_cf(cf, key.as_bytes())? {
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
        let db = self.reader.db();
        let cf = db.cf_handle(CF_AI_RESOURCE_VERSION).ok_or_else(|| {
            anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE_VERSION)
        })?;
        let json = serde_json::to_vec(info)?;
        db.put_cf(cf, key.as_bytes(), &json)
            .map_err(|e| anyhow::anyhow!("RocksDB put error: {}", e))
    }

    /// Scan all ai_resource entries matching a prefix
    fn scan_ai_resources(&self, prefix: &str) -> anyhow::Result<Vec<AiResourceInfo>> {
        let db = self.reader.db();
        let cf = db
            .cf_handle(CF_AI_RESOURCE)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE))?;
        let mut results = Vec::new();
        let iter = db.prefix_iterator_cf(cf, prefix.as_bytes());
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
        let db = self.reader.db();
        let cf = db.cf_handle(CF_AI_RESOURCE_VERSION).ok_or_else(|| {
            anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE_VERSION)
        })?;
        let mut results = Vec::new();
        let iter = db.prefix_iterator_cf(cf, prefix.as_bytes());
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
}

#[async_trait]
impl AiResourcePersistence for DistributedPersistService {
    // ========================================================================
    // ai_resource operations
    // ========================================================================

    async fn ai_resource_find(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
    ) -> anyhow::Result<Option<AiResourceInfo>> {
        self.ensure_consistent_read().await?;
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
        let db = self.reader.db();
        let cf = db
            .cf_handle(CF_AI_RESOURCE)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE))?;
        if db.get_cf(cf, key.as_bytes())?.is_some() {
            db.delete_cf(cf, key.as_bytes())
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
        name_filter: Option<&str>,
        search_accurate: bool,
        order_by_downloads: bool,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<AiResourceInfo>> {
        self.ensure_consistent_read().await?;
        let prefix = format!("{}:{}:", namespace_id, resource_type);
        let mut items = self.scan_ai_resources(&prefix)?;

        // Apply name filter
        if let Some(filter) = name_filter
            && !filter.is_empty()
        {
            items.retain(|item| {
                if search_accurate {
                    item.name == filter
                } else {
                    item.name.contains(filter)
                }
            });
        }

        // Sort
        if order_by_downloads {
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
        self.ensure_consistent_read().await?;
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
        self.ensure_consistent_read().await?;
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
        self.ensure_consistent_read().await?;
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
        self.ensure_consistent_read().await?;
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
        let db = self.reader.db();
        let cf = db.cf_handle(CF_AI_RESOURCE_VERSION).ok_or_else(|| {
            anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE_VERSION)
        })?;
        if db.get_cf(cf, key.as_bytes())?.is_some() {
            db.delete_cf(cf, key.as_bytes())
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
        let db = self.reader.db();
        let cf = db.cf_handle(CF_AI_RESOURCE_VERSION).ok_or_else(|| {
            anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE_VERSION)
        })?;
        let mut count = 0u64;
        let mut keys_to_delete = Vec::new();
        let iter = db.prefix_iterator_cf(cf, prefix.as_bytes());
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
            db.delete_cf(cf, key)
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
        let db = self.reader.db();
        let cf = db.cf_handle(CF_AI_RESOURCE_VERSION).ok_or_else(|| {
            anyhow::anyhow!("Column family '{}' not found", CF_AI_RESOURCE_VERSION)
        })?;
        let mut count = 0u64;
        let mut keys_to_delete = Vec::new();
        let iter = db.prefix_iterator_cf(cf, prefix.as_bytes());
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
            db.delete_cf(cf, key)
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
        self.ensure_consistent_read().await?;
        let db = self.reader.db();
        let cf = db.cf_handle(CF_PIPELINE_EXECUTION).ok_or_else(|| {
            anyhow::anyhow!("Column family '{}' not found", CF_PIPELINE_EXECUTION)
        })?;
        match db.get_cf(cf, execution_id.as_bytes())? {
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
        self.ensure_consistent_read().await?;
        let db = self.reader.db();
        let cf = db.cf_handle(CF_PIPELINE_EXECUTION).ok_or_else(|| {
            anyhow::anyhow!("Column family '{}' not found", CF_PIPELINE_EXECUTION)
        })?;
        let mut items = Vec::new();
        let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
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
}
