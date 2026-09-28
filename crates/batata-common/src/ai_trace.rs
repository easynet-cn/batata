//! AI resource trace events.
//!
//! Mirrors upstream Nacos `AiResourceTraceService` (`@since 3.2.1`) and its
//! default subscriber `AiResourceTraceLogSubscriber`: trace records are **not**
//! persisted — they are emitted as a single JSON line per event, meant for
//! ELK/Loki ingestion.
//!
//! Upstream publishes an `AiResourceTraceEvent` through `NotifyCenter`; the
//! default subscriber serialises it and logs it at INFO under the logger
//! `com.alibaba.nacos.ai.resource.trace`. Batata has no event bus for this, so
//! the helpers here write the JSON line directly using the same logger name and
//! field layout, keeping the output wire-compatible.
//!
//! This lives in `batata-common` rather than `batata-ai` because the operator
//! and client IP are only known at the HTTP layer, and the console handlers
//! depend on this crate rather than on `batata-ai`.

use serde::Serialize;
use tracing::info;

/// Logger name used upstream, kept so log shippers can match on it.
pub const TRACE_LOGGER: &str = "com.alibaba.nacos.ai.resource.trace";

/// Resource type value for MCP servers.
pub const RESOURCE_TYPE_MCP: &str = "mcp";
/// Resource type value for A2A agents in the `ai_resource` table.
pub const RESOURCE_TYPE_AGENT: &str = "agent";
/// Resource type value for agent specs.
pub const RESOURCE_TYPE_AGENTSPEC: &str = "agentspec";
/// Resource type value for skills.
pub const RESOURCE_TYPE_SKILL: &str = "skill";
/// Resource type value for prompts.
pub const RESOURCE_TYPE_PROMPT: &str = "prompt";
/// Resource type upstream emits on **A2A trace records**.
///
/// Note this is deliberately *not* [`RESOURCE_TYPE_AGENT`]: upstream
/// `LegacyA2aOperationService` passes the literal `"a2a"` to
/// `AiResourceTraceService` even though the stored resource type is `agent`.
/// Kept as-is so trace output matches upstream.
pub const RESOURCE_TYPE_A2A: &str = "a2a";

/// Placeholder emitted when a field is blank (upstream `defaultIfBlank`).
const BLANK: &str = "-";

// =============================================================================
// Operation constants
// =============================================================================

/// Create a new draft version.
pub const OP_CREATE_DRAFT: &str = "CREATE_DRAFT";
/// Update an existing draft version.
pub const OP_UPDATE_DRAFT: &str = "UPDATE_DRAFT";
/// Delete a draft version.
pub const OP_DELETE_DRAFT: &str = "DELETE_DRAFT";
/// Upload a skill/resource.
pub const OP_UPLOAD: &str = "UPLOAD";
/// Submit version for review.
pub const OP_SUBMIT_REVIEW: &str = "SUBMIT_REVIEW";
/// Review approved.
pub const OP_REVIEW_APPROVED: &str = "REVIEW_APPROVED";
/// Review rejected.
pub const OP_REVIEW_REJECTED: &str = "REVIEW_REJECTED";
/// Force skip review (admin operation).
pub const OP_REVIEW_FORCE_SKIP: &str = "REVIEW_FORCE_SKIP";
/// Re-edit a reviewed version (transition back to draft).
pub const OP_REDRAFT: &str = "REDRAFT";
/// Publish a version to online.
pub const OP_PUBLISH: &str = "PUBLISH";
/// Force publish (bypass review).
pub const OP_FORCE_PUBLISH: &str = "FORCE_PUBLISH";
/// Take a version offline.
pub const OP_OFFLINE_VERSION: &str = "OFFLINE_VERSION";
/// Bring a version back online.
pub const OP_ONLINE_VERSION: &str = "ONLINE_VERSION";
/// Delete a version.
pub const OP_DELETE_VERSION: &str = "DELETE_VERSION";
/// Delete the entire resource (including all versions).
pub const OP_DELETE_RESOURCE: &str = "DELETE_RESOURCE";
/// Set/update label for a version.
pub const OP_SET_LABEL: &str = "SET_LABEL";
/// Remove label from a version.
pub const OP_REMOVE_LABEL: &str = "REMOVE_LABEL";
/// Update labels.
pub const OP_UPDATE_LABELS: &str = "UPDATE_LABELS";
/// Update resource scope.
pub const OP_UPDATE_SCOPE: &str = "UPDATE_SCOPE";
/// Update resource description.
pub const OP_UPDATE_DESCRIPTION: &str = "UPDATE_DESCRIPTION";
/// Update resource bizTags.
pub const OP_UPDATE_BIZ_TAGS: &str = "UPDATE_BIZ_TAGS";
/// Update a resource's complete writable metadata.
pub const OP_UPDATE_RESOURCE: &str = "UPDATE_RESOURCE";
/// Enable resource.
pub const OP_ENABLE: &str = "ENABLE";
/// Disable resource.
pub const OP_DISABLE: &str = "DISABLE";
/// Search external AI resource import candidates.
pub const OP_IMPORT_SEARCH: &str = "IMPORT_SEARCH";
/// Validate selected external AI resource import candidates.
pub const OP_IMPORT_VALIDATE: &str = "IMPORT_VALIDATE";
/// Execute external AI resource import.
pub const OP_IMPORT_EXECUTE: &str = "IMPORT_EXECUTE";

// =============================================================================
// Status constants
// =============================================================================

/// Operation succeeded.
pub const STATUS_SUCCESS: &str = "SUCCESS";
/// Operation failed.
pub const STATUS_FAILURE: &str = "FAILURE";
/// Operation skipped by request policy.
pub const STATUS_SKIPPED: &str = "SKIPPED";

