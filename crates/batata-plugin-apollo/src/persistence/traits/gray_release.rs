use async_trait::async_trait;

use crate::persistence::shared::StoredGrayReleaseRule;

#[async_trait]
pub trait GrayReleasePersistence: Send + Sync {
    async fn create(&self, rule: StoredGrayReleaseRule) -> anyhow::Result<StoredGrayReleaseRule>;
    async fn get_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredGrayReleaseRule>>;
    async fn update_rules(
        &self,
        id: i32,
        rules: String,
        release_id: i64,
    ) -> anyhow::Result<StoredGrayReleaseRule>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredGrayReleaseRule>>;
}