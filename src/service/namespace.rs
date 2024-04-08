use std::collections::HashMap;

use sea_orm::*;

use crate::core::model::Namespace;
use crate::entity::{config_info, tenant_info};

#[derive(Debug, FromQueryResult)]
struct SelectResult {
    tenant_id: Option<String>,
    count: i32,
}

const DEFAULT_NAMESPACE: &'static str = "public";
const DEFAULT_NAMESPACE_SHOW_NAME: &'static str = "Public";
const DEFAULT_NAMESPACE_DESCRIPTION: &'static str = "Public Namespace";
const DEFAULT_NAMESPACE_QUOTA: i32 = 200;
const DEFAULT_CREATE_SOURCE: &'static str = "nacos";
const DEFAULT_KP: &'static str = "1";

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

            Namespace {
                namespace: tenant_info.tenant_id.clone().unwrap_or_default(),
                namespace_show_name: tenant_info.tenant_name.clone().unwrap_or_default(),
                namespace_desc: tenant_info.tenant_desc.clone().unwrap_or_default(),
                quota: DEFAULT_NAMESPACE_QUOTA,
                config_count: 0,
                type_: 2,
            }
        })
        .collect();

    namespaces.insert(
        0,
        Namespace {
            namespace: "".to_string(),
            namespace_show_name: DEFAULT_NAMESPACE.to_string(),
            namespace_desc: "".to_string(),
            quota: DEFAULT_NAMESPACE_QUOTA,
            config_count: 0,
            type_: 0,
        },
    );

    tenant_ids.push("".to_string());

    let config_infos = config_info::Entity::find()
        .select_only()
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
        namspace = Namespace {
            namespace: "".to_string(),
            namespace_show_name: DEFAULT_NAMESPACE.to_string(),
            namespace_desc: DEFAULT_NAMESPACE_DESCRIPTION.to_string(),
            quota: DEFAULT_NAMESPACE_QUOTA,
            config_count: 0,
            type_: 0,
        };
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

        namspace = Namespace {
            namespace: tenant_info.tenant_id.clone().unwrap_or_default(),
            namespace_show_name: tenant_info.tenant_name.clone().unwrap_or_default(),
            namespace_desc: tenant_info.tenant_desc.clone().unwrap_or_default(),
            quota: DEFAULT_NAMESPACE_QUOTA,
            config_count: 0,
            type_: 2,
        };
    }

    let config_info = config_info::Entity::find()
        .select_only()
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
