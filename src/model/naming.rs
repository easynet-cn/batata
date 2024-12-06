use serde::{Deserialize, Serialize};

use crate::entity;

const DEFAULT_NAMESPACE_QUOTA: i32 = 200;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Namespace {
    pub namespace: String,
    pub namespace_show_name: String,
    pub namespace_desc: String,
    pub quota: i32,
    pub config_count: i32,
    pub type_: i32,
}

impl Default for Namespace {
    fn default() -> Self {
        Namespace {
            namespace: String::from(""),
            namespace_show_name: String::from("public"),
            namespace_desc: String::from("Public Namespace"),
            quota: 200,
            config_count: 0,
            type_: 0,
        }
    }
}

impl From<entity::tenant_info::Model> for Namespace {
    fn from(value: entity::tenant_info::Model) -> Self {
        Self {
            namespace: value.tenant_id.unwrap_or_default(),
            namespace_show_name: value.tenant_name.unwrap_or_default(),
            namespace_desc: value.tenant_desc.unwrap_or_default(),
            quota: DEFAULT_NAMESPACE_QUOTA,
            config_count: 0,
            type_: 2,
        }
    }
}
