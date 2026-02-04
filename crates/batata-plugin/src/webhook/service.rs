//! Webhook Plugin Service Implementation
//!
//! Provides:
//! - Webhook Plugin SPI (PLG-101)
//! - Config change notification (PLG-102)
//! - Service change notification (PLG-103)
//! - Retry mechanism (PLG-104)

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

use super::model::*;
use crate::Plugin;

/// Webhook Plugin SPI (PLG-101)
#[async_trait]
pub trait WebhookPlugin: Plugin {
    /// Register a new webhook
    async fn register(&self, config: WebhookConfig) -> anyhow::Result<String>;

    /// Update an existing webhook
    async fn update(&self, config: WebhookConfig) -> anyhow::Result<()>;

    /// Delete a webhook
    async fn delete(&self, id: &str) -> anyhow::Result<bool>;

    /// Get a webhook by ID
    async fn get(&self, id: &str) -> anyhow::Result<Option<WebhookConfig>>;

    /// List all webhooks
    async fn list(&self) -> anyhow::Result<Vec<WebhookConfig>>;

    /// Trigger a webhook event (PLG-102, PLG-103)
    async fn trigger(&self, event: WebhookEvent) -> anyhow::Result<Vec<WebhookResult>>;

    /// Get delivery history for a webhook
    async fn get_deliveries(
        &self,
        webhook_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WebhookDelivery>>;

    /// Get webhook statistics
    async fn get_stats(&self) -> WebhookStats;

    /// Retry a failed delivery
    async fn retry_delivery(&self, delivery_id: &str) -> anyhow::Result<WebhookResult>;
}

/// HTTP client for webhook delivery
struct WebhookHttpClient {
    client: reqwest::Client,
}

impl WebhookHttpClient {
    fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    async fn send(
        &self,
        config: &WebhookConfig,
        event: &WebhookEvent,
    ) -> Result<(u16, String), String> {
        let payload = serde_json::to_string(event).map_err(|e| e.to_string())?;

        let mut request = match config.method.to_uppercase().as_str() {
            "PUT" => self.client.put(&config.url),
            _ => self.client.post(&config.url),
        };

        request = request
            .header("Content-Type", &config.content_type)
            .header("X-Webhook-Event", event.event_type.to_string())
            .header("X-Webhook-Delivery", &event.id)
            .header("X-Webhook-Timestamp", event.timestamp.to_string());

        // Add HMAC signature if secret is configured
        if !config.secret.is_empty() {
            let signature = compute_signature(&payload, &config.secret);
            request = request.header("X-Webhook-Signature", format!("sha256={}", signature));
        }

        // Add custom headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        request = request.body(payload);

        let response = request
            .timeout(Duration::from_millis(config.timeout_ms))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| String::new());

        // Truncate body if too large
        let body = if body.len() > 1024 {
            format!("{}...[truncated]", &body[..1024])
        } else {
            body
        };

        if status >= 200 && status < 300 {
            Ok((status, body))
        } else {
            Err(format!("HTTP {}: {}", status, body))
        }
    }
}

/// Compute HMAC-SHA256 signature
fn compute_signature(payload: &str, secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(payload.as_bytes());
    let result = mac.finalize();

    hex::encode(result.into_bytes())
}

/// Default webhook plugin implementation
pub struct DefaultWebhookPlugin {
    /// Webhook configurations
    webhooks: DashMap<String, WebhookConfig>,
    /// Delivery history (limited size)
    deliveries: Arc<DashMap<String, WebhookDelivery>>,
    /// Delivery queue sender
    queue_tx: mpsc::Sender<(WebhookConfig, WebhookEvent)>,
    /// HTTP client
    http_client: Arc<WebhookHttpClient>,
    /// Statistics
    total_events: Arc<AtomicU64>,
    successful_deliveries: Arc<AtomicU64>,
    failed_deliveries: Arc<AtomicU64>,
    total_delivery_time: Arc<AtomicU64>,
    /// Maximum deliveries to keep in history
    #[allow(dead_code)]
    max_deliveries: usize,
}

impl DefaultWebhookPlugin {
    pub fn new() -> Self {
        let (queue_tx, queue_rx) = mpsc::channel::<(WebhookConfig, WebhookEvent)>(10000);
        let http_client = Arc::new(WebhookHttpClient::new());

        let deliveries = Arc::new(DashMap::new());
        let successful = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicU64::new(0));
        let delivery_time = Arc::new(AtomicU64::new(0));

