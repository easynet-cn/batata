//! Apollo Commit repository

use batata_persistence::entity::apollo_commit;
use sea_orm::*;

/// Create a commit record
pub async fn create(
    db: &DatabaseConnection,
    model: apollo_commit::ActiveModel,
) -> anyhow::Result<apollo_commit::Model> {
    let result = model.insert(db).await?;
    Ok(result)
}

/// Find commits by namespace
pub async fn find_by_namespace(
    db: &DatabaseConnection,
    app_id: &str,
    cluster_name: &str,
    namespace_name: &str,
    page: u64,
    page_size: u64,
) -> anyhow::Result<Vec<apollo_commit::Model>> {
    let result = apollo_commit::Entity::find()
        .filter(apollo_commit::Column::AppId.eq(app_id))
        .filter(apollo_commit::Column::ClusterName.eq(cluster_name))
        .filter(apollo_commit::Column::NamespaceName.eq(namespace_name))
        .filter(apollo_commit::Column::IsDeleted.eq(false))
        .order_by_desc(apollo_commit::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size)
        .all(db)
        .await?;
    Ok(result)
}
