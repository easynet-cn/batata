//! Apollo Release repository

use batata_persistence::entity::{apollo_release, apollo_release_history, apollo_release_message};
use sea_orm::prelude::Expr;
use sea_orm::*;

// ============================================================================
// Release
// ============================================================================

/// Create a new release
pub async fn create(
    db: &DatabaseConnection,
    model: apollo_release::ActiveModel,
) -> anyhow::Result<apollo_release::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find release by id
pub async fn find_by_id(
    db: &DatabaseConnection,
    id: i64,
) -> anyhow::Result<Option<apollo_release::Model>> {
    let result = apollo_release::Entity::find_by_id(id)
        .filter(apollo_release::Column::IsDeleted.eq(false))
        .one(db)
        .await?;
    Ok(result)
}

/// Find the latest non-abandoned release for a namespace
pub async fn find_latest(
    db: &DatabaseConnection,
    app_id: &str,
    cluster_name: &str,
    namespace_name: &str,
) -> anyhow::Result<Option<apollo_release::Model>> {
    let result = apollo_release::Entity::find()
        .filter(apollo_release::Column::AppId.eq(app_id))
        .filter(apollo_release::Column::ClusterName.eq(cluster_name))
        .filter(apollo_release::Column::NamespaceName.eq(namespace_name))
        .filter(apollo_release::Column::IsAbandoned.eq(false))
        .filter(apollo_release::Column::IsDeleted.eq(false))
        .order_by_desc(apollo_release::Column::Id)
        .one(db)
        .await?;
    Ok(result)
}

/// Abandon a release
pub async fn abandon(db: &DatabaseConnection, id: i64) -> anyhow::Result<bool> {
    let result = apollo_release::Entity::update_many()
        .filter(apollo_release::Column::Id.eq(id))
        .filter(apollo_release::Column::IsDeleted.eq(false))
        .col_expr(apollo_release::Column::IsAbandoned, Expr::value(true))
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Find all releases for a namespace (for rollback selection)
pub async fn find_all_by_namespace(
    db: &DatabaseConnection,
    app_id: &str,
    cluster_name: &str,
    namespace_name: &str,
    page: u64,
    page_size: u64,
) -> anyhow::Result<Vec<apollo_release::Model>> {
    let result = apollo_release::Entity::find()
        .filter(apollo_release::Column::AppId.eq(app_id))
        .filter(apollo_release::Column::ClusterName.eq(cluster_name))
        .filter(apollo_release::Column::NamespaceName.eq(namespace_name))
        .filter(apollo_release::Column::IsDeleted.eq(false))
        .order_by_desc(apollo_release::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size)
        .all(db)
        .await?;
    Ok(result)
}

// ============================================================================
// Release History
// ============================================================================

/// Create a release history record
pub async fn create_history(
    db: &DatabaseConnection,
    model: apollo_release_history::ActiveModel,
) -> anyhow::Result<apollo_release_history::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find release history by namespace
pub async fn find_history_by_namespace(
    db: &DatabaseConnection,
    app_id: &str,
    cluster_name: &str,
    namespace_name: &str,
    page: u64,
    page_size: u64,
) -> anyhow::Result<Vec<apollo_release_history::Model>> {
    let result = apollo_release_history::Entity::find()
        .filter(apollo_release_history::Column::AppId.eq(app_id))
        .filter(apollo_release_history::Column::ClusterName.eq(cluster_name))
        .filter(apollo_release_history::Column::NamespaceName.eq(namespace_name))
        .filter(apollo_release_history::Column::IsDeleted.eq(false))
        .order_by_desc(apollo_release_history::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size)
        .all(db)
        .await?;
    Ok(result)
}

// ============================================================================
// Release Message
// ============================================================================

/// Send a release message
pub async fn send_message(
    db: &DatabaseConnection,
    model: apollo_release_message::ActiveModel,
) -> anyhow::Result<apollo_release_message::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find latest release message id
pub async fn find_latest_message_id(db: &DatabaseConnection) -> anyhow::Result<i64> {
    let result = apollo_release_message::Entity::find()
        .order_by_desc(apollo_release_message::Column::Id)
        .one(db)
        .await?;
    Ok(result.map(|m| m.id).unwrap_or(0))
}

/// Find latest release message id by message key
pub async fn find_latest_message_id_by_key(
    db: &DatabaseConnection,
    message_key: &str,
) -> anyhow::Result<Option<i64>> {
    let result = apollo_release_message::Entity::find()
        .filter(apollo_release_message::Column::Message.eq(message_key))
        .filter(apollo_release_message::Column::IsDeleted.eq(false))
        .order_by_desc(apollo_release_message::Column::Id)
        .one(db)
        .await?;
    Ok(result.map(|m| m.id))
}

/// Find messages after a given id
pub async fn find_messages_after(
    db: &DatabaseConnection,
    after_id: i64,
    limit: u64,
) -> anyhow::Result<Vec<apollo_release_message::Model>> {
    let result = apollo_release_message::Entity::find()
        .filter(apollo_release_message::Column::Id.gt(after_id))
        .filter(apollo_release_message::Column::IsDeleted.eq(false))
        .order_by_asc(apollo_release_message::Column::Id)
        .limit(limit)
        .all(db)
        .await?;
    Ok(result)
}
