use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entity::apollo_instance;
use crate::persistence::shared::StoredInstance;
use crate::persistence::traits::InstancePersistence;

pub struct InstanceSqlPersistence {
    db: DatabaseConnection,
}

impl InstanceSqlPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<StoredInstance> for apollo_instance::ActiveModel {
    fn from(instance: StoredInstance) -> Self {
        Self {
            id: Set(instance.id),
            app_id: Set(instance.app_id),
            cluster_name: Set(instance.cluster_name),
            data_center: Set(instance.data_center),
            ip: Set(instance.ip),
            data_change_created_time: Set(chrono::DateTime::from_timestamp_millis(
                instance.data_change_created_time,
            )
            .unwrap_or_default()
            .naive_utc()),
            data_change_last_time: Set(instance.data_change_last_time.and_then(|t| {
                chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
            })),
        }
    }
}

impl From<apollo_instance::Model> for StoredInstance {
    fn from(model: apollo_instance::Model) -> Self {
        Self {
            id: model.id,
            app_id: model.app_id,
            cluster_name: model.cluster_name,
            data_center: model.data_center,
            ip: model.ip,
            data_change_created_time: model.data_change_created_time.and_utc().timestamp_millis(),
            data_change_last_time: model.data_change_last_time.map(|t| t.and_utc().timestamp_millis()),
        }
    }
}

#[async_trait]
impl InstancePersistence for InstanceSqlPersistence {
    async fn upsert(&self, instance: StoredInstance) -> anyhow::Result<StoredInstance> {
        // Try to find existing instance by app_id, cluster_name and ip
        let existing = apollo_instance::Entity::find()
            .filter(apollo_instance::Column::AppId.eq(&instance.app_id))
            .filter(apollo_instance::Column::ClusterName.eq(&instance.cluster_name))
            .filter(apollo_instance::Column::Ip.eq(&instance.ip))
            .one(&self.db)
            .await?;

        match existing {
            Some(model) => {
                // Update existing instance
                let mut active_model: apollo_instance::ActiveModel = model.into();
                active_model.data_center = Set(instance.data_center);
                active_model.data_change_last_time = Set(Some(chrono::Utc::now().naive_utc()));
                let updated = active_model.update(&self.db).await?;
                Ok(updated.into())
            }
            None => {
                // Create new instance
                let active_model: apollo_instance::ActiveModel = instance.into();
                let result = apollo_instance::Entity::insert(active_model)
                    .exec(&self.db)
                    .await?;
                let model = apollo_instance::Entity::find_by_id(result.last_insert_id)
                    .one(&self.db)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Failed to fetch created instance"))?;
                Ok(model.into())
            }
        }
    }

    async fn get_by_app(
        &self,
        app_id: &str,
        cluster_name: Option<&str>,
    ) -> anyhow::Result<Vec<StoredInstance>> {
        let mut query = apollo_instance::Entity::find()
            .filter(apollo_instance::Column::AppId.eq(app_id));

        if let Some(cluster) = cluster_name {
            query = query.filter(apollo_instance::Column::ClusterName.eq(cluster));
        }

        let results = query
            .order_by_desc(apollo_instance::Column::DataChangeLastTime)
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn delete_expired(&self, before_timestamp: i64) -> anyhow::Result<usize> {
        let before_time = chrono::DateTime::from_timestamp_millis(before_timestamp)
            .unwrap_or_default();

        let results = apollo_instance::Entity::find()
            .filter(apollo_instance::Column::DataChangeLastTime.lt(before_time))
            .all(&self.db)
            .await?;

        let count = results.len();
        for model in results {
            apollo_instance::Entity::delete_by_id(model.id)
                .exec(&self.db)
                .await?;
        }

        Ok(count)
    }

    async fn list_all(&self) -> anyhow::Result<Vec<StoredInstance>> {
        let results = apollo_instance::Entity::find()
            .order_by_desc(apollo_instance::Column::DataChangeLastTime)
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }
}