use std::sync::Arc;

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, Set, ActiveModelTrait, ColumnTrait, QueryOrder};
use crate::entity::apollo_instance_config;
use crate::api::dto::InstanceConfigDTO;
use crate::persistence::traits::ApolloPersistenceService;
use chrono::Utc;

pub struct InstanceConfigService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl InstanceConfigService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    fn db(&self) -> Result<DatabaseConnection, anyhow::Error> {
        self.persistence.get_db_connection().ok_or_else(|| anyhow::anyhow!("Database connection not available"))
    }

    pub async fn create_or_update(&self, dto: InstanceConfigDTO) -> Result<InstanceConfigDTO, anyhow::Error> {
        let db = self.db()?;
        let existing = apollo_instance_config::Entity::find()
            .filter(apollo_instance_config::Column::InstanceId.eq(dto.instance_id))
            .filter(apollo_instance_config::Column::NamespaceName.eq(&dto.namespace_name))
            .filter(apollo_instance_config::Column::ClusterName.eq(&dto.cluster_name))
            .one(&db)
            .await?;

        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "system".to_string());

        if let Some(model) = existing {
            let mut active_model: apollo_instance_config::ActiveModel = model.into();
            active_model.release_key = Set(dto.release_key);
            active_model.configurations = Set(dto.configurations);
            active_model.data_change_last_modified_by = Set(Some(created_by));
            active_model.data_change_last_time = Set(Some(now));
            let updated = active_model.update(&db).await?;
            return Ok(self.model_to_dto(&updated));
        }

        let active_model = apollo_instance_config::ActiveModel {
            instance_id: Set(dto.instance_id),
            namespace_name: Set(dto.namespace_name),
            cluster_name: Set(dto.cluster_name),
            release_key: Set(dto.release_key),
            configurations: Set(dto.configurations),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };

        let model = active_model.insert(&db).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn get_by_instance(&self, instance_id: i32) -> Result<Vec<InstanceConfigDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_instance_config::Entity::find()
            .filter(apollo_instance_config::Column::InstanceId.eq(instance_id))
            .order_by_desc(apollo_instance_config::Column::DataChangeLastTime)
            .all(&db)
            .await?;

        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn list_by_app_cluster(&self, _app_id: &str, cluster_name: &str, _namespace_name: &str) -> Result<Vec<InstanceConfigDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_instance_config::Entity::find()
            .filter(apollo_instance_config::Column::ClusterName.eq(cluster_name))
            .order_by_desc(apollo_instance_config::Column::DataChangeLastTime)
            .all(&db)
            .await?;

        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn delete_by_instance(&self, instance_id: i32) -> Result<(), anyhow::Error> {
        let db = self.db()?;
        apollo_instance_config::Entity::delete_many()
            .filter(apollo_instance_config::Column::InstanceId.eq(instance_id))
            .exec(&db)
            .await?;
        Ok(())
    }

    fn model_to_dto(&self, model: &apollo_instance_config::Model) -> InstanceConfigDTO {
        InstanceConfigDTO {
            id: Some(model.id),
            instance_id: model.instance_id,
            namespace_name: model.namespace_name.clone(),
            cluster_name: model.cluster_name.clone(),
            release_key: model.release_key.clone(),
            configurations: model.configurations.clone(),
            data_change_created_by: Some(model.data_change_created_by.clone()),
            data_change_created_time: Some(model.data_change_created_time.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()),
            data_change_last_time: model.data_change_last_time.map(|t| t.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()),
        }
    }
}
