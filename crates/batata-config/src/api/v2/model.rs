//! Request and response models for V2 Config API
//!
//! These models follow the Nacos V2 API specification with camelCase JSON serialization.

use batata_common::{default_page_no, default_page_size, resolve_group_opt, resolve_namespace_id};
use serde::{Deserialize, Serialize};

/// Implements the upstream Nacos parameter resolution chains.
///
/// `namespaceId`, `tenant` and `namespace` are **not** interchangeable
/// aliases: `ConfigDefaultHttpParamExtractor` walks them as a precedence chain
/// with blank-value fallback, and `NamespaceUtil.processNamespaceParameter`
/// trims the winner. Likewise `groupName` is preferred over `group`, and
/// `ConfigFormV3` only carries `groupName`.
///
/// Serde aliases cannot model this — they share one field variant, so a request
/// carrying two of the names fails with `duplicate field`, and an empty value
/// never falls through. The candidates are therefore kept as separate fields
/// and resolved here.
macro_rules! impl_param_resolvers {
    () => {
        /// Group name resolved from `groupName`, falling back to `group`.
        ///
        /// `None` when every candidate is blank, so callers can still report a
        /// missing required parameter.
        pub fn resolved_group(&self) -> Option<String> {
            resolve_group_opt(self.group_name.as_deref(), self.group.as_deref())
        }

        /// Namespace ID resolved from `namespaceId` -> `tenant` -> `namespace`,
        /// falling back to the default namespace.
        pub fn resolved_namespace_id(&self) -> String {
            resolve_namespace_id(
                self.namespace_id.as_deref(),
                self.tenant.as_deref(),
                self.namespace.as_deref(),
            )
        }
    };
}

// =============================================================================
// Config API Models
// =============================================================================

/// Request parameters for getting a config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGetParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required). `groupName` wins; `group` is the v2 legacy name.
    #[serde(default)]
    pub group: Option<String>,
    /// Group name as sent by v3 clients and by `ConfigFormV3`
    #[serde(default)]
    pub group_name: Option<String>,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// `tenant` — the namespace name used by the v1/v2 client protocol
    #[serde(default)]
    pub tenant: Option<String>,
    /// `namespace` — the third fallback accepted upstream
    #[serde(default)]
    pub namespace: Option<String>,
    /// Tag for config filtering (optional)
    #[serde(default)]
    pub tag: Option<String>,
}

impl ConfigGetParam {
    impl_param_resolvers!();
}

/// Request parameters for publishing a config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPublishParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required). `groupName` wins; `group` is the v2 legacy name.
    #[serde(default)]
    pub group: Option<String>,
    /// Group name as sent by v3 clients and by `ConfigFormV3`
    #[serde(default)]
    pub group_name: Option<String>,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// `tenant` — the namespace name used by the v1/v2 client protocol
    #[serde(default)]
    pub tenant: Option<String>,
    /// `namespace` — the third fallback accepted upstream
    #[serde(default)]
    pub namespace: Option<String>,
    /// Config content (required)
    pub content: String,
    /// Tag for config (optional)
    #[serde(default)]
    pub tag: Option<String>,
    /// Application name (optional)
    #[serde(default, alias = "appName")]
    pub app_name: Option<String>,
    /// Source user (optional)
    #[serde(default, alias = "srcUser")]
    pub src_user: Option<String>,
    /// Config tags (comma-separated, optional)
    #[serde(default, alias = "configTags")]
    pub config_tags: Option<String>,
    /// Description (optional)
    #[serde(default)]
    pub desc: Option<String>,
    /// Usage information (optional)
    #[serde(default)]
    pub r#use: Option<String>,
    /// Effect type (optional)
    #[serde(default)]
    pub effect: Option<String>,
    /// Config type (e.g., "yaml", "json", "properties", optional)
    #[serde(default)]
    pub r#type: Option<String>,
    /// Schema for config validation (optional)
    #[serde(default)]
    pub schema: Option<String>,
    /// Encrypted data key (optional)
    #[serde(default)]
    pub encrypted_data_key: Option<String>,
}

impl ConfigPublishParam {
    impl_param_resolvers!();
}

/// Request parameters for deleting a config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDeleteParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required). `groupName` wins; `group` is the v2 legacy name.
    #[serde(default)]
    pub group: Option<String>,
    /// Group name as sent by v3 clients and by `ConfigFormV3`
    #[serde(default)]
    pub group_name: Option<String>,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// `tenant` — the namespace name used by the v1/v2 client protocol
    #[serde(default)]
    pub tenant: Option<String>,
    /// `namespace` — the third fallback accepted upstream
    #[serde(default)]
    pub namespace: Option<String>,
    /// Tag for config (optional)
    #[serde(default)]
    pub tag: Option<String>,
}

impl ConfigDeleteParam {
    impl_param_resolvers!();
}

