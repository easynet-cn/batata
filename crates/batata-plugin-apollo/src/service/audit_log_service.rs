//! Distributed audit-log service.

use batata_common::datetime::DateTimeExt;
use batata_common::datetime::NaiveDateTimeExt;
use std::sync::Arc;

use crate::api::dto::{
    AuditLogDTO, AuditLogDataInfluenceDTO, NewAuditLog, NewAuditLogDataInfluence,
};
use crate::persistence::traits::{ApolloPersistenceService, AuditLogPersistence};

/// Writes and queries Apollo distributed audit logs.
pub struct AuditLogService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl AuditLogService {
    /// Creates the service.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Records an operation and all of its data influences.
    pub async fn record(
        &self,
        dto: NewAuditLog,
        influences: Vec<NewAuditLogDataInfluence>,
    ) -> anyhow::Result<AuditLogDTO> {
        let log = AuditLogPersistence::create_audit_log(&self.persistence, dto).await?;
        let mut influence_dtos = Vec::with_capacity(influences.len());
        for influence in influences {
            let span_id = log.span_id.clone();
            let created = AuditLogPersistence::create_data_influence(
                &self.persistence,
                NewAuditLogDataInfluence {
                    span_id,
                    ..influence
                },
            )
            .await?;
            influence_dtos.push(Self::influence_to_dto(&created));
        }
        Ok(Self::log_to_dto(&log, influence_dtos))
    }

    /// Lists audit logs matching the supplied filters.
    pub async fn list(
        &self,
        op_name: Option<&str>,
        operator: Option<&str>,
        trace_id: Option<&str>,
        query: Option<&str>,
        page: u64,
        size: u64,
    ) -> anyhow::Result<(Vec<AuditLogDTO>, u64)> {
        let (logs, total) = AuditLogPersistence::list_audit_logs(
            &self.persistence,
            op_name,
            operator,
            trace_id,
            query,
            page,
            size,
        )
        .await?;
        let span_ids: Vec<String> = logs.iter().map(|log| log.span_id.clone()).collect();
        let influences = AuditLogPersistence::list_influences(&self.persistence, &span_ids).await?;
        let dtos = logs
            .iter()
            .map(|log| {
                let rows = influences
                    .iter()
                    .filter(|row| row.span_id == log.span_id)
                    .map(Self::influence_to_dto)
                    .collect();
                Self::log_to_dto(log, rows)
            })
            .collect();
        Ok((dtos, total))
    }

    fn log_to_dto(
        model: &crate::entity::apollo_audit_log::Model,
        data_influences: Vec<AuditLogDataInfluenceDTO>,
    ) -> AuditLogDTO {
        AuditLogDTO {
            id: Some(model.id),
            trace_id: model.trace_id.clone(),
            span_id: model.span_id.clone(),
            parent_span_id: model.parent_span_id.clone(),
            follows_from_span_id: model.follows_from_span_id.clone(),
            operator: model.operator.clone(),
            op_type: model.op_type.clone(),
            op_name: model.op_name.clone(),
            description: model.description.clone(),
            is_deleted: model.is_deleted,
            deleted_at: model.deleted_at,
            data_change_created_time: Some(model.data_change_created_time.and_utc().to_rfc3339()),
            data_change_last_time: Some(model.data_change_last_time.and_utc().to_rfc3339()),
            data_influences,
        }
    }

    fn influence_to_dto(
        model: &crate::entity::apollo_audit_log_data_influence::Model,
    ) -> AuditLogDataInfluenceDTO {
        AuditLogDataInfluenceDTO {
            id: Some(model.id),
            entity_id: model.influence_entity_id.clone(),
            entity_name: model.influence_entity_name.clone(),
            field_name: model.field_name.clone(),
            field_old_value: model.field_old_value.clone(),
            field_new_value: model.field_new_value.clone(),
        }
    }
}
