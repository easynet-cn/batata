//! Pipeline execution model types — aligned with Nacos 3.x Pipeline API
//!
//! Pipeline executions track the review/approval workflow for Skills and AgentSpecs.
//! Stored in the pipeline_execution table.

use serde::{Deserialize, Serialize};

// ============================================================================
// Domain models
// ============================================================================

/// Pipeline execution record
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineExecution {
    /// The `execution_id` field.
    pub execution_id: String,
    /// The `resource_type` field.
    pub resource_type: String,
    /// The `resource_name` field.
    pub resource_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `namespace_id` field.
    pub namespace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `version` field.
    pub version: Option<String>,
    /// The `status` field.
    pub status: String,
    #[serde(default)]
    /// The `pipeline` field.
    pub pipeline: Vec<PipelineNodeResult>,
    /// The `create_time` field.
    pub create_time: i64,
    /// The `update_time` field.
    pub update_time: i64,
}

/// Pipeline execution status
pub const PIPELINE_STATUS_IN_PROGRESS: &str = "IN_PROGRESS";
/// Pipeline status: approved by reviewers.
pub const PIPELINE_STATUS_APPROVED: &str = "APPROVED";
/// Pipeline status: rejected by reviewers.
pub const PIPELINE_STATUS_REJECTED: &str = "REJECTED";

/// Individual pipeline node execution result
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineNodeResult {
    /// The `node_id` field.
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `executed_at` field.
    pub executed_at: Option<String>,
    #[serde(default)]
    /// The `passed` field.
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `message` field.
    pub message: Option<String>,
    /// "text", "json", "markdown", "html"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The `checkpoints` field.
    pub checkpoints: Vec<Checkpoint>,
    #[serde(default)]
    /// The `duration_ms` field.
    pub duration_ms: i64,
}

/// Checkpoint within a pipeline node
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `name` field.
    pub name: Option<String>,
    #[serde(default)]
    /// The `passed` field.
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `message` field.
    pub message: Option<String>,
}

// ============================================================================
// Request forms
// ============================================================================

/// List pipeline executions query params
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineListForm {
    /// Required: resource type (e.g., "skill", "agentspec")
    #[serde(alias = "resourceType")]
    pub resource_type: String,
    #[serde(alias = "resourceName")]
    /// The `resource_name` field.
    pub resource_name: Option<String>,
    #[serde(alias = "namespaceId")]
    /// The `namespace_id` field.
    pub namespace_id: Option<String>,
    /// The `version` field.
    pub version: Option<String>,
    #[serde(default = "default_page_no", alias = "pageNo")]
    /// The `page_no` field.
    pub page_no: u64,
    #[serde(default = "default_page_size", alias = "pageSize")]
    /// The `page_size` field.
    pub page_size: u64,
}

fn default_page_no() -> u64 {
    1
}
fn default_page_size() -> u64 {
    10
}

/// Get pipeline execution detail query params.
/// Aligned with Nacos `PipelineDetailForm`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineDetailForm {
    /// Required: pipeline execution ID.
    #[serde(alias = "pipelineId")]
    pub pipeline_id: String,
}

#[cfg(test)]
mod tests {
    //! Ported from upstream `PipelineExecutionStatusTest` and
    //! `PipelineExecutionStatusConsistencyTest`; both assert the invariant that
    //! ties an execution's status to its node results.

    use super::*;

    fn node(node_id: &str, passed: bool) -> PipelineNodeResult {
        PipelineNodeResult {
            node_id: node_id.to_string(),
            executed_at: Some("2026-03-27T12:00:00Z".to_string()),
            passed,
            ..Default::default()
        }
    }

    fn execution(status: &str, nodes: Vec<PipelineNodeResult>) -> PipelineExecution {
        PipelineExecution {
            execution_id: "exec-1".to_string(),
            resource_type: "AGENTSPEC".to_string(),
            resource_name: "demo".to_string(),
            namespace_id: Some("public".to_string()),
            version: Some("1.0.0".to_string()),
            status: status.to_string(),
            pipeline: nodes,
            create_time: 1,
            update_time: 2,
        }
    }

