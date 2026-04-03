//! Skills Registry HTTP API — .well-known skill discovery for CLI tools
//!
//! Public endpoints (no authentication required) mounted at `/registry`:
//!
//! - `GET /registry/{namespace_id}/.well-known/skills/index.json`
//! - `GET /registry/{namespace_id}/.well-known/agent-skills/index.json`
//! - `GET /registry/{namespace_id}/api/search`
//! - `GET /registry/{namespace_id}/.well-known/skills/{skill_name}/SKILL.md`
//! - `GET /registry/{namespace_id}/.well-known/agent-skills/{skill_name}/SKILL.md`
//! - `GET /registry/{namespace_id}/.well-known/skills/{skill_name}/{path}`
//! - `GET /registry/{namespace_id}/.well-known/agent-skills/{skill_name}/{path}`

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, get, web};
use serde::{Deserialize, Serialize};
use tracing::debug;

use batata_common::DEFAULT_NAMESPACE_ID;

use crate::service::skill_service::SkillOperationService;

// ============================================================================
// Response models
// ============================================================================

/// Response for `.well-known/skills/index.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WellKnownSkillsIndex {
    pub skills: Vec<WellKnownSkillEntry>,
}

/// Single skill entry in the index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WellKnownSkillEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub files: Vec<String>,
}

/// Response for `/api/search`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsSearchResponse {
    pub skills: Vec<SkillsSearchItem>,
}

/// Single skill in search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsSearchItem {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub installs: i64,
    pub source: String,
}

// ============================================================================
// Query parameters
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default = "default_search_limit")]
    pub limit: u64,
}

fn default_search_limit() -> u64 {
    20
}

// ============================================================================
// Helper functions
// ============================================================================

fn normalize_namespace(ns: &str) -> &str {
    if ns.is_empty() {
        DEFAULT_NAMESPACE_ID
    } else {
        ns
    }
}

/// Build the index of all online skills in a namespace.
///
/// For each enabled skill with at least one online version, resolves the
/// "latest" label to find the online version, then extracts file names from
/// its storage.
async fn build_skills_index(
    skill_service: &SkillOperationService,
    namespace_id: &str,
) -> Result<WellKnownSkillsIndex, actix_web::Error> {
    // List all skills (no limit, get everything for registry index)
    let page = skill_service
        .list_skills(namespace_id, None, None, None, 1, u64::MAX)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut entries = Vec::new();

    for summary in &page.page_items {
        // Only include enabled skills with online versions
        if !summary.enable || summary.online_cnt <= 0 {
            continue;
        }

        // Resolve latest online version via query_skill (uses "latest" label)
        let skill = match skill_service
            .query_skill(&summary.namespace_id, &summary.name, None, None)
            .await
        {
            Ok(Some(s)) => s,
            _ => continue,
        };

        // Collect file names: SKILL.md + resource files
        let mut files = Vec::new();
        if skill.skill_md.is_some() {
            files.push("SKILL.md".to_string());
        }
        for resource in skill.resource.values() {
            files.push(resource.name.clone());
        }

        entries.push(WellKnownSkillEntry {
            name: summary.name.clone(),
            description: summary.description.clone(),
            files,
        });
    }

    Ok(WellKnownSkillsIndex { skills: entries })
}

/// Resolve a file's content from the latest online version of a skill.
///
/// Returns `None` if the skill or file is not found.
async fn resolve_skill_file(
    skill_service: &SkillOperationService,
    namespace_id: &str,
    skill_name: &str,
    file_name: &str,
) -> Result<Option<String>, actix_web::Error> {
    let skill = skill_service
        .query_skill(namespace_id, skill_name, None, None)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let skill = match skill {
        Some(s) => s,
        None => return Ok(None),
    };

    // Check SKILL.md first
    if file_name == "SKILL.md" {
        return Ok(skill.skill_md);
    }

    // Search resource files by name
    for resource in skill.resource.values() {
        if resource.name == file_name {
            return Ok(resource.content.clone());
        }
    }

    Ok(None)
}

/// Build the source URL for a skill pointing back to this registry.
fn build_source_url(req: &HttpRequest, namespace_id: &str, skill_name: &str) -> String {
    let conn_info = req.connection_info();
    let scheme = conn_info.scheme();
    let host = conn_info.host();
    format!(
        "{}://{}/registry/{}/.well-known/skills/{}/SKILL.md",
        scheme, host, namespace_id, skill_name
    )
}

// ============================================================================
// Handlers — .well-known/skills (primary paths)
// ============================================================================

/// GET /registry/{namespace_id}/.well-known/skills/index.json
#[get("/{namespace_id}/.well-known/skills/index.json")]
async fn skills_index(
    path: web::Path<String>,
    skill_service: web::Data<Arc<SkillOperationService>>,
) -> HttpResponse {
    let raw_ns = path.into_inner();
    let namespace_id = normalize_namespace(&raw_ns);
    debug!(namespace_id, "Skills registry: index.json requested");

    match build_skills_index(skill_service.get_ref(), namespace_id).await {
        Ok(index) => HttpResponse::Ok().json(index),
        Err(e) => {
            HttpResponse::InternalServerError().body(format!("Failed to build skills index: {}", e))
        }
    }
}