        let plugin = Self {
            webhooks: DashMap::new(),
            deliveries: deliveries.clone(),
            queue_tx,
            http_client: http_client.clone(),
            total_events: Arc::new(AtomicU64::new(0)),
            successful_deliveries: successful.clone(),
            failed_deliveries: failed.clone(),
            total_delivery_time: delivery_time.clone(),
            max_deliveries: 10000,
        };

        // Start background delivery worker

        tokio::spawn(async move {
            delivery_worker(queue_rx, http_client, deliveries, successful, failed, delivery_time).await;
        });

        plugin
    }

    fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    fn matches_event(&self, config: &WebhookConfig, event: &WebhookEvent) -> bool {
        // Check if event type matches
        if !config.event_types.iter().any(|t| *t == event.event_type) {
            return false;
        }

        // Check namespace filter
        if !config.namespace_filter.is_empty()
            && !config.namespace_filter.contains(&event.namespace)
        {
            return false;
        }

        // Check group filter
        if !config.group_filter.is_empty() && !config.group_filter.contains(&event.group) {
            return false;
        }

        true
    }

    async fn deliver_with_retry(
        &self,
        config: &WebhookConfig,
        event: &WebhookEvent,
    ) -> WebhookResult {
        let start = Instant::now();
        let mut attempts = 0;
        #[allow(unused_assignments)]
        let mut last_error: Option<String> = None;

        loop {
            attempts += 1;

            match self.http_client.send(config, event).await {
                Ok((status, body)) => {
                    let duration = start.elapsed().as_millis() as u64;
                    return WebhookResult {
                        success: true,
                        status_code: Some(status),
                        response_body: Some(body),
                        error: None,
                        attempts,
                        duration_ms: duration,
                        webhook_id: config.id.clone(),
                        event_id: event.id.clone(),
                    };
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }

            // Check if we should retry
            if !config.retry.enabled || attempts >= config.retry.max_retries + 1 {
                break;
            }

            // Wait before retry
            let delay = config.retry.calculate_delay(attempts);
            tokio::time::sleep(delay).await;
        }

        let duration = start.elapsed().as_millis() as u64;
        WebhookResult {
            success: false,
            status_code: None,
            response_body: None,
            error: last_error,
            attempts,
            duration_ms: duration,
            webhook_id: config.id.clone(),
            event_id: event.id.clone(),
        }
    }
}

impl Default for DefaultWebhookPlugin {
    fn default() -> Self {
        Self::new()
    }
}

async fn delivery_worker(
    mut queue_rx: mpsc::Receiver<(WebhookConfig, WebhookEvent)>,
    http_client: Arc<WebhookHttpClient>,
    deliveries: Arc<DashMap<String, WebhookDelivery>>,
    successful: Arc<AtomicU64>,
    failed: Arc<AtomicU64>,
    delivery_time: Arc<AtomicU64>,
) {
    while let Some((config, event)) = queue_rx.recv().await {
        let delivery_id = uuid::Uuid::new_v4().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        // Create delivery record
        let mut delivery = WebhookDelivery {
            id: delivery_id.clone(),
            webhook_id: config.id.clone(),
            event_id: event.id.clone(),
            event_type: event.event_type.clone(),
            status: WebhookDeliveryStatus::InProgress,
            attempts: 0,
            last_attempt: now,
            next_retry: None,
            last_error: None,
            created_at: now,
            completed_at: None,
        };

        deliveries.insert(delivery_id.clone(), delivery.clone());

        // Attempt delivery with retry
        let start = Instant::now();
        let mut attempts = 0;
        let mut success = false;

        loop {
            attempts += 1;
            delivery.attempts = attempts;
            delivery.last_attempt = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);

            match http_client.send(&config, &event).await {
                Ok(_) => {
                    success = true;
                    delivery.status = WebhookDeliveryStatus::Delivered;
                    delivery.last_error = None;
                    break;
                }
                Err(e) => {
                    delivery.last_error = Some(e);
                }
            }

            if !config.retry.enabled || attempts >= config.retry.max_retries + 1 {
                delivery.status = WebhookDeliveryStatus::Failed;
                break;
            }

            let delay = config.retry.calculate_delay(attempts);
            delivery.next_retry = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0)
                    + delay.as_millis() as i64,
            );
            deliveries.insert(delivery_id.clone(), delivery.clone());

            tokio::time::sleep(delay).await;
        }

        let duration = start.elapsed().as_millis() as u64;
        delivery.completed_at = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
        );
        delivery.next_retry = None;
        deliveries.insert(delivery_id, delivery);

        if success {
            successful.fetch_add(1, Ordering::Relaxed);
        } else {
            failed.fetch_add(1, Ordering::Relaxed);
        }
        delivery_time.fetch_add(duration, Ordering::Relaxed);
    }
}