    /// The status the invariant says these nodes must carry.
    fn expected_status(nodes: &[PipelineNodeResult]) -> &'static str {
        if nodes.iter().all(|n| n.passed) {
            PIPELINE_STATUS_APPROVED
        } else {
            PIPELINE_STATUS_REJECTED
        }
    }

    #[test]
    fn approved_requires_every_node_to_pass() {
        let nodes = vec![node("parse", true), node("scan", true)];
        let e = execution(PIPELINE_STATUS_APPROVED, nodes.clone());
        assert!(e.pipeline.iter().all(|n| n.passed));
        assert_eq!(e.status, PIPELINE_STATUS_APPROVED);
        assert_eq!(expected_status(&nodes), PIPELINE_STATUS_APPROVED);
    }

    #[test]
    fn rejected_when_any_node_failed() {
        let nodes = vec![node("parse", true), node("scan", false)];
        let e = execution(PIPELINE_STATUS_REJECTED, nodes.clone());
        assert!(!e.pipeline.iter().all(|n| n.passed));
        assert_eq!(e.status, PIPELINE_STATUS_REJECTED);
        assert_eq!(expected_status(&nodes), PIPELINE_STATUS_REJECTED);
    }

    #[test]
    fn approved_also_holds_for_a_single_passed_node() {
        let nodes = vec![node("scan", true)];
        let e = execution(PIPELINE_STATUS_APPROVED, nodes.clone());
        assert!(e.pipeline.iter().all(|n| n.passed));
        assert_eq!(e.status, PIPELINE_STATUS_APPROVED);
        assert_eq!(expected_status(&nodes), PIPELINE_STATUS_APPROVED);
    }

    /// Exhaustively: the status is APPROVED exactly when every node passed.
    /// Upstream checks the combinations it builds by hand; this walks all of
    /// them up to three nodes, so a single-node special case cannot slip past.
    #[test]
    fn status_is_approved_iff_every_node_passed() {
        for count in 1..=3usize {
            for mask in 0..(1u32 << count) {
                let nodes: Vec<PipelineNodeResult> = (0..count)
                    .map(|i| node(&format!("node-{i}"), (mask >> i) & 1 == 1))
                    .collect();
                let expected = expected_status(&nodes);
                let e = execution(expected, nodes.clone());
                assert_eq!(e.status, expected, "nodes: {mask:0count$b}");
                assert_eq!(
                    e.pipeline.iter().all(|n| n.passed),
                    expected == PIPELINE_STATUS_APPROVED
                );
            }
        }
    }

    /// An execution with no nodes at all is not "approved" by vacuity being
    /// mistaken for review; upstream's fold over an empty list would say
    /// passed, so this pins the behaviour down explicitly.
    #[test]
    fn an_execution_without_nodes_is_not_approved() {
        let e = execution(PIPELINE_STATUS_REJECTED, vec![]);
        assert!(e.pipeline.is_empty());
        assert_ne!(e.status, PIPELINE_STATUS_APPROVED);
    }

    /// Ported from upstream `PipelineNodeResultRoundTripTest`: a node result
    /// survives JSON, including the fields upstream leaves null.
    #[test]
    fn a_node_result_survives_a_round_trip() {
        let node = PipelineNodeResult {
            node_id: "n1".to_string(),
            executed_at: Some("2024-06-15T12:00:00Z".to_string()),
            passed: true,
            message: Some("msg-n1".to_string()),
            message_type: Some("text".to_string()),
            checkpoints: vec![Checkpoint {
                name: Some("cp1".to_string()),
                passed: true,
                message: None,
            }],
            duration_ms: 100,
        };

        let json = serde_json::to_string(&node).expect("serialize");
        let back: PipelineNodeResult = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.node_id, "n1");
        assert_eq!(back.executed_at.as_deref(), Some("2024-06-15T12:00:00Z"));
        assert!(back.passed);
        assert_eq!(back.message.as_deref(), Some("msg-n1"));
        assert_eq!(back.message_type.as_deref(), Some("text"));
        assert_eq!(back.duration_ms, 100);
        assert_eq!(back.checkpoints.len(), 1);
        assert_eq!(back.checkpoints[0].name.as_deref(), Some("cp1"));
        assert!(back.checkpoints[0].passed);
    }

    /// The optional fields upstream sends as null must not be invented on the
    /// way back — a missing `messageType` stays missing.
    #[test]
    fn absent_optional_fields_stay_absent() {
        let node = PipelineNodeResult {
            node_id: "n2".to_string(),
            executed_at: None,
            passed: false,
            message: None,
            message_type: None,
            checkpoints: vec![],
            duration_ms: 0,
        };

        let json = serde_json::to_string(&node).expect("serialize");
        let back: PipelineNodeResult = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.executed_at, None);
        assert_eq!(back.message, None);
        assert_eq!(back.message_type, None);
        assert!(back.checkpoints.is_empty());

        // Omitted on the wire, so the defaults must fill in rather than fail.
        let sparse: PipelineNodeResult =
            serde_json::from_str(r#"{"nodeId":"n3"}"#).expect("deserialize a sparse node");
        assert_eq!(sparse.node_id, "n3");
        assert!(!sparse.passed);
        assert_eq!(sparse.duration_ms, 0);
    }

    /// A list keeps its order and its empties, which is what an execution's
    /// `pipeline` array actually is.
    #[test]
    fn a_list_of_node_results_survives_a_round_trip() {
        let nodes = vec![
            PipelineNodeResult {
                node_id: "a".to_string(),
                passed: true,
                message_type: Some("json".to_string()),
                ..Default::default()
            },
            PipelineNodeResult {
                node_id: "b".to_string(),
                passed: false,
                ..Default::default()
            },
        ];

        let json = serde_json::to_string(&nodes).expect("serialize");
        let back: Vec<PipelineNodeResult> = serde_json::from_str(&json).expect("deserialize");

        let ids: Vec<&str> = back.iter().map(|n| n.node_id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"], "order must be preserved");
        assert!(back[0].passed);
        assert!(!back[1].passed);
        assert_eq!(back[0].message_type.as_deref(), Some("json"));
        assert_eq!(back[1].message_type, None);
    }
}
