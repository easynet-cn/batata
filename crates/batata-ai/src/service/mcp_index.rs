// MCP Server Index - DashMap L1 cache backed by config queries
// Provides fast lookup and search for MCP server entries

use dashmap::DashMap;
use tracing::{debug, warn};

use batata_persistence::PersistenceService;

use crate::model::{McpResourceExt, ResourceVersionInfo};
use crate::repository::resource_type;

/// Cached index entry for an MCP server
#[derive(Debug, Clone)]
pub struct McpServerIndexData {
    /// Server id.
    pub id: String,
    /// Server name (identifier).
    pub name: String,
    /// Namespace the server belongs to.
    pub namespace: String,
    /// Protocol identifier (e.g. `"mcp"`).
    pub protocol: String,
    /// Human-readable description.
    pub description: String,
    /// Latest published version string.
    pub latest_published_version: String,
    /// Number of stored versions.
    pub version_count: usize,
    /// Creation time in epoch millis.
    pub create_time: i64,
    /// Last modification time in epoch millis.
    pub modify_time: i64,
    /// Visibility scope (`PUBLIC` / `PRIVATE`), mirrored from `ai_resource.scope`.
    pub scope: String,
    /// Owning user, mirrored from `ai_resource.owner`.
    pub owner: String,
}

/// MCP Server Index with DashMap L1 cache
pub struct McpServerIndex {
    /// L1 cache mapping server id to its index entry.
    by_id: DashMap<String, McpServerIndexData>,
    /// L1 cache mapping `(namespace, name)` to server id.
    by_name: DashMap<String, DashMap<String, String>>,
}

impl McpServerIndex {
    /// Creates an empty index.
    pub fn new() -> Self {
        Self {
            by_id: DashMap::new(),
            by_name: DashMap::new(),
        }
    }

    /// Insert or update a cache entry
    pub fn upsert(&self, data: McpServerIndexData) {
        let ns_map = self.by_name.entry(data.namespace.clone()).or_default();
        ns_map.insert(data.name.clone(), data.id.clone());
        self.by_id.insert(data.id.clone(), data);
    }

    /// Get by ID from cache
    pub fn get_by_id(&self, id: &str) -> Option<McpServerIndexData> {
        self.by_id.get(id).map(|e| e.value().clone())
    }

    /// Get by namespace + name from cache
    pub fn get_by_name(&self, namespace: &str, name: &str) -> Option<McpServerIndexData> {
        let ns_map = self.by_name.get(namespace)?;
        let id = ns_map.get(name)?;
        self.by_id.get(id.value()).map(|e| e.value().clone())
    }

    /// Remove by namespace + name from cache
    pub fn remove_by_name(&self, namespace: &str, name: &str) {
        if let Some(ns_map) = self.by_name.get(namespace)
            && let Some((_, id)) = ns_map.remove(name)
        {
            self.by_id.remove(&id);
        }
    }

    /// Remove by ID from cache
    pub fn remove_by_id(&self, id: &str) {
        if let Some((_, data)) = self.by_id.remove(id)
            && let Some(ns_map) = self.by_name.get(&data.namespace)
        {
            ns_map.remove(&data.name);
        }
    }

    /// Search by name with pagination (from cache)
    ///
    /// `visible` is an optional visibility predicate. It is applied **before**
    /// the total is computed — filtering afterwards would report a total that
    /// includes rows the caller is not allowed to see.
    pub fn search_by_name(
        &self,
        namespace: &str,
        name: Option<&str>,
        search_type: &str,
        offset: usize,
        limit: usize,
        visible: Option<&dyn Fn(&McpServerIndexData) -> bool>,
    ) -> (Vec<McpServerIndexData>, u64) {
        let mut results: Vec<McpServerIndexData> = self
            .by_id
            .iter()
            .map(|e| e.value().clone())
            .filter(|entry| {
                // Filter by namespace
                if !namespace.is_empty() && entry.namespace != namespace {
                    return false;
                }
                // Filter by name. This must not return early: doing so would
                // skip the visibility check below.
                if let Some(n) = name
                    && !n.is_empty()
                {
                    let matches = if search_type == "accurate" {
                        entry.name == n
                    } else {
                        entry.name.contains(n)
                    };
                    if !matches {
                        return false;
                    }
                }
                // Filter by visibility, before pagination
                if let Some(predicate) = visible
                    && !predicate(entry)
                {
                    return false;
                }
                true
            })
            .collect();

        results.sort_by(|a, b| a.name.cmp(&b.name));
        let total = results.len() as u64;

        let page: Vec<McpServerIndexData> = results.into_iter().skip(offset).take(limit).collect();

        (page, total)
    }

    /// Refresh the entire cache from persistence
    ///
    /// MCP servers are stored as `ai_resource` rows of type `mcp`, with the
    /// version index serialized into `version_info`.
    pub async fn refresh(&self, persistence: &dyn PersistenceService) {
        // Namespaces to scan. The default (empty) namespace is included
        // explicitly because `namespace_find_all` does not return it.
        let mut namespaces = vec![String::new()];
        match persistence.namespace_find_all().await {
            Ok(list) => namespaces.extend(list.into_iter().map(|n| n.namespace_id)),
            Err(e) => {
                warn!(error = %e, "Failed to list namespaces for MCP index refresh");
            }
        }

        // Clear existing cache
        self.by_id.clear();
        self.by_name.clear();

        for namespace in namespaces {
            let resources = match persistence
                .ai_resource_find_all(&namespace, resource_type::MCP)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!(
                        error = %e,
                        namespace = %namespace,
                        "Failed to list MCP resources for index refresh"
                    );
                    continue;
                }
            };

