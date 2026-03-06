//! NamespacePersistence implementation for ExternalDbPersistService

use std::collections::HashMap;

use async_trait::async_trait;
use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use crate::entity::{config_info, tenant_info};
use crate::model::*;
use crate::traits::*;

use super::ExternalDbPersistService;

const DEFAULT_KP: &str = "1";
const DEFAULT_CREATE_SOURCE: &str = "nacos";

#[async_trait]
impl NamespacePersistence for ExternalDbPersistService {
    async fn namespace_find_all(&self) -> anyhow::Result<Vec<NamespaceInfo>> {
        let tenants: Vec<tenant_info::Model> = tenant_info::Entity::find()
            .filter(tenant_info::Column::Kp.eq(DEFAULT_KP))
            .all(&self.db)
            .await?;

        let config_count_rows: Vec<(String, i32)> = config_info::Entity::find()
            .select_only()
            .column(config_info::Column::TenantId)
            .column_as(config_info::Column::Id.count(), "count")
            .filter(config_info::Column::TenantId.is_not_null())
            .group_by(config_info::Column::TenantId)
            .into_tuple::<(String, i32)>()
            .all(&self.db)
            .await?;

        let config_counts: HashMap<String, i32> = config_count_rows.into_iter().collect();

        let mut namespaces: Vec<NamespaceInfo> = tenants
            .into_iter()
            .map(|t| {
                let tenant_id = t.tenant_id.clone().unwrap_or_default();
                let count = config_counts.get(&tenant_id).copied().unwrap_or(0);
                NamespaceInfo {
                    namespace_id: tenant_id,
                    namespace_name: t.tenant_name.unwrap_or_default(),
                    namespace_desc: t.tenant_desc.unwrap_or_default(),
                    config_count: count,
                    quota: 200,
                }
            })
            .collect();

        // Insert default namespace at beginning
        let default_count = config_counts.get("").copied().unwrap_or(0);
        namespaces.insert(
            0,
            NamespaceInfo {
                namespace_id: "public".to_string(),
                namespace_name: "Public".to_string(),
                namespace_desc: "Public Namespace".to_string(),
                config_count: default_count,
                quota: 200,
            },
        );

        Ok(namespaces)
    }

    async fn namespace_get_by_id(
        &self,
        namespace_id: &str,
    ) -> anyhow::Result<Option<NamespaceInfo>> {
        if namespace_id.is_empty() || namespace_id == "public" {
            let count = self.config_count_by_namespace("").await.unwrap_or(0);
            return Ok(Some(NamespaceInfo {
                namespace_id: "public".to_string(),
                namespace_name: "Public".to_string(),
                namespace_desc: "Public Namespace".to_string(),
                config_count: count,
                quota: 200,
            }));
        }

        let tenant_opt = tenant_info::Entity::find()
            .filter(tenant_info::Column::TenantId.eq(namespace_id))
            .filter(tenant_info::Column::Kp.eq(DEFAULT_KP))
            .one(&self.db)
            .await?;

        if let Some(t) = tenant_opt {
            let count: i32 = self
                .config_count_by_namespace(namespace_id)
                .await
                .unwrap_or(0);
            Ok(Some(NamespaceInfo {
                namespace_id: t.tenant_id.unwrap_or_default(),
                namespace_name: t.tenant_name.unwrap_or_default(),
                namespace_desc: t.tenant_desc.unwrap_or_default(),
                config_count: count,
                quota: 200,
            }))
        } else {
            Ok(None)
        }
    }

    async fn namespace_create(
        &self,
        namespace_id: &str,
        name: &str,
        desc: &str,
    ) -> anyhow::Result<()> {
        let entity = tenant_info::ActiveModel {
            tenant_id: Set(Some(namespace_id.to_string())),
            tenant_name: Set(Some(name.to_string())),
            tenant_desc: Set(Some(desc.to_string())),
            kp: Set(DEFAULT_KP.to_string()),
            create_source: Set(Some(DEFAULT_CREATE_SOURCE.to_string())),
            gmt_create: Set(chrono::Utc::now().timestamp_millis()),
            gmt_modified: Set(chrono::Utc::now().timestamp_millis()),
            ..Default::default()
        };

        tenant_info::Entity::insert(entity).exec(&self.db).await?;
        Ok(())
    }

    async fn namespace_update(
        &self,
        namespace_id: &str,
        name: &str,
        desc: &str,
    ) -> anyhow::Result<bool> {
        if let Some(entity) = tenant_info::Entity::find()
            .filter(tenant_info::Column::TenantId.eq(namespace_id))
            .one(&self.db)
            .await?
        {
            let mut active: tenant_info::ActiveModel = entity.into();
            active.tenant_name = Set(Some(name.to_string()));
            active.tenant_desc = Set(Some(desc.to_string()));
            active.gmt_modified = Set(chrono::Utc::now().timestamp_millis());
            active.update(&self.db).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn namespace_delete(&self, namespace_id: &str) -> anyhow::Result<bool> {
        let result = tenant_info::Entity::delete_many()
            .filter(tenant_info::Column::TenantId.eq(namespace_id))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    async fn namespace_check(&self, namespace_id: &str) -> anyhow::Result<bool> {
        let count = tenant_info::Entity::find()
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .filter(tenant_info::Column::TenantId.eq(namespace_id))
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default();

        Ok(count > 0)
    }
}
