//! Typed AI resource DTOs for the ADMIN Skill / Prompt / AgentSpec / Pipeline
//! read endpoints.
//!
//! Nacos 3.3.0 GA exposes these resources through `AiMaintainerService`
//! delegates with typed Java models (`SkillMeta`, `PromptMetaInfo`,
//! `AgentSpecMeta`, `PipelineExecution`, ...). batata's admin layer returns the
//! same wire keys with a few extra or missing governance fields. Each DTO below
//! types the fields shared by both sides and keeps every other wire field in
//! `extra`, following the same convention as [`crate::model::ai`]. All scalar
//! fields are optional so a `null` from either implementation still decodes.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Deserialize a collection field that either implementation may emit as
/// `null`, treating it the same way as an absent field.
fn null_to_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Additional wire fields that are not modelled by this crate.
pub type AiExtra = HashMap<String, serde_json::Value>;

/// Governance fields shared by the Skill / AgentSpec metadata and summary DTOs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiResourceGovernance {
    /// Namespace ID that owns the resource.
    pub namespace_id: Option<String>,
    /// Resource name.
    pub name: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Last modification time in epoch milliseconds.
    pub update_time: Option<i64>,
    /// Whether the resource is globally enabled.
    pub enable: Option<bool>,
    /// Resource owner (GA only).
    pub owner: Option<String>,
    /// Business tags as a JSON string.
    pub biz_tags: Option<String>,
    /// Source marker for IP attribution.
    pub from: Option<String>,
    /// Visibility scope, e.g. `PUBLIC` or `PRIVATE`.
    pub scope: Option<String>,
    /// Label to version mapping.
    #[serde(default, deserialize_with = "null_to_default")]
    pub labels: HashMap<String, String>,
    /// Parsed SKILL.md frontmatter (GA only).
    #[serde(default, deserialize_with = "null_to_default")]
    pub front_matter: HashMap<String, String>,
    /// Whether the frontmatter projection was truncated (GA only).
    pub front_matter_truncated: Option<bool>,
    /// Version currently being edited.
    pub editing_version: Option<String>,
    /// Version currently under pipeline review.
    pub reviewing_version: Option<String>,
    /// Number of online versions.
    pub online_cnt: Option<i64>,
    /// Total download count across all versions.
    pub download_count: Option<i64>,
    /// Whether the requester may modify the resource (GA only).
    pub writable: Option<bool>,
}

