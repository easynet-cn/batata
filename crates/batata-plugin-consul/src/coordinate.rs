//! Consul Coordinate API
//!
//! Provides network coordinate endpoints for measuring node distances.

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::acl::{AclService, AclServicePersistent, ResourceType};
use crate::model::ConsulError;

// ============================================================================
// Models
// ============================================================================

/// Network coordinate (8-dimensional Vivaldi coordinate)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Coordinate {
    /// Adjustment value
    pub adjustment: f64,
    /// Error estimate
    pub error: f64,
    /// Height component
    pub height: f64,
    /// 8-dimensional vector
    pub vec: Vec<f64>,
}

impl Default for Coordinate {
    fn default() -> Self {
        Self {
            adjustment: 0.0,
            error: 1.5,
            height: 1.0e-05,
            vec: vec![0.0; 8],
        }
    }
}

/// A node's coordinate entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CoordinateEntry {
    /// Node name
    pub node: String,
    /// Network segment
    pub segment: String,
    /// The coordinate
    pub coord: Coordinate,
}

/// Datacenter coordinate map (for WAN coordinates)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DatacenterMap {
    /// Datacenter name
    pub datacenter: String,
    /// Area ID
    #[serde(rename = "AreaID")]
    pub area_id: String,
    /// Coordinates for nodes in this datacenter
    pub coordinates: Vec<CoordinateEntry>,
}

/// Request body for coordinate update
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CoordinateUpdateRequest {
    /// Node name
    pub node: String,
    /// Network segment
    #[serde(default)]
    pub segment: String,
    /// The coordinate to set
    pub coord: Coordinate,
}

/// Query parameters for coordinate endpoints
#[derive(Debug, Deserialize)]
pub struct CoordinateQueryParams {
    /// Datacenter
    pub dc: Option<String>,
    /// Network segment
    pub segment: Option<String>,
}

// ============================================================================
// Service (In-Memory)
// ============================================================================

/// In-memory coordinate service
pub struct ConsulCoordinateService {
    /// Node coordinates: key = "node:segment"
    coordinates: Arc<DashMap<String, CoordinateEntry>>,
    /// Datacenter name
    datacenter: String,
}

impl ConsulCoordinateService {
    pub fn new() -> Self {
        let node_name = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "batata-node".to_string());

        let service = Self {
            coordinates: Arc::new(DashMap::new()),
            datacenter: "dc1".to_string(),
        };

        // Add self with default coordinate
        let entry = CoordinateEntry {
            node: node_name.clone(),
            segment: String::new(),
            coord: Coordinate::default(),
        };
        service.coordinates.insert(format!("{}:", node_name), entry);

        service
    }

    pub fn get_datacenters(&self) -> Vec<DatacenterMap> {
        let coordinates: Vec<CoordinateEntry> =
            self.coordinates.iter().map(|r| r.value().clone()).collect();

        vec![DatacenterMap {
            datacenter: self.datacenter.clone(),
            area_id: "WAN".to_string(),
            coordinates,
        }]
    }

    pub fn get_nodes(&self, segment: Option<&str>) -> Vec<CoordinateEntry> {
        let mut entries: Vec<CoordinateEntry> = self
            .coordinates
            .iter()
            .filter(|r| segment.map(|s| r.value().segment == s).unwrap_or(true))
            .map(|r| r.value().clone())
            .collect();
        entries.sort_by(|a, b| a.node.cmp(&b.node));
        entries
    }

    pub fn get_node(&self, node: &str) -> Option<Vec<CoordinateEntry>> {
        let entries: Vec<CoordinateEntry> = self
            .coordinates
            .iter()
            .filter(|r| r.value().node == node)
            .map(|r| r.value().clone())
            .collect();

        if entries.is_empty() {
            None
        } else {
            Some(entries)
        }
    }

    pub fn update_coordinate(&self, req: CoordinateUpdateRequest) -> Result<(), String> {
        // Validate coordinate dimensions
        if req.coord.vec.len() != 8 {
            return Err("Coordinate must have exactly 8 dimensions".to_string());
        }

        // Check for NaN or Inf
        for v in &req.coord.vec {
            if v.is_nan() || v.is_infinite() {
                return Err("Coordinate contains invalid values".to_string());
            }
        }
        if req.coord.error.is_nan()
            || req.coord.error.is_infinite()
            || req.coord.adjustment.is_nan()
            || req.coord.adjustment.is_infinite()
            || req.coord.height.is_nan()
            || req.coord.height.is_infinite()
        {
            return Err("Coordinate contains invalid values".to_string());
        }

        let key = format!("{}:{}", req.node, req.segment);
        let entry = CoordinateEntry {
            node: req.node,
            segment: req.segment,
            coord: req.coord,
        };
        self.coordinates.insert(key, entry);
        Ok(())
    }
}

