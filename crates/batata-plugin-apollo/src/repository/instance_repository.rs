//! Apollo Instance repository

use batata_persistence::entity::{apollo_instance, apollo_instance_config};
use sea_orm::*;

// ============================================================================
// Instance
// ============================================================================

/// Upsert an instance (create or update)
pub async fn upsert_instance(
    db: &DatabaseConnection,
    model: apollo_instance::ActiveModel,
) -> anyhow::Result<apollo_instance::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find instance by app, cluster, and ip
pub async fn find_instance(
    db: &DatabaseConnection,
    app_id: &str,
    cluster_name: &str,
    ip: &str,
) -> anyhow::Result<Option<apollo_instance::Model>> {
    let result = apollo_instance::Entity::find()
        .filter(apollo_instance::Column::AppId.eq(app_id))
        .filter(apollo_instance::Column::ClusterName.eq(cluster_name))
        .filter(apollo_instance::Column::Ip.eq(ip))
        .filter(apollo_instance::Column::IsDeleted.eq(false))
        .one(db)
        .await?;
    Ok(result)
}

/// Find all instances for an app
pub async fn find_instances_by_app(
    db: &DatabaseConnection,
    app_id: &str,
) -> anyhow::Result<Vec<apollo_instance::Model>> {
    let result = apollo_instance::Entity::find()
        .filter(apollo_instance::Column::AppId.eq(app_id))
        .filter(apollo_instance::Column::IsDeleted.eq(false))
        .all(db)
        .await?;
    Ok(result)
}

// ============================================================================
// Instance Config
// ============================================================================

/// Create or update instance config
pub async fn upsert_instance_config(
    db: &DatabaseConnection,
    model: apollo_instance_config::ActiveModel,
) -> anyhow::Result<apollo_instance_config::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find configs for an instance
pub async fn find_configs_by_instance(
    db: &DatabaseConnection,
    instance_id: i64,
) -> anyhow::Result<Vec<apollo_instance_config::Model>> {
    let result = apollo_instance_config::Entity::find()
        .filter(apollo_instance_config::Column::InstanceId.eq(instance_id))
        .filter(apollo_instance_config::Column::IsDeleted.eq(false))
        .all(db)
        .await?;
    Ok(result)
}

/// Count instances watching a namespace
pub async fn count_by_namespace(
    db: &DatabaseConnection,
    config_app_id: &str,
    config_cluster_name: &str,
    config_namespace_name: &str,
) -> anyhow::Result<u64> {
    let result = apollo_instance_config::Entity::find()
        .filter(apollo_instance_config::Column::ConfigAppId.eq(config_app_id))
        .filter(apollo_instance_config::Column::ConfigClusterName.eq(config_cluster_name))
        .filter(apollo_instance_config::Column::ConfigNamespaceName.eq(config_namespace_name))
        .filter(apollo_instance_config::Column::IsDeleted.eq(false))
        .count(db)
        .await?;
    Ok(result)
}
