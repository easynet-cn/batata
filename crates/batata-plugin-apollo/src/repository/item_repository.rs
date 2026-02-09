//! Apollo Item repository

use batata_persistence::entity::apollo_item;
use sea_orm::prelude::Expr;
use sea_orm::*;

/// Create a new item
pub async fn create(
    db: &DatabaseConnection,
    model: apollo_item::ActiveModel,
) -> anyhow::Result<apollo_item::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find item by namespace_id and key
pub async fn find_by_key(
    db: &DatabaseConnection,
    namespace_id: i64,
    key: &str,
) -> anyhow::Result<Option<apollo_item::Model>> {
    let result = apollo_item::Entity::find()
        .filter(apollo_item::Column::NamespaceId.eq(namespace_id))
        .filter(apollo_item::Column::Key.eq(key))
        .filter(apollo_item::Column::IsDeleted.eq(false))
        .one(db)
        .await?;
    Ok(result)
}

/// Find all items in a namespace
pub async fn find_all_by_namespace(
    db: &DatabaseConnection,
    namespace_id: i64,
) -> anyhow::Result<Vec<apollo_item::Model>> {
    let result = apollo_item::Entity::find()
        .filter(apollo_item::Column::NamespaceId.eq(namespace_id))
        .filter(apollo_item::Column::IsDeleted.eq(false))
        .order_by_asc(apollo_item::Column::LineNum)
        .all(db)
        .await?;
    Ok(result)
}

/// Update an item
pub async fn update(
    db: &DatabaseConnection,
    model: apollo_item::ActiveModel,
) -> anyhow::Result<apollo_item::Model> {
    let result = model.update(db).await?;
    Ok(result)
}

/// Soft delete an item
pub async fn soft_delete(db: &DatabaseConnection, id: i64) -> anyhow::Result<bool> {
    let result = apollo_item::Entity::update_many()
        .filter(apollo_item::Column::Id.eq(id))
        .filter(apollo_item::Column::IsDeleted.eq(false))
        .col_expr(apollo_item::Column::IsDeleted, Expr::value(true))
        .col_expr(
            apollo_item::Column::DeletedAt,
            Expr::current_timestamp().into(),
        )
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Soft delete an item by namespace_id and key
pub async fn soft_delete_by_key(
    db: &DatabaseConnection,
    namespace_id: i64,
    key: &str,
) -> anyhow::Result<bool> {
    let result = apollo_item::Entity::update_many()
        .filter(apollo_item::Column::NamespaceId.eq(namespace_id))
        .filter(apollo_item::Column::Key.eq(key))
        .filter(apollo_item::Column::IsDeleted.eq(false))
        .col_expr(apollo_item::Column::IsDeleted, Expr::value(true))
        .col_expr(
            apollo_item::Column::DeletedAt,
            Expr::current_timestamp().into(),
        )
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Find max line_num in a namespace
pub async fn find_max_line_num(db: &DatabaseConnection, namespace_id: i64) -> anyhow::Result<i32> {
    let result = apollo_item::Entity::find()
        .filter(apollo_item::Column::NamespaceId.eq(namespace_id))
        .filter(apollo_item::Column::IsDeleted.eq(false))
        .order_by_desc(apollo_item::Column::LineNum)
        .one(db)
        .await?;
    Ok(result.and_then(|m| m.line_num).unwrap_or(0))
}
