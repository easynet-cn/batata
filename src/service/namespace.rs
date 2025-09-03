use std::collections::HashMap;

use actix_web::http::StatusCode;
use anyhow::Ok;
use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use crate::{
    entity::{config_info, tenant_info},
    error::{self, BatataError, ErrorCode},
    model::{common::DEFAULT_NAMESPACE_ID, naming::Namespace},
};

const DEFAULT_NAMESPACE: &'static str = "public";
const DEFAULT_CREATE_SOURCE: &'static str = "nacos";
const DEFAULT_KP: &'static str = "1";

// Find all namespaces

pub async fn find_all(db: &DatabaseConnection) -> Vec<Namespace> {
    let tenant_infos: Vec<tenant_info::Model> = tenant_info::Entity::find()
        .filter(tenant_info::Column::Kp.eq(DEFAULT_KP))
        .all(db)
        .await
        .unwrap();

    let mut tenant_ids: Vec<String> = Vec::new();
    let mut namespaces: Vec<Namespace> = tenant_infos
        .iter()
        .map(|tenant_info| {
            tenant_ids.push(tenant_info.tenant_id.clone().unwrap_or_default());

            Namespace::from(tenant_info.clone())
        })
        .collect();

    namespaces.insert(0, Namespace::default());

    tenant_ids.push("".to_string());

    let config_infos = config_info::Entity::find()
        .column(config_info::Column::TenantId)
        .column_as(config_info::Column::Id.count(), "count")
        .filter(config_info::Column::TenantId.is_in(tenant_ids))
        .filter(config_info::Column::TenantId.is_not_null())
        .group_by(config_info::Column::TenantId)
        .into_tuple::<(String, i32)>()
        .all(db)
        .await
        .unwrap()
        .iter()
        .map(|x| (x.0.to_string(), x.1))
        .collect::<HashMap<String, i32>>();

    namespaces.iter_mut().for_each(|namespace| {
        if let Some(count) = config_infos.get(&namespace.namespace) {
            namespace.config_count = *count;
        }
    });

    namespaces
}

pub async fn get_by_namespace_id(
    db: &DatabaseConnection,
    namespace_id: &str,
    namespace_type: &str,
) -> anyhow::Result<Namespace> {
    if namespace_id.is_empty() || namespace_id == DEFAULT_NAMESPACE {
        return Ok(Namespace::default());
    }

    if let Some(tenant_info) = tenant_info::Entity::find()
        .filter(tenant_info::Column::TenantId.eq(namespace_id))
        .filter(tenant_info::Column::Kp.eq(namespace_type))
        .one(db)
        .await?
    {
        let mut namespace = Namespace::from(tenant_info);

        namespace.type_ = namespace_type.parse::<i32>().unwrap_or_default();

        if let Some(config_count) = config_info::Entity::find()
            .column(config_info::Column::TenantId)
            .column_as(config_info::Column::Id.count(), "count")
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .filter(config_info::Column::TenantId.is_not_null())
            .group_by(config_info::Column::TenantId)
            .into_tuple::<(String, i32)>()
            .one(db)
            .await?
        {
            namespace.config_count = config_count.1;
        }

        return Ok(namespace);
    } else {
        return Err(BatataError::ApiError(
            StatusCode::NOT_FOUND.as_u16() as i32,
            error::NAMESPACE_NOT_EXIST.code,
            error::NAMESPACE_NOT_EXIST.message.to_string(),
            format!("namespaceId [{}] not exist", namespace_id),
        )
        .into());
    }
}

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

pub async fn delete(db: &DatabaseConnection, namespace_id: &str) -> anyhow::Result<bool> {
    let res = tenant_info::Entity::delete_many()
        .filter(tenant_info::Column::TenantId.eq(namespace_id))
        .exec(db)
        .await?;

    Ok(res.rows_affected > 0)
}

pub async fn check(db: &DatabaseConnection, namespace_id: &str) -> anyhow::Result<bool> {
    if DEFAULT_NAMESPACE_ID == namespace_id {
        return Err(BatataError::ApiError(
            StatusCode::BAD_REQUEST.as_u16() as i32,
            error::NAMESPACE_ALREADY_EXIST.code,
            error::NAMESPACE_ALREADY_EXIST.message.to_string(),
            format!(
                "namespaceId [{}] is default namespace id and already exist.",
                namespace_id
            ),
        )
        .into());
    }

    let count = get_count_by_tenant_id(db, namespace_id).await?;

    if count > 0 {
        return Err(BatataError::ApiError(
            StatusCode::BAD_REQUEST.as_u16() as i32,
            error::NAMESPACE_ALREADY_EXIST.code,
            error::NAMESPACE_ALREADY_EXIST.message.to_string(),
            format!("namespaceId [{}] already exist.", namespace_id),
        )
        .into());
    }

    Ok(false)
}
