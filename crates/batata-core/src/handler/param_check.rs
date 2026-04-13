//! gRPC request parameter validation.
//!
//! Validates namespace IDs, group names, data IDs, and service names
//! from gRPC requests before handler dispatch. Follows Nacos-compatible conventions.

use regex::Regex;
use std::sync::LazyLock;

/// Maximum allowed lengths for parameters (Nacos-compatible)
const MAX_NAMESPACE_ID_LEN: usize = 128;
const MAX_GROUP_LEN: usize = 128;
const MAX_DATA_ID_LEN: usize = 256;
const MAX_SERVICE_NAME_LEN: usize = 512;

/// Forbidden pattern in group names
const FORBIDDEN_GROUP_PATTERN: &str = "@@";

/// Valid namespace ID pattern: alphanumeric, hyphens, underscores
static NAMESPACE_ID_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_\-]*$").unwrap());

/// Parameter info extracted from a gRPC request
#[derive(Debug, Default)]
pub struct ParamInfo {
    pub namespace_id: Option<String>,
    pub group: Option<String>,
    pub data_id: Option<String>,
    pub service_name: Option<String>,
}

/// Validate extracted parameters. Returns Ok(()) if all valid.
pub fn validate_params(params: &ParamInfo) -> Result<(), String> {
    validate_params_ref(
        params.namespace_id.as_deref(),
        params.group.as_deref(),
        params.data_id.as_deref(),
        params.service_name.as_deref(),
    )
}

/// Zero-allocation parameter validation using string references.
fn validate_params_ref(
    namespace_id: Option<&str>,
    group: Option<&str>,
    data_id: Option<&str>,
    service_name: Option<&str>,
) -> Result<(), String> {
    if let Some(ns) = namespace_id
        && !ns.is_empty()
    {
        validate_namespace_id(ns)?;
    }
    if let Some(group) = group
        && !group.is_empty()
    {
        validate_group(group)?;
    }
    if let Some(data_id) = data_id
        && !data_id.is_empty()
    {
        validate_data_id(data_id)?;
    }
    if let Some(service_name) = service_name
        && !service_name.is_empty()
    {
        validate_service_name(service_name)?;
    }
    Ok(())
}

fn validate_namespace_id(ns: &str) -> Result<(), String> {
    if ns.len() > MAX_NAMESPACE_ID_LEN {
        return Err(format!(
            "Namespace ID too long: {} chars (max {})",
            ns.len(),
            MAX_NAMESPACE_ID_LEN
        ));
    }
    if !NAMESPACE_ID_REGEX.is_match(ns) {
        return Err(format!(
            "Invalid namespace ID '{}': only alphanumeric, hyphens and underscores allowed",
            ns
        ));
    }
    Ok(())
}

fn validate_group(group: &str) -> Result<(), String> {
    if group.len() > MAX_GROUP_LEN {
        return Err(format!(
            "Group name too long: {} chars (max {})",
            group.len(),
            MAX_GROUP_LEN
        ));
    }
    if group.contains(FORBIDDEN_GROUP_PATTERN) {
        return Err(format!(
            "Group name '{}' contains forbidden pattern '{}'",
            group, FORBIDDEN_GROUP_PATTERN
        ));
    }
    Ok(())
}

fn validate_data_id(data_id: &str) -> Result<(), String> {
    if data_id.len() > MAX_DATA_ID_LEN {
        return Err(format!(
            "Data ID too long: {} chars (max {})",
            data_id.len(),
            MAX_DATA_ID_LEN
        ));
    }
    Ok(())
}

fn validate_service_name(service_name: &str) -> Result<(), String> {
    if service_name.len() > MAX_SERVICE_NAME_LEN {
        return Err(format!(
            "Service name too long: {} chars (max {})",
            service_name.len(),
            MAX_SERVICE_NAME_LEN
        ));
    }
    Ok(())
}

