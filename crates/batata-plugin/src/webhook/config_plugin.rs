//! Webhook Config Change Plugin — ConfigChangePluginV2 adapter for webhook notifications
//!
//! Bridges the enhanced config change plugin chain with the webhook delivery system.
//! Sends webhook notifications on config publish/remove operations.

use std::sync::Arc;

use crate::spi::{
    ConfigChangePluginV2, ConfigChangeRequest, ConfigChangeResult, ConfigPointcut, ExecuteType,
};
use crate::webhook::{WebhookEvent, WebhookEventType, WebhookPlugin};

/// Config change plugin that triggers webhooks on config operations.
pub struct WebhookConfigChangePlugin {
    webhook_plugin: Arc<dyn WebhookPlugin>,
}

impl WebhookConfigChangePlugin {
    pub fn new(webhook_plugin: Arc<dyn WebhookPlugin>) -> Self {
        Self { webhook_plugin }
    }

    fn build_event(&self, ctx: &ConfigChangeRequest) -> Option<WebhookEvent> {
        let event_type = match (ctx.pointcut, ctx.execute_type) {
            (ConfigPointcut::Publish, ExecuteType::After) => Some(WebhookEventType::ConfigUpdated),
            (ConfigPointcut::Remove, ExecuteType::After) => Some(WebhookEventType::ConfigDeleted),
            (ConfigPointcut::Import, ExecuteType::After) => Some(WebhookEventType::ConfigCreated),
            _ => None,
        }?;

        let mut event = WebhookEvent::new(event_type)
            .with_namespace(&ctx.tenant)
            .with_group(&ctx.group)
            .with_resource(&ctx.data_id)
            .with_data("content_length", serde_json::json!(ctx.content.len()));

        if let Some(ref content_type) = ctx.content_type {
            event = event.with_metadata("content_type", content_type);
        }
        if let Some(ref client_ip) = ctx.client_ip {
            event = event.with_metadata("client_ip", client_ip);
        }
        event = event.with_metadata("operator", &ctx.operator);

        Some(event)
    }
}

#[async_trait::async_trait]
impl ConfigChangePluginV2 for WebhookConfigChangePlugin {
    fn name(&self) -> &str {
        "webhook-config-change"
    }

    fn order(&self) -> i32 {
        200 // Run after audit plugin
    }

    fn interested_pointcuts(&self) -> Vec<ConfigPointcut> {
        vec![ConfigPointcut::Publish, ConfigPointcut::Remove, ConfigPointcut::Import]
    }

    fn interested_execute_types(&self) -> Vec<ExecuteType> {
        vec![ExecuteType::After] // Only notify after successful operations
    }

    async fn execute(&self, ctx: &mut ConfigChangeRequest) -> ConfigChangeResult {
        if let Some(event) = self.build_event(ctx) {
            if let Err(e) = self.webhook_plugin.trigger(event).await {
                tracing::warn!(
                    target: "webhook_config_plugin",
                    "Failed to trigger webhook for config change: {}",
                    e
                );
            }
        }
        ConfigChangeResult::allow()
    }
}
