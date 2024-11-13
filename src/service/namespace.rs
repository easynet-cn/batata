use std::collections::HashMap;

use sea_orm::*;

use crate::common::model::Namespace;
use crate::entity::{config_info, tenant_info};

#[derive(Debug, FromQueryResult)]
struct SelectResult {
    tenant_id: Option<String>,
    count: i32,
}

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
        .into_model::<SelectResult>()
        .all(db)
        .await
        .unwrap()
        .iter()
        .map(|x| (x.tenant_id.clone().unwrap_or_default(), x.count))
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
    namespace_id: String,
) -> Option<Namespace> {
    let mut namspace: Namespace;

    if namespace_id.is_empty() || namespace_id.eq(DEFAULT_NAMESPACE) {
        namspace = Namespace::default();
    } else {
        let tenant_info_option = tenant_info::Entity::find()
            .filter(tenant_info::Column::TenantId.eq(namespace_id))
            .one(db)
            .await
            .unwrap();

        if tenant_info_option.is_none() {
            return None;
        }

        let tenant_info = tenant_info_option.unwrap();

        namspace = Namespace::from(tenant_info.clone());
    }

    let config_info = config_info::Entity::find()
        .column(config_info::Column::TenantId)
        .column_as(config_info::Column::Id.count(), "count")
        .filter(config_info::Column::TenantId.eq(namspace.namespace.clone()))
        .filter(config_info::Column::TenantId.is_not_null())
        .group_by(config_info::Column::TenantId)
        .into_model::<SelectResult>()
        .one(db)
        .await
        .unwrap();

    if config_info.is_some() {
        namspace.config_count = config_info.unwrap().count;
    }

    return Some(namspace);
}

pub async fn create(
    db: &DatabaseConnection,
    namespace_id: String,
    namespace_name: String,
    namespace_desc: String,
) -> bool {
    let entity = tenant_info::ActiveModel {
        tenant_id: Set(Some(namespace_id)),
        tenant_name: Set(Some(namespace_name)),
        tenant_desc: Set(Some(namespace_desc)),
        kp: Set(DEFAULT_KP.to_string()),
        create_source: Set(Some(DEFAULT_CREATE_SOURCE.to_string())),
        gmt_create: Set(chrono::Utc::now().timestamp_millis()),
        gmt_modified: Set(chrono::Utc::now().timestamp_millis()),
        ..Default::default()
    };

    let res = tenant_info::Entity::insert(entity).exec(db).await;

    if res.is_err() {
        return false;
    }

    return true;
}

pub async fn get_count_by_tenant_id(db: &DatabaseConnection, namespace_id: String) -> u64 {
    return tenant_info::Entity::find()
        .filter(tenant_info::Column::TenantId.eq(namespace_id))
        .count(db)
        .await
        .unwrap();
}

pub async fn update(
    db: &DatabaseConnection,
    namespace: String,
    namespace_show_name: String,
    namespace_desc: String,
) -> bool {
    let entity_option = tenant_info::Entity::find()
        .filter(tenant_info::Column::TenantId.eq(namespace))
        .one(db)
        .await
        .unwrap();

    if entity_option.is_none() {
        return false;
    }

    let mut entity: tenant_info::ActiveModel = entity_option.unwrap().into();

    entity.tenant_name = Set(Some(namespace_show_name));
    entity.tenant_desc = Set(Some(namespace_desc));

    if entity.is_changed() {
        entity.gmt_modified = Set(chrono::Utc::now().timestamp_millis());

        let res = entity.update(db).await;

        if res.is_err() {
            return false;
        }
    }

    return true;
}

pub async fn delete(db: &DatabaseConnection, namespace_id: String) -> bool {
    let res = tenant_info::Entity::delete_many()
        .filter(tenant_info::Column::TenantId.eq(namespace_id))
        .exec(db)
        .await;

    if res.is_err() {
        return false;
    }

    return true;
}
