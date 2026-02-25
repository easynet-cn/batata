//! Config persistence trait
//!
//! Defines the interface for configuration storage operations.

use async_trait::async_trait;

use crate::model::{ConfigGrayStorageData, ConfigHistoryStorageData, ConfigStorageData, Page};

/// Configuration persistence operations
#[async_trait]
pub trait ConfigPersistence: Send + Sync {
    /// Find a single config by data_id, group, and namespace_id
    async fn config_find_one(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigStorageData>>;

    /// Search configs with pagination and filters
    #[allow(clippy::too_many_arguments)]
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
    ) -> anyhow::Result<Page<ConfigStorageData>>;

    /// Create or update a config
    #[allow(clippy::too_many_arguments)]
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
    ) -> anyhow::Result<bool>;

    /// Delete a config
    #[allow(clippy::too_many_arguments)]
    async fn config_delete(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        gray_name: &str,
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<bool>;

    /// Find gray/beta config
    async fn config_find_gray_one(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigGrayStorageData>>;

    /// Create or update gray/beta config
    #[allow(clippy::too_many_arguments)]
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
    ) -> anyhow::Result<bool>;

    /// Delete gray/beta config
    async fn config_delete_gray(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<bool>;

    /// Batch delete configs by IDs
    async fn config_batch_delete(
        &self,
        ids: &[i64],
        client_ip: &str,
        src_user: &str,
    ) -> anyhow::Result<usize>;

    /// Find a config history entry by its ID (nid)
    async fn config_history_find_by_id(
        &self,
        nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryStorageData>>;

    /// Search config history with pagination
    async fn config_history_search_page(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        page_no: u64,
        page_size: u64,
    ) -> anyhow::Result<Page<ConfigHistoryStorageData>>;

    /// Count configs in a namespace
    async fn config_count_by_namespace(&self, namespace_id: &str) -> anyhow::Result<i32>;

    /// Find all group names in a namespace
    async fn config_find_all_group_names(&self, namespace_id: &str) -> anyhow::Result<Vec<String>>;

    /// Get the previous version of a config history entry
    async fn config_history_get_previous(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        current_nid: u64,
    ) -> anyhow::Result<Option<ConfigHistoryStorageData>>;

    /// Find all configs in a namespace (lightweight, without content)
    async fn config_find_by_namespace(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Vec<ConfigStorageData>>;

    /// Find configs for export with optional filters
    async fn config_find_for_export(
        &self,
        namespace_id: &str,
        group: Option<&str>,
        data_ids: Option<Vec<String>>,
        app_name: Option<&str>,
    ) -> anyhow::Result<Vec<ConfigStorageData>>;
}
