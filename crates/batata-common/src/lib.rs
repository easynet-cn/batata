//! Batata Common - Shared types, traits, and utilities
//!
//! This crate provides the foundational types used across all Batata components:
//! - Error types and error codes
//! - Context traits for dependency injection
//! - Utility functions
//! - Common constants
//! - Configuration encryption
#![warn(missing_docs)]
pub mod ai_trace;
pub mod bincode;
pub mod crypto;
pub mod error;
/// Shared macros used across Batata components.
#[macro_use]
pub mod macros;
/// Data models shared across Batata components.
pub mod model;
pub mod server_status;
pub mod traits;
pub mod utils;

// Re-exports for convenience
pub use error::{AppError, BatataError, ErrorCode};
pub use traits::*;
pub use utils::{get_or_compile_regex, glob_matches, is_valid, regex_matches};

/// Default namespace ID used when no namespace is specified
pub const DEFAULT_NAMESPACE_ID: &str = "public";

/// Default group name
pub const DEFAULT_GROUP: &str = "DEFAULT_GROUP";

/// Default page number for pagination (used with serde default)
pub fn default_page_no() -> u64 {
    1
}

/// Default page size for pagination - large (used with serde default)
pub fn default_page_size() -> u64 {
    100
}

/// Default page size for pagination - small (used with serde default)
pub fn default_page_size_small() -> u64 {
    20
}

// ============================================================================
// Upstream-compatible parameter aliasing
// ============================================================================

/// Returns the first candidate that is not blank, with surrounding
/// whitespace removed.
///
/// Upstream Nacos uses `StringUtils.isBlank` rather than a null check when it
/// walks its alias chains, so `namespaceId=&tenant=ns` resolves to `ns`, and
/// `NamespaceUtil.processNamespaceParameter` trims the winner before use.
#[inline]
fn first_non_blank<'a>(candidates: &[Option<&'a str>]) -> Option<&'a str> {
    candidates
        .iter()
        .copied()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
}

/// Resolves the namespace ID from the v1/v2/v3-compatible request parameter names.
///
/// Upstream Nacos does **not** treat `namespaceId`, `tenant` and `namespace` as
/// interchangeable aliases. `ConfigDefaultHttpParamExtractor` applies a
/// precedence chain with blank-value fallback — `namespaceId` -> `tenant` ->
/// `namespace` — and `NamespaceUtil.processNamespaceParameter` then trims the
/// winner, mapping a blank result to the default namespace.
///
/// A serde `alias` cannot express this: every alias shares a single field
/// variant, so a request carrying more than one of the names is rejected with
/// `duplicate field` (`serde_derive` emits `Err::duplicate_field` once the
/// field is already `Some`), and an explicitly empty value does not fall
/// through to the next candidate.
///
/// # Examples
/// ```
/// use batata_common::resolve_namespace_id;
///
/// assert_eq!(resolve_namespace_id(Some("ns-1"), None, None), "ns-1");
/// // Blank values fall through, and the winner is trimmed.
/// assert_eq!(resolve_namespace_id(Some("  "), Some(" ns-2 "), None), "ns-2");
/// assert_eq!(resolve_namespace_id(None, Some("ns-3"), Some("ns-4")), "ns-3");
/// assert_eq!(resolve_namespace_id(None, None, None), "public");
/// ```
pub fn resolve_namespace_id(
    namespace_id: Option<&str>,
    tenant: Option<&str>,
    namespace: Option<&str>,
) -> String {
    first_non_blank(&[namespace_id, tenant, namespace])
        .map(str::to_owned)
        .unwrap_or_else(|| DEFAULT_NAMESPACE_ID.to_owned())
}

/// Resolves the group name from the v1/v2/v3-compatible request parameter names,
/// returning `None` when every candidate is blank.
///
/// `ConfigDefaultHttpParamExtractor` prefers `groupName` and falls back to
/// `group` on a blank value; `ConfigFormV3` (the v3 form object) only carries
/// `groupName`. Callers that treat the group as required should use this
/// function so a missing value stays distinguishable from an explicit one.
///
/// # Examples
/// ```
/// use batata_common::resolve_group_opt;
///
/// assert_eq!(resolve_group_opt(Some("G1"), Some("G2")), Some("G1".to_string()));
/// assert_eq!(resolve_group_opt(None, Some(" G2 ")), Some("G2".to_string()));
/// assert_eq!(resolve_group_opt(Some(" "), None), None);
/// ```
pub fn resolve_group_opt(group_name: Option<&str>, group: Option<&str>) -> Option<String> {
    first_non_blank(&[group_name, group]).map(str::to_owned)
}

