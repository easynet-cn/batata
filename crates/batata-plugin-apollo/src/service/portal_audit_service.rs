//! Audit services for portal user tokens and open-platform consumers.

use std::sync::Arc;

use crate::api::dto::{ConsumerAuditDTO, NewConsumerAudit, NewUserTokenAudit, UserTokenAuditDTO};
use crate::persistence::traits::{
    ApolloPersistenceService, ConsumerAuditPersistence, UserTokenAuditPersistence,
};

/// Records and queries user-token request audit rows.
pub struct UserTokenAuditService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl UserTokenAuditService {
    /// Creates the service.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Records one authenticated request.
    pub async fn record(&self, dto: NewUserTokenAudit) -> anyhow::Result<UserTokenAuditDTO> {
        let model =
            UserTokenAuditPersistence::create_user_token_audit(&self.persistence, dto).await?;
        Ok(UserTokenAuditDTO {
            id: Some(model.id),
            token_id: model.token_id,
            user_id: model.user_id,
            uri: model.uri,
            method: model.method,
            created_time: model.data_change_created_time.and_utc().to_rfc3339(),
        })
    }

    /// Lists requests made with one token.
    pub async fn list_by_token(&self, token_id: i64) -> anyhow::Result<Vec<UserTokenAuditDTO>> {
        let rows =
            UserTokenAuditPersistence::list_user_token_audits(&self.persistence, token_id).await?;
        Ok(rows.iter().map(Self::to_dto).collect())
    }

    /// Lists requests attributed to one account.
    pub async fn list_by_user(&self, user_id: &str) -> anyhow::Result<Vec<UserTokenAuditDTO>> {
        let rows =
            UserTokenAuditPersistence::list_user_token_audits_by_user(&self.persistence, user_id)
                .await?;
        Ok(rows.iter().map(Self::to_dto).collect())
    }

    fn to_dto(model: &crate::entity::apollo_user_token_audit::Model) -> UserTokenAuditDTO {
        UserTokenAuditDTO {
            id: Some(model.id),
            token_id: model.token_id,
            user_id: model.user_id.clone(),
            uri: model.uri.clone(),
            method: model.method.clone(),
            created_time: model.data_change_created_time.and_utc().to_rfc3339(),
        }
    }
}

/// Records and queries consumer management audit rows.
pub struct ConsumerAuditService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ConsumerAuditService {
    /// Creates the service.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Records one consumer operation.
    pub async fn record(&self, dto: NewConsumerAudit) -> anyhow::Result<ConsumerAuditDTO> {
        let model = ConsumerAuditPersistence::create_consumer_audit(&self.persistence, dto).await?;
        Ok(Self::to_dto(&model))
    }

    /// Lists operations for one consumer.
    pub async fn list(&self, consumer_id: i64) -> anyhow::Result<Vec<ConsumerAuditDTO>> {
        let rows =
            ConsumerAuditPersistence::list_consumer_audits(&self.persistence, consumer_id).await?;
        Ok(rows.iter().map(Self::to_dto).collect())
    }

    fn to_dto(model: &crate::entity::apollo_consumer_audit::Model) -> ConsumerAuditDTO {
        ConsumerAuditDTO {
            id: Some(model.id),
            consumer_id: model.consumer_id,
            op_name: model.op_name.clone(),
            op_by: model.op_by.clone(),
            op_time: model.op_time.and_utc().to_rfc3339(),
        }
    }
}