#[async_trait]
impl Plugin for DefaultWebhookPlugin {
    fn name(&self) -> &str {
        "webhook"
    }

    async fn init(&self) -> anyhow::Result<()> {
        tracing::info!("Webhook plugin initialized");
        Ok(())
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        tracing::info!("Webhook plugin shutdown");
        Ok(())
    }
}

#[async_trait]
impl WebhookPlugin for DefaultWebhookPlugin {
    async fn register(&self, mut config: WebhookConfig) -> anyhow::Result<String> {
        let now = Self::current_timestamp();

        if config.id.is_empty() {
            config.id = uuid::Uuid::new_v4().to_string();
        }
        config.created_at = now;
        config.updated_at = now;

        let id = config.id.clone();
        self.webhooks.insert(id.clone(), config);

        tracing::info!("Webhook registered: {}", id);
        Ok(id)
    }

    async fn update(&self, mut config: WebhookConfig) -> anyhow::Result<()> {
        if !self.webhooks.contains_key(&config.id) {
            anyhow::bail!("Webhook not found: {}", config.id);
        }

        config.updated_at = Self::current_timestamp();
        self.webhooks.insert(config.id.clone(), config);
        Ok(())
    }

    async fn delete(&self, id: &str) -> anyhow::Result<bool> {
        Ok(self.webhooks.remove(id).is_some())
    }

    async fn get(&self, id: &str) -> anyhow::Result<Option<WebhookConfig>> {
        Ok(self.webhooks.get(id).map(|r| r.clone()))
    }

    async fn list(&self) -> anyhow::Result<Vec<WebhookConfig>> {
        Ok(self.webhooks.iter().map(|r| r.clone()).collect())
    }

    async fn trigger(&self, event: WebhookEvent) -> anyhow::Result<Vec<WebhookResult>> {
        self.total_events.fetch_add(1, Ordering::Relaxed);

        let matching_webhooks: Vec<_> = self
            .webhooks
            .iter()
            .filter(|w| w.enabled && self.matches_event(&w, &event))
            .map(|w| w.clone())
            .collect();

        let mut results = Vec::with_capacity(matching_webhooks.len());

        for webhook in matching_webhooks {
            // Queue for async delivery
            if self.queue_tx.try_send((webhook.clone(), event.clone())).is_err() {
                // Queue full, deliver synchronously
                let result = self.deliver_with_retry(&webhook, &event).await;
                if result.success {
                    self.successful_deliveries.fetch_add(1, Ordering::Relaxed);
                } else {
                    self.failed_deliveries.fetch_add(1, Ordering::Relaxed);
                }
                self.total_delivery_time
                    .fetch_add(result.duration_ms, Ordering::Relaxed);
                results.push(result);
            } else {
                // Return pending result
                results.push(WebhookResult {
                    success: true,
                    webhook_id: webhook.id,
                    event_id: event.id.clone(),
                    ..Default::default()
                });
            }
        }

        Ok(results)
    }