impl Default for ConsulCoordinateService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Service (Persistent) - uses ServerMemberManager for real cluster info
// ============================================================================

/// Persistent coordinate service backed by cluster member data
pub struct ConsulCoordinateServicePersistent {
    /// Node coordinates stored in memory (coordinates are ephemeral)
    coordinates: Arc<DashMap<String, CoordinateEntry>>,
    /// Datacenter name
    datacenter: String,
}

impl ConsulCoordinateServicePersistent {
    pub fn new(datacenter: &str) -> Self {
        Self {
            coordinates: Arc::new(DashMap::new()),
            datacenter: datacenter.to_string(),
        }
    }

    pub fn get_datacenters(&self) -> Vec<DatacenterMap> {
        let coordinates: Vec<CoordinateEntry> =
            self.coordinates.iter().map(|r| r.value().clone()).collect();

        vec![DatacenterMap {
            datacenter: self.datacenter.clone(),
            area_id: "WAN".to_string(),
            coordinates,
        }]
    }

    pub fn get_nodes(&self, segment: Option<&str>) -> Vec<CoordinateEntry> {
        let mut entries: Vec<CoordinateEntry> = self
            .coordinates
            .iter()
            .filter(|r| segment.map(|s| r.value().segment == s).unwrap_or(true))
            .map(|r| r.value().clone())
            .collect();
        entries.sort_by(|a, b| a.node.cmp(&b.node));
        entries
    }

    pub fn get_node(&self, node: &str) -> Option<Vec<CoordinateEntry>> {
        let entries: Vec<CoordinateEntry> = self
            .coordinates
            .iter()
            .filter(|r| r.value().node == node)
            .map(|r| r.value().clone())
            .collect();

        if entries.is_empty() {
            None
        } else {
            Some(entries)
        }
    }

    pub fn update_coordinate(&self, req: CoordinateUpdateRequest) -> Result<(), String> {
        if req.coord.vec.len() != 8 {
            return Err("Coordinate must have exactly 8 dimensions".to_string());
        }

        for v in &req.coord.vec {
            if v.is_nan() || v.is_infinite() {
                return Err("Coordinate contains invalid values".to_string());
            }
        }
        if req.coord.error.is_nan()
            || req.coord.error.is_infinite()
            || req.coord.adjustment.is_nan()
            || req.coord.adjustment.is_infinite()
            || req.coord.height.is_nan()
            || req.coord.height.is_infinite()
        {
            return Err("Coordinate contains invalid values".to_string());
        }

        let key = format!("{}:{}", req.node, req.segment);
        let entry = CoordinateEntry {
            node: req.node,
            segment: req.segment,
            coord: req.coord,
        };
        self.coordinates.insert(key, entry);
        Ok(())
    }
}

// ============================================================================
// HTTP Handlers (In-Memory)
// ============================================================================

/// GET /v1/coordinate/datacenters - List WAN coordinates
pub async fn get_coordinate_datacenters(
    coord_service: web::Data<ConsulCoordinateService>,
) -> HttpResponse {
    HttpResponse::Ok().json(coord_service.get_datacenters())
}

/// GET /v1/coordinate/nodes - List LAN coordinates for nodes
pub async fn get_coordinate_nodes(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    coord_service: web::Data<ConsulCoordinateService>,
    query: web::Query<CoordinateQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Node, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let entries = coord_service.get_nodes(query.segment.as_deref());
    HttpResponse::Ok().json(entries)
}

/// GET /v1/coordinate/node/{node} - Read a node's coordinates
pub async fn get_coordinate_node(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    coord_service: web::Data<ConsulCoordinateService>,
    path: web::Path<String>,
    _query: web::Query<CoordinateQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Node, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let node = path.into_inner();
    match coord_service.get_node(&node) {
        Some(entries) => HttpResponse::Ok().json(entries),
        None => {
            HttpResponse::NotFound().json(ConsulError::new(format!("Node '{}' not found", node)))
        }
    }
}

