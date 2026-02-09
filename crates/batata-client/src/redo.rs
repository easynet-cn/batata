//! Redo service for replaying registrations and subscriptions after reconnect

use std::sync::Arc;

use tracing::{error, info};

use crate::config::BatataConfigService;
use crate::error::Result;
use crate::naming::BatataNamingService;

/// Service that replays all client state after a reconnection.
///
/// On reconnect, the server has lost track of this client's registrations,
/// subscriptions, and config listeners. The RedoService re-establishes them.
pub struct RedoService {
    config_service: Option<Arc<BatataConfigService>>,
    naming_service: Option<Arc<BatataNamingService>>,
}

impl RedoService {
    /// Create a new RedoService.
    pub fn new(
        config_service: Option<Arc<BatataConfigService>>,
        naming_service: Option<Arc<BatataNamingService>>,
    ) -> Self {
        Self {
            config_service,
            naming_service,
        }
    }

    /// Replay all state: config listeners, instance registrations, and subscriptions.
    pub async fn redo_all(&self) -> Result<()> {
        info!("Starting redo after reconnect");

        if let Some(config_service) = &self.config_service
            && let Err(e) = config_service.redo_listeners().await
        {
            error!("Failed to redo config listeners: {}", e);
        }

        if let Some(naming_service) = &self.naming_service
            && let Err(e) = naming_service.redo().await
        {
            error!("Failed to redo naming registrations/subscriptions: {}", e);
        }

        info!("Redo complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redo_service_creation() {
        let redo = RedoService::new(None, None);
        assert!(redo.config_service.is_none());
        assert!(redo.naming_service.is_none());
    }

    #[tokio::test]
    async fn test_redo_all_empty() {
        let redo = RedoService::new(None, None);
        let result = redo.redo_all().await;
        assert!(result.is_ok());
    }
}