    async fn get_deliveries(
        &self,
        webhook_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WebhookDelivery>> {
        let mut deliveries: Vec<_> = self
            .deliveries
            .iter()
            .filter(|d| d.webhook_id == webhook_id)
            .map(|d| d.clone())
            .collect();

        deliveries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        deliveries.truncate(limit);

        Ok(deliveries)
    }

    async fn get_stats(&self) -> WebhookStats {
        let total = self.webhooks.len() as u32;
        let enabled = self.webhooks.iter().filter(|w| w.enabled).count() as u32;
        let pending = self
            .deliveries
            .iter()
            .filter(|d| d.status == WebhookDeliveryStatus::InProgress)
            .count() as u32;

        let successful = self.successful_deliveries.load(Ordering::Relaxed);
        let total_time = self.total_delivery_time.load(Ordering::Relaxed);
        let avg_time = if successful > 0 {
            total_time / successful
        } else {
            0
        };

        WebhookStats {
            total_webhooks: total,
            enabled_webhooks: enabled,
            total_events: self.total_events.load(Ordering::Relaxed),
            successful_deliveries: successful,
            failed_deliveries: self.failed_deliveries.load(Ordering::Relaxed),
            pending_deliveries: pending,
            avg_delivery_time_ms: avg_time,
        }
    }

    async fn retry_delivery(&self, delivery_id: &str) -> anyhow::Result<WebhookResult> {
        let delivery = self
            .deliveries
            .get(delivery_id)
            .map(|d| d.clone())
            .ok_or_else(|| anyhow::anyhow!("Delivery not found: {}", delivery_id))?;

        let webhook = self
            .webhooks
            .get(&delivery.webhook_id)
            .map(|w| w.clone())
            .ok_or_else(|| anyhow::anyhow!("Webhook not found: {}", delivery.webhook_id))?;

        let event = WebhookEvent {
            id: delivery.event_id,
            event_type: delivery.event_type,
            timestamp: Self::current_timestamp(),
            source: "batata-retry".to_string(),
            namespace: String::new(),
            group: String::new(),
            resource: String::new(),
            data: Default::default(),
            metadata: Default::default(),
        };

        let result = self.deliver_with_retry(&webhook, &event).await;

        if result.success {
            self.successful_deliveries.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_deliveries.fetch_add(1, Ordering::Relaxed);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_delay_calculation() {
        let config = WebhookRetryConfig {
            enabled: true,
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 10000,
            multiplier: 2.0,
            jitter: 0.0, // No jitter for deterministic test
        };

        // First retry (attempt 1)
        let delay1 = config.calculate_delay(1);
        assert_eq!(delay1.as_millis(), 1000);

        // Second retry (attempt 2)
        let delay2 = config.calculate_delay(2);
        assert_eq!(delay2.as_millis(), 2000);

        // Third retry (attempt 3)
        let delay3 = config.calculate_delay(3);
        assert_eq!(delay3.as_millis(), 4000);

        // Fourth retry (attempt 4) - should be capped
        let delay4 = config.calculate_delay(4);
        assert_eq!(delay4.as_millis(), 8000);
    }

    #[test]
    fn test_compute_signature() {
        let payload = r#"{"event":"test"}"#;
        let secret = "test-secret";

        let sig1 = compute_signature(payload, secret);
        let sig2 = compute_signature(payload, secret);

        assert_eq!(sig1, sig2);
        assert!(!sig1.is_empty());
    }

    #[tokio::test]
    async fn test_webhook_plugin_register() {
        let plugin = DefaultWebhookPlugin::new();

        let config = WebhookConfig {
            name: "Test Webhook".to_string(),
            url: "http://localhost:8080/webhook".to_string(),
            event_types: vec![WebhookEventType::ConfigUpdated],
            ..Default::default()
        };

        let id = plugin.register(config).await.unwrap();
        assert!(!id.is_empty());

        let retrieved = plugin.get(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Webhook");
    }

    #[tokio::test]
    async fn test_event_matching() {
        let plugin = DefaultWebhookPlugin::new();

        let config = WebhookConfig {
            id: "test".to_string(),
            enabled: true,
            event_types: vec![WebhookEventType::ConfigUpdated],
            namespace_filter: vec!["public".to_string()],
            ..Default::default()
        };

        // Matching event
        let event1 = WebhookEvent::new(WebhookEventType::ConfigUpdated).with_namespace("public");
        assert!(plugin.matches_event(&config, &event1));

        // Non-matching event type
        let event2 = WebhookEvent::new(WebhookEventType::ConfigCreated).with_namespace("public");
        assert!(!plugin.matches_event(&config, &event2));

        // Non-matching namespace
        let event3 = WebhookEvent::new(WebhookEventType::ConfigUpdated).with_namespace("private");
        assert!(!plugin.matches_event(&config, &event3));
    }
}
