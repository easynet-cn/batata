use std::sync::Arc;

use crate::api::dto::RoleDTO;
use crate::entity::apollo_role;
use crate::persistence::traits::{ApolloPersistenceService, RolePersistence};

pub struct RoleService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl RoleService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn create(&self, dto: RoleDTO) -> Result<RoleDTO, anyhow::Error> {
        let model = self.persistence.create_role(dto).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn get(&self, id: i32) -> Result<Option<RoleDTO>, anyhow::Error> {
        let model = self.persistence.get_role(id).await?;
        Ok(model.map(|m| self.model_to_dto(&m)))
    }

    pub async fn list_by_target(&self, target_id: &str) -> Result<Vec<RoleDTO>, anyhow::Error> {
        let models = self.persistence.list_role_by_target(target_id).await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn delete(&self, id: i32) -> Result<(), anyhow::Error> {
        self.persistence.delete_role(id).await?;
        Ok(())
    }

    pub async fn assign_permission(&self, role_id: i32, permission_id: i32, created_by: &str) -> Result<(), anyhow::Error> {
        self.persistence.assign_role_permission(role_id, permission_id, created_by).await?;
        Ok(())
    }

    pub async fn remove_permission(&self, role_id: i32, permission_id: i32) -> Result<(), anyhow::Error> {
        self.persistence.remove_role_permission(role_id, permission_id).await?;
        Ok(())
    }

    pub async fn list_permissions(&self, role_id: i32) -> Result<Vec<i32>, anyhow::Error> {
        self.persistence.list_role_permissions(role_id).await
    }

    pub async fn assign_role_to_user(&self, user_id: &str, role_id: i32, created_by: &str) -> Result<(), anyhow::Error> {
        self.persistence.assign_role_to_user(user_id, role_id, created_by).await?;
        Ok(())
    }

    pub async fn remove_role_from_user(&self, user_id: &str, role_id: i32) -> Result<(), anyhow::Error> {
        self.persistence.remove_role_from_user(user_id, role_id).await?;
        Ok(())
    }

    pub async fn list_user_roles(&self, user_id: &str) -> Result<Vec<RoleDTO>, anyhow::Error> {
        let models = self.persistence.list_user_roles(user_id).await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
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