/// Admin metadata for one skill, including its version summaries.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillMeta {
    /// Governance fields shared with the list view.
    #[serde(flatten)]
    pub governance: AiResourceGovernance,
    /// All version summaries for this skill.
    #[serde(default, deserialize_with = "null_to_default")]
    pub versions: Vec<SkillVersionSummary>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One skill as returned by the admin list endpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillSummary {
    /// Governance fields shared with the detail view.
    #[serde(flatten)]
    pub governance: AiResourceGovernance,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One skill version summary embedded in [`SkillMeta`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillVersionSummary {
    /// Version string.
    pub version: Option<String>,
    /// Lifecycle status.
    pub status: Option<String>,
    /// Author of the version.
    pub author: Option<String>,
    /// GA commit message; batata uses `description` instead.
    pub commit_msg: Option<String>,
    /// batata's version description.
    pub description: Option<String>,
    /// Creation time in epoch milliseconds.
    pub create_time: Option<i64>,
    /// Last modification time in epoch milliseconds.
    pub update_time: Option<i64>,
    /// Pipeline information for the publish review.
    pub publish_pipeline_info: Option<String>,
    /// Download count of this version.
    pub download_count: Option<i64>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// Full content of one published skill version.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Skill {
    /// Namespace ID that owns the skill.
    pub namespace_id: Option<String>,
    /// Skill name.
    pub name: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Full SKILL.md content.
    pub skill_md: Option<String>,
    /// Resource map keyed by resource identifier.
    #[serde(default, deserialize_with = "null_to_default")]
    pub resource: HashMap<String, SkillResource>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One file or tool bundled with a skill version.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillResource {
    /// Resource name including its file extension.
    pub name: Option<String>,
    /// Resource type, e.g. `template`, `data` or `script`.
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    /// Resource content.
    pub content: Option<String>,
    /// Optional resource metadata.
    #[serde(default, deserialize_with = "null_to_default")]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// Result of checking one skill ZIP before applying the upload.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillUploadPrecheckResult {
    /// Namespace ID the upload targets.
    pub namespace_id: Option<String>,
    /// Path of the skill entry inside the ZIP.
    pub entry_path: Option<String>,
    /// Skill name parsed from the ZIP.
    pub skill_name: Option<String>,
    /// Human-readable precheck reason.
    pub reason: Option<String>,
    /// Resource owner.
    pub owner: Option<String>,
    /// Highest already published version.
    pub max_published_version: Option<String>,
    /// Version parsed from the SKILL.md frontmatter.
    pub parsed_version: Option<String>,
    /// Version the upload would create.
    pub target_version: Option<String>,
    /// Whether the skill already exists.
    pub exists: Option<bool>,
    /// Version currently being edited.
    pub editing_version: Option<String>,
    /// Version currently under pipeline review.
    pub reviewing_version: Option<String>,
    /// Machine-readable precheck result code.
    pub precheck_code: Option<String>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// Aggregate result of a multi-skill ZIP upload.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BatchUploadResult {
    /// Names of the skills uploaded successfully.
    #[serde(default, deserialize_with = "null_to_default")]
    pub succeeded: Vec<String>,
    /// Skills that failed to upload (legacy projection).
    #[serde(default, deserialize_with = "null_to_default")]
    pub failed: Vec<BatchUploadFailedItem>,
    /// Per-skill results (GA projection).
    #[serde(default, deserialize_with = "null_to_default")]
    pub results: Vec<BatchUploadItemResult>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One failed skill in the legacy batch-upload projection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BatchUploadFailedItem {
    /// Skill name.
    pub name: Option<String>,
    /// Resource owner.
    pub owner: Option<String>,
    /// Failure reason.
    pub reason: Option<String>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// Per-skill result in the GA batch-upload projection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BatchUploadItemResult {
    /// Skill name.
    pub name: Option<String>,
    /// Whether this item succeeded.
    pub success: Option<bool>,
    /// Machine-readable error code.
    pub error_code: Option<String>,
    /// Human-readable error message.
    pub error_message: Option<String>,
    /// Resource owner.
    pub owner: Option<String>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// Prompt metadata as returned by the admin list endpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PromptMetaSummary {
    /// Metadata schema version.
    pub schema_version: Option<i32>,
    /// Prompt key.
    pub prompt_key: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Business tags.
    #[serde(default, deserialize_with = "null_to_default")]
    pub biz_tags: Vec<String>,
    /// Business tags serialized as one string.
    pub biz_tags_str: Option<String>,
    /// Latest published version.
    pub latest_version: Option<String>,
    /// Last modification time in epoch milliseconds.
    pub gmt_modified: Option<i64>,
    /// Version currently being edited.
    pub editing_version: Option<String>,
    /// Version currently under pipeline review.
    pub reviewing_version: Option<String>,
    /// Number of online versions.
    pub online_cnt: Option<i64>,
    /// Label to version mapping.
    #[serde(default, deserialize_with = "null_to_default")]
    pub labels: HashMap<String, String>,
    /// Total download count.
    pub download_count: Option<i64>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// Prompt metadata including all version summaries (governance / metadata
/// endpoints).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PromptMetaInfo {
    /// Metadata schema version.
    pub schema_version: Option<i32>,
    /// Prompt key.
    pub prompt_key: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Business tags.
    #[serde(default, deserialize_with = "null_to_default")]
    pub biz_tags: Vec<String>,
    /// Business tags serialized as one string.
    pub biz_tags_str: Option<String>,
    /// Latest published version.
    pub latest_version: Option<String>,
    /// Last modification time in epoch milliseconds.
    pub gmt_modified: Option<i64>,
    /// Version currently being edited.
    pub editing_version: Option<String>,
    /// Version currently under pipeline review.
    pub reviewing_version: Option<String>,
    /// Number of online versions.
    pub online_cnt: Option<i64>,
    /// Label to version mapping.
    #[serde(default, deserialize_with = "null_to_default")]
    pub labels: HashMap<String, String>,
    /// Total download count.
    pub download_count: Option<i64>,
    /// Version strings belonging to this prompt.
    #[serde(default, deserialize_with = "null_to_default")]
    pub versions: Vec<String>,
    /// Detailed version summaries.
    #[serde(default, deserialize_with = "null_to_default")]
    pub version_details: Vec<PromptVersionSummary>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One prompt version summary from the version-history endpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PromptVersionSummary {
    /// Prompt key.
    pub prompt_key: Option<String>,
    /// Version string.
    pub version: Option<String>,
    /// Lifecycle status.
    pub status: Option<String>,
    /// Commit message.
    pub commit_msg: Option<String>,
    /// User who created the version.
    pub src_user: Option<String>,
    /// Last modification time in epoch milliseconds.
    pub gmt_modified: Option<i64>,
    /// Pipeline information for the publish review.
    pub publish_pipeline_info: Option<String>,
    /// Download count of this version.
    pub download_count: Option<i64>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// Full content of one prompt version.
///
/// GA returns `PromptVersionInfo` from the version/detail endpoints while
/// batata's `prompt/detail` returns its client-facing `Prompt`. The union of
/// both shapes is typed here so either response decodes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PromptVersionInfo {
    /// Prompt key.
    pub prompt_key: Option<String>,
    /// Version string.
    pub version: Option<String>,
    /// Prompt template content.
    pub template: Option<String>,
    /// Content MD5.
    pub md5: Option<String>,
    /// Commit message (GA version info only).
    pub commit_msg: Option<String>,
    /// User who created the version (GA version info only).
    pub src_user: Option<String>,
    /// Last modification time in epoch milliseconds.
    pub gmt_modified: Option<i64>,
    /// Lifecycle status (GA version info only).
    pub status: Option<String>,
    /// Pipeline information for the publish review (GA version info only).
    pub publish_pipeline_info: Option<String>,
    /// Download count (GA version info only).
    pub download_count: Option<i64>,
    /// Template variables.
    #[serde(default, deserialize_with = "null_to_default")]
    pub variables: Vec<PromptVariable>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One template variable declared by a prompt version.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PromptVariable {
    /// Variable name.
    pub name: Option<String>,
    /// Default value used when the caller does not supply one.
    pub default_value: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// Full content of one agentspec version.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSpec {
    /// Namespace ID that owns the spec.
    pub namespace_id: Option<String>,
    /// Spec name.
    pub name: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Business tags as a JSON string.
    pub biz_tags: Option<String>,
    /// Full agentspec content.
    pub content: Option<String>,
    /// Resource map keyed by resource identifier.
    #[serde(default, deserialize_with = "null_to_default")]
    pub resource: HashMap<String, AgentSpecResource>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One file bundled with an agentspec version.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSpecResource {
    /// Resource name including its file extension.
    pub name: Option<String>,
    /// Resource type.
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    /// Resource content.
    pub content: Option<String>,
    /// Optional resource metadata.
    #[serde(default, deserialize_with = "null_to_default")]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// Admin metadata for one agentspec, including its version summaries.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSpecMeta {
    /// Governance fields shared with the list view.
    #[serde(flatten)]
    pub governance: AiResourceGovernance,
    /// All version summaries for this spec.
    #[serde(default, deserialize_with = "null_to_default")]
    pub versions: Vec<AgentSpecVersionSummary>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One agentspec as returned by the admin list endpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSpecSummary {
    /// Governance fields shared with the detail view.
    #[serde(flatten)]
    pub governance: AiResourceGovernance,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One agentspec as returned by the plain list endpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSpecBasicInfo {
    /// Namespace ID that owns the spec.
    pub namespace_id: Option<String>,
    /// Spec name.
    pub name: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Last modification time in epoch milliseconds.
    pub update_time: Option<i64>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One agentspec version summary embedded in [`AgentSpecMeta`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSpecVersionSummary {
    /// Version string.
    pub version: Option<String>,
    /// Lifecycle status.
    pub status: Option<String>,
    /// Author of the version.
    pub author: Option<String>,
    /// GA commit message.
    pub commit_msg: Option<String>,
    /// batata's version description.
    pub description: Option<String>,
    /// Creation time in epoch milliseconds.
    pub create_time: Option<i64>,
    /// Last modification time in epoch milliseconds.
    pub update_time: Option<i64>,
    /// Pipeline information for the publish review.
    pub publish_pipeline_info: Option<String>,
    /// Download count of this version.
    pub download_count: Option<i64>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One pipeline execution.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PipelineExecution {
    /// Execution ID.
    pub execution_id: Option<String>,
    /// Resource type the pipeline belongs to.
    pub resource_type: Option<String>,
    /// Resource name.
    pub resource_name: Option<String>,
    /// Namespace ID.
    pub namespace_id: Option<String>,
    /// Version under review.
    pub version: Option<String>,
    /// Execution status.
    pub status: Option<String>,
    /// Ordered node results.
    #[serde(default, deserialize_with = "null_to_default")]
    pub pipeline: Vec<PipelineNodeResult>,
    /// Creation time in epoch milliseconds.
    pub create_time: Option<i64>,
    /// Last update time in epoch milliseconds.
    pub update_time: Option<i64>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// Result of one node within a pipeline execution.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PipelineNodeResult {
    /// Node ID.
    pub node_id: Option<String>,
    /// Timestamp at which the node ran.
    pub executed_at: Option<String>,
    /// Whether the node passed.
    pub passed: Option<bool>,
    /// Human-readable result message.
    pub message: Option<String>,
    /// Machine-readable message type.
    pub message_type: Option<String>,
    /// Checkpoints evaluated by the node.
    #[serde(default, deserialize_with = "null_to_default")]
    pub checkpoints: Vec<PipelineCheckpoint>,
    /// Node duration in milliseconds.
    pub duration_ms: Option<i64>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

/// One checkpoint evaluated inside a pipeline node.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PipelineCheckpoint {
    /// GA checkpoint title.
    pub title: Option<String>,
    /// batata checkpoint name.
    pub name: Option<String>,
    /// Whether the checkpoint passed.
    pub passed: Option<bool>,
    /// Human-readable result message.
    pub message: Option<String>,
    /// Any additional fields returned by the server.
    #[serde(flatten)]
    pub extra: AiExtra,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_meta_decodes_ga_shape_and_keeps_unknown_fields() {
        let json = serde_json::json!({
            "namespaceId": "ns-1",
            "name": "my-skill",
            "owner": "alice",
            "enable": true,
            "labels": {"latest": "1.0.0"},
            "onlineCnt": 2,
            "downloadCount": 7,
            "frontMatter": {"version": "1.0.0"},
            "frontMatterTruncated": false,
            "writable": true,
            "versions": [{"version": "1.0.0", "status": "online", "commitMsg": "first"}],
            "untypedGaField": "kept"
        });
        let meta: SkillMeta = serde_json::from_value(json).unwrap();
        assert_eq!(meta.governance.name.as_deref(), Some("my-skill"));
        assert_eq!(meta.governance.owner.as_deref(), Some("alice"));
        assert_eq!(meta.governance.enable, Some(true));
        assert_eq!(meta.governance.online_cnt, Some(2));
        assert_eq!(
            meta.governance.labels.get("latest").map(String::as_str),
            Some("1.0.0")
        );
        assert_eq!(meta.governance.writable, Some(true));
        assert_eq!(meta.versions.len(), 1);
        assert_eq!(meta.versions[0].commit_msg.as_deref(), Some("first"));
        assert_eq!(meta.extra["untypedGaField"], "kept");
    }

    #[test]
    fn skill_version_uses_type_alias_and_tolerates_null_collections() {
        let skill: Skill = serde_json::from_value(serde_json::json!({
            "namespaceId": "ns-1",
            "name": "demo",
            "skillMd": "# demo",
            "resource": null,
            "gaOnly": 42
        }))
        .unwrap();
        assert_eq!(skill.name.as_deref(), Some("demo"));
        assert_eq!(skill.skill_md.as_deref(), Some("# demo"));
        assert!(skill.resource.is_empty());
        assert_eq!(skill.extra["gaOnly"], 42);

        let resource: SkillResource = serde_json::from_value(serde_json::json!({
            "name": "demo.json",
            "type": "template",
            "content": "{}",
            "metadata": {"weight": 3}
        }))
        .unwrap();
        assert_eq!(resource.resource_type.as_deref(), Some("template"));
        assert_eq!(resource.metadata["weight"], 3);
    }

    #[test]
    fn prompt_meta_and_version_info_decode_ga_and_batata_shapes() {
        let meta: PromptMetaInfo = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "promptKey": "hello",
            "bizTags": ["chat"],
            "latestVersion": "1.0.0",
            "labels": {"stable": "1.0.0"},
            "versions": ["1.0.0"],
            "versionDetails": [{"version": "1.0.0", "status": "online", "srcUser": "alice"}],
            "untyped": true
        }))
        .unwrap();
        assert_eq!(meta.prompt_key.as_deref(), Some("hello"));
        assert_eq!(meta.schema_version, Some(1));
        assert_eq!(meta.biz_tags, vec!["chat".to_string()]);
        assert_eq!(meta.versions, vec!["1.0.0".to_string()]);
        assert_eq!(meta.version_details[0].src_user.as_deref(), Some("alice"));
        assert_eq!(meta.extra["untyped"], true);

        // GA `PromptVersionInfo` and batata's client-facing `Prompt` share the
        // same version/detail endpoints.
        let ga: PromptVersionInfo = serde_json::from_value(serde_json::json!({
            "promptKey": "hello",
            "version": "1.0.0",
            "template": "Hi {{name}}",
            "md5": "abc",
            "commitMsg": "first",
            "srcUser": "alice",
            "variables": [{"name": "name", "defaultValue": "world"}]
        }))
        .unwrap();
        assert_eq!(ga.commit_msg.as_deref(), Some("first"));
        assert_eq!(ga.variables[0].default_value.as_deref(), Some("world"));

        let batata: PromptVersionInfo = serde_json::from_value(serde_json::json!({
            "promptKey": "hello",
            "version": "1.0.0",
            "template": "Hi {{name}}",
            "md5": "abc",
            "variables": null
        }))
        .unwrap();
        assert_eq!(batata.template.as_deref(), Some("Hi {{name}}"));
        assert!(batata.variables.is_empty());
    }

    #[test]
    fn agentspec_meta_and_version_decode_ga_shape() {
        let meta: AgentSpecMeta = serde_json::from_value(serde_json::json!({
            "namespaceId": "ns-1",
            "name": "my-spec",
            "owner": "alice",
            "enable": true,
            "versions": [{"version": "1.0", "status": "online", "downloadCount": 3}],
            "untyped": "kept"
        }))
        .unwrap();
        assert_eq!(meta.governance.owner.as_deref(), Some("alice"));
        assert_eq!(meta.versions[0].download_count, Some(3));
        assert_eq!(meta.extra["untyped"], "kept");

        let spec: AgentSpec = serde_json::from_value(serde_json::json!({
            "namespaceId": "ns-1",
            "name": "my-spec",
            "content": "name: my-spec",
            "resource": {"template::a.yaml": {"name": "a.yaml", "type": "template"}}
        }))
        .unwrap();
        assert_eq!(spec.content.as_deref(), Some("name: my-spec"));
        assert_eq!(
            spec.resource["template::a.yaml"].resource_type.as_deref(),
            Some("template")
        );
    }

    #[test]
    fn pipeline_execution_decodes_node_and_checkpoint_shapes() {
        let execution: PipelineExecution = serde_json::from_value(serde_json::json!({
            "executionId": "pipe-1",
            "resourceType": "skill",
            "status": "SUCCESS",
            "createTime": 100,
            "updateTime": 200,
            "pipeline": [{
                "nodeId": "n1",
                "passed": true,
                "durationMs": 12,
                "checkpoints": [
                    {"title": "lint", "passed": true},
                    {"name": "unit", "passed": false, "message": "failed"}
                ]
            }],
            "untyped": "kept"
        }))
        .unwrap();
        assert_eq!(execution.execution_id.as_deref(), Some("pipe-1"));
        assert_eq!(execution.create_time, Some(100));
        assert_eq!(execution.status.as_deref(), Some("SUCCESS"));
        assert_eq!(execution.pipeline[0].node_id.as_deref(), Some("n1"));
        assert_eq!(
            execution.pipeline[0].checkpoints[0].title.as_deref(),
            Some("lint")
        );
        assert_eq!(
            execution.pipeline[0].checkpoints[1].name.as_deref(),
            Some("unit")
        );
        assert_eq!(execution.extra["untyped"], "kept");
    }

    #[test]
    fn upload_results_decode_both_projections() {
        let precheck: SkillUploadPrecheckResult = serde_json::from_value(serde_json::json!({
            "namespaceId": "ns-1",
            "skillName": "demo",
            "precheckCode": "READY",
            "exists": false,
            "targetVersion": "1.0.0"
        }))
        .unwrap();
        assert_eq!(precheck.precheck_code.as_deref(), Some("READY"));
        assert_eq!(precheck.exists, Some(false));

        let batch: BatchUploadResult = serde_json::from_value(serde_json::json!({
            "succeeded": ["demo"],
            "failed": [{"name": "broken", "reason": "bad zip"}],
            "results": [{"name": "demo", "success": true, "errorCode": "SUCCESS"}],
            "successCount": 1
        }))
        .unwrap();
        assert_eq!(batch.succeeded, vec!["demo".to_string()]);
        assert_eq!(batch.failed[0].reason.as_deref(), Some("bad zip"));
        assert_eq!(batch.results[0].error_code.as_deref(), Some("SUCCESS"));
        assert_eq!(batch.extra["successCount"], 1);
    }

    #[test]
    fn null_and_missing_fields_still_decode() {
        let meta: SkillMeta = serde_json::from_value(serde_json::json!({
            "name": null,
            "versions": null,
            "description": "only description"
        }))
        .unwrap();
        assert_eq!(meta.governance.name, None);
        assert!(meta.versions.is_empty());
        assert_eq!(
            meta.governance.description.as_deref(),
            Some("only description")
        );
    }
}
