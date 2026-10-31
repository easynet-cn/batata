// AI resource persistence for the distributed (Raft cluster) backend
//
// Reads from the local RocksDB state machine via self.reader.
//
// Writes go through Raft. AI resources live in column families of the Raft
// state machine, so writing them locally would leave every node with its own
// private copy: a GET answered by one node would not see what another node
// accepted, and a snapshot install would overwrite a follower's local writes
// with the leader's. Every mutation is therefore proposed as
// `RaftRequest::AiWrite` (or `AiAllocateIds` for the auto-increment counters)
// and applied on every node.

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

use super::DistributedPersistService;
use batata_consistency::raft::request::{AiWriteOp, RaftRequest, RaftResponse};

/// Key for the auto-increment counter stored in the ai_resource CF
const KEY_AI_RESOURCE_NEXT_ID: &str = "__ai_resource_next_id__";
/// Key for the auto-increment counter stored in the ai_resource_version CF
const KEY_AI_RESOURCE_VERSION_NEXT_ID: &str = "__ai_resource_version_next_id__";
/// Key for the auto-increment counter stored in the search document CF
const KEY_SEARCH_DOCUMENT_NEXT_ID: &str = "__search_document_next_id__";
/// Key for the auto-increment counter stored in the search chunk CF
const KEY_SEARCH_CHUNK_NEXT_ID: &str = "__search_chunk_next_id__";

