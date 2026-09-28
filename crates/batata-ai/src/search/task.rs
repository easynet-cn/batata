//! Durable search-index task identity and payload.
//!
//! Mirrors upstream `JdbcAiResourceIndexTaskRepository`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::repository::search;

/// Current payload schema version, matching upstream
/// `AiResourceIndexTaskPayload.CURRENT_SCHEMA_VERSION`.
pub const PAYLOAD_SCHEMA_VERSION: i32 = 1;

/// Build the durable task key for a resource.
///
/// Upstream hashes `TASK_TYPE \n namespaceId \n resourceType \n resourceName`
/// with SHA-256. The key is stable, so re-scheduling the same resource updates
/// the existing row instead of creating a second one.
pub fn task_key(namespace_id: &str, resource_type: &str, resource_name: &str) -> String {
    let identity = format!(
        "{}\n{}\n{}\n{}",
        search::TASK_TYPE,
        namespace_id,
        resource_type,
        resource_name
    );
    let mut hasher = Sha256::new();
    hasher.update(identity.as_bytes());
    const_hex::encode(hasher.finalize())
}

/// Canonical resource identity owned by the search-index task type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexTaskSubject {
    /// Resource type.
    #[serde(rename = "resourceType")]
    pub resource_type: String,
    /// Resource name.
    #[serde(rename = "resourceName")]
    pub resource_name: String,
}

/// Options carried by the task payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexTaskOptions {
    /// Whether LLM enhancement was requested when the task was scheduled.
    #[serde(rename = "enhancementRequested")]
    pub enhancement_requested: bool,
}

/// Versioned input payload for the search-index task type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexTaskPayload {
    /// Payload schema version.
    #[serde(rename = "schemaVersion")]
    pub schema_version: i32,
    /// Resource the task applies to.
    pub subject: IndexTaskSubject,
    /// Scheduling options.
    pub options: IndexTaskOptions,
}

impl IndexTaskPayload {
    /// Build a payload for one resource.
    pub fn new(resource_type: &str, resource_name: &str, enhancement_requested: bool) -> Self {
        Self {
            schema_version: PAYLOAD_SCHEMA_VERSION,
            subject: IndexTaskSubject {
                resource_type: resource_type.to_string(),
                resource_name: resource_name.to_string(),
            },
            options: IndexTaskOptions {
                enhancement_requested,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_stable_and_distinct_per_resource() {
        let a = task_key("public", "mcp", "server-a");
        let b = task_key("public", "mcp", "server-a");
        let c = task_key("public", "mcp", "server-b");
        assert_eq!(a, b, "same resource must yield the same key");
        assert_ne!(a, c, "different resources must not collide");
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn namespace_participates_in_key() {
        assert_ne!(
            task_key("public", "mcp", "s"),
            task_key("other", "mcp", "s"),
            "namespace must participate in the key"
        );
    }

    #[test]
    fn payload_serializes_with_camel_case() {
        let json = serde_json::to_value(IndexTaskPayload::new("mcp", "s", false)).unwrap();
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["subject"]["resourceType"], "mcp");
        assert_eq!(json["subject"]["resourceName"], "s");
        assert_eq!(json["options"]["enhancementRequested"], false);
    }
}