/// Response data for config retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    /// Config ID
    pub id: String,
    /// Data ID
    pub data_id: String,
    /// Group name
    pub group: String,
    /// Config content
    pub content: String,
    /// MD5 hash of content
    pub md5: String,
    /// Encrypted data key (if encrypted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_data_key: Option<String>,
    /// Namespace/tenant ID
    pub tenant: String,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// Config type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// Request parameters for searching config detail
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSearchDetailParam {
    /// Data ID filter (optional, supports wildcard *)
    #[serde(default)]
    pub data_id: Option<String>,
    /// Group filter (optional). `groupName` wins; `group` is the legacy name.
    #[serde(default)]
    pub group: Option<String>,
    /// Group filter as sent by v3 clients
    #[serde(default)]
    pub group_name: Option<String>,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// `tenant` — the namespace name used by the v1/v2 client protocol
    #[serde(default)]
    pub tenant: Option<String>,
    /// `namespace` — the third fallback accepted upstream
    #[serde(default)]
    pub namespace: Option<String>,
    /// Application name filter (optional)
    #[serde(default)]
    pub app_name: Option<String>,
    /// Config tags filter (comma-separated, optional)
    #[serde(default)]
    pub config_tags: Option<String>,
    /// Config type filter (comma-separated, optional)
    #[serde(default)]
    pub config_type: Option<String>,
    /// Content search filter (optional)
    #[serde(default)]
    pub content: Option<String>,
    /// Search type (blur/accurate, defaults to "blur")
    #[serde(default = "default_search")]
    pub search: String,
    /// Config detail/content search filter (optional)
    #[serde(default)]
    pub config_detail: Option<String>,
    /// Page number (1-based, defaults to 1)
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    /// Page size (defaults to 100)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

impl ConfigSearchDetailParam {
    impl_param_resolvers!();
}

// =============================================================================
// History API Models
// =============================================================================

/// Request parameters for history list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryListParam {
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required). `groupName` wins; `group` is the v2 legacy name.
    #[serde(default)]
    pub group: Option<String>,
    /// Group name as sent by v3 clients and by `ConfigFormV3`
    #[serde(default)]
    pub group_name: Option<String>,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// `tenant` — the namespace name used by the v1/v2 client protocol
    #[serde(default)]
    pub tenant: Option<String>,
    /// `namespace` — the third fallback accepted upstream
    #[serde(default)]
    pub namespace: Option<String>,
    /// Page number (1-based, defaults to 1)
    #[serde(default = "default_page_no")]
    pub page_no: u64,
    /// Page size (defaults to 100)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

fn default_search() -> String {
    "blur".to_string()
}

impl HistoryListParam {
    impl_param_resolvers!();
}

/// Request parameters for getting a specific history entry
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDetailParam {
    /// History entry ID (nid)
    pub nid: i64,
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required). `groupName` wins; `group` is the v2 legacy name.
    #[serde(default)]
    pub group: Option<String>,
    /// Group name as sent by v3 clients and by `ConfigFormV3`
    #[serde(default)]
    pub group_name: Option<String>,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// `tenant` — the namespace name used by the v1/v2 client protocol
    #[serde(default)]
    pub tenant: Option<String>,
    /// `namespace` — the third fallback accepted upstream
    #[serde(default)]
    pub namespace: Option<String>,
}

impl HistoryDetailParam {
    impl_param_resolvers!();
}

/// Request parameters for getting the previous history entry
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPreviousParam {
    /// Current history entry ID
    pub id: i64,
    /// Data ID of the config (required)
    pub data_id: String,
    /// Group name (required). `groupName` wins; `group` is the v2 legacy name.
    #[serde(default)]
    pub group: Option<String>,
    /// Group name as sent by v3 clients and by `ConfigFormV3`
    #[serde(default)]
    pub group_name: Option<String>,
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// `tenant` — the namespace name used by the v1/v2 client protocol
    #[serde(default)]
    pub tenant: Option<String>,
    /// `namespace` — the third fallback accepted upstream
    #[serde(default)]
    pub namespace: Option<String>,
}

impl HistoryPreviousParam {
    impl_param_resolvers!();
}

/// Request parameters for getting configs in a namespace
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceConfigsParam {
    /// Namespace ID (optional, defaults to "public")
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// `tenant` — the namespace name used by the v1/v2 client protocol
    #[serde(default)]
    pub tenant: Option<String>,
    /// `namespace` — the third fallback accepted upstream
    #[serde(default)]
    pub namespace: Option<String>,
}

impl NamespaceConfigsParam {
    /// Namespace ID resolved from `namespaceId` -> `tenant` -> `namespace`.
    pub fn resolved_namespace_id(&self) -> String {
        resolve_namespace_id(
            self.namespace_id.as_deref(),
            self.tenant.as_deref(),
            self.namespace.as_deref(),
        )
    }
}

/// Response data for history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItemResponse {
    /// History entry ID
    pub id: String,
    /// Last modified ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<i64>,
    /// Data ID
    pub data_id: String,
    /// Group name
    pub group: String,
    /// Namespace/tenant ID
    pub tenant: String,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// MD5 hash of content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    /// Content (only in detail view)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Source IP address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    /// Source user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_user: Option<String>,
    /// Operation type (I=Insert, U=Update, D=Delete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_type: Option<String>,
    /// Publish type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_type: Option<String>,
    /// Gray/canary release name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gray_name: Option<String>,
    /// Extended information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext_info: Option<String>,
    /// Config type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Creation time (timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    /// Last modified time (timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_time: Option<String>,
    /// Encrypted data key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_data_key: Option<String>,
}