            for resource in resources {
                // The MCP server id lives in `ai_resource.ext`.
                let mcp_id = resource
                    .ext
                    .as_deref()
                    .and_then(|json| serde_json::from_str::<McpResourceExt>(json).ok())
                    .map(|e| e.mcp_id)
                    .unwrap_or_default();

                // The shared version index holds the server-managed `latest`
                // label; there is no separate latest field.
                let resource_version = match resource.version_info.as_deref() {
                    Some(json) => match serde_json::from_str::<ResourceVersionInfo>(json) {
                        Ok(rv) => rv,
                        Err(e) => {
                            warn!(
                                resource = %resource.name,
                                error = %e,
                                "Failed to parse MCP version info"
                            );
                            continue;
                        }
                    },
                    None => continue,
                };

                self.upsert(McpServerIndexData {
                    id: mcp_id.clone(),
                    name: resource.name.clone(),
                    namespace: resource.namespace_id.clone(),
                    protocol: "mcp".to_string(),
                    description: String::new(),
                    latest_published_version: resource_version
                        .latest_version()
                        .cloned()
                        .unwrap_or_default(),
                    version_count: resource_version.online_cnt as usize,
                    create_time: 0,
                    modify_time: 0,
                    scope: resource.scope.clone(),
                    owner: resource.owner.clone(),
                });
            }
        }

        debug!(
            count = self.by_id.len(),
            "MCP server index refreshed from persistence"
        );
    }

    /// Get count of cached entries
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

impl Default for McpServerIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_index_data(id: &str, name: &str, namespace: &str) -> McpServerIndexData {
        McpServerIndexData {
            id: id.to_string(),
            name: name.to_string(),
            namespace: namespace.to_string(),
            protocol: "mcp".to_string(),
            description: String::new(),
            latest_published_version: "1.0.0".to_string(),
            version_count: 1,
            create_time: 0,
            modify_time: 0,
            scope: batata_visibility::SCOPE_PUBLIC.to_string(),
            owner: String::new(),
        }
    }

    #[test]
    fn test_upsert_and_get() {
        let index = McpServerIndex::new();
        let data = make_index_data("id1", "server1", "public");
        index.upsert(data);

        assert!(index.get_by_id("id1").is_some());
        assert!(index.get_by_name("public", "server1").is_some());
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn test_remove_by_name() {
        let index = McpServerIndex::new();
        index.upsert(make_index_data("id1", "server1", "public"));
        index.remove_by_name("public", "server1");

        assert!(index.get_by_id("id1").is_none());
        assert!(index.get_by_name("public", "server1").is_none());
    }

    #[test]
    fn test_remove_by_id() {
        let index = McpServerIndex::new();
        index.upsert(make_index_data("id1", "server1", "public"));
        index.remove_by_id("id1");

        assert!(index.get_by_id("id1").is_none());
        assert!(index.get_by_name("public", "server1").is_none());
    }

    #[test]
    fn test_search_by_name_blur() {
        let index = McpServerIndex::new();
        index.upsert(make_index_data("id1", "my-server", "public"));
        index.upsert(make_index_data("id2", "other-server", "public"));
        index.upsert(make_index_data("id3", "my-tool", "public"));

        let (results, total) = index.search_by_name("public", Some("server"), "blur", 0, 10, None);
        assert_eq!(total, 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_by_name_accurate() {
        let index = McpServerIndex::new();
        index.upsert(make_index_data("id1", "my-server", "public"));
        index.upsert(make_index_data("id2", "my-server-v2", "public"));

        let (results, total) =
            index.search_by_name("public", Some("my-server"), "accurate", 0, 10, None);
        assert_eq!(total, 1);
        assert_eq!(results[0].name, "my-server");
    }

    #[test]
    fn test_search_pagination() {
        let index = McpServerIndex::new();
        for i in 0..25 {
            index.upsert(make_index_data(
                &format!("id{}", i),
                &format!("server-{:02}", i),
                "public",
            ));
        }

        let (page1, total) = index.search_by_name("public", None, "blur", 0, 10, None);
        assert_eq!(total, 25);
        assert_eq!(page1.len(), 10);

        let (page3, _) = index.search_by_name("public", None, "blur", 20, 10, None);
        assert_eq!(page3.len(), 5);
    }

    /// The visibility predicate must shrink the **total**, not just the page —
    /// otherwise pagination reports rows the caller cannot see.
    #[test]
    fn test_search_visibility_filters_total() {
        let index = McpServerIndex::new();
        let mut public = make_index_data("id1", "public-server", "public");
        public.scope = batata_visibility::SCOPE_PUBLIC.to_string();
        let mut private = make_index_data("id2", "private-server", "public");
        private.scope = batata_visibility::SCOPE_PRIVATE.to_string();
        index.upsert(public);
        index.upsert(private);

        let visible = |e: &McpServerIndexData| e.scope == batata_visibility::SCOPE_PUBLIC;
        let (page, total) =
            index.search_by_name("public", None, "blur", 0, 10, Some(&visible));

        assert_eq!(total, 1, "total must exclude non-visible rows");
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].name, "public-server");
    }
}