/// Extract and validate parameters from a gRPC payload.
/// Returns Ok(()) if params are valid or if no params need checking.
pub fn check_request_params(
    message_type: &str,
    payload: &crate::api::grpc::Payload,
) -> Result<(), tonic::Status> {
    let headers = match payload.metadata.as_ref() {
        Some(m) => &m.headers,
        None => return Ok(()),
    };

    let result = match message_type {
        "ConfigPublishRequest" | "ConfigQueryRequest" | "ConfigRemoveRequest" => {
            let ns = headers
                .get("tenant")
                .or(headers.get("namespace"))
                .map(|s| s.as_str());
            let group = headers.get("group").map(|s| s.as_str());
            let data_id = headers.get("dataId").map(|s| s.as_str());
            validate_params_ref(ns, group, data_id, None)
        }
        "ConfigBatchListenRequest" | "ConfigFuzzyWatchRequest" => {
            let ns = headers
                .get("tenant")
                .or(headers.get("namespace"))
                .map(|s| s.as_str());
            validate_params_ref(ns, None, None, None)
        }
        "InstanceRequest"
        | "BatchInstanceRequest"
        | "PersistentInstanceRequest"
        | "ServiceQueryRequest"
        | "SubscribeServiceRequest"
        | "ServiceListRequest" => {
            let ns = headers
                .get("namespace")
                .or(headers.get("namespaceId"))
                .map(|s| s.as_str());
            let group = headers
                .get("groupName")
                .or(headers.get("group"))
                .map(|s| s.as_str());
            let svc = headers.get("serviceName").map(|s| s.as_str());
            validate_params_ref(ns, group, None, svc)
        }
        _ => return Ok(()),
    };

    result.map_err(|msg| {
        tracing::warn!(
            message_type = message_type,
            "Parameter validation failed: {}",
            msg
        );
        tonic::Status::invalid_argument(msg)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_namespace_id() {
        assert!(validate_namespace_id("public").is_ok());
        assert!(validate_namespace_id("my-namespace").is_ok());
        assert!(validate_namespace_id("ns_123").is_ok());
        assert!(validate_namespace_id("abc-DEF_123").is_ok());
    }

    #[test]
    fn test_invalid_namespace_id() {
        assert!(validate_namespace_id("ns with spaces").is_err());
        assert!(validate_namespace_id("ns@special").is_err());
        let long_ns = "a".repeat(MAX_NAMESPACE_ID_LEN + 1);
        assert!(validate_namespace_id(&long_ns).is_err());
    }

    #[test]
    fn test_valid_group() {
        assert!(validate_group("DEFAULT_GROUP").is_ok());
        assert!(validate_group("my-group").is_ok());
    }

    #[test]
    fn test_invalid_group() {
        assert!(validate_group("bad@@group").is_err());
        let long_group = "g".repeat(MAX_GROUP_LEN + 1);
        assert!(validate_group(&long_group).is_err());
    }

    #[test]
    fn test_valid_data_id() {
        assert!(validate_data_id("config.yaml").is_ok());
        assert!(validate_data_id("app-prod.properties").is_ok());
    }

    #[test]
    fn test_data_id_too_long() {
        let long_id = "d".repeat(MAX_DATA_ID_LEN + 1);
        assert!(validate_data_id(&long_id).is_err());
    }

    #[test]
    fn test_valid_service_name() {
        assert!(validate_service_name("my-service").is_ok());
        assert!(validate_service_name("com.example.Service").is_ok());
    }

    #[test]
    fn test_service_name_too_long() {
        let long_name = "s".repeat(MAX_SERVICE_NAME_LEN + 1);
        assert!(validate_service_name(&long_name).is_err());
    }

    #[test]
    fn test_validate_params_all_valid() {
        let params = ParamInfo {
            namespace_id: Some("public".to_string()),
            group: Some("DEFAULT_GROUP".to_string()),
            data_id: Some("app.yaml".to_string()),
            service_name: None,
        };
        assert!(validate_params(&params).is_ok());
    }

    #[test]
    fn test_validate_params_empty_ok() {
        let params = ParamInfo::default();
        assert!(validate_params(&params).is_ok());
    }

    #[test]
    fn test_validate_params_invalid_group() {
        let params = ParamInfo {
            group: Some("bad@@name".to_string()),
            ..Default::default()
        };
        assert!(validate_params(&params).is_err());
    }
}