/// GET /registry/{namespace_id}/.well-known/skills/{skill_name}/SKILL.md
#[get("/{namespace_id}/.well-known/skills/{skill_name}/SKILL.md")]
async fn skills_skill_md(
    path: web::Path<(String, String)>,
    skill_service: web::Data<Arc<SkillOperationService>>,
) -> HttpResponse {
    let (namespace_id, skill_name) = path.into_inner();
    let namespace_id = normalize_namespace(&namespace_id);
    debug!(
        namespace_id,
        skill_name, "Skills registry: SKILL.md requested"
    );

    match resolve_skill_file(
        skill_service.get_ref(),
        namespace_id,
        &skill_name,
        "SKILL.md",
    )
    .await
    {
        Ok(Some(content)) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body(content),
        Ok(None) => HttpResponse::NotFound().body("SKILL.md not found"),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

/// GET /registry/{namespace_id}/.well-known/skills/{skill_name}/{path:.*}
#[get("/{namespace_id}/.well-known/skills/{skill_name}/{path:.*}")]
async fn skills_file(
    path: web::Path<(String, String, String)>,
    skill_service: web::Data<Arc<SkillOperationService>>,
) -> HttpResponse {
    let (namespace_id, skill_name, file_path) = path.into_inner();
    let namespace_id = normalize_namespace(&namespace_id);
    debug!(
        namespace_id,
        skill_name, file_path, "Skills registry: file requested"
    );

    match resolve_skill_file(
        skill_service.get_ref(),
        namespace_id,
        &skill_name,
        &file_path,
    )
    .await
    {
        Ok(Some(content)) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body(content),
        Ok(None) => HttpResponse::NotFound().body(format!("File '{}' not found", file_path)),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

// ============================================================================
// Handlers — .well-known/agent-skills (alias paths)
// ============================================================================

/// GET /registry/{namespace_id}/.well-known/agent-skills/index.json
#[get("/{namespace_id}/.well-known/agent-skills/index.json")]
async fn agent_skills_index(
    path: web::Path<String>,
    skill_service: web::Data<Arc<SkillOperationService>>,
) -> HttpResponse {
    let raw_ns = path.into_inner();
    let namespace_id = normalize_namespace(&raw_ns);
    debug!(
        namespace_id,
        "Skills registry: agent-skills index.json requested"
    );

    match build_skills_index(skill_service.get_ref(), namespace_id).await {
        Ok(index) => HttpResponse::Ok().json(index),
        Err(e) => {
            HttpResponse::InternalServerError().body(format!("Failed to build skills index: {}", e))
        }
    }
}

/// GET /registry/{namespace_id}/.well-known/agent-skills/{skill_name}/SKILL.md
#[get("/{namespace_id}/.well-known/agent-skills/{skill_name}/SKILL.md")]
async fn agent_skills_skill_md(
    path: web::Path<(String, String)>,
    skill_service: web::Data<Arc<SkillOperationService>>,
) -> HttpResponse {
    let (namespace_id, skill_name) = path.into_inner();
    let namespace_id = normalize_namespace(&namespace_id);
    debug!(
        namespace_id,
        skill_name, "Skills registry: agent-skills SKILL.md requested"
    );

    match resolve_skill_file(
        skill_service.get_ref(),
        namespace_id,
        &skill_name,
        "SKILL.md",
    )
    .await
    {
        Ok(Some(content)) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body(content),
        Ok(None) => HttpResponse::NotFound().body("SKILL.md not found"),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

/// GET /registry/{namespace_id}/.well-known/agent-skills/{skill_name}/{path:.*}
#[get("/{namespace_id}/.well-known/agent-skills/{skill_name}/{path:.*}")]
async fn agent_skills_file(
    path: web::Path<(String, String, String)>,
    skill_service: web::Data<Arc<SkillOperationService>>,
) -> HttpResponse {
    let (namespace_id, skill_name, file_path) = path.into_inner();
    let namespace_id = normalize_namespace(&namespace_id);
    debug!(
        namespace_id,
        skill_name, file_path, "Skills registry: agent-skills file requested"
    );

    match resolve_skill_file(
        skill_service.get_ref(),
        namespace_id,
        &skill_name,
        &file_path,
    )
    .await
    {
        Ok(Some(content)) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body(content),
        Ok(None) => HttpResponse::NotFound().body(format!("File '{}' not found", file_path)),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

// ============================================================================
// Handler — search API
// ============================================================================

/// GET /registry/{namespace_id}/api/search
#[get("/{namespace_id}/api/search")]
async fn search_skills(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<SearchQuery>,
    skill_service: web::Data<Arc<SkillOperationService>>,
) -> HttpResponse {
    let raw_ns = path.into_inner();
    let namespace_id = normalize_namespace(&raw_ns);
    let keyword = query.q.as_deref();
    let limit = query.limit.min(100); // Cap at 100 to prevent abuse
    debug!(
        namespace_id,
        ?keyword,
        limit,
        "Skills registry: search requested"
    );

    let page = match skill_service
        .search_skills(namespace_id, keyword, 1, limit)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Failed to search skills: {}", e));
        }
    };

    let skills: Vec<SkillsSearchItem> = page
        .page_items
        .iter()
        .map(|info| {
            let source = build_source_url(&req, &info.namespace_id, &info.name);
            SkillsSearchItem {
                id: format!("{}:{}", info.namespace_id, info.name),
                name: info.name.clone(),
                description: info.description.clone(),
                installs: 0, // SkillBasicInfo does not carry download_count; default to 0
                source,
            }
        })
        .collect();

    HttpResponse::Ok().json(SkillsSearchResponse { skills })
}

// ============================================================================
// Route configuration
// ============================================================================

/// Configure skills registry routes mounted at `/registry`.
///
/// These endpoints are public (no authentication) and intended for CLI tools
/// and other automated consumers that need to discover available skills.
pub fn registry_routes() -> actix_web::Scope {
    web::scope("/registry")
        // Primary .well-known/skills paths
        .service(skills_index)
        .service(skills_skill_md)
        .service(skills_file)
        // Alias .well-known/agent-skills paths
        .service(agent_skills_index)
        .service(agent_skills_skill_md)
        .service(agent_skills_file)
        // Search API
        .service(search_skills)
}