/// Resolves the group name, falling back to [`DEFAULT_GROUP`] when every
/// candidate is blank.
///
/// # Examples
/// ```
/// use batata_common::resolve_group;
///
/// assert_eq!(resolve_group(Some("G1"), None), "G1");
/// assert_eq!(resolve_group(None, None), "DEFAULT_GROUP");
/// ```
pub fn resolve_group(group_name: Option<&str>, group: Option<&str>) -> String {
    resolve_group_opt(group_name, group).unwrap_or_else(|| DEFAULT_GROUP.to_owned())
}

// ============================================================================
// Key Builders — centralized to prevent format inconsistencies
// ============================================================================

/// Build a naming service key: "namespace@@group@@service"
///
/// This is the canonical key format used across naming service, health check,
/// distro protocol, persistence layer, and Raft state machine.
#[inline]
pub fn build_service_key(namespace: &str, group: &str, service: &str) -> String {
    let mut key = String::with_capacity(namespace.len() + group.len() + service.len() + 4);

    key.push_str(namespace);
    key.push_str("@@");
    key.push_str(group);
    key.push_str("@@");
    key.push_str(service);

    key
}

/// Parse a service key into (namespace, group, service) components.
///
/// Returns `None` if the key doesn't contain exactly 2 `@@` separators.
#[inline]
pub fn parse_service_key(key: &str) -> Option<(&str, &str, &str)> {
    let mut parts = key.splitn(3, "@@");

    let ns = parts.next()?;
    let group = parts.next()?;
    let service = parts.next()?;

    Some((ns, group, service))
}

/// Build a config group key: "dataId+group+tenant"
///
/// This is the canonical key format used for config persistence,
/// listener change detection, and batch MD5 comparison.
/// Compatible with Nacos Java `GroupKey.getKeyTenant(dataId, group, tenant)`.
#[inline]
pub fn build_config_key(data_id: &str, group: &str, tenant: &str) -> String {
    let mut key = String::with_capacity(data_id.len() + group.len() + tenant.len() + 2);

    key.push_str(data_id);
    key.push('+');
    key.push_str(group);
    key.push('+');
    key.push_str(tenant);

    key
}

/// Parse a config group key into (data_id, group, tenant) components.
#[inline]
pub fn parse_config_key(key: &str) -> Option<(&str, &str, &str)> {
    let mut parts = key.splitn(3, '+');

    let data_id = parts.next()?;
    let group = parts.next()?;
    let tenant = parts.next()?;

    Some((data_id, group, tenant))
}

// ============================================================================
// Pagination Utilities
// ============================================================================

/// Calculate pagination offsets from page number and page size.
///
/// Returns `(start_index, end_index)` for slicing a sorted collection.
/// `page_no` is 1-based; values < 1 are treated as 1.
///
/// # Example
/// ```
/// use batata_common::paginate;
/// let (start, end) = paginate(2, 10, 25); // page 2, 10 per page, 25 total
/// assert_eq!(start, 10);
/// assert_eq!(end, 20);
/// ```
#[inline]
pub fn paginate(page_no: u64, page_size: u64, total: usize) -> (usize, usize) {
    let start = ((page_no.max(1) - 1) * page_size) as usize;
    let end = (start + page_size as usize).min(total);

    (start, end)
}

/// Query parameter names
pub const TENANT: &str = "tenant";
/// Query parameter for the namespace ID.
pub const NAMESPACE_ID: &str = "namespaceId";
/// Query parameter for the group name.
pub const GROUP: &str = "group";
/// Query parameter for the group name (alternate key).
pub const GROUP_NAME: &str = "groupName";
/// Query parameter for the data ID.
pub const DATA_ID: &str = "dataId";
/// Query parameter for the service name.
pub const SERVICE_NAME: &str = "serviceName";

