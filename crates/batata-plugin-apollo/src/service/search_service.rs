use std::sync::Arc;

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, QueryOrder};
use crate::entity::{apollo_item, apollo_namespace};
use crate::api::dto::SearchDTO;
use crate::persistence::traits::ApolloPersistenceService;

pub struct SearchService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl SearchService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    fn db(&self) -> Result<DatabaseConnection, anyhow::Error> {
        self.persistence.get_db_connection().ok_or_else(|| anyhow::anyhow!("Database connection not available"))
    }

    pub async fn search_items(&self, app_id: &str, cluster_name: &str, key: Option<&str>, value: Option<&str>) -> Result<Vec<SearchDTO>, anyhow::Error> {
        let db = self.db()?;
        let namespaces = apollo_namespace::Entity::find()
            .filter(apollo_namespace::Column::AppId.eq(app_id))
            .filter(apollo_namespace::Column::ClusterName.eq(cluster_name))
            .filter(apollo_namespace::Column::IsDeleted.eq(false))
            .all(&db)
            .await?;

        let mut results = Vec::new();

        for ns in namespaces {
            let mut query = apollo_item::Entity::find()
                .filter(apollo_item::Column::NamespaceId.eq(ns.id))
                .filter(apollo_item::Column::IsDeleted.eq(false));

            if let Some(k) = key {
                if !k.is_empty() {
                    query = query.filter(apollo_item::Column::Key.like(format!("%{}%", k)));
                }
            }

            if let Some(v) = value {
                if !v.is_empty() {
                    query = query.filter(apollo_item::Column::Value.like(format!("%{}%", v)));
                }
            }

            let items = query.order_by_asc(apollo_item::Column::Key).all(&db).await?;

            for item in items {
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

    pub async fn search_across_apps(&self, key: Option<&str>, value: Option<&str>) -> Result<Vec<SearchDTO>, anyhow::Error> {
        let db = self.db()?;
        let namespaces = apollo_namespace::Entity::find()
            .filter(apollo_namespace::Column::IsDeleted.eq(false))
            .all(&db)
            .await?;

        let mut results = Vec::new();

        for ns in namespaces {
            let mut query = apollo_item::Entity::find()
                .filter(apollo_item::Column::NamespaceId.eq(ns.id))
                .filter(apollo_item::Column::IsDeleted.eq(false));

            if let Some(k) = key {
                if !k.is_empty() {
                    query = query.filter(apollo_item::Column::Key.like(format!("%{}%", k)));
                }
            }

            if let Some(v) = value {
                if !v.is_empty() {
                    query = query.filter(apollo_item::Column::Value.like(format!("%{}%", v)));
                }
            }

            let items = query.order_by_asc(apollo_item::Column::Key).all(&db).await?;

            for item in items {
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
