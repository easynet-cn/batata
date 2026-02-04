//! A2A (Agent-to-Agent) Communication API
//!
//! This module provides HTTP endpoints for AI agent registration,
//! discovery, and communication.

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpResponse, delete, get, post, put, web};
use chrono::Utc;
use dashmap::DashMap;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::model::*;
use crate::model::response::RestResult;

/// A2A Agent Registry
///
/// In-memory registry for AI agents. In production, this would be
/// backed by the database.
pub struct AgentRegistry {
    /// Agents indexed by ID
    agents: DashMap<String, RegisteredAgent>,
    /// Index by namespace -> name -> ID
    name_index: DashMap<String, DashMap<String, String>>,
    /// Index by skill -> IDs
    skill_index: DashMap<String, Vec<String>>,
}

impl AgentRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            agents: DashMap::new(),
            name_index: DashMap::new(),
            skill_index: DashMap::new(),
        }
    }

    /// Register a new agent
    pub fn register(&self, request: AgentRegistrationRequest) -> Result<RegisteredAgent, String> {
        let namespace = &request.namespace;
        let name = &request.card.name;

        // Check if agent already exists
        let ns_index = self
            .name_index
            .entry(namespace.clone())
            .or_insert_with(DashMap::new);

        if ns_index.contains_key(name) {
            return Err(format!(
                "Agent '{}' already exists in namespace '{}'",
                name, namespace
            ));
        }

        // Generate registered agent
        let now = Utc::now().timestamp_millis();
        let agent = RegisteredAgent {
            id: Uuid::new_v4().to_string(),
            card: request.card.clone(),
            namespace: request.namespace.clone(),
            health_status: HealthStatus::Unknown,
            registered_at: now,
            last_health_check: None,
            updated_at: now,
        };

        // Store agent
        let id = agent.id.clone();
        self.agents.insert(id.clone(), agent.clone());
        ns_index.insert(name.clone(), id.clone());

        // Update skill index
        for skill in &request.card.skills {
            self.skill_index
                .entry(skill.name.to_lowercase())
                .or_insert_with(Vec::new)
                .push(id.clone());
        }

        info!(
            agent_name = %name,
            namespace = %namespace,
            agent_id = %agent.id,
            "Agent registered"
        );

        Ok(agent)
    }

    /// Update an existing agent
    pub fn update(
        &self,
        namespace: &str,
        name: &str,
        request: AgentRegistrationRequest,
    ) -> Result<RegisteredAgent, String> {
        let ns_index = self
            .name_index
            .get(namespace)
            .ok_or_else(|| format!("Namespace '{}' not found", namespace))?;

        let id = ns_index
            .get(name)
            .ok_or_else(|| format!("Agent '{}' not found in namespace '{}'", name, namespace))?
            .clone();

        let mut agent = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| "Agent not found".to_string())?;

        // Remove old skills from index
        for skill in &agent.card.skills {
            if let Some(mut ids) = self.skill_index.get_mut(&skill.name.to_lowercase()) {
                ids.retain(|i| i != &id);
            }
        }

        // Update fields
        agent.card = request.card.clone();
        agent.updated_at = Utc::now().timestamp_millis();

        // Add new skills to index
        for skill in &request.card.skills {
            self.skill_index
                .entry(skill.name.to_lowercase())
                .or_insert_with(Vec::new)
                .push(id.clone());
        }

        info!(
            agent_name = %name,
            namespace = %namespace,
            "Agent updated"
        );

        Ok(agent.clone())
    }

    /// Deregister an agent
    pub fn deregister(&self, namespace: &str, name: &str) -> Result<(), String> {
        let ns_index = self
            .name_index
            .get(namespace)
            .ok_or_else(|| format!("Namespace '{}' not found", namespace))?;

        let id = ns_index
            .get(name)
            .ok_or_else(|| format!("Agent '{}' not found in namespace '{}'", name, namespace))?
            .clone();

        // Get agent to remove skills from index
        if let Some((_, agent)) = self.agents.remove(&id) {
            for skill in &agent.card.skills {
                if let Some(mut ids) = self.skill_index.get_mut(&skill.name.to_lowercase()) {
                    ids.retain(|i| i != &id);
                }
            }
        }

        // Remove from name index
        ns_index.remove(name);

        info!(
            agent_name = %name,
            namespace = %namespace,
            "Agent deregistered"
        );

        Ok(())
    }

    /// Get an agent by namespace and name
    pub fn get(&self, namespace: &str, name: &str) -> Option<RegisteredAgent> {
        let ns_index = self.name_index.get(namespace)?;
        let id = ns_index.get(name)?;
        self.agents.get(&*id).map(|a| a.clone())
    }

    /// Get an agent by ID
    pub fn get_by_id(&self, id: &str) -> Option<RegisteredAgent> {
        self.agents.get(id).map(|a| a.clone())
    }

    /// List agents with optional filtering
    pub fn list(&self, query: &AgentQuery) -> AgentListResponse {
        let mut agents: Vec<RegisteredAgent> = self
            .agents
            .iter()
            .map(|e| e.value().clone())
            .filter(|a| {
                // Filter by namespace
                if let Some(ref ns) = query.namespace {
                    if &a.namespace != ns {
                        return false;
                    }
                }

                // Filter by name pattern
                if let Some(ref pattern) = query.name_pattern {
                    if !matches_pattern(&a.card.name, pattern) {
                        return false;
                    }
                }

                // Filter by health status
                if let Some(hs) = query.health_status {
                    if a.health_status != hs {
                        return false;
                    }
                }

                // Filter by tags
                if let Some(ref tags) = query.tags {
                    if !tags.iter().any(|t| a.card.tags.contains(t)) {
                        return false;
                    }
                }

                // Filter by skill
                if let Some(ref skill) = query.skill {
                    if !a
                        .card
                        .skills
                        .iter()
                        .any(|s| s.name.to_lowercase() == skill.to_lowercase())
                    {
                        return false;
                    }
                }

                // Filter by capabilities
                if let Some(streaming) = query.streaming {
                    if a.card.capabilities.streaming != streaming {
                        return false;
                    }
                }

                if let Some(tool_use) = query.tool_use {
                    if a.card.capabilities.tool_use != tool_use {
                        return false;
                    }
                }

                true
            })
            .collect();

        // Sort by name
        agents.sort_by(|a, b| a.card.name.cmp(&b.card.name));

        let total = agents.len() as u64;

        // Pagination (page is 1-indexed, ensure page >= 1)
        let page = query.page.max(1);
        let start = ((page - 1) * query.page_size) as usize;
        let end = (start + query.page_size as usize).min(agents.len());

        let agents = if start < agents.len() {
            agents[start..end].to_vec()
        } else {
            vec![]
        };

        AgentListResponse {
            agents,
            total,
            page: query.page,
            page_size: query.page_size,
        }
    }

    /// Find agents by skill
    pub fn find_by_skill(&self, skill: &str) -> Vec<RegisteredAgent> {
        let skill_lower = skill.to_lowercase();
        if let Some(ids) = self.skill_index.get(&skill_lower) {
            ids.iter()
                .filter_map(|id| self.agents.get(id).map(|a| a.clone()))
                .collect()
        } else {
            vec![]
        }
    }

    /// Batch register agents
    pub fn batch_register(
        &self,
        request: BatchAgentRegistrationRequest,
    ) -> BatchRegistrationResponse {
        let mut success_count = 0u32;
        let mut errors = Vec::new();

        for agent_req in request.agents {
            let name = agent_req.card.name.clone();
            let namespace = agent_req.namespace.clone();

            // Check if exists and handle overwrite
            if self.get(&namespace, &name).is_some() {
                if request.overwrite {
                    match self.update(&namespace, &name, agent_req) {
                        Ok(_) => success_count += 1,
                        Err(e) => errors.push(RegistrationError { name, error: e }),
                    }
                } else {
                    errors.push(RegistrationError {
                        name,
                        error: "Agent already exists".to_string(),
                    });
                }
            } else {
                match self.register(agent_req) {
                    Ok(_) => success_count += 1,
                    Err(e) => errors.push(RegistrationError { name, error: e }),
                }
            }
        }

        BatchRegistrationResponse {
            success_count,
            failed_count: errors.len() as u32,
            errors,
        }
    }

    /// Update agent health status
    pub fn update_health(&self, id: &str, status: HealthStatus) {
        if let Some(mut agent) = self.agents.get_mut(id) {
            agent.health_status = status;
            agent.last_health_check = Some(Utc::now().timestamp_millis());
        }
    }

    /// Get all namespaces
    pub fn namespaces(&self) -> Vec<String> {
        self.name_index.iter().map(|e| e.key().clone()).collect()
    }

    /// Get stats
    pub fn stats(&self) -> AgentRegistryStats {
        let mut by_namespace: HashMap<String, u32> = HashMap::new();
        let mut by_skill: HashMap<String, u32> = HashMap::new();
        let mut healthy = 0u32;
        let mut unhealthy = 0u32;

        for entry in self.agents.iter() {
            let agent = entry.value();
            *by_namespace.entry(agent.namespace.clone()).or_insert(0) += 1;

            for skill in &agent.card.skills {
                *by_skill.entry(skill.name.clone()).or_insert(0) += 1;
            }

            match agent.health_status {
                HealthStatus::Healthy => healthy += 1,
                HealthStatus::Unhealthy => unhealthy += 1,
                _ => {}
            }
        }

        AgentRegistryStats {
            total_agents: self.agents.len() as u32,
            healthy_agents: healthy,
            unhealthy_agents: unhealthy,
            by_namespace,
            by_skill,
        }
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple wildcard pattern matching
fn matches_pattern(name: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }

    if pattern == "*" {
        return true;
    }

    if pattern.starts_with('*') && pattern.ends_with('*') {
        let inner = &pattern[1..pattern.len() - 1];
        return name.contains(inner);
    }

    if pattern.starts_with('*') {
        let suffix = &pattern[1..];
        return name.ends_with(suffix);
    }

    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        return name.starts_with(prefix);
    }

    name == pattern
}

