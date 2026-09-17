use std::sync::Arc;

use crate::api::dto::SearchDTO;
use crate::persistence::traits::{ApolloPersistenceService, ItemPersistence, NamespacePersistence};

/// Represents the `SearchService` entity.
pub struct SearchService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl SearchService {
    /// Creates a new `SearchService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Returns the requested value.
    pub async fn search_items(&self, app_id: &str, cluster_name: &str, key: Option<&str>, value: Option<&str>) -> Result<Vec<SearchDTO>, anyhow::Error> {
        let namespaces = self.persistence.list_by_app(app_id).await?;
        let mut results = Vec::new();
        for ns in namespaces {
            if ns.cluster_name != cluster_name || ns.is_deleted {
                continue;
            }
            let items = self.persistence.list_by_namespace(ns.id).await?;
            for item in items {
                if item.is_deleted {
                    continue;
                }
                if let Some(k) = key
                    && !k.is_empty() && !item.key.contains(k) {
                        continue;
                    }
                if let Some(v) = value
                    && !v.is_empty() && !item.value.contains(v) {
                        continue;
                    }
                results.push(SearchDTO {
                    app_id: app_id.to_string(),
                    cluster_name: cluster_name.to_string(),
                    namespace_name: ns.namespace_name.clone(),
                    key: item.key,
                    value: item.value,
                });
            }
        }
        Ok(results)
    }

    /// Returns the requested value.
    pub async fn search_across_apps(&self, key: Option<&str>, value: Option<&str>) -> Result<Vec<SearchDTO>, anyhow::Error> {
        let namespaces = self.persistence.list_all().await?;
        let mut results = Vec::new();
        for ns in namespaces {
            if ns.is_deleted {
                continue;
            }
            let items = self.persistence.list_by_namespace(ns.id).await?;
            for item in items {
                if item.is_deleted {
                    continue;
                }
                if let Some(k) = key
                    && !k.is_empty() && !item.key.contains(k) {
                        continue;
                    }
                if let Some(v) = value
                    && !v.is_empty() && !item.value.contains(v) {
                        continue;
                    }
                results.push(SearchDTO {
                    app_id: ns.app_id.clone(),
                    cluster_name: ns.cluster_name.clone(),
                    namespace_name: ns.namespace_name.clone(),
                    key: item.key,
                    value: item.value,
                });
            }
        }
        Ok(results)
    }
}