/// Action types for permission control
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionTypes {
    #[default]
    /// The `Read` variant.
    Read,
    /// The `Write` variant.
    Write,
}

impl ActionTypes {
    /// The `as_str` method.
    pub fn as_str(self) -> &'static str {
        match self {
            ActionTypes::Read => "r",
            ActionTypes::Write => "w",
        }
    }
}

impl std::fmt::Display for ActionTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ActionTypes {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "r" => Ok(ActionTypes::Read),
            "w" => Ok(ActionTypes::Write),
            _ => Err(format!("Invalid action: {}", s)),
        }
    }
}

/// Signature types for different service modules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignType {
    #[default]
    /// The `Naming` variant.
    Naming,
    /// The `Config` variant.
    Config,
    /// The `Lock` variant.
    Lock,
    /// The `Ai` variant.
    Ai,
    /// The `Console` variant.
    Console,
    /// The `Specified` variant.
    Specified,
}

impl SignType {
    /// The `as_str` method.
    pub fn as_str(&self) -> &'static str {
        match self {
            SignType::Naming => "naming",
            SignType::Config => "config",
            SignType::Lock => "lock",
            SignType::Ai => "ai",
            SignType::Console => "console",
            SignType::Specified => "specified",
        }
    }
}

impl std::fmt::Display for SignType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for SignType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "naming" => Ok(SignType::Naming),
            "config" => Ok(SignType::Config),
            "lock" => Ok(SignType::Lock),
            "ai" => Ok(SignType::Ai),
            "console" => Ok(SignType::Console),
            "specified" => Ok(SignType::Specified),
            _ => Err(format!("Invalid sign type: {}", s)),
        }
    }
}

/// API access types with different permission levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ApiType {
    /// The `AdminApi` variant.
    AdminApi,
    /// The `ConsoleApi` variant.
    ConsoleApi,
    #[default]
    /// The `OpenApi` variant.
    OpenApi,
    /// The `InnerApi` variant.
    InnerApi,
}

impl ApiType {
    /// The `description` method.
    pub fn description(&self) -> &'static str {
        match self {
            ApiType::AdminApi => "ADMIN_API",
            ApiType::ConsoleApi => "CONSOLE_API",
            ApiType::OpenApi => "OPEN_API",
            ApiType::InnerApi => "INNER_API",
        }
    }
}