/// Whether the compare-and-set ops in an [`RaftRequest::AiWrite`] all matched.
///
/// The state machine reports the outcome in [`RaftResponse::data`] as a
/// little-endian `i64`. Missing data is treated as a lost comparison so a
/// transport that dropped it can never be read as a successful claim.
fn cas_matched(response: &RaftResponse) -> bool {
    let Some(data) = response.data.as_deref() else {
        return false;
    };
    let bytes: [u8; 8] = match data.try_into() {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    i64::from_le_bytes(bytes) != 0
}

/// Current time as an RFC 3339 string, matching the other AI writers.
fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

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
    async fn next_ai_resource_id(&self) -> anyhow::Result<i64> {
        self.ai_alloc_one_id(CF_AI_RESOURCE, KEY_AI_RESOURCE_NEXT_ID)
            .await
    }

    /// Get the next auto-increment ID for ai_resource_version
    async fn next_ai_resource_version_id(&self) -> anyhow::Result<i64> {
        self.ai_alloc_one_id(CF_AI_RESOURCE_VERSION, KEY_AI_RESOURCE_VERSION_NEXT_ID)
            .await
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
    async fn put_ai_resource(&self, key: &str, info: &AiResourceInfo) -> anyhow::Result<()> {
        let json = serde_json::to_vec(info)?;
        self.ai_apply(vec![AiWriteOp::Put {
            cf: CF_AI_RESOURCE.to_string(),
            key: key.as_bytes().to_vec(),
            value: json,
        }])
        .await
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
    async fn put_ai_resource_version(
        &self,
        key: &str,
        info: &AiResourceVersionInfo,
    ) -> anyhow::Result<()> {
        let json = serde_json::to_vec(info)?;
        self.ai_apply(vec![AiWriteOp::Put {
            cf: CF_AI_RESOURCE_VERSION.to_string(),
            key: key.as_bytes().to_vec(),
            value: json,
        }])
        .await
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

    /// Resolve a column family handle, failing loudly when it is missing.
    fn cf(&self, name: &str) -> anyhow::Result<&rocksdb::ColumnFamily> {
        self.reader
            .db()
            .cf_handle(name)
            .ok_or_else(|| anyhow::anyhow!("Column family '{}' not found", name))
    }

    /// Apply AI storage mutations through Raft.
    ///
    /// One `RaftRequest::AiWrite` carries the whole batch, so a multi-key
    /// operation (replace every chunk of a version, delete every version of a
    /// resource) is replicated atomically instead of key by key.
    async fn ai_apply(&self, ops: Vec<AiWriteOp>) -> anyhow::Result<()> {
        if ops.is_empty() {
            return Ok(());
        }
        self.raft_write(RaftRequest::AiWrite { ops }).await
    }

    /// Overwrite (or, for `updated = None`, delete) one index task, but only
    /// while it still holds exactly `stored`.
    ///
    /// The comparison is evaluated during the Raft apply against the same state
    /// on every node, so two nodes that both read `stored` and both propose a
    /// transition are ordered instead of racing: the loser's comparison fails
    /// and its write is dropped. Returns whether the comparison matched.
    async fn task_write_if_unchanged(
        &self,
        stored: &AiResourceTaskInfo,
        updated: Option<&AiResourceTaskInfo>,
    ) -> anyhow::Result<bool> {
        let key = stored.task_key.as_bytes().to_vec();
        let expected = Some(serde_json::to_vec(stored)?);
        let op = match updated {
            Some(task) => AiWriteOp::PutIfUnchanged {
                cf: CF_AI_RESOURCE_TASK.to_string(),
                key,
                expected,
                value: serde_json::to_vec(task)?,
            },
            None => AiWriteOp::DeleteIfUnchanged {
                cf: CF_AI_RESOURCE_TASK.to_string(),
                key,
                expected,
            },
        };
        let response = self
            .raft_write_response(RaftRequest::AiWrite { ops: vec![op] })
            .await?;
        Ok(cas_matched(&response))
    }

    /// Allocate consecutive IDs from an AI auto-increment counter.
    ///
    /// The bump happens during apply, so the IDs are handed out under the same
    /// total order as the writes that use them.
    async fn ai_alloc_ids(
        &self,
        cf: &str,
        counter_key: &str,
        count: u32,
    ) -> anyhow::Result<Vec<i64>> {
        let response = self
            .raft_node
            .write(RaftRequest::AiAllocateIds {
                cf: cf.to_string(),
                counter_key: counter_key.to_string(),
                count,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Raft write failed: {e}"))?;

        if !response.success {
            return Err(anyhow::anyhow!(
                "Raft write rejected: {}",
                response.message.unwrap_or_default()
            ));
        }

        let data = response.data.unwrap_or_default();
        if data.len() != count as usize * std::mem::size_of::<i64>() {
            return Err(anyhow::anyhow!(
                "AI id allocation returned {} bytes, expected {}",
                data.len(),
                count as usize * std::mem::size_of::<i64>()
            ));
        }

        Ok(data
            .chunks_exact(std::mem::size_of::<i64>())
            .map(|chunk| {
                let mut bytes = [0u8; std::mem::size_of::<i64>()];
                bytes.copy_from_slice(chunk);
                i64::from_le_bytes(bytes)
            })
            .collect())
    }

    /// Allocate a single ID from an AI auto-increment counter.
    async fn ai_alloc_one_id(&self, cf: &str, counter_key: &str) -> anyhow::Result<i64> {
        let ids = self.ai_alloc_ids(cf, counter_key, 1).await?;
        ids.into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("AI id allocation returned no id"))
    }

    /// Get the next auto-increment ID for ai_resource_search_document
    async fn next_search_document_id(&self) -> anyhow::Result<i64> {
        self.ai_alloc_one_id(CF_AI_RESOURCE_SEARCH_DOCUMENT, KEY_SEARCH_DOCUMENT_NEXT_ID)
            .await
    }

    /// Read one ai_resource_search_document entry
    fn get_search_document(
        &self,
        key: &str,
    ) -> anyhow::Result<Option<AiResourceSearchDocumentInfo>> {
        let db = self.reader.db();
        let cf = self.cf(CF_AI_RESOURCE_SEARCH_DOCUMENT)?;
        match db.get_cf(cf, key.as_bytes())? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Write one ai_resource_search_document entry
    async fn put_search_document(
        &self,
        key: &str,
        info: &AiResourceSearchDocumentInfo,
    ) -> anyhow::Result<()> {
        let json = serde_json::to_vec(info)?;
        self.ai_apply(vec![AiWriteOp::Put {
            cf: CF_AI_RESOURCE_SEARCH_DOCUMENT.to_string(),
            key: key.as_bytes().to_vec(),
            value: json,
        }])
        .await
    }

    /// Scan the chunks stored under a resource-version prefix
    fn scan_search_chunks(&self, prefix: &str) -> anyhow::Result<Vec<AiResourceSearchChunkInfo>> {
        let db = self.reader.db();
        let cf = self.cf(CF_AI_RESOURCE_SEARCH_CHUNK)?;
        let mut results = Vec::new();
        for item in db.prefix_iterator_cf(cf, prefix.as_bytes()) {
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

    /// Count the chunks stored under a resource-version prefix.
    ///
    /// `DeletePrefix` is applied on every node and cannot report a count, so
    /// the caller reads it here. Reads go to this node's copy of the state
    /// machine, which is the leader's state for any write path.
    fn count_search_chunks(&self, prefix: &str) -> anyhow::Result<u64> {
        let db = self.reader.db();
        let cf = self.cf(CF_AI_RESOURCE_SEARCH_CHUNK)?;
        let mut found = 0u64;
        for item in db.prefix_iterator_cf(cf, prefix.as_bytes()) {
            let (key, _) = item.map_err(|e| anyhow::anyhow!("RocksDB iterator error: {}", e))?;
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with(prefix) {
                break;
            }
            if key_str.starts_with("__") {
                continue;
            }
            found += 1;
        }
        Ok(found)
    }

    /// Build the op that drops every chunk stored under a prefix
    fn delete_prefix_op(cf: &str, prefix: &str) -> AiWriteOp {
        AiWriteOp::DeletePrefix {
            cf: cf.to_string(),
            prefix: prefix.as_bytes().to_vec(),
        }
    }

    /// Read one ai_resource_task entry
    fn get_task(&self, task_key: &str) -> anyhow::Result<Option<AiResourceTaskInfo>> {
        let db = self.reader.db();
        let cf = self.cf(CF_AI_RESOURCE_TASK)?;
        match db.get_cf(cf, task_key.as_bytes())? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Write one ai_resource_task entry
    async fn put_task(&self, task: &AiResourceTaskInfo) -> anyhow::Result<()> {
        let json = serde_json::to_vec(task)?;
        self.ai_apply(vec![AiWriteOp::Put {
            cf: CF_AI_RESOURCE_TASK.to_string(),
            key: task.task_key.as_bytes().to_vec(),
            value: json,
        }])
        .await
    }

    /// Scan every stored task (empty prefix iterates the whole column family)
    fn scan_tasks(&self) -> anyhow::Result<Vec<AiResourceTaskInfo>> {
        let db = self.reader.db();
        let cf = self.cf(CF_AI_RESOURCE_TASK)?;
        let mut results = Vec::new();
        for item in db.prefix_iterator_cf(cf, b"") {
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
        let key = Self::ai_resource_key(namespace_id, resource_type, name);
        self.get_ai_resource(&key)
    }

    async fn ai_resource_insert(&self, resource: &AiResourceInfo) -> anyhow::Result<i64> {
        let id = self.next_ai_resource_id().await?;
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
        self.put_ai_resource(&key, &info).await?;
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
                self.put_ai_resource(&key, &info).await?;
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
                self.put_ai_resource(&key, &info).await
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
                self.put_ai_resource(&key, &info).await
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
                self.put_ai_resource(&key, &info).await
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
                self.put_ai_resource(&key, &info).await
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
                self.put_ai_resource(&key, &info).await
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
        if self.get_ai_resource(&key)?.is_some() {
            self.ai_apply(vec![AiWriteOp::Delete {
                cf: CF_AI_RESOURCE.to_string(),
                key: key.into_bytes(),
            }])
            .await?;
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
        let id = self.next_ai_resource_version_id().await?;
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
        self.put_ai_resource_version(&key, &info).await?;
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
                self.put_ai_resource_version(&key, &info).await
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
                self.put_ai_resource_version(&key, &info).await
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
                self.put_ai_resource_version(&key, &info).await
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
        if self.get_ai_resource_version(&key)?.is_some() {
            self.ai_apply(vec![AiWriteOp::Delete {
                cf: CF_AI_RESOURCE_VERSION.to_string(),
                key: key.into_bytes(),
            }])
            .await?;
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
        let versions = self.scan_ai_resource_versions(&prefix)?;
        if versions.is_empty() {
            return Ok(0);
        }
        self.ai_apply(vec![Self::delete_prefix_op(
            CF_AI_RESOURCE_VERSION,
            &prefix,
        )])
        .await?;
        Ok(versions.len() as u64)
    }

    async fn ai_resource_version_delete_by_status(
        &self,
        namespace_id: &str,
        name: &str,
        resource_type: &str,
        status: &str,
    ) -> anyhow::Result<u64> {
        let prefix = Self::ai_resource_version_prefix(namespace_id, resource_type, name);
        let versions = self.scan_ai_resource_versions(&prefix)?;
        let doomed: Vec<AiWriteOp> = versions
            .iter()
            .filter(|v| v.status == status)
            .map(|v| AiWriteOp::Delete {
                cf: CF_AI_RESOURCE_VERSION.to_string(),
                key: Self::ai_resource_version_key(namespace_id, resource_type, name, &v.version)
                    .into_bytes(),
            })
            .collect();
        let count = doomed.len() as u64;
        self.ai_apply(doomed).await?;
        Ok(count)
    }

    // ========================================================================
    // pipeline_execution operations
    // ========================================================================

    async fn pipeline_execution_find(
        &self,
        execution_id: &str,
    ) -> anyhow::Result<Option<PipelineExecutionInfo>> {
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
                self.put_search_document(&key, &updated).await?;
                Ok(existing.id)
            }
            None => {
                let id = self.next_search_document_id().await?;
                let mut created = document.clone();
                created.id = id;
                created.gmt_create = Some(now.clone());
                created.gmt_modified = Some(now);
                self.put_search_document(&key, &created).await?;
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
        self.ai_apply(vec![AiWriteOp::Delete {
            cf: CF_AI_RESOURCE_SEARCH_DOCUMENT.to_string(),
            key: key.into_bytes(),
        }])
        .await?;
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

        if chunks.is_empty() {
            // Still replicated: the previous chunks have to go away on every
            // node, not just this one.
            self.ai_apply(vec![Self::delete_prefix_op(
                CF_AI_RESOURCE_SEARCH_CHUNK,
                &prefix,
            )])
            .await?;
            return Ok(0);
        }

        let document_id = self
            .search_document_find(namespace_id, resource_type, resource_name, resource_version)
            .await?
            .map(|d| d.id)
            .unwrap_or_default();

        // Allocate every ID in one Raft write; the chunks then go out as a
        // single batch, so a version is never half-replaced on any node.
        let ids = self
            .ai_alloc_ids(
                CF_AI_RESOURCE_SEARCH_CHUNK,
                KEY_SEARCH_CHUNK_NEXT_ID,
                chunks.len() as u32,
            )
            .await?;

        let now = chrono::Utc::now().to_rfc3339();
        let mut ops = Vec::with_capacity(chunks.len() + 1);
        ops.push(Self::delete_prefix_op(CF_AI_RESOURCE_SEARCH_CHUNK, &prefix));
        for (chunk, id) in chunks.iter().zip(ids) {
            let mut stored = chunk.clone();
            stored.id = id;
            stored.document_id = document_id;
            stored.gmt_create = Some(now.clone());
            stored.gmt_modified = Some(now.clone());
            ops.push(AiWriteOp::Put {
                cf: CF_AI_RESOURCE_SEARCH_CHUNK.to_string(),
                key: format!("{prefix}{id}").into_bytes(),
                value: serde_json::to_vec(&stored)?,
            });
        }
        self.ai_apply(ops).await?;
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
        let deleted = self.count_search_chunks(&prefix)?;
        self.ai_apply(vec![Self::delete_prefix_op(
            CF_AI_RESOURCE_SEARCH_CHUNK,
            &prefix,
        )])
        .await?;
        Ok(deleted)
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
        let now = chrono::Utc::now().to_rfc3339();
        let mut stored = task.clone();
        match self.get_task(&task.task_key)? {
            Some(existing) => {
                stored.gmt_create = existing.gmt_create;
                stored.gmt_modified = Some(now);
                // See `AiResourcePersistence::task_upsert`: overwriting an
                // existing task retires whatever lease is held on it.
                stored.revision = existing.revision + 1;
            }
            None => {
                stored.gmt_create = Some(now.clone());
                stored.gmt_modified = Some(now);
            }
        }
        self.put_task(&stored).await
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
        if self.get_task(task_key)?.is_none() {
            return Ok(0);
        }
        self.ai_apply(vec![AiWriteOp::Delete {
            cf: CF_AI_RESOURCE_TASK.to_string(),
            key: task_key.as_bytes().to_vec(),
        }])
        .await?;
        Ok(1)
    }

    async fn task_claim(
        &self,
        task: &AiResourceTaskInfo,
        lease_millis: i64,
        now_millis: i64,
    ) -> anyhow::Result<Option<AiResourceTaskInfo>> {
        // The lease checks run here on a local read, but they are only a
        // filter: what makes the claim exclusive is that the write carries the
        // bytes this node read, so a node that lost the race simply does not
        // write. Both nodes propose; the apply order decides the winner.
        let Some(stored) = self.get_task(&task.task_key)? else {
            return Ok(None);
        };
        if stored.revision != task.revision
            || stored.lease_token != task.lease_token
            || !task_is_due(&stored, now_millis)
        {
            return Ok(None);
        }

        let mut claimed = stored.clone();
        claimed.status = TASK_STATUS_PROCESSING.to_string();
        claimed.lease_token = stored.lease_token + 1;
        claimed.lease_expire_at = Some(now_millis + lease_millis);
        claimed.gmt_modified = Some(now_rfc3339());
        let landed = self
            .task_write_if_unchanged(&stored, Some(&claimed))
            .await?;
        Ok(landed.then_some(claimed))
    }

    async fn task_renew_lease(
        &self,
        task: &AiResourceTaskInfo,
        lease_millis: i64,
        now_millis: i64,
    ) -> anyhow::Result<bool> {
        // As on the other backends, holding the token is not enough: a lease
        // whose expiry has passed may already belong to another node, and a
        // renewal that succeeded anyway would steal it back.
        let Some(stored) = self.get_task(&task.task_key)? else {
            return Ok(false);
        };
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
        self.task_write_if_unchanged(&stored, Some(&renewed)).await
    }

    async fn task_update_leased(
        &self,
        task: &AiResourceTaskInfo,
        lease_token: i64,
    ) -> anyhow::Result<bool> {
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
        updated.gmt_create = stored.gmt_create.clone();
        updated.gmt_modified = Some(now_rfc3339());
        self.task_write_if_unchanged(&stored, Some(&updated)).await
    }

    async fn task_delete_leased(&self, task_key: &str, lease_token: i64) -> anyhow::Result<bool> {
        let Some(stored) = self.get_task(task_key)? else {
            return Ok(false);
        };
        if stored.lease_token != lease_token || stored.status != TASK_STATUS_PROCESSING {
            return Ok(false);
        }
        self.task_write_if_unchanged(&stored, None).await
    }
}