/// Agent Registry statistics
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRegistryStats {
    pub total_agents: u32,
    pub healthy_agents: u32,
    pub unhealthy_agents: u32,
    pub by_namespace: HashMap<String, u32>,
    pub by_skill: HashMap<String, u32>,
}

// =============================================================================
// HTTP Handlers
// =============================================================================

/// Register a new agent
#[post("/v1/ai/a2a/agents")]
pub async fn register_agent(
    registry: web::Data<Arc<AgentRegistry>>,
    body: web::Json<AgentRegistrationRequest>,
) -> HttpResponse {
    debug!(agent_name = %body.card.name, "Registering agent");

    match registry.register(body.into_inner()) {
        Ok(agent) => HttpResponse::Ok().json(RestResult::ok(Some(agent))),
        Err(e) => {
            warn!(error = %e, "Failed to register agent");
            HttpResponse::BadRequest().json(RestResult::<()>::err(400, &e))
        }
    }
}

/// Update an existing agent
#[put("/v1/ai/a2a/agents/{namespace}/{name}")]
pub async fn update_agent(
    registry: web::Data<Arc<AgentRegistry>>,
    path: web::Path<(String, String)>,
    body: web::Json<AgentRegistrationRequest>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();
    debug!(agent_name = %name, namespace = %namespace, "Updating agent");

    match registry.update(&namespace, &name, body.into_inner()) {
        Ok(agent) => HttpResponse::Ok().json(RestResult::ok(Some(agent))),
        Err(e) => {
            warn!(error = %e, "Failed to update agent");
            HttpResponse::NotFound().json(RestResult::<()>::err(404, &e))
        }
    }
}