/// PUT /v1/coordinate/update - Update node coordinates
pub async fn update_coordinate(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    coord_service: web::Data<ConsulCoordinateService>,
    body: web::Json<CoordinateUpdateRequest>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Node, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    match coord_service.update_coordinate(body.into_inner()) {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ConsulError::new(e)),
    }
}

// ============================================================================
// HTTP Handlers (Persistent)
// ============================================================================

/// GET /v1/coordinate/datacenters (persistent)
pub async fn get_coordinate_datacenters_persistent(
    coord_service: web::Data<ConsulCoordinateServicePersistent>,
) -> HttpResponse {
    HttpResponse::Ok().json(coord_service.get_datacenters())
}

/// GET /v1/coordinate/nodes (persistent)
pub async fn get_coordinate_nodes_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclServicePersistent>,
    coord_service: web::Data<ConsulCoordinateServicePersistent>,
    query: web::Query<CoordinateQueryParams>,
) -> HttpResponse {
    let authz = acl_service
        .authorize_request(&req, ResourceType::Node, "", false)
        .await;
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let entries = coord_service.get_nodes(query.segment.as_deref());
    HttpResponse::Ok().json(entries)
}

/// GET /v1/coordinate/node/{node} (persistent)
pub async fn get_coordinate_node_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclServicePersistent>,
    coord_service: web::Data<ConsulCoordinateServicePersistent>,
    path: web::Path<String>,
    _query: web::Query<CoordinateQueryParams>,
) -> HttpResponse {
    let authz = acl_service
        .authorize_request(&req, ResourceType::Node, "", false)
        .await;
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let node = path.into_inner();
    match coord_service.get_node(&node) {
        Some(entries) => HttpResponse::Ok().json(entries),
        None => {
            HttpResponse::NotFound().json(ConsulError::new(format!("Node '{}' not found", node)))
        }
    }
}

