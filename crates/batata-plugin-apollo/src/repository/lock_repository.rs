//! Apollo Namespace Lock repository

use batata_persistence::entity::apollo_namespace_lock;
use sea_orm::*;

/// Acquire a lock (insert or update)
pub async fn acquire(
    db: &DatabaseConnection,
    model: apollo_namespace_lock::ActiveModel,
) -> anyhow::Result<apollo_namespace_lock::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Release a lock (delete)
pub async fn release(db: &DatabaseConnection, namespace_id: i64) -> anyhow::Result<bool> {
    let result = apollo_namespace_lock::Entity::delete_many()
        .filter(apollo_namespace_lock::Column::NamespaceId.eq(namespace_id))
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Find lock by namespace_id
pub async fn find_by_namespace(
    db: &DatabaseConnection,
    namespace_id: i64,
) -> anyhow::Result<Option<apollo_namespace_lock::Model>> {
    let result = apollo_namespace_lock::Entity::find()
        .filter(apollo_namespace_lock::Column::NamespaceId.eq(namespace_id))
        .filter(apollo_namespace_lock::Column::IsDeleted.eq(false))
        .one(db)
        .await?;
    Ok(result)
}

/// Update lock owner and times
pub async fn update(
    db: &DatabaseConnection,
    model: apollo_namespace_lock::ActiveModel,
) -> anyhow::Result<apollo_namespace_lock::Model> {
    let result = model.update(db).await?;
    Ok(result)
}
