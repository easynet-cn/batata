//! Configuration filter chain
//!
//! Provides extensible config processing pipeline:
//! - IConfigFilter: Filter interface
//! - ConfigFilterChainManager: Manages filter chain
//! - ConfigRequest/ConfigResponse: Filter context

use dashmap::DashMap;
use std::sync::Arc;

/// Config request for filtering before publishing
#[derive(Debug, Clone)]
pub struct ConfigRequest {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub r#type: String,
    pub encrypted_data_key: String,
    pub addition_map: dashmap::DashMap<String, String>,
}

impl ConfigRequest {
    pub fn new(data_id: String, group: String, tenant: String) -> Self {
        Self {
            data_id,
            group,
            tenant,
            content: String::new(),
            r#type: "text".to_string(),
            encrypted_data_key: String::new(),
            addition_map: DashMap::new(),
        }
    }
}

/// Config response for filtering after querying
#[derive(Debug, Clone)]
pub struct ConfigResponse {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub encrypted_data_key: String,
    pub r#type: String,
    pub addition_map: dashmap::DashMap<String, String>,
}

impl Default for ConfigResponse {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigResponse {
    pub fn new() -> Self {
        Self {
            data_id: String::new(),
            group: String::new(),
            tenant: String::new(),
            content: String::new(),
            encrypted_data_key: String::new(),
            r#type: "text".to_string(),
            addition_map: DashMap::new(),
        }
    }

    pub fn from_request(req: &ConfigRequest) -> Self {
        Self {
            data_id: req.data_id.clone(),
            group: req.group.clone(),
            tenant: req.tenant.clone(),
            content: req.content.clone(),
            encrypted_data_key: req.encrypted_data_key.clone(),
            r#type: req.r#type.clone(),
            addition_map: DashMap::new(),
        }
    }
}

/// Config filter trait for processing config before publish/after query
#[async_trait::async_trait]
pub trait IConfigFilter: Send + Sync {
    /// Get filter order (lower value = higher priority)
    fn order(&self) -> i32;

    /// Get filter name
    fn name(&self) -> &str;

    /// Filter config before publishing
    async fn filter_publish(&self, request: &mut ConfigRequest) -> anyhow::Result<()>;

    /// Filter config after querying
    async fn filter_query(&self, response: &mut ConfigResponse) -> anyhow::Result<()>;
}

/// Config filter chain manager
pub struct ConfigFilterChainManager {
    filters: Vec<Arc<dyn IConfigFilter>>,
}

impl ConfigFilterChainManager {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
        }
    }

    /// Add a filter to the chain
    pub fn add_filter(&mut self, filter: Arc<dyn IConfigFilter>) {
        self.filters.push(filter);
        self.filters.sort_by_key(|f| f.order());
    }

    /// Remove a filter by name
    pub fn remove_filter(&mut self, name: &str) {
        self.filters.retain(|f| f.name() != name);
    }

    /// Get all filters
    pub fn get_filters(&self) -> &Vec<Arc<dyn IConfigFilter>> {
        &self.filters
    }

    /// Do filter before publishing
    pub async fn do_filter_publish(&self, request: &mut ConfigRequest) -> anyhow::Result<()> {
        for filter in &self.filters {
            filter.filter_publish(request).await?;
        }
        Ok(())
    }

    /// Do filter after querying
    pub async fn do_filter_query(&self, response: &mut ConfigResponse) -> anyhow::Result<()> {
        for filter in &self.filters {
            filter.filter_query(response).await?;
        }
        Ok(())
    }
}

impl Default for ConfigFilterChainManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Default config filter that does nothing
pub struct DefaultConfigFilter;

#[async_trait::async_trait]
impl IConfigFilter for DefaultConfigFilter {
    fn order(&self) -> i32 {
        100
    }

    fn name(&self) -> &str {
        "default"
    }

    async fn filter_publish(&self, _request: &mut ConfigRequest) -> anyhow::Result<()> {
        Ok(())
    }

    async fn filter_query(&self, _response: &mut ConfigResponse) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Config filter for logging
pub struct LoggingConfigFilter {
    order: i32,
}

impl LoggingConfigFilter {
    pub fn new(order: i32) -> Self {
        Self { order }
    }
}

#[async_trait::async_trait]
impl IConfigFilter for LoggingConfigFilter {
    fn order(&self) -> i32 {
        self.order
    }

