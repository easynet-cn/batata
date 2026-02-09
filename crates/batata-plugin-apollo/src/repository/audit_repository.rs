//! Apollo Audit repository

use batata_persistence::entity::apollo_audit;
use sea_orm::*;

/// Insert an audit record
pub async fn insert(
    db: &DatabaseConnection,
    model: apollo_audit::ActiveModel,
) -> anyhow::Result<apollo_audit::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find audit records by entity
pub async fn find_by_entity(
    db: &DatabaseConnection,
    entity_name: &str,
    entity_id: &str,
    page: u64,
    page_size: u64,
) -> anyhow::Result<Vec<apollo_audit::Model>> {
    let result = apollo_audit::Entity::find()
        .filter(apollo_audit::Column::EntityName.eq(entity_name))
        .filter(apollo_audit::Column::EntityId.eq(entity_id))
        .filter(apollo_audit::Column::IsDeleted.eq(false))
        .order_by_desc(apollo_audit::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size)
        .all(db)
        .await?;
    Ok(result)
}
