//! Apollo Namespace repository

use batata_persistence::entity::{apollo_app_namespace, apollo_namespace};
use sea_orm::prelude::Expr;
use sea_orm::*;

// ============================================================================
// App Namespace (definitions/templates)
// ============================================================================

/// Create a namespace definition
pub async fn create_definition(
    db: &DatabaseConnection,
    model: apollo_app_namespace::ActiveModel,
) -> anyhow::Result<apollo_app_namespace::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find namespace definition by app_id and name
pub async fn find_definition(
    db: &DatabaseConnection,
    app_id: &str,
    name: &str,
) -> anyhow::Result<Option<apollo_app_namespace::Model>> {
    let result = apollo_app_namespace::Entity::find()
        .filter(apollo_app_namespace::Column::AppId.eq(app_id))
        .filter(apollo_app_namespace::Column::Name.eq(name))
        .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
        .one(db)
        .await?;
    Ok(result)
}

/// Find all namespace definitions for an app
pub async fn find_definitions_by_app(
    db: &DatabaseConnection,
    app_id: &str,
) -> anyhow::Result<Vec<apollo_app_namespace::Model>> {
    let result = apollo_app_namespace::Entity::find()
        .filter(apollo_app_namespace::Column::AppId.eq(app_id))
        .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
        .all(db)
        .await?;
    Ok(result)
}

/// Find all public namespace definitions
pub async fn find_public_definitions(
    db: &DatabaseConnection,
) -> anyhow::Result<Vec<apollo_app_namespace::Model>> {
    let result = apollo_app_namespace::Entity::find()
        .filter(apollo_app_namespace::Column::IsPublic.eq(true))
        .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
        .all(db)
        .await?;
    Ok(result)
}

/// Soft delete a namespace definition
pub async fn soft_delete_definition(
    db: &DatabaseConnection,
    app_id: &str,
    name: &str,
) -> anyhow::Result<bool> {
    let result = apollo_app_namespace::Entity::update_many()
        .filter(apollo_app_namespace::Column::AppId.eq(app_id))
        .filter(apollo_app_namespace::Column::Name.eq(name))
        .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
        .col_expr(apollo_app_namespace::Column::IsDeleted, Expr::value(true))
        .col_expr(
            apollo_app_namespace::Column::DeletedAt,
            Expr::current_timestamp().into(),
        )
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

// ============================================================================
// Namespace instances (per cluster)
// ============================================================================

/// Create a namespace instance
pub async fn create_instance(
    db: &DatabaseConnection,
    model: apollo_namespace::ActiveModel,
) -> anyhow::Result<apollo_namespace::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find namespace instance
pub async fn find_instance(
    db: &DatabaseConnection,
    app_id: &str,
    cluster_name: &str,
    namespace_name: &str,
) -> anyhow::Result<Option<apollo_namespace::Model>> {
    let result = apollo_namespace::Entity::find()
        .filter(apollo_namespace::Column::AppId.eq(app_id))
        .filter(apollo_namespace::Column::ClusterName.eq(cluster_name))
        .filter(apollo_namespace::Column::NamespaceName.eq(namespace_name))
        .filter(apollo_namespace::Column::IsDeleted.eq(false))
        .one(db)
        .await?;
    Ok(result)
}

/// Find all namespace instances for an app and cluster
pub async fn find_instances_by_cluster(
    db: &DatabaseConnection,
    app_id: &str,
    cluster_name: &str,
) -> anyhow::Result<Vec<apollo_namespace::Model>> {
    let result = apollo_namespace::Entity::find()
        .filter(apollo_namespace::Column::AppId.eq(app_id))
        .filter(apollo_namespace::Column::ClusterName.eq(cluster_name))
        .filter(apollo_namespace::Column::IsDeleted.eq(false))
        .all(db)
        .await?;
    Ok(result)
}

/// Soft delete a namespace instance
pub async fn soft_delete_instance(
    db: &DatabaseConnection,
    app_id: &str,
    cluster_name: &str,
    namespace_name: &str,
) -> anyhow::Result<bool> {
    let result = apollo_namespace::Entity::update_many()
        .filter(apollo_namespace::Column::AppId.eq(app_id))
        .filter(apollo_namespace::Column::ClusterName.eq(cluster_name))
        .filter(apollo_namespace::Column::NamespaceName.eq(namespace_name))
        .filter(apollo_namespace::Column::IsDeleted.eq(false))
        .col_expr(apollo_namespace::Column::IsDeleted, Expr::value(true))
        .col_expr(
            apollo_namespace::Column::DeletedAt,
            Expr::current_timestamp().into(),
        )
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}
