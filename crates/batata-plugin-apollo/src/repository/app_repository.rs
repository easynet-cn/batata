//! Apollo App repository

use batata_persistence::entity::apollo_app;
use sea_orm::prelude::Expr;
use sea_orm::*;

/// Create a new app
pub async fn create(
    db: &DatabaseConnection,
    model: apollo_app::ActiveModel,
) -> anyhow::Result<apollo_app::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find app by app_id
pub async fn find_by_app_id(
    db: &DatabaseConnection,
    app_id: &str,
) -> anyhow::Result<Option<apollo_app::Model>> {
    let result = apollo_app::Entity::find()
        .filter(apollo_app::Column::AppId.eq(app_id))
        .filter(apollo_app::Column::IsDeleted.eq(false))
        .one(db)
        .await?;
    Ok(result)
}

/// Find all apps
pub async fn find_all(db: &DatabaseConnection) -> anyhow::Result<Vec<apollo_app::Model>> {
    let result = apollo_app::Entity::find()
        .filter(apollo_app::Column::IsDeleted.eq(false))
        .all(db)
        .await?;
    Ok(result)
}

/// Update an app
pub async fn update(
    db: &DatabaseConnection,
    model: apollo_app::ActiveModel,
) -> anyhow::Result<apollo_app::Model> {
    let result = model.update(db).await?;
    Ok(result)
}

/// Soft delete an app
pub async fn soft_delete(db: &DatabaseConnection, app_id: &str) -> anyhow::Result<bool> {
    let result = apollo_app::Entity::update_many()
        .filter(apollo_app::Column::AppId.eq(app_id))
        .filter(apollo_app::Column::IsDeleted.eq(false))
        .col_expr(apollo_app::Column::IsDeleted, Expr::value(true))
        .col_expr(
            apollo_app::Column::DeletedAt,
            Expr::current_timestamp().into(),
        )
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}
