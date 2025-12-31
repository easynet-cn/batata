use serde::{Deserialize, Serialize};

use crate::{entity, model::common::DEFAULT_NAMESPACE_ID};

// Common parameters for service discovery.
pub const CODE: &str = "code";
pub const SERVICE_NAME: &str = "serviceName";
pub const CLUSTER_NAME: &str = "clusterName";
pub const NAMESPACE_ID: &str = "namespaceId";
pub const GROUP_NAME: &str = "groupName";
pub const LIGHT_BEAT_ENABLED: &str = "lightBeatEnabled";
pub const NAMING_REQUEST_TIMEOUT: &str = "namingRequestTimeout";

const DEFAULT_NAMESPACE_QUOTA: i32 = 200;
const DEFAULT_NAMESPACE_SHOW_NAME: &str = "public";
const DEFAULT_NAMESPACE_DESCRIPTION: &str = "Default Namespace";
#[allow(dead_code)]
const DEFAULT_CREATE_SOURCE: &str = "nacos";
#[allow(dead_code)]
const DEFAULT_KP: &str = "1";

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
            namespace: String::from(DEFAULT_NAMESPACE_ID),
            namespace_show_name: String::from(DEFAULT_NAMESPACE_SHOW_NAME),
            namespace_desc: String::from(DEFAULT_NAMESPACE_DESCRIPTION),
            quota: DEFAULT_NAMESPACE_QUOTA,
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
            type_: value.kp.parse().unwrap_or(1),
        }
    }
}