/// One AI resource trace record.
///
/// Field names follow the upstream JSON line, which uses snake_case.
#[derive(Clone, Debug, Serialize)]
pub struct AiResourceTraceEvent {
    /// ISO-8601 instant derived from the event time in epoch millis.
    pub timestamp: String,
    /// Operator identity (user id or username).
    pub operator: String,
    /// Resource type (e.g. `skill`, `agentspec`, `mcp`, `prompt`).
    pub resource_type: String,
    /// Resource identifier (name).
    pub resource_id: String,
    /// Version being operated on; omitted from the JSON when blank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Operation type (one of the `OP_*` constants).
    pub operation: String,
    /// One of the `STATUS_*` constants.
    pub status: String,
    /// Client IP address.
    pub ip: String,
    /// Extra information or error message; omitted from the JSON when blank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<String>,
}

/// Build a trace event, applying the upstream blank-to-`-` defaults.
///
/// `version` and `ext` are dropped entirely when blank, matching
/// `AiResourceTraceLogSubscriber.buildLogEntry`.
pub fn build_trace_event(
    event_time_millis: i64,
    operator: &str,
    resource_type: &str,
    resource_id: &str,
    version: Option<&str>,
    operation: &str,
    status: &str,
    client_ip: &str,
    ext: Option<&str>,
) -> AiResourceTraceEvent {
    fn or_blank(value: &str) -> String {
        if value.is_empty() {
            BLANK.to_string()
        } else {
            value.to_string()
        }
    }
    fn non_blank(value: Option<&str>) -> Option<String> {
        value
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    }

    let timestamp = chrono::DateTime::from_timestamp_millis(event_time_millis)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_default();

    AiResourceTraceEvent {
        timestamp,
        operator: or_blank(operator),
        resource_type: or_blank(resource_type),
        resource_id: or_blank(resource_id),
        version: non_blank(version),
        operation: or_blank(operation),
        status: or_blank(status),
        ip: or_blank(client_ip),
        ext: non_blank(ext),
    }
}

/// Emit an AI resource trace record as a single JSON line.
///
/// Logged at INFO under [`TRACE_LOGGER`] so ELK/Loki pipelines written for
/// upstream Nacos keep working unchanged.
pub fn log(
    resource_type: &str,
    resource_id: &str,
    version: Option<&str>,
    operation: &str,
    status: &str,
    operator: &str,
    client_ip: &str,
    ext: Option<&str>,
) {
    let event = build_trace_event(
        chrono::Utc::now().timestamp_millis(),
        operator,
        resource_type,
        resource_id,
        version,
        operation,
        status,
        client_ip,
        ext,
    );
    // `serde_json::to_string` on this struct cannot fail: every field is a
    // String or an Option<String>.
    let line = serde_json::to_string(&event).unwrap_or_default();
    info!(target: TRACE_LOGGER, "{}", line);
}

/// Log a successful AI resource operation.
pub fn log_success(
    resource_type: &str,
    resource_id: &str,
    version: Option<&str>,
    operation: &str,
    operator: &str,
    client_ip: &str,
) {
    log(
        resource_type,
        resource_id,
        version,
        operation,
        STATUS_SUCCESS,
        operator,
        client_ip,
        None,
    );
}

/// Log a failed AI resource operation, carrying the error message in `ext`.
pub fn log_failure(
    resource_type: &str,
    resource_id: &str,
    version: Option<&str>,
    operation: &str,
    operator: &str,
    client_ip: &str,
    error_msg: &str,
) {
    log(
        resource_type,
        resource_id,
        version,
        operation,
        STATUS_FAILURE,
        operator,
        client_ip,
        Some(error_msg),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_matches_upstream_json_layout() {
        let event = build_trace_event(
            1_800_000_000_000,
            "admin",
            "skill",
            "my-skill",
            Some("v1.0"),
            OP_PUBLISH,
            STATUS_SUCCESS,
            "192.168.1.1",
            None,
        );
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["operator"], "admin");
        assert_eq!(json["resource_type"], "skill");
        assert_eq!(json["resource_id"], "my-skill");
        assert_eq!(json["version"], "v1.0");
        assert_eq!(json["operation"], "PUBLISH");
        assert_eq!(json["status"], "SUCCESS");
        assert_eq!(json["ip"], "192.168.1.1");
        assert!(
            json.get("ext").is_none(),
            "a blank ext must be omitted, not emitted as null"
        );
        assert!(json["timestamp"].as_str().unwrap().ends_with('Z'));
    }

    #[test]
    fn blank_fields_fall_back_to_dash() {
        let event = build_trace_event(0, "", "", "", None, "", "", "", None);
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["operator"], "-");
        assert_eq!(json["resource_type"], "-");
        assert_eq!(json["resource_id"], "-");
        assert_eq!(json["operation"], "-");
        assert_eq!(json["status"], "-");
        assert_eq!(json["ip"], "-");
        assert!(
            json.get("version").is_none(),
            "a blank version must be omitted"
        );
    }

    #[test]
    fn whitespace_only_treated_as_blank() {
        let event = build_trace_event(
            0,
            "admin",
            "mcp",
            "s",
            Some("   "),
            OP_PUBLISH,
            STATUS_SUCCESS,
            "1.1.1.1",
            Some("  "),
        );
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert!(json.get("version").is_none());
        assert!(json.get("ext").is_none());
    }

    #[test]
    fn failure_carries_error_in_ext() {
        let event = build_trace_event(
            0,
            "admin",
            "mcp",
            "s",
            Some("1.0.0"),
            OP_PUBLISH,
            STATUS_FAILURE,
            "1.1.1.1",
            Some("version not found"),
        );
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["status"], "FAILURE");
        assert_eq!(json["ext"], "version not found");
    }
}
