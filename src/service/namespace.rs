use sea_orm::*;

use crate::core::model::Namespace;
use crate::entity::tenant_info;

pub async fn find_all(db: &DatabaseConnection) -> Vec<Namespace> {
    let tenant_infos: Vec<tenant_info::Model> = tenant_info::Entity::find().all(db).await.unwrap();

    let namespaces: Vec<Namespace> = tenant_infos
        .iter()
        .map(|tenant_info| Namespace {
            namespace: tenant_info.tenant_id.clone().unwrap_or_default(),
            namespace_show_name: tenant_info.tenant_name.clone().unwrap_or_default(),
            namespace_desc: tenant_info.tenant_desc.clone().unwrap_or_default(),
            quota: 0,
            config_count: 0,
            type_: 0,
        })
        .collect();

    namespaces
}
