//! TPS checker implementation that bridges batata-core's TpsChecker trait
//! with batata-plugin's ControlPlugin.

use std::sync::Arc;

use batata_core::handler::rpc::{TpsChecker, grpc_tps_point};
use batata_plugin::{ControlContext, ControlPlugin, ExceedAction};
use tonic::Status;

/// TPS checker backed by the control plugin.
pub struct ControlPluginTpsChecker {
    control_plugin: Arc<dyn ControlPlugin>,
}

impl ControlPluginTpsChecker {
    pub fn new(control_plugin: Arc<dyn ControlPlugin>) -> Self {
        Self { control_plugin }
    }
}

#[tonic::async_trait]
impl TpsChecker for ControlPluginTpsChecker {
    async fn check_tps(&self, message_type: &str, client_ip: &str) -> Result<(), Status> {
        let Some(point_name) = grpc_tps_point(message_type) else {
            return Ok(());
        };

        let ctx = ControlContext::new()
            .with_ip(client_ip)
            .with_path(point_name);

        let result = self.control_plugin.check_rate_limit(&ctx).await;

        if !result.allowed {
            match result.action {
                ExceedAction::Warn => {
                    tracing::warn!(
                        point = point_name,
                        client_ip = %client_ip,
                        "gRPC TPS warning: approaching limit (remaining={})",
                        result.remaining
                    );
                    Ok(())
                }
                _ => {
                    tracing::warn!(
                        point = point_name,
                        client_ip = %client_ip,
                        "gRPC TPS control: request rejected (limit={})",
                        result.limit
                    );
                    Err(Status::resource_exhausted(format!(
                        "Over threshold: {}",
                        point_name
                    )))
                }
            }
        } else {
            Ok(())
        }
    }
}
