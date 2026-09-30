//! Version lifecycle shared by every AI resource type.
//!
//! The draft → reviewing → online → offline state machine, the editing /
//! reviewing / latest markers and the label handling are identical for MCP
//! servers, agents, skills, prompts and agent specs — upstream drives them all
//! from the same `ai_resource` / `ai_resource_version` tables. This module
//! holds that logic once, parameterised by resource type, so a new domain does
//! not re-implement (and re-invent) the transitions.
//!
//! It was extracted from `McpServerOperationService`, so the rules here are the
//! ones the MCP route tests already pin down.

use std::collections::HashMap;

use batata_common::model::ai::ResourceVersionInfo;
use batata_persistence::model::AiResourceInfo;
use batata_persistence::PersistenceService;

use crate::repository::{resource_type, version_status};

/// Parse the resource-level version index, defaulting when absent.
pub fn parse_version_info(resource: &AiResourceInfo) -> ResourceVersionInfo {
    match resource.version_info {
        Some(ref json) => serde_json::from_str::<ResourceVersionInfo>(json).unwrap_or_default(),
        None => ResourceVersionInfo::default(),
    }
}

/// Find the resource, or fail naming the resource type.
pub async fn find_resource(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
) -> anyhow::Result<AiResourceInfo> {
    persistence
        .ai_resource_find(namespace, name, rt)
        .await?
        .ok_or_else(|| anyhow::anyhow!("{} '{}' not found in namespace '{}'", rt, name, namespace))
}

/// Find one version row, or fail naming the resource type.
pub async fn find_version(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    version: &str,
) -> anyhow::Result<batata_persistence::model::AiResourceVersionInfo> {
    persistence
        .ai_resource_version_find(namespace, name, rt, version)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Version '{}' of {} '{}' not found", version, rt, name))
}

/// Persist the resource-level version index with an optimistic lock.
pub async fn save_version_info(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    resource: &AiResourceInfo,
    version_info: &ResourceVersionInfo,
) -> anyhow::Result<()> {
    persistence
        .ai_resource_update_version_info_cas(
            namespace,
            name,
            rt,
            resource.meta_version,
            &serde_json::to_string(version_info)?,
            resource.meta_version + 1,
        )
        .await?;
    Ok(())
}

/// Move `version` to `target` and reconcile the resource-level markers.
///
/// Handles the status update and the `editing_version` / `reviewing_version`
/// bookkeeping. Domain-specific follow-up work — rebuilding a search index,
/// refreshing derived metadata — is the caller's, because only the caller knows
/// what it keeps.
pub async fn transition_status(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    version: &str,
    target: &str,
) -> anyhow::Result<()> {
    let resource = find_resource(persistence, namespace, name, rt).await?;

    persistence
        .ai_resource_version_update_status(namespace, name, rt, version, target)
        .await?;

    let mut version_info = parse_version_info(&resource);
    match target {
        version_status::DRAFT => {
            version_info.editing_version = Some(version.to_string());
            if version_info.reviewing_version.as_deref() == Some(version) {
                version_info.reviewing_version = None;
            }
            save_version_info(persistence, namespace, name, rt, &resource, &version_info).await?;
        }
        version_status::REVIEWING => {
            version_info.editing_version = None;
            version_info.reviewing_version = Some(version.to_string());
            save_version_info(persistence, namespace, name, rt, &resource, &version_info).await?;
        }
        version_status::ONLINE => {
            version_info.editing_version = None;
            version_info.reviewing_version = None;
            save_version_info(persistence, namespace, name, rt, &resource, &version_info).await?;
            // The online set grew, so `latest` and `online_cnt` move with it.
            refresh_latest(persistence, namespace, name, rt, Some(version)).await?;
        }
        version_status::OFFLINE => {
            save_version_info(persistence, namespace, name, rt, &resource, &version_info).await?;
            // The version left the online set, so `latest` may have to move to
            // another version.
            refresh_latest(persistence, namespace, name, rt, None).await?;
        }
        _ => {
            save_version_info(persistence, namespace, name, rt, &resource, &version_info).await?;
        }
    }
    Ok(())
}

/// Recompute `online_cnt` and the server-managed `latest` label.
///
/// The resource is re-read on purpose: callers usually update the version
/// index just before this, which advances `meta_version`, and writing back a
/// stale snapshot would make the optimistic-lock update fail silently.
///
/// `preferred` is the version that just went online, if any. Otherwise the
/// current `latest` is kept while it is still online, and the highest online
/// version is used once it is gone.
pub async fn refresh_latest(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    preferred: Option<&str>,
) -> anyhow::Result<()> {
    let resource = match persistence.ai_resource_find(namespace, name, rt).await? {
        Some(r) => r,
        None => return Ok(()),
    };

    let rows = persistence.ai_resource_version_list(namespace, name, rt).await?;
    let mut online: Vec<String> = rows
        .iter()
        .filter(|r| r.status == version_status::ONLINE)
        .map(|r| r.version.clone())
        .collect();
    online.sort();

    let mut version_info = parse_version_info(&resource);
    version_info.online_cnt = online.len() as i64;

    let current = version_info.latest_version().cloned();
    let next = match preferred {
        Some(v) if online.iter().any(|o| o == v) => Some(v.to_string()),
        _ => match current {
            Some(ref v) if online.contains(v) => Some(v.clone()),
            _ => online.last().cloned(),
        },
    };

    match next {
        Some(v) => version_info.set_latest(&v),
        None => version_info.clear_latest(),
    }

    save_version_info(persistence, namespace, name, rt, &resource, &version_info).await
}

