//! Apollo Access Key repository

use batata_persistence::entity::apollo_access_key;
use sea_orm::prelude::Expr;
use sea_orm::*;

/// Create a new access key
pub async fn create(
    db: &DatabaseConnection,
    model: apollo_access_key::ActiveModel,
) -> anyhow::Result<apollo_access_key::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find access keys by app_id
pub async fn find_by_app(
    db: &DatabaseConnection,
    app_id: &str,
) -> anyhow::Result<Vec<apollo_access_key::Model>> {
    let result = apollo_access_key::Entity::find()
        .filter(apollo_access_key::Column::AppId.eq(app_id))
        .filter(apollo_access_key::Column::IsDeleted.eq(false))
        .all(db)
        .await?;
    Ok(result)
}

/// Find access key by id
pub async fn find_by_id(
    db: &DatabaseConnection,
    id: i64,
) -> anyhow::Result<Option<apollo_access_key::Model>> {
    let result = apollo_access_key::Entity::find_by_id(id)
        .filter(apollo_access_key::Column::IsDeleted.eq(false))
        .one(db)
        .await?;
    Ok(result)
}

/// Enable/disable an access key
pub async fn set_enabled(db: &DatabaseConnection, id: i64, enabled: bool) -> anyhow::Result<bool> {
    let result = apollo_access_key::Entity::update_many()
        .filter(apollo_access_key::Column::Id.eq(id))
        .filter(apollo_access_key::Column::IsDeleted.eq(false))
        .col_expr(apollo_access_key::Column::IsEnabled, Expr::value(enabled))
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Soft delete an access key
pub async fn soft_delete(db: &DatabaseConnection, id: i64) -> anyhow::Result<bool> {
    let result = apollo_access_key::Entity::update_many()
        .filter(apollo_access_key::Column::Id.eq(id))
        .filter(apollo_access_key::Column::IsDeleted.eq(false))
        .col_expr(apollo_access_key::Column::IsDeleted, Expr::value(true))
        .col_expr(
            apollo_access_key::Column::DeletedAt,
            Expr::current_timestamp().into(),
        )
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Find access key by secret (for signature verification)
pub async fn find_by_secret(
    db: &DatabaseConnection,
    secret: &str,
) -> anyhow::Result<Option<apollo_access_key::Model>> {
    let result = apollo_access_key::Entity::find()
        .filter(apollo_access_key::Column::Secret.eq(secret))
        .filter(apollo_access_key::Column::IsDeleted.eq(false))
        .one(db)
        .await?;
    Ok(result)
}
