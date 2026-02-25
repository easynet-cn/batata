//! Namespace data models
//!
//! This module defines data structures for namespace management.

use serde::{Deserialize, Serialize};

use batata_persistence::entity::tenant_info;

/// Default namespace ID
pub const DEFAULT_NAMESPACE_ID: &str = "public";
/// Default namespace display name
pub const DEFAULT_NAMESPACE_SHOW_NAME: &str = "Public";
/// Default namespace description
pub const DEFAULT_NAMESPACE_DESCRIPTION: &str = "Public Namespace";
/// Default namespace quota
pub const DEFAULT_NAMESPACE_QUOTA: i32 = 200;

/// Namespace information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Namespace {
    pub namespace: String,
    pub namespace_show_name: String,
    pub namespace_desc: String,
    pub quota: i32,
    pub config_count: i32,
    #[serde(rename = "type")]
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

impl From<tenant_info::Model> for Namespace {
    fn from(value: tenant_info::Model) -> Self {
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

impl From<batata_persistence::NamespaceInfo> for Namespace {
    fn from(value: batata_persistence::NamespaceInfo) -> Self {
        let type_ = if value.namespace_id == DEFAULT_NAMESPACE_ID {
            0
        } else {
            2
        };
        Self {
            namespace: value.namespace_id,
            namespace_show_name: value.namespace_name,
            namespace_desc: value.namespace_desc,
            quota: value.quota,
            config_count: value.config_count,
            type_,
        }
    }
}

/// Namespace creation form
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NamespaceForm {
    pub namespace_id: String,
    pub namespace_name: String,
    pub namespace_desc: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_default() {
        let ns = Namespace::default();
        assert_eq!(ns.namespace, "public");
        assert_eq!(ns.namespace_show_name, "Public");
        assert_eq!(ns.quota, 200);
        assert_eq!(ns.config_count, 0);
    }

    #[test]
    fn test_namespace_serialization() {
        let ns = Namespace::default();
        let json = serde_json::to_string(&ns).unwrap();
        assert!(json.contains("\"namespace\":\"public\""));
        assert!(json.contains("\"type\":0"));
    }
}
