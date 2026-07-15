use std::sync::Arc;

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, Set, ActiveModelTrait, ColumnTrait};
use crate::entity::{apollo_role, apollo_role_permission, apollo_user_role};
use crate::api::dto::RoleDTO;
use crate::persistence::traits::ApolloPersistenceService;
use chrono::Utc;

pub struct RoleService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl RoleService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    fn db(&self) -> Result<DatabaseConnection, anyhow::Error> {
        self.persistence.get_db_connection().ok_or_else(|| anyhow::anyhow!("Database connection not available"))
    }

    pub async fn create(&self, dto: RoleDTO) -> Result<RoleDTO, anyhow::Error> {
        let db = self.db()?;
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "admin".to_string());

        let active_model = apollo_role::ActiveModel {
            role_name: Set(dto.role_name),
            role_type: Set(dto.role_type),
            target_id: Set(dto.target_id),
            is_deleted: Set(false),
            deleted_at: Set(0),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        let model = active_model.insert(&db).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn get(&self, id: i32) -> Result<Option<RoleDTO>, anyhow::Error> {
        let db = self.db()?;
        let model = apollo_role::Entity::find_by_id(id)
            .filter(apollo_role::Column::IsDeleted.eq(false))
            .one(&db)
            .await?;
        Ok(model.map(|m| self.model_to_dto(&m)))
    }

    pub async fn list_by_target(&self, target_id: &str) -> Result<Vec<RoleDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_role::Entity::find()
            .filter(apollo_role::Column::TargetId.eq(target_id))
            .filter(apollo_role::Column::IsDeleted.eq(false))
            .all(&db)
            .await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn delete(&self, id: i32) -> Result<(), anyhow::Error> {
        let db = self.db()?;
        let model = apollo_role::Entity::find_by_id(id)
            .filter(apollo_role::Column::IsDeleted.eq(false))
            .one(&db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Role not found: {}", id))?;

        let now = Utc::now().naive_utc();
        let mut active_model: apollo_role::ActiveModel = model.into();
        active_model.is_deleted = Set(true);
        active_model.deleted_at = Set(now.and_utc().timestamp_millis());
        active_model.update(&db).await?;
        Ok(())
    }

    pub async fn assign_permission(&self, role_id: i32, permission_id: i32, created_by: &str) -> Result<(), anyhow::Error> {
        let db = self.db()?;
        let now = Utc::now().naive_utc();
        let active_model = apollo_role_permission::ActiveModel {
            role_id: Set(role_id),
            permission_id: Set(permission_id),
            data_change_created_by: Set(created_by.to_string()),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        apollo_role_permission::Entity::insert(active_model).exec(&db).await?;
        Ok(())
    }

    pub async fn remove_permission(&self, role_id: i32, permission_id: i32) -> Result<(), anyhow::Error> {
        let db = self.db()?;
        apollo_role_permission::Entity::delete_many()
            .filter(apollo_role_permission::Column::RoleId.eq(role_id))
            .filter(apollo_role_permission::Column::PermissionId.eq(permission_id))
            .exec(&db)
            .await?;
        Ok(())
    }

    pub async fn list_permissions(&self, role_id: i32) -> Result<Vec<i32>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_role_permission::Entity::find()
            .filter(apollo_role_permission::Column::RoleId.eq(role_id))
            .all(&db)
            .await?;
        Ok(models.iter().map(|m| m.permission_id).collect())
    }

    pub async fn assign_role_to_user(&self, user_id: &str, role_id: i32, created_by: &str) -> Result<(), anyhow::Error> {
        let db = self.db()?;
        let now = Utc::now().naive_utc();
        let active_model = apollo_user_role::ActiveModel {
            user_id: Set(user_id.to_string()),
            role_id: Set(role_id),
            data_change_created_by: Set(created_by.to_string()),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        apollo_user_role::Entity::insert(active_model).exec(&db).await?;
        Ok(())
    }

    pub async fn remove_role_from_user(&self, user_id: &str, role_id: i32) -> Result<(), anyhow::Error> {
        let db = self.db()?;
        apollo_user_role::Entity::delete_many()
            .filter(apollo_user_role::Column::UserId.eq(user_id))
            .filter(apollo_user_role::Column::RoleId.eq(role_id))
            .exec(&db)
            .await?;
        Ok(())
    }

    pub async fn list_user_roles(&self, user_id: &str) -> Result<Vec<RoleDTO>, anyhow::Error> {
        let db = self.db()?;
        let user_roles = apollo_user_role::Entity::find()
            .filter(apollo_user_role::Column::UserId.eq(user_id))
            .all(&db)
            .await?;

        let role_ids: Vec<i32> = user_roles.iter().map(|ur| ur.role_id).collect();
        if role_ids.is_empty() {
            return Ok(vec![]);
        }

        let roles = apollo_role::Entity::find()
            .filter(apollo_role::Column::IsDeleted.eq(false))
            .filter(apollo_role::Column::Id.is_in(role_ids))
            .all(&db)
            .await?;

        Ok(roles.iter().map(|m| self.model_to_dto(m)).collect())
    }

    fn model_to_dto(&self, model: &apollo_role::Model) -> RoleDTO {
        RoleDTO {
            id: Some(model.id),
            role_name: model.role_name.clone(),
            role_type: model.role_type,
            target_id: model.target_id.clone(),
            data_change_created_by: Some(model.data_change_created_by.clone()),
            data_change_created_time: Some(model.data_change_created_time.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()),
        }
    }
}