/// Deregister an agent
#[delete("/v1/ai/a2a/agents/{namespace}/{name}")]
pub async fn deregister_agent(
    registry: web::Data<Arc<AgentRegistry>>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();
    debug!(agent_name = %name, namespace = %namespace, "Deregistering agent");

    match registry.deregister(&namespace, &name) {
        Ok(()) => HttpResponse::Ok().json(RestResult::ok(Some(true))),
        Err(e) => {
            warn!(error = %e, "Failed to deregister agent");
            HttpResponse::NotFound().json(RestResult::<()>::err(404, &e))
        }
    }
}

/// Get an agent by namespace and name
#[get("/v1/ai/a2a/agents/{namespace}/{name}")]
pub async fn get_agent(
    registry: web::Data<Arc<AgentRegistry>>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();

    match registry.get(&namespace, &name) {
        Some(agent) => HttpResponse::Ok().json(RestResult::ok(Some(agent))),
        None => HttpResponse::NotFound().json(RestResult::<()>::err(
            404,
            &format!("Agent '{}' not found in namespace '{}'", name, namespace),
        )),
    }
}

/// List agents
#[get("/v1/ai/a2a/agents")]
pub async fn list_agents(
    registry: web::Data<Arc<AgentRegistry>>,
    query: web::Query<AgentQuery>,
) -> HttpResponse {
    let result = registry.list(&query.into_inner());
    HttpResponse::Ok().json(RestResult::ok(Some(result)))
}

/// Find agents by skill
#[get("/v1/ai/a2a/agents/by-skill/{skill}")]
pub async fn find_agents_by_skill(
    registry: web::Data<Arc<AgentRegistry>>,
    path: web::Path<String>,
) -> HttpResponse {
    let skill = path.into_inner();
    let agents = registry.find_by_skill(&skill);
    HttpResponse::Ok().json(RestResult::ok(Some(agents)))
}

/// Batch register agents
#[post("/v1/ai/a2a/agents/batch")]
pub async fn batch_register_agents(
    registry: web::Data<Arc<AgentRegistry>>,
    body: web::Json<BatchAgentRegistrationRequest>,
) -> HttpResponse {
    debug!("Batch registering agents");

    let result = registry.batch_register(body.into_inner());
    HttpResponse::Ok().json(RestResult::ok(Some(result)))
}