/// PUT /v1/coordinate/update (persistent)
pub async fn update_coordinate_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclServicePersistent>,
    coord_service: web::Data<ConsulCoordinateServicePersistent>,
    body: web::Json<CoordinateUpdateRequest>,
) -> HttpResponse {
    let authz = acl_service
        .authorize_request(&req, ResourceType::Node, "", true)
        .await;
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    match coord_service.update_coordinate(body.into_inner()) {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ConsulError::new(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_coordinate() {
        let coord = Coordinate::default();
        assert_eq!(coord.vec.len(), 8);
        assert_eq!(coord.error, 1.5);
        assert_eq!(coord.adjustment, 0.0);
    }

    #[test]
    fn test_coordinate_service_new() {
        let service = ConsulCoordinateService::new();
        let nodes = service.get_nodes(None);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].coord.vec.len(), 8);
    }

    #[test]
    fn test_get_datacenters() {
        let service = ConsulCoordinateService::new();
        let dcs = service.get_datacenters();
        assert_eq!(dcs.len(), 1);
        assert_eq!(dcs[0].datacenter, "dc1");
        assert_eq!(dcs[0].area_id, "WAN");
    }

    #[test]
    fn test_update_coordinate() {
        let service = ConsulCoordinateService::new();
        let result = service.update_coordinate(CoordinateUpdateRequest {
            node: "test-node".to_string(),
            segment: String::new(),
            coord: Coordinate {
                adjustment: 0.1,
                error: 0.5,
                height: 0.001,
                vec: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            },
        });
        assert!(result.is_ok());

        let entries = service.get_node("test-node");
        assert!(entries.is_some());
        assert_eq!(entries.unwrap()[0].coord.vec[0], 1.0);
    }

    #[test]
    fn test_invalid_coordinate_dimensions() {
        let service = ConsulCoordinateService::new();
        let result = service.update_coordinate(CoordinateUpdateRequest {
            node: "test-node".to_string(),
            segment: String::new(),
            coord: Coordinate {
                adjustment: 0.0,
                error: 1.0,
                height: 0.0,
                vec: vec![1.0, 2.0], // Wrong dimensions
            },
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_coordinate_nan() {
        let service = ConsulCoordinateService::new();
        let result = service.update_coordinate(CoordinateUpdateRequest {
            node: "test-node".to_string(),
            segment: String::new(),
            coord: Coordinate {
                adjustment: 0.0,
                error: f64::NAN,
                height: 0.0,
                vec: vec![0.0; 8],
            },
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_get_node_not_found() {
        let service = ConsulCoordinateService::new();
        assert!(service.get_node("nonexistent").is_none());
    }

    #[test]
    fn test_node_segment_filter() {
        let service = ConsulCoordinateService::new();
        service
            .update_coordinate(CoordinateUpdateRequest {
                node: "node-a".to_string(),
                segment: "alpha".to_string(),
                coord: Coordinate::default(),
            })
            .unwrap();
        service
            .update_coordinate(CoordinateUpdateRequest {
                node: "node-b".to_string(),
                segment: "beta".to_string(),
                coord: Coordinate::default(),
            })
            .unwrap();

        let alpha = service.get_nodes(Some("alpha"));
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha[0].node, "node-a");

        let all = service.get_nodes(None);
        assert!(all.len() >= 3); // self + node-a + node-b
    }

    #[test]
    fn test_update_coordinate_replaces_existing() {
        let service = ConsulCoordinateService::new();

        service
            .update_coordinate(CoordinateUpdateRequest {
                node: "node-x".to_string(),
                segment: String::new(),
                coord: Coordinate {
                    adjustment: 0.1,
                    error: 0.5,
                    height: 0.001,
                    vec: vec![1.0; 8],
                },
            })
            .unwrap();

        // Update same node
        service
            .update_coordinate(CoordinateUpdateRequest {
                node: "node-x".to_string(),
                segment: String::new(),
                coord: Coordinate {
                    adjustment: 0.2,
                    error: 0.3,
                    height: 0.002,
                    vec: vec![2.0; 8],
                },
            })
            .unwrap();

        let entries = service.get_node("node-x").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].coord.vec[0], 2.0);
        assert_eq!(entries[0].coord.adjustment, 0.2);
    }

    #[test]
    fn test_invalid_coordinate_infinity() {
        let service = ConsulCoordinateService::new();
        let result = service.update_coordinate(CoordinateUpdateRequest {
            node: "bad-node".to_string(),
            segment: String::new(),
            coord: Coordinate {
                adjustment: f64::INFINITY,
                error: 1.0,
                height: 0.0,
                vec: vec![0.0; 8],
            },
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_coordinate_vec_infinity() {
        let service = ConsulCoordinateService::new();
        let result = service.update_coordinate(CoordinateUpdateRequest {
            node: "inf-vec".to_string(),
            segment: String::new(),
            coord: Coordinate {
                adjustment: 0.0,
                error: 1.0,
                height: 0.0,
                vec: vec![0.0, 0.0, f64::INFINITY, 0.0, 0.0, 0.0, 0.0, 0.0],
            },
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_get_nodes_sorted() {
        let service = ConsulCoordinateService::new();

        service
            .update_coordinate(CoordinateUpdateRequest {
                node: "zzz-node".to_string(),
                segment: String::new(),
                coord: Coordinate::default(),
            })
            .unwrap();
        service
            .update_coordinate(CoordinateUpdateRequest {
                node: "aaa-node".to_string(),
                segment: String::new(),
                coord: Coordinate::default(),
            })
            .unwrap();

        let nodes = service.get_nodes(None);
        // Should be sorted by node name
        for i in 1..nodes.len() {
            assert!(nodes[i].node >= nodes[i - 1].node);
        }
    }

    #[test]
    fn test_persistent_service_basic() {
        let persistent = ConsulCoordinateServicePersistent::new("dc1");
        assert!(persistent.get_nodes(None).is_empty());

        persistent
            .update_coordinate(CoordinateUpdateRequest {
                node: "persistent-node".to_string(),
                segment: String::new(),
                coord: Coordinate::default(),
            })
            .unwrap();

        assert_eq!(persistent.get_nodes(None).len(), 1);

        let dcs = persistent.get_datacenters();
        assert_eq!(dcs[0].datacenter, "dc1");
    }

    #[test]
    fn test_node_multiple_segments() {
        let service = ConsulCoordinateService::new();

        service
            .update_coordinate(CoordinateUpdateRequest {
                node: "multi-seg".to_string(),
                segment: "lan".to_string(),
                coord: Coordinate::default(),
            })
            .unwrap();
        service
            .update_coordinate(CoordinateUpdateRequest {
                node: "multi-seg".to_string(),
                segment: "wan".to_string(),
                coord: Coordinate::default(),
            })
            .unwrap();

        let entries = service.get_node("multi-seg").unwrap();
        assert_eq!(entries.len(), 2);
    }
}