/// Response for /v2/cs/history/detail - includes original and updated versions for comparison.
/// Matches Nacos ConfigHistoryInfoDetail.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryInfoDetail {
    /// History entry ID
    pub id: String,
    /// Last modified ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<i64>,
    /// Data ID
    pub data_id: String,
    /// Group name
    pub group: String,
    /// Namespace/tenant ID
    pub tenant: String,
    /// Operation type (I=Insert, U=Update, D=Delete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_type: Option<String>,
    /// Publish type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_type: Option<String>,
    /// Gray/canary release name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gray_name: Option<String>,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// Source IP
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    /// Source user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_user: Option<String>,
    /// MD5 hash of the original version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_md5: Option<String>,
    /// Content of the original version before this change
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_content: Option<String>,
    /// Encrypted data key of the original version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_encrypted_data_key: Option<String>,
    /// Extended info of the original version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_ext_info: Option<String>,
    /// MD5 hash of the updated version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_md5: Option<String>,
    /// Content of the updated version (for comparison)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_content: Option<String>,
    /// Encrypted data key of the updated version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_encrypted_data_key: Option<String>,
    /// Extended info of the updated version (note: Nacos uses "updateExtInfo" not "updatedExtInfo")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_ext_info: Option<String>,
    /// Creation time (timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    /// Last modified time (timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_time: Option<String>,
}

/// Response data for config info in namespace listing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfoResponse {
    /// Config ID
    pub id: String,
    /// Data ID
    pub data_id: String,
    /// Group name
    pub group: String,
    /// Namespace/tenant ID
    pub tenant: String,
    /// Application name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    /// Config type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Config content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// MD5 hash of content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    /// Encrypted data key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_data_key: Option<String>,
    /// Last modified time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_common::DEFAULT_NAMESPACE_ID;

    fn get(json: &str) -> ConfigGetParam {
        serde_json::from_str(json).expect("ConfigGetParam should deserialize")
    }

    #[test]
    fn namespace_id_wins_over_tenant_and_namespace() {
        let p = get(
            r#"{"dataId":"d","group":"G","namespaceId":"ns-a","tenant":"ns-b","namespace":"ns-c"}"#,
        );

        assert_eq!(p.resolved_namespace_id(), "ns-a");
    }

    #[test]
    fn blank_namespace_id_falls_through_to_tenant() {
        // Upstream uses StringUtils.isBlank, so an empty value is not a value.
        let p = get(r#"{"dataId":"d","group":"G","namespaceId":"  ","tenant":"ns-b"}"#);

        assert_eq!(p.resolved_namespace_id(), "ns-b");
    }

    #[test]
    fn namespace_is_the_last_fallback_before_default() {
        let p = get(r#"{"dataId":"d","group":"G","namespace":"ns-c"}"#);

        assert_eq!(p.resolved_namespace_id(), "ns-c");
    }

    #[test]
    fn namespace_defaults_to_public_when_all_candidates_are_blank() {
        let p = get(r#"{"dataId":"d","group":"G","namespaceId":"","tenant":""}"#);

        assert_eq!(p.resolved_namespace_id(), DEFAULT_NAMESPACE_ID);
    }

    #[test]
    fn namespace_is_trimmed() {
        let p = get(r#"{"dataId":"d","group":"G","tenant":"  ns-x  "}"#);

        assert_eq!(p.resolved_namespace_id(), "ns-x");
    }

    #[test]
    fn carrying_two_namespace_names_is_accepted_not_rejected() {
        // A serde alias would fail here with `duplicate field`; the candidates
        // are separate fields precisely so upstream clients may send both.
        let p = get(r#"{"dataId":"d","group":"G","namespaceId":"ns-a","tenant":"ns-a"}"#);

        assert_eq!(p.resolved_namespace_id(), "ns-a");
    }

    #[test]
    fn group_name_wins_over_group() {
        let p = get(r#"{"dataId":"d","groupName":"G1","group":"G2"}"#);

        assert_eq!(p.resolved_group().as_deref(), Some("G1"));
    }

    #[test]
    fn group_alone_is_still_accepted() {
        let p = get(r#"{"dataId":"d","group":"G2"}"#);

        assert_eq!(p.resolved_group().as_deref(), Some("G2"));
    }

    #[test]
    fn missing_group_stays_missing_so_callers_can_report_it() {
        let p = get(r#"{"dataId":"d","group":"  "}"#);

        assert_eq!(p.resolved_group(), None);
    }

    #[test]
    fn group_is_trimmed() {
        let p = get(r#"{"dataId":"d","groupName":"  G1  "}"#);

        assert_eq!(p.resolved_group().as_deref(), Some("G1"));
    }
}
