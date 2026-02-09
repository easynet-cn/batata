//! Apollo Gray Release Rule repository

use batata_persistence::entity::apollo_gray_release_rule;
use sea_orm::prelude::Expr;
use sea_orm::*;

/// Create a gray release rule
pub async fn create(
    db: &DatabaseConnection,
    model: apollo_gray_release_rule::ActiveModel,
) -> anyhow::Result<apollo_gray_release_rule::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find gray release rule
pub async fn find(
    db: &DatabaseConnection,
    app_id: &str,
    cluster_name: &str,
    namespace_name: &str,
    branch_name: &str,
) -> anyhow::Result<Option<apollo_gray_release_rule::Model>> {
    let result = apollo_gray_release_rule::Entity::find()
        .filter(apollo_gray_release_rule::Column::AppId.eq(app_id))
        .filter(apollo_gray_release_rule::Column::ClusterName.eq(cluster_name))
        .filter(apollo_gray_release_rule::Column::NamespaceName.eq(namespace_name))
        .filter(apollo_gray_release_rule::Column::BranchName.eq(branch_name))
        .filter(apollo_gray_release_rule::Column::IsDeleted.eq(false))
        .one(db)
        .await?;
    Ok(result)
}

/// Find all gray release rules for a namespace
pub async fn find_by_namespace(
    db: &DatabaseConnection,
    app_id: &str,
    cluster_name: &str,
    namespace_name: &str,
) -> anyhow::Result<Vec<apollo_gray_release_rule::Model>> {
    let result = apollo_gray_release_rule::Entity::find()
        .filter(apollo_gray_release_rule::Column::AppId.eq(app_id))
        .filter(apollo_gray_release_rule::Column::ClusterName.eq(cluster_name))
        .filter(apollo_gray_release_rule::Column::NamespaceName.eq(namespace_name))
        .filter(apollo_gray_release_rule::Column::IsDeleted.eq(false))
        .all(db)
        .await?;
    Ok(result)
}

/// Update rules
pub async fn update_rules(db: &DatabaseConnection, id: i64, rules: &str) -> anyhow::Result<bool> {
    let result = apollo_gray_release_rule::Entity::update_many()
        .filter(apollo_gray_release_rule::Column::Id.eq(id))
        .col_expr(apollo_gray_release_rule::Column::Rules, Expr::value(rules))
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Update branch status
pub async fn update_status(db: &DatabaseConnection, id: i64, status: i16) -> anyhow::Result<bool> {
    let result = apollo_gray_release_rule::Entity::update_many()
        .filter(apollo_gray_release_rule::Column::Id.eq(id))
        .col_expr(
            apollo_gray_release_rule::Column::BranchStatus,
            Expr::value(status),
        )
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Update release_id for a rule
pub async fn update_release_id(
    db: &DatabaseConnection,
    id: i64,
    release_id: i64,
) -> anyhow::Result<bool> {
    let result = apollo_gray_release_rule::Entity::update_many()
        .filter(apollo_gray_release_rule::Column::Id.eq(id))
        .col_expr(
            apollo_gray_release_rule::Column::ReleaseId,
            Expr::value(release_id),
        )
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Soft delete a gray release rule
pub async fn soft_delete(db: &DatabaseConnection, id: i64) -> anyhow::Result<bool> {
    let result = apollo_gray_release_rule::Entity::update_many()
        .filter(apollo_gray_release_rule::Column::Id.eq(id))
        .filter(apollo_gray_release_rule::Column::IsDeleted.eq(false))
        .col_expr(
            apollo_gray_release_rule::Column::IsDeleted,
            Expr::value(true),
        )
        .col_expr(
            apollo_gray_release_rule::Column::DeletedAt,
            Expr::current_timestamp().into(),
        )
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}
