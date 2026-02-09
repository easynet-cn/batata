//! Apollo Cluster repository

use batata_persistence::entity::apollo_cluster;
use sea_orm::prelude::Expr;
use sea_orm::*;

/// Create a new cluster
pub async fn create(
    db: &DatabaseConnection,
    model: apollo_cluster::ActiveModel,
) -> anyhow::Result<apollo_cluster::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find cluster by app_id and name
pub async fn find_by_app_and_name(
    db: &DatabaseConnection,
    app_id: &str,
    name: &str,
) -> anyhow::Result<Option<apollo_cluster::Model>> {
    let result = apollo_cluster::Entity::find()
        .filter(apollo_cluster::Column::AppId.eq(app_id))
        .filter(apollo_cluster::Column::Name.eq(name))
        .filter(apollo_cluster::Column::IsDeleted.eq(false))
        .one(db)
        .await?;
    Ok(result)
}

/// Find all clusters for an app
pub async fn find_by_app(
    db: &DatabaseConnection,
    app_id: &str,
) -> anyhow::Result<Vec<apollo_cluster::Model>> {
    let result = apollo_cluster::Entity::find()
        .filter(apollo_cluster::Column::AppId.eq(app_id))
        .filter(apollo_cluster::Column::IsDeleted.eq(false))
        .all(db)
        .await?;
    Ok(result)
}

/// Find child clusters (branches) for a parent cluster
pub async fn find_children(
    db: &DatabaseConnection,
    parent_cluster_id: i64,
) -> anyhow::Result<Vec<apollo_cluster::Model>> {
    let result = apollo_cluster::Entity::find()
        .filter(apollo_cluster::Column::ParentClusterId.eq(parent_cluster_id))
        .filter(apollo_cluster::Column::IsDeleted.eq(false))
        .all(db)
        .await?;
    Ok(result)
}

/// Soft delete a cluster
pub async fn soft_delete(
    db: &DatabaseConnection,
    app_id: &str,
    name: &str,
) -> anyhow::Result<bool> {
    let result = apollo_cluster::Entity::update_many()
        .filter(apollo_cluster::Column::AppId.eq(app_id))
        .filter(apollo_cluster::Column::Name.eq(name))
        .filter(apollo_cluster::Column::IsDeleted.eq(false))
        .col_expr(apollo_cluster::Column::IsDeleted, Expr::value(true))
        .col_expr(
            apollo_cluster::Column::DeletedAt,
            Expr::current_timestamp().into(),
        )
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}
