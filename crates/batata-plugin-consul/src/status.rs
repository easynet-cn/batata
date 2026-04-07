//! Consul Status API HTTP handlers
//!
//! Implements Consul-compatible status endpoints for Raft information.
//! Supports both fixed (fallback) and real cluster information via ClusterManager.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;

use batata_common::ClusterManager;

use crate::acl::{AclService, ResourceType};
use crate::model::{ConsulDatacenterConfig, ConsulError};

/// Query parameters for status endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct StatusQueryParams {
    pub dc: Option<String>,
}

// ============================================================================
// Fixed/Fallback Handlers (Original Implementation)
// ============================================================================

/// GET /v1/status/leader (Fixed fallback version)
/// Returns the Raft leader address: "IP:raft_port"
pub async fn get_leader(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Consul returns "IP:raft_port", NOT hostname
    let local_ip = batata_common::local_ip();
    let leader = format!("{}:{}", local_ip, dc_config.raft_port());
    HttpResponse::Ok().json(leader)
}

/// GET /v1/status/peers (Fixed fallback version)
/// Returns the local node as the only peer: "IP:raft_port"
pub async fn get_peers(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let local_ip = batata_common::local_ip();
    let peers = vec![format!("{}:{}", local_ip, dc_config.raft_port())];
    HttpResponse::Ok().json(peers)
}

// ============================================================================
// Real Cluster Handlers (Using ClusterManager)
// ============================================================================

/// Convert Batata member address to Raft address format.
fn to_raft_address(batata_address: &str, raft_port: u16) -> String {
    if let Some(pos) = batata_address.rfind(':') {
        let ip = &batata_address[..pos];
        format!("{}:{}", ip, raft_port)
    } else {
        format!("{}:{}", batata_address, raft_port)
    }
}

/// GET /v1/status/leader (Real cluster version)
/// Returns the actual Raft leader address from ClusterManager
pub async fn get_leader_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<dyn ClusterManager>>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let raft_port = dc_config.raft_port();
    let leader = member_manager
        .leader_address()
        .map(|addr| to_raft_address(&addr, raft_port))
        .unwrap_or_default();

    HttpResponse::Ok().json(leader)
}

/// GET /v1/status/peers (Real cluster version)
/// Returns the actual Raft peer addresses from ClusterManager
pub async fn get_peers_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<dyn ClusterManager>>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let raft_port = dc_config.raft_port();
    let peers: Vec<String> = member_manager
        .healthy_members_extended()
        .iter()
        .map(|m| to_raft_address(&m.address, raft_port))
        .collect();

    let peers = if peers.is_empty() {
        vec![to_raft_address(member_manager.local_address(), raft_port)]
    } else {
        peers
    };

    HttpResponse::Ok().json(peers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_endpoints_exist() {
        // Basic test to ensure the module compiles
    }

    #[test]
    fn test_to_raft_address() {
        assert_eq!(
            to_raft_address("192.168.1.1:8848", 7848),
            "192.168.1.1:7848"
        );
        assert_eq!(to_raft_address("10.0.0.1:9848", 8848), "10.0.0.1:8848");
        assert_eq!(to_raft_address("[::1]:8848", 7848), "[::1]:7848");
        assert_eq!(to_raft_address("10.0.0.1", 7848), "10.0.0.1:7848");
    }

    #[test]
    fn test_leader_format() {
        let leader = "192.168.1.1:7848";
        let json = serde_json::to_string(&leader).unwrap();
        assert_eq!(json, "\"192.168.1.1:7848\"");
        assert!(leader.contains(':'));
    }

    #[test]
    fn test_peers_format() {
        let peers = vec!["192.168.1.1:7848"];
        let json = serde_json::to_string(&peers).unwrap();
        assert_eq!(json, "[\"192.168.1.1:7848\"]");
    }
}