impl std::fmt::Display for ApiType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl std::str::FromStr for ApiType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ADMIN_API" => Ok(ApiType::AdminApi),
            "CONSOLE_API" => Ok(ApiType::ConsoleApi),
            "OPEN_API" => Ok(ApiType::OpenApi),
            "INNER_API" => Ok(ApiType::InnerApi),
            _ => Err(format!("Invalid API type: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_types() {
        assert_eq!(ActionTypes::default(), ActionTypes::Read);
        assert_eq!(ActionTypes::Read.as_str(), "r");
        assert_eq!(ActionTypes::Write.as_str(), "w");
        assert_eq!("r".parse::<ActionTypes>().unwrap(), ActionTypes::Read);
        assert_eq!("w".parse::<ActionTypes>().unwrap(), ActionTypes::Write);
    }

    #[test]
    fn test_sign_type() {
        assert_eq!(SignType::default(), SignType::Naming);
        assert_eq!(SignType::Config.as_str(), "config");
        assert_eq!("config".parse::<SignType>().unwrap(), SignType::Config);
    }

    #[test]
    fn test_api_type() {
        assert_eq!(ApiType::default(), ApiType::OpenApi);
        assert_eq!(ApiType::AdminApi.description(), "ADMIN_API");
        assert_eq!("ADMIN_API".parse::<ApiType>().unwrap(), ApiType::AdminApi);
    }

    #[test]
    fn test_action_types_display() {
        assert_eq!(format!("{}", ActionTypes::Read), "r");
        assert_eq!(format!("{}", ActionTypes::Write), "w");
    }

    #[test]
    fn test_action_types_from_str_invalid() {
        assert!("x".parse::<ActionTypes>().is_err());
        assert!("read".parse::<ActionTypes>().is_err());
        assert!("write".parse::<ActionTypes>().is_err());
        assert!("".parse::<ActionTypes>().is_err());
    }

    #[test]
    fn test_sign_type_all_variants() {
        let variants = vec![
            (SignType::Naming, "naming"),
            (SignType::Config, "config"),
            (SignType::Lock, "lock"),
            (SignType::Ai, "ai"),
            (SignType::Console, "console"),
            (SignType::Specified, "specified"),
        ];

        for (variant, expected) in variants {
            assert_eq!(variant.as_str(), expected);
            assert_eq!(format!("{}", variant), expected);
            assert_eq!(expected.parse::<SignType>().unwrap(), variant);
        }
    }

    #[test]
    fn test_sign_type_from_str_invalid() {
        assert!("unknown".parse::<SignType>().is_err());
        assert!("".parse::<SignType>().is_err());
    }

    #[test]
    fn test_api_type_all_variants() {
        let variants = vec![
            (ApiType::AdminApi, "ADMIN_API"),
            (ApiType::ConsoleApi, "CONSOLE_API"),
            (ApiType::OpenApi, "OPEN_API"),
            (ApiType::InnerApi, "INNER_API"),
        ];

        for (variant, expected) in variants {
            assert_eq!(variant.description(), expected);
            assert_eq!(format!("{}", variant), expected);
            assert_eq!(expected.parse::<ApiType>().unwrap(), variant);
        }
    }

    #[test]
    fn test_api_type_from_str_invalid() {
        assert!("UNKNOWN".parse::<ApiType>().is_err());
        assert!("admin_api".parse::<ApiType>().is_err()); // Case sensitive
    }

    #[test]
    fn test_default_pagination() {
        assert_eq!(default_page_no(), 1);
        assert_eq!(default_page_size(), 100);
        assert_eq!(default_page_size_small(), 20);
    }

    #[test]
    fn test_build_service_key() {
        assert_eq!(
            build_service_key("public", "DEFAULT_GROUP", "my-service"),
            "public@@DEFAULT_GROUP@@my-service"
        );
    }

    #[test]
    fn test_parse_service_key() {
        let (ns, group, svc) = parse_service_key("public@@DEFAULT_GROUP@@my-service").unwrap();

        assert_eq!(ns, "public");
        assert_eq!(group, "DEFAULT_GROUP");
        assert_eq!(svc, "my-service");
        assert!(parse_service_key("invalid").is_none());
        assert!(parse_service_key("a@@b").is_none());
    }

    #[test]
    fn test_build_config_key() {
        assert_eq!(
            build_config_key("app.yaml", "DEFAULT_GROUP", "public"),
            "app.yaml+DEFAULT_GROUP+public"
        );
    }

    #[test]
    fn test_parse_config_key() {
        let (data_id, group, tenant) = parse_config_key("app.yaml+DEFAULT_GROUP+public").unwrap();

        assert_eq!(data_id, "app.yaml");
        assert_eq!(group, "DEFAULT_GROUP");
        assert_eq!(tenant, "public");
    }

    #[test]
    fn test_paginate() {
        // Page 1 of 25 items, 10 per page
        assert_eq!(paginate(1, 10, 25), (0, 10));
        // Page 2
        assert_eq!(paginate(2, 10, 25), (10, 20));
        // Page 3 (partial)
        assert_eq!(paginate(3, 10, 25), (20, 25));
        // Page beyond range
        assert_eq!(paginate(4, 10, 25), (30, 25)); // start > total, end capped
        // Page 0 treated as 1
        assert_eq!(paginate(0, 10, 25), (0, 10));
        // Empty collection
        assert_eq!(paginate(1, 10, 0), (0, 0));
    }

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_NAMESPACE_ID, "public");
        assert_eq!(DEFAULT_GROUP, "DEFAULT_GROUP");
        assert_eq!(TENANT, "tenant");
        assert_eq!(NAMESPACE_ID, "namespaceId");
        assert_eq!(GROUP, "group");
        assert_eq!(GROUP_NAME, "groupName");
        assert_eq!(DATA_ID, "dataId");
        assert_eq!(SERVICE_NAME, "serviceName");
    }
}