/// Get agent registry statistics
#[get("/v1/ai/a2a/stats")]
pub async fn get_stats(registry: web::Data<Arc<AgentRegistry>>) -> HttpResponse {
    let stats = registry.stats();
    HttpResponse::Ok().json(RestResult::ok(Some(stats)))
}

/// Configure A2A routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(register_agent)
        .service(update_agent)
        .service(deregister_agent)
        .service(get_agent)
        .service(list_agents)
        .service(find_agents_by_skill)
        .service(batch_register_agents)
        .service(get_stats);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_agent_card() -> AgentCard {
        AgentCard {
            name: "test-agent".to_string(),
            display_name: "Test Agent".to_string(),
            description: "A test agent".to_string(),
            version: "1.0.0".to_string(),
            endpoint: "http://localhost:8080".to_string(),
            protocol_version: "1.0".to_string(),
            capabilities: AgentCapabilities {
                streaming: true,
                tool_use: true,
                ..Default::default()
            },
            skills: vec![
                AgentSkill {
                    name: "coding".to_string(),
                    description: "Code generation".to_string(),
                    proficiency: 90,
                    examples: vec![],
                },
                AgentSkill {
                    name: "writing".to_string(),
                    description: "Content writing".to_string(),
                    proficiency: 85,
                    examples: vec![],
                },
            ],
            input_modes: vec![InputMode::Text],
            output_modes: vec![OutputMode::Text],
            authentication: None,
            rate_limits: None,
            metadata: HashMap::new(),
            tags: vec!["test".to_string()],
        }
    }

    fn create_test_registration() -> AgentRegistrationRequest {
        AgentRegistrationRequest {
            card: create_test_agent_card(),
            namespace: "default".to_string(),
        }
    }

    #[test]
    fn test_register_agent() {
        let registry = AgentRegistry::new();
        let request = create_test_registration();

        let agent = registry.register(request).unwrap();
        assert_eq!(agent.card.name, "test-agent");
        assert!(!agent.id.is_empty());
    }

    #[test]
    fn test_get_agent() {
        let registry = AgentRegistry::new();
        let request = create_test_registration();

        registry.register(request).unwrap();

        let agent = registry.get("default", "test-agent").unwrap();
        assert_eq!(agent.card.name, "test-agent");
    }

    #[test]
    fn test_deregister_agent() {
        let registry = AgentRegistry::new();
        let request = create_test_registration();

        registry.register(request).unwrap();
        registry.deregister("default", "test-agent").unwrap();

        assert!(registry.get("default", "test-agent").is_none());
    }

    #[test]
    fn test_list_agents() {
        let registry = AgentRegistry::new();

        // Register multiple agents
        for i in 0..5 {
            let mut req = create_test_registration();
            req.card.name = format!("agent-{}", i);
            registry.register(req).unwrap();
        }

        let result = registry.list(&AgentQuery::default());
        assert_eq!(result.total, 5);
    }

    #[test]
    fn test_find_by_skill() {
        let registry = AgentRegistry::new();

        // Register agents with different skills
        let req1 = create_test_registration();
        registry.register(req1).unwrap();

        let mut req2 = create_test_registration();
        req2.card.name = "agent-2".to_string();
        req2.card.skills = vec![AgentSkill {
            name: "data-analysis".to_string(),
            description: "Data analysis".to_string(),
            proficiency: 80,
            examples: vec![],
        }];
        registry.register(req2).unwrap();

        let coding_agents = registry.find_by_skill("coding");
        assert_eq!(coding_agents.len(), 1);
        assert_eq!(coding_agents[0].card.name, "test-agent");

        let data_agents = registry.find_by_skill("data-analysis");
        assert_eq!(data_agents.len(), 1);
        assert_eq!(data_agents[0].card.name, "agent-2");
    }

    #[test]
    fn test_batch_register() {
        let registry = AgentRegistry::new();

        let agents = (0..3)
            .map(|i| {
                let mut req = create_test_registration();
                req.card.name = format!("agent-{}", i);
                req
            })
            .collect();

        let request = BatchAgentRegistrationRequest {
            agents,
            overwrite: false,
        };

        let result = registry.batch_register(request);
        assert_eq!(result.success_count, 3);
        assert_eq!(result.failed_count, 0);
    }

    #[test]
    fn test_list_with_skill_filter() {
        let registry = AgentRegistry::new();

        let req1 = create_test_registration();
        registry.register(req1).unwrap();

        let result = registry.list(&AgentQuery {
            skill: Some("coding".to_string()),
            ..Default::default()
        });

        assert_eq!(result.total, 1);
    }
}
