//! Consul Status API HTTP handlers
//!
//! Implements Consul-compatible status endpoints for Raft information.
//! Supports both fixed (fallback) and real cluster information via ServerMemberManager.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;

use batata_core::service::cluster::ServerMemberManager;

use crate::acl::{AclService, ResourceType};
use crate::model::ConsulError;

/// Query parameters for status endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct StatusQueryParams {
    pub dc: Option<String>,
}

// ============================================================================
// Fixed/Fallback Handlers (Original Implementation)
// ============================================================================

/// GET /v1/status/leader (Fixed fallback version)
/// Returns a fixed Raft leader address
pub async fn get_leader(req: HttpRequest, acl_service: web::Data<AclService>) -> HttpResponse {
    // Check ACL authorization - status endpoints require agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Return fixed leader address (fallback mode)
    let leader = "127.0.0.1:8300";
    HttpResponse::Ok().json(leader)
}

/// GET /v1/status/peers (Fixed fallback version)
/// Returns fixed Raft peer addresses
pub async fn get_peers(req: HttpRequest, acl_service: web::Data<AclService>) -> HttpResponse {
    // Check ACL authorization - status endpoints require agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Return fixed peer address (fallback mode)
    let peers = vec!["127.0.0.1:8300"];
    HttpResponse::Ok().json(peers)
}

// ============================================================================
// Real Cluster Handlers (Using ServerMemberManager)
// ============================================================================

/// Convert Batata member address to Consul-style Raft address
/// Batata uses format "ip:port" (e.g., "192.168.1.1:8848")
/// Consul expects Raft address (e.g., "192.168.1.1:8300")
fn to_consul_raft_address(batata_address: &str) -> String {
    // Parse the address and replace port with Consul Raft port (8300)
    if let Some(pos) = batata_address.rfind(':') {
        let ip = &batata_address[..pos];
        format!("{}:8300", ip)
    } else {
        // If no port found, assume it's just IP
        format!("{}:8300", batata_address)
    }
}

/// GET /v1/status/leader (Real cluster version)
/// Returns the actual Raft leader address from ServerMemberManager
pub async fn get_leader_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
) -> HttpResponse {
    // Check ACL authorization - status endpoints require agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Get real leader address from ServerMemberManager
    let leader = member_manager
        .leader_address()
        .map(|addr| to_consul_raft_address(&addr))
        .unwrap_or_default();

    HttpResponse::Ok().json(leader)
}

/// GET /v1/status/peers (Real cluster version)
/// Returns the actual Raft peer addresses from ServerMemberManager
pub async fn get_peers_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
) -> HttpResponse {
    // Check ACL authorization - status endpoints require agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Get all healthy members as peers
    let peers: Vec<String> = member_manager
        .healthy_members()
        .iter()
        .map(|m| to_consul_raft_address(&m.address))
        .collect();

    // If no healthy members, return at least the local node
    let peers = if peers.is_empty() {
        vec![to_consul_raft_address(member_manager.local_address())]
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
        assert!(true);
    }

    #[test]
    fn test_to_consul_raft_address() {
        // Test address conversion
        assert_eq!(
            to_consul_raft_address("192.168.1.1:8848"),
            "192.168.1.1:8300"
        );
        assert_eq!(to_consul_raft_address("10.0.0.1:9848"), "10.0.0.1:8300");
        assert_eq!(to_consul_raft_address("127.0.0.1:8848"), "127.0.0.1:8300");
        // IPv6 address
        assert_eq!(to_consul_raft_address("[::1]:8848"), "[::1]:8300");
    }
}
