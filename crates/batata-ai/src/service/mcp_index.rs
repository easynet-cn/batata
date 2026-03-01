// MCP Server Index - DashMap L1 cache backed by config queries
// Provides fast lookup and search for MCP server entries

use dashmap::DashMap;
use tracing::{debug, warn};

use batata_persistence::PersistenceService;

use super::constants::*;
use crate::model::McpServerVersionInfo;

/// Cached index entry for an MCP server
#[derive(Debug, Clone)]
pub struct McpServerIndexData {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub protocol: String,
    pub description: String,
    pub latest_published_version: String,
    pub version_count: usize,
    pub create_time: i64,
    pub modify_time: i64,
}

/// MCP Server Index with DashMap L1 cache
pub struct McpServerIndex {
    /// L1 cache: id -> McpServerIndexData
    by_id: DashMap<String, McpServerIndexData>,
    /// L1 cache: (namespace, name) -> id
    by_name: DashMap<String, DashMap<String, String>>,
}

impl McpServerIndex {
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
    pub fn search_by_name(
        &self,
        namespace: &str,
        name: Option<&str>,
        search_type: &str,
        offset: usize,
        limit: usize,
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
                // Filter by name
                if let Some(n) = name
                    && !n.is_empty()
                {
                    if search_type == "accurate" {
                        return entry.name == n;
                    } else {
                        return entry.name.contains(n);
                    }
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
    pub async fn refresh(&self, persistence: &dyn PersistenceService) {
        // Search all entries in the mcp-server-versions group
        match persistence
            .config_search_page(
                1,
                10000, // Large page to get all entries
                "",    // All namespaces
                "",    // All data IDs
                MCP_SERVER_VERSIONS_GROUP,
                AI_APP_NAME,
                vec![],
                vec![AI_CONFIG_TYPE.to_string()],
                "",
            )
            .await
        {
            Ok(page) => {
                // Clear existing cache
                self.by_id.clear();
                self.by_name.clear();

                for item in page.page_items {
                    // Parse the stored content as McpServerVersionInfo
                    match serde_json::from_str::<McpServerVersionInfo>(&item.content) {
                        Ok(version_info) => {
                            let data = McpServerIndexData {
                                id: version_info.id.clone(),
                                name: version_info.name.clone(),
                                namespace: item.tenant.clone(),
                                protocol: version_info.protocol,
                                description: version_info.description,
                                latest_published_version: version_info.latest_published_version,
                                version_count: version_info.version_details.len(),
                                create_time: item.created_time,
                                modify_time: item.modified_time,
                            };
                            self.upsert(data);
                        }
                        Err(e) => {
                            warn!(
                                data_id = %item.data_id,
                                error = %e,
                                "Failed to parse MCP version info from config"
                            );
                        }
                    }
                }

                debug!(
                    count = self.by_id.len(),
                    "MCP server index refreshed from persistence"
                );
            }
            Err(e) => {
                warn!(error = %e, "Failed to refresh MCP server index from persistence");
            }
        }
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

        let (results, total) = index.search_by_name("public", Some("server"), "blur", 0, 10);
        assert_eq!(total, 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_by_name_accurate() {
        let index = McpServerIndex::new();
        index.upsert(make_index_data("id1", "my-server", "public"));
        index.upsert(make_index_data("id2", "my-server-v2", "public"));

        let (results, total) = index.search_by_name("public", Some("my-server"), "accurate", 0, 10);
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

        let (page1, total) = index.search_by_name("public", None, "blur", 0, 10);
        assert_eq!(total, 25);
        assert_eq!(page1.len(), 10);

        let (page3, _) = index.search_by_name("public", None, "blur", 20, 10);
        assert_eq!(page3.len(), 5);
    }
}