    fn name(&self) -> &str {
        "logging"
    }

    async fn filter_publish(&self, request: &mut ConfigRequest) -> anyhow::Result<()> {
        tracing::debug!(
            "Filtering publish: dataId={}, group={}, tenant={}, content_len={}",
            request.data_id,
            request.group,
            request.tenant,
            request.content.len()
        );
        Ok(())
    }

    async fn filter_query(&self, response: &mut ConfigResponse) -> anyhow::Result<()> {
        tracing::debug!(
            "Filtering query: dataId={}, group={}, tenant={}, content_len={}",
            response.data_id,
            response.group,
            response.tenant,
            response.content.len()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test filter that transforms content to uppercase
    struct UppercaseFilter;

    #[async_trait::async_trait]
    impl IConfigFilter for UppercaseFilter {
        fn order(&self) -> i32 {
            10
        }

        fn name(&self) -> &str {
            "uppercase"
        }

        async fn filter_publish(&self, request: &mut ConfigRequest) -> anyhow::Result<()> {
            request.content = request.content.to_uppercase();
            Ok(())
        }

        async fn filter_query(&self, response: &mut ConfigResponse) -> anyhow::Result<()> {
            response.content = response.content.to_uppercase();
            Ok(())
        }
    }

    /// Test filter that adds prefix
    struct PrefixFilter {
        prefix: String,
    }

    #[async_trait::async_trait]
    impl IConfigFilter for PrefixFilter {
        fn order(&self) -> i32 {
            20
        }

        fn name(&self) -> &str {
            "prefix"
        }

        async fn filter_publish(&self, request: &mut ConfigRequest) -> anyhow::Result<()> {
            request.content = format!("{}{}", self.prefix, request.content);
            Ok(())
        }

        async fn filter_query(&self, _response: &mut ConfigResponse) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_filter_chain_publish() {
        let mut manager = ConfigFilterChainManager::new();
        manager.add_filter(Arc::new(UppercaseFilter));
        manager.add_filter(Arc::new(PrefixFilter {
            prefix: "pref:".to_string(),
        }));

        let mut request = ConfigRequest::new(
            "test".to_string(),
            "group".to_string(),
            "tenant".to_string(),
        );
        request.content = "hello".to_string();

        manager.do_filter_publish(&mut request).await.unwrap();

        assert_eq!(request.content, "pref:HELLO");
    }

    #[tokio::test]
    async fn test_filter_chain_query() {
        let mut manager = ConfigFilterChainManager::new();
        manager.add_filter(Arc::new(UppercaseFilter));

        let mut response = ConfigResponse::new();
        response.data_id = "test".to_string();
        response.group = "group".to_string();
        response.content = "hello".to_string();

        manager.do_filter_query(&mut response).await.unwrap();

        assert_eq!(response.content, "HELLO");
    }

    #[tokio::test]
    async fn test_filter_order() {
        let mut manager = ConfigFilterChainManager::new();
        manager.add_filter(Arc::new(PrefixFilter {
            prefix: "first:".to_string(),
        }));
        manager.add_filter(Arc::new(UppercaseFilter));
        manager.add_filter(Arc::new(PrefixFilter {
            prefix: "second:".to_string(),
        }));

        // Uppercase (order 10) should be before Prefix (order 20)
        assert_eq!(manager.get_filters()[0].order(), 10);
        assert_eq!(manager.get_filters()[1].order(), 20);
        assert_eq!(manager.get_filters()[2].order(), 20);
    }

    #[tokio::test]
    async fn test_remove_filter() {
        let mut manager = ConfigFilterChainManager::new();
        manager.add_filter(Arc::new(UppercaseFilter));
        manager.add_filter(Arc::new(PrefixFilter {
            prefix: "test:".to_string(),
        }));

        assert_eq!(manager.get_filters().len(), 2);

        manager.remove_filter("uppercase");

        assert_eq!(manager.get_filters().len(), 1);
        assert_eq!(manager.get_filters()[0].name(), "prefix");
    }
}
