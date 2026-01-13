//! Namespace service layer
//!
//! This module provides database operations for namespace management.

use std::collections::HashMap;

use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use batata_common::error::BatataError;
use batata_persistence::entity::{config_info, tenant_info};

use crate::model::{DEFAULT_NAMESPACE_ID, Namespace};

const DEFAULT_NAMESPACE: &str = "public";
const DEFAULT_CREATE_SOURCE: &str = "nacos";
const DEFAULT_KP: &str = "1";

/// Find all namespaces
pub async fn find_all(db: &DatabaseConnection) -> Vec<Namespace> {
    // Execute both queries concurrently to reduce latency
    let (tenant_result, config_counts_result) = tokio::join!(
        tenant_info::Entity::find()
            .filter(tenant_info::Column::Kp.eq(DEFAULT_KP))
            .all(db),
        config_info::Entity::find()
            .select_only()
            .column(config_info::Column::TenantId)
            .column_as(config_info::Column::Id.count(), "count")
            .filter(config_info::Column::TenantId.is_not_null())
            .group_by(config_info::Column::TenantId)
            .into_tuple::<(String, i32)>()
            .all(db)
    );

    let tenant_infos: Vec<tenant_info::Model> = match tenant_result {
        Ok(infos) => infos,
        Err(e) => {
            tracing::error!("Failed to fetch tenant infos: {}", e);
            return vec![Namespace::default()];
        }
    };

    let config_infos: HashMap<String, i32> = match config_counts_result {
        Ok(infos) => infos.into_iter().collect(),
        Err(e) => {
            tracing::error!("Failed to fetch config counts: {}", e);
            HashMap::new()
        }
    };

    let mut namespaces: Vec<Namespace> = tenant_infos
        .into_iter()
        .map(|tenant_info| {
            let mut ns = Namespace::from(tenant_info.clone());
            let tenant_id = tenant_info.tenant_id.unwrap_or_default();
            if let Some(&count) = config_infos.get(&tenant_id) {
                ns.config_count = count;
            }
            ns
        })
        .collect();

    // Insert default namespace at the beginning
    let mut default_ns = Namespace::default();
    if let Some(&count) = config_infos.get("") {
        default_ns.config_count = count;
    }
    namespaces.insert(0, default_ns);

    namespaces
}

/// Get namespace by ID
pub async fn get_by_namespace_id(
    db: &DatabaseConnection,
    namespace_id: &str,
    namespace_type: &str,
) -> anyhow::Result<Namespace> {
    if namespace_id.is_empty() || namespace_id == DEFAULT_NAMESPACE {
        return Ok(Namespace::default());
    }

    // Execute both queries concurrently to reduce latency
    let (tenant_result, config_count_result) = tokio::join!(
        tenant_info::Entity::find()
            .filter(tenant_info::Column::TenantId.eq(namespace_id))
            .filter(tenant_info::Column::Kp.eq(namespace_type))
            .one(db),
        config_info::Entity::find()
            .select_only()
            .column_as(config_info::Column::Id.count(), "count")
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .into_tuple::<i32>()
            .one(db)
    );

    if let Some(tenant_info) = tenant_result? {
        let mut namespace = Namespace::from(tenant_info);
        namespace.type_ = namespace_type.parse::<i32>().unwrap_or_default();
        namespace.config_count = match config_count_result {
            Ok(Some(count)) => count,
            Ok(None) => 0,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    namespace_id,
                    "Failed to count configs for namespace"
                );
                0
            }
        };

        Ok(namespace)
    } else {
        Err(BatataError::NamespaceNotExist(namespace_id.to_string()).into())
    }
}

/// Create a new namespace
pub async fn create(
    db: &DatabaseConnection,
    namespace_id: &str,
    namespace_name: &str,
    namespace_desc: &str,
) -> anyhow::Result<bool> {
    let entity = tenant_info::ActiveModel {
        tenant_id: Set(Some(namespace_id.to_string())),
        tenant_name: Set(Some(namespace_name.to_string())),
        tenant_desc: Set(Some(namespace_desc.to_string())),
        kp: Set(DEFAULT_KP.to_string()),
        create_source: Set(Some(DEFAULT_CREATE_SOURCE.to_string())),
        gmt_create: Set(chrono::Utc::now().timestamp_millis()),
        gmt_modified: Set(chrono::Utc::now().timestamp_millis()),
        ..Default::default()
    };

    tenant_info::Entity::insert(entity).exec(db).await?;

    Ok(true)
}

/// Get count of namespaces by tenant ID
pub async fn get_count_by_tenant_id(
    db: &DatabaseConnection,
    namespace_id: &str,
) -> anyhow::Result<u64> {
    let count = tenant_info::Entity::find()
        .select_only()
        .column_as(Expr::col(Asterisk).count(), "count")
        .filter(tenant_info::Column::TenantId.eq(namespace_id))
        .into_tuple::<i64>()
        .one(db)
        .await?
        .unwrap_or_default() as u64;

    Ok(count)
}

/// Update an existing namespace
pub async fn update(
    db: &DatabaseConnection,
    namespace_id: &str,
    namespace_name: &str,
    namespace_desc: &str,
) -> anyhow::Result<bool> {
    if let Some(entity) = tenant_info::Entity::find()
        .filter(tenant_info::Column::TenantId.eq(namespace_id))
        .one(db)
        .await?
    {
        let mut tenant_info: tenant_info::ActiveModel = entity.into();

        tenant_info.tenant_name = Set(Some(namespace_name.to_string()));
        tenant_info.tenant_desc = Set(Some(namespace_desc.to_string()));

        if tenant_info.is_changed() {
            tenant_info.gmt_modified = Set(chrono::Utc::now().timestamp_millis());
            tenant_info.update(db).await?;
        }

        return Ok(true);
    }

    Ok(false)
}

/// Delete a namespace
pub async fn delete(db: &DatabaseConnection, namespace_id: &str) -> anyhow::Result<bool> {
    let res = tenant_info::Entity::delete_many()
        .filter(tenant_info::Column::TenantId.eq(namespace_id))
        .exec(db)
        .await?;

    Ok(res.rows_affected > 0)
}

/// Check if namespace ID is available
pub async fn check(db: &DatabaseConnection, namespace_id: &str) -> anyhow::Result<bool> {
    if DEFAULT_NAMESPACE_ID == namespace_id {
        return Err(BatataError::NamespaceAlreadyExist(namespace_id.to_string()).into());
    }

    let count = get_count_by_tenant_id(db, namespace_id).await?;

    if count > 0 {
        return Err(BatataError::NamespaceAlreadyExist(namespace_id.to_string()).into());
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_namespace_check() {
        // This is a sync test for the check logic
        assert_eq!(DEFAULT_NAMESPACE_ID, "public");
        assert_eq!(DEFAULT_KP, "1");
    }
}