/// Assert `version` is in one of `allowed` before a transition.
fn require_status(version: &str, current: &str, allowed: &[&str], action: &str) -> anyhow::Result<()> {
    if !allowed.contains(&current) {
        anyhow::bail!(
            "Version '{}' must be in {} status to {} (current: '{}')",
            version,
            allowed.join(", "),
            action,
            current
        );
    }
    Ok(())
}

/// Submit a draft for review (draft → reviewing).
pub async fn submit(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    version: &str,
) -> anyhow::Result<()> {
    let row = find_version(persistence, namespace, name, rt, version).await?;
    require_status(version, &row.status, &[version_status::DRAFT], "submit")?;
    transition_status(persistence, namespace, name, rt, version, version_status::REVIEWING).await
}

/// Publish a version that passed review (reviewing / reviewed → online).
///
/// Publishing an already-online version is idempotent.
pub async fn publish(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    version: &str,
) -> anyhow::Result<()> {
    let row = find_version(persistence, namespace, name, rt, version).await?;
    require_status(
        version,
        &row.status,
        &[
            version_status::REVIEWING,
            version_status::REVIEWED,
            version_status::ONLINE,
        ],
        "publish",
    )?;
    transition_status(persistence, namespace, name, rt, version, version_status::ONLINE).await
}

/// Publish a version bypassing the review gate.
pub async fn force_publish(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    version: &str,
) -> anyhow::Result<()> {
    find_version(persistence, namespace, name, rt, version).await?;
    transition_status(persistence, namespace, name, rt, version, version_status::ONLINE).await
}

/// Move a version back to draft so it can be edited again.
pub async fn redraft(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    version: &str,
) -> anyhow::Result<()> {
    find_version(persistence, namespace, name, rt, version).await?;
    transition_status(persistence, namespace, name, rt, version, version_status::DRAFT).await
}

/// Bring an offline version back online.
pub async fn online(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    version: &str,
) -> anyhow::Result<()> {
    let row = find_version(persistence, namespace, name, rt, version).await?;
    require_status(version, &row.status, &[version_status::OFFLINE], "bring online")?;
    transition_status(persistence, namespace, name, rt, version, version_status::ONLINE).await
}

/// Take an online version offline.
pub async fn offline(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    version: &str,
) -> anyhow::Result<()> {
    let row = find_version(persistence, namespace, name, rt, version).await?;
    require_status(version, &row.status, &[version_status::ONLINE], "take offline")?;
    transition_status(persistence, namespace, name, rt, version, version_status::OFFLINE).await
}

/// Replace the custom labels, preserving the server-managed `latest` label.
///
/// Mirrors upstream `validateAndUpdateLabels`: every custom label must point at
/// a version that is currently online. Returns the stored labels.
pub async fn update_labels(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    labels: HashMap<String, String>,
) -> anyhow::Result<HashMap<String, String>> {
    let resource = find_resource(persistence, namespace, name, rt).await?;

    let rows = persistence.ai_resource_version_list(namespace, name, rt).await?;
    let online: std::collections::HashSet<&str> = rows
        .iter()
        .filter(|r| r.status == version_status::ONLINE)
        .map(|r| r.version.as_str())
        .collect();

    for (label, version) in &labels {
        if !online.contains(version.as_str()) {
            anyhow::bail!(
                "Label '{}' points to version '{}' which is not online",
                label,
                version
            );
        }
    }

    let mut version_info = parse_version_info(&resource);
    let latest = version_info.latest_version().cloned();
    version_info.labels = labels;
    if let Some(latest) = latest {
        version_info.set_latest(&latest);
    }
    save_version_info(persistence, namespace, name, rt, &resource, &version_info).await?;
    Ok(version_info.labels.clone())
}

/// Change the visibility scope.
pub async fn set_scope(
    persistence: &dyn PersistenceService,
    namespace: &str,
    name: &str,
    rt: &str,
    scope: &str,
) -> anyhow::Result<()> {
    let resource = find_resource(persistence, namespace, name, rt).await?;
    persistence
        .ai_resource_update_scope(namespace, &resource.name, rt, scope)
        .await
}

/// The resource types this lifecycle applies to.
pub fn known_types() -> [&'static str; 5] {
    [
        resource_type::MCP,
        resource_type::AGENT,
        resource_type::SKILL,
        resource_type::PROMPT,
        resource_type::AGENT_SPEC,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_type_is_distinct() {
        let types = known_types();
        let unique: std::collections::HashSet<_> = types.iter().collect();
        assert_eq!(unique.len(), types.len());
    }

    #[test]
    fn status_messages_name_the_allowed_states() {
        assert!(require_status("1.0.0", "online", &["draft"], "submit").is_err());
        assert!(require_status("1.0.0", "draft", &["draft"], "submit").is_ok());
        let err = require_status("1.0.0", "online", &["draft"], "submit").unwrap_err();
        assert!(err.to_string().contains("must be in draft status to submit"));
        assert!(err.to_string().contains("current: 'online'"));
    }

    #[test]
    fn an_empty_version_info_parses_to_defaults() {
        let resource = AiResourceInfo::default();
        let info = parse_version_info(&resource);
        assert!(info.editing_version.is_none());
        assert!(info.labels.is_empty());
    }
}
