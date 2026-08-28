use async_trait::async_trait;
use sea_orm::{ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::entity::apollo_service_registry;
use crate::persistence::traits::service_registry::{ServiceRegistryEntry, ServiceRegistryPersistence};

pub struct ServiceRegistrySqlPersistence {
    db: DatabaseConnection,
}

impl ServiceRegistrySqlPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

fn to_entry(m: apollo_service_registry::Model) -> ServiceRegistryEntry {
    ServiceRegistryEntry {
        id: m.id,
        service_name: m.service_name,
        uri: m.uri,
        cluster: m.cluster,
        metadata: m.metadata,
        data_change_last_time: m
            .data_change_last_time
            .map(|t| t.and_utc().timestamp_millis())
            .unwrap_or(0),
    }
}

#[async_trait]
impl ServiceRegistryPersistence for ServiceRegistrySqlPersistence {
    async fn heartbeat(
        &self,
        service_name: &str,
        uri: &str,
        cluster: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().naive_utc();
        let existing = apollo_service_registry::Entity::find()
            .filter(apollo_service_registry::Column::ServiceName.eq(service_name))
            .filter(apollo_service_registry::Column::Uri.eq(uri))
            .one(&self.db)
            .await?;
        match existing {
            Some(model) => {
                let mut active: apollo_service_registry::ActiveModel = model.into();
                active.cluster = Set(cluster.to_string());
                active.data_change_last_time = Set(Some(now));
                apollo_service_registry::Entity::update(active)
                    .exec(&self.db)
                    .await?;
            }
            None => {
                let active = apollo_service_registry::ActiveModel {
                    id: Default::default(),
                    service_name: Set(service_name.to_string()),
                    uri: Set(uri.to_string()),
                    cluster: Set(cluster.to_string()),
                    metadata: Set(Some("{}".to_string())),
                    data_change_created_time: Set(now),
                    data_change_last_time: Set(Some(now)),
                };
                let _ = apollo_service_registry::Entity::insert(active)
                    .exec(&self.db)
                    .await; // unique(ServiceName,Uri) may race → ignore dup error
            }
        }
        Ok(())
    }

    async fn deregister(&self, service_name: &str, uri: &str) -> anyhow::Result<()> {
        apollo_service_registry::Entity::delete_many()
            .filter(apollo_service_registry::Column::ServiceName.eq(service_name))
            .filter(apollo_service_registry::Column::Uri.eq(uri))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn find_alive(
        &self,
        service_name: &str,
        window_secs: i64,
    ) -> anyhow::Result<Vec<ServiceRegistryEntry>> {
        use sea_orm::QueryOrder;
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(window_secs);
        let rows = apollo_service_registry::Entity::find()
            .filter(apollo_service_registry::Column::ServiceName.eq(service_name))
            .filter(apollo_service_registry::Column::DataChangeLastTime.gt(Some(cutoff.naive_utc())))
            .order_by_asc(apollo_service_registry::Column::Id)
            .all(&self.db)
            .await?;
        Ok(rows.into_iter().map(to_entry).collect())
    }
}
