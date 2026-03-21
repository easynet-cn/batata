//! Consul Status API HTTP handlers
//!
//! Implements Consul-compatible status endpoints for Raft information.
//! Supports both fixed (fallback) and real cluster information via ServerMemberManager.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;

use batata_core::service::cluster::ServerMemberManager;

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
/// Returns the local node's address with the configured consul port
pub async fn get_leader(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    // Check ACL authorization - status endpoints require agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Return local address with the configured consul port
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let leader = format!("{}:{}", hostname, dc_config.consul_port);
    HttpResponse::Ok().json(leader)
}

/// GET /v1/status/peers (Fixed fallback version)
/// Returns the local node as the only peer
pub async fn get_peers(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    // Check ACL authorization - status endpoints require agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Return local address with the configured consul port
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let peers = vec![format!("{}:{}", hostname, dc_config.consul_port)];
    HttpResponse::Ok().json(peers)
}

// ============================================================================
// Real Cluster Handlers (Using ServerMemberManager)
// ============================================================================

/// Convert Batata member address to Consul-style address with the configured consul port.
/// Batata uses format "ip:port" (e.g., "192.168.1.1:8848")
/// Consul expects address with the consul HTTP port (e.g., "192.168.1.1:8500")
fn to_consul_address(batata_address: &str, consul_port: u16) -> String {
    // Parse the address and replace port with the configured consul port
    if let Some(pos) = batata_address.rfind(':') {
        let ip = &batata_address[..pos];
        format!("{}:{}", ip, consul_port)
    } else {
        // If no port found, assume it's just IP
        format!("{}:{}", batata_address, consul_port)
    }
}

/// GET /v1/status/leader (Real cluster version)
/// Returns the actual Raft leader address from ServerMemberManager
pub async fn get_leader_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    // Check ACL authorization - status endpoints require agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Get real leader address from ServerMemberManager
    let leader = member_manager
        .leader_address()
        .map(|addr| to_consul_address(&addr, dc_config.consul_port))
        .unwrap_or_default();

    HttpResponse::Ok().json(leader)
}

/// GET /v1/status/peers (Real cluster version)
/// Returns the actual Raft peer addresses from ServerMemberManager
pub async fn get_peers_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    // Check ACL authorization - status endpoints require agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Get all healthy members as peers
    let consul_port = dc_config.consul_port;
    let peers: Vec<String> = member_manager
        .healthy_members()
        .iter()
        .map(|m| to_consul_address(&m.address, consul_port))
        .collect();

    // If no healthy members, return at least the local node
    let peers = if peers.is_empty() {
        vec![to_consul_address(
            member_manager.local_address(),
            consul_port,
        )]
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
    fn test_to_consul_address() {
        // Test address conversion with default consul port
        assert_eq!(
            to_consul_address("192.168.1.1:8848", 8500),
            "192.168.1.1:8500"
        );
        assert_eq!(to_consul_address("10.0.0.1:9848", 8500), "10.0.0.1:8500");
        assert_eq!(
            to_consul_address("127.0.0.1:8848", 8500),
            "127.0.0.1:8500"
        );
        // IPv6 address
        assert_eq!(to_consul_address("[::1]:8848", 8500), "[::1]:8500");
        // Custom port
        assert_eq!(
            to_consul_address("192.168.1.1:8848", 9500),
            "192.168.1.1:9500"
        );
    }

    #[test]
    fn test_leader_format() {
        // Leader response should be a quoted IP:port string
        let leader = "127.0.0.1:8500";
        let json = serde_json::to_string(&leader).unwrap();
        assert_eq!(json, "\"127.0.0.1:8500\"");
        assert!(leader.contains(':'));
    }

    #[test]
    fn test_peers_format() {
        // Peers response should be a JSON array of strings
        let peers = vec!["127.0.0.1:8500"];
        let json = serde_json::to_string(&peers).unwrap();
        assert_eq!(json, "[\"127.0.0.1:8500\"]");
        let parsed: Vec<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].contains(':'));
    }

    #[test]
    fn test_to_consul_address_with_port() {
        // Test address conversion with various custom ports
        assert_eq!(to_consul_address("10.0.0.1:3000", 8500), "10.0.0.1:8500");
        assert_eq!(to_consul_address("10.0.0.1:443", 8500), "10.0.0.1:8500");
        // IP only (no port)
        assert_eq!(to_consul_address("10.0.0.1", 8500), "10.0.0.1:8500");
    }
}
